//! Checks the port against the ORIGINAL game's output without needing Java: `tools/oracle.sh` stores the sha256 of
//! each Java probe's output once it is byte-identical to `pz-dump`; here the Rust side is recomputed over the
//! player's install and must still hash to the same value. Needs the game in the workspace's parent folder.
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

fn game_root() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    root.join("media/newtiledefinitions.tiles")
        .exists()
        .then_some(root)
}

fn check(case: &str, args: &[&str]) {
    check_as(case, case, args)
}

fn check_as(golden: &str, case: &str, args: &[&str]) {
    let Some(root) = game_root() else {
        eprintln!("SKIPPED {case}: no game install next to the workspace");
        return;
    };
    let want = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/golden/engine/{golden}.sha256")),
    )
    .expect("golden hash");
    let out = Command::new(env!("CARGO_BIN_EXE_pz-dump"))
        .arg(case)
        .arg(&root)
        .args(args)
        .output()
        .expect("run pz-dump");
    assert!(
        out.status.success(),
        "pz-dump {case} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let got: String = Sha256::digest(&out.stdout)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(got, want.trim(), "{case}: Rust output no longer matches the original's (run sh tools/oracle.sh {case} for a diff)");
}

#[test]
fn tiledefs_match_original() {
    check("tiledefs", &[]);
}

#[test]
fn fonts_matches_original() {
    // the second argument is the probe's scratch cache dir, unused by pz-dump
    check("fonts", &["-"]);
}

#[test]
fn lotheaders_match_original() {
    check("lotheaders", &["Muldraugh, KY"]);
}

#[test]
fn lotpack_matches_original() {
    check("lotpack", &["Muldraugh, KY", "30", "43", "34", "47"]);
}

#[test]
fn metagrid_matches_original() {
    check("metagrid", &["Muldraugh, KY", "30", "43", "34", "47"]);
}

#[test]
fn chunkobjects_match_original() {
    check("chunkobjects", &["1036", "1460", "1039", "1463"]);
}

#[test]
fn collide_matches_original() {
    check("collide", &["1036", "1460", "1039", "1463"]);
}

#[test]
fn entitysprites_match_original() {
    check("entitysprites", &[]);
}

#[test]
fn specialcollide_matches_original() {
    check("specialcollide", &["129", "182"]);
}

#[test]
fn basements_match_original() {
    check("basements", &["30", "43", "34", "47"]);
}

#[test]
fn doortoggle_matches_original() {
    check_as("doortoggle", "specialcollide", &["129", "182", "toggle"]);
}

#[test]
fn los_matches_original() {
    check("los", &["129", "182"]);
}

#[test]
fn los_with_doors_flipped_matches_original() {
    check_as("losdoors", "los", &["129", "182", "toggle"]);
}

#[test]
fn stairs_match_original() {
    check("stairs", &["126", "181"]);
}

#[test]
fn sandbox_matches_original() {
    let save = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../profile/Saves/Sandbox/2026-11-07_22-34-13/map_sand.bin"
    );
    if !std::path::Path::new(save).exists() {
        eprintln!("SKIPPED sandbox: the isolated profile's save is not present");
        return;
    }
    check("sandbox", &[save]);
}

/// The isolated profile's save the save-loading oracles read (not committed; skipped when absent).
fn profile_save() -> Option<String> {
    let save = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../profile/Saves/Sandbox/2026-11-07_22-34-13"
    );
    std::path::Path::new(save)
        .join("map")
        .exists()
        .then(|| save.to_string())
}

#[test]
fn world_dictionary_matches_original() {
    let Some(save) = profile_save() else {
        eprintln!("SKIPPED worlddictionary: the isolated profile's save is not present");
        return;
    };
    check("worlddictionary", &[&format!("{save}/WorldDictionary.bin")]);
}

#[test]
fn chunk_save_matches_original() {
    let Some(save) = profile_save() else {
        eprintln!("SKIPPED chunksave: the isolated profile's save is not present");
        return;
    };
    check("chunksave", &[&save]);
}

#[test]
fn map_meta_matches_original() {
    let Some(save) = profile_save() else {
        eprintln!("SKIPPED mapmeta: the isolated profile's save is not present");
        return;
    };
    check("mapmeta", &[&save]);
}

#[test]
fn map_zone_matches_original() {
    let Some(save) = profile_save() else {
        eprintln!("SKIPPED mapzone: the isolated profile's save is not present");
        return;
    };
    check("mapzone", &[&save]);
}

#[test]
fn zone_outlines_match_original() {
    let Some(save) = profile_save() else {
        eprintln!("SKIPPED zoneoutlines: the isolated profile's save is not present");
        return;
    };
    check("zoneoutlines", &[&save, "outlines"]);
}

#[test]
fn zone_triangles_match_original() {
    let Some(save) = profile_save() else {
        eprintln!("SKIPPED zonetriangles: the isolated profile's save is not present");
        return;
    };
    check("zonetriangles", &[&save, "triangles"]);
}

#[test]
fn animal_zones_match_original() {
    let Some(save) = profile_save() else {
        eprintln!("SKIPPED animalzones: the isolated profile's save is not present");
        return;
    };
    check("animalzones", &[&save]);
}

#[test]
fn game_time_matches_original() {
    let Some(save) = profile_save() else {
        eprintln!("SKIPPED gametime: the isolated profile's save is not present");
        return;
    };
    check("gametime", &[&save]);
}

#[test]
fn climate_matches_original() {
    // ClimateProbe's scenarios (the probe's first argument, its scratch dir, is not passed to pz-dump); keep in step
    // with the `climate` line of tools/oracle.sh
    check(
        "climate",
        &[
            "1984:1993:6:8:9.0:6",
            "12:1995:2:10:6.0:4",
            "8:1995:0:10:6.0:4",
            "8:1995:5:5:6.0:5",
            "5:1995:1:3:6.0:2:climate=6,fog=1",
            "5:1995:4:3:6.0:2:climate=3,fog=3,rain=5",
            "5:1995:4:3:6.0:2:climate=2,fog=2,rain=1",
            "5:1995:5:3:6.0:2:climate=5",
            "5:1995:10:3:6.0:5:mode=Winter is Coming",
            "5:1995:5:3:6.0:3:mode=The Descending Fog",
            "5:1995:7:3:6.0:2:mode=A Storm is Coming",
            "5:1995:5:3:6.0:1:daynight=2",
            "5:1995:5:3:6.0:1:daynight=3",
        ],
    );
}

#[test]
fn erosion_matches_original() {
    // ErosionProbe's scenarios; keep in step with the `erosion` line of tools/oracle.sh
    check(
        "erosion",
        &[
            "1033,1457,1038,1462:1993:6:8:0:0:speed=1",
            "1033,1457,1038,1462:1993:9:20:200:40/80/120/160/200:speed=1",
            "1017,1418,1020,1421:1993:3:10:100:20/50/100:speed=1",
            "1327,1285,1332,1290:1993:3:10:100:20/50/100:speed=1",
            "1034,1458,1036,1460:1993:6:8:400:100/200/300/400",
            "1034,1458,1036,1460:1993:6:8:40:0/40:speed=2",
            "1034,1458,1036,1460:1993:6:8:40:0/40:speed=4",
            "1034,1458,1036,1460:1993:6:8:40:0/40:speed=5",
            "1034,1458,1036,1460:1993:6:8:40:0/40:edays=30",
            "1034,1458,1036,1460:1993:6:8:40:0/40:edays=-1",
            "1034,1458,1036,1460:1993:6:8:40:0/40:apo=7",
            "1034,1458,1036,1460:1993:6:8:20:0/20:apo=13,speed=5",
            "1034,1458,1036,1460:1993:6:8:40:0/40:temp=1",
            "1034,1458,1036,1460:1993:6:8:40:0/40:temp=2",
            "1034,1458,1036,1460:1993:6:8:40:0/40:temp=4",
            "1034,1458,1036,1460:1993:6:8:40:0/40:temp=5",
            "1034,1458,1036,1460:1993:6:8:3:0/3:speed=1",
            "1034,1458,1036,1460:1993:6:11:4:0/4:speed=5,keep=1",
            "1034,1458,1036,1460:1994:0:1:2:2:speed=3,apo=5,temp=1,keep=1",
        ],
    );
}

#[test]
fn zombie_matches_original() {
    // ZombieProbe's scenarios; keep in step with the `zombie` line of tools/oracle.sh
    check(
        "zombie",
        &[
            "12345:40:0",
            "1:40:0:Speed=4,SprinterPercentage=50",
            "2:30:0:Speed=1",
            "3:30:0:Speed=3",
            "4:30:0:Speed=4,SprinterPercentage=0,Strength=4,Cognition=4,DoorOpeningPercentage=50",
            "5:30:0:Memory=5,Sight=4,Hearing=4,CrawlUnderVehicle=2",
            "6:30:0:Memory=6,Sight=5,Hearing=5,CrawlUnderVehicle=7,Strength=1,Cognition=1",
            "7:30:0:Strength=3,Memory=1,CrawlUnderVehicle=1,Speed=4,SprinterPercentage=100",
            "8:30:0:Memory=3,CrawlUnderVehicle=3,Sight=1,Hearing=3",
            "9:30:0:Memory=4,CrawlUnderVehicle=4,Sight=3,Hearing=1",
            "10:30:0:CrawlUnderVehicle=6,Speed=2,Cognition=2",
            "12345:1:9000",
            "1:1:12000",
            "2:1:9000:Speed=1",
            "3:1:9000:Speed=3",
            "4:1:6000:Speed=4",
            "5:1:15000",
            "6:1:9000",
            "7:1:9000",
            "8:1:9000",
        ],
    );
}

#[test]
fn mod_data_matches_original() {
    let Some(save) = profile_save() else {
        eprintln!("SKIPPED moddata: the isolated profile's save is not present");
        return;
    };
    check("moddata", &[&save]);
}

#[test]
fn outfits_match_original() {
    let Some(save) = profile_save() else {
        eprintln!("SKIPPED outfits: the isolated profile's save is not present");
        return;
    };
    check("outfits", &[&save]);
}

#[test]
fn clothing_items_match_original() {
    check("clothingitems", &[]);
}

/// LocationRNG, Color.HSBtoRGB, OutfitRNG colours, decal groups and ClothingItemReference.randomize of every outfit.
#[test]
fn outfit_random_matches_original() {
    check("outfitrandom", &[]);
}

#[test]
fn animsets_matches_original() {
    // the second argument is the probe's scratch cache dir, unused by pz-dump
    check("animsets", &["-"]);
}

#[test]
fn items_match_original() {
    // The field list tools/gen_item_params.py hands to both sides: the table's fields minus aimingMod.
    let names: Vec<&str> = zomboid_engine::zombie::scripting::item_params_table::FIELDS
        .iter()
        .map(|f| f.name)
        .filter(|n| *n != "aimingMod")
        .collect();
    let list = Path::new(env!("CARGO_TARGET_TMPDIR")).join("item_fields.txt");
    std::fs::write(&list, names.join("\n") + "\n").expect("write field list");
    check("items", &[list.to_str().expect("utf-8 path")]);
}

/// Every craftRecipe script after CraftRecipe.Load, OnScriptsLoaded and the ported part of OnPostWorldDictionaryInit.
#[test]
fn recipes_matches_original() {
    check("recipes", &[]);
}

/// zombie.scripting.objects.VehicleScript (Load with its templates, Loaded) and VehicleTemplate: every vehicle script
/// and vehicle template of the server's ScriptManager.
#[test]
fn vehicles_matches_original() {
    check("vehicles", &[]);
}

/// The XUI scripts (xuiConfig, xuiSkin, ...) and XuiManager.ParseScripts' skins, Lua styles, var registry and
/// colours, as the server's ScriptManager holds them.
#[test]
fn xui_matches_original() {
    check("xui", &[]);
}

/// Mod loading over the crafted mods of tests/mods_cases.txt (written by pz-dump under its own cache dir in the
/// scratch folder): mod.info fields, availability, the version folders, ActiveMods, the load order and the file map.
#[test]
fn mods_matches_original() {
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("pz-oracle-mods");
    let cases = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/mods_cases.txt");
    check(
        "mods",
        &[
            scratch.to_str().expect("utf-8 path"),
            cases.to_str().expect("utf-8 path"),
        ],
    );
    // pz-dump's cache dir there holds only the crafted mods
    let _ = std::fs::remove_dir_all(&scratch);
}

/// Every sound script and the GameSounds registry as the booted server holds them, then the crafted Load bodies and
/// GameSounds calls of tests/sounds_cases.txt (the `sounds.ini` they write goes to a scratch folder).
#[test]
fn sounds_matches_original() {
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("pz-oracle-sounds");
    let cases = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/sounds_cases.txt");
    check(
        "sounds",
        &[
            cases.to_str().expect("utf-8 path"),
            scratch.to_str().expect("utf-8 path"),
        ],
    );
    let _ = std::fs::remove_dir_all(&scratch);
}

/// zombie.scripting.ScriptParser over the crafted cases of tests/script_parser_cases.txt.
#[test]
fn scriptparser_matches_original() {
    let cases = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/script_parser_cases.txt");
    check("scriptparser", &[cases.to_str().expect("utf-8 path")]);
}

#[test]
fn luacompile_matches_original() {
    let cases = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/lua_compile_cases.txt");
    check("luacompile", &[cases.to_str().expect("utf-8 path")]);
}

#[test]
fn luavm_matches_original() {
    let cases = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/lua_vm_cases.txt");
    check("luavm", &[cases.to_str().expect("utf-8 path")]);
}

#[test]
fn luastring_matches_original() {
    let cases = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/lua_string_cases.txt");
    check("luastring", &[cases.to_str().expect("utf-8 path")]);
}

#[test]
fn javachars_match_original() {
    check("javachars", &[]);
}

#[test]
fn fdlibm_matches_original() {
    check("fdlibm", &[]);
}

#[test]
fn luaos_matches_original() {
    let cases = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/lua_os_cases.txt");
    check("luaos", &[cases.to_str().expect("utf-8 path")]);
}

#[test]
fn luaload_matches_original() {
    // the second argument is the probe's scratch cache dir, unused by pz-dump
    check("luaload", &["-", "shared"]);
}

#[test]
fn luaserver_matches_original() {
    // media/lua/shared then media/lua/server, as the game's server loads them
    check("luaserver", &["-", "shared", "server"]);
}

#[test]
fn luaclient_matches_original() {
    // media/lua/shared then media/lua/client, as the game's client loads them
    check("luaclient", &["-", "shared", "client"]);
}

#[test]
fn translator_matches_original() {
    // the second argument is the probe's scratch cache dir, unused by pz-dump
    let commands = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/translator_cases.txt");
    check("translator", &["-", commands.to_str().expect("utf-8 path")]);
}

#[test]
fn luaboot_matches_original() {
    // the client load, then ZomboidGlobals.Load and the events the game fires on its way to the main menu
    check("luaboot", &["-", "shared", "client"]);
}
