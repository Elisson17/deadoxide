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
fn actioncontext_matches_original() {
    // ActionContextProbe's scenarios; keep in step with the `actioncontext` line of tools/oracle.sh
    check(
        "actioncontext",
        &[
            "ctx:zombie:11:4000",
            "ctx:zombie:12:4000",
            "ctx:zombie-crawler:13:3000",
            "ctx:player:14:6000",
            "ctx:player:15:6000",
            "ctx:player-vehicle:16:3000",
            "ctx:rabbit:17:2000",
            "clips",
            "zanim:12345:1200",
            "zanim:7:600",
            "anim:zombie:51:1500",
            "anim:zombie:52:1500:idle,lunge,staggerback,staggerback-knockeddown,getup,getup-fromonback,getup-fromonfront,getup-fromsitting,walktoward,attack,hitreaction",
            "anim:zombie-crawler:61:1200",
            "anim:player:41:1500:idle,aim,aim-strafe,run,sprint,strafe,movement,melee,ext",
            "anim:player:43:1500",
            "anim:player:45:1500:idle,movement,run,sprint,strafe,melee,actions,aim,aim-strafe,ext,sneak",
            "anim:player-vehicle:62:1200",
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

/// The player's five `gos_*.bin` plus the files in tests/gos_crafted (the original writer's own output and hand-made
/// broken ones), read into global object systems with the shared lookup.
#[test]
fn global_objects_match_original() {
    let Some(save) = profile_save() else {
        eprintln!("SKIPPED gos: the isolated profile's save is not present");
        return;
    };
    let crafted = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/gos_crafted");
    check("gos", &[&save, crafted.to_str().expect("utf-8 path")]);
}

/// `map_worldgen.bin`, `map_symbols.bin`, `statistics.bin` and `metadata.bin`: the player's real files plus the files in
/// tests/savemeta_crafted (the original writers' own output and hand-made broken ones).
#[test]
fn save_meta_matches_original() {
    let Some(save) = profile_save() else {
        eprintln!("SKIPPED savemeta: the isolated profile's save is not present");
        return;
    };
    let crafted = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/savemeta_crafted");
    check("savemeta", &[&save, crafted.to_str().expect("utf-8 path")]);
}

/// The original `WorldMapVisited` driven by tests/visited_cases.txt: the player's real `map_visited.bin`, the setters,
/// `save`, reading back, and hand-made files of the current and the legacy formats.
#[test]
fn world_map_visited_matches_original() {
    let Some(save) = profile_save() else {
        eprintln!("SKIPPED visited: the isolated profile's save is not present");
        return;
    };
    let cases = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/visited_cases.txt");
    check("visited", &[cases.to_str().expect("utf-8 path"), &save]);
}

/// `IsoRegionWorker.loadSaveMap` over the player's real `isoregiondata` and the scenario folders of
/// tests/isoregion_crafted (the original writer's files and hand-made ones).
#[test]
fn iso_region_matches_original() {
    let Some(save) = profile_save() else {
        eprintln!("SKIPPED isoregion: the isolated profile's save is not present");
        return;
    };
    let crafted = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/isoregion_crafted");
    check("isoregion", &[&save, crafted.to_str().expect("utf-8 path")]);
}

/// `ErosionConfig.readFile`/`writeFile` over the player's real `erosion.ini` and the hand-made files of
/// tests/erosionini_crafted.
#[test]
fn erosion_ini_matches_original() {
    let Some(save) = profile_save() else {
        eprintln!("SKIPPED erosionini: the isolated profile's save is not present");
        return;
    };
    let crafted = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/erosionini_crafted");
    check(
        "erosionini",
        &[&save, crafted.to_str().expect("utf-8 path")],
    );
}

/// `IsoWorld.readWorldVersion` and the `map_ver.bin` read over the player's real files and the hand-made pairs of
/// tests/mapver_crafted.
#[test]
fn map_ver_matches_original() {
    let Some(save) = profile_save() else {
        eprintln!("SKIPPED mapver: the isolated profile's save is not present");
        return;
    };
    let crafted = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/mapver_crafted");
    check("mapver", &[&save, crafted.to_str().expect("utf-8 path")]);
}

/// The radio of a world (`ZomboidRadio.Init`/`Load`/`Save`/`update`, the script manager, channels, scripts, broadcasts,
/// `RadioData.xml`, `RadioTranslationData`, `computerize`/`scrambleString`) over the commands of tests/radio_cases.txt,
/// the crafted files of tests/radio_crafted and the player's own RADIO_SAVE.txt.
#[test]
fn radio_matches_original() {
    let Some(save) = profile_save() else {
        eprintln!("SKIPPED radio: the isolated profile's save is not present");
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cases = root.join("tests/radio_cases.txt");
    let crafted = root.join("tests/radio_crafted");
    check(
        "radio",
        &[
            cases.to_str().expect("utf-8 path"),
            crafted.to_str().expect("utf-8 path"),
            &save,
        ],
    );
}

/// assimp's X importer (E59) over every `.x` of `media/models_X` and `media/anims_X`, no post-processing step.
#[test]
fn xscene_matches_original() {
    check("xscene", &["none", "x", "media/models_X", "media/anims_X"]);
}

/// The same files with the animation flags of `FileTask_LoadAnimation` (E59).
#[test]
fn animscene_matches_original() {
    check("animscene", &["anim", "x", "media/anims_X"]);
}

/// The same files with the mesh flags of `FileTask_LoadMesh` (E59).
#[test]
fn xmodelscene_matches_original() {
    check("xmodelscene", &["mesh", "x", "media/models_X"]);
}

/// `ProcessedAiScene.process` (E59) on the scenes of every `.x` model: skeleton, skinned and static meshes in the game's
/// modes, by mesh name, all meshes, and skinned to another skeleton.
#[test]
fn xmodels_matches_original() {
    check(
        "xmodels",
        &[
            "Normal,StaticMesh,Normal:last,StaticMesh:all,Normal:skin=media/models_X/Skinned/Cow_Skeleton.x",
            "mesh",
            "x",
            "media/models_X",
        ],
    );
}

/// `ProcessedAiScene.process` (E59) on every `.x` animation: the clips with their keyframes, with the `.fbx`
/// modifiers, and skinned to another skeleton.
#[test]
fn animclips_matches_original() {
    check(
        "animclips",
        &[
            "AnimationOnly,AnimationOnly:fbx,AnimationOnly:skin=media/models_X/Skinned/Cow_Skeleton.x",
            "anim",
            "x",
            "media/anims_X",
        ],
    );
}

/// `ModelLoader.loadTxt` (E59) on every `media/models/*.txt` and `media/animsold/*.txt` in several
/// (static, reverse, skinnedTo) combinations: vertices, elements, bones, matrices, clips and keyframes.
#[test]
fn txtmodels_matches_original() {
    check("txtmodels", &[]);
}

/// BodyDamage, Thermoregulator, Nutrition, Stats and Moodles of a headless player over the scenarios of
/// tests/body_cases.txt.
#[test]
fn body_matches_original() {
    // BodyProbe's scenarios: tests/body_cases.txt, shared with the `body` line of tools/oracle.sh
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cases = root.join("tests/body_cases.txt");
    check("body", &[cases.to_str().expect("utf-8 path")]);
}

/// The FMOD bank list of `FMODManager.init()` (per-bank event and VCA counts) and the RIFF container of the bank files.
#[test]
fn fmodbanks_matches_original() {
    check("fmodbanks", &["-", "banks"]);
}

/// The FMOD string table: the events (path and GUID) and VCAs of every bank, `FMODManager`'s sorted path list, the
/// parameter names and GetBus/GetVCA over the candidate paths the port writes from its own table.
#[test]
fn fmodstrings_matches_original() {
    let Some(root) = game_root() else {
        eprintln!("SKIPPED fmodstrings: no game install next to the workspace");
        return;
    };
    let cand = Path::new(env!("CARGO_TARGET_TMPDIR")).join("fmod_candidates.txt");
    let out = Command::new(env!("CARGO_BIN_EXE_pz-dump"))
        .arg("fmodcandidates")
        .arg(&root)
        .output()
        .expect("run pz-dump");
    assert!(out.status.success(), "pz-dump fmodcandidates failed");
    std::fs::write(&cand, &out.stdout).expect("write candidates");
    check(
        "fmodstrings",
        &["-", "strings", cand.to_str().expect("utf-8 path")],
    );
    let _ = std::fs::remove_file(&cand);
}

/// The FMOD event and parameter records: sustain flag, length and parameters of every event, the global parameter list
/// and `FMODManager`'s parameter table.
#[test]
fn fmodevents_matches_original() {
    check("fmodevents", &["-", "events"]);
}

/// The FSB5 sample banks: header numbers, stream count, frequency, channels and lengths of every stream, names.
#[test]
fn fmodsamples_matches_original() {
    check("fmodsamples", &["-", "samples"]);
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

#[test]
fn luagame_matches_original() {
    // the client's startup steps, the client load, then luaboot's events and the UI the scripts built; the port
    // writes the files the scripts make (Lua/, options.ini, Saves/) into a fresh scratch cache dir, as the probe's
    let scratch = std::env::temp_dir().join("pz-oracle-hashes-luagame");
    let _ = std::fs::remove_dir_all(&scratch);
    check(
        "luagame",
        &[scratch.to_str().expect("utf-8 path"), "shared", "client"],
    );
    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn loot_matches_original() {
    // the client's startup, shared + server Lua, the distribution merge events, then ItemPickerJava.Parse and its
    // tables (and the loot scenarios rolled over them); a fresh scratch cache dir, as luagame's
    let scratch = std::env::temp_dir().join("pz-oracle-hashes-loot");
    let _ = std::fs::remove_dir_all(&scratch);
    let scenarios = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/loot_scenarios.txt"),
    )
    .expect("tests/loot_scenarios.txt");
    let mut args = vec![scratch.to_str().expect("utf-8 path")];
    args.extend(scenarios.split_whitespace());
    check("loot", &args);
    let _ = std::fs::remove_dir_all(&scratch);
}
