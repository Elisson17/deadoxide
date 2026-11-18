//! `pz-dump inventory <game root> <case file>`: InventoryProbe.java on the port. The case lines are those of the probe
//! (see its header); the output is the probe's, byte for byte.
use std::fmt::Write as _;
use std::path::Path;

use zomboid_engine::zombie::inventory::inventory_item::ItemType;
use zomboid_engine::zombie::inventory::runtime::{LiveItem, Scripts, World};
use zomboid_engine::zombie::lua::lua_manager::{run_lua, LuaManager};
use zomboid_engine::zombie::sandbox_options::SandboxOptions;
use zomboid_engine::zombie::scripting::item::Value;
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
    let sandbox = SandboxOptions::new();
    let multiplier = equipped_multiplier(root)?;
    let mut out = String::new();
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let p: Vec<&str> = l.split_whitespace().collect();
        match p[0] {
            "create" => create(&mut out, &scripts, &sandbox, multiplier)?,
            _ => return Err(format!("case {l}")),
        }
    }
    Ok(out)
}

/// Whether the probe creates items of this class at all.
fn modelled(t: ItemType) -> bool {
    matches!(
        t,
        ItemType::Normal | ItemType::Drainable | ItemType::Food | ItemType::Literature
    )
}

/// InventoryProbe.create
fn create(
    out: &mut String,
    scripts: &Scripts,
    sandbox: &SandboxOptions,
    multiplier: f64,
) -> Result<(), String> {
    let mut all = scripts.manager.all_items();
    all.sort_by_key(|&i| {
        scripts
            .item(i)
            .full_name()
            .encode_utf16()
            .collect::<Vec<u16>>()
    });
    let mut world = World::new(scripts, sandbox, multiplier, 0);
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
