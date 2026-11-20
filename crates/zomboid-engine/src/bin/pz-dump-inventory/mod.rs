//! `pz-dump inventory <game root> <case file>`: InventoryProbe.java on the port. The case lines are those of the probe
//! (see its header); the output is the probe's, byte for byte.
use std::fmt::Write as _;
use std::path::Path;

use zomboid_engine::java;
use zomboid_engine::java_nio::{ByteBuffer, ByteBufferWriter};
use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::{KahluaTable, LuaValue};
use zomboid_engine::se::krka::kahlua::vm::object::Object;
use zomboid_engine::zombie::inventory::inventory_item::ItemType;
use zomboid_engine::zombie::inventory::runtime::item::Kind;
use zomboid_engine::zombie::inventory::runtime::{
    ContainerId, ItemId, LiveItem, Pred, Scripts, World,
};
use zomboid_engine::zombie::iso::chunk_save::WORLD_VERSION;
use zomboid_engine::zombie::lua::lua_manager::{run_lua, LuaManager};
use zomboid_engine::zombie::sandbox_options::SandboxOptions;
use zomboid_engine::zombie::scripting::item::Value;
use zomboid_engine::zombie::scripting::objects::resource_location::ResourceLocation;
use zomboid_engine::zombie::scripting::script_manager::ScriptManager;

fn hx(f: f32) -> String {
    format!("{:x}", f.to_bits())
}

/// InventoryProbe.hex
fn hexs(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// InventoryProbe.unhex
fn unhex(h: &str) -> Result<Vec<u8>, String> {
    (0..h.len() / 2)
        .map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

/// InventoryProbe.bits
fn bits_text(w: &ByteBufferWriter) -> String {
    format!("len={} {}", w.position(), hexs(w.bytes()))
}

fn opt_str(s: &Option<String>) -> String {
    s.clone().unwrap_or_else(|| "null".to_string())
}

/// `String.valueOf(List<String>)`: `[a, b]`
fn list_text(l: &[String]) -> String {
    format!("[{}]", l.join(", "))
}

/// InventoryProbe.tableText
fn table_text(t: &KahluaTable) -> String {
    let mut b = String::from("{");
    for (n, (k, v)) in t.entries.iter().enumerate() {
        if n > 0 {
            b.push(',');
        }
        let value = |v: &LuaValue| match v {
            LuaValue::String(s) => format!("s:{s}"),
            LuaValue::Number(d) => format!("d:{}", d.to_bits() as i64),
            LuaValue::Boolean(x) => format!("b:{x}"),
            LuaValue::Table(t) => table_text(t),
        };
        let _ = write!(b, "{}={}", value(k), value(v));
    }
    b.push('}');
    b
}

/// InventoryProbe.fields: the fields the save writes that `show` does not print, as the probe reads them.
fn fields_text(it: &LiveItem) -> String {
    let mut b = String::new();
    let mut put = |n: &str, v: String| {
        let _ = write!(b, " {n}={v}");
    };
    put(
        "table",
        it.mod_data.as_ref().map_or("null".into(), table_text),
    );
    put("customName", it.custom_name.to_string());
    put("customColor", it.custom_color.to_string());
    put(
        "col",
        format!(
            "rgba:{},{},{},{}",
            hx(it.col[0]),
            hx(it.col[1]),
            hx(it.col[2]),
            hx(it.col[3])
        ),
    );
    put("itemCapacity", f(it.item_capacity));
    put("keyId", it.key_id.to_string());
    put("remoteControlId", it.remote_control_id.to_string());
    put("remoteRange", it.remote_range.to_string());
    put("colorRed", f(it.color_red));
    put("colorGreen", f(it.color_green));
    put("colorBlue", f(it.color_blue));
    put("worker", opt_str(&it.worker));
    put("stashMap", opt_str(&it.stash_map));
    put("zombieInfected", it.zombie_infected.to_string());
    put("attachedSlot", it.attached_slot.to_string());
    put("attachedSlotType", opt_str(&it.attached_slot_type));
    put("attachedToModel", opt_str(&it.attached_to_model));
    put("maxCapacity", it.max_capacity.to_string());
    put("recordedMediaIndex", it.recorded_media_index.to_string());
    put("worldXRotation", f(it.world_x_rotation));
    put("worldYRotation", f(it.world_y_rotation));
    put("worldZRotation", f(it.world_z_rotation));
    put("worldScale", f(it.world_scale));
    put("isInitialised", it.is_initialised.to_string());
    put("modelIndex", it.model_index.to_string());
    put("activated", it.activated.to_string());
    put("haveBeenRepaired", it.have_been_repaired.to_string());
    put("favorite", it.favorite.to_string());
    put(
        "byteData",
        it.byte_data
            .as_ref()
            .map_or("null".into(), |d| format!("bytes:{}", hexs(d))),
    );
    put("extraItems", list_text(&it.extra_items));
    put("name", it.name.clone());
    put("originalName", it.original_name.clone());
    put("registryId", it.registry_id.to_string());
    put("currentAmmoCount", it.current_ammo_count.to_string());
    put("customWeight", it.custom_weight.to_string());
    put("isCookable", it.is_cookable.to_string());
    match &it.kind {
        Kind::Food(fd) => {
            let mut put = |n: &str, v: String| {
                let _ = write!(b, " F.{n}={v}");
            };
            put("calories", f(fd.calories));
            put("proteins", f(fd.proteins));
            put("lipids", f(fd.lipids));
            put("carbohydrates", f(fd.carbohydrates));
            put("dangerousUncooked", fd.dangerous_uncooked.to_string());
            put(
                "poisonDetectionLevel",
                fd.poison_detection_level.to_string(),
            );
            put(
                "spices",
                fd.spices.as_ref().map_or("null".into(), |s| list_text(s)),
            );
            put("poisonPower", fd.poison_power.to_string());
            put("chef", opt_str(&fd.chef));
            put("painReduction", f(fd.pain_reduction));
            put("fluReduction", fd.flu_reduction.to_string());
            put("foodSicknessChange", fd.food_sickness_change.to_string());
            put("poison", fd.poison.to_string());
            put("useForPoison", fd.use_for_poison.to_string());
            put("rottenTime", f(fd.rotten_time));
            put("compostTime", f(fd.compost_time));
            put("cookedInMicrowave", fd.cooked_in_microwave.to_string());
            put("fatigueChange", f(it.fatigue_change));
            put("endChange", f(fd.end_change));
            put("milkQty", fd.milk_qty.to_string());
            put("milkType", opt_str(&fd.milk_type));
            put("stressChange", f(it.stress_change));
            put("frozen", fd.frozen.to_string());
            put("isCookable", it.is_cookable.to_string());
        }
        Kind::Literature(l) => {
            let mut put = |n: &str, v: String| {
                let _ = write!(b, " L.{n}={v}");
            };
            put("numberOfPages", l.number_of_pages.to_string());
            put("alreadyReadPages", l.already_read_pages.to_string());
            put("canBeWrite", l.can_be_write.to_string());
            put(
                "customPages",
                l.custom_pages.as_ref().map_or("null".into(), |p| {
                    let v: Vec<String> = p
                        .iter()
                        .enumerate()
                        .map(|(i, t)| format!("{}={t}", i + 1))
                        .collect();
                    format!("{{{}}}", v.join(", "))
                }),
            );
            put("lockedBy", opt_str(&l.locked_by));
            put(
                "learnedRecipes",
                l.learned_recipes
                    .as_ref()
                    .map_or("null".into(), |s| list_text(s)),
            );
            put("alreadyRead", l.already_read.to_string());
        }
        _ => {}
    }
    b
}

/// ZomboidGlobals.Load of the real defines.lua: `equippedOrWornEncumbranceMultiplier`.
fn equipped_multiplier(root: &Path) -> Result<f64, String> {
    let mut lm = LuaManager::init(root).map_err(|e| format!("lua: {e:?}"))?;
    let defines = root.join("media/lua/shared/defines.lua");
    run_lua(
        &lm.loader,
        &lm.platform,
        &lm.env,
        &mut lm.thread,
        &defines.to_string_lossy(),
    )
    .map_err(|e| format!("defines.lua: {e:?}"))?;
    let mut g = zomboid_engine::zombie::zomboid_globals::ZomboidGlobals::default();
    g.load(&lm.env)
        .map_err(|e| format!("ZomboidGlobals.Load: {e:?}"))?;
    Ok(g.field("equippedOrWornEncumbranceMultiplier"))
}

pub fn inventory(root: &Path, cases: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(cases).map_err(|e| format!("{}: {e}", cases.display()))?;
    let manager = ScriptManager::load(root)?;
    let scripts = Scripts::new(&manager);
    let multiplier = equipped_multiplier(root)?;
    let mut out = String::new();
    let mut sc: Option<Scenario> = None;
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let p: Vec<&str> = l.split_whitespace().collect();
        match p[0] {
            "create" => create(&mut out, &scripts, multiplier)?,
            "reset" => {
                let mut s = Scenario::new(&scripts, multiplier, &p)?;
                s.reset(&mut out, &p)?;
                sc = Some(s);
            }
            _ => {
                let s = sc.as_mut().ok_or_else(|| format!("no scenario for {l}"))?;
                s.run(&mut out, &p)?;
            }
        }
    }
    Ok(out)
}

/// Float.toString and the raw bits, as the probe's `f`.
fn f(v: f32) -> String {
    format!("{}({})", hx(v), java::float_to_string(v))
}

/// Integer.parseInt of a case argument.
fn int(s: &str) -> Result<i32, String> {
    java::parse_int(s).ok_or_else(|| format!("NumberFormatException: {s}"))
}

/// Float.parseFloat of a case argument.
fn float(s: &str) -> Result<f32, String> {
    java::parse_float(s).ok_or_else(|| format!("NumberFormatException: {s}"))
}

/// The state of InventoryProbe's scenario: a world, the named containers (in the order they were made) and variables.
struct Scenario<'a> {
    world: World<'a>,
    containers: Vec<(String, ContainerId)>,
    vars: Vec<(String, ItemId)>,
}

impl<'a> Scenario<'a> {
    /// A fresh world: `swapRand(seed)` and the clock and sandbox options of the line's `k=v` list.
    fn new(scripts: &'a Scripts<'a>, multiplier: f64, p: &[&str]) -> Result<Self, String> {
        let seed: i64 = p
            .get(1)
            .and_then(|s| s.parse().ok())
            .ok_or("reset needs a seed")?;
        let mut world = World::new(scripts, SandboxOptions::new(), multiplier, seed);
        // InventoryProbe.reset fixes the world's poisonous berry and mushroom
        world.poisonous_berry = Some("Base.BerryGeneric2".to_string());
        world.poisonous_mushroom = Some("Base.MushroomGeneric3".to_string());
        Ok(Scenario {
            world,
            containers: Vec::new(),
            vars: Vec::new(),
        })
    }

    /// InventoryProbe.clock: the clock and sandbox options of a `k=v,...` list.
    fn clock(&mut self, list: &str) -> Result<(), String> {
        for e in list.split(',') {
            let (k, v) = e.split_once('=').ok_or_else(|| format!("option {e}"))?;
            match k {
                "tod" => self.world.game_time.time_of_day = float(v)?,
                "nights" => self.world.game_time.nights_survived = int(v)?,
                "mult" => self.world.game_time.multiplier = float(v)?,
                _ => {
                    let n = k.strip_prefix("sb.").ok_or_else(|| format!("option {e}"))?;
                    let d: f64 = v
                        .parse()
                        .map_err(|_| format!("NumberFormatException: {v}"))?;
                    if !self.world.sandbox.set_by_name(n, &Object::Number(d)) {
                        return Err(format!("no sandbox option {n}"));
                    }
                }
            }
        }
        Ok(())
    }

    /// InventoryProbe.reset (the generator and the clock are those `new` made)
    fn reset(&mut self, out: &mut String, p: &[&str]) -> Result<(), String> {
        if let Some(opts) = p.get(2) {
            self.clock(opts)?;
        }
        let w = &self.world;
        let gt = &w.game_time;
        let _ = writeln!(
            out,
            "RESET {} mult={} tod={} nights={} ageHours={} foodRot={} fridge={} elecShut={} removal={} apo={}",
            p.join(" "),
            f(gt.get_multiplier()),
            f(gt.time_of_day),
            gt.nights_survived,
            java::double_to_string(gt.get_world_age_hours()),
            w.sandbox_int("FoodRotSpeed")?,
            w.sandbox_int("FridgeFactor")?,
            w.sandbox_int("ElecShutModifier")?,
            w.sandbox_int("DaysForRottenFoodRemoval")?,
            w.sandbox_int("TimeSinceApo")?,
        );
        Ok(())
    }

    fn item(&self, v: &str) -> Result<ItemId, String> {
        (self.vars.iter().find(|(n, _)| n == v))
            .map(|&(_, i)| i)
            .ok_or_else(|| format!("no variable {v}"))
    }

    fn cont(&self, c: &str) -> Result<ContainerId, String> {
        (self.containers.iter().find(|(n, _)| n == c))
            .map(|&(_, i)| i)
            .ok_or_else(|| format!("no container {c}"))
    }

    fn put_var(&mut self, name: &str, id: ItemId) {
        match self.vars.iter_mut().find(|(n, _)| n == name) {
            Some(e) => e.1 = id,
            None => self.vars.push((name.to_string(), id)),
        }
    }

    /// InventoryProbe.name(ItemContainer)
    fn name(&self, c: Option<ContainerId>) -> &str {
        match c {
            None => "-",
            Some(c) => {
                (self.containers.iter().find(|&&(_, i)| i == c)).map_or("?", |(n, _)| n.as_str())
            }
        }
    }

    /// InventoryProbe.ref
    fn refer(&self, i: Option<ItemId>) -> String {
        match i {
            None => "null".to_string(),
            Some(i) => format!(
                "{}@{}",
                self.world.items[i].full_type, self.world.items[i].id
            ),
        }
    }

    fn refs(&self, l: &[ItemId]) -> String {
        let v: Vec<String> = l.iter().map(|&i| self.refer(Some(i))).collect();
        format!("[{}]", v.join(","))
    }

    /// InventoryProbe.show
    fn show(&self, id: ItemId) -> String {
        let w = &self.world;
        let it = &w.items[id];
        let mut b = String::new();
        let _ = write!(
            b,
            "{} {} in={} cond={}/{} uses={}/{} count={} age={} lastAged={} aw={} w={} uw={} ew={} customW={} cook={}{}{} off={}/{}",
            it.class_name(),
            self.refer(Some(id)),
            self.name(it.container),
            it.condition,
            it.condition_max,
            w.current_uses(id),
            w.max_uses(id),
            it.count,
            f(it.age),
            f(it.last_aged),
            f(w.actual_weight(id)),
            f(w.weight(id)),
            f(w.unequipped_weight(id)),
            f(w.equipped_weight(id)),
            u8::from(it.custom_weight),
            f(it.cooking_time),
            if it.cooked { " cooked" } else { "" },
            if it.burnt { " burnt" } else { "" },
            it.off_age,
            it.off_age_max
        );
        let _ = write!(
            b,
            " wet={} wetCd={}",
            u8::from(it.is_wet),
            f(it.wet_cooldown)
        );
        match &it.kind {
            Kind::Food(fd) => {
                let _ = write!(
                    b,
                    " heat={} hung={} hunger={} base={} thirst={} unhappy={} boredom={} stress={} endur={} rotten={} fresh={} frozen={} freezing={} lastFrozen={} lastCook={} tempAccum={} cal={} poison={} tainted={}",
                    f(fd.heat),
                    f(fd.hung_change),
                    f(w.food_hunger_change(id)),
                    f(fd.base_hunger),
                    f(w.food_thirst_change(id)),
                    f(w.food_unhappy_change(id)),
                    f(w.food_boredom_change(id)),
                    f(w.food_stress_change(id)),
                    f(w.food_endurance_change(id)),
                    u8::from(w.food_is_rotten(id)),
                    u8::from(w.food_is_fresh(id)),
                    u8::from(fd.frozen),
                    f(fd.freezing_time),
                    f(fd.last_frozen_update),
                    fd.last_cook_minute,
                    f(fd.temperature_time_accum),
                    f(fd.calories),
                    fd.poison_power,
                    u8::from(fd.is_tainted)
                );
            }
            Kind::Drainable(d) => {
                let _ = write!(
                    b,
                    " useDelta={} ticks={} heat={} weightEmpty={} lastUpd={} lastCook={}",
                    f(d.use_delta),
                    f(d.ticks),
                    f(d.heat),
                    f(d.weight_empty),
                    d.last_update_minutes,
                    d.last_cook_minute
                );
            }
            Kind::Literature(l) => {
                let _ = write!(b, " pages={}", l.number_of_pages);
            }
            Kind::Combo => {}
        }
        b
    }

    fn tag(a: &str) -> Result<ResourceLocation, String> {
        ResourceLocation::of(a)
    }

    /// InventoryProbe.query
    fn query(&mut self, out: &mut String, p: &[&str]) -> Result<(), String> {
        let c = self.cont(p[1])?;
        let q = p[2];
        let a = p.get(3).copied();
        let arg = || a.ok_or_else(|| format!("query {q} needs an argument"));
        // the item a query names (when it names one), found before the world is borrowed
        let named = a.and_then(|v| self.item(v).ok());
        let item_arg = || named.ok_or_else(|| format!("query {q}: no variable"));
        let w = &mut self.world;
        let r = match q {
            "contains" => {
                let i = item_arg()?;
                w.containers[c].items.contains(&i).to_string()
            }
            "has" => w.contains_type(c, arg()?, false).to_string(),
            "hasr" => w.contains_type(c, arg()?, false).to_string(),
            "hasnb" => w.contains_type(c, arg()?, true).to_string(),
            "count" => w.get_count(c, &Pred::Type(arg()?.to_string())).to_string(),
            "alltype" => {
                let l = w.get_all(c, &Pred::Type(arg()?.to_string()));
                self.refs(&l)
            }
            "alltag" => {
                let l = w.get_all(c, &Pred::Tag(Self::tag(arg()?)?));
                self.refs(&l)
            }
            "allcat" => {
                let l = w.get_all(c, &Pred::Category(arg()?.to_string()));
                self.refs(&l)
            }
            "counttag" => w.get_count(c, &Pred::Tag(Self::tag(arg()?)?)).to_string(),
            "firsttype" => {
                let i = w.get_first(c, &Pred::Type(arg()?.to_string()));
                self.refer(i)
            }
            "firstcat" => {
                let i = w.get_first(c, &Pred::Category(arg()?.to_string()));
                self.refer(i)
            }
            "firsttag" => {
                let i = w.get_first(c, &Pred::Tag(Self::tag(arg()?)?));
                self.refer(i)
            }
            "some" => {
                let n = int(p.get(4).ok_or("some needs a count")?)?;
                let l = w.get_some(c, &Pred::Type(arg()?.to_string()), n);
                self.refs(&l)
            }
            "best" => {
                let i = w.get_best_condition(c, &Pred::Type(arg()?.to_string()));
                self.refer(i)
            }
            "find" => {
                let i = w.find_item(c, arg()?, false);
                self.refer(i)
            }
            "numberof" => w.number_of_item(c, arg()?, false).to_string(),
            "numberofdepl" => w.number_of_item(c, arg()?, true).to_string(),
            "withmodule" => w.contains_with_module(c, arg()?, false).to_string(),
            "withmoduledelta" => w.contains_with_module(c, arg()?, true).to_string(),
            "stack" => {
                let i = w.find_and_return_stack(c, arg()?)?;
                self.refer(i)
            }
            "room" => {
                let i = item_arg()?;
                w.has_room_for_item(c, i).to_string()
            }
            "roomw" => w.has_room_for(c, float(arg()?)?).to_string(),
            "allowed" => {
                let i = item_arg()?;
                w.is_item_allowed(c, i).to_string()
            }
            "weight" => format!(
                "capacityWeight={} contents={} max={} capacity={} effective={} free={} full={} available={} empty={} size={} temp={}",
                f(w.capacity_weight(c)),
                f(w.container_contents_weight(c)),
                f(w.container_max_weight(c)),
                w.container_capacity(c),
                w.effective_capacity(c),
                f(w.free_capacity(c)),
                w.is_full(c),
                f(w.available_weight_capacity(c)),
                w.containers[c].items.is_empty(),
                w.containers[c].items.len(),
                f(w.container_temperature(c)?)
            ),
            _ => return Err(format!("query {q}")),
        };
        let _ = writeln!(out, "Q {} = {r}", p.join(" "));
        Ok(())
    }

    /// InventoryProbe.advance: the clock moves `dtod` hours, a day over at 24.
    fn advance(&mut self, dtod: f32) {
        let gt = &mut self.world.game_time;
        let mut t = gt.time_of_day + dtod;
        let mut nights = gt.nights_survived;
        while t >= 24.0f32 {
            t -= 24.0f32;
            nights += 1;
        }
        gt.time_of_day = t;
        gt.nights_survived = nights;
    }

    /// InventoryProbe.set
    fn set(&mut self, p: &[&str]) -> Result<(), String> {
        let id = self.item(p[1])?;
        let v = *p.get(3).ok_or("set needs a value")?;
        let w = &mut self.world;
        let is_food = matches!(w.items[id].kind, Kind::Food(_));
        match p[2] {
            "cond" => w.set_condition(id, int(v)?)?,
            "uses" => w.set_current_uses_virtual(id, int(v)?),
            "usesf" => w.set_current_uses_float(id, float(v)?),
            "age" => w.items[id].age = float(v)?,
            "lastaged" => w.items[id].last_aged = float(v)?,
            "count" => w.items[id].count = int(v)?,
            "cooked" => w.set_cooked(id, java::equals_ignore_case(v, "true")),
            "burnt" => w.set_burnt(id, java::equals_ignore_case(v, "true")),
            "cooktime" => w.items[id].cooking_time = float(v)?,
            "freeze" if is_food => w.set_freezing_time(id, float(v)?),
            "heat" if is_food => w.set_food_heat(id, float(v)?),
            "hung" if is_food => {
                if let Kind::Food(fd) = &mut w.items[id].kind {
                    fd.hung_change = float(v)?;
                }
            }
            "taint" if is_food => {
                if let Kind::Food(fd) = &mut w.items[id].kind {
                    fd.is_tainted = java::equals_ignore_case(v, "true");
                }
            }
            "lastcook" if is_food => {
                if let Kind::Food(fd) = &mut w.items[id].kind {
                    fd.last_cook_minute = int(v)?;
                }
            }
            "wet" => w.items[id].is_wet = java::equals_ignore_case(v, "true"),
            "wetcd" => w.items[id].wet_cooldown = float(v)?,
            "dheat" => {
                if let Kind::Drainable(d) = &mut w.items[id].kind {
                    d.heat = float(v)?;
                } else {
                    return Err("ClassCastException: not a DrainableComboItem".to_string());
                }
            }
            "off" => w.items[id].off_age = int(v)?,
            "offmax" => w.items[id].off_age_max = int(v)?,
            _ => return Err(format!("field {}", p[2])),
        }
        Ok(())
    }

    /// InventoryProbe.field: a field of the item set directly (the value is read by the field's own type).
    fn field(&mut self, p: &[&str]) -> Result<(), String> {
        let id = self.item(p[1])?;
        let v = p[3];
        let b = || java::equals_ignore_case(v, "true");
        let s = || (v != "null").then(|| v.to_string());
        let w = &mut self.world;
        let it = &mut w.items[id];
        match p[2] {
            "customColor" => it.custom_color = b(),
            "col" => {
                let x: Vec<&str> = v.split(',').collect();
                if x.len() != 4 {
                    return Err(format!("field col {v}"));
                }
                // new Color(r, g, b, a) clamps to [0, 1]
                for (n, c) in x.iter().enumerate() {
                    it.col[n] = float(c)?.clamp(0.0, 1.0);
                }
            }
            "itemCapacity" => it.item_capacity = float(v)?,
            "customName" => it.custom_name = b(),
            "keyId" => it.key_id = int(v)?,
            "remoteControlId" => it.remote_control_id = int(v)?,
            "remoteRange" => it.remote_range = int(v)?,
            "colorRed" => it.color_red = float(v)?,
            "colorGreen" => it.color_green = float(v)?,
            "colorBlue" => it.color_blue = float(v)?,
            "worker" => it.worker = s(),
            "stashMap" => it.stash_map = s(),
            "zombieInfected" => it.zombie_infected = b(),
            "attachedSlot" => it.attached_slot = int(v)?,
            "attachedSlotType" => it.attached_slot_type = s(),
            "attachedToModel" => it.attached_to_model = s(),
            "maxCapacity" => it.max_capacity = int(v)?,
            "recordedMediaIndex" => {
                it.recorded_media_index = v
                    .parse()
                    .map_err(|_| format!("NumberFormatException: {v}"))?
            }
            "worldXRotation" => it.world_x_rotation = float(v)?,
            "worldYRotation" => it.world_y_rotation = float(v)?,
            "worldZRotation" => it.world_z_rotation = float(v)?,
            "worldScale" => it.world_scale = float(v)?,
            "isInitialised" => it.is_initialised = b(),
            "modelIndex" => it.model_index = int(v)?,
            "activated" => it.activated = b(),
            "haveBeenRepaired" => it.have_been_repaired = int(v)?,
            "favorite" => it.favorite = b(),
            "name" => it.name = v.to_string(),
            "currentAmmoCount" => it.current_ammo_count = int(v)?,
            "customWeight" => it.custom_weight = b(),
            "isCookable" => it.is_cookable = b(),
            "stressChange" => it.stress_change = float(v)?,
            "fatigueChange" => it.fatigue_change = float(v)?,
            "unhappyChange" => it.unhappy_change = float(v)?,
            "boredomChange" => it.boredom_change = float(v)?,
            "cookingTime" => it.cooking_time = float(v)?,
            "cooked" => it.cooked = b(),
            "burnt" => it.burnt = b(),
            "offAge" => it.off_age = int(v)?,
            "offAgeMax" => it.off_age_max = int(v)?,
            "age" => it.age = float(v)?,
            "lastAged" => it.last_aged = float(v)?,
            "wetCooldown" => it.wet_cooldown = float(v)?,
            _ => match &mut it.kind {
                Kind::Food(fd) => match p[2] {
                    "calories" => fd.calories = float(v)?,
                    "proteins" => fd.proteins = float(v)?,
                    "lipids" => fd.lipids = float(v)?,
                    "carbohydrates" => fd.carbohydrates = float(v)?,
                    "dangerousUncooked" => fd.dangerous_uncooked = b(),
                    "poisonDetectionLevel" => fd.poison_detection_level = int(v)?,
                    "poisonPower" => fd.poison_power = int(v)?,
                    "chef" => fd.chef = s(),
                    "painReduction" => fd.pain_reduction = float(v)?,
                    "fluReduction" => fd.flu_reduction = int(v)?,
                    "foodSicknessChange" => fd.food_sickness_change = int(v)?,
                    "poison" => fd.poison = b(),
                    "useForPoison" => fd.use_for_poison = int(v)?,
                    "rottenTime" => fd.rotten_time = float(v)?,
                    "compostTime" => fd.compost_time = float(v)?,
                    "cookedInMicrowave" => fd.cooked_in_microwave = b(),
                    "endChange" => fd.end_change = float(v)?,
                    "hungChange" => fd.hung_change = float(v)?,
                    "baseHunger" => fd.base_hunger = float(v)?,
                    "thirstChange" => fd.thirst_change = float(v)?,
                    "heat" => fd.heat = float(v)?,
                    "lastCookMinute" => fd.last_cook_minute = int(v)?,
                    "freezingTime" => fd.freezing_time = float(v)?,
                    "lastFrozenUpdate" => fd.last_frozen_update = float(v)?,
                    "milkQty" => fd.milk_qty = int(v)?,
                    "milkType" => fd.milk_type = s(),
                    "frozen" => fd.frozen = b(),
                    _ => return Err(format!("field {}", p[2])),
                },
                Kind::Literature(l) => match p[2] {
                    "numberOfPages" => l.number_of_pages = int(v)?,
                    "alreadyReadPages" => l.already_read_pages = int(v)?,
                    "canBeWrite" => l.can_be_write = b(),
                    "lockedBy" => l.locked_by = s(),
                    _ => return Err(format!("field {}", p[2])),
                },
                _ => return Err(format!("field {}", p[2])),
            },
        }
        Ok(())
    }

    /// InventoryProbe.list
    fn list(&mut self, p: &[&str]) -> Result<(), String> {
        let id = self.item(p[1])?;
        let x: Option<Vec<String>> = match p[3] {
            "null" => None,
            "empty" => Some(Vec::new()),
            v => Some(v.split(',').map(str::to_string).collect()),
        };
        let it = &mut self.world.items[id];
        match (p[2], &mut it.kind) {
            // Food.setSpices: a null or empty list clears the list the item has (a null stays null)
            ("spices", Kind::Food(fd)) => match (x, &mut fd.spices) {
                (Some(l), spices) if !l.is_empty() => *spices = Some(l),
                (_, Some(spices)) => spices.clear(),
                _ => {}
            },
            ("extraItems", _) => it.extra_items = x.unwrap_or_default(),
            ("learnedRecipes", Kind::Literature(l)) => l.learned_recipes = x,
            ("customPages", Kind::Literature(l)) => l.custom_pages = x,
            _ => return Err(format!("list {}", p[2])),
        }
        Ok(())
    }

    /// InventoryProbe.mod: `getModData()` of the item, a dotted path of keys (`#n` is a number key), a typed value.
    fn mod_data(&mut self, id: ItemId, p: &[&str]) -> Result<(), String> {
        let key = |k: &str| -> Result<LuaValue, String> {
            Ok(match k.strip_prefix('#') {
                Some(n) => LuaValue::Number(
                    n.parse()
                        .map_err(|_| format!("NumberFormatException: {n}"))?,
                ),
                None => LuaValue::String(k.to_string()),
            })
        };
        let path: Vec<&str> = p[2].split('.').collect();
        let mut t = self.world.mod_data(id);
        for k in &path[..path.len() - 1] {
            let k = key(k)?;
            if !matches!(t.rawget(&k), Some(LuaValue::Table(_))) {
                t.rawset(k.clone(), LuaValue::Table(KahluaTable::default()));
            }
            let Some(entry) = t.entries.iter_mut().find(|(ek, _)| ek == &k) else {
                return Err("modData path".to_string());
            };
            let LuaValue::Table(n) = &mut entry.1 else {
                return Err("modData path".to_string());
            };
            t = n;
        }
        let k = key(path[path.len() - 1])?;
        let value = match p[3] {
            "s" => LuaValue::String(p[4].to_string()),
            "d" => LuaValue::Number(
                p[4].parse()
                    .map_err(|_| format!("NumberFormatException: {}", p[4]))?,
            ),
            "b" => LuaValue::Boolean(java::equals_ignore_case(p[4], "true")),
            "t" => LuaValue::Table(KahluaTable::default()),
            _ => return Err(format!("mod type {}", p[3])),
        };
        t.rawset(k, value);
        Ok(())
    }

    /// InventoryProbe.scenario for the commands after `reset`
    fn run(&mut self, out: &mut String, p: &[&str]) -> Result<(), String> {
        match p[0] {
            "container" => {
                let (ty, cap) = (p[2], int(p[3])?);
                let c = self.world.new_container(ty, cap);
                if let Some(opts) = p.get(4) {
                    for e in opts.split(',') {
                        let (k, v) = e
                            .split_once('=')
                            .ok_or_else(|| format!("container option {e}"))?;
                        match k {
                            "accept" => self.world.set_only_accept_category(c, v),
                            "temp" => self.world.containers[c].custom_temperature = float(v)?,
                            "explored" => {
                                self.world.containers[c].explored =
                                    java::equals_ignore_case(v, "true")
                            }
                            "looted" => {
                                self.world.containers[c].has_been_looted =
                                    java::equals_ignore_case(v, "true")
                            }
                            _ => return Err(format!("container option {e}")),
                        }
                    }
                }
                match self.containers.iter_mut().find(|(n, _)| n == p[1]) {
                    Some(e) => e.1 = c,
                    None => self.containers.push((p[1].to_string(), c)),
                }
                let _ = writeln!(
                    out,
                    "CONTAINER {} type={} capacity={}",
                    p[1],
                    self.world.containers[c].type_name,
                    self.world.container_capacity(c)
                );
            }
            "new" => {
                let i = self.world.create_item(p[2])?;
                if let Some(i) = i {
                    self.put_var(p[1], i);
                }
                let shown = i.map_or("null".to_string(), |i| self.show(i));
                let _ = writeln!(out, "NEW {} {shown} draws={}", p[1], self.world.rand.id());
            }
            "add" => {
                let c = self.cont(p[1])?;
                let i = self.world.add_item_type(c, p[3])?;
                if let Some(i) = i {
                    self.put_var(p[2], i);
                }
                let shown = i.map_or("null".to_string(), |i| self.show(i));
                let _ = writeln!(
                    out,
                    "ADD {} {} {shown} draws={}",
                    p[1],
                    p[2],
                    self.world.rand.id()
                );
            }
            "put" => {
                let (c, it) = (self.cont(p[1])?, self.item(p[2])?);
                let i = self.world.add_item(c, it)?;
                let _ = writeln!(
                    out,
                    "PUT {} {} draws={}",
                    p[1],
                    self.refer(Some(i)),
                    self.world.rand.id()
                );
            }
            "remove" => {
                let (c, it) = (self.cont(p[1])?, self.item(p[2])?);
                self.world.remove_item(c, it)?;
                let _ = writeln!(
                    out,
                    "REMOVE {} {} in={}",
                    p[1],
                    p[2],
                    self.name(self.world.items[it].container)
                );
            }
            "removeall" => {
                let c = self.cont(p[1])?;
                let n = match p.get(3) {
                    Some(n) => int(n)?,
                    None => self.world.containers[c].items.len() as i32,
                };
                let l = self.world.remove_all(c, p[2], n);
                let _ = writeln!(out, "REMOVEALL {} {} = {}", p[1], p[2], self.refs(&l));
            }
            "removeone" => {
                let c = self.cont(p[1])?;
                let i = self.world.remove_one_of(c, p[2])?;
                let _ = writeln!(out, "REMOVEONE {} {} = {}", p[1], p[2], self.refer(i));
            }
            "removetype" => {
                let c = self.cont(p[1])?;
                self.world.remove_type(c, p[2])?;
                let _ = writeln!(out, "REMOVETYPE {} {}", p[1], p[2]);
            }
            "set" => {
                self.set(p)?;
                let id = self.item(p[1])?;
                let _ = writeln!(out, "SET {} -> {}", p.join(" "), self.show(id));
            }
            "show" => {
                let id = self.item(p[1])?;
                let _ = writeln!(out, "SHOW {} {}", p[1], self.show(id));
            }
            "items" => {
                let c = self.cont(p[1])?;
                let l = self.world.containers[c].items.clone();
                let _ = writeln!(out, "ITEMS {} {}", p[1], self.refs(&l));
            }
            "q" => self.query(out, p)?,
            "clock" => {
                self.clock(p[1])?;
                let gt = &self.world.game_time;
                let _ = writeln!(
                    out,
                    "CLOCK {} tod={} nights={}",
                    p[1],
                    f(gt.time_of_day),
                    gt.nights_survived
                );
            }
            "pick" => {
                let c = self.cont(p[2])?;
                let n = usize::try_from(int(p[3])?)
                    .map_err(|_| "IndexOutOfBoundsException".to_string())?;
                let i = *self.world.containers[c]
                    .items
                    .get(n)
                    .ok_or("IndexOutOfBoundsException")?;
                self.put_var(p[1], i);
                let _ = writeln!(out, "PICK {} {}", p[1], self.refer(Some(i)));
            }
            "use" => {
                let id = self.item(p[1])?;
                self.world.use_item(id)?;
                let _ = writeln!(
                    out,
                    "USE {} -> {} draws={}",
                    p[1],
                    self.show(id),
                    self.world.rand.id()
                );
            }
            "useitem" => {
                let id = self.item(p[1])?;
                let flag = |n: usize| -> bool {
                    p.get(n)
                        .is_some_and(|v| java::equals_ignore_case(v, "true"))
                };
                let consumes = match p.get(4) {
                    Some(v) => int(v)?,
                    None => 1,
                };
                let consumed = self.world.item_user_use_item(
                    id,
                    flag(2),
                    flag(3),
                    consumes,
                    flag(5),
                    flag(6),
                )?;
                let _ = writeln!(
                    out,
                    "USEITEM {} = {consumed} -> {} draws={}",
                    p.join(" "),
                    self.show(id),
                    self.world.rand.id()
                );
            }
            "tick" => {
                let id = self.item(p[1])?;
                let n = int(p[2])?;
                let dtod = float(p[3])?;
                for _ in 0..n {
                    self.advance(dtod);
                    self.world.update_item(id)?;
                }
                let _ = writeln!(
                    out,
                    "TICK {} -> {} tod={} tm={} draws={}",
                    p.join(" "),
                    self.show(id),
                    f(self.world.game_time.time_of_day),
                    f(self.world.items[id].time_multiplier),
                    self.world.rand.id()
                );
            }
            "field" => {
                self.field(p)?;
                let id = self.item(p[1])?;
                let _ = writeln!(
                    out,
                    "FIELD {} -> {}{}",
                    p.join(" "),
                    self.show(id),
                    fields_text(&self.world.items[id])
                );
            }
            "list" => {
                self.list(p)?;
                let id = self.item(p[1])?;
                let _ = writeln!(
                    out,
                    "LIST {} -> {}",
                    p.join(" "),
                    fields_text(&self.world.items[id])
                );
            }
            "bytes" => {
                let id = self.item(p[1])?;
                self.world.items[id].byte_data = if p[2] == "null" {
                    None
                } else {
                    Some(unhex(p[2])?)
                };
                let _ = writeln!(out, "BYTES {} {}", p[1], p[2]);
            }
            "mod" => {
                let id = self.item(p[1])?;
                self.mod_data(id, p)?;
                let t = self.world.items[id].mod_data.as_ref().map(table_text);
                let _ = writeln!(
                    out,
                    "MOD {} -> {}",
                    p.join(" "),
                    t.unwrap_or_else(|| "null".into())
                );
            }
            "fields" => {
                let id = self.item(p[1])?;
                let _ = writeln!(out, "FIELDS {}{}", p[1], fields_text(&self.world.items[id]));
            }
            "save" => {
                let id = self.item(p[1])?;
                let mut w = ByteBufferWriter::new();
                self.world.save_item(id, &mut w)?;
                let _ = writeln!(out, "SAVE {} {}", p[1], bits_text(&w));
            }
            "roundtrip" => {
                let id = self.item(p[1])?;
                let mut w = ByteBufferWriter::new();
                self.world.save_item_with_size(id, &mut w)?;
                let back = self
                    .world
                    .load_item(&mut ByteBuffer::wrap(w.bytes()), WORLD_VERSION)?;
                if let Some(b) = back {
                    self.put_var(p[2], b);
                }
                let _ = writeln!(
                    out,
                    "ROUNDTRIP {} {} {} draws={}",
                    p[1],
                    p[2],
                    bits_text(&w),
                    self.world.rand.id()
                );
                match back {
                    None => {
                        let _ = writeln!(out, "BACK {} null", p[2]);
                    }
                    Some(b) => {
                        let mut again = ByteBufferWriter::new();
                        self.world.save_item_with_size(b, &mut again)?;
                        let _ = writeln!(
                            out,
                            "BACK {} {}{} again={}",
                            p[2],
                            self.show(b),
                            fields_text(&self.world.items[b]),
                            bits_text(&again)
                        );
                    }
                }
            }
            "csave" => {
                let c = self.cont(p[1])?;
                let mut w = ByteBufferWriter::new();
                self.world.container_save(c, &mut w)?;
                let _ = writeln!(out, "CSAVE {} {}", p[1], bits_text(&w));
            }
            "cload" => {
                let c = self.cont(p[1])?;
                let mut w = ByteBufferWriter::new();
                self.world.container_save(c, &mut w)?;
                let n = self.world.new_container("", 50);
                let mut inp = ByteBuffer::wrap(w.bytes());
                self.world.container_load(n, &mut inp, WORLD_VERSION)?;
                match self.containers.iter_mut().find(|(x, _)| x == p[2]) {
                    Some(e) => e.1 = n,
                    None => self.containers.push((p[2].to_string(), n)),
                }
                let mut again = ByteBufferWriter::new();
                self.world.container_save(n, &mut again)?;
                let _ = writeln!(
                    out,
                    "CLOAD {} {} type={} capacity={} explored={} looted={} read={}/{} draws={}",
                    p[1],
                    p[2],
                    self.world.containers[n].type_name,
                    self.world.containers[n].capacity,
                    self.world.containers[n].explored,
                    self.world.containers[n].has_been_looted,
                    inp.position(),
                    inp.limit(),
                    self.world.rand.id()
                );
                let loaded: Vec<ItemId> = self.world.containers[n].items.clone();
                for i in loaded {
                    let _ = writeln!(
                        out,
                        "LOADED {} {}{}",
                        p[2],
                        self.show(i),
                        fields_text(&self.world.items[i])
                    );
                }
                let _ = writeln!(out, "CAGAIN {} {}", p[2], bits_text(&again));
            }
            // InventoryProbe `recordedmedia`: the media of a save, which this port does not model (an item that names one is
            // skipped)
            "recordedmedia" => {}
            "rawitem" => {
                // InventoryProbe `rawitem`: a block of a real save, loaded, its state, saved again
                let raw = unhex(p[2])?;
                let mut inp = ByteBuffer::wrap(&raw);
                let back = match self.world.load_item(&mut inp, WORLD_VERSION) {
                    Err(e) if e.starts_with("not ported") => {
                        eprintln!("SKIPPED {}: {e}", p[1]);
                        let _ = writeln!(out, "SKIPPED {}", p[1]);
                        return Ok(());
                    }
                    r => r?,
                };
                let Some(back) = back else {
                    let _ = writeln!(out, "RAWITEM {} null", p[1]);
                    return Ok(());
                };
                if self.world.items[back].full_type == "Base.CorpseAnimal" {
                    let _ = writeln!(out, "SKIPPED {}", p[1]);
                    return Ok(());
                }
                let mut again = ByteBufferWriter::new();
                self.world.save_item_with_size(back, &mut again)?;
                let same = again.bytes() == &raw[..];
                let _ = writeln!(
                    out,
                    "RAWITEM {} {}{} read={}/{} same={}{}",
                    p[1],
                    self.show(back),
                    fields_text(&self.world.items[back]),
                    inp.position(),
                    inp.limit(),
                    u8::from(same),
                    if same {
                        String::new()
                    } else {
                        format!(" again={}", bits_text(&again))
                    }
                );
            }
            "rawcont" => {
                // InventoryProbe `rawcont`: the bytes of an ItemContainer.save of a real save
                let raw = unhex(p[2])?;
                let mut inp = ByteBuffer::wrap(&raw);
                let n = self.world.new_container("", 50);
                match self.world.container_load(n, &mut inp, WORLD_VERSION) {
                    Err(e) if e.starts_with("not ported") => {
                        eprintln!("SKIPPED {}: {e}", p[1]);
                        let _ = writeln!(out, "SKIPPED {}", p[1]);
                        return Ok(());
                    }
                    r => {
                        r?;
                    }
                }
                let items: Vec<ItemId> = self.world.containers[n].items.clone();
                if items
                    .iter()
                    .any(|&i| self.world.items[i].full_type == "Base.CorpseAnimal")
                {
                    let _ = writeln!(out, "SKIPPED {}", p[1]);
                    return Ok(());
                }
                let mut again = ByteBufferWriter::new();
                self.world.container_save(n, &mut again)?;
                let same = again.bytes() == &raw[..];
                let c = &self.world.containers[n];
                let _ = writeln!(
                    out,
                    "RAWCONT {} type={} capacity={} explored={} looted={} items={} read={}/{} same={}{}",
                    p[1],
                    c.type_name,
                    c.capacity,
                    c.explored,
                    c.has_been_looted,
                    items.len(),
                    inp.position(),
                    inp.limit(),
                    u8::from(same),
                    if same {
                        String::new()
                    } else {
                        format!(" again={}", bits_text(&again))
                    }
                );
                for i in items {
                    let _ = writeln!(
                        out,
                        "LOADED {} {}{}",
                        p[1],
                        self.show(i),
                        fields_text(&self.world.items[i])
                    );
                }
            }
            "createsave" => {
                let mut all = self.world.scripts.manager.all_items();
                all.sort_by_key(|&i| {
                    self.world
                        .scripts
                        .item(i)
                        .full_name()
                        .encode_utf16()
                        .collect::<Vec<u16>>()
                });
                for i in all {
                    let script = self.world.scripts.item(i);
                    let full = script.full_name();
                    let class = self.world.script_class(i)?;
                    if !modelled(class)
                        || script.has_components()
                        || matches!(script.get("luaCreate"), Some(Value::Str(Some(_))))
                    {
                        continue;
                    }
                    if full == "Base.CorpseAnimal" {
                        let _ = writeln!(out, "SKIP {full} load builds an animal corpse");
                        continue;
                    }
                    let Some(id) = self.world.create_item(&full)? else {
                        let _ = writeln!(out, "SAVED {full} null");
                        continue;
                    };
                    let mut w = ByteBufferWriter::new();
                    self.world.save_item_with_size(id, &mut w)?;
                    let back = self
                        .world
                        .load_item(&mut ByteBuffer::wrap(w.bytes()), WORLD_VERSION)?;
                    let again = match back {
                        None => "null".to_string(),
                        Some(b) => {
                            let mut a = ByteBufferWriter::new();
                            self.world.save_item_with_size(b, &mut a)?;
                            bits_text(&a)
                        }
                    };
                    let _ = writeln!(
                        out,
                        "SAVED {full} {} back={again} draws={}",
                        bits_text(&w),
                        self.world.rand.id()
                    );
                }
            }
            "regs" => {
                let mut all: Vec<(String, usize)> = (0..self.world.scripts.manager.items.len())
                    .map(|i| (self.world.scripts.item(i).full_name(), i))
                    .collect();
                all.sort();
                for (name, _) in all {
                    let id = self.world.scripts.registry_id(&name);
                    let _ = writeln!(out, "REG {name} {id} {id}");
                }
            }
            "ageupdate" => {
                let id = self.item(p[1])?;
                self.world.update_age(id, false)?;
                let _ = writeln!(out, "AGEUPDATE {} -> {}", p[1], self.show(id));
            }
            _ => return Err(format!("case {}", p.join(" "))),
        }
        Ok(())
    }
}

/// Whether the probe creates items of this class at all.
fn modelled(t: ItemType) -> bool {
    matches!(
        t,
        ItemType::Normal | ItemType::Drainable | ItemType::Food | ItemType::Literature
    )
}

/// InventoryProbe.create
fn create(out: &mut String, scripts: &Scripts, multiplier: f64) -> Result<(), String> {
    let mut all = scripts.manager.all_items();
    all.sort_by_key(|&i| {
        scripts
            .item(i)
            .full_name()
            .encode_utf16()
            .collect::<Vec<u16>>()
    });
    let mut world = World::new(scripts, SandboxOptions::new(), multiplier, 0);
    for i in all {
        let script = scripts.item(i);
        let full = script.full_name();
        let class = world.script_class(i)?;
        if !modelled(class) {
            let _ = writeln!(out, "SKIP {full} class {}", class.location());
            continue;
        }
        if script.has_components() {
            let _ = writeln!(out, "SKIP {full} component");
            continue;
        }
        if matches!(script.get("luaCreate"), Some(Value::Str(Some(_)))) {
            let _ = writeln!(out, "SKIP {full} oncreate");
            continue;
        }
        let Some(id) = world.create_item(&full)? else {
            let _ = writeln!(out, "NULL {full}");
            continue;
        };
        let item = &world.items[id];
        let mut tags: Vec<String> = LiveItem::tags_of(script)
            .iter()
            .map(|t| t.to_string())
            .collect();
        tags.sort();
        let _ = writeln!(
            out,
            "ITEM {full} {} cat={} aw={} w={} uw={} ew={} cond={}/{} count={} tags=[{}]",
            item.class_name(),
            item.category(),
            hx(world.actual_weight(id)),
            hx(world.weight(id)),
            hx(world.unequipped_weight(id)),
            hx(world.equipped_weight(id)),
            item.condition,
            item.condition_max,
            item.count,
            tags.join(", ")
        );
    }
    Ok(())
}
