//! `pz-dump inventory <game root> <case file>`: InventoryProbe.java on the port. The case lines are those of the probe
//! (see its header); the output is the probe's, byte for byte.
use std::fmt::Write as _;
use std::path::Path;

use zomboid_engine::java;
use zomboid_engine::se::krka::kahlua::vm::object::Object;
use zomboid_engine::zombie::inventory::inventory_item::ItemType;
use zomboid_engine::zombie::inventory::runtime::item::Kind;
use zomboid_engine::zombie::inventory::runtime::{
    ContainerId, ItemId, LiveItem, Pred, Scripts, World,
};
use zomboid_engine::zombie::lua::lua_manager::{run_lua, LuaManager};
use zomboid_engine::zombie::sandbox_options::SandboxOptions;
use zomboid_engine::zombie::scripting::item::Value;
use zomboid_engine::zombie::scripting::objects::resource_location::ResourceLocation;
use zomboid_engine::zombie::scripting::script_manager::ScriptManager;

fn hx(f: f32) -> String {
    format!("{:x}", f.to_bits())
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
        Ok(Scenario {
            world: World::new(scripts, SandboxOptions::new(), multiplier, seed),
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
        match &it.kind {
            Kind::Food(fd) => {
                let _ = write!(
                    b,
                    " heat={} hung={} hunger={} base={} thirst={} unhappy={} boredom={} stress={} endur={} rotten={} fresh={} frozen={} freezing={} lastFrozen={} lastCook={} tempAccum={} cal={} poison={}",
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
                    fd.poison_power
                );
            }
            Kind::Drainable(d) => {
                let _ = write!(
                    b,
                    " useDelta={} ticks={} heat={} weightEmpty={}",
                    f(d.use_delta),
                    f(d.ticks),
                    f(d.heat),
                    f(d.weight_empty)
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
            "off" => w.items[id].off_age = int(v)?,
            "offmax" => w.items[id].off_age_max = int(v)?,
            _ => return Err(format!("field {}", p[2])),
        }
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
