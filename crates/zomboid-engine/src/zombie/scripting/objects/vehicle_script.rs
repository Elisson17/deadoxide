//! zombie.scripting.objects.VehicleScript: a `vehicle` script as a dedicated server loads it (`Load`, with `template`
//! and `template!`, then `Loaded`). `toBullet` (skipped when GameServer.server) and the skin textures (loaded by the
//! client only) are not ported; the Texture fields of a Skin are always null on a server.
use super::model_attachment::ModelAttachment;
use super::vehicle_template::{Found, TemplateContext, VehicleTemplate};
use crate::java;
use crate::org::joml::vector2f::Vector2f;
use crate::org::joml::vector3f::Vector3f;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::{KahluaTable, LuaValue};
use crate::zombie::core::immutable_color::ImmutableColor;
use crate::zombie::scripting::entity::components::crafting::craft_recipe::VANILLA_ID;
use crate::zombie::scripting::script_parser::{self, Block, BlockElement};
use crate::zombie::util::hash::pz_hash;
use crate::zombie::util::string_utils;

type R<T> = Result<T, String>;

const NPE: &str = "java.lang.NullPointerException";

/// How deep `template!` may recurse before the port gives up where the original would end in a StackOverflowError.
const MAX_LOAD_DEPTH: u32 = 64;

/// `ss[i]` of a Java array: the ArrayIndexOutOfBoundsException when it is too short.
fn index<'a>(ss: &[&'a str], i: usize) -> R<&'a str> {
    ss.get(i).copied().ok_or_else(|| {
        format!(
            "java.lang.ArrayIndexOutOfBoundsException: Index {i} out of bounds for length {}",
            ss.len()
        )
    })
}

/// java.lang.Float.parseFloat
fn parse_float(s: &str) -> R<f32> {
    java::parse_float(s).ok_or_else(|| {
        format!(
            "java.lang.NumberFormatException: {}",
            java::number_format_message(s)
        )
    })
}

/// java.lang.Integer.parseInt
fn parse_int(s: &str) -> R<i32> {
    java::parse_int(s).ok_or_else(|| {
        format!(
            "java.lang.NumberFormatException: {}",
            java::number_format_message(s)
        )
    })
}

/// java.lang.Boolean.parseBoolean: "true" in any case.
fn parse_boolean(s: &str) -> bool {
    java::equals_ignore_case(s, "true")
}

/// zombie.util.StringUtils.discardNullOrWhitespace of a value.
fn discard(v: &str) -> Option<String> {
    string_utils::discard_null_or_whitespace(Some(v.to_string()))
}

/// `v.isEmpty() ? null : v`
fn empty_null(v: &str) -> Option<String> {
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

/// `x.id != null && x.id.equals(id)`, the test of every `get...ById` here.
fn same_id(a: &Option<String>, b: Option<&str>) -> bool {
    matches!((a.as_deref(), b), (Some(a), Some(b)) if a == b)
}

/// `map.put(k, v)` on a hash map kept as (key, value) pairs.
fn put<K: PartialEq, V>(map: &mut Vec<(K, V)>, k: K, v: V) {
    match map.iter_mut().find(|e| e.0 == k) {
        Some(e) => e.1 = v,
        None => map.push((k, v)),
    }
}

/// The trimmed `getKey()` and `getValue()` of a ScriptParser value.
fn key_value(value: &script_parser::Value) -> (&str, &str) {
    (java::trim(value.get_key()), java::trim(value.get_value()))
}

/// zombie.scripting.objects.VehicleScript.LoadVector2f: `split(" ")`, two floats.
fn load_vector2f(s: &str, v: &mut Vector2f) -> R<()> {
    let ss = java::split_char(s, ' ');
    let x = parse_float(index(&ss, 0)?)?;
    let y = parse_float(index(&ss, 1)?)?;
    *v = Vector2f::new(x, y);
    Ok(())
}

/// zombie.scripting.objects.VehicleScript.LoadVector3f: `split(" ")`, three floats.
fn load_vector3f(s: &str, v: &mut Vector3f) -> R<()> {
    let ss = java::split_char(s, ' ');
    let x = parse_float(index(&ss, 0)?)?;
    let y = parse_float(index(&ss, 1)?)?;
    let z = parse_float(index(&ss, 2)?)?;
    *v = Vector3f::new(x, y, z);
    Ok(())
}

/// java.util.Arrays.sort(float[]) order: -0.0 before 0.0, NaN last.
fn java_float_cmp(a: f32, b: f32) -> std::cmp::Ordering {
    if a.is_nan() || b.is_nan() {
        a.is_nan().cmp(&b.is_nan())
    } else {
        a.total_cmp(&b)
    }
}

/// zombie.scripting.objects.VehicleScript.globMatch: `Pattern.compile(pattern.replaceAll("\\*", ".*"))
/// .matcher(str).matches()`. A `*` is any run and a `.` any character but a line terminator; every other character
/// is matched literally (regex metacharacters other than `*` and `.` are UNKNOWN: no game script uses them in a part
/// or passenger pattern). A null `str` is the original's NullPointerException.
fn glob_match(pattern: &str, s: Option<&str>) -> R<bool> {
    let s: Vec<char> = s.ok_or(NPE)?.chars().collect();
    let p: Vec<char> = pattern.chars().collect();
    let any = |c: char| !matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}');
    // match[i][j]: p[i..] matches s[j..]
    let mut m = vec![vec![false; s.len() + 1]; p.len() + 1];
    m[p.len()][s.len()] = true;
    for i in (0..p.len()).rev() {
        for j in (0..=s.len()).rev() {
            m[i][j] = match p[i] {
                // ".*": zero characters, or one non-terminator and again
                '*' => m[i + 1][j] || (j < s.len() && any(s[j]) && m[i][j + 1]),
                '.' => j < s.len() && any(s[j]) && m[i + 1][j + 1],
                c => j < s.len() && s[j] == c && m[i + 1][j + 1],
            };
        }
    }
    Ok(m[0][0])
}

/// zombie.scripting.objects.VehicleScript.Anim
#[derive(Clone, Debug, PartialEq)]
pub struct Anim {
    pub id: Option<String>,
    pub anim: Option<String>,
    pub rate: f32,
    pub animate: bool,
    pub r#loop: bool,
    pub reverse: bool,
    pub offset: Vector3f,
    pub angle: Vector3f,
    pub sound: Option<String>,
}

impl Default for Anim {
    fn default() -> Self {
        Anim {
            id: None,
            anim: None,
            rate: 1.0,
            animate: true,
            r#loop: false,
            reverse: false,
            offset: Vector3f::default(),
            angle: Vector3f::default(),
            sound: None,
        }
    }
}

/// zombie.scripting.objects.VehicleScript.Area
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Area {
    pub id: Option<String>,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// zombie.scripting.objects.VehicleScript.Container
#[derive(Clone, Debug, PartialEq)]
pub struct Container {
    pub capacity: i32,
    pub seat: i32,
    pub seat_id: Option<String>,
    pub lua_test: Option<String>,
    pub content_type: Option<String>,
    pub condition_affects_capacity: bool,
    /// HashMap<String, String>, kept as pairs in insertion order.
    pub sound_map: Vec<(String, String)>,
}

impl Default for Container {
    fn default() -> Self {
        Container {
            capacity: 0,
            seat: -1,
            seat_id: None,
            lua_test: None,
            content_type: None,
            condition_affects_capacity: false,
            sound_map: Vec::new(),
        }
    }
}

/// zombie.scripting.objects.VehicleScript.Door: no fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Door;

/// zombie.scripting.objects.VehicleScript.LightBar
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LightBar {
    pub enable: bool,
    pub sound_siren0: String,
    pub sound_siren1: String,
    pub sound_siren2: String,
}

/// zombie.scripting.objects.VehicleScript.Model
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub id: Option<String>,
    pub file: Option<String>,
    pub scale: f32,
    pub offset: Vector3f,
    pub rotate: Vector3f,
    pub attachment_name_parent: Option<String>,
    pub attachment_name_self: Option<String>,
    pub ignore_vehicle_scale: bool,
}

impl Default for Model {
    fn default() -> Self {
        Model {
            id: None,
            file: None,
            scale: 1.0,
            offset: Vector3f::default(),
            rotate: Vector3f::default(),
            attachment_name_parent: None,
            attachment_name_self: None,
            ignore_vehicle_scale: false,
        }
    }
}

/// zombie.scripting.objects.VehicleScript.Part. Its `makeCopy` copies every field (tables through
/// LuaManager.copyTable, a deep copy), so `clone` is the copy.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub id: Option<String>,
    pub parent: Option<String>,
    pub item_type: Option<Vec<String>>,
    pub container: Option<Container>,
    pub area: Option<String>,
    pub mechanic_area: Option<String>,
    pub wheel: Option<String>,
    /// THashMap<String, KahluaTable>, kept as pairs in insertion order.
    pub tables: Option<Vec<(Option<String>, KahluaTable)>>,
    /// THashMap<String, String>, kept as pairs in insertion order.
    pub lua_functions: Option<Vec<(String, String)>>,
    pub models: Option<Vec<Model>>,
    pub set_all_models_visible: bool,
    pub door: Option<Door>,
    pub window: Option<Window>,
    pub anims: Option<Vec<Anim>>,
    pub category: Option<String>,
    pub specific_item: bool,
    pub mechanic_require_key: bool,
    pub repair_mechanic: bool,
    pub has_lights_rear: bool,
    pub durability: f32,
}

impl Default for Part {
    fn default() -> Self {
        Part {
            id: Some("Unknown".into()),
            parent: None,
            item_type: None,
            container: None,
            area: None,
            mechanic_area: None,
            wheel: None,
            tables: None,
            lua_functions: None,
            models: None,
            set_all_models_visible: true,
            door: None,
            window: None,
            anims: None,
            category: None,
            specific_item: true,
            mechanic_require_key: false,
            repair_mechanic: false,
            has_lights_rear: false,
            durability: 0.0,
        }
    }
}

/// zombie.scripting.objects.VehicleScript.Passenger.SwitchSeat
#[derive(Clone, Debug, PartialEq)]
pub struct SwitchSeat {
    pub id: Option<String>,
    pub seat: i32,
    pub anim: Option<String>,
    pub rate: f32,
    pub sound: Option<String>,
}

/// zombie.scripting.objects.VehicleScript.Passenger. `makeCopy` copies every field, its positions through
/// Position.makeCopy (which drops `area`).
#[derive(Clone, Debug, PartialEq)]
pub struct Passenger {
    pub id: Option<String>,
    pub anims: Vec<Anim>,
    pub switch_seats: Vec<SwitchSeat>,
    pub has_roof: bool,
    pub show_passenger: bool,
    pub door: Option<String>,
    pub door2: Option<String>,
    pub area: Option<String>,
    pub positions: Vec<Position>,
}

impl Passenger {
    fn new(id: Option<String>) -> Self {
        Passenger {
            id,
            anims: Vec::new(),
            switch_seats: Vec::new(),
            has_roof: true,
            show_passenger: false,
            door: None,
            door2: None,
            area: None,
            positions: Vec::new(),
        }
    }

    /// zombie.scripting.objects.VehicleScript.Passenger.makeCopy
    pub fn make_copy(&self) -> Passenger {
        Passenger {
            positions: self.positions.iter().map(Position::make_copy).collect(),
            ..self.clone()
        }
    }
}

/// zombie.scripting.objects.VehicleScript.PhysicsShape
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PhysicsShape {
    pub r#type: i32,
    pub offset: Vector3f,
    pub rotate: Vector3f,
    pub extents: Vector3f,
    pub radius: f32,
    pub physics_shape_script: Option<String>,
}

impl PhysicsShape {
    /// zombie.scripting.objects.VehicleScript.PhysicsShape.makeCopy: `physicsShapeScript` is not copied.
    pub fn make_copy(&self) -> PhysicsShape {
        PhysicsShape {
            physics_shape_script: None,
            ..self.clone()
        }
    }
}

/// zombie.scripting.objects.VehicleScript.Position
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Position {
    pub id: Option<String>,
    pub offset: Vector3f,
    pub rotate: Vector3f,
    pub area: Option<String>,
}

impl Position {
    /// zombie.scripting.objects.VehicleScript.Position.makeCopy: `area` is not copied.
    pub fn make_copy(&self) -> Position {
        Position {
            area: None,
            ..self.clone()
        }
    }
}

/// zombie.scripting.objects.VehicleScript.Skin: the texture names (the Texture objects stay null on a server).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Skin {
    pub texture: Option<String>,
    pub texture_rust: Option<String>,
    pub texture_mask: Option<String>,
    pub texture_lights: Option<String>,
    pub texture_damage1_overlay: Option<String>,
    pub texture_damage1_shell: Option<String>,
    pub texture_damage2_overlay: Option<String>,
    pub texture_damage2_shell: Option<String>,
    pub texture_shadow: Option<String>,
}

/// zombie.scripting.objects.VehicleScript.Sounds
#[derive(Clone, Debug, PartialEq)]
pub struct Sounds {
    pub alarm_enable: bool,
    pub alarm: Vec<String>,
    pub alarm_loop: Vec<String>,
    pub horn_enable: bool,
    pub horn: Option<String>,
    pub back_signal_enable: bool,
    pub back_signal: Option<String>,
    pub engine: Option<String>,
    pub engine_start: Option<String>,
    pub engine_turn_off: Option<String>,
    pub hand_brake: Option<String>,
    pub ignition_fail: Option<String>,
    pub ignition_fail_no_power: Option<String>,
    /// HashSet<String>, kept in insertion order.
    pub specified: Vec<String>,
}

impl Default for Sounds {
    fn default() -> Self {
        Sounds {
            alarm_enable: false,
            alarm: Vec::new(),
            alarm_loop: Vec::new(),
            horn_enable: false,
            horn: Some(String::new()),
            back_signal_enable: false,
            back_signal: Some(String::new()),
            engine: None,
            engine_start: None,
            engine_turn_off: None,
            hand_brake: None,
            ignition_fail: None,
            ignition_fail_no_power: None,
            specified: Vec::new(),
        }
    }
}

impl Sounds {
    /// `specified.add(s)`
    fn specify(&mut self, s: &str) {
        if !self.specified.iter().any(|x| x == s) {
            self.specified.push(s.to_string());
        }
    }
}

/// zombie.scripting.objects.VehicleScript.Wheel
#[derive(Clone, Debug, PartialEq)]
pub struct Wheel {
    pub id: Option<String>,
    pub model: Option<String>,
    pub front: bool,
    pub offset: Vector3f,
    pub radius: f32,
    pub width: f32,
}

impl Wheel {
    fn new(id: Option<String>) -> Self {
        Wheel {
            id,
            model: None,
            front: false,
            offset: Vector3f::default(),
            radius: 0.5,
            width: 0.4,
        }
    }
}

/// zombie.scripting.objects.VehicleScript.Window
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Window {
    pub openable: bool,
}

/// zombie.scripting.objects.VehicleScript, with the BaseScriptObject state the probe prints.
#[derive(Clone, Debug, PartialEq)]
pub struct VehicleScript {
    /// BaseScriptObject.module (its name)
    pub module: String,
    /// BaseScriptObject.scriptObjectName
    pub script_object_name: Option<String>,
    pub enabled: bool,
    pub debug_only: bool,
    pub script_version: i64,
    /// BaseScriptObject.loadedScriptBodies as (mod id, body) pairs.
    pub loaded_script_bodies: Vec<(String, String)>,
    pub file_name: Option<String>,
    pub name: Option<String>,
    pub models: Vec<Model>,
    pub attachments: Vec<ModelAttachment>,
    pub mass: f32,
    pub center_of_mass_offset: Vector3f,
    pub engine_force: f32,
    pub engine_idle_speed: f32,
    pub steering_increment: f32,
    pub steering_clamp: f32,
    pub steering_clamp_max: f32,
    pub wheel_friction: f32,
    pub stopping_movement_force: f32,
    pub animal_trailer_size: f32,
    pub suspension_stiffness: f32,
    pub suspension_damping: f32,
    pub suspension_compression: f32,
    pub suspension_rest_length: f32,
    pub max_suspension_travel_cm: f32,
    pub roll_influence: f32,
    pub extents: Vector3f,
    pub shadow_extents: Vector2f,
    pub shadow_offset: Vector2f,
    pub had_shadow_o_extents: bool,
    pub had_shadow_offset: bool,
    pub extents_offset: Vector2f,
    pub physics_chassis_shape: Vector3f,
    pub physics_shapes: Vec<PhysicsShape>,
    pub wheels: Vec<Wheel>,
    pub crawl_through_wheels: Vec<Wheel>,
    pub passengers: Vec<Passenger>,
    pub max_speed: f32,
    pub use_chassis_physics_collision: bool,
    pub max_speed_reverse: f32,
    pub is_small_vehicle: bool,
    pub front_end_health: i32,
    pub rear_end_health: i32,
    pub storage_capacity: i32,
    pub engine_loudness: i32,
    pub engine_quality: i32,
    pub seats: i32,
    pub mechanic_type: i32,
    pub engine_repair_level: i32,
    pub player_damage_protection: f32,
    pub forced_hue: f32,
    pub forced_sat: f32,
    pub forced_val: f32,
    pub left_siren_col: Option<ImmutableColor>,
    pub right_siren_col: Option<ImmutableColor>,
    pub engine_rpm_type: String,
    pub offroad_efficiency: f32,
    pub crawl_offsets: Vec<f32>,
    pub zombie_type: Option<Vec<String>>,
    pub special_key_ring: Option<Vec<String>>,
    pub not_kill_crops: bool,
    pub has_lighter: bool,
    pub car_mechanics_overlay: Option<String>,
    pub car_model_name: Option<String>,
    pub special_loot_chance: i32,
    pub special_key_ring_chance: i32,
    pub never_spawn_key: bool,
    pub gear_ratio_count: i32,
    pub gear_ratio: [f32; 9],
    pub textures: Skin,
    pub skins: Vec<Skin>,
    pub areas: Vec<Area>,
    pub parts: Vec<Part>,
    pub has_siren: bool,
    pub lightbar: LightBar,
    pub sound: Sounds,
    pub texture_mask_enable: bool,
}

impl VehicleScript {
    /// zombie.scripting.objects.VehicleScript.<init> with BaseScriptObject.setModule.
    pub fn new(module: &str) -> Self {
        VehicleScript {
            module: module.to_string(),
            script_object_name: None,
            enabled: true,
            debug_only: false,
            script_version: -1,
            loaded_script_bodies: Vec::new(),
            file_name: None,
            name: None,
            models: Vec::new(),
            attachments: Vec::new(),
            mass: 800.0,
            center_of_mass_offset: Vector3f::default(),
            engine_force: 3000.0,
            engine_idle_speed: 750.0,
            steering_increment: 0.04,
            steering_clamp: 0.4,
            steering_clamp_max: 0.9,
            wheel_friction: 800.0,
            stopping_movement_force: 1.0,
            animal_trailer_size: 0.0,
            suspension_stiffness: 20.0,
            suspension_damping: 2.3,
            suspension_compression: 4.4,
            suspension_rest_length: 0.6,
            max_suspension_travel_cm: 500.0,
            roll_influence: 0.1,
            extents: Vector3f::new(0.75, 0.5, 2.0),
            shadow_extents: Vector2f::default(),
            shadow_offset: Vector2f::default(),
            had_shadow_o_extents: false,
            had_shadow_offset: false,
            extents_offset: Vector2f::new(0.5, 0.5),
            physics_chassis_shape: Vector3f::default(),
            physics_shapes: Vec::new(),
            wheels: Vec::new(),
            crawl_through_wheels: Vec::new(),
            passengers: Vec::new(),
            max_speed: 20.0,
            use_chassis_physics_collision: true,
            max_speed_reverse: 40.0,
            is_small_vehicle: true,
            front_end_health: 100,
            rear_end_health: 100,
            storage_capacity: 100,
            engine_loudness: 100,
            engine_quality: 100,
            seats: 2,
            mechanic_type: 0,
            engine_repair_level: 0,
            player_damage_protection: 0.0,
            forced_hue: -1.0,
            forced_sat: -1.0,
            forced_val: -1.0,
            left_siren_col: None,
            right_siren_col: None,
            engine_rpm_type: "jeep".into(),
            offroad_efficiency: 1.0,
            crawl_offsets: Vec::new(),
            zombie_type: None,
            special_key_ring: None,
            not_kill_crops: false,
            has_lighter: true,
            car_mechanics_overlay: None,
            car_model_name: None,
            special_loot_chance: 8,
            special_key_ring_chance: 0,
            never_spawn_key: false,
            gear_ratio_count: 4,
            gear_ratio: [7.09, 6.44, 4.1, 2.29, 1.47, 1.0, 0.0, 0.0, 0.0],
            textures: Skin::default(),
            skins: Vec::new(),
            areas: Vec::new(),
            parts: Vec::new(),
            has_siren: false,
            lightbar: LightBar::default(),
            sound: Sounds::default(),
            texture_mask_enable: false,
        }
    }

    /// zombie.scripting.objects.VehicleScript.InitLoadPP with BaseScriptObject.InitLoadPP, and the bucket's
    /// `addLoadedScriptBody` that follows it: run for every body found, so `fileName` is the file of the last body.
    /// Only the game's own files are read, so the mod id is always `pz-vanilla`.
    pub fn init_load_pp(&mut self, name: &str, file_name: &str, body: &str) {
        self.script_object_name = Some(name.to_string());
        self.file_name = Some(file_name.to_string());
        self.name = Some(name.to_string());
        self.loaded_script_bodies
            .push((VANILLA_ID.to_string(), body.to_string()));
    }

    /// BaseScriptObject.getScriptObjectFullType
    pub fn get_script_object_full_type(&self) -> String {
        format!(
            "{}.{}",
            self.module,
            self.script_object_name.as_deref().unwrap_or("null")
        )
    }

    /// zombie.scripting.objects.VehicleScript.getFullName: module name, ".", name.
    pub fn get_full_name(&self) -> String {
        format!("{}.{}", self.module, self.name.as_deref().unwrap_or("null"))
    }

    /// BaseScriptObject.calculateScriptVersion: murmur_64 of the full type followed by every (mod id, body).
    pub fn calculate_script_version(&mut self) {
        let mut sb = self.get_script_object_full_type();
        for (m, b) in &self.loaded_script_bodies {
            sb.push_str(m);
            sb.push_str(b);
        }
        self.script_version = pz_hash::murmur_64_str(&sb);
    }

    /// BaseScriptObject.LoadCommonBlock (Block): `enabled` and `debugOnly`, keys in any case.
    fn load_common_block(&mut self, block: &Block) {
        for value in &block.values {
            let (key, val) = key_value(value);
            if !key.is_empty() && !val.is_empty() {
                if java::equals_ignore_case(key, "enabled") {
                    self.enabled = java::equals_ignore_case(val, "true");
                } else if java::equals_ignore_case(key, "debugOnly") {
                    self.debug_only = java::equals_ignore_case(val, "true");
                }
            }
        }
    }

    /// zombie.scripting.objects.VehicleScript.Load. `Err` is the exception the original throws; what was set before
    /// it stays set (VehicleTemplate.getScript keeps such a partly loaded script). `this_template` is the template
    /// whose script this is, `depth` the `template!` nesting.
    pub fn load(
        &mut self,
        name: &str,
        total_file: &str,
        ctx: &mut TemplateContext,
        this_template: Option<usize>,
        depth: u32,
    ) -> R<()> {
        if depth > MAX_LOAD_DEPTH {
            return Err("java.lang.StackOverflowError".into());
        }
        let parsed = script_parser::parse(total_file);
        let block = parsed.children.first().ok_or_else(|| {
            "java.lang.IndexOutOfBoundsException: Index 0 out of bounds for length 0".to_string()
        })?;
        self.load_common_block(block);
        for element in &block.elements {
            match element {
                BlockElement::Value(value) => {
                    let ss = java::split_char(&value.string, '=');
                    let k = java::trim(index(&ss, 0)?);
                    let v = java::trim(index(&ss, 1)?);
                    self.load_value(name, k, v, ctx, this_template, depth)?;
                }
                BlockElement::Block(i) => self.load_child(&block.children[*i])?,
            }
        }
        Ok(())
    }

    /// The `key = value` branch of VehicleScript.Load.
    fn load_value(
        &mut self,
        name: &str,
        k: &str,
        v: &str,
        ctx: &mut TemplateContext,
        this_template: Option<usize>,
        depth: u32,
    ) -> R<()> {
        if k == "extents" {
            load_vector3f(v, &mut self.extents)?;
        } else if k == "shadowExtents" {
            load_vector2f(v, &mut self.shadow_extents)?;
            self.had_shadow_o_extents = true;
        } else if k == "shadowOffset" {
            load_vector2f(v, &mut self.shadow_offset)?;
            self.had_shadow_offset = true;
        } else if k == "physicsChassisShape" {
            load_vector3f(v, &mut self.physics_chassis_shape)?;
        } else if k == "extentsOffset" {
            load_vector2f(v, &mut self.extents_offset)?;
        } else if k == "mass" {
            self.mass = parse_float(v)?;
        } else if java::equals_ignore_case(k, "offRoadEfficiency") {
            self.offroad_efficiency = parse_float(v)?;
        } else if k == "centerOfMassOffset" {
            load_vector3f(v, &mut self.center_of_mass_offset)?;
        } else if k == "engineForce" {
            self.engine_force = parse_float(v)?;
        } else if k == "engineIdleSpeed" {
            self.engine_idle_speed = parse_float(v)?;
        } else if k == "gearRatioCount" {
            self.gear_ratio_count = parse_int(v)?;
        } else if k == "gearRatioR" {
            self.gear_ratio[0] = parse_float(v)?;
        } else if let Some(n) = k.strip_prefix("gearRatio").and_then(|d| {
            ["1", "2", "3", "4", "5", "6", "7", "8"]
                .iter()
                .position(|x| *x == d)
        }) {
            self.gear_ratio[n + 1] = parse_float(v)?;
        } else if k == "textureMaskEnable" {
            self.texture_mask_enable = parse_boolean(v);
        } else if k == "textureRust" {
            self.textures.texture_rust = discard(v);
        } else if k == "textureMask" {
            self.textures.texture_mask = discard(v);
        } else if k == "textureLights" {
            self.textures.texture_lights = discard(v);
        } else if k == "textureDamage1Overlay" {
            self.textures.texture_damage1_overlay = discard(v);
        } else if k == "textureDamage1Shell" {
            self.textures.texture_damage1_shell = discard(v);
        } else if k == "textureDamage2Overlay" {
            self.textures.texture_damage2_overlay = discard(v);
        } else if k == "textureDamage2Shell" {
            self.textures.texture_damage2_shell = discard(v);
        } else if k == "textureShadow" {
            self.textures.texture_shadow = discard(v);
        } else if k == "rollInfluence" {
            self.roll_influence = parse_float(v)?;
        } else if k == "steeringIncrement" {
            self.steering_increment = parse_float(v)?;
        } else if k == "steeringClamp" {
            self.steering_clamp = parse_float(v)?;
        } else if k == "suspensionStiffness" {
            self.suspension_stiffness = parse_float(v)?;
        } else if k == "suspensionDamping" {
            self.suspension_damping = parse_float(v)?;
        } else if k == "suspensionCompression" {
            self.suspension_compression = parse_float(v)?;
        } else if k == "suspensionRestLength" {
            self.suspension_rest_length = parse_float(v)?;
        } else if k == "maxSuspensionTravelCm" {
            self.max_suspension_travel_cm = parse_float(v)?;
        } else if k == "wheelFriction" {
            self.wheel_friction = parse_float(v)?;
        } else if k == "stoppingMovementForce" {
            self.stopping_movement_force = parse_float(v)?;
        } else if java::equals_ignore_case(k, "animalTrailerSize") {
            self.animal_trailer_size = parse_float(v)?;
        } else if k == "maxSpeed" {
            self.max_speed = parse_float(v)?;
        } else if k == "maxSpeedReverse" {
            self.max_speed_reverse = parse_float(v)?;
        } else if k == "isSmallVehicle" {
            self.is_small_vehicle = parse_boolean(v);
        } else if k == "frontEndDurability" {
            self.front_end_health = parse_int(v)?;
        } else if k == "rearEndDurability" {
            self.rear_end_health = parse_int(v)?;
        } else if k == "storageCapacity" {
            self.storage_capacity = parse_int(v)?;
        } else if k == "engineLoudness" {
            self.engine_loudness = parse_int(v)?;
        } else if k == "engineQuality" {
            self.engine_quality = parse_int(v)?;
        } else if k == "seats" {
            self.seats = parse_int(v)?;
        } else if k == "hasSiren" {
            self.has_siren = parse_boolean(v);
        } else if k == "mechanicType" {
            self.mechanic_type = parse_int(v)?;
        } else if k == "forcedColor" {
            let hsv = java::split_char(v, ' ');
            self.forced_hue = parse_float(index(&hsv, 0)?)?;
            self.forced_sat = parse_float(index(&hsv, 1)?)?;
            self.forced_val = parse_float(index(&hsv, 2)?)?;
        } else if k == "engineRPMType" {
            self.engine_rpm_type = java::trim(v).to_string();
        } else if k == "zombieType" {
            let list = self.zombie_type.insert(Vec::new());
            for s in java::split_char(v, ';') {
                list.push(java::trim(s).to_string());
            }
        } else if k == "specialKeyRing" {
            let list = self.special_key_ring.insert(Vec::new());
            for s in java::split_char(v, ';') {
                let item_type = java::trim(s);
                if !string_utils::is_null_or_whitespace(Some(item_type)) {
                    list.push(item_type.to_string());
                }
            }
        } else if k == "notKillCrops" {
            self.not_kill_crops = parse_boolean(v);
        } else if k == "hasLighter" {
            self.has_lighter = parse_boolean(v);
        } else if k == "neverSpawnKey" {
            self.never_spawn_key = parse_boolean(v);
        } else if k == "carMechanicsOverlay" {
            self.car_mechanics_overlay = Some(java::trim(v).to_string());
        } else if k == "carModelName" {
            self.car_model_name = Some(java::trim(v).to_string());
        } else if k == "specialLootChance" {
            self.special_loot_chance = parse_int(v)?;
        } else if k == "specialKeyRingChance" {
            self.special_key_ring_chance = parse_int(v)?;
        } else if k == "template" {
            self.load_template(v, ctx, this_template)?;
        } else if k == "template!" {
            match (ctx.get_vehicle_template)(v) {
                // DebugLog: "ERROR: template \"v\" not found in: <fileName>"
                None => ctx.log.push(format!(
                    "ERROR: template \"{v}\" not found in: {:?}",
                    self.file_name
                )),
                Some(t) => {
                    let body = ctx.templates[t].body.clone();
                    self.load(name, &body, ctx, this_template, depth + 1)?;
                }
            }
        } else if k == "engineRepairLevel" {
            self.engine_repair_level = parse_int(v)?;
        } else if k == "playerDamageProtection" {
            self.player_damage_protection = parse_float(v)?;
        } else if k == "useChassisPhysicsCollision" {
            self.use_chassis_physics_collision = parse_boolean(v);
        }
        Ok(())
    }

    /// The child-block branch of VehicleScript.Load.
    fn load_child(&mut self, child: &Block) -> R<()> {
        match child.r#type.as_str() {
            "area" => self.load_area(child)?,
            "attachment" => self.load_attachment(child)?,
            "model" => load_model(child, &mut self.models)?,
            "part" => match child.id.as_deref() {
                Some(pattern) if pattern.contains('*') => {
                    // for (Part part : this.parts) { if (globMatch(pattern, part.id)) { child.id = part.id; ... } }
                    for i in 0..self.parts.len() {
                        let id = self.parts[i].id.clone();
                        if glob_match(pattern, id.as_deref())? {
                            let mut c = child.clone();
                            c.id = id;
                            self.load_part(&c)?;
                        }
                    }
                }
                _ => self.load_part(child)?,
            },
            "passenger" => match child.id.as_deref() {
                Some(pattern) if pattern.contains('*') => {
                    for i in 0..self.passengers.len() {
                        let id = self.passengers[i].id.clone();
                        if glob_match(pattern, id.as_deref())? {
                            let mut c = child.clone();
                            c.id = id;
                            self.load_passenger(&c)?;
                        }
                    }
                }
                _ => self.load_passenger(child)?,
            },
            "physics" => {
                if let Some(shape) = load_physics_shape(child)? {
                    if self.physics_shapes.len() < 10 {
                        self.physics_shapes.push(shape);
                    }
                }
            }
            "skin" => {
                let skin = load_skin(child);
                if !string_utils::is_null_or_whitespace(skin.texture.as_deref()) {
                    self.skins.push(skin);
                }
            }
            "wheel" => self.load_wheel(child, false)?,
            "crawlThroughWheel" => self.load_wheel(child, true)?,
            "lightbar" => {
                for value in &child.values {
                    let (k, v) = key_value(value);
                    if k == "soundSiren" {
                        self.lightbar.sound_siren0 = format!("{v}Yelp");
                        self.lightbar.sound_siren1 = format!("{v}Wall");
                        self.lightbar.sound_siren2 = format!("{v}Alarm");
                    }
                    if k == "soundSiren0" {
                        self.lightbar.sound_siren0 = v.to_string();
                    }
                    if k == "soundSiren1" {
                        self.lightbar.sound_siren1 = v.to_string();
                    }
                    if k == "soundSiren2" {
                        self.lightbar.sound_siren2 = v.to_string();
                    }
                    if k == "leftCol" {
                        let split = java::split_char(v, ';');
                        self.left_siren_col = Some(ImmutableColor::new_rgb(
                            parse_float(index(&split, 0)?)?,
                            parse_float(index(&split, 1)?)?,
                            parse_float(index(&split, 2)?)?,
                        ));
                    }
                    if k == "rightCol" {
                        let split = java::split_char(v, ';');
                        self.right_siren_col = Some(ImmutableColor::new_rgb(
                            parse_float(index(&split, 0)?)?,
                            parse_float(index(&split, 1)?)?,
                            parse_float(index(&split, 2)?)?,
                        ));
                    }
                    self.lightbar.enable = true;
                    if self.get_part_by_id(Some("lightbar")).is_none() {
                        self.parts.push(Part {
                            id: Some("lightbar".into()),
                            ..Part::default()
                        });
                    }
                }
            }
            "sound" => {
                for value in &child.values {
                    let (k, v) = key_value(value);
                    let sound = &mut self.sound;
                    if k == "alarm" {
                        let ss = java::split_whitespace_regex(v);
                        if !ss.is_empty() {
                            sound.alarm.extend(ss.iter().map(|s| s.to_string()));
                            sound.alarm_enable = true;
                        }
                    } else if k == "alarmLoop" {
                        let ss = java::split_whitespace_regex(v);
                        if !ss.is_empty() {
                            sound.alarm_loop.extend(ss.iter().map(|s| s.to_string()));
                            sound.alarm_enable = true;
                        }
                    } else if k == "backSignal" {
                        sound.back_signal = discard(v);
                        sound.back_signal_enable = sound.back_signal.is_some();
                    } else if k == "engine" {
                        sound.engine = discard(v);
                    } else if k == "engineStart" {
                        sound.engine_start = discard(v);
                    } else if k == "engineTurnOff" {
                        sound.engine_turn_off = discard(v);
                    } else if k == "handBrake" {
                        sound.hand_brake = discard(v);
                    } else if k == "horn" {
                        sound.horn = discard(v);
                        sound.horn_enable = sound.horn.is_some();
                    } else if k == "ignitionFail" {
                        sound.ignition_fail = discard(v);
                    } else if k == "ignitionFailNoPower" {
                        sound.ignition_fail_no_power = discard(v);
                    }
                    sound.specify(k);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// zombie.scripting.objects.VehicleScript.LoadAttachment: `getAttachmentById` (`getId().equals(id)`), else a new
    /// ModelAttachment owned by this script.
    fn load_attachment(&mut self, block: &Block) -> R<()> {
        let i = match self
            .attachments
            .iter()
            .position(|a| Some(a.id.as_str()) == block.id.as_deref())
        {
            Some(i) => i,
            None => {
                let mut a = ModelAttachment::new(block.id.as_deref())?;
                a.owned = true;
                self.attachments.push(a);
                self.attachments.len() - 1
            }
        };
        let attachment = &mut self.attachments[i];
        for value in &block.values {
            let (k, v) = key_value(value);
            if k == "bone" {
                attachment.set_bone(v);
            } else if k == "offset" {
                load_vector3f(v, &mut attachment.offset)?;
            } else if k == "rotate" {
                load_vector3f(v, &mut attachment.rotate)?;
            } else if k == "canAttach" {
                attachment.can_attach = Some(
                    java::split_char(v, ';')
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                );
            } else if k == "zoffset" {
                attachment.zoffset = parse_float(v)?;
            } else if k == "updateconstraint" {
                attachment.update_constraint = parse_boolean(v);
            }
        }
        Ok(())
    }

    /// zombie.scripting.objects.VehicleScript.LoadWheel: `getWheelById` looks in `wheels` only, a new wheel goes to
    /// `crawlThroughWheels` or `wheels`.
    fn load_wheel(&mut self, block: &Block, crawl_through_wheel: bool) -> R<()> {
        let wheel = match self
            .wheels
            .iter()
            .position(|w| same_id(&w.id, block.id.as_deref()))
        {
            Some(i) => &mut self.wheels[i],
            None => {
                let list = if crawl_through_wheel {
                    &mut self.crawl_through_wheels
                } else {
                    &mut self.wheels
                };
                list.push(Wheel::new(block.id.clone()));
                let n = list.len() - 1;
                &mut list[n]
            }
        };
        for value in &block.values {
            let (k, v) = key_value(value);
            if k == "model" {
                wheel.model = Some(v.to_string());
            } else if k == "front" {
                wheel.front = parse_boolean(v);
            } else if k == "offset" {
                load_vector3f(v, &mut wheel.offset)?;
            } else if k == "radius" {
                wheel.radius = parse_float(v)?;
            } else if k == "width" {
                wheel.width = parse_float(v)?;
            }
        }
        Ok(())
    }

    /// zombie.scripting.objects.VehicleScript.LoadPassenger
    fn load_passenger(&mut self, block: &Block) -> R<()> {
        let i = match self.get_passenger_index(block.id.as_deref()) {
            Some(i) => i,
            None => {
                self.passengers.push(Passenger::new(block.id.clone()));
                self.passengers.len() - 1
            }
        };
        let pngr = &mut self.passengers[i];
        for value in &block.values {
            let (k, v) = key_value(value);
            if k == "area" {
                pngr.area = Some(v.to_string());
            } else if k == "door" {
                pngr.door = Some(v.to_string());
            } else if k == "door2" {
                pngr.door2 = Some(v.to_string());
            } else if k == "hasRoof" {
                pngr.has_roof = parse_boolean(v);
            } else if k == "showPassenger" {
                pngr.show_passenger = parse_boolean(v);
            }
        }
        for child in &block.children {
            match child.r#type.as_str() {
                "anim" => load_anim(child, &mut pngr.anims)?,
                "position" => load_position(child, &mut pngr.positions)?,
                "switchSeat" => load_passenger_switch_seat(child, pngr)?,
                _ => {}
            }
        }
        Ok(())
    }

    /// zombie.scripting.objects.VehicleScript.LoadArea: a new area's id is `block.id.intern()` (null is a
    /// NullPointerException); `xywh` sets x, y, w and h one after the other.
    fn load_area(&mut self, block: &Block) -> R<()> {
        let i = match self
            .areas
            .iter()
            .position(|a| same_id(&a.id, block.id.as_deref()))
        {
            Some(i) => i,
            None => {
                let id = block.id.clone().ok_or(NPE)?;
                self.areas.push(Area {
                    id: Some(id),
                    ..Area::default()
                });
                self.areas.len() - 1
            }
        };
        let area = &mut self.areas[i];
        for value in &block.values {
            let (k, v) = key_value(value);
            if k == "xywh" {
                let ss2 = java::split_char(v, ' ');
                area.x = parse_float(index(&ss2, 0)?)?;
                area.y = parse_float(index(&ss2, 1)?)?;
                area.w = parse_float(index(&ss2, 2)?)?;
                area.h = parse_float(index(&ss2, 3)?)?;
            }
        }
        Ok(())
    }

    /// zombie.scripting.objects.VehicleScript.LoadPart
    fn load_part(&mut self, block: &Block) -> R<()> {
        let i = match self.get_index_of_part_by_id(block.id.as_deref()) {
            Some(i) => i,
            None => {
                self.parts.push(Part {
                    id: block.id.clone(),
                    ..Part::default()
                });
                self.parts.len() - 1
            }
        };
        let full_name = self.get_full_name();
        let part = &mut self.parts[i];
        for value in &block.values {
            let (k, v) = key_value(value);
            if k == "area" {
                part.area = empty_null(v);
            }
            if k == "mechanicArea" {
                part.mechanic_area = Some(v.to_string());
            } else if k == "itemType" {
                part.item_type = Some(
                    java::split_char(v, ';')
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                );
            } else if k == "parent" {
                part.parent = empty_null(v);
            } else if k == "mechanicRequireKey" {
                part.mechanic_require_key = parse_boolean(v);
            } else if k == "repairMechanic" {
                part.repair_mechanic = parse_boolean(v);
            } else if k == "setAllModelsVisible" {
                part.set_all_models_visible = parse_boolean(v);
            } else if k == "wheel" {
                part.wheel = Some(v.to_string());
            } else if k == "category" {
                part.category = Some(v.to_string());
            } else if k == "durability" {
                part.durability = parse_float(v)?;
            } else if k == "specificItem" {
                part.specific_item = parse_boolean(v);
            } else if k == "hasLightsRear" {
                part.has_lights_rear = parse_boolean(v);
            }
        }
        for child in &block.children {
            match child.r#type.as_str() {
                "anim" => load_anim(child, part.anims.get_or_insert_with(Vec::new))?,
                "container" => match &mut part.container {
                    // LoadContainer(child, existing) fills the existing container in place.
                    Some(existing) => load_container(child, existing)?,
                    None => {
                        let mut c = Container::default();
                        load_container(child, &mut c)?;
                        part.container = Some(c);
                    }
                },
                "door" => part.door = Some(Door),
                "lua" => part.lua_functions = Some(load_lua_functions(child, &full_name)?),
                "model" => load_model(child, part.models.get_or_insert_with(Vec::new))?,
                "table" => {
                    let existing = part
                        .tables
                        .as_mut()
                        .and_then(|t| t.iter_mut().find(|e| e.0 == child.id));
                    match existing {
                        // LoadTable fills the existing table in place; the put that follows stores the same table.
                        Some(e) => load_table(child, &mut e.1)?,
                        None => {
                            let mut table = KahluaTable::default();
                            load_table(child, &mut table)?;
                            put(
                                part.tables.get_or_insert_with(Vec::new),
                                child.id.clone(),
                                table,
                            );
                        }
                    }
                }
                "window" => part.window = Some(load_window(child)),
                _ => {}
            }
        }
        Ok(())
    }

    /// zombie.scripting.objects.VehicleScript.LoadTemplate. `VehicleTemplate.getScript` runs before the kind after
    /// the first '/' is checked, as in the original. The original's DebugLog lines go to `ctx.log`.
    fn load_template(
        &mut self,
        s: &str,
        ctx: &mut TemplateContext,
        this_template: Option<usize>,
    ) -> R<()> {
        if s.contains('/') {
            let ss: Vec<&str> = java::split_char(s, '/');
            if ss.is_empty() || ss.len() > 3 {
                ctx.log.push(format!("ERROR: template \"{s}\""));
                return Ok(());
            }
            let ss: Vec<&str> = ss.into_iter().map(java::trim).collect();
            if ss.iter().any(|x| x.is_empty()) {
                ctx.log.push(format!("ERROR: template \"{s}\""));
                return Ok(());
            }
            let Some(t) = (ctx.get_vehicle_template)(ss[0]) else {
                ctx.log.push(format!("ERROR: template \"{s}\" not found A"));
                return Ok(());
            };
            let Some(found) = VehicleTemplate::get_script(ctx, t, this_template) else {
                return Ok(());
            };
            // ss[1] of "name/" (split drops the trailing empty piece) is an ArrayIndexOutOfBoundsException.
            let kind = index(&ss, 1)?;
            if !matches!(kind, "area" | "part" | "passenger" | "wheel" | "physics") || ss.len() == 2
            {
                ctx.log.push(format!("ERROR: template \"{s}\""));
                return Ok(());
            }
            let spec = ss[2];
            let snapshot;
            let other = match found {
                Found::This => {
                    snapshot = self.clone();
                    &snapshot
                }
                Found::Other => match ctx.templates[t].script.as_ref() {
                    Some(o) => o,
                    None => return Ok(()),
                },
            };
            let log = &mut ctx.log;
            match kind {
                "area" => self.copy_areas_from(other, spec, log),
                "part" => self.copy_parts_from(other, spec, log),
                "passenger" => self.copy_passengers_from(other, spec, log),
                "wheel" => self.copy_wheels_from(other, spec, log),
                _ => self.copy_physics_from(other, matches!(found, Found::This)),
            }
        } else {
            let template_name = java::trim(s);
            let Some(t) = (ctx.get_vehicle_template)(template_name) else {
                ctx.log.push(format!("ERROR: template \"{s}\" not found B"));
                return Ok(());
            };
            let Some(found) = VehicleTemplate::get_script(ctx, t, this_template) else {
                return Ok(());
            };
            // A template copying from itself: the copies of its areas, parts, passengers, sounds and wheels leave
            // them as they are (each is replaced by a copy of itself), so a snapshot gives the same result.
            let snapshot;
            let other = match found {
                Found::This => {
                    snapshot = self.clone();
                    &snapshot
                }
                Found::Other => match ctx.templates[t].script.as_ref() {
                    Some(o) => o,
                    None => return Ok(()),
                },
            };
            let log = &mut ctx.log;
            self.copy_areas_from(other, "*", log);
            self.copy_parts_from(other, "*", log);
            self.copy_passengers_from(other, "*", log);
            self.copy_sound_from(other, "*");
            self.copy_wheels_from(other, "*", log);
            self.copy_physics_from(other, matches!(found, Found::This));
        }
        Ok(())
    }

    /// zombie.scripting.objects.VehicleScript.copyAreasFrom: each area (or the one named) replaces the area of the
    /// same id or is added, as `makeCopy` (every field).
    pub fn copy_areas_from(&mut self, other: &VehicleScript, spec: &str, log: &mut Vec<String>) {
        let one = |this: &mut VehicleScript, a: &Area| match this
            .areas
            .iter()
            .position(|x| same_id(&x.id, a.id.as_deref()))
        {
            None => this.areas.push(a.clone()),
            Some(i) => this.areas[i] = a.clone(),
        };
        if spec == "*" {
            for a in &other.areas {
                one(self, a);
            }
        } else {
            match other.areas.iter().find(|x| same_id(&x.id, Some(spec))) {
                None => log.push(format!("ERROR: area \"{spec}\" not found")),
                Some(a) => one(self, a),
            }
        }
    }

    /// zombie.scripting.objects.VehicleScript.copyPartsFrom: as copyAreasFrom, with Part.makeCopy.
    pub fn copy_parts_from(&mut self, other: &VehicleScript, spec: &str, log: &mut Vec<String>) {
        let one = |this: &mut VehicleScript, p: &Part| match this
            .get_index_of_part_by_id(p.id.as_deref())
        {
            None => this.parts.push(p.clone()),
            Some(i) => this.parts[i] = p.clone(),
        };
        if spec == "*" {
            for p in &other.parts {
                one(self, p);
            }
        } else {
            match other.get_index_of_part_by_id(Some(spec)) {
                None => log.push(format!("ERROR: part \"{spec}\" not found")),
                Some(i) => one(self, &other.parts[i]),
            }
        }
    }

    /// zombie.scripting.objects.VehicleScript.copyPhysicsFrom: the shapes are cleared, then every shape of `other`
    /// is added as `makeCopy` (without `physicsShapeScript`), and `useChassisPhysicsCollision` is taken. When `other`
    /// is this script (`is_self`), nothing is left to copy once the shapes are cleared.
    pub fn copy_physics_from(&mut self, other: &VehicleScript, is_self: bool) {
        self.physics_shapes.clear();
        if is_self {
            return;
        }
        self.physics_shapes
            .extend(other.physics_shapes.iter().map(PhysicsShape::make_copy));
        self.use_chassis_physics_collision = other.use_chassis_physics_collision;
    }

    /// zombie.scripting.objects.VehicleScript.copyPassengersFrom: as copyAreasFrom, with Passenger.makeCopy.
    pub fn copy_passengers_from(
        &mut self,
        other: &VehicleScript,
        spec: &str,
        log: &mut Vec<String>,
    ) {
        let one = |this: &mut VehicleScript, p: &Passenger| match this
            .get_passenger_index(p.id.as_deref())
        {
            None => this.passengers.push(p.make_copy()),
            Some(i) => this.passengers[i] = p.make_copy(),
        };
        if spec == "*" {
            for p in &other.passengers {
                one(self, p);
            }
        } else {
            match other.get_passenger_index(Some(spec)) {
                None => log.push(format!("ERROR: passenger \"{spec}\" not found")),
                Some(i) => one(self, &other.passengers[i]),
            }
        }
    }

    /// zombie.scripting.objects.VehicleScript.copySoundFrom: for "*" only, each sound `other` specified among
    /// backSignal (with its enable flag), engine, engineStart, engineTurnOff, handBrake, ignitionFail and
    /// ignitionFailNoPower is taken; every specified key is added to this script's set (alarm and horn are not
    /// copied).
    pub fn copy_sound_from(&mut self, other: &VehicleScript, spec: &str) {
        if spec != "*" {
            return;
        }
        let o = &other.sound;
        for s in &o.specified {
            let t = &mut self.sound;
            match s.as_str() {
                "backSignal" => {
                    t.back_signal = o.back_signal.clone();
                    t.back_signal_enable = o.back_signal_enable;
                }
                "engine" => t.engine = o.engine.clone(),
                "engineStart" => t.engine_start = o.engine_start.clone(),
                "engineTurnOff" => t.engine_turn_off = o.engine_turn_off.clone(),
                "handBrake" => t.hand_brake = o.hand_brake.clone(),
                "ignitionFail" => t.ignition_fail = o.ignition_fail.clone(),
                "ignitionFailNoPower" => {
                    t.ignition_fail_no_power = o.ignition_fail_no_power.clone()
                }
                _ => {}
            }
            t.specify(s);
        }
    }

    /// zombie.scripting.objects.VehicleScript.copyWheelsFrom: as copyAreasFrom over `wheels` (crawl-through wheels
    /// are not copied).
    pub fn copy_wheels_from(&mut self, other: &VehicleScript, spec: &str, log: &mut Vec<String>) {
        let one = |this: &mut VehicleScript, w: &Wheel| match this
            .wheels
            .iter()
            .position(|x| same_id(&x.id, w.id.as_deref()))
        {
            None => this.wheels.push(w.clone()),
            Some(i) => this.wheels[i] = w.clone(),
        };
        if spec == "*" {
            for w in &other.wheels {
                one(self, w);
            }
        } else {
            match other.wheels.iter().find(|x| same_id(&x.id, Some(spec))) {
                None => log.push(format!("ERROR: wheel \"{spec}\" not found")),
                Some(w) => one(self, w),
            }
        }
    }

    /// zombie.scripting.objects.VehicleScript.getPartById (as an index).
    pub fn get_index_of_part_by_id(&self, id: Option<&str>) -> Option<usize> {
        self.parts.iter().position(|p| same_id(&p.id, id))
    }

    /// zombie.scripting.objects.VehicleScript.getPartById
    pub fn get_part_by_id(&self, id: Option<&str>) -> Option<&Part> {
        self.get_index_of_part_by_id(id).map(|i| &self.parts[i])
    }

    /// zombie.scripting.objects.VehicleScript.getPassengerIndex: `None` is the original's -1.
    pub fn get_passenger_index(&self, id: Option<&str>) -> Option<usize> {
        self.passengers.iter().position(|p| same_id(&p.id, id))
    }

    /// zombie.scripting.objects.VehicleScript.getModelScale: the first model's scale, 1 without a model.
    pub fn get_model_scale(&self) -> f32 {
        self.models.first().map_or(1.0, |m| m.scale)
    }

    /// zombie.scripting.objects.VehicleScript.Loaded, as a dedicated server runs it (ScriptModule's vehicle bucket
    /// calls it from `onScriptLoad`): every size and offset times the first model's scale, the switch seats' and
    /// containers' seat indices, the part item types suffixed with the mechanic type, the horn as alarm, the crawl
    /// offsets and the special key ring resolved to full item types (`resolve_item_type` is
    /// ScriptManager.resolveItemType with this script's module). `toBullet` is skipped on a server. `Err` is the
    /// exception the original throws (a null wheel id), which leaves the script out.
    pub fn loaded(&mut self, resolve_item_type: &dyn Fn(&str) -> Option<String>) -> R<()> {
        let scale = self.get_model_scale();
        self.extents.mul(scale);
        self.max_suspension_travel_cm *= scale;
        self.suspension_rest_length *= scale;
        self.center_of_mass_offset.mul(scale);
        self.physics_chassis_shape.mul(scale);
        if self.had_shadow_o_extents {
            self.shadow_extents.mul(scale);
        } else {
            self.shadow_extents = Vector2f::new(self.extents.x, self.extents.z);
        }
        if self.had_shadow_offset {
            self.shadow_offset.mul(scale);
        } else {
            self.shadow_offset =
                Vector2f::new(self.center_of_mass_offset.x, self.center_of_mass_offset.z);
        }
        for model in &mut self.models {
            model.offset.mul(scale);
        }
        for attachment in &mut self.attachments {
            attachment.offset.mul(scale);
        }
        for shape in &mut self.physics_shapes {
            shape.offset.mul(scale);
            match shape.r#type {
                1 | 3 => shape.extents.mul(scale),
                2 => shape.radius *= scale,
                _ => {}
            }
        }
        for wheel in &mut self.wheels {
            wheel.radius *= scale;
            wheel.offset.mul(scale);
        }
        for area in &mut self.areas {
            area.x *= scale;
            area.y *= scale;
            area.w *= scale;
            area.h *= scale;
        }
        for i in 0..self.passengers.len() {
            for posn in &mut self.passengers[i].positions {
                posn.offset.mul(scale);
            }
            for j in 0..self.passengers[i].switch_seats.len() {
                let id = self.passengers[i].switch_seats[j].id.clone();
                let seat = self.get_passenger_index(id.as_deref());
                self.passengers[i].switch_seats[j].seat = seat.map_or(-1, |s| s as i32);
            }
        }
        for i in 0..self.parts.len() {
            let seat_id = self.parts[i]
                .container
                .as_ref()
                .and_then(|c| c.seat_id.clone())
                .filter(|s| !s.is_empty());
            if let Some(seat_id) = seat_id {
                let seat = self
                    .get_passenger_index(Some(&seat_id))
                    .map_or(-1, |s| s as i32);
                if let Some(c) = self.parts[i].container.as_mut() {
                    c.seat = seat;
                }
            }
            let mechanic_type = self.mechanic_type;
            let part = &mut self.parts[i];
            if part.specific_item {
                if let Some(item_type) = part.item_type.as_mut() {
                    for t in item_type.iter_mut() {
                        // String + int
                        *t = format!("{t}{mechanic_type}");
                    }
                }
            }
        }
        if !self.sound.alarm_enable && self.sound.horn_enable {
            self.sound.alarm_enable = true;
            let horn = self.sound.horn.clone().unwrap_or_else(|| "null".into());
            self.sound.alarm.push(horn);
        }
        self.init_crawl_offsets()?;
        if let Some(ring) = self.special_key_ring.as_mut() {
            for item_type in ring.iter_mut() {
                // The entries are never null or whitespace, so resolveItemType never answers null here.
                if let Some(t) = resolve_item_type(item_type) {
                    *item_type = t;
                }
            }
        }
        Ok(())
    }

    /// `wheel.id.contains("Left")`: a null id is the original's NullPointerException.
    fn is_left(wheel: &Wheel) -> R<bool> {
        Ok(wheel.id.as_deref().ok_or(NPE)?.contains("Left"))
    }

    /// zombie.scripting.objects.VehicleScript.initCrawlOffsets (): the offsets of every left wheel, then of every
    /// left crawl-through wheel, mapped onto the vehicle's length, sorted, and thinned to 0.15 apart.
    fn init_crawl_offsets(&mut self) -> R<()> {
        for i in 0..self.wheels.len() {
            if Self::is_left(&self.wheels[i])? {
                let wheel = self.wheels[i].clone();
                self.init_crawl_offsets_of(&wheel)?;
            }
        }
        for i in 0..self.crawl_through_wheels.len() {
            if Self::is_left(&self.crawl_through_wheels[i])? {
                let wheel = self.crawl_through_wheels[i].clone();
                self.init_crawl_offsets_of(&wheel)?;
            }
        }
        let poly_length = self.extents.z + 0.3;
        for i in 0..self.crawl_offsets.len() {
            self.crawl_offsets[i] = (self.extents.z / 2.0 + 0.15 + self.crawl_offsets[i]
                - self.center_of_mass_offset.z)
                / poly_length;
        }
        self.crawl_offsets.sort_by(|a, b| java_float_cmp(*a, *b));
        let mut i = 0;
        while i < self.crawl_offsets.len() {
            let o1 = self.crawl_offsets[i];
            let mut j = i + 1;
            while j < self.crawl_offsets.len() {
                let o2 = self.crawl_offsets[j];
                if (o2 - o1) * poly_length < 0.15 {
                    self.crawl_offsets.remove(j);
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
        Ok(())
    }

    /// zombie.scripting.objects.VehicleScript.initCrawlOffsets (Wheel): up to ten spots in front of and behind the
    /// wheel, 0.3 apart, that stay inside the vehicle and clear of every left wheel.
    fn init_crawl_offsets_of(&mut self, wheel: &Wheel) -> R<()> {
        let model_offset_z = self.models.first().map_or(0.0, |m| m.offset.z);
        let front = self.center_of_mass_offset.z + self.extents.z / 2.0;
        let rear = self.center_of_mass_offset.z - self.extents.z / 2.0;
        for i in 0..10 {
            let fi = i as f32;
            let z_offset = model_offset_z + wheel.offset.z + wheel.radius + 0.3 + 0.3 * fi;
            if z_offset + 0.3 <= front && !self.is_overlapping_wheel(z_offset)? {
                self.crawl_offsets.push(z_offset);
            }
            let z_offset = model_offset_z + wheel.offset.z - wheel.radius - 0.3 - 0.3 * fi;
            if z_offset - 0.3 >= rear && !self.is_overlapping_wheel(z_offset)? {
                self.crawl_offsets.push(z_offset);
            }
        }
        Ok(())
    }

    /// zombie.scripting.objects.VehicleScript.isOverlappingWheel
    fn is_overlapping_wheel(&self, z_offset: f32) -> R<bool> {
        let model_offset_z = self.models.first().map_or(0.0, |m| m.offset.z);
        for wheel in &self.wheels {
            if Self::is_left(wheel)?
                && (model_offset_z + wheel.offset.z - z_offset).abs() < (wheel.radius + 0.3) * 0.99
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// zombie.scripting.objects.VehicleScript.LoadModel: `getModelById` (two null-or-whitespace ids match, else equal
/// ids), else a new model with the block's id.
fn load_model(block: &Block, models: &mut Vec<Model>) -> R<()> {
    let id = block.id.as_deref();
    let found = models.iter().position(|m| {
        (string_utils::is_null_or_whitespace(m.id.as_deref())
            && string_utils::is_null_or_whitespace(id))
            || same_id(&m.id, id)
    });
    let i = match found {
        Some(i) => i,
        None => {
            models.push(Model {
                id: block.id.clone(),
                ..Model::default()
            });
            models.len() - 1
        }
    };
    let model = &mut models[i];
    for value in &block.values {
        let (k, v) = key_value(value);
        if k == "file" {
            model.file = Some(v.to_string());
        } else if k == "offset" {
            load_vector3f(v, &mut model.offset)?;
        } else if k == "rotate" {
            load_vector3f(v, &mut model.rotate)?;
        } else if k == "scale" {
            model.scale = parse_float(v)?;
        } else if k == "attachmentParent" {
            model.attachment_name_parent = empty_null(v);
        } else if k == "attachmentSelf" {
            model.attachment_name_self = empty_null(v);
        } else if java::equals_ignore_case(k, "ignoreVehicleScale") {
            model.ignore_vehicle_scale = parse_boolean(v);
        }
    }
    Ok(())
}

/// zombie.scripting.objects.VehicleScript.LoadSkin: a new skin from the nine texture names.
fn load_skin(block: &Block) -> Skin {
    let mut skin = Skin::default();
    for value in &block.values {
        let (k, v) = key_value(value);
        let slot = match k {
            "texture" => &mut skin.texture,
            "textureRust" => &mut skin.texture_rust,
            "textureMask" => &mut skin.texture_mask,
            "textureLights" => &mut skin.texture_lights,
            "textureDamage1Overlay" => &mut skin.texture_damage1_overlay,
            "textureDamage1Shell" => &mut skin.texture_damage1_shell,
            "textureDamage2Overlay" => &mut skin.texture_damage2_overlay,
            "textureDamage2Shell" => &mut skin.texture_damage2_shell,
            "textureShadow" => &mut skin.texture_shadow,
            _ => continue,
        };
        *slot = discard(v);
    }
    skin
}

/// zombie.scripting.objects.VehicleScript.LoadAnim: `getAnimationById`, else a new anim whose id is
/// `block.id.intern()` (null is a NullPointerException).
fn load_anim(block: &Block, anims: &mut Vec<Anim>) -> R<()> {
    let i = match anims
        .iter()
        .position(|a| same_id(&a.id, block.id.as_deref()))
    {
        Some(i) => i,
        None => {
            let id = block.id.clone().ok_or(NPE)?;
            anims.push(Anim {
                id: Some(id),
                ..Anim::default()
            });
            anims.len() - 1
        }
    };
    let anim = &mut anims[i];
    for value in &block.values {
        let (k, v) = key_value(value);
        if k == "angle" {
            load_vector3f(v, &mut anim.angle)?;
        } else if k == "anim" {
            anim.anim = Some(v.to_string());
        } else if k == "animate" {
            anim.animate = parse_boolean(v);
        } else if k == "loop" {
            anim.r#loop = parse_boolean(v);
        } else if k == "reverse" {
            anim.reverse = parse_boolean(v);
        } else if k == "rate" {
            anim.rate = parse_float(v)?;
        } else if k == "offset" {
            load_vector3f(v, &mut anim.offset)?;
        } else if k == "sound" {
            anim.sound = Some(v.to_string());
        }
    }
    Ok(())
}

/// zombie.scripting.objects.VehicleScript.LoadPassengerSwitchSeat: an empty block removes the switch seat of that
/// id; else it is found or added (`id = block.id`) and filled.
fn load_passenger_switch_seat(block: &Block, passenger: &mut Passenger) -> R<()> {
    let found = passenger
        .switch_seats
        .iter()
        .position(|s| same_id(&s.id, block.id.as_deref()));
    if block.elements.is_empty() {
        if let Some(i) = found {
            passenger.switch_seats.remove(i);
        }
        return Ok(());
    }
    let i = match found {
        Some(i) => i,
        None => {
            passenger.switch_seats.push(SwitchSeat {
                id: block.id.clone(),
                seat: 0,
                anim: None,
                rate: 1.0,
                sound: None,
            });
            passenger.switch_seats.len() - 1
        }
    };
    let switch_seat = &mut passenger.switch_seats[i];
    for value in &block.values {
        let (k, v) = key_value(value);
        if k == "anim" {
            switch_seat.anim = Some(v.to_string());
        } else if k == "rate" {
            switch_seat.rate = parse_float(v)?;
        } else if k == "sound" {
            switch_seat.sound = empty_null(v);
        }
    }
    Ok(())
}

/// zombie.scripting.objects.VehicleScript.LoadPosition: an empty block removes the position of that id; else it is
/// found or added (`id = block.id.intern()`, null is a NullPointerException) and filled.
fn load_position(block: &Block, positions: &mut Vec<Position>) -> R<()> {
    let found = positions
        .iter()
        .position(|p| same_id(&p.id, block.id.as_deref()));
    if block.elements.is_empty() {
        if let Some(i) = found {
            positions.remove(i);
        }
        return Ok(());
    }
    let i = match found {
        Some(i) => i,
        None => {
            let id = block.id.clone().ok_or(NPE)?;
            positions.push(Position {
                id: Some(id),
                ..Position::default()
            });
            positions.len() - 1
        }
    };
    let position = &mut positions[i];
    for value in &block.values {
        let (k, v) = key_value(value);
        if k == "rotate" {
            load_vector3f(v, &mut position.rotate)?;
        } else if k == "offset" {
            load_vector3f(v, &mut position.offset)?;
        } else if k == "area" {
            position.area = empty_null(v);
        }
    }
    Ok(())
}

/// zombie.scripting.objects.VehicleScript.LoadPhysicsShape: `box`, `sphere` or `mesh` (any other id is no shape, a
/// null id a NullPointerException); a box without positive extents, a sphere without a positive radius and a mesh
/// without a shape script are dropped, a mesh without a positive x extent gets extents 1.
fn load_physics_shape(block: &Block) -> R<Option<PhysicsShape>> {
    let r#type = match block.id.as_deref().ok_or(NPE)? {
        "box" => 1,
        "sphere" => 2,
        "mesh" => 3,
        _ => return Ok(None),
    };
    let mut shape = PhysicsShape {
        r#type,
        ..PhysicsShape::default()
    };
    for value in &block.values {
        let (k, v) = key_value(value);
        if java::equals_ignore_case(k, "extents") {
            load_vector3f(v, &mut shape.extents)?;
        } else if java::equals_ignore_case(k, "offset") {
            load_vector3f(v, &mut shape.offset)?;
        } else if java::equals_ignore_case(k, "radius") {
            shape.radius = parse_float(v)?;
        } else if java::equals_ignore_case(k, "rotate") {
            load_vector3f(v, &mut shape.rotate)?;
        } else if java::equals_ignore_case(k, "physicsShapeScript") {
            shape.physics_shape_script = discard(v);
        } else if java::equals_ignore_case(k, "scale") {
            let scale = parse_float(v)?;
            shape.extents = Vector3f::new(scale, scale, scale);
        }
    }
    match shape.r#type {
        1 => {
            if shape.extents.x <= 0.0 || shape.extents.y <= 0.0 || shape.extents.z <= 0.0 {
                return Ok(None);
            }
        }
        2 => {
            if shape.radius <= 0.0 {
                return Ok(None);
            }
        }
        _ => {
            if shape.physics_shape_script.is_none() {
                return Ok(None);
            }
            if shape.extents.x <= 0.0 {
                shape.extents = Vector3f::new(1.0, 1.0, 1.0);
            }
        }
    }
    Ok(Some(shape))
}

/// zombie.scripting.objects.VehicleScript.LoadWindow
fn load_window(block: &Block) -> Window {
    let mut window = Window::default();
    for value in &block.values {
        let (k, v) = key_value(value);
        if k == "openable" {
            window.openable = parse_boolean(v);
        }
    }
    window
}

/// zombie.scripting.objects.VehicleScript.LoadContainer, filling `container` (the existing one, or a new one the
/// caller stores once this returns).
fn load_container(block: &Block, container: &mut Container) -> R<()> {
    for value in &block.values {
        let (k, v) = key_value(value);
        if k == "capacity" {
            container.capacity = parse_int(v)?;
        } else if k == "conditionAffectsCapacity" {
            container.condition_affects_capacity = parse_boolean(v);
        } else if k == "contentType" {
            container.content_type = Some(v.to_string());
        } else if k == "seat" {
            container.seat_id = Some(v.to_string());
        } else if k == "soundMap" {
            let ss = java::split_whitespace_regex(v);
            if ss.len() == 2 && !java::trim(ss[0]).is_empty() {
                put(
                    &mut container.sound_map,
                    java::trim(ss[0]).to_string(),
                    java::trim(ss[1]).to_string(),
                );
            }
        } else if k == "test" {
            container.lua_test = Some(v.to_string());
        }
    }
    Ok(())
}

/// zombie.scripting.objects.VehicleScript.LoadLuaFunctions: a new map of every `key = value`; a value without '='
/// is the original's RuntimeException.
fn load_lua_functions(block: &Block, full_name: &str) -> R<Vec<(String, String)>> {
    let mut map = Vec::new();
    for value in &block.values {
        if !value.string.contains('=') {
            return Err(format!(
                "java.lang.RuntimeException: expected \"key = value\", got \"{}\" in {full_name}",
                java::trim(&value.string)
            ));
        }
        let (k, v) = key_value(value);
        put(&mut map, k.to_string(), v.to_string());
    }
    Ok(map)
}

/// zombie.scripting.objects.VehicleScript.checkIntegerKey: a key of digits only (Character.isDigit; also the empty
/// key, which then fails) is `Double.valueOf` of it, any other the string.
fn check_integer_key(key: &str) -> R<LuaValue> {
    if key.encode_utf16().all(crate::java_chars::is_digit) {
        return java::parse_double(key)
            .map(LuaValue::Number)
            .ok_or_else(|| {
                format!(
                    "java.lang.NumberFormatException: {}",
                    if key.is_empty() {
                        "empty String".to_string()
                    } else {
                        java::number_format_message(key)
                    }
                )
            });
    }
    Ok(LuaValue::String(key.to_string()))
}

/// zombie.scripting.objects.VehicleScript.LoadTable, filling `table` (the existing one or a new one): every value
/// is `rawset(checkIntegerKey(k), v)` (an empty value is nil, which removes the key); every child block fills the
/// table found under its type as a *string* key (`rawget(child.type)`), or a new one, and is set under
/// `checkIntegerKey(type)`.
fn load_table(block: &Block, table: &mut KahluaTable) -> R<()> {
    for value in &block.values {
        let (k, v) = key_value(value);
        let key = check_integer_key(k)?;
        if v.is_empty() {
            table.rawset_nil(&key);
        } else {
            table.rawset(key, LuaValue::String(v.to_string()));
        }
    }
    for child in &block.children {
        let string_key = LuaValue::String(child.r#type.clone());
        let existing = match table.rawget(&string_key) {
            Some(LuaValue::Table(t)) => Some(t.clone()),
            _ => None,
        };
        let is_existing = existing.is_some();
        let mut table2 = existing.unwrap_or_default();
        let result = load_table(child, &mut table2);
        if is_existing {
            // The existing table was filled in place, also when the load failed part way.
            table.rawset(string_key, LuaValue::Table(table2.clone()));
        }
        result?;
        table.rawset(check_integer_key(&child.r#type)?, LuaValue::Table(table2));
    }
    Ok(())
}
