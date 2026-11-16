//! Dumps engine state in the exact text format of the Java probes in `tests/original_probe`, for byte-for-byte diffs.
//!   pz-dump tiledefs <game root>                  (pairs with TileDefsProbe.java)
//!   pz-dump fonts <game root> <scratch>           (pairs with FontsProbe.java)
//!   pz-dump lotheaders <game root> <map folder>   (pairs with LotHeadersProbe.java)
//!   pz-dump lotpack <game root> <map folder> x1 y1 x2 y2   (pairs with LotPackProbe.java)
//!   pz-dump mapmeta <game root> <save dir>    (pairs with MapMetaProbe.java)
//!   pz-dump mapzone <game root> <save dir>    (pairs with MapZoneProbe.java)
//!   pz-dump animalzones <game root> <save dir>   (pairs with AnimalZonesProbe.java)
//!   pz-dump zoneoutlines <game root> <save dir> outlines   (MapZoneProbe.java ... outlines)
//!   pz-dump zonetriangles <game root> <save dir> triangles (MapZoneProbe.java ... triangles)
//!   pz-dump translator <game root> <scratch, unused> <command file>   (TranslatorProbe.java)
//!   pz-dump recipes <game root>               (pairs with RecipesProbe.java)
//!   pz-dump animsets <game root> <scratch dir>   (pairs with AnimSetsProbe.java)
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;

use zomboid_engine::java;
use zomboid_engine::zombie::core::random::rand_abstract::RandAbstract;
use zomboid_engine::zombie::core::tile_property_alias_map::TilePropertyAliasMap;
use zomboid_engine::zombie::iso::cell_loader::{self, LoadContext};
use zomboid_engine::zombie::iso::iso_chunk::IsoChunk;
use zomboid_engine::zombie::iso::iso_grid_square::{line_clear, GetSquare, IsoGridSquare};
use zomboid_engine::zombie::iso::iso_lot::IsoLot;
use zomboid_engine::zombie::iso::iso_meta_grid::{AlarmSettings, IsoMetaGrid};
use zomboid_engine::zombie::iso::iso_object::ObjectKind;
use zomboid_engine::zombie::iso::iso_world::{
    self, IsoWorld, TileDefinitions, VANILLA_TILE_DEFINITION_FILES,
};
use zomboid_engine::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;
use zomboid_engine::zombie::iso::sprite_details::iso_flag_type::IsoFlagType;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let r = match a.get(1).map(String::as_str) {
        Some("tiledefs") if a.len() == 3 => tiledefs(Path::new(&a[2])),
        Some("fonts") if a.len() == 4 => fonts(Path::new(&a[2])),
        Some("translator") if a.len() == 5 => translator(Path::new(&a[2]), Path::new(&a[4])),
        Some("lotheaders") if a.len() == 4 => lotheaders(Path::new(&a[2]), &a[3]),
        Some("lotpack") if a.len() == 8 => lotpack(Path::new(&a[2]), &a[3], &a[4..8]),
        Some("metagrid") if a.len() == 8 => metagrid(Path::new(&a[2]), &a[3], &a[4..8]),
        Some("chunkobjects") if a.len() == 7 => chunkobjects(Path::new(&a[2]), &a[3..7]),
        Some("collide") if a.len() == 7 => collide(Path::new(&a[2]), &a[3..7]),
        Some("entitysprites") if a.len() == 3 => entitysprites(Path::new(&a[2])),
        Some("items") if a.len() == 4 => items(Path::new(&a[2]), Path::new(&a[3])),
        Some("recipes") if a.len() == 3 => recipes(Path::new(&a[2])),
        Some("vehicles") if a.len() == 3 => vehicles(Path::new(&a[2])),
        Some("xui") if a.len() == 3 => xui(Path::new(&a[2])),
        Some("scriptparser") if a.len() == 4 => scriptparser(Path::new(&a[3])),
        Some("sandbox") if a.len() == 4 => sandbox(Path::new(&a[2]), Path::new(&a[3])),
        Some("specialcollide") if a.len() == 5 || a.len() == 6 => {
            specialcollide(Path::new(&a[2]), &a[3..], false)
        }
        Some("stairs") if a.len() == 5 => specialcollide_mode(Path::new(&a[2]), &a[3..], 2),
        Some("los") if a.len() == 5 || a.len() == 6 => {
            specialcollide(Path::new(&a[2]), &a[3..], true)
        }
        Some("basements") if a.len() == 7 => basements(Path::new(&a[2]), &a[3..7]),
        Some("worlddictionary") if a.len() == 4 => worlddictionary(Path::new(&a[3])),
        Some("chunksave") if a.len() == 4 => chunksave(Path::new(&a[2]), Path::new(&a[3])),
        Some("mapmeta") if a.len() == 4 => mapmeta(Path::new(&a[2]), Path::new(&a[3])),
        Some("animalzones") if a.len() == 4 => animalzones(Path::new(&a[2]), Path::new(&a[3])),
        Some("gametime") if a.len() == 4 => gametime(Path::new(&a[3])),
        Some("climate") if a.len() >= 4 => climate(Path::new(&a[2]), &a[3..]),
        Some("moddata") if a.len() == 4 => moddata(Path::new(&a[3])),
        Some("outfits") if a.len() == 4 => outfits(Path::new(&a[2]), Path::new(&a[3])),
        Some("clothingitems") if a.len() == 3 => clothingitems(Path::new(&a[2])),
        Some("outfitrandom") if a.len() == 3 => outfitrandom(Path::new(&a[2])),
        Some("animsets") if a.len() == 4 => animsets(Path::new(&a[2])),
        Some("luacompile") if a.len() == 4 => luacompile(Path::new(&a[2]), Path::new(&a[3])),
        Some("luavm") if a.len() == 4 => luavm(Path::new(&a[2]), Path::new(&a[3])),
        Some("luastring") if a.len() == 4 => luavm(Path::new(&a[2]), Path::new(&a[3])),
        Some("luaos") if a.len() == 4 => luavm(Path::new(&a[2]), Path::new(&a[3])),
        Some("javachars") if a.len() == 3 => javachars(),
        Some("fdlibm") if a.len() == 3 => fdlibm(),
        Some("luaload" | "luaserver" | "luaclient") if a.len() >= 4 => {
            luaload(Path::new(&a[2]), &a[3], &a[4..], false)
        }
        Some("luaboot") if a.len() >= 4 => luaload(Path::new(&a[2]), &a[3], &a[4..], true),
        Some("mapzone") if a.len() == 4 => {
            mapzone(Path::new(&a[2]), Path::new(&a[3]), false, false)
        }
        Some("zonetriangles") if a.len() == 5 && a[4] == "triangles" => {
            mapzone(Path::new(&a[2]), Path::new(&a[3]), true, true)
        }
        Some("zoneoutlines") if a.len() == 5 && a[4] == "outlines" => {
            mapzone(Path::new(&a[2]), Path::new(&a[3]), true, false)
        }
        _ => Err("usage: see the header of pz-dump.rs".into()),
    };
    match r {
        Ok(text) => {
            if let Err(e) = std::io::stdout().lock().write_all(text.as_bytes()) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

/// The meta grid as a loaded save has it (pairs with MapMetaProbe.java): `IsoMetaGrid.Create`, the save's basements
/// from `map_basements.bin`, then `IsoMetaGrid.load` of its `map_meta.bin` as a single-player game reads it.
pub fn mapmeta(root: &Path, save: &Path) -> Result<String, String> {
    use zomboid_engine::java_nio::ByteBuffer;
    use zomboid_engine::zombie::basements::Basements;
    use zomboid_engine::zombie::iso::map_meta::MetaSide;
    let media = root.join("media");
    let mut g = IsoMetaGrid::default();
    let dirs = IsoMetaGrid::lot_directories(&media, "Muldraugh, KY");
    g.create_step1(&media, &dirs, (-250, -250, 250, 250))?;
    for e in g.load_all_cells(&mut |_| 1, &mut |_, _| Vec::new()) {
        eprintln!("{e}");
    }
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    g.create_step2(&alarm, &mut |_| 1, &mut |a, _| a);
    let read = |n: &str| {
        let p = save.join(n);
        std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))
    };
    let b = Basements::before_load_meta_grid_from_save(
        &media,
        &mut g,
        &dirs,
        &read("map_basements.bin")?,
    )?;
    for l in &b.log {
        eprintln!("{l}");
    }
    let mut sb = String::new();
    let mut h: u64 = 0xcbf29ce484222325;
    for p in &b.basement_placements {
        for v in [p.x, p.y, p.z, p.w, p.h] {
            mix(&mut h, v);
        }
        for u in p.name.encode_utf16() {
            mix(&mut h, i32::from(u));
        }
    }
    let _ = writeln!(sb, "BP {} {h:x}", b.basement_placements.len());
    let mut h: u64 = 0xcbf29ce484222325;
    for bd in &g.buildings {
        mix_long(&mut h, g.building(*bd).id);
        mix_long(&mut h, g.building(*bd).meta_id);
    }
    let _ = writeln!(sb, "B {} {h:x}", g.buildings.len());
    let data = read("map_meta.bin")?;
    let mut bb = ByteBuffer::wrap(&data);
    let int_at = |i: usize| {
        data.get(i..i + 4)
            .map_or(0, |s| i32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    };
    let _ = writeln!(
        sb,
        "MM {} {} {} {} {}",
        int_at(4),
        int_at(8),
        int_at(12),
        int_at(16),
        int_at(20)
    );
    // As MapMetaProbe: the state Create drew from Rand goes back to the field defaults before the load.
    for r in &mut g.room_defs {
        (r.explored, r.lights_active, r.done_spawn, r.roof_fixed) = (false, false, false, false);
    }
    for d in &mut g.building_defs {
        (d.alarmed, d.key_id, d.seen, d.has_been_visited) = (false, -1, false, false);
        (d.loot_respawn_hour, d.alarm_decay) = (-1, 10000000);
    }
    let m = g.load(&mut bb, MetaSide::SinglePlayer)?;
    for l in &m.log {
        eprintln!("{l}");
    }
    let _ = writeln!(sb, "REST {}", bb.remaining());
    let mut t = [0i32; 9];
    for x in g.min_x..=g.max_x {
        for y in g.min_y..=g.max_y {
            let Some(cell) = g.get_cell_data(x, y) else {
                continue;
            };
            let mut detail = String::new();
            let mut h: u64 = 0xcbf29ce484222325;
            for rr in &cell.room_list {
                let r = g.room(*rr);
                let f = i32::from(r.explored)
                    | i32::from(r.lights_active) << 1
                    | i32::from(r.done_spawn) << 2
                    | i32::from(r.roof_fixed) << 3;
                mix_long(&mut h, r.meta_id);
                mix(&mut h, f);
                t[0] += 1;
                for i in 0..4 {
                    if f & 1 << i != 0 {
                        t[2 + i] += 1;
                    }
                }
                if f & !2 != 0 {
                    let _ = writeln!(detail, "MR {} {f}", r.meta_id);
                }
            }
            for bd in &cell.buildings {
                let d = g.building(*bd);
                mix_long(&mut h, d.meta_id);
                for v in [
                    i32::from(d.alarmed),
                    d.key_id,
                    i32::from(d.seen),
                    i32::from(d.has_been_visited),
                    d.loot_respawn_hour,
                    d.alarm_decay,
                ] {
                    mix(&mut h, v);
                }
                t[1] += 1;
                t[6] += i32::from(d.alarmed);
                t[7] += i32::from(d.seen);
                t[8] += i32::from(d.has_been_visited);
                if d.alarmed || d.seen || d.has_been_visited {
                    let _ = writeln!(
                        detail,
                        "MD {} {} {} {} {} {} {}",
                        d.meta_id,
                        u8::from(d.alarmed),
                        d.key_id,
                        u8::from(d.seen),
                        u8::from(d.has_been_visited),
                        d.loot_respawn_hour,
                        d.alarm_decay
                    );
                }
            }
            let _ = writeln!(
                sb,
                "MC {x} {y} {} {} {h:x}",
                cell.room_list.len(),
                cell.buildings.len()
            );
            sb.push_str(&detail);
        }
    }
    let _ = writeln!(
        sb,
        "MT {}",
        t.iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    );
    let _ = writeln!(sb, "SH {}", m.safehouses.len());
    for s in &m.safehouses {
        let _ = writeln!(
            sb,
            "S {} {} {} {} {} {} [{}] {} {} {} {} [{}]",
            s.x,
            s.y,
            s.w,
            s.h,
            s.owner,
            s.hit_points,
            s.players.join(", "),
            s.last_visited,
            s.title,
            s.datetime_created,
            s.location,
            s.players_respawn.join(", ")
        );
    }
    let _ = writeln!(sb, "NP {}", m.non_pvp_zones.len());
    for z in &m.non_pvp_zones {
        let _ = writeln!(
            sb,
            "N {} {} {} {} {} {}",
            z.x, z.y, z.x2, z.y2, z.size, z.title
        );
    }
    let _ = writeln!(sb, "FA {}", m.factions.len());
    for f in &m.factions {
        let _ = writeln!(
            sb,
            "F {} {} {} {} {} {} [{}]",
            f.name,
            f.owner,
            f.tag.as_deref().unwrap_or("null"),
            f.tag_color[0].to_bits() as i32,
            f.tag_color[1].to_bits() as i32,
            f.tag_color[2].to_bits() as i32,
            f.players.join(", ")
        );
    }
    let _ = writeln!(sb, "DZ {}", m.designation_zones.len());
    let st = m.stash_system.unwrap_or_default();
    let _ = writeln!(
        sb,
        "ST {} {} {}",
        st.possible_stashes.len(),
        st.buildings_to_do.len(),
        st.already_read_map.len()
    );
    for s in &st.possible_stashes {
        let _ = writeln!(sb, "SP {} {} {}", s.stash_name, s.building_x, s.building_y);
    }
    for s in &st.buildings_to_do {
        let _ = writeln!(sb, "SB {} {} {}", s.stash_name, s.building_x, s.building_y);
    }
    for s in &st.already_read_map {
        let _ = writeln!(sb, "SR {s}");
    }
    let _ = writeln!(sb, "RDS {}", m.unique_rds_spawned.len());
    for s in &m.unique_rds_spawned {
        let _ = writeln!(sb, "R {s}");
    }
    Ok(sb)
}

/// The loading sequence of IsoWorld.init without mods and without entity scripts, then one line per sprite.
pub fn tiledefs(root: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    for (f, n) in VANILLA_TILE_DEFINITION_FILES {
        let p = root.join("media").join(f);
        files.push((
            f,
            n,
            std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?,
        ));
    }
    let mut defs = TileDefinitions::default();
    let mut alias = TilePropertyAliasMap::default();
    let mut sm = IsoSpriteManager::new();
    for (f, n, data) in &files {
        if let Err(e) = defs.load_tile_definitions_property_strings(data, f, *n) {
            eprintln!("PROBE strings {f}: {e}");
        }
    }
    defs.set_custom_property_values()?;
    defs.generate_tile_property_lookup_tables(&mut alias, &[]);
    for (f, n, data) in &files {
        if let Err(e) = iso_world::load_tile_definitions(&mut sm, &alias, data, f, *n) {
            eprintln!("PROBE load {f}: {e}");
        }
    }
    iso_world::register_fake_jumbo_tree(&mut sm, &alias, 7);

    let mut names: Vec<&String> = sm.named_map.keys().collect();
    names.sort_unstable();
    let mut sb = String::new();
    let null = |o: Option<&str>| o.unwrap_or("null").to_string();
    for n in &names {
        let r = sm.named_map[*n];
        let s = sm.sprite(r);
        let p = &s.properties;
        let flags: Vec<&str> = p.get_flags_list().iter().map(|f| f.name()).collect();
        let _ = write!(
            sb,
            "{n}\t{}\t{}\t{}\t",
            s.id,
            s.tile_type.name(),
            flags.join(",")
        );
        for k in p.get_property_names(&alias) {
            let _ = write!(sb, "{k}={};", null(p.get(&alias, k)));
        }
        sb.push('\t');
        for x in [
            s.force_ambient,
            s.solidfloor,
            s.can_be_removed,
            s.attached_floor,
            s.cut_w,
            s.cut_n,
            s.solid,
            s.solid_trans,
            s.invisible,
            s.always_draw,
            s.force_render,
            s.move_with_wind,
            s.is_bush,
            s.treat_as_wall_order,
        ] {
            sb.push(if x { '1' } else { '0' });
        }
        let _ = write!(
            sb,
            "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            s.firerequirement,
            null(s.burnt_tile.as_deref()),
            s.wind_type,
            s.render_layer,
            null(s.tileset_name.as_deref()),
            s.tile_sheet_index,
            null(s.parent_object_name.as_deref()),
            s.fascia_edge.map_or("null", |f| f.name())
        );
        match s.sprite_grid {
            None => sb.push_str("\t-"),
            Some(g) => {
                let g = sm.grid(g);
                let anchor = g
                    .get_anchor_sprite()
                    .map(|a| null(sm.sprite(a).name.as_deref()))
                    .unwrap_or("null".into());
                let _ = write!(
                    sb,
                    "\t{}x{}x{}@{}:{}",
                    g.get_width(),
                    g.get_height(),
                    g.get_levels(),
                    g.get_sprite_index(r).map_or(-1, |i| i as i32),
                    anchor
                );
            }
        }
        let u = p.init_surface(&alias);
        let b = |x: bool| if x { 1 } else { 0 };
        let _ = writeln!(
            sb,
            "\t{},{}{}{},{},{},{},{},{}",
            u.surface,
            b(u.is_surface_offset),
            b(u.is_table),
            b(u.is_table_top),
            u.stack_replace_tile_offset,
            u.item_height,
            u.sloped_surface_direction.map_or("null", |d| d.name()),
            u.sloped_surface_height_min,
            u.sloped_surface_height_max
        );
    }
    eprintln!("PROBE sprites={} intMap={}", names.len(), sm.int_map.len());
    Ok(sb)
}

/// `pz-dump fonts <root> <scratch>` (pairs with FontsProbe.java): every `.fnt` under `media/fonts` and in `media/`
/// through the ported `AngelCodeFont`, then the `TextManager` tables for every language, font size option and dyslexic
/// flag. The scratch argument is the probe's cache folder, unused here.
pub fn fonts(root: &Path) -> Result<String, String> {
    use std::rc::Rc;
    use zomboid_engine::zombie::core::core::get_option_font_size_real;
    use zomboid_engine::zombie::core::fonts::angel_code_font::AngelCodeFont;
    use zomboid_engine::zombie::ui::fonts_file;
    use zomboid_engine::zombie::ui::text_manager::{FontRef, TextManager};
    use zomboid_engine::zombie::ui::ui_font::UIFont;
    use zomboid_engine::zombie::zomboid_file_system::ZomboidFileSystem;

    const LONG: &str = "This is a long sentence that should wrap across several lines when the width is small enough, with averyveryverylongwordthatcannotfitonasingleline inside.";
    const SAMPLES: [&str; 19] = [
        "",
        "Hello, World!",
        "The quick brown fox jumps over the lazy dog. 0123456789",
        "Line one\nLine two\n\nfourth line",
        "\n",
        "AV AW Ta To Wa Yo ff fi LT",
        "  two  spaces  ",
        "Tab\there\u{0}nul",
        "a\r\nb\rc",
        "\u{dc}ber na\u{ef}ve caf\u{e9} \u{e0}\u{e9}\u{ee}\u{f5}\u{fc} \u{df} \u{a9}\u{ae}\u{20ac}\u{2122}",
        "\u{41f}\u{440}\u{438}\u{432}\u{435}\u{442}, \u{43c}\u{438}\u{440}! \u{401}\u{451}",
        "\u{4f60}\u{597d}\u{4e16}\u{754c}\u{ff0c}\u{6d4b}\u{8bd5}\u{3002}",
        "\u{3053}\u{3093}\u{306b}\u{3061}\u{306f}\u{3001}\u{30ab}\u{30bf}\u{30ab}\u{30ca}\u{6f22}\u{5b57}",
        "\u{c548}\u{b155}\u{d558}\u{c138}\u{c694} \u{d55c}\u{ad6d}\u{c5b4}",
        "\u{e2a}\u{e27}\u{e31}\u{e2a}\u{e14}\u{e35}\u{e04}\u{e23}\u{e31}\u{e1a} \u{e01}\u{e48}\u{e33}",
        "\u{1}\u{7f}\u{fffd}\u{ffff}\u{1f600}",
        "<RGB:1,0,0> Red <LINE> <SIZE:large> [img=music] <IMAGE:media/ui/x.png>",
        "Za\u{17c}\u{f3}\u{142}\u{107} g\u{119}\u{15b}l\u{105} ja\u{17a}\u{144}",
        LONG,
    ];
    const INIT_SAMPLES: [usize; 6] = [0, 1, 3, 10, 11, 15];
    const CJK4: &str = "\u{4f60}\u{597d}\u{4e16}\u{754c}\u{4f60}\u{597d}\u{4e16}\u{754c}\u{4f60}\u{597d}\u{4e16}\u{754c}\u{4f60}\u{597d}\u{4e16}\u{754c}";
    const WRAPS: [(&str, i32, i32, &str); 16] = [
        (LONG, 200, -1, ""),
        (LONG, 50, -1, ""),
        (LONG, 50, 2, "..."),
        (LONG, 120, 3, "\u{2026}"),
        (
            "first line\r\nsecond line is longer than the first\nthird",
            80,
            -1,
            "",
        ),
        ("", 100, -1, ""),
        ("   ", 0, -1, ""),
        ("short", 1000, 1, "..."),
        (CJK4, 60, -1, ""),
        ("word  double  spaces here", 40, -1, ""),
        ("\n\nlead", 100, -1, ""),
        ("trail\n\n", 100, -1, ""),
        (" x", -1, -1, ""),
        (LONG, 90, 2, "a much longer suffix than the room"),
        (
            "Supercalifragilisticexpialidociousandsomemore end",
            60,
            -1,
            "",
        ),
        ("Antidisestablishmentarianism\r\n\r\nx", 25, 4, "~"),
    ];
    const WRAP_FONTS: [UIFont; 5] = [
        UIFont::Small,
        UIFont::Medium,
        UIFont::Large,
        UIFont::Code,
        UIFont::Dialogue,
    ];
    const HEIGHTS: [i32; 29] = [
        0, 720, 1079, 1080, 1211, 1212, 1343, 1344, 1475, 1476, 1607, 1608, 1739, 1740, 1871, 1872,
        2003, 2004, 2135, 2136, 2267, 2268, 2399, 2400, 2531, 2532, 3000, 4320, -5000,
    ];
    let u16s = |s: &str| -> Vec<u16> { s.encode_utf16().collect() };
    let esc16 = |s: &[u16]| -> String {
        let mut b = String::new();
        for &c in s {
            if (0x20..0x7f).contains(&c) && c != u16::from(b'\\') {
                b.push(c as u8 as char);
            } else {
                let _ = write!(b, "\\u{c:04x}");
            }
        }
        b
    };
    let esc = |s: Option<&str>| -> String {
        match s {
            None => "null".into(),
            Some(s) => esc16(&u16s(s)),
        }
    };
    // FontsProbe.exc: the simple name of the exception's class.
    let exc = |e: &str| -> String {
        let class = e.split(':').next().unwrap_or(e);
        format!("E:{}", class.rsplit('.').next().unwrap_or(class))
    };
    let walk = |dir: &Path| -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            out.push(d.clone());
            if d.is_dir() {
                if let Ok(rd) = std::fs::read_dir(&d) {
                    for e in rd.flatten() {
                        stack.push(e.path());
                    }
                }
            }
        }
        out
    };
    // Files.walk(Paths.get("media/...")) as the probe sees it from the game folder: paths relative to the root.
    let rel_to_root = |p: &Path| -> String {
        p.strip_prefix(root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/")
    };

    let fs = ZomboidFileSystem::init(root)?;
    let base = format!("{}/", fs.base().to_string_lossy());
    let rel = |abs: &str| -> String {
        match abs.strip_prefix(&base) {
            Some(r) => format!("$/{r}"),
            None => abs.to_string(),
        }
    };
    let mut sb = String::new();

    // Part A: every font file.
    let mut files: Vec<String> = Vec::new();
    for p in walk(&root.join("media/fonts")) {
        let s = rel_to_root(&p);
        if s.ends_with(".fnt") && p.is_file() {
            files.push(s);
        }
    }
    if let Ok(rd) = std::fs::read_dir(root.join("media")) {
        for e in rd.flatten() {
            let s = rel_to_root(&e.path());
            if s.ends_with(".fnt") && e.path().is_file() {
                files.push(s);
            }
        }
    }
    files.sort_by(|a, b| java::string_compare_to(a, b));
    let _ = writeln!(sb, "FONTS {}", files.len());
    for path in &files {
        let _ = writeln!(sb, "FONT {path}");
        let f = match AngelCodeFont::new(&fs, path, None) {
            Ok(f) => f,
            Err(e) => {
                let _ = writeln!(sb, "FAIL {}", exc(&e));
                continue;
            }
        };
        let defined = f.chars.iter().flatten().count();
        // pages: the loaded page textures; none without a renderer.
        let _ = writeln!(
            sb,
            "FILE {} sdf={} lh={} chars={} defined={defined} pages=0",
            rel(f.fnt_file.as_deref().unwrap_or("null")),
            u8::from(f.is_sdf()),
            f.get_line_height(),
            f.chars.len()
        );
        for t in f.texture_requests() {
            let _ = writeln!(sb, "TEX {}", esc(Some(&rel(t))));
        }
        for d in f.chars.iter().flatten() {
            let _ = write!(
                sb,
                "C {} {} {} {} {} {} {} {} {}",
                d.id, d.x, d.y, d.width, d.height, d.xoffset, d.yoffset, d.xadvance, d.page
            );
            if let (Some(second), Some(amount)) = (&d.kerning_second, &d.kerning_amount) {
                sb.push_str(" K");
                for (s, a) in second.iter().zip(amount) {
                    let _ = write!(sb, " {s}:{a}");
                }
            }
            sb.push('\n');
        }
        for (s, sample) in SAMPLES.iter().enumerate() {
            let t = u16s(sample);
            let _ = write!(
                sb,
                "M {s} {} {} {} {} {} ",
                f.get_width(&t),
                f.get_width_xadvance(&t, true),
                f.get_height(&t),
                f.get_height_ext(&t, true, false),
                f.get_height_ext(&t, false, true)
            );
            match f.get_y_offset(&t) {
                Ok(y) => {
                    let _ = write!(sb, "{y}");
                }
                Err(e) => sb.push_str(&exc(&e)),
            }
            if t.len() >= 3 {
                let end = t.len() as i32 - 2;
                let _ = write!(
                    sb,
                    " {} {}",
                    f.get_width_range(&t, 1, end, false)?,
                    f.get_width_range(&t, 1, end, true)?
                );
            }
            sb.push('\n');
        }
    }

    // Part B: the TextManager tables.
    for o in 1..=6 {
        for h in HEIGHTS {
            let _ = writeln!(sb, "FSR {o} {h} {}", get_option_font_size_real(o, h));
        }
    }
    // Languages.init: EN, then every folder of media/lua/shared/Translate with a language.json (named after it).
    let mut langs: Vec<String> = vec!["EN".into()];
    if let Ok(rd) = std::fs::read_dir(root.join("media/lua/shared/Translate")) {
        for e in rd.flatten() {
            if e.path().is_dir() && e.path().join("language.json").exists() {
                langs.push(e.file_name().to_string_lossy().into_owned());
            }
        }
    }
    langs.sort_by(|a, b| java::string_compare_to(a, b));
    langs.dedup();
    let _ = writeln!(sb, "LANG {} {}", langs.len(), langs.join(" "));

    let mut names: Vec<String> = Vec::new();
    let mut fonts_files: Vec<String> = Vec::new();
    for p in walk(&root.join("media/fonts")) {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.len() >= 9 && name.starts_with("fonts") && name.ends_with(".txt") {
            fonts_files.push(rel_to_root(&p));
        }
    }
    fonts_files.sort_by(|a, b| java::string_compare_to(a, b));
    let mut log = Vec::new();
    for ff in &fonts_files {
        let mut m = std::collections::HashMap::new();
        let ok = fonts_file::read(&fs, fs.get_string_or(ff), &mut m, &mut log);
        let _ = writeln!(sb, "FONTSFILE {ff} {ok} {}", m.len());
        let mut keys: Vec<&String> = m.keys().collect();
        keys.sort_by(|a, b| java::string_compare_to(a, b));
        for k in keys {
            let v = &m[k];
            let _ = writeln!(
                sb,
                "FF {} id={} fnt={} img={} type={}",
                esc(Some(k)),
                esc(Some(&v.id)),
                esc(Some(&v.fnt)),
                esc(v.img.as_deref()),
                esc(v.r#type.as_deref())
            );
            names.push(v.fnt.clone());
            if let Some(img) = &v.img {
                names.push(img.clone());
            }
        }
    }
    names.sort_by(|a, b| java::string_compare_to(a, b));
    names.dedup();
    let size_dirs = [None, Some("1x"), Some("2x"), Some("3x"), Some("4x")];
    for l in &langs {
        for sd in size_dirs {
            for n in &names {
                let _ = writeln!(
                    sb,
                    "PATH {l} {} {n} {}",
                    sd.unwrap_or("null"),
                    TextManager::get_font_file_path(&fs, l, sd, n)
                );
            }
        }
    }

    let fonts = UIFont::values();
    let font_line = |f: &FontRef| -> String {
        let f = f.borrow();
        format!(
            "{} sdf={} lh={} chars={}",
            rel(f.fnt_file.as_deref().unwrap_or("null")),
            u8::from(f.is_sdf()),
            f.get_line_height(),
            f.chars.len()
        )
    };
    for l in &langs {
        for o in 1..=5 {
            for dys in 0..=1 {
                let mut tm = TextManager::new();
                let _ = write!(sb, "INIT {l} size={o} dyslexic={dys}");
                match tm.init(&fs, l, dys == 1, get_option_font_size_real(o, 720)) {
                    Ok(()) => sb.push_str(" ok\n"),
                    Err(e) => {
                        let _ = writeln!(sb, " {}", exc(&e));
                    }
                }
                let mut distinct: Vec<FontRef> = Vec::new();
                let mut id = |f: Option<&FontRef>| -> String {
                    let Some(f) = f else {
                        return "null".into();
                    };
                    if let Some(i) = distinct.iter().position(|d| Rc::ptr_eq(d, f)) {
                        return format!("#{i}");
                    }
                    distinct.push(f.clone());
                    format!("#{} {}", distinct.len() - 1, font_line(f))
                };
                for u in fonts {
                    let _ = writeln!(
                        sb,
                        "U {} {}",
                        u.name(),
                        id(tm.enum_to_font[u.ordinal()].as_ref())
                    );
                }
                for n in 0..tm.normal.len() as i32 {
                    let f = tm.get_normal_from_font_size(n + 11)?;
                    let _ = writeln!(sb, "N {} {}", n + 11, id(f.as_ref()));
                }
                let fields = [
                    &tm.font,
                    &tm.font2,
                    &tm.font3,
                    &tm.font4,
                    &tm.main1,
                    &tm.main2,
                    &tm.zombiefontcredits1,
                    &tm.zombiefontcredits2,
                    &tm.zombienew1,
                    &tm.zombienew2,
                    &tm.zombienew3,
                    &tm.zomboid_dialogue,
                    &tm.codetext,
                    &tm.code_small,
                    &tm.code_medium,
                    &tm.code_large,
                    &tm.debug_console,
                    &tm.intro,
                    &tm.handwritten,
                ];
                sb.push_str("FIELDS");
                for f in fields {
                    let _ = write!(sb, " {}", id(f.as_ref()));
                }
                sb.push('\n');
                let _ = writeln!(
                    sb,
                    "NONEN {} code={}",
                    TextManager::is_using_non_english_fonts(&fs, l),
                    tm.get_current_code_font().name()
                );
                let with_null: Vec<Option<UIFont>> =
                    fonts.iter().map(|f| Some(*f)).chain([None]).collect();
                for u in with_null {
                    let name = u.map_or("null", |f| f.name());
                    let _ = write!(sb, "H {name}");
                    for r in [tm.get_font_height(u), tm.measure_font(u)] {
                        match r {
                            Ok(v) => {
                                let _ = write!(sb, " {v}");
                            }
                            Err(e) => {
                                let _ = write!(sb, " {}", exc(&e));
                            }
                        }
                    }
                    match tm.is_sdf(u) {
                        Ok(v) => {
                            let _ = write!(sb, " {v}");
                        }
                        Err(e) => {
                            let _ = write!(sb, " {}", exc(&e));
                        }
                    }
                    sb.push('\n');
                    for s in INIT_SAMPLES {
                        let t = u16s(SAMPLES[s]);
                        let t = Some(t.as_slice());
                        let _ = writeln!(
                            sb,
                            "MS {name} {s} {} {} {} {} {}",
                            tm.measure_string_x(u, t)?,
                            tm.measure_string_y(u, t)?,
                            tm.measure_string_y_real(u, t)?,
                            tm.measure_string_y_offset(u, t)?,
                            tm.centre_string_y_offset(u, t)?
                        );
                    }
                }
                for u in WRAP_FONTS {
                    for (w, (text, width, lines, suffix)) in WRAPS.iter().enumerate() {
                        let _ = write!(sb, "WRAP {} {w} ", u.name());
                        match tm.wrap_text_ext(Some(u), &u16s(text), *width, *lines, &u16s(suffix))
                        {
                            Ok(r) => sb.push_str(&esc16(&r)),
                            Err(e) => sb.push_str(&exc(&e)),
                        }
                        sb.push('\n');
                    }
                    let r = tm.wrap_text(Some(u), &u16s(LONG), 150)?;
                    let _ = writeln!(sb, "WRAP2 {} {}", u.name(), esc16(&r));
                }
            }
        }
    }
    sb.push_str("ALLFONTS");
    for u in TextManager::new().get_all_fonts() {
        let _ = write!(sb, " {}", u.name());
    }
    sb.push('\n');
    for s in ["Small", "small", "SdfCaveat", "", "Nope", "DebugConsole"] {
        let _ = writeln!(
            sb,
            "FROMSTRING {} {}",
            esc(Some(s)),
            UIFont::from_string(s).map_or("null", |f| f.name())
        );
    }
    Ok(sb)
}

const NO_WORLD_GEN: (i32, i32, i32, i32) = (10000000, 10000000, -10000000, -10000000);

fn mix(h: &mut u64, v: i32) {
    for b in v.to_le_bytes() {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x100000001b3);
    }
}
fn mix_long(h: &mut u64, v: i64) {
    mix(h, v as i32);
    mix(h, (v >> 32) as i32);
}

/// Every lotheader of one map folder through the port of MetaGridLoaderThread.loadCell, cells in x-then-y order.
/// `lightsActive` is random in the original and not printed.
pub fn lotheaders(root: &Path, dir: &str) -> Result<String, String> {
    let media = root.join("media");
    let mut g = IsoMetaGrid::default();
    g.create_step1(&media, &[dir.to_string()], NO_WORLD_GEN)?;
    let mut sb = String::new();
    {
        let mf = &g.map_files[0];
        let _ = writeln!(
            sb,
            "M {} {} {} {} {} {} {} {}",
            mf.min_x,
            mf.min_y,
            mf.max_x,
            mf.max_y,
            mf.min_cell300_x,
            mf.min_cell300_y,
            mf.max_cell300_x,
            mf.max_cell300_y
        );
        for y in mf.min_y..=mf.max_y {
            for x in mf.min_x..=mf.max_x {
                sb.push(if mf.has_cell(x, y) { '1' } else { '0' });
            }
            sb.push('\n');
        }
        for y in mf.min_cell300_y..=mf.max_cell300_y {
            for x in mf.min_cell300_x..=mf.max_cell300_x {
                sb.push(if mf.has_cell300(x, y) { '1' } else { '0' });
            }
            sb.push('\n');
        }
    }
    let (x1, y1, x2, y2) = (
        g.map_files[0].min_x,
        g.map_files[0].min_y,
        g.map_files[0].max_x,
        g.map_files[0].max_y,
    );
    for x in x1..=x2 {
        for y in y1..=y2 {
            if !g.map_files[0].has_cell(x, y) {
                continue;
            }
            let path = g.map_files[0]
                .get_lot_header(x, y)
                .ok_or("header missing")?
                .absolute_file_path
                .clone();
            let data = std::fs::read(&path).map_err(|e| e.to_string())?;
            if let Err(e) = g.load_cell(0, &data, x, y, &mut |_| 1, &[]) {
                eprintln!("ERROR loading {}: {e}", path.display());
            }
            let h = g.map_files[0]
                .get_lot_header(x, y)
                .ok_or("header missing")?;
            let c = g.get_cell_data(x, y).ok_or("cell missing")?;
            let _ = writeln!(
                sb,
                "C {x} {y} {} {} {} {} {} {} {} {} {}",
                h.version,
                h.width,
                h.height,
                h.min_level,
                h.max_level,
                h.fixed2x,
                h.tiles_used.len(),
                c.room_list.len(),
                c.buildings.len()
            );
            for t in &h.tiles_used {
                let _ = writeln!(sb, "T {t}");
            }
            for r in &c.room_list {
                let r = g.room(*r);
                let _ = write!(
                    sb,
                    "R {} {} {} {} {} {} {} {} {} [{}]",
                    r.id,
                    r.meta_id,
                    r.level,
                    r.x,
                    r.y,
                    r.x2,
                    r.y2,
                    r.area,
                    r.building.map_or(-1, |b| g.building(b).id),
                    r.name
                );
                for q in &r.rects {
                    let _ = write!(sb, " {},{},{},{}", q.x, q.y, q.w, q.h);
                }
                sb.push_str(" |");
                for o in &r.objects {
                    let _ = write!(sb, " {},{},{}", o.r#type, o.x, o.y);
                }
                sb.push('\n');
            }
            for b in &c.buildings {
                let b = g.building(*b);
                let _ = write!(
                    sb,
                    "B {} {} {} {} {} {} r",
                    b.id, b.meta_id, b.x, b.y, b.x2, b.y2
                );
                for r in &b.rooms {
                    let _ = write!(sb, " {}", g.room(*r).id);
                }
                sb.push_str(" e");
                for r in &b.emptyoutside {
                    let _ = write!(sb, " {}", g.room(*r).id);
                }
                sb.push_str(" c");
                for v in &b.overlapped_chunks {
                    let _ = write!(sb, " {v}");
                }
                sb.push('\n');
            }
            sb.push_str("Z ");
            for i in 0..1024 {
                let _ = write!(sb, "{:02x}", h.get_zombie_intensity(i));
            }
            sb.push('\n');
        }
    }
    Ok(sb)
}

/// Every chunk of a cell range through the port of IsoLot.load; pairs with LotPackProbe.java.
pub fn lotpack(root: &Path, dir: &str, range: &[String]) -> Result<String, String> {
    let r: Vec<i32> = range
        .iter()
        .map(|s| s.parse::<i32>().map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let media = root.join("media");
    let mut g = IsoMetaGrid::default();
    g.create_step1(&media, &[dir.to_string()], NO_WORLD_GEN)?;
    let mut sb = String::new();
    let mut lot = IsoLot::default();
    for cx in r[0]..=r[2] {
        for cy in r[1]..=r[3] {
            if !g.map_files[0].has_cell(cx, cy) {
                continue;
            }
            let path = g.map_files[0]
                .get_lot_header(cx, cy)
                .ok_or("header missing")?
                .absolute_file_path
                .clone();
            let data = std::fs::read(&path).map_err(|e| e.to_string())?;
            g.load_cell(0, &data, cx, cy, &mut |_| 1, &[])?;
            let info = g.map_files[0]
                .get_lot_header(cx, cy)
                .ok_or("header missing")?;
            let pack_path = g.map_files[0]
                .info_file_names
                .get(&format!("world_{cx}_{cy}.lotpack"))
                .ok_or("no lotpack")?;
            let pack = std::fs::read(pack_path).map_err(|e| e.to_string())?;
            let mut h: u64 = 0xcbf29ce484222325;
            let n = ((info.max_level.min(31) - info.min_level.max(-32) + 1) * 64) as usize;
            let base = ((info.min_level.max(-32) - info.min_level) * 64) as usize;
            for lx in 0..32 {
                for ly in 0..32 {
                    if let Some(e) = lot.load(info, &pack, cx, cy, cx * 32 + lx, cy * 32 + ly) {
                        eprintln!("ERROR {cx},{cy} chunk {lx},{ly}: {e}");
                    }
                    mix(&mut h, lot.min_level);
                    mix(&mut h, lot.max_level);
                    mix(&mut h, lot.data.len() as i32);
                    for v in &lot.offset_in_data[base..base + n] {
                        mix(&mut h, *v);
                    }
                    for v in &lot.data {
                        mix(&mut h, *v);
                    }
                    if lx == 16 && ly == 16 {
                        let _ = write!(sb, "D {cx} {cy} {} {}", lot.min_level, lot.max_level);
                        for v in &lot.offset_in_data[base..base + n] {
                            let _ = write!(sb, " {v}");
                        }
                        sb.push_str(" |");
                        for v in &lot.data {
                            let _ = write!(sb, " {v}");
                        }
                        sb.push('\n');
                    }
                }
            }
            let _ = writeln!(sb, "L {cx} {cy} {h:x}");
        }
    }
    Ok(sb)
}

/// The whole meta grid as IsoMetaGrid.Create builds it for `Core.gameMap`, then rooms per chunk and room lookups per
/// square for a cell range; pairs with MetaGridProbe.java (run inside the booted original).
pub fn metagrid(root: &Path, game_map: &str, range: &[String]) -> Result<String, String> {
    let r: Vec<i32> = range
        .iter()
        .map(|s| s.parse::<i32>().map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let media = root.join("media");
    let mut g = IsoMetaGrid::default();
    let dirs = IsoMetaGrid::lot_directories(&media, game_map);
    g.create_step1(&media, &dirs, (-250, -250, 250, 250))?;
    for e in g.load_all_cells(&mut |_| 1, &mut |_, _| Vec::new()) {
        eprintln!("{e}");
    }
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    g.create_step2(&alarm, &mut |_| 1, &mut |a, _| a);
    let mut sb = String::new();
    let _ = writeln!(
        sb,
        "G {} {} {} {} dirs={}",
        g.min_x,
        g.min_y,
        g.max_x,
        g.max_y,
        dirs.join(";")
    );
    let mut h: u64 = 0xcbf29ce484222325;
    for b in &g.buildings {
        mix_long(&mut h, g.building(*b).id);
    }
    let _ = writeln!(sb, "B {} {h:x}", g.buildings.len());
    for cx in r[0]..=r[2] {
        for cy in r[1]..=r[3] {
            let Some(info) = g.get_header(cx, cy) else {
                continue;
            };
            let adj: String = info
                .adjacent_cells
                .iter()
                .map(|b| if *b { '1' } else { '0' })
                .collect();
            let _ = writeln!(sb, "A {cx} {cy} {adj}");
            let cell = g.get_cell_data(cx, cy).ok_or("cell missing")?;
            for ly in 0..32 {
                for lx in 0..32 {
                    if let Some(ch) = cell.chunk((ly * 32 + lx) as usize) {
                        if ch.get_rooms_size() > 0 {
                            let _ = write!(sb, "K {cx} {cy} {lx} {ly}");
                            for rr in ch.rooms() {
                                let _ = write!(sb, " {}", g.room(*rr).id);
                            }
                            sb.push('\n');
                        }
                    }
                }
            }
            let mut h: u64 = 0xcbf29ce484222325;
            for z in info.min_level..=info.max_level {
                for y in cy * 256..cy * 256 + 256 {
                    for x in cx * 256..cx * 256 + 256 {
                        mix_long(
                            &mut h,
                            g.get_room_at(x, y, z).map_or(-1, |rr| g.room(rr).id),
                        );
                        mix_long(
                            &mut h,
                            g.get_empty_outside_at(x, y, z)
                                .map_or(-1, |rr| g.room(rr).id),
                        );
                    }
                }
            }
            let _ = writeln!(sb, "H {cx} {cy} {h:x}");
        }
    }
    Ok(sb)
}

/// Tile definitions as IsoWorld.init loads them (vanilla files, no mods, no entity scripts).
fn load_tile_definitions(
    root: &Path,
) -> Result<(IsoSpriteManager, TilePropertyAliasMap, TileDefinitions), String> {
    let mut files = Vec::new();
    for (f, n) in VANILLA_TILE_DEFINITION_FILES {
        let p = root.join("media").join(f);
        files.push((
            f,
            n,
            std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?,
        ));
    }
    let mut defs = TileDefinitions::default();
    let mut alias = TilePropertyAliasMap::default();
    let mut sm = IsoSpriteManager::new();
    for (f, n, data) in &files {
        if let Err(e) = defs.load_tile_definitions_property_strings(data, f, *n) {
            eprintln!("PROBE strings {f}: {e}");
        }
    }
    defs.set_custom_property_values()?;
    defs.generate_tile_property_lookup_tables(&mut alias, &[]);
    for (f, n, data) in &files {
        if let Err(e) = iso_world::load_tile_definitions(&mut sm, &alias, data, f, *n) {
            eprintln!("PROBE load {f}: {e}");
        }
    }
    iso_world::register_fake_jumbo_tree(&mut sm, &alias, 7);
    Ok((sm, alias, defs))
}

/// Squares and objects of a chunk range through the port of CellLoader.LoadCellBinaryChunk; pairs with
/// ChunkObjectsProbe.java (run inside the booted original).
pub fn chunkobjects(root: &Path, range: &[String]) -> Result<String, String> {
    let r: Vec<i32> = range
        .iter()
        .map(|s| s.parse::<i32>().map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let media = root.join("media");
    let (mut sm, alias, mut defs) = load_tile_definitions(root)?;
    sm.add_sprite("media/ui/missing-tile.png");
    defs.loaded_tile_definitions(&mut sm, &alias, &|_| false)?;
    let mut g = IsoMetaGrid::default();
    let dirs = IsoMetaGrid::lot_directories(&media, "Muldraugh, KY");
    g.create_step1(&media, &dirs, (-250, -250, 250, 250))?;
    for e in g.load_all_cells(&mut |_| 1, &mut |_, _| Vec::new()) {
        eprintln!("{e}");
    }
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    g.create_step2(&alarm, &mut |_| 1, &mut |a, _| a);
    let mut rand = RandAbstract::new(java::Random::new(12345));
    // LockedHouses is 6 in the dedicated server's default sandbox preset, which the probe runs under.
    let mut ctx = LoadContext {
        sprites: &mut sm,
        alias: &alias,
        glass_removed_window_sprite_map: &defs.glass_removed_window_sprite_map,
        smashed_window_sprite_map: &defs.smashed_window_sprite_map,
        meta_grid: &mut g,
        rand: &mut rand,
        locked_houses: 6,
        missing_tiles: Vec::new(),
        lotpack_cache: None,
    };
    let mut sb = String::new();
    let _ = writeln!(sb, "O lockedHouses={}", ctx.locked_houses);
    let b = |v: bool| if v { 1 } else { 0 };
    for wx in r[0]..=r[2] {
        for wy in r[1]..=r[3] {
            *ctx.rand = RandAbstract::new(java::Random::new(12345));
            let mut ch = IsoChunk::new(wx, wy);
            let mut errors = Vec::new();
            let ok = cell_loader::load_cell_binary_chunk(&mut ctx, wx, wy, &mut ch, &mut errors);
            for e in errors {
                eprintln!("chunk {wx},{wy}: {e}");
            }
            let mut taint = false;
            for z in ch.get_min_level()..=ch.get_max_level() {
                for i in 0..64 {
                    if let Some(sq) = ch.get_grid_square(i % 8, i / 8, z) {
                        taint |= sq.objects.iter().any(|o| {
                            matches!(o.kind, ObjectKind::IsoRadio | ObjectKind::IsoTelevision)
                        });
                    }
                }
            }
            let _ = writeln!(
                sb,
                "C {wx} {wy} {ok} {} {}",
                ch.get_min_level(),
                ch.get_max_level()
            );
            for z in ch.get_min_level()..=ch.get_max_level() {
                for y in 0..8 {
                    for x in 0..8 {
                        let Some(sq) = ch.get_grid_square(x, y, z) else {
                            continue;
                        };
                        let _ = write!(sb, "S {x} {y} {z} {}", sq.room_id);
                        let name = |s: Option<_>| {
                            s.and_then(|s| ctx.sprites.sprite(s).name.clone())
                                .unwrap_or_else(|| "null".to_string())
                        };
                        for o in &sq.objects {
                            let _ = write!(sb, " {}:{}", o.kind.class_name(), name(o.sprite));
                            for at in o.attached_anim_sprite.iter().flatten() {
                                let _ = write!(sb, "+{}", name(Some(*at)));
                            }
                            if o.special {
                                sb.push('*');
                            }
                            if o.render_y_offset != 0.0 {
                                let _ = write!(sb, "@{:x}", o.render_y_offset.to_bits());
                            }
                            match &o.kind {
                                ObjectKind::IsoDoor(d) if taint => {
                                    let _ = write!(sb, "{{o{}n{}?}}", b(d.open), b(d.north));
                                }
                                ObjectKind::IsoDoor(d) => {
                                    let _ = write!(
                                        sb,
                                        "{{o{}n{}l{}k{}}}",
                                        b(d.open),
                                        b(d.north),
                                        b(d.locked),
                                        b(d.locked_by_key)
                                    );
                                }
                                ObjectKind::IsoWindow(w) if taint => {
                                    let _ = write!(
                                        sb,
                                        "{{n{}?p{}s{}}}",
                                        b(w.north),
                                        b(w.perma_locked),
                                        b(w.smashed)
                                    );
                                }
                                ObjectKind::IsoWindow(w) => {
                                    let _ = write!(
                                        sb,
                                        "{{n{}l{}p{}s{}}}",
                                        b(w.north),
                                        b(w.locked),
                                        b(w.perma_locked),
                                        b(w.smashed)
                                    );
                                }
                                ObjectKind::IsoTree(t) => {
                                    let _ = write!(sb, "{{s{}}}", t.size);
                                }
                                _ => {}
                            }
                            if o.sprite.is_some_and(|s| {
                                ctx.sprites
                                    .sprite(s)
                                    .properties
                                    .has(IsoFlagType::vegitation)
                            }) {
                                if taint {
                                    sb.push_str("{t?}");
                                } else {
                                    let _ = write!(
                                        sb,
                                        "{{t{:x},{:x},{:x}}}",
                                        o.tintr.to_bits(),
                                        o.tintg.to_bits(),
                                        o.tintb.to_bits()
                                    );
                                }
                            }
                        }
                        sb.push('\n');
                    }
                }
            }
            if !taint {
                let _ = writeln!(sb, "X {}", ctx.rand.next(1000000));
            }
        }
    }
    for t in &ctx.missing_tiles {
        eprintln!("Missing tile definition: {t}");
    }
    Ok(sb)
}

struct ChunkGrid<'a>(std::collections::HashMap<(i32, i32, i32), &'a IsoGridSquare>);
impl GetSquare for ChunkGrid<'_> {
    fn get_grid_square(&self, x: i32, y: i32, z: i32) -> Option<&IsoGridSquare> {
        self.0.get(&(x, y, z)).copied()
    }
}

/// Square properties and collide/path/vision matrices of a chunk range; pairs with CollideProbe.java.
pub fn collide(root: &Path, range: &[String]) -> Result<String, String> {
    let r: Vec<i32> = range
        .iter()
        .map(|s| s.parse::<i32>().map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let mut rand = RandAbstract::new(java::Random::new(12345));
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    let mut world = IsoWorld::init(root, "Muldraugh, KY", &|_| false, &alarm, None, &mut rand)?;
    let mut chunks = Vec::new();
    {
        let IsoWorld {
            sprites,
            alias,
            tile_definitions,
            meta_grid,
            ..
        } = &mut world;
        let mut ctx = LoadContext {
            sprites,
            alias,
            glass_removed_window_sprite_map: &tile_definitions.glass_removed_window_sprite_map,
            smashed_window_sprite_map: &tile_definitions.smashed_window_sprite_map,
            meta_grid,
            rand: &mut rand,
            locked_houses: 6,
            missing_tiles: Vec::new(),
            lotpack_cache: None,
        };
        for wx in r[0]..=r[2] {
            for wy in r[1]..=r[3] {
                *ctx.rand = RandAbstract::new(java::Random::new(12345));
                let mut ch = IsoChunk::new(wx, wy);
                cell_loader::load_cell_binary_chunk(&mut ctx, wx, wy, &mut ch, &mut Vec::new());
                chunks.push(ch);
            }
        }
    }
    for ch in &mut chunks {
        for z in ch.get_min_level()..=ch.get_max_level() {
            for i in 0..64 {
                if let Some(sq) = ch.get_grid_square_mut(i % 8, i / 8, z) {
                    sq.recalc_properties(&world.sprites);
                }
            }
        }
    }
    let mut all = Vec::new();
    for ch in &chunks {
        for z in ch.get_min_level()..=ch.get_max_level() {
            for y in 0..8 {
                for x in 0..8 {
                    all.extend(ch.get_grid_square(x, y, z));
                }
            }
        }
    }
    let grid = ChunkGrid(all.iter().map(|s| ((s.x, s.y, s.z), *s)).collect());
    let mut sb = String::new();
    for sq in &all {
        let flags: Vec<&str> = sq
            .properties
            .get_flags_list()
            .iter()
            .map(|f| f.name())
            .collect();
        let _ = write!(
            sb,
            "Q {} {} {} {} {} ",
            sq.x,
            sq.y,
            sq.z,
            if sq.have_roof { 1 } else { 0 },
            flags.join(",")
        );
        for k in sq.properties.get_property_names(&world.alias) {
            let _ = write!(
                sb,
                "{k}={};",
                sq.properties.get(&world.alias, k).unwrap_or("null")
            );
        }
        let (c, p, v) = sq.calculate_matrices(&world.alias, &grid);
        let _ = writeln!(sb, " {:x} {c:x} {p:x} {v:x}", sq.has_types);
    }
    Ok(sb)
}

/// Entity scripts in ScriptManager order and the sprites they mark; pairs with EntitySpritesProbe.java.
pub fn entitysprites(root: &Path) -> Result<String, String> {
    let mut rand = RandAbstract::new(java::Random::new(12345));
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    let world = IsoWorld::init(root, "Muldraugh, KY", &|_| false, &alarm, None, &mut rand)?;
    for l in &world.log {
        eprintln!("{l}");
    }
    let mut sb = String::new();
    for e in &world.scripts.entities {
        let _ = writeln!(sb, "E {}", e.name);
    }
    let mut names: Vec<&String> = world.sprites.named_map.keys().collect();
    names.sort_unstable();
    for n in names {
        let p = &world.sprites.sprite(world.sprites.named_map[n]).properties;
        let name = p.get(&world.alias, "EntityScriptName");
        if p.has(IsoFlagType::EntityScript) || name.is_some() {
            let _ = writeln!(
                sb,
                "S {n} {} {}",
                if p.has(IsoFlagType::EntityScript) {
                    1
                } else {
                    0
                },
                name.unwrap_or("null")
            );
        }
    }
    Ok(sb)
}

/// Collide matrices and door/window collision objects of one 64-square server cell; pairs with SpecialCollideProbe.java.
pub fn specialcollide(root: &Path, cell: &[String], los: bool) -> Result<String, String> {
    specialcollide_mode(root, cell, if los { 1 } else { 0 })
}

/// mode 0: collision objects, 1: line of sight (LosProbe.java), 2: stair heights (StairsProbe.java).
fn specialcollide_mode(root: &Path, cell: &[String], mode: u8) -> Result<String, String> {
    let los = mode == 1;
    let toggle = cell.len() == 3;
    let c: Vec<i32> = cell[..2]
        .iter()
        .map(|s| s.parse::<i32>().map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let mut rand = RandAbstract::new(java::Random::new(12345));
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    let mut world = IsoWorld::init(root, "Muldraugh, KY", &|_| false, &alarm, None, &mut rand)?;
    let mut chunks = Vec::new();
    {
        let IsoWorld {
            sprites,
            alias,
            tile_definitions,
            meta_grid,
            ..
        } = &mut world;
        let mut ctx = LoadContext {
            sprites,
            alias,
            glass_removed_window_sprite_map: &tile_definitions.glass_removed_window_sprite_map,
            smashed_window_sprite_map: &tile_definitions.smashed_window_sprite_map,
            meta_grid,
            rand: &mut rand,
            locked_houses: 6,
            missing_tiles: Vec::new(),
            lotpack_cache: None,
        };
        for cx in 0..8 {
            for cy in 0..8 {
                *ctx.rand = RandAbstract::new(java::Random::new(12345));
                let (wx, wy) = (c[0] * 8 + cx, c[1] * 8 + cy);
                let mut ch = IsoChunk::new(wx, wy);
                cell_loader::load_cell_binary_chunk(&mut ctx, wx, wy, &mut ch, &mut Vec::new());
                chunks.push(ch);
            }
        }
    }
    for ch in &mut chunks {
        for z in ch.get_min_level()..=ch.get_max_level() {
            for i in 0..64 {
                if let Some(sq) = ch.get_grid_square_mut(i % 8, i / 8, z) {
                    if toggle {
                        for o in &mut sq.objects {
                            o.toggle_door_silent();
                        }
                    }
                    sq.recalc_properties(&world.sprites);
                }
            }
        }
    }
    if los {
        // Store the vision matrices (RecalcAllWithNeighbours), which testVisionAdjacent reads.
        let mut vm = Vec::new();
        {
            let mut all = Vec::new();
            for ch in &chunks {
                for z in ch.get_min_level()..=ch.get_max_level() {
                    for i in 0..64 {
                        all.extend(ch.get_grid_square(i % 8, i / 8, z));
                    }
                }
            }
            let grid = ChunkGrid(all.iter().map(|s| ((s.x, s.y, s.z), *s)).collect());
            for sq in &all {
                vm.push(sq.calculate_matrices(&world.alias, &grid).2);
            }
        }
        let mut it = vm.into_iter();
        for ch in &mut chunks {
            for z in ch.get_min_level()..=ch.get_max_level() {
                for i in 0..64 {
                    if let Some(sq) = ch.get_grid_square_mut(i % 8, i / 8, z) {
                        sq.vision_matrix = it.next().unwrap_or(0);
                    }
                }
            }
        }
    }
    let mut all = Vec::new();
    for ch in &chunks {
        for z in ch.get_min_level()..=ch.get_max_level() {
            for y in 0..8 {
                for x in 0..8 {
                    all.extend(ch.get_grid_square(x, y, z));
                }
            }
        }
    }
    let grid = ChunkGrid(all.iter().map(|s| ((s.x, s.y, s.z), *s)).collect());
    if mode == 2 {
        let mut sb = String::from("F true\n");
        for sq in &all {
            if !sq.has_stairs() && sq.get_sloped_surface_direction(&world.alias).is_none() {
                continue;
            }
            let (cm, _, _) = sq.calculate_matrices(&world.alias, &grid);
            let _ = write!(
                sb,
                "Z {} {} {} {} {cm:x}",
                sq.x,
                sq.y,
                sq.z,
                sq.get_stairs().name()
            );
            for py in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
                for px in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
                    let _ = write!(
                        sb,
                        " {:x}",
                        sq.get_apparent_z(&world.alias, px, py, true).to_bits()
                    );
                }
            }
            sb.push('\n');
        }
        return Ok(sb);
    }
    if los {
        let mut sb = String::new();
        for sq in &all {
            if sq.z != 0 || sq.x % 8 != 3 || sq.y % 8 != 3 {
                continue;
            }
            let _ = write!(sb, "L {} {} ", sq.x, sq.y);
            for z in 0..=1 {
                for dy in -10..=10 {
                    for dx in -10..=10 {
                        let r = line_clear(
                            &world.sprites,
                            &world.alias,
                            &grid,
                            sq.x,
                            sq.y,
                            0,
                            sq.x + dx,
                            sq.y + dy,
                            z,
                            false,
                            10000,
                            true,
                        );
                        let _ = write!(sb, "{}", r as u8);
                    }
                }
            }
            sb.push('\n');
        }
        return Ok(sb);
    }
    let mut sb = String::new();
    for sq in &all {
        let (cm, _, _) = sq.calculate_matrices(&world.alias, &grid);
        let _ = write!(sb, "P {} {} {} {cm:x}", sq.x, sq.y, sq.z);
        for (dx, dy) in [
            (0, -1),
            (-1, -1),
            (-1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
            (1, 0),
            (1, -1),
        ] {
            let next = grid.get_grid_square(sq.x + dx, sq.y + dy, sq.z);
            match sq.test_collide_special_objects(&world.alias, next, &grid) {
                None => sb.push_str(" -"),
                Some((x, y, z, i)) => {
                    let o = &grid
                        .get_grid_square(x, y, z)
                        .ok_or("object square")?
                        .objects[i];
                    let name = o
                        .sprite
                        .and_then(|s| world.sprites.sprite(s).name.clone())
                        .unwrap_or_else(|| "null".into());
                    let _ = write!(sb, " {}:{name}@{x},{y}", o.kind.class_name());
                }
            }
        }
        sb.push('\n');
    }
    Ok(sb)
}

/// The meta grid with procedural basements merged in for the oracle seed; pairs with BasementsProbe.java.
pub fn basements(root: &Path, range: &[String]) -> Result<String, String> {
    use zomboid_engine::zombie::basements::Basements;
    let r: Vec<i32> = range
        .iter()
        .map(|s| s.parse::<i32>().map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let media = root.join("media");
    let mut g = IsoMetaGrid::default();
    let dirs = IsoMetaGrid::lot_directories(&media, "Muldraugh, KY");
    g.create_step1(&media, &dirs, (-250, -250, 250, 250))?;
    for e in g.load_all_cells(&mut |_| 1, &mut |_, _| Vec::new()) {
        eprintln!("{e}");
    }
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    g.create_step2(&alarm, &mut |_| 1, &mut |a, _| a);
    // WorldGenParams.setSeedString: the seed is the Java hash of the seed text.
    let seed = java::string_hash_code("DeadOxideOracle") as i64;
    let b = Basements::before_load_meta_grid(&media, &mut g, &dirs, seed)?;
    for l in &b.log {
        eprintln!("{l}");
    }
    let mut sb = String::new();
    let _ = writeln!(sb, "SEED {seed}");
    for p in &b.basement_placements {
        let _ = writeln!(sb, "P {} {} {} {} {} {}", p.name, p.x, p.y, p.z, p.w, p.h);
    }
    let mut h: u64 = 0xcbf29ce484222325;
    for bd in &g.buildings {
        mix_long(&mut h, g.building(*bd).id);
    }
    let _ = writeln!(sb, "B {} {h:x}", g.buildings.len());
    for cx in r[0]..=r[2] {
        for cy in r[1]..=r[3] {
            if g.get_header(cx, cy).is_none() {
                continue;
            }
            let Some(cell) = g.get_cell_data(cx, cy) else {
                continue;
            };
            let _ = writeln!(
                sb,
                "C {cx} {cy} {} {}",
                cell.room_list.len(),
                cell.buildings.len()
            );
            for rr in &cell.room_list {
                let room = g.room(*rr);
                let _ = writeln!(
                    sb,
                    "R {} {} {} {} {} {} {} {} [{}]",
                    room.id,
                    room.meta_id,
                    room.level,
                    room.x,
                    room.y,
                    room.x2,
                    room.y2,
                    room.building.map_or(-1, |b| g.building(b).id),
                    room.name
                );
            }
            for bd in &cell.buildings {
                let d = g.building(*bd);
                let _ = writeln!(
                    sb,
                    "D {} {} {} {} {} {} {} {}",
                    d.id,
                    d.meta_id,
                    d.x,
                    d.y,
                    d.x2,
                    d.y2,
                    d.rooms.len(),
                    d.emptyoutside.len()
                );
            }
            for ly in 0..32 {
                for lx in 0..32 {
                    let Some(ch) = cell.chunk((ly * 32 + lx) as usize) else {
                        continue;
                    };
                    if ch.get_rooms_size() == 0 {
                        continue;
                    }
                    let mut ids: Vec<i64> = ch.rooms().iter().map(|rr| g.room(*rr).id).collect();
                    ids.sort_unstable();
                    let _ = write!(sb, "K {cx} {cy} {lx} {ly}");
                    for id in ids {
                        let _ = write!(sb, " {id}");
                    }
                    sb.push('\n');
                }
            }
            let mut h: u64 = 0xcbf29ce484222325;
            for z in -32..=8 {
                for y in cy * 256..cy * 256 + 256 {
                    for x in cx * 256..cx * 256 + 256 {
                        mix_long(
                            &mut h,
                            g.get_room_at(x, y, z).map_or(-1, |rr| g.room(rr).id),
                        );
                        mix_long(
                            &mut h,
                            g.get_empty_outside_at(x, y, z)
                                .map_or(-1, |rr| g.room(rr).id),
                        );
                    }
                }
            }
            let _ = writeln!(sb, "H {cx} {cy} {h:x}");
        }
    }
    Ok(sb)
}

/// Sandbox options at their defaults and after loading a save's map_sand.bin; pairs with SandboxProbe.java.
pub fn sandbox(root: &Path, file: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::sandbox_options::{Kind, SandboxOptions};
    let mut s = SandboxOptions::new();
    let preset = std::fs::read_to_string(root.join("media/lua/shared/Sandbox/Apocalypse.lua"))
        .map_err(|e| e.to_string())?;
    s.load_game_file(&preset);
    let mut sb = String::new();
    let dump = |sb: &mut String, tag: &str, s: &SandboxOptions| {
        for (i, d) in s.defs().iter().enumerate() {
            let ty = match d.kind {
                Kind::Boolean(_) => "boolean",
                Kind::Double { .. } => "double",
                Kind::Integer { .. } => "integer",
                Kind::Enum { .. } => "enum",
                Kind::Str { .. } => "string",
            };
            let _ = writeln!(sb, "{tag}\t{}\t{ty}\t{}", d.name, s.value_as_string(i));
        }
    };
    dump(&mut sb, "D", &s);
    s.load(&std::fs::read(file).map_err(|e| e.to_string())?)?;
    for l in &s.log {
        eprintln!("{l}");
    }
    dump(&mut sb, "L", &s);
    let _ = writeln!(sb, "P\t{}", s.preset);
    Ok(sb)
}

/// Item scripts, pairs with ItemsProbe.java: every loaded item sorted by full name, with the fields named in `fields`
/// (one per line, from tools/gen_item_params.py); floats as raw bits, null as "null".
pub fn items(root: &Path, fields: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::scripting::item::Value;
    use zomboid_engine::zombie::scripting::script_manager::ScriptManager;
    let names =
        std::fs::read_to_string(fields).map_err(|e| format!("{}: {e}", fields.display()))?;
    let names: Vec<&str> = names.lines().filter(|l| !l.is_empty()).collect();
    let m = ScriptManager::load(root)?;
    for (n, e) in &m.item_errors {
        eprintln!("dropped {n}: {e}");
    }
    let mut items: Vec<_> = m.items.iter().collect();
    // java.lang.String.compareTo: UTF-16 order; item names are ASCII.
    items.sort_by_key(|i| i.full_name());
    let mut sb = String::new();
    for it in items {
        sb.push_str(&it.full_name());
        for n in &names {
            let v = it.get(n).ok_or_else(|| format!("no field {n}"))?;
            sb.push('\t');
            match v {
                Value::F32(f) => {
                    let _ = write!(sb, "{:x}", f.to_bits());
                }
                Value::I32(i) => {
                    let _ = write!(sb, "{i}");
                }
                Value::Bool(b) => {
                    let _ = write!(sb, "{b}");
                }
                Value::Str(x) => sb.push_str(x.as_deref().unwrap_or("null")),
            }
        }
        sb.push('\n');
    }
    Ok(sb)
}

/// Craft recipes (`craftRecipe` scripts), pairs with RecipesProbe.java: every loaded recipe sorted by full type, with
/// the fields Load, InitLoadPP, OnScriptsLoaded and OnPostWorldDictionaryInit set; io lines by their index in
/// `ioLines`.
pub fn recipes(root: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::scripting::entity::components::crafting::craft_recipe::{
        IoScript, RequiredSkill, LUA_CALLS,
    };
    use zomboid_engine::zombie::scripting::entity::components::crafting::output_script::MapperRef;
    use zomboid_engine::zombie::scripting::script_manager::ScriptManager;
    /// RecipesProbe.esc: '\\', tab, LF, CR and every UTF-16 unit outside 0x20..0x7e escaped.
    fn esc(s: Option<&str>) -> String {
        let Some(s) = s else {
            return "null".into();
        };
        let mut sb = String::new();
        for u in s.encode_utf16() {
            match u {
                0x5c => sb.push_str("\\\\"),
                0x09 => sb.push_str("\\t"),
                0x0a => sb.push_str("\\n"),
                0x0d => sb.push_str("\\r"),
                0x20..=0x7e => sb.push(char::from(u as u8)),
                _ => {
                    let _ = write!(sb, "\\u{u:04x}");
                }
            }
        }
        sb
    }
    /// RecipesProbe.val of a Collection: `[a, b]`.
    fn list<T>(v: &[T], f: impl Fn(&T) -> String) -> String {
        let items: Vec<String> = v.iter().map(f).collect();
        format!("[{}]", items.join(", "))
    }
    let strs = |v: &[String]| list(v, |s| esc(Some(s)));
    let floats = |v: &[f32]| list(v, |f| java::float_to_string(*f));
    let skills = |v: &Option<Vec<RequiredSkill>>| match v {
        None => "null".to_string(),
        Some(v) => list(v, |s| format!("{}:{}", esc(Some(s.perk)), s.level)),
    };
    let opt = |v: &Option<usize>| v.map_or("null".to_string(), |i| i.to_string());
    let idx = |v: &[usize]| list(v, |i| i.to_string());

    let m = ScriptManager::load(root)?;
    for (n, e) in &m.craft_recipe_errors {
        eprintln!("dropped {n}: {e}");
    }
    for e in [
        &m.craft_recipe_scripts_loaded_error,
        &m.craft_recipe_post_init_error,
    ]
    .into_iter()
    .flatten()
    {
        eprintln!("aborted: {e}");
    }
    let mut recipes: Vec<_> = m.craft_recipes.iter().collect();
    recipes.sort_by(|a, b| {
        java::string_compare_to(
            &a.get_script_object_full_type(),
            &b.get_script_object_full_type(),
        )
    });
    let mut sb = String::new();
    for r in recipes {
        let ft = esc(Some(&r.get_script_object_full_type()));
        let mut lines: Vec<String> = Vec::new();
        lines.push(format!("name\t{}", esc(r.name.as_deref())));
        lines.push(format!(
            "translationName\t{}",
            esc(r.translation_name.as_deref())
        ));
        lines.push(format!(
            "enabled\t{}\tdebugOnly\t{}",
            r.enabled, r.debug_only
        ));
        lines.push(format!(
            "modId\t{}\texistsAsVanilla\t{}",
            esc(r.mod_id.as_deref()),
            r.exists_as_vanilla
        ));
        lines.push(format!("scriptVersion\t{:x}", r.script_version as u64));
        lines.push(format!("iconName\t{}", esc(r.icon_name.as_deref())));
        lines.push(format!("recipeGroup\t{}", esc(r.recipe_group)));
        lines.push(format!("needToBeLearn\t{}", r.need_to_be_learn));
        lines.push(format!("skillRequired\t{}", skills(&r.skill_required)));
        lines.push(format!("autoLearnAny\t{}", skills(&r.auto_learn_any)));
        lines.push(format!("autoLearnAll\t{}", skills(&r.auto_learn_all)));
        lines.push(format!(
            "xpAward\t{}",
            match &r.xp_award {
                None => "null".to_string(),
                Some(v) => list(v, |x| format!("{}:{}", esc(Some(x.perk)), x.amount)),
            }
        ));
        lines.push(format!("metaRecipe\t{}", esc(r.meta_recipe.as_deref())));
        lines.push(format!("time\t{}", r.time));
        lines.push(format!(
            "loadedTimedActionScript\t{}",
            esc(r.loaded_timed_action_script.as_deref())
        ));
        lines.push(format!(
            "timedActionScript\t{}",
            esc(r.timed_action_script.as_deref())
        ));
        let mut lc = String::from("luaCalls");
        for (i, c) in LUA_CALLS.iter().enumerate() {
            if let Some(v) = &r.lua_calls[i] {
                let _ = write!(lc, "\t{c}={}", esc(Some(v)));
            }
        }
        lines.push(lc);
        lines.push(format!("onAddToMenu\t{}", esc(r.on_add_to_menu.as_deref())));
        lines.push(format!("category\t{}", esc(r.category.as_deref())));
        lines.push(format!("categoryTags\t{}", strs(&r.category_tags)));
        lines.push(format!("animation\t{}", esc(r.animation.as_deref())));
        lines.push(format!("tooltip\t{}", esc(r.tooltip.as_deref())));
        lines.push(format!(
            "allowBatchCraft\t{}\tcanWalk\t{}",
            r.allow_batch_craft, r.can_walk
        ));
        lines.push(format!("researchSkillLevel\t{}", r.research_skill_level));
        lines.push(format!(
            "researchAll\t{}\tresearchAny\t{}",
            list(&r.research_all, |p| esc(Some(p))),
            list(&r.research_any, |p| esc(Some(p)))
        ));
        lines.push(format!(
            "inputs\t{}\toutputs\t{}",
            idx(&r.inputs),
            idx(&r.outputs)
        ));
        for (i, s) in r.io_lines.iter().enumerate() {
            let mut l = format!("io\t{i}");
            match s {
                IoScript::Input(s) => {
                    let mut tags: Vec<String> = s
                        .item_tags
                        .iter()
                        .map(|t| t.as_ref().map_or("null".to_string(), |t| t.to_string()))
                        .collect();
                    tags.sort_by(|a, b| java::string_compare_to(a, b));
                    let _ = write!(
                        l,
                        "\tin\t{}\tline\t{}\tamount\t{}\t{}\tamounts\t{}\t{}\tloadedItems\t{}\tloadedFluids\t{}\tloadedEnergies\t{}\titemTags\t{}\tcategories\t{}\tany\t{}\t{}\t{}\tapplyMode\t{}\tfluidMode\t{}\tapplyOnTick\t{}\tshapedIndex\t{}\tflags\t{}\tcreateTo\t{}\tconsumeFrom\t{}\tparent\t{}",
                        s.r#type.name(),
                        esc(Some(&s.original_line)),
                        java::float_to_string(s.amount),
                        java::float_to_string(s.maxamount),
                        floats(&s.amounts),
                        floats(&s.maxamounts),
                        strs(&s.loaded_items),
                        strs(&s.loaded_fluids),
                        strs(&s.loaded_energies),
                        strs(&tags),
                        strs(&s.categories),
                        s.accepts_any_item,
                        s.accepts_any_fluid,
                        s.accepts_any_energy,
                        s.item_apply_mode.name(),
                        s.fluid_match_mode.name(),
                        s.apply_on_tick,
                        s.shaped_index,
                        list(&s.flags.iter().collect::<Vec<_>>(), |f| f.name().to_string()),
                        opt(&s.create_to_item_script),
                        opt(&s.consume_from_item_script),
                        opt(&s.parent_script),
                    );
                }
                // the null of a bare `+` line: the probe would fail on it (none in the game's scripts)
                IoScript::Null => l.push_str("\tnull"),
                IoScript::Output(o) => {
                    let mapper = match &o.output_mapper {
                        Some(MapperRef::Recipe(n)) => r.get_output_mapper(n),
                        Some(MapperRef::Own(m)) => Some(m),
                        None => None,
                    };
                    let _ = write!(
                        l,
                        "\tout\t{}\tline\t{}\tamount\t{}\t{}\tchance\t{}\tloadedFluid\t{}\tloadedEnergy\t{}\tapplyMode\t{}\tfluidMode\t{}\tapplyOnTick\t{}\tshapedIndex\t{}\tflags\t{}\tcreateTo\t{}\tmapper\t{}\tresults\t{}",
                        o.r#type.name(),
                        esc(Some(&o.original_line)),
                        java::float_to_string(o.amount),
                        java::float_to_string(o.maxamount),
                        java::float_to_string(o.chance),
                        esc(o.loaded_fluid.as_deref()),
                        esc(o.loaded_energy.as_deref()),
                        o.item_apply_mode.name(),
                        o.fluid_match_mode.name(),
                        o.apply_on_tick,
                        o.shaped_index,
                        list(&o.flags.iter().collect::<Vec<_>>(), |f| f.name().to_string()),
                        opt(&o.create_to_item_script),
                        esc(o.mapper_name()),
                        mapper.map_or("null".to_string(), |m| strs(&m.result_items)),
                    );
                }
            }
            lines.push(l);
        }
        lines.push(format!(
            "prop1\t{}\tprop2\t{}",
            opt(&r.prop1),
            opt(&r.prop2)
        ));
        lines.push(format!(
            "toolLeft\t{}\ttoolRight\t{}\tusesTools\t{}",
            opt(&r.tool_left),
            opt(&r.tool_right),
            r.uses_tools
        ));
        lines.push(format!(
            "hasOnTick\t{}\t{}",
            r.has_on_tick_inputs, r.has_on_tick_outputs
        ));
        let mut mappers: Vec<_> = r.output_mappers.iter().collect();
        mappers.sort_by(|a, b| java::string_compare_to(&a.0, &b.0));
        for (n, m) in mappers {
            let mut l = format!("outputMapper\t{}\tdefault\t", esc(Some(n)));
            match &m.default_output_entree {
                None => l.push_str("null"),
                Some(d) => {
                    let _ = write!(
                        l,
                        "{}={}",
                        esc(Some(&d.full_type)),
                        esc(d.result.as_deref())
                    );
                }
            }
            let entrees = list(&m.entrees, |e| {
                format!(
                    "{}={} <- {}={}",
                    esc(Some(&e.full_type)),
                    esc(e.result.as_deref()),
                    strs(&e.pattern_full_types),
                    strs(&e.pattern)
                )
            });
            let mut map: Vec<String> = m
                .entree_map
                .iter()
                .map(|(k, i)| format!("{}={i}", esc(Some(k))))
                .collect();
            map.sort_by(|a, b| java::string_compare_to(a, b));
            let _ = write!(
                l,
                "\tentrees\t{entrees}\tentreeMap\t{}\tresults\t{}\tinputs\t{}",
                strs(&map),
                strs(&m.result_items),
                idx(&m.input_scripts)
            );
            lines.push(l);
        }
        let om = &r.overlay_mapper;
        let mut styles: Vec<String> = om
            .style_map
            .iter()
            .map(|(item, style)| format!("{item}={style}"))
            .collect();
        styles.sort_by(|a, b| java::string_compare_to(a, b));
        lines.push(format!(
            "overlayMapper\tdefault\t{}\tstyles\t{}\tinput\t{}",
            strs(&om.default_styles),
            strs(&styles),
            opt(&om.input_script)
        ));
        for line in lines {
            let _ = writeln!(sb, "{ft}\t{line}");
        }
    }
    Ok(sb)
}

/// zombie.scripting.objects.VehicleScript / VehicleTemplate as the server loads them, pairs with VehiclesProbe.java:
/// every vehicle script sorted by full name, then every vehicle template sorted by full type with the script its
/// getScript() built (if any); every field in declaration order, one line each.
pub fn vehicles(root: &Path) -> Result<String, String> {
    use zomboid_engine::org::joml::vector2f::Vector2f;
    use zomboid_engine::org::joml::vector3f::Vector3f;
    use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::{KahluaTable, LuaValue};
    use zomboid_engine::zombie::core::immutable_color::ImmutableColor;
    use zomboid_engine::zombie::scripting::objects::vehicle_script::{
        Anim, Area, Model, Part, Passenger, PhysicsShape, Position, Skin, SwitchSeat,
        VehicleScript, Wheel,
    };
    use zomboid_engine::zombie::scripting::script_manager::ScriptManager;
    /// VehiclesProbe.esc: '\\', tab, LF, CR and every UTF-16 unit outside 0x20..0x7e escaped.
    fn esc(s: Option<&str>) -> String {
        let Some(s) = s else {
            return "null".into();
        };
        let mut sb = String::new();
        for u in s.encode_utf16() {
            match u {
                0x5c => sb.push_str("\\\\"),
                0x09 => sb.push_str("\\t"),
                0x0a => sb.push_str("\\n"),
                0x0d => sb.push_str("\\r"),
                0x20..=0x7e => sb.push(char::from(u as u8)),
                _ => {
                    let _ = write!(sb, "\\u{u:04x}");
                }
            }
        }
        sb
    }
    fn o(s: &Option<String>) -> String {
        esc(s.as_deref())
    }
    fn f(x: f32) -> String {
        java::float_to_string(x)
    }
    fn v3(v: &Vector3f) -> String {
        format!("({}, {}, {})", f(v.x), f(v.y), f(v.z))
    }
    fn v2(v: &Vector2f) -> String {
        format!("({}, {})", f(v.x), f(v.y))
    }
    fn color(c: &Option<ImmutableColor>) -> String {
        match c {
            None => "null".into(),
            Some(c) => format!("rgba({}, {}, {}, {})", f(c.r), f(c.g), f(c.b), f(c.a)),
        }
    }
    fn floats(v: &[f32]) -> String {
        let l: Vec<String> = v.iter().map(|x| f(*x)).collect();
        format!("[{}]", l.join(", "))
    }
    fn strs(v: &[String]) -> String {
        let l: Vec<String> = v.iter().map(|x| esc(Some(x))).collect();
        format!("[{}]", l.join(", "))
    }
    fn ostrs(v: &Option<Vec<String>>) -> String {
        v.as_ref().map_or("null".into(), |v| strs(v))
    }
    /// VehiclesProbe.val of a Set: the escaped elements, sorted.
    fn set(v: &[String]) -> String {
        let mut l: Vec<String> = v.iter().map(|x| esc(Some(x))).collect();
        l.sort();
        format!("[{}]", l.join(", "))
    }
    /// VehiclesProbe.val of a Map: "k=v" entries, sorted.
    fn map(v: &[(String, String)]) -> String {
        let mut l: Vec<String> = v
            .iter()
            .map(|(k, x)| format!("{}={}", esc(Some(k)), esc(Some(x))))
            .collect();
        l.sort();
        format!("{{{}}}", l.join(", "))
    }
    /// VehiclesProbe.table: "n:" + Double.toString or "s:" + the escaped key, "=", the value; sorted.
    fn table(t: &KahluaTable) -> String {
        let mut l: Vec<String> = t
            .entries
            .iter()
            .map(|(k, v)| {
                let ks = match k {
                    LuaValue::Number(d) => format!("n:{}", java::double_to_string(*d)),
                    LuaValue::String(s) => format!("s:{}", esc(Some(s))),
                    LuaValue::Boolean(b) => format!("s:{b}"),
                    LuaValue::Table(_) => "s:table".into(),
                };
                let vs = match v {
                    LuaValue::String(s) => esc(Some(s)),
                    LuaValue::Number(d) => java::double_to_string(*d),
                    LuaValue::Boolean(b) => b.to_string(),
                    LuaValue::Table(t) => table(t),
                };
                format!("{ks}={vs}")
            })
            .collect();
        l.sort();
        format!("{{{}}}", l.join(", "))
    }
    fn line(sb: &mut String, path: &str, field: &str, value: &str) {
        let _ = writeln!(sb, "{path}\t{field}\t{value}");
    }
    /// VehiclesProbe.dump of a list of objects: "[]" when empty, else "size N" and one block per element.
    fn objs<T>(
        sb: &mut String,
        path: &str,
        field: &str,
        list: &[T],
        each: fn(&mut String, &str, &T),
    ) {
        if list.is_empty() {
            line(sb, path, field, "[]");
            return;
        }
        line(sb, path, field, &format!("size {}", list.len()));
        for (i, x) in list.iter().enumerate() {
            each(sb, &format!("{path}\t{field}[{i}]"), x);
        }
    }
    fn oobjs<T>(
        sb: &mut String,
        path: &str,
        field: &str,
        list: &Option<Vec<T>>,
        each: fn(&mut String, &str, &T),
    ) {
        match list {
            None => line(sb, path, field, "null"),
            Some(l) => objs(sb, path, field, l, each),
        }
    }
    fn model(sb: &mut String, p: &str, m: &Model) {
        line(sb, p, "id", &o(&m.id));
        line(sb, p, "file", &o(&m.file));
        line(sb, p, "scale", &f(m.scale));
        line(sb, p, "offset", &v3(&m.offset));
        line(sb, p, "rotate", &v3(&m.rotate));
        line(sb, p, "attachmentNameParent", &o(&m.attachment_name_parent));
        line(sb, p, "attachmentNameSelf", &o(&m.attachment_name_self));
        line(
            sb,
            p,
            "ignoreVehicleScale",
            &m.ignore_vehicle_scale.to_string(),
        );
    }
    fn anim(sb: &mut String, p: &str, a: &Anim) {
        line(sb, p, "id", &o(&a.id));
        line(sb, p, "anim", &o(&a.anim));
        line(sb, p, "rate", &f(a.rate));
        line(sb, p, "animate", &a.animate.to_string());
        line(sb, p, "loop", &a.r#loop.to_string());
        line(sb, p, "reverse", &a.reverse.to_string());
        line(sb, p, "offset", &v3(&a.offset));
        line(sb, p, "angle", &v3(&a.angle));
        line(sb, p, "sound", &o(&a.sound));
    }
    fn switch_seat(sb: &mut String, p: &str, s: &SwitchSeat) {
        line(sb, p, "id", &o(&s.id));
        line(sb, p, "seat", &s.seat.to_string());
        line(sb, p, "anim", &o(&s.anim));
        line(sb, p, "rate", &f(s.rate));
        line(sb, p, "sound", &o(&s.sound));
    }
    fn position(sb: &mut String, p: &str, x: &Position) {
        line(sb, p, "id", &o(&x.id));
        line(sb, p, "offset", &v3(&x.offset));
        line(sb, p, "rotate", &v3(&x.rotate));
        line(sb, p, "area", &o(&x.area));
    }
    fn passenger(sb: &mut String, p: &str, x: &Passenger) {
        line(sb, p, "id", &o(&x.id));
        objs(sb, p, "anims", &x.anims, anim);
        objs(sb, p, "switchSeats", &x.switch_seats, switch_seat);
        line(sb, p, "hasRoof", &x.has_roof.to_string());
        line(sb, p, "showPassenger", &x.show_passenger.to_string());
        line(sb, p, "door", &o(&x.door));
        line(sb, p, "door2", &o(&x.door2));
        line(sb, p, "area", &o(&x.area));
        objs(sb, p, "positions", &x.positions, position);
    }
    fn physics(sb: &mut String, p: &str, x: &PhysicsShape) {
        line(sb, p, "type", &x.r#type.to_string());
        line(sb, p, "offset", &v3(&x.offset));
        line(sb, p, "rotate", &v3(&x.rotate));
        line(sb, p, "extents", &v3(&x.extents));
        line(sb, p, "radius", &f(x.radius));
        line(sb, p, "physicsShapeScript", &o(&x.physics_shape_script));
    }
    fn wheel(sb: &mut String, p: &str, x: &Wheel) {
        line(sb, p, "id", &o(&x.id));
        line(sb, p, "model", &o(&x.model));
        line(sb, p, "front", &x.front.to_string());
        line(sb, p, "offset", &v3(&x.offset));
        line(sb, p, "radius", &f(x.radius));
        line(sb, p, "width", &f(x.width));
    }
    fn skin(sb: &mut String, p: &str, x: &Skin) {
        line(sb, p, "texture", &o(&x.texture));
        line(sb, p, "textureRust", &o(&x.texture_rust));
        line(sb, p, "textureMask", &o(&x.texture_mask));
        line(sb, p, "textureLights", &o(&x.texture_lights));
        line(
            sb,
            p,
            "textureDamage1Overlay",
            &o(&x.texture_damage1_overlay),
        );
        line(sb, p, "textureDamage1Shell", &o(&x.texture_damage1_shell));
        line(
            sb,
            p,
            "textureDamage2Overlay",
            &o(&x.texture_damage2_overlay),
        );
        line(sb, p, "textureDamage2Shell", &o(&x.texture_damage2_shell));
        line(sb, p, "textureShadow", &o(&x.texture_shadow));
        // The Texture objects, never loaded on a server.
        for t in [
            "textureData",
            "textureDataRust",
            "textureDataMask",
            "textureDataLights",
            "textureDataDamage1Overlay",
            "textureDataDamage1Shell",
            "textureDataDamage2Overlay",
            "textureDataDamage2Shell",
            "textureDataShadow",
        ] {
            line(sb, p, t, "null");
        }
    }
    fn area(sb: &mut String, p: &str, x: &Area) {
        line(sb, p, "id", &o(&x.id));
        line(sb, p, "x", &f(x.x));
        line(sb, p, "y", &f(x.y));
        line(sb, p, "w", &f(x.w));
        line(sb, p, "h", &f(x.h));
    }
    fn part(sb: &mut String, p: &str, x: &Part) {
        line(sb, p, "id", &o(&x.id));
        line(sb, p, "parent", &o(&x.parent));
        line(sb, p, "itemType", &ostrs(&x.item_type));
        match &x.container {
            None => line(sb, p, "container", "null"),
            Some(c) => {
                line(sb, p, "container", "object");
                let q = format!("{p}\tcontainer");
                line(sb, &q, "capacity", &c.capacity.to_string());
                line(sb, &q, "seat", &c.seat.to_string());
                line(sb, &q, "seatId", &o(&c.seat_id));
                line(sb, &q, "luaTest", &o(&c.lua_test));
                line(sb, &q, "contentType", &o(&c.content_type));
                line(
                    sb,
                    &q,
                    "conditionAffectsCapacity",
                    &c.condition_affects_capacity.to_string(),
                );
                line(sb, &q, "soundMap", &map(&c.sound_map));
            }
        }
        line(sb, p, "area", &o(&x.area));
        line(sb, p, "mechanicArea", &o(&x.mechanic_area));
        line(sb, p, "wheel", &o(&x.wheel));
        match &x.tables {
            None => line(sb, p, "tables", "null"),
            Some(t) if t.is_empty() => line(sb, p, "tables", "{}"),
            Some(t) => {
                line(sb, p, "tables", &format!("size {}", t.len()));
                let mut keys: Vec<&(Option<String>, KahluaTable)> = t.iter().collect();
                keys.sort_by(|a, b| {
                    java::string_compare_to(
                        a.0.as_deref().unwrap_or(""),
                        b.0.as_deref().unwrap_or(""),
                    )
                });
                for (k, v) in keys {
                    let _ = writeln!(sb, "{p}\ttables[{}]\t{}", o(k), table(v));
                }
            }
        }
        line(
            sb,
            p,
            "luaFunctions",
            &x.lua_functions.as_ref().map_or("null".into(), |m| map(m)),
        );
        oobjs(sb, p, "models", &x.models, model);
        line(
            sb,
            p,
            "setAllModelsVisible",
            &x.set_all_models_visible.to_string(),
        );
        line(
            sb,
            p,
            "door",
            if x.door.is_some() { "object" } else { "null" },
        );
        match &x.window {
            None => line(sb, p, "window", "null"),
            Some(w) => {
                line(sb, p, "window", "object");
                line(
                    sb,
                    &format!("{p}\twindow"),
                    "openable",
                    &w.openable.to_string(),
                );
            }
        }
        oobjs(sb, p, "anims", &x.anims, anim);
        line(sb, p, "category", &o(&x.category));
        line(sb, p, "specificItem", &x.specific_item.to_string());
        line(
            sb,
            p,
            "mechanicRequireKey",
            &x.mechanic_require_key.to_string(),
        );
        line(sb, p, "repairMechanic", &x.repair_mechanic.to_string());
        line(sb, p, "hasLightsRear", &x.has_lights_rear.to_string());
        line(sb, p, "durability", &f(x.durability));
    }
    fn base(sb: &mut String, p: &str, s: &VehicleScript) {
        let _ = writeln!(
            sb,
            "{p}\tmodule\t{}\tenabled\t{}\tdebugOnly\t{}\tscriptVersion\t{:x}\tbodies\t{}",
            esc(Some(&s.module)),
            s.enabled,
            s.debug_only,
            s.script_version as u64,
            s.loaded_script_bodies.len() * 2
        );
    }
    fn dump(sb: &mut String, p: &str, s: &VehicleScript) {
        line(sb, p, "fileName", &o(&s.file_name));
        line(sb, p, "name", &o(&s.name));
        objs(sb, p, "models", &s.models, model);
        if s.attachments.is_empty() {
            line(sb, p, "attachments", "[]");
        } else {
            line(
                sb,
                p,
                "attachments",
                &format!("size {}", s.attachments.len()),
            );
            for (i, a) in s.attachments.iter().enumerate() {
                let q = format!("{p}\tattachments[{i}]");
                line(sb, &q, "owner", if a.owned { "self" } else { "null" });
                line(sb, &q, "id", &esc(Some(&a.id)));
                line(sb, &q, "offset", &v3(&a.offset));
                line(sb, &q, "rotate", &v3(&a.rotate));
                line(sb, &q, "scale", &f(a.scale));
                line(sb, &q, "bone", &o(&a.bone));
                line(sb, &q, "canAttach", &ostrs(&a.can_attach));
                line(sb, &q, "zoffset", &f(a.zoffset));
                line(sb, &q, "updateConstraint", &a.update_constraint.to_string());
            }
        }
        line(sb, p, "mass", &f(s.mass));
        line(sb, p, "centerOfMassOffset", &v3(&s.center_of_mass_offset));
        for (k, v) in [
            ("engineForce", s.engine_force),
            ("engineIdleSpeed", s.engine_idle_speed),
            ("steeringIncrement", s.steering_increment),
            ("steeringClamp", s.steering_clamp),
            ("steeringClampMax", s.steering_clamp_max),
            ("wheelFriction", s.wheel_friction),
            ("stoppingMovementForce", s.stopping_movement_force),
            ("animalTrailerSize", s.animal_trailer_size),
            ("suspensionStiffness", s.suspension_stiffness),
            ("suspensionDamping", s.suspension_damping),
            ("suspensionCompression", s.suspension_compression),
            ("suspensionRestLength", s.suspension_rest_length),
            ("maxSuspensionTravelCm", s.max_suspension_travel_cm),
            ("rollInfluence", s.roll_influence),
        ] {
            line(sb, p, k, &f(v));
        }
        line(sb, p, "extents", &v3(&s.extents));
        line(sb, p, "shadowExtents", &v2(&s.shadow_extents));
        line(sb, p, "shadowOffset", &v2(&s.shadow_offset));
        line(
            sb,
            p,
            "hadShadowOExtents",
            &s.had_shadow_o_extents.to_string(),
        );
        line(sb, p, "hadShadowOffset", &s.had_shadow_offset.to_string());
        line(sb, p, "extentsOffset", &v2(&s.extents_offset));
        line(sb, p, "physicsChassisShape", &v3(&s.physics_chassis_shape));
        objs(sb, p, "physicsShapes", &s.physics_shapes, physics);
        objs(sb, p, "wheels", &s.wheels, wheel);
        objs(sb, p, "crawlThroughWheels", &s.crawl_through_wheels, wheel);
        objs(sb, p, "passengers", &s.passengers, passenger);
        line(sb, p, "maxSpeed", &f(s.max_speed));
        line(
            sb,
            p,
            "useChassisPhysicsCollision",
            &s.use_chassis_physics_collision.to_string(),
        );
        line(sb, p, "maxSpeedReverse", &f(s.max_speed_reverse));
        line(sb, p, "isSmallVehicle", &s.is_small_vehicle.to_string());
        for (k, v) in [
            ("frontEndHealth", s.front_end_health),
            ("rearEndHealth", s.rear_end_health),
            ("storageCapacity", s.storage_capacity),
            ("engineLoudness", s.engine_loudness),
            ("engineQuality", s.engine_quality),
            ("seats", s.seats),
            ("mechanicType", s.mechanic_type),
            ("engineRepairLevel", s.engine_repair_level),
        ] {
            line(sb, p, k, &v.to_string());
        }
        for (k, v) in [
            ("playerDamageProtection", s.player_damage_protection),
            ("forcedHue", s.forced_hue),
            ("forcedSat", s.forced_sat),
            ("forcedVal", s.forced_val),
        ] {
            line(sb, p, k, &f(v));
        }
        line(sb, p, "leftSirenCol", &color(&s.left_siren_col));
        line(sb, p, "rightSirenCol", &color(&s.right_siren_col));
        line(sb, p, "engineRpmType", &esc(Some(&s.engine_rpm_type)));
        line(sb, p, "offroadEfficiency", &f(s.offroad_efficiency));
        line(sb, p, "crawlOffsets", &floats(&s.crawl_offsets));
        line(sb, p, "zombieType", &ostrs(&s.zombie_type));
        line(sb, p, "specialKeyRing", &ostrs(&s.special_key_ring));
        line(sb, p, "notKillCrops", &s.not_kill_crops.to_string());
        line(sb, p, "hasLighter", &s.has_lighter.to_string());
        line(sb, p, "carMechanicsOverlay", &o(&s.car_mechanics_overlay));
        line(sb, p, "carModelName", &o(&s.car_model_name));
        line(
            sb,
            p,
            "specialLootChance",
            &s.special_loot_chance.to_string(),
        );
        line(
            sb,
            p,
            "specialKeyRingChance",
            &s.special_key_ring_chance.to_string(),
        );
        line(sb, p, "neverSpawnKey", &s.never_spawn_key.to_string());
        line(sb, p, "gearRatioCount", &s.gear_ratio_count.to_string());
        line(sb, p, "gearRatio", &floats(&s.gear_ratio));
        line(sb, p, "textures", "object");
        skin(sb, &format!("{p}\ttextures"), &s.textures);
        objs(sb, p, "skins", &s.skins, skin);
        objs(sb, p, "areas", &s.areas, area);
        objs(sb, p, "parts", &s.parts, part);
        line(sb, p, "hasSiren", &s.has_siren.to_string());
        line(sb, p, "lightbar", "object");
        let q = format!("{p}\tlightbar");
        line(sb, &q, "enable", &s.lightbar.enable.to_string());
        line(sb, &q, "soundSiren0", &esc(Some(&s.lightbar.sound_siren0)));
        line(sb, &q, "soundSiren1", &esc(Some(&s.lightbar.sound_siren1)));
        line(sb, &q, "soundSiren2", &esc(Some(&s.lightbar.sound_siren2)));
        line(sb, p, "sound", "object");
        let q = format!("{p}\tsound");
        let so = &s.sound;
        line(sb, &q, "alarmEnable", &so.alarm_enable.to_string());
        line(sb, &q, "alarm", &strs(&so.alarm));
        line(sb, &q, "alarmLoop", &strs(&so.alarm_loop));
        line(sb, &q, "hornEnable", &so.horn_enable.to_string());
        line(sb, &q, "horn", &o(&so.horn));
        line(
            sb,
            &q,
            "backSignalEnable",
            &so.back_signal_enable.to_string(),
        );
        line(sb, &q, "backSignal", &o(&so.back_signal));
        line(sb, &q, "engine", &o(&so.engine));
        line(sb, &q, "engineStart", &o(&so.engine_start));
        line(sb, &q, "engineTurnOff", &o(&so.engine_turn_off));
        line(sb, &q, "handBrake", &o(&so.hand_brake));
        line(sb, &q, "ignitionFail", &o(&so.ignition_fail));
        line(
            sb,
            &q,
            "ignitionFailNoPower",
            &o(&so.ignition_fail_no_power),
        );
        line(sb, &q, "specified", &set(&so.specified));
        line(
            sb,
            p,
            "textureMaskEnable",
            &s.texture_mask_enable.to_string(),
        );
    }

    let m = ScriptManager::load(root)?;
    for (n, e) in &m.vehicle_errors {
        eprintln!("dropped {n}: {e}");
    }
    for l in &m.vehicle_log {
        eprintln!("log: {l}");
    }
    let mut sb = String::new();
    let mut vehicles: Vec<&VehicleScript> = m.vehicles.iter().collect();
    vehicles.sort_by(|a, b| java::string_compare_to(&a.get_full_name(), &b.get_full_name()));
    for v in vehicles {
        let p = esc(Some(&v.get_full_name()));
        base(&mut sb, &p, v);
        dump(&mut sb, &p, v);
    }
    let mut templates: Vec<_> = m.vehicle_templates.iter().collect();
    templates.sort_by(|a, b| {
        java::string_compare_to(
            &a.get_script_object_full_type(),
            &b.get_script_object_full_type(),
        )
    });
    for t in templates {
        let p = format!("template:{}", esc(Some(&t.get_script_object_full_type())));
        let _ = writeln!(
            sb,
            "{p}\tname\t{}\tbody\t{}\t{}\tscript\t{}",
            esc(Some(&t.name)),
            t.body.encode_utf16().count(),
            java::string_hash_code(&t.body),
            if t.script.is_some() { "built" } else { "null" }
        );
        if let Some(s) = &t.script {
            base(&mut sb, &p, s);
            dump(&mut sb, &p, s);
        }
    }
    Ok(sb)
}

/// The XUI scripts and XuiManager.ParseScripts' state, pairs with XuiProbe.java: the six XUI script lists sorted by
/// full type, XuiLuaStyle's statics (the var registry), XuiManager's statics with every skin, and the colours of
/// zombie.core.Colors by name.
pub fn xui(root: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::core::color::Color;
    use zomboid_engine::zombie::scripting::objects::base_script_object::BaseScriptObject;
    use zomboid_engine::zombie::scripting::objects::xui_colors_script::XuiColorsScript;
    use zomboid_engine::zombie::scripting::objects::xui_config_script::XuiConfigScript;
    use zomboid_engine::zombie::scripting::objects::xui_layout_script::XuiLayoutScript;
    use zomboid_engine::zombie::scripting::objects::xui_skin_script::{
        ComponentUiScript, EntityUiScript, StyleInfoScript, XuiSkinScript,
    };
    use zomboid_engine::zombie::scripting::script_manager::ScriptManager;
    use zomboid_engine::zombie::scripting::ui::xui_lua_style::{
        XuiLuaStyle, XuiValue, XuiVar, ALLOWED_VAR_TYPES,
    };
    use zomboid_engine::zombie::scripting::ui::xui_manager::{DEFAULT_SKIN_NAME, XUI_SCRIPT_TYPES};
    use zomboid_engine::zombie::scripting::ui::xui_skin::{
        ComponentUiStyle, EntityUiStyle, StyleInfo, XuiSkin,
    };
    use zomboid_engine::zombie::scripting::ui::JHashMap;
    /// XuiProbe.esc: '\\', tab, LF, CR and every UTF-16 unit outside 0x20..0x7e escaped.
    fn esc(s: Option<&str>) -> String {
        let Some(s) = s else {
            return "null".into();
        };
        let mut sb = String::new();
        for u in s.encode_utf16() {
            match u {
                0x5c => sb.push_str("\\\\"),
                0x09 => sb.push_str("\\t"),
                0x0a => sb.push_str("\\n"),
                0x0d => sb.push_str("\\r"),
                0x20..=0x7e => sb.push(char::from(u as u8)),
                _ => {
                    let _ = write!(sb, "\\u{u:04x}");
                }
            }
        }
        sb
    }
    fn e(s: &str) -> String {
        esc(Some(s))
    }
    fn o(s: &Option<String>) -> String {
        esc(s.as_deref())
    }
    fn rgba(c: &Color) -> String {
        format!(
            "rgba({}, {}, {}, {})",
            java::float_to_string(c.r),
            java::float_to_string(c.g),
            java::float_to_string(c.b),
            java::float_to_string(c.a)
        )
    }
    fn line(sb: &mut String, p: &str, f: &str, v: &str) {
        let _ = writeln!(sb, "{p}\t{f}\t{v}");
    }
    fn head(sb: &mut String, p: &str, n: usize) {
        let _ = writeln!(sb, "{p}\tsize {n}");
    }
    /// XuiProbe.val of a List.
    fn list(v: &[String]) -> String {
        let l: Vec<String> = v.iter().map(|s| e(s)).collect();
        format!("[{}]", l.join(", "))
    }
    /// XuiProbe.val of a Set: sorted.
    fn set(mut v: Vec<String>) -> String {
        v.sort_by(|a, b| java::string_compare_to(a, b));
        format!("[{}]", v.join(", "))
    }
    fn texture(t: &Option<String>) -> String {
        t.as_ref()
            .map_or("null".into(), |n| format!("texture:{}", e(n)))
    }
    fn base(sb: &mut String, p: &str, b: &BaseScriptObject) {
        let _ = writeln!(
            sb,
            "{p}\tmodule\t{}\tenabled\t{}\tdebugOnly\t{}\tscriptVersion\t{:x}\tbodies\t{}",
            o(&b.module),
            b.enabled,
            b.debug_only,
            b.script_version as u64,
            b.get_loaded_script_body_count()
        );
    }
    fn value(v: Option<&XuiValue>) -> String {
        match v {
            None => "null".into(),
            Some(XuiValue::Boolean(b)) => b.to_string(),
            Some(XuiValue::Double(d)) => java::double_to_string(*d),
            Some(XuiValue::Font(f)) => f.name().to_string(),
            Some(XuiValue::Str(s)) => e(s),
            Some(XuiValue::StrList(l)) => list(l),
            Some(XuiValue::Color(c)) => rgba(c),
        }
    }
    fn str_map(sb: &mut String, p: &str, m: &JHashMap<Option<String>>) {
        head(sb, p, m.len());
        for (k, v) in m.sorted() {
            let _ = writeln!(sb, "{p}[{}]\t=\t{}", e(k), o(v));
        }
    }
    fn color_map(sb: &mut String, p: &str, m: &JHashMap<Color>) {
        head(sb, p, m.len());
        for (k, v) in m.sorted() {
            let _ = writeln!(sb, "{p}[{}]\t=\t{}", e(k), rgba(v));
        }
    }
    fn var(sb: &mut String, p: &str, v: &XuiVar) {
        line(sb, p, "class", v.class.simple_name());
        line(sb, p, "uiOrder", &v.ui_order.to_string());
        line(sb, p, "type", v.r#type.name());
        line(sb, p, "parent", if v.has_parent { "owner" } else { "null" });
        line(sb, p, "valueSet", &v.value_set.to_string());
        line(sb, p, "autoApply", v.auto_apply.name());
        line(sb, p, "defaultValue", &value(v.default_value.as_ref()));
        line(sb, p, "value", &value(v.value.as_ref()));
        line(sb, p, "luaTableKey", &e(&v.lua_table_key));
    }
    fn style(sb: &mut String, p: &str, s: &XuiLuaStyle) {
        line(sb, p, "xuiLuaClass", &e(&s.xui_lua_class));
        line(sb, p, "xuiStyleName", &e(&s.xui_style_name));
        let skin = s
            .xui_skin
            .as_ref()
            .map_or("null".into(), |n| format!("skin:{}", e(n)));
        line(sb, p, "xuiSkin", &skin);
        let pm = format!("{p}\tvarsMap");
        head(sb, &pm, s.vars_map.len());
        for (k, i) in s.vars_map.sorted() {
            let _ = writeln!(sb, "{pm}[{}]\t=\t#{i}", e(k));
        }
        if s.vars.is_empty() {
            line(sb, p, "vars", "[]");
        } else {
            line(sb, p, "vars", &format!("size {}", s.vars.len()));
            for (i, v) in s.vars.iter().enumerate() {
                var(sb, &format!("{p}\tvars[{i}]"), v);
            }
        }
    }
    fn component_script(sb: &mut String, p: &str, c: &ComponentUiScript) {
        line(sb, p, "luaPanelClass", &o(&c.lua_panel_class));
        line(sb, p, "xuiStyle", &o(&c.xui_style));
        line(sb, p, "displayName", &o(&c.display_name));
        line(sb, p, "iconPath", &o(&c.icon_path));
        line(sb, p, "listOrderZset", &c.list_order_zset.to_string());
        line(sb, p, "listOrderZ", &c.list_order_z.to_string());
        line(sb, p, "enabledSet", &c.enabled_set.to_string());
        line(sb, p, "enabled", &c.enabled.to_string());
    }
    fn entity_script(sb: &mut String, p: &str, u: &EntityUiScript) {
        line(sb, p, "luaWindowClass", &o(&u.lua_window_class));
        line(sb, p, "xuiStyle", &o(&u.xui_style));
        line(sb, p, "luaCanOpenWindow", &o(&u.lua_can_open_window));
        line(sb, p, "luaOpenWindow", &o(&u.lua_open_window));
        line(sb, p, "displayName", &o(&u.display_name));
        line(sb, p, "description", &o(&u.description));
        line(sb, p, "buildDescription", &o(&u.build_description));
        line(sb, p, "iconPath", &o(&u.icon_path));
        line(sb, p, "clearComponents", &u.clear_components.to_string());
        let pm = format!("{p}\tcomponentUiScriptMap");
        head(sb, &pm, u.component_ui_script_map.len());
        let mut m: Vec<_> = u.component_ui_script_map.iter().collect();
        m.sort_by(|a, b| java::string_compare_to(a.0.name(), b.0.name()));
        for (t, c) in m {
            component_script(sb, &format!("{pm}[{}]", t.name()), c);
        }
    }
    fn style_info_script(sb: &mut String, p: &str, s: &StyleInfoScript) {
        str_map(
            sb,
            &format!("{p}\tdefaultStyleBlock"),
            &s.default_style_block,
        );
        let pm = format!("{p}\tstyleBlocks");
        head(sb, &pm, s.style_blocks.len());
        for (k, m) in s.style_blocks.sorted() {
            str_map(sb, &format!("{pm}[{}]", e(k)), m);
        }
    }
    fn colors_script(sb: &mut String, p: &str, c: &XuiColorsScript) {
        base(sb, p, &c.base);
        line(sb, p, "name", &o(&c.name));
        color_map(sb, &format!("{p}\tcolorMap"), &c.color_map);
    }
    fn skin_script(sb: &mut String, p: &str, s: &XuiSkinScript) {
        base(sb, p, &s.base);
        line(sb, p, "imports", &list(&s.imports));
        line(sb, p, "defaultEntityUiScript", "object");
        entity_script(
            sb,
            &format!("{p}\tdefaultEntityUiScript"),
            &s.default_entity_ui_script,
        );
        let pm = format!("{p}\tentityUiScriptMap");
        head(sb, &pm, s.entity_ui_script_map.len());
        for (k, u) in s.entity_ui_script_map.sorted() {
            entity_script(sb, &format!("{pm}[{}]", e(k)), u);
        }
        let pm = format!("{p}\tstyleInfoMap");
        head(sb, &pm, s.style_info_map.len());
        for (k, i) in s.style_info_map.sorted() {
            style_info_script(sb, &format!("{pm}[{}]", e(k)), i);
        }
        line(sb, p, "colorsScript", "object");
        colors_script(sb, &format!("{p}\tcolorsScript"), &s.colors_script);
    }
    fn config(sb: &mut String, p: &str, c: &XuiConfigScript) {
        base(sb, p, &c.base);
        let pm = format!("{p}\tvarConfigs");
        head(sb, &pm, c.var_configs.len());
        let mut m: Vec<_> = c.var_configs.iter().collect();
        m.sort_by(|a, b| java::string_compare_to(a.0.name(), b.0.name()));
        for (t, keys) in m {
            let _ = writeln!(sb, "{pm}[{}]\t=\t{}", t.name(), list(keys));
        }
    }
    fn layout(sb: &mut String, p: &str, l: &XuiLayoutScript) {
        base(sb, p, &l.base);
        let x = l
            .xui_script
            .as_ref()
            .map_or("null".into(), |n| format!("xuiscript:{}", e(n)));
        line(sb, p, "xuiScript", &x);
        line(sb, p, "name", &o(&l.name));
        let total = l.total_file.as_ref().map_or("null".into(), |t| {
            format!(
                "{}\t{}",
                t.encode_utf16().count(),
                java::string_hash_code(t)
            )
        });
        line(sb, p, "totalFile", &total);
        line(sb, p, "hasParsed", &l.has_parsed.to_string());
        line(sb, p, "scriptType", l.script_type.name());
        line(sb, p, "block", if l.block { "set" } else { "null" });
    }
    fn component_style(sb: &mut String, p: &str, c: &ComponentUiStyle) {
        line(sb, p, "luaPanelClass", &o(&c.lua_panel_class));
        line(sb, p, "xuiStyle", &o(&c.xui_style));
        line(sb, p, "displayName", &o(&c.display_name));
        line(sb, p, "icon", &texture(&c.icon));
        line(sb, p, "listOrderZ", &c.list_order_z.to_string());
        line(sb, p, "enabled", &c.enabled.to_string());
    }
    fn entity_style(sb: &mut String, p: &str, u: &EntityUiStyle) {
        line(sb, p, "luaWindowClass", &o(&u.lua_window_class));
        line(sb, p, "xuiStyle", &o(&u.xui_style));
        line(sb, p, "luaCanOpenWindow", &o(&u.lua_can_open_window));
        line(sb, p, "luaOpenWindow", &o(&u.lua_open_window));
        let pm = format!("{p}\tcomponentUiStyleMap");
        head(sb, &pm, u.component_ui_style_map.len());
        let mut m: Vec<_> = u.component_ui_style_map.iter().collect();
        m.sort_by(|a, b| java::string_compare_to(a.0.name(), b.0.name()));
        for (t, c) in m {
            component_style(sb, &format!("{pm}[{}]", t.name()), c);
        }
        line(sb, p, "displayName", &o(&u.display_name));
        line(sb, p, "description", &o(&u.description));
        line(sb, p, "buildDescription", &o(&u.build_description));
        line(sb, p, "icon", &texture(&u.icon));
    }
    fn style_info(sb: &mut String, p: &str, s: &StyleInfo) {
        match &s.default_style {
            None => line(sb, p, "defaultStyle", "null"),
            Some(d) => {
                line(sb, p, "defaultStyle", "object");
                style(sb, &format!("{p}\tdefaultStyle"), d);
            }
        }
        let pm = format!("{p}\tstyles");
        head(sb, &pm, s.styles.len());
        for (k, st) in s.styles.sorted() {
            style(sb, &format!("{pm}[{}]", e(k)), st);
        }
    }
    fn skin(sb: &mut String, p: &str, s: &XuiSkin, scripts: &[XuiSkinScript]) {
        let imports: Vec<String> = s.imports.iter().map(|n| format!("skin:{}", e(n))).collect();
        line(sb, p, "imports", &format!("[{}]", imports.join(", ")));
        color_map(sb, &format!("{p}\tcolorMap"), &s.color_map);
        line(sb, p, "defaultEntityUiStyle", "object");
        entity_style(
            sb,
            &format!("{p}\tdefaultEntityUiStyle"),
            &s.default_entity_ui_style,
        );
        let pm = format!("{p}\tentityUiStyleMap");
        head(sb, &pm, s.entity_ui_style_map.len());
        for (k, u) in s.entity_ui_style_map.sorted() {
            entity_style(sb, &format!("{pm}[{}]", e(k)), u);
        }
        let pm = format!("{p}\tstyles");
        head(sb, &pm, s.styles.len());
        for (k, i) in s.styles.sorted() {
            style_info(sb, &format!("{pm}[{}]", e(k)), i);
        }
        line(sb, p, "name", &e(&s.name));
        let script = scripts.get(s.script).map_or("null".into(), |x| {
            format!("script:{}", e(&x.base.get_script_object_full_type()))
        });
        line(sb, p, "script", &script);
        line(sb, p, "hasLoaded", &s.has_loaded.to_string());
        line(sb, p, "invalidated", &s.invalidated.to_string());
    }
    fn sorted_by_full<T>(v: &[T], base: impl Fn(&T) -> &BaseScriptObject) -> Vec<(String, &T)> {
        let mut l: Vec<(String, &T)> = v
            .iter()
            .map(|x| (base(x).get_script_object_full_type(), x))
            .collect();
        l.sort_by(|a, b| java::string_compare_to(&a.0, &b.0));
        l
    }
    let m = ScriptManager::load(root)?;
    let mut sb = String::new();
    let l = sorted_by_full(&m.xui_configs, |x| &x.base);
    head(&mut sb, "xuiConfig", l.len());
    for (full, x) in l {
        config(&mut sb, &format!("xuiConfig:{}", e(&full)), x);
    }
    for (tag, list) in [
        ("xuiLayout", &m.xui_layouts),
        ("xuiStyle", &m.xui_styles),
        ("xuiDefaultStyle", &m.xui_default_styles),
    ] {
        let l = sorted_by_full(list, |x| &x.base);
        head(&mut sb, tag, l.len());
        for (full, x) in l {
            layout(&mut sb, &format!("{tag}:{}", e(&full)), x);
        }
    }
    let l = sorted_by_full(&m.xui_colors, |x| &x.base);
    head(&mut sb, "xuiGlobalColors", l.len());
    for (full, x) in l {
        colors_script(&mut sb, &format!("xuiGlobalColors:{}", e(&full)), x);
    }
    let l = sorted_by_full(&m.xui_skin_scripts, |x| &x.base);
    head(&mut sb, "xuiSkin", l.len());
    for (full, x) in l {
        skin_script(&mut sb, &format!("xuiSkin:{}", e(&full)), x);
    }
    // XuiLuaStyle's static fields
    let x = &m.xui_manager;
    line(
        &mut sb,
        "XuiLuaStyle",
        "ALLOWED_VAR_TYPES",
        &set(ALLOWED_VAR_TYPES
            .iter()
            .map(|t| t.name().to_string())
            .collect()),
    );
    head(
        &mut sb,
        "XuiLuaStyle\tvarRegisteryMap",
        x.var_registery_map.len(),
    );
    for (k, v) in x.var_registery_map.sorted() {
        var(
            &mut sb,
            &format!("XuiLuaStyle\tvarRegisteryMap[{}]", e(k)),
            v,
        );
    }
    // XuiManager's static fields
    let p = "XuiManager";
    line(&mut sb, p, "DEFAULT_SKIN_NAME", DEFAULT_SKIN_NAME);
    line(
        &mut sb,
        p,
        "XUI_SCRIPT_TYPES",
        &set(XUI_SCRIPT_TYPES.iter().map(|t| t.to_string()).collect()),
    );
    for (f, map, list) in [
        ("layoutScriptsMap", &x.layout_scripts_map, &m.xui_layouts),
        ("stylesScriptsMap", &x.styles_scripts_map, &m.xui_styles),
        (
            "defaultStylesScriptsMap",
            &x.default_styles_scripts_map,
            &m.xui_default_styles,
        ),
    ] {
        let pm = format!("{p}\t{f}");
        head(&mut sb, &pm, map.len());
        for (k, i) in map.sorted() {
            let full = list.get(*i).map_or("null".into(), |s| {
                format!("script:{}", e(&s.base.get_script_object_full_type()))
            });
            let _ = writeln!(sb, "{pm}[{}]\t=\t{full}", e(k));
        }
    }
    for (f, names) in [
        ("combinedList", &x.combined_list),
        ("xuiLayoutsList", &x.xui_layouts_list),
        ("xuiStylesList", &x.xui_styles_list),
        ("xuiDefaultStylesList", &x.xui_default_styles_list),
    ] {
        let l: Vec<String> = names
            .iter()
            .map(|n| format!("xuiscript:{}", e(n)))
            .collect();
        line(&mut sb, p, f, &format!("[{}]", l.join(", ")));
    }
    for (f, map) in [
        ("xuiLayouts", &x.xui_layouts),
        ("xuiStyles", &x.xui_styles),
        ("xuiDefaultStyles", &x.xui_default_styles),
    ] {
        let pm = format!("{p}\t{f}");
        head(&mut sb, &pm, map.len());
        for (k, n) in map.sorted() {
            let _ = writeln!(sb, "{pm}[{}]\t=\txuiscript:{}", e(k), e(n));
        }
    }
    head(&mut sb, &format!("{p}\txuiSkins"), x.xui_skins.len());
    for (k, s) in x.xui_skins.sorted() {
        skin(&mut sb, &format!("skin:{}", e(k)), s, &m.xui_skin_scripts);
    }
    let def = x
        .xui_default_skin
        .as_ref()
        .map_or("null".into(), |n| format!("skin:{}", e(n)));
    line(&mut sb, p, "xuiDefaultSkin", &def);
    line(&mut sb, p, "parseOnce", &x.parse_once.to_string());
    line(&mut sb, p, "hasParsedOnce", &x.has_parsed_once.to_string());
    let d = x.get_default_skin();
    let _ = writeln!(
        sb,
        "{p}\tGetDefaultSkin\t{}\tsame\t{}",
        d.map_or("null".into(), |s| format!("skin:{}", e(&s.name))),
        d.is_some_and(|s| x.get_skin(Some(&s.name)).is_some_and(|t| t.name == s.name))
    );
    let names = m.colors.get_color_names();
    let _ = writeln!(
        sb,
        "Colors\tsize {}\t{}",
        names.len(),
        m.colors.get_colors_count()
    );
    for (i, n) in names.iter().enumerate() {
        let c = m
            .colors
            .get_color_by_name(n)
            .map_or("null".into(), |c| rgba(&c));
        let set = m
            .colors
            .get_color_info(n)
            .map_or("null", |i| i.color_set.name());
        let _ = writeln!(sb, "Colors[{i}]\t{}\t{c}\t{set}", e(n));
    }
    let cb = m.colors.cb_get_color_names();
    let _ = writeln!(sb, "CB\tsize {}", cb.len());
    for (i, n) in cb.iter().enumerate() {
        let c = m
            .colors
            .cb_get_color_by_name(n)
            .map_or("null".into(), |c| rgba(&c));
        let _ = writeln!(sb, "CB[{i}]\t{}\t{c}", e(n));
    }
    Ok(sb)
}

/// zombie.scripting.ScriptParser over crafted cases, pairs with ScriptParserProbe.java: per case the stripComments
/// text, its parseTokens, and the tree parse() builds from the case text.
pub fn scriptparser(cases: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::scripting::script_parser::{self, Block, BlockElement};
    fn esc(s: Option<&str>) -> String {
        let Some(s) = s else {
            return "null".into();
        };
        let mut sb = String::new();
        for u in s.encode_utf16() {
            match u {
                0x5c => sb.push_str("\\\\"),
                0x09 => sb.push_str("\\t"),
                0x0a => sb.push_str("\\n"),
                0x0d => sb.push_str("\\r"),
                0x20..=0x7e => sb.push(char::from(u as u8)),
                _ => {
                    let _ = write!(sb, "\\u{u:04x}");
                }
            }
        }
        sb
    }
    fn tree(sb: &mut String, b: &Block, depth: usize) {
        // the root block's type is the original's null; a child's is never null
        let ty = if depth == 0 {
            None
        } else {
            Some(b.r#type.as_str())
        };
        let _ = writeln!(
            sb,
            "B\t{depth}\t{}\t{}\tvalues\t{}\tchildren\t{}",
            esc(ty),
            esc(b.id.as_deref()),
            b.values.len(),
            b.children.len()
        );
        for e in &b.elements {
            match e {
                BlockElement::Block(i) => tree(sb, &b.children[*i], depth + 1),
                BlockElement::Value(v) => {
                    let _ = writeln!(
                        sb,
                        "V\t{}\t{}\tkey\t{}\tvalue\t{}",
                        depth + 1,
                        esc(Some(&v.string)),
                        esc(Some(v.get_key())),
                        esc(Some(v.get_value()))
                    );
                }
            }
        }
    }
    let text = std::fs::read_to_string(cases).map_err(|e| format!("{}: {e}", cases.display()))?;
    // text.split("(^|\n)@@case\n", -1)
    let text = match text.strip_prefix("@@case\n") {
        Some(rest) => format!("\n@@case\n{rest}"),
        None => text,
    };
    let mut sb = String::new();
    for (n, c) in text.split("\n@@case\n").enumerate().skip(1) {
        let _ = writeln!(sb, "case\t{n}");
        let stripped = script_parser::strip_comments(c);
        let _ = writeln!(sb, "strip\t{}", esc(Some(&stripped)));
        for t in script_parser::parse_tokens(&stripped) {
            let _ = writeln!(sb, "token\t{}", esc(Some(&t)));
        }
        tree(&mut sb, &script_parser::parse(c), 0);
    }
    Ok(sb)
}

/// Every table of a save's WorldDictionary.bin; pairs with WorldDictionaryProbe.java.
pub fn worlddictionary(path: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::world::dictionary_data::{DictionaryData, DictionaryInfo};
    let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let d = DictionaryData::load_from_byte_buffer(&data)?;
    let info = |sb: &mut String, tag: &str, i: &DictionaryInfo| {
        let _ = writeln!(
            sb,
            "{tag}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            i.registry_id,
            i.full_type,
            i.module_name,
            i.name,
            i.mod_id,
            i.is_modded,
            i.exists_as_vanilla,
            i.obsolete,
            i.removed,
            i.mod_overrides
                .as_ref()
                .map_or("null".to_string(), |v| v.join(","))
        );
    };
    let mut sb = String::new();
    let _ = writeln!(
        sb,
        "N\t{}\t{}\t{}",
        d.next_info_id, d.next_object_name_id, d.next_sprite_name_id
    );
    for i in d.items.values() {
        info(&mut sb, "I", i);
    }
    let _ = writeln!(sb, "IT\t{}", d.item_types.len());
    for i in d.entities.values() {
        info(&mut sb, "E", i);
    }
    let _ = writeln!(sb, "ET\t{}", d.entity_types.len());
    for (k, v) in &d.object_names {
        let _ = writeln!(sb, "O\t{k}\t{v}");
    }
    let _ = writeln!(sb, "OT\t{}", d.object_ids.len());
    for (k, v) in &d.sprite_names {
        let _ = writeln!(sb, "S\t{k}\t{v}");
    }
    let _ = writeln!(sb, "ST\t{}", d.sprite_ids.len());
    for r in &d.string_registers {
        let _ = writeln!(sb, "R\t{}\t{}", r.name, r.next_id);
        for i in r.by_id.values() {
            let _ = writeln!(sb, "r\t{}\t{}\t{}", i.registry_id, i.string, i.is_loaded);
        }
    }
    for r in &d.script_registers {
        let _ = writeln!(sb, "C\t{}\t{}", r.name, r.next_id);
        for i in r.by_id.values() {
            let _ = writeln!(
                sb,
                "c\t{}\t{}\t{}\t{}",
                i.registry_id, i.version, i.name, i.is_loaded
            );
        }
    }
    Ok(sb)
}

mod save_dump {
    use std::fmt::Write as _;

    use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::{KahluaTable, LuaValue};
    use zomboid_engine::zombie::core::skinnedmodel::visual::human_visual::HumanVisual;
    use zomboid_engine::zombie::inventory::inventory_item::{InventoryItem, ItemKind, ItemType};
    use zomboid_engine::zombie::inventory::item_container::ItemContainer;
    use zomboid_engine::zombie::inventory::types::alarm_clock::AlarmClockSave;
    use zomboid_engine::zombie::inventory::types::clothing::ClothingSave;
    use zomboid_engine::zombie::inventory::types::moveable::MoveableSave;
    use zomboid_engine::zombie::iso::chunk_save::{ClassSave, ObjectSave};
    use zomboid_engine::zombie::iso::iso_chunk::IsoChunk;
    use zomboid_engine::zombie::iso::iso_grid_square::IsoGridSquare;
    use zomboid_engine::zombie::iso::iso_object::{IsoObject, ObjectKind};
    use zomboid_engine::zombie::iso::objects::iso_dead_body::IsoDeadBody;
    use zomboid_engine::zombie::iso::sprite::iso_sprite_manager::{IsoSpriteManager, SpriteRef};
    use zomboid_engine::zombie::world::dictionary_data::DictionaryData;
    use zomboid_engine::zombie::world_map::symbols::world_map_symbols::WorldMapSymbol;

    /// `Integer.toHexString(Float.floatToRawIntBits(f))`
    pub fn fh(f: f32) -> String {
        format!("{:x}", f.to_bits())
    }
    pub fn dh(d: f64) -> String {
        format!("{:x}", d.to_bits())
    }
    fn b(v: bool) -> char {
        if v {
            '1'
        } else {
            '0'
        }
    }
    fn s(v: Option<&str>) -> &str {
        v.unwrap_or("null")
    }
    /// `sp == null ? "-" : String.valueOf(sp.name)`
    fn sn(sm: &IsoSpriteManager, sp: Option<SpriteRef>) -> String {
        sp.map_or("-".to_string(), |r| {
            s(sm.sprite(r).name.as_deref()).to_string()
        })
    }
    fn lua(v: &LuaValue) -> String {
        match v {
            LuaValue::String(x) => format!("s:{x}"),
            LuaValue::Number(x) => format!("n:{}", dh(*x)),
            LuaValue::Boolean(x) => format!("b:{x}"),
            LuaValue::Table(t) => table(t),
        }
    }
    pub fn table(t: &KahluaTable) -> String {
        let mut l: Vec<String> = t
            .entries
            .iter()
            .map(|(k, v)| format!("{}={}", lua(k), lua(v)))
            .collect();
        l.sort();
        format!("{{{}}}", l.join(","))
    }
    /// `hasModData() ? lua(getModData()) : "-"`
    fn mod_data(t: Option<&KahluaTable>) -> String {
        t.filter(|t| !t.entries.is_empty())
            .map_or("-".to_string(), table)
    }
    fn color(c: Option<[i32; 3]>) -> String {
        c.map_or("null".to_string(), |c| {
            c.map(|v| fh(v as f32 / 255.0)).join(",")
        })
    }
    fn bytes(a: Option<&[i8]>) -> String {
        a.map_or("null".to_string(), |a| {
            a.iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        })
    }
    fn human_visual(sb: &mut String, v: &HumanVisual) {
        let white = Some([255, 255, 255]);
        let _ = writeln!(
            sb,
            "HV\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            color(v.skin_color.or(white)),
            v.skin_texture,
            s(v.skin_texture_name.as_deref()),
            v.zombie_rot_stage,
            color(v.hair_color),
            color(v.beard_color),
            color(v.natural_hair_color),
            color(v.natural_beard_color),
            s(v.hair_model.as_deref()),
            s(v.beard_model.as_deref()),
            v.body_hair,
            bytes(Some(&v.blood)),
            bytes(Some(&v.dirt)),
            bytes(Some(&v.holes)),
            s(v.non_attached_hair.as_deref())
        );
        for iv in &v.body_visuals {
            let _ = write!(
                sb,
                "IV\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                iv.full_type,
                iv.alternate_model_name,
                iv.clothing_item_name,
                color(iv.tint),
                iv.base_texture,
                iv.texture_choice,
                fh(iv.hue),
                s(iv.decal.as_deref())
            );
            for a in [
                &iv.blood,
                &iv.dirt,
                &iv.holes,
                &iv.basic_patches,
                &iv.denim_patches,
                &iv.leather_patches,
            ] {
                let _ = write!(sb, "\t{}", bytes(a.as_ref().map(|a| &a[..])));
            }
            sb.push('\n');
        }
    }
    fn container(sb: &mut String, dict: &DictionaryData, c: &ItemContainer) {
        let _ = write!(
            sb,
            "C\t{}\t{}\t{}\t{}",
            c.r#type,
            b(c.explored),
            b(c.has_been_looted),
            c.capacity
        );
        for it in &c.items {
            let _ = write!(sb, "\t{}:{}", it.full_type, it.id);
        }
        sb.push('\n');
        for it in &c.items {
            item(sb, dict, it);
        }
    }

    /// `o == null ? "null" : "s:" + o`
    fn st(v: Option<&str>) -> String {
        v.map_or("null".to_string(), |v| format!("s:{v}"))
    }
    /// java.util.List.toString
    fn list(v: Option<&[String]>) -> String {
        v.map_or("null".to_string(), |v| format!("[{}]", v.join(", ")))
    }

    /// The rest of the V line, the W line, the key and the P lines ChunkSaveProbe.java prints for a loaded vehicle.
    pub fn vehicle(
        sb: &mut String,
        dict: &DictionaryData,
        v: &zomboid_engine::zombie::vehicles::base_vehicle::BaseVehicle,
    ) {
        let m = &v.moving;
        let _ = writeln!(
            sb,
            "\t{},{},{}\t{},{}\t{}\t{}",
            fh(m.x),
            fh(m.y),
            fh(m.z),
            fh(m.offset_x),
            fh(m.offset_y),
            m.dir & 7,
            mod_data(m.table.as_ref())
        );
        let blood: Vec<String> = v
            .blood_intensity
            .iter()
            .map(|(k, b)| format!("{k}={b}"))
            .collect();
        let mut by_id: Vec<String> = v
            .parts
            .parts_by_id
            .iter()
            .map(|(k, i)| format!("{k}={i}"))
            .collect();
        by_id.sort();
        let idx = |i: Option<usize>| i.map_or(-1, |i| i as i64);
        let q = v.saved_rot;
        let _ = writeln!(
            sb,
            "W\t{}\t{},{},{},{}\t{}\t{}\t{},{},{},{}\t{}\t{}\t{}{}{}{}\t{},{}\t{}{}{}{}\t{},{},{},{}\t{}\t{}\t{}\t{}\t{}\t[{}]\t{},{},{},{}\t{}\t{}{}\t{}\t{}\t{},{}\t[{}]",
            fh(v.saved_physics_z),
            fh(q[0]),
            fh(q[1]),
            fh(q[2]),
            fh(q[3]),
            v.script_name,
            v.skin_index,
            v.front_end_durability,
            v.rear_end_durability,
            v.current_front_end_durability,
            v.current_rear_end_durability,
            v.key_id,
            v.key_spawned,
            b(v.headlights_on),
            b(v.created),
            b(v.sound_horn_on),
            b(v.sound_back_move_on),
            v.lightbar_lights_mode,
            v.lightbar_siren_mode,
            b(v.key_is_on_door),
            b(v.hotwired),
            b(v.hotwired_broken),
            b(v.keys_in_ignition),
            fh(v.rust),
            fh(v.color_hue),
            fh(v.color_saturation),
            fh(v.color_value),
            v.mechanical_id,
            b(v.alarmed),
            dh(v.vehicle_alarm.start_time),
            s(v.vehicle_alarm.chosen_sound.as_deref()),
            dh(v.siren_start_time),
            blood.join(", "),
            v.vehicle_towing_id,
            s(v.tow_attachment_self.as_deref()),
            s(v.tow_attachment_other.as_deref()),
            fh(v.row_constraint_z_offset),
            fh(v.regulator_speed),
            b(v.previously_entered),
            b(v.previously_moved),
            b(v.do_damage_overlay),
            v.parts.parts.len(),
            idx(v.parts.engine),
            idx(v.parts.battery),
            by_id.join(", ")
        );
        if let Some(k) = &v.current_key {
            sb.push_str("KEY\n");
            item(sb, dict, k);
        }
        for (i, p) in v.parts.parts.iter().enumerate() {
            let light = p.light.as_ref().map_or("-".to_string(), |l| {
                format!(
                    "{},{},{},{},{},{}",
                    b(l.active),
                    fh(l.offset_x),
                    fh(l.offset_y),
                    fh(l.intensity),
                    fh(l.dist),
                    l.focusing
                )
            });
            let door = p.door.map_or("-".to_string(), |d| {
                format!("{},{},{}", b(d.open), b(d.locked), b(d.lock_broken))
            });
            let window = p.window.map_or("-".to_string(), |w| {
                format!("{},{}", b(w.open), fh(w.open_delta))
            });
            let engine = p.engine.map_or("-".to_string(), |e| {
                format!("{},{},{},{}", b(e.running), e.quality, e.loudness, e.power)
            });
            let device = p.device_data.as_ref().map_or("-".to_string(), |d| {
                format!(
                    "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                    d.device_name,
                    b(d.two_way),
                    d.transmit_range,
                    d.mic_range,
                    b(d.mic_is_muted),
                    fh(d.base_volume_range),
                    fh(d.device_volume),
                    b(d.is_portable),
                    b(d.is_television),
                    b(d.is_high_tier),
                    b(d.is_turned_on),
                    d.channel,
                    d.min_channel_range,
                    d.max_channel_range,
                    b(d.is_battery_powered),
                    b(d.has_battery),
                    fh(d.power_delta),
                    fh(d.use_delta),
                    d.headphone_type,
                    d.media_index,
                    d.media_type,
                    s(d.media_item.as_deref()),
                    b(d.no_transmit)
                )
            });
            let mut comps: Vec<i16> = p.components.iter().flatten().map(|(id, _)| *id).collect();
            comps.sort();
            let comps: Vec<String> = comps.iter().map(|c| c.to_string()).collect();
            let it = p.item.as_ref().map_or("-".to_string(), |it| {
                format!(
                    "{}:{},{}",
                    it.full_type,
                    it.id,
                    b(it.broken.unwrap_or(false))
                )
            });
            let _ = writeln!(
                sb,
                "P\t{i}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{light}\t{door}\t{window}\t{engine}\t{device}\t[{}]\t{it}",
                p.part_id,
                b(p.created),
                fh(p.last_updated),
                p.condition,
                fh(p.wheel_friction),
                p.mechanic_skill_installer,
                fh(p.suspension_compression),
                fh(p.suspension_damping),
                mod_data(p.mod_data.as_ref()),
                comps.join(", ")
            );
            if let Some(it) = &p.item {
                item(sb, dict, it);
            }
            if let Some(c) = &p.container {
                container(sb, dict, c);
            }
        }
    }

    /// `"s:" + v` / "null", as ChunkSaveProbe.v prints a String.
    fn sv(v: Option<&str>) -> String {
        v.map_or("null".to_string(), |v| format!("s:{v}"))
    }
    fn sorted(mut l: Vec<String>) -> String {
        l.sort();
        format!("[{}]", l.join(", "))
    }

    /// The rest of the PL line and the SD, HV, C/I, SI, CH, XP, BD, BP, TH, TN and PLY lines ChunkSaveProbe.java prints
    /// for a loaded local player.
    pub fn player(
        sb: &mut String,
        dict: &DictionaryData,
        p: &zomboid_engine::zombie::characters::iso_player::IsoPlayer,
        remaining: usize,
    ) {
        let c = &p.character;
        let m = &c.moving;
        let _ = writeln!(
            sb,
            "\t{remaining}\t{},{},{}\t{},{}\t{}\t{}",
            fh(m.x),
            fh(m.y),
            fh(m.z),
            fh(m.offset_x),
            fh(m.offset_y),
            m.dir & 7,
            mod_data(m.table.as_ref())
        );
        if let Some(desc) = &c.descriptor {
            let _ = writeln!(
                sb,
                "SD\t{}\t{}\t{}\t{}\t{}\t[{}]\t{}\t{}\t{}",
                desc.id,
                desc.forename,
                desc.surname,
                desc.torso,
                b(desc.female),
                desc.extra.join(", "),
                desc.voice_prefix,
                fh(desc.voice_pitch),
                desc.voice_type
            );
        }
        human_visual(sb, &c.visual);
        container(sb, dict, &c.inventory);
        let _ = write!(sb, "SI\t{}", c.saved_items.len());
        for (i, it) in c.saved_items.iter().enumerate() {
            if it.is_none() {
                let _ = write!(sb, "\t{i}");
            }
        }
        sb.push('\n');
        let stats: Vec<String> = c.stats.values.iter().map(|v| fh(*v)).collect();
        let hand = |h: Option<usize>| {
            h.filter(|&i| c.saved_items[i].is_some())
                .map_or(-1, |i| i as i64)
        };
        let effects = [
            c.depress_effect,
            c.depress_first_take_time,
            c.beta_effect,
            c.beta_delta,
            c.pain_effect,
            c.pain_delta,
            c.sleeping_tablet_effect,
            c.sleeping_tablet_delta,
        ]
        .map(fh)
        .join(",");
        let books: Vec<String> = c
            .read_books
            .iter()
            .map(|(t, n)| format!("{t}:{n}"))
            .collect();
        let lit: Vec<String> = c
            .read_literature
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        let media: Vec<String> = c.read_print_media.iter().cloned().collect();
        let cheats: Vec<String> = c.cheats.cheats.iter().map(|i| i.to_string()).collect();
        let _ = writeln!(
            sb,
            "CH\t{},{}\t{}\t{},{}\t{effects}\t[{}]\t{}\t[{}]\t{},{},{},{},{},{}\t{}\t{}\t{}\t[{}]",
            b(c.asleep),
            fh(c.force_wake_up_time),
            stats.join(","),
            hand(c.left_hand),
            hand(c.right_hand),
            books.join(", "),
            fh(c.reduce_infection_power),
            c.known_recipes.join(", "),
            c.last_hour_sleeped,
            fh(c.time_since_last_smoke),
            fh(c.beard_grow_timing),
            fh(c.hair_grow_timing),
            b(c.sneaking),
            b(c.death_drag_down),
            sorted(lit),
            sorted(media),
            c.last_animal_pet,
            cheats.join(", ")
        );
        let traits: Vec<String> = c
            .traits
            .known_traits
            .iter()
            .map(|t| t.to_string())
            .collect();
        let xp_map: Vec<String> =
            c.xp.xp_map
                .iter()
                .map(|(k, v)| format!("{k}={}", fh(*v)))
                .collect();
        let perks: Vec<String> =
            c.xp.perk_list
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
        let mult: Vec<String> =
            c.xp.xp_map_multiplier
                .iter()
                .map(|(k, x)| format!("{k}={},{},{}", fh(x.multiplier), x.min_level, x.max_level))
                .collect();
        let _ = writeln!(
            sb,
            "XP\t[{}]\t{},{},{}\t{}\t[{}]\t{}",
            traits.join(", "),
            fh(c.xp.total_xp),
            c.xp.level,
            c.xp.lastlevel,
            sorted(xp_map),
            perks.join(", "),
            sorted(mult)
        );
        let bd = &c.body_damage;
        let _ = writeln!(
            sb,
            "BD\t{},{},{},{},{},{},{},{},{},{},{},{}",
            fh(bd.catch_a_cold),
            b(bd.has_a_cold),
            fh(bd.cold_strength),
            fh(bd.time_to_sneeze_or_cough),
            b(bd.reduce_fake_infection),
            fh(bd.health_from_food_timer),
            fh(bd.pain_reduction),
            fh(bd.cold_reduction),
            fh(bd.infection_time),
            fh(bd.infection_mortality_duration),
            fh(bd.cold_damage_stage),
            fh(bd.overall_body_health)
        );
        for (i, q) in bd.body_parts.iter().enumerate() {
            let f = [
                fh(q.health),
                b(q.bandaged).to_string(),
                b(q.bitten).to_string(),
                b(q.bleeding).to_string(),
                b(q.is_bleeding_stemmed).to_string(),
                b(q.is_cauterized).to_string(),
                b(q.scratched).to_string(),
                b(q.stitched).to_string(),
                b(q.deep_wounded).to_string(),
                b(q.is_infected).to_string(),
                b(q.is_fake_infected).to_string(),
                fh(q.bandage_life),
                fh(q.scratch_time),
                fh(q.bite_time),
                b(q.alcoholic_bandage).to_string(),
                fh(q.stiffness),
                fh(q.wound_infection_level),
                b(q.infected_wound).to_string(),
                fh(q.bleeding_time),
                fh(q.deep_wound_time),
                b(q.have_glass).to_string(),
                fh(q.stitch_time),
                fh(q.alcohol_level),
                fh(q.additional_pain),
                sv(q.bandage_type.as_deref()),
                b(q.get_bandage_xp).to_string(),
                b(q.get_stitch_xp).to_string(),
                b(q.get_splint_xp).to_string(),
                fh(q.fracture_time),
                b(q.splint).to_string(),
                fh(q.splint_factor),
                b(q.have_bullet).to_string(),
                fh(q.burn_time),
                b(q.need_burn_wash).to_string(),
                fh(q.last_time_burn_wash),
                sv(q.splint_item.as_deref()),
                fh(q.plantain_factor),
                fh(q.comfrey_factor),
                fh(q.garlic_factor),
                fh(q.cut_time),
                b(q.cut).to_string(),
                fh(q.wetness),
            ];
            let _ = writeln!(sb, "BP\t{i}\t{}", f.join(","));
        }
        if let Some(t) = &bd.thermoregulator {
            let _ = writeln!(
                sb,
                "TH\t{},{},{},{},{},{},{},{},{}",
                fh(t.set_point),
                fh(t.metabolic_rate),
                fh(t.metabolic_rate_real.unwrap_or(0.0)),
                fh(t.metabolic_target),
                fh(t.body_heat_delta),
                fh(t.core_heat_delta),
                fh(t.thermal_damage),
                fh(t.damage_counter),
                fh(t.thermal_duration.unwrap_or(0.0))
            );
            for (i, n) in &t.nodes {
                let _ = writeln!(
                    sb,
                    "TN\t{i}\t{}",
                    [
                        n.celcius,
                        n.skin_celcius,
                        n.heat_delta,
                        n.primary_delta,
                        n.secondary_delta,
                        n.insulation,
                        n.windresist,
                        n.body_wetness,
                        n.clothing_wetness
                    ]
                    .map(fh)
                    .join(",")
                );
            }
        }
        let worn: Vec<String> = p
            .worn_items
            .iter()
            .map(|(l, i)| format!("{l}={i}"))
            .collect();
        let n = &p.nutrition;
        let sv_ = p.saved_vehicle;
        let mech: Vec<String> = p
            .mechanics_item
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        let f = &p.fitness;
        let fit = format!(
            "{}{}{}[{}]{}",
            sorted(
                f.stiffness_inc_map
                    .iter()
                    .map(|(k, v)| format!("{k}={}", fh(*v)))
                    .collect()
            ),
            sorted(
                f.stiffness_timer_map
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect()
            ),
            sorted(
                f.regularity_map
                    .iter()
                    .map(|(k, v)| format!("{k}={}", fh(*v)))
                    .collect()
            ),
            f.bodypart_to_inc_stiffness.join(", "),
            sorted(
                f.exe_timer
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect()
            )
        );
        let craft: Vec<String> = p
            .craft_history
            .craft_history
            .iter()
            .map(|(k, e)| format!("{k}=s:{},{}", e.craft_count, dh(e.last_craft_time)))
            .collect();
        let known: Vec<String> = p.known_media_lines.iter().cloned().collect();
        let _ = writeln!(
            sb,
            "PLY\t{},{},{}\t{}\t{},{},{},{},{}\t{},s:{}\t{},{},{}\t{},{},{},{},{},{},{},{}\t{}\t{fit}\t[{}]\t{}\t{}",
            dh(p.hours_survived),
            p.zombie_kills,
            p.survivor_kills,
            sorted(worn),
            fh(n.calories),
            fh(n.proteins),
            fh(n.lipids),
            fh(n.carbohydrates),
            dh(n.weight),
            b(p.all_chat_muted),
            p.tag_prefix,
            fh(p.tag_color[0]),
            fh(p.tag_color[1]),
            fh(p.tag_color[2]),
            sv(Some(&p.display_name)),
            b(p.show_tag),
            b(p.faction_pvp),
            b(p.auto_drink.unwrap_or(true)),
            fh(sv_.map_or(0.0, |v| v.x)),
            fh(sv_.map_or(0.0, |v| v.y)),
            sv_.map_or(-1, |v| i32::from(v.seat)),
            b(sv_.is_some_and(|v| v.running)),
            sorted(mech),
            p.already_read_book.join(", "),
            sorted(known),
            sorted(craft)
        );
    }

    /// The I and J lines ChunkSaveProbe.java prints for an item (and its bag's contents).
    pub fn item(sb: &mut String, dict: &DictionaryData, it: &InventoryItem) {
        let cls = it.item_type.class_name().unwrap_or("?");
        let tinted = matches!(
            it.item_type,
            ItemType::Clothing | ItemType::AlarmClockClothing | ItemType::Container
        );
        let moveable = matches!(it.item_type, ItemType::Moveable | ItemType::Radio);
        let names: Vec<String> = it
            .extra_items
            .iter()
            .map(|id| {
                dict.items
                    .get(id)
                    .map_or("null".to_string(), |i| i.full_type.clone())
            })
            .collect();
        let extra = format!("[{}]", names.join(", "));
        let _ = write!(
            sb,
            "I\t{}:{}\t{cls}\t{}\t{}/{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{extra}\t{}\t{}\t{}\t{},{}\t{},{},{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{},{},{}\t{}\t{}",
            it.full_type,
            it.id,
            it.uses,
            it.condition,
            it.condition_max,
            b(it.custom_color),
            if tinted { "~".to_string() } else { it.col.map(fh).join(",") },
            fh(it.item_capacity),
            b(it.activated),
            it.have_been_repaired,
            if moveable || it.recorded_media_id.is_some() {
                "~".to_string()
            } else {
                it.name.as_deref().map_or("=".to_string(), |n| format!("s:{n}"))
            },
            it.byte_data.as_ref().map_or("-".to_string(), |d| d.len().to_string()),
            b(it.custom_name),
            if moveable { "~".to_string() } else { it.custom_weight.map_or("-".to_string(), fh) },
            it.key_id,
            it.remote_control_id,
            it.remote_range,
            fh(it.color_red),
            fh(it.color_green),
            fh(it.color_blue),
            st(it.worker.as_deref()),
            fh(it.wet_cooldown),
            b(it.favorite),
            st(it.stash_map.as_deref()),
            b(it.zombie_infected),
            it.current_ammo_count,
            it.attached_slot,
            st(it.attached_slot_type.as_deref()),
            st(it.attached_to_model.as_deref()),
            it.max_capacity,
            it.recorded_media_id.as_deref().map_or(it.recorded_media_index.to_string(), |id| format!(
                "{}={id}",
                it.recorded_media_index
            )),
            fh(it.world_x_rotation),
            fh(it.world_y_rotation),
            fh(it.world_z_rotation),
            fh(it.world_scale),
            b(it.is_initialised)
        );
        let _ = writeln!(
            sb,
            "\t{}\t{}",
            mod_data(it.table.as_ref()),
            it.custom_name_entry.as_ref().map_or("-".to_string(), |n| {
                if moveable {
                    "~".to_string()
                } else {
                    n.as_deref().map_or("=".to_string(), |n| format!("s:{n}"))
                }
            })
        );
        let mut x = String::from("J");
        match &it.kind {
            ItemKind::Plain => {}
            ItemKind::Key(k) => {
                let _ = write!(x, "\t{}", k.number_of_key);
            }
            ItemKind::AlarmClock(a) => alarm(&mut x, a),
            ItemKind::Clothing(c, a) => {
                clothing(&mut x, c);
                if let Some(a) = a {
                    alarm(&mut x, a);
                }
            }
            ItemKind::Container(c) => {
                let _ = write!(x, "\t{}", c.weight_reduction);
            }
            ItemKind::Food(f) => {
                let la = if f.last_aged == -1.0 {
                    "W".to_string()
                } else {
                    fh(f.last_aged)
                };
                let _ = write!(x, "\t{}\t{la}", fh(f.age));
                for v in [
                    f.calories,
                    f.proteins,
                    f.lipids,
                    f.carbohydrates,
                    f.hung_change,
                    f.base_hunger,
                    f.unhappy_change,
                    f.boredom_change,
                    f.thirst_change,
                    f.heat,
                ] {
                    let _ = write!(x, "\t{}", fh(v));
                }
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    f.last_cook_minute,
                    fh(f.cooking_time),
                    b(f.cooked),
                    b(f.burnt),
                    b(f.is_cookable),
                    b(f.dangerous_uncooked),
                    f.poison_detection_level,
                    list(f.spices.as_deref()),
                    f.poison_power,
                    st(f.chef.as_deref()),
                    f.off_age,
                    f.off_age_max,
                    fh(f.pain_reduction),
                    f.flu_reduction,
                    f.food_sickness_change,
                    b(f.poison),
                    f.use_for_poison,
                    fh(f.freezing_time),
                    b(f.frozen),
                    fh(f.last_frozen_update),
                    fh(f.rotten_time),
                    fh(f.compost_time),
                    b(f.cooked_in_microwave),
                    fh(f.fatigue_change),
                    fh(f.end_change),
                    f.milk.as_ref().map_or(0, |m| m.0),
                    st(f.milk.as_ref().map(|m| m.1.as_str())),
                    fh(f.stress_change)
                );
            }
            ItemKind::Literature(l) => {
                let pages = l.custom_pages.as_ref().map_or(String::new(), |p| {
                    p.iter()
                        .enumerate()
                        .map(|(i, t)| format!("{}={t}", i + 1))
                        .collect::<Vec<_>>()
                        .join(", ")
                });
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}\t{{{pages}}}\t{}\t{}",
                    l.number_of_pages,
                    l.already_read_pages,
                    b(l.can_be_write),
                    st(l.locked_by.as_deref()),
                    list(l.learned_recipes.as_deref())
                );
            }
            ItemKind::Weapon(w) => {
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t",
                    fh(w.max_range),
                    fh(w.min_range_ranged),
                    w.clip_size,
                    fh(w.min_damage),
                    fh(w.max_damage),
                    w.recoil_delay,
                    w.aiming_time,
                    w.reload_time,
                    w.hit_chance,
                    fh(w.min_angle),
                    w.explosion_timer,
                    fh(w.max_angle),
                    fh(w.blood_level),
                    b(w.is_jammed)
                );
                for p in &w.parts {
                    let _ = write!(x, "{p};");
                }
            }
            ItemKind::Moveable(m) => moveable_fields(&mut x, m),
            ItemKind::Radio(m, r) => {
                moveable_fields(&mut x, m);
                let d = r.device_data.clone().unwrap_or_default();
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    d.device_name,
                    b(d.two_way),
                    d.transmit_range,
                    d.mic_range,
                    b(d.mic_is_muted),
                    fh(d.base_volume_range),
                    fh(d.device_volume),
                    b(d.is_portable),
                    b(d.is_television),
                    b(d.is_high_tier),
                    b(d.is_turned_on),
                    d.channel,
                    d.min_channel_range,
                    d.max_channel_range,
                    b(d.is_battery_powered),
                    b(d.has_battery),
                    fh(d.power_delta),
                    fh(d.use_delta),
                    d.headphone_type,
                    d.media_index,
                    d.media_type,
                    s(d.media_item.as_deref()),
                    b(d.no_transmit)
                );
            }
            ItemKind::Map(m) => {
                let _ = write!(x, "\t{}", st(Some(&m.map_id)));
                for sy in &m.symbols.symbols {
                    let (name, base) = match sy {
                        WorldMapSymbol::Text { base, .. } => ("WorldMapTextSymbol", base),
                        WorldMapSymbol::Texture { base, .. } => ("WorldMapTextureSymbol", base),
                    };
                    let _ = write!(x, "\t{name}");
                    for v in [
                        base.x,
                        base.y,
                        base.anchor_x,
                        base.anchor_y,
                        base.scale,
                        base.rotation,
                        base.r,
                        base.g,
                        base.b,
                        base.a,
                    ] {
                        let _ = write!(x, ",{}", fh(v));
                    }
                    let _ = write!(
                        x,
                        ",{}{}{},{},{}",
                        b(base.collide),
                        b(base.match_perspective),
                        b(base.apply_zoom),
                        fh(base.min_zoom.unwrap_or(0.0)),
                        fh(base.max_zoom.unwrap_or(24.0))
                    );
                    match sy {
                        WorldMapSymbol::Text {
                            text,
                            translated,
                            layer_id,
                            ..
                        } => {
                            let _ = write!(
                                x,
                                ",s:{text},{},{}",
                                b(*translated),
                                layer_id.as_deref().unwrap_or("text-note")
                            );
                        }
                        WorldMapSymbol::Texture { symbol_id, .. } => {
                            let _ = write!(x, ",s:{symbol_id}");
                        }
                    }
                }
            }
        }
        if x.len() > 1 {
            sb.push_str(&x);
            sb.push('\n');
        }
        if let ItemKind::Container(c) = &it.kind {
            container(sb, dict, &c.container);
        }
    }

    fn alarm(x: &mut String, a: &AlarmClockSave) {
        let _ = write!(
            x,
            "\t{}\t{}\t{}\t{}\t{}",
            a.alarm_hour,
            a.alarm_minutes,
            b(a.alarm_set),
            dh(f64::from(a.ring_since)),
            a.force_dont_ring
        );
    }

    fn clothing(x: &mut String, c: &ClothingSave) {
        let _ = write!(x, "\t{}\t", fh(c.wetness));
        for (k, p) in c.patches.iter().flatten() {
            let _ = write!(
                x,
                "{k}:{}/{}/{}/{}/{}/{};",
                p.tailor_lvl,
                p.fabric_type,
                p.scratch_defense,
                p.bite_defense,
                b(p.has_hole),
                p.condition_gain
            );
        }
    }

    fn moveable_fields(x: &mut String, m: &MoveableSave) {
        let _ = write!(
            x,
            "\t{}\t{}",
            st(Some(&m.world_sprite)),
            b(m.light.is_some())
        );
        if let Some(l) = &m.light {
            let _ = write!(
                x,
                "\t{}\t{}\t{}",
                b(l.use_battery),
                b(l.has_battery),
                st(Some(l.bulb_item.as_deref().unwrap_or("Base.LightBulb")))
            );
            for v in [l.power, l.delta, l.r, l.g, l.b] {
                let _ = write!(x, "\t{}", fh(v));
            }
        }
    }

    fn object(
        sb: &mut String,
        sm: &IsoSpriteManager,
        dict: &DictionaryData,
        sq: &IsoGridSquare,
        index: usize,
        o: &IsoObject,
    ) {
        let default = ObjectSave::default();
        let os = o.save.as_deref().unwrap_or(&default);
        let world = sq
            .save
            .as_ref()
            .is_some_and(|s| s.world_objects.contains(&index));
        let key_id = match &os.class {
            ClassSave::Door { key_id, .. } => *key_id,
            _ => os.key_id,
        };
        let mut comps: Vec<String> = os.components.iter().map(|(id, _)| id.to_string()).collect();
        comps.sort_by_key(|c| c.parse::<i32>().unwrap_or(0));
        let _ = writeln!(
            sb,
            "O\t{index}\t{}\t{}\t{}\t{}\t{}{}\t{}{}{}{}{}\t{}\t{key_id}\t{}\t{}\t{}\t{}\t{}\t[{}]\t{}",
            o.kind.class_name(),
            sn(sm, o.sprite),
            s(os.name.as_deref()),
            s(os.sprite_name.as_deref()),
            b(o.special),
            b(world),
            b(o.outline_on_mouseover),
            b(os.do_not_sync),
            b(os.special_tooltip),
            b(os.uses_external_water_source),
            b(os.moved_thumpable),
            fh(o.render_y_offset),
            os.custom_color.map_or("-".to_string(), |c| c.map(fh).join(",")),
            os.sheet_rope_health.map_or("-".to_string(), fh),
            sn(sm, os.overlay_sprite),
            os.overlay_sprite_color.map_or("-".to_string(), |c| c.map(fh).join(",")),
            s(os.sprite_model_name.as_deref()),
            comps.join(", "),
            mod_data(os.mod_data.as_ref())
        );
        for (spr, a) in o.attached_anim_sprite.iter().flatten().zip(&os.attached) {
            let d = a.data.unwrap_or([0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
            let _ = writeln!(
                sb,
                "A\t{}\t{}{}{}\t{},{},{}\t{},{},{}\t{}",
                sn(sm, Some(*spr)),
                b(a.flip),
                b(a.copy_target_alpha),
                b(a.multiply_object_alpha),
                fh(d[0]),
                fh(d[1]),
                fh(d[2]),
                fh(d[3]),
                fh(d[4]),
                fh(d[5]),
                fh(a.alpha.unwrap_or(1.0))
            );
        }
        for w in &os.wall_blood_splats {
            let _ = writeln!(sb, "W\t{}", sn(sm, w.sprite));
        }
        for c in &os.containers {
            container(sb, dict, c);
        }
        let mut x = String::from("X");
        match (&o.kind, &os.class) {
            (
                ObjectKind::IsoDoor(d),
                ClassSave::Door {
                    health,
                    max_health,
                    key_id,
                    has_curtain,
                    curtain_open,
                    curtain_inside,
                },
            ) => {
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}\t{health}\t{max_health}\t{}\t{}\t{key_id}\t{}\t{}\t{}\t{}",
                    b(d.open),
                    b(d.locked),
                    b(d.north),
                    sn(sm, d.closed_sprite),
                    sn(sm, d.open_sprite),
                    b(d.locked_by_key),
                    b(*has_curtain),
                    b(*curtain_open),
                    b(*curtain_inside)
                );
            }
            (ObjectKind::IsoWindow(w), ClassSave::Window { health, max_health }) => {
                let _ = write!(
                    x,
                    "\t{}\t{}\t{health}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{max_health}",
                    b(w.open),
                    b(w.north),
                    b(w.locked),
                    b(w.perma_locked),
                    b(w.smashed),
                    b(w.glass_removed),
                    sn(sm, w.open_sprite),
                    sn(sm, w.closed_sprite),
                    sn(sm, w.smashed_sprite),
                    sn(sm, w.glass_removed_sprite)
                );
            }
            (
                ObjectKind::IsoCurtain(c),
                ClassSave::Curtain {
                    health,
                    barricade_strength,
                },
            ) => {
                let _ = write!(
                    x,
                    "\t{}\t{}\t{health}\t{barricade_strength}\t{}\t{}",
                    b(c.open),
                    b(c.north),
                    sn(sm, c.closed_sprite),
                    sn(sm, c.open_sprite)
                );
            }
            (ObjectKind::IsoWindowFrame { north }, _) => {
                let _ = write!(x, "\t{}", b(*north));
            }
            (ObjectKind::IsoTree(t), _) => {
                let _ = write!(x, "\t{}\t{}\t{}", t.size, t.log_yield, t.damage);
            }
            (_, ClassSave::LightSwitch(l)) => {
                let activated = if l.activated_from_room {
                    "?".to_string()
                } else {
                    b(l.activated).to_string()
                };
                let _ = write!(
                    x,
                    "\t{}\t{}\t{activated}\t{}\t{}\t{}\t{}",
                    b(l.light_room),
                    l.room_id,
                    b(l.can_be_modified),
                    b(l.use_battery),
                    b(l.has_battery),
                    s(l.bulb_item.as_deref())
                );
                for f in [l.power, l.delta, l.primary[0], l.primary[1], l.primary[2]] {
                    let _ = write!(x, "\t{}", fh(f));
                }
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}",
                    l.last_minute_stamp,
                    l.bulb_burn_minutes,
                    b(l.street_light)
                );
            }
            (
                ObjectKind::IsoBarbecue {
                    has_propane_tank,
                    fuel_amount_minutes,
                },
                ClassSave::Barbecue {
                    lit,
                    last_update_time,
                    minutes_since_extinguished,
                    normal,
                    no_tank,
                },
            ) => {
                let _ = write!(
                    x,
                    "\t{}\t{fuel_amount_minutes}\t{}\t{}\t{minutes_since_extinguished}\t{}\t{}",
                    b(*has_propane_tank),
                    b(*lit),
                    fh(*last_update_time),
                    sn(sm, *normal),
                    sn(sm, *no_tank)
                );
            }
            (
                _,
                ClassSave::Fireplace {
                    fuel_amount_minutes,
                    lit,
                    last_update_time,
                    minutes_since_extinguished,
                },
            ) => {
                let _ = write!(
                    x,
                    "\t{fuel_amount_minutes}\t{}\t{}\t{minutes_since_extinguished}",
                    b(*lit),
                    fh(*last_update_time)
                );
            }
            (
                _,
                ClassSave::Stove {
                    activated,
                    seconds_timer,
                    max_temperature,
                    first_turn_on,
                    broken,
                },
            ) => {
                let _ = write!(
                    x,
                    "\t{}\t{seconds_timer}\t{}\t{}\t{}",
                    b(*activated),
                    fh(*max_temperature),
                    b(*first_turn_on),
                    b(*broken)
                );
            }
            (
                _,
                ClassSave::Washer {
                    activated,
                    last_update,
                },
            ) => {
                let _ = write!(x, "\t{}\t{}", b(*activated), fh(*last_update));
            }
            (_, ClassSave::Dryer { activated }) => {
                let _ = write!(x, "\t{}", b(*activated));
            }
            (
                _,
                ClassSave::WasherDryer {
                    dryer,
                    washer_activated,
                    washer_last_update,
                    dryer_activated,
                },
            ) => {
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}\t{}",
                    b(*dryer),
                    b(*washer_activated),
                    fh(*washer_last_update),
                    b(*dryer_activated)
                );
            }
            (_, ClassSave::WaveSignal(d)) => {
                let d = d.clone().unwrap_or_default();
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    d.device_name,
                    b(d.two_way),
                    d.transmit_range,
                    d.mic_range,
                    b(d.mic_is_muted),
                    fh(d.base_volume_range),
                    fh(d.device_volume),
                    b(d.is_portable),
                    b(d.is_television),
                    b(d.is_high_tier),
                    b(d.is_turned_on),
                    d.channel,
                    d.min_channel_range,
                    d.max_channel_range,
                    b(d.is_battery_powered),
                    b(d.has_battery),
                    fh(d.power_delta),
                    fh(d.use_delta),
                    d.headphone_type,
                    d.media_index,
                    d.media_type,
                    s(d.media_item.as_deref()),
                    b(d.no_transmit)
                );
                let p = d.presets.unwrap_or_default();
                let _ = write!(
                    x,
                    "\t{}",
                    if p.max_presets == 0 && p.presets.is_empty() {
                        10
                    } else {
                        p.max_presets
                    }
                );
                for (name, freq) in &p.presets {
                    let _ = write!(x, "\t{name}:{freq}");
                }
            }
            (_, ClassSave::Mannequin(m)) => {
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    m.dir & 7,
                    b(m.init),
                    b(m.female),
                    b(m.zombie),
                    b(m.skeleton),
                    m.mannequin_script_name,
                    m.pose
                );
            }
            (_, ClassSave::WorldInventory(w)) => {
                let (ft, id) = w
                    .item
                    .as_ref()
                    .map_or(("null".to_string(), 0), |i| (i.full_type.clone(), i.id));
                let _ = write!(
                    x,
                    "\t{}\t{}\t{}\t{ft}:{id}\t{}\t{}\t{}",
                    fh(w.xoff),
                    fh(w.yoff),
                    fh(w.zoff),
                    dh(w.drop_time),
                    b(w.ignore_remove_sandbox),
                    b(w.extended_placement)
                );
            }
            _ => {}
        }
        if x.len() > 1 {
            sb.push_str(&x);
            sb.push('\n');
        }
        if let ClassSave::Mannequin(m) = &os.class {
            human_visual(sb, &m.human_visual);
        }
        if let ClassSave::WorldInventory(w) = &os.class {
            if let Some(i) = &w.item {
                item(sb, dict, i);
            }
        }
    }

    fn corpse(sb: &mut String, dict: &DictionaryData, d: &IsoDeadBody) {
        let _ = write!(
            sb,
            "D\t{},{},{}\t{}{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            fh(d.moving.x),
            fh(d.moving.y),
            fh(d.moving.z),
            b(d.female),
            b(d.was_zombie),
            d.object_id.0,
            d.persistent_outfit_id,
            fh(d.death_time),
            fh(d.reanimate_time),
            b(d.fall_on_front),
            b(d.killed_by_fall),
            b(d.was_skeleton),
            fh(d.angle),
            d.zombie_rot_stage_at_death,
            d.animal_rot_stage_at_death,
            s(d.rotten_texture.as_deref()),
            s(d.skel_inv_icon.as_deref())
        );
        let _ = writeln!(
            sb,
            "\t{}\t{}\t{}\t{}",
            b(d.crawling),
            b(d.fake_dead),
            b(d.ragdoll_fall),
            mod_data(d.moving.table.as_ref())
        );
        if let Some(desc) = &d.desc {
            let _ = writeln!(
                sb,
                "SD\t{}\t{}\t{}\t{}\t{}\t[{}]\t{}\t{}\t{}",
                desc.id,
                desc.forename,
                desc.surname,
                desc.torso,
                b(desc.female),
                desc.extra.join(", "),
                desc.voice_prefix,
                fh(desc.voice_pitch),
                desc.voice_type
            );
        }
        human_visual(sb, &d.human_visual);
        if let Some(c) = &d.container {
            container(sb, dict, c);
        }
    }

    /// The text ChunkSaveProbe.java prints for one loaded chunk.
    pub fn chunk(sb: &mut String, sm: &IsoSpriteManager, dict: &DictionaryData, ch: &IsoChunk) {
        let Some(c) = ch.save.as_deref() else { return };
        let _ = write!(
            sb,
            "K\t{}\t{}\t{}\t{}\t{}{}",
            ch.wx,
            ch.wy,
            ch.get_min_level(),
            ch.get_max_level(),
            b(c.blending_done_full),
            b(c.blending_done_partial)
        );
        for f in c.blending_modified {
            sb.push(b(f));
        }
        let _ = write!(
            sb,
            "\t{}\t{}",
            bytes(Some(&c.blending_depth)),
            b(c.attachments_done_full)
        );
        for f in c.attachments_state {
            sb.push(b(f));
        }
        let e = &c.erosion;
        let _ = write!(
            sb,
            "\t{}\t{},{},{},{},{},{}\t{}\t{}",
            c.attachments_partial
                .as_ref()
                .map_or("-".to_string(), |v| v.len().to_string()),
            b(e.init),
            e.e_tick_stamp,
            e.epoch,
            fh(e.moisture),
            fh(e.minerals),
            e.soil,
            c.generators_touching_this_chunk
                .as_ref()
                .map_or("-".to_string(), |v| v.len().to_string()),
            c.loot_respawn_hour
        );
        for (i, r) in c.spawned_rooms.iter().enumerate() {
            let _ = write!(sb, "{}{r}", if i == 0 { '\t' } else { ',' });
        }
        sb.push('\n');
        for s in &c.floor_blood_splats {
            let _ = writeln!(
                sb,
                "B\t{},{},{}\t{}\t{}",
                fh(s.x),
                fh(s.y),
                fh(s.z),
                s.r#type,
                s.index
            );
        }
        for z in ch.get_min_level()..=ch.get_max_level() {
            for i in 0..64 {
                let Some(sq) = ch.get_grid_square(i % 8, i / 8, z) else {
                    continue;
                };
                let default = Default::default();
                let ss = sq.save.as_deref().unwrap_or(&default);
                let es = &ss.erosion;
                let _ = write!(
                    sb,
                    "Q\t{}\t{}\t{}\t{}\t{}{}\t{}{}{}{}\t{}{},{},{},{}",
                    sq.x,
                    sq.y,
                    sq.z,
                    sq.room_id,
                    b(sq.room_id != -1 && sq.room.is_some()),
                    b(sq.roof_hide_building.is_some()),
                    b(ss.overlay_done),
                    b(sq.have_roof),
                    b(ss.burnt_out),
                    b(ss.have_sheet_rope),
                    b(es.init),
                    b(es.do_nothing),
                    es.noise_main_byte,
                    es.soil,
                    es.magic_num_byte
                );
                for r in &es.regions {
                    let _ = write!(
                        sb,
                        ",{}/{}/{}{}{}/{}/{}/{}",
                        r.region_id,
                        r.category_id,
                        b(r.do_nothing),
                        b(r.has_spawned),
                        b(r.disp_bloom),
                        r.stage,
                        r.disp_season,
                        r.game_obj
                    );
                }
                let _ = writeln!(sb, "\t{}", mod_data(ss.mod_data.as_ref()));
                for (oi, o) in sq.objects.iter().enumerate() {
                    object(sb, sm, dict, sq, oi, o);
                }
                for d in &ss.static_moving_objects {
                    corpse(sb, dict, d);
                }
            }
        }
    }
}

/// Every chunk of a save loaded through the port of IsoChunk.LoadFromDiskOrBuffer, in the world the oracle server
/// runs (seeded basements merged into the meta grid); pairs with ChunkSaveProbe.java.
pub fn chunksave(root: &Path, save: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::basements::Basements;
    use zomboid_engine::zombie::inventory::inventory_item_factory::InventoryItemFactory;
    use zomboid_engine::zombie::iso::chunk_save::SaveLoadContext;
    use zomboid_engine::zombie::radio::media::recorded_media::RecordedMedia;
    use zomboid_engine::zombie::scripting::script_manager::ScriptManager;
    use zomboid_engine::zombie::world::dictionary_data::DictionaryData;

    let media = root.join("media");
    let (mut sm, alias, mut defs) = load_tile_definitions(root)?;
    sm.add_sprite("media/ui/missing-tile.png");
    defs.loaded_tile_definitions(&mut sm, &alias, &|_| false)?;
    let mut g = IsoMetaGrid::default();
    let dirs = IsoMetaGrid::lot_directories(&media, "Muldraugh, KY");
    g.create_step1(&media, &dirs, (-250, -250, 250, 250))?;
    for e in g.load_all_cells(&mut |_| 1, &mut |_, _| Vec::new()) {
        eprintln!("{e}");
    }
    let alarm = AlarmSettings {
        last_stand: false,
        power_grid_exists: true,
        alarm: 1,
        alarm_decay: 1,
    };
    g.create_step2(&alarm, &mut |_| 1, &mut |a, _| a);
    let seed = java::string_hash_code("DeadOxideOracle") as i64;
    Basements::before_load_meta_grid(&media, &mut g, &dirs, seed)?;
    let dict_path = save.join("WorldDictionary.bin");
    let dictionary = DictionaryData::load_from_byte_buffer(
        &std::fs::read(&dict_path).map_err(|e| format!("{}: {e}", dict_path.display()))?,
    )?;
    let scripts = ScriptManager::load(root)?;
    let media_path = save.join("recorded_media.bin");
    let recorded_media = RecordedMedia::load(
        &std::fs::read(&media_path).map_err(|e| format!("{}: {e}", media_path.display()))?,
    )?;
    let lua_path = media.join("lua/shared/RecordedMedia/recorded_media.lua");
    let factory = InventoryItemFactory::new(
        &dictionary,
        &scripts,
        recorded_media,
        &std::fs::read_to_string(&lua_path).map_err(|e| format!("{}: {e}", lua_path.display()))?,
    );
    let mut files = std::collections::BTreeMap::new();
    let map = save.join("map");
    for xd in std::fs::read_dir(&map).map_err(|e| format!("{}: {e}", map.display()))? {
        let xd = xd.map_err(|e| e.to_string())?.path();
        let Some(wx) = xd
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.parse::<i32>().ok())
        else {
            continue;
        };
        for yf in std::fs::read_dir(&xd).map_err(|e| format!("{}: {e}", xd.display()))? {
            let yf = yf.map_err(|e| e.to_string())?.path();
            let Some(wy) = yf
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".bin"))
                .and_then(|n| n.parse::<i32>().ok())
            else {
                continue;
            };
            files.insert((wx, wy), yf);
        }
    }
    let mut rand = RandAbstract::new(java::Random::new(0));
    let mut sb = String::new();
    for ((wx, wy), path) in files {
        let data = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let has_lot_header = g.get_header(wx * 8 / 256, wy * 8 / 256).is_some();
        let mut ctx = SaveLoadContext {
            sprites: &mut sm,
            alias: &alias,
            dictionary: &dictionary,
            items: &factory,
            meta_grid: &mut g,
            rand: &mut rand,
            has_lot_header,
        };
        match IsoChunk::load_from_disk_or_buffer(&data, wx, wy, &mut ctx) {
            Ok(ch) => save_dump::chunk(&mut sb, ctx.sprites, &dictionary, &ch),
            Err(e) => {
                let _ = writeln!(sb, "K\t{wx}\t{wy}\tFAIL\t{e}");
            }
        }
    }
    // zombie.vehicles.VehiclesDB2: every chunk that holds vehicles, its rows as SQLStore.loadChunk reads them.
    use zomboid_engine::zombie::vehicles::vehicles_db2::SqlStore;
    let store = SqlStore::open(&save.join("vehicles.db"))?;
    for (wx, wy) in store.chunks()? {
        for vb in store.load_chunk(wx, wy)? {
            let _ = write!(
                sb,
                "V\t{}\t{wx}\t{wy}\t{},{}\t{}\t{}",
                vb.id,
                save_dump::fh(vb.x),
                save_dump::fh(vb.y),
                vb.world_version,
                u8::from(vb.in_meta)
            );
            match vb.load_vehicle(&factory) {
                Ok(v) => save_dump::vehicle(&mut sb, &dictionary, &v),
                Err(e) => {
                    let _ = writeln!(sb, "\tFAIL\t{e}");
                }
            }
        }
    }
    // zombie.savefile.PlayerDB: every local player, as PlayerDB.loadPlayer reads it.
    use zomboid_engine::java_nio::ByteBuffer;
    use zomboid_engine::zombie::characters::body_damage::body_part::BodySandbox;
    use zomboid_engine::zombie::characters::iso_player::{player_db, IsoPlayer};
    let sandbox = zomboid_engine::zombie::sandbox_options::SandboxOptions::new();
    let opt = |n: &str| sandbox.get_int(n).unwrap_or(0);
    let body_sandbox = BodySandbox {
        transmission: opt("ZombieLore.Transmission"),
        mortality: opt("ZombieLore.Mortality"),
        injury_severity: opt("InjurySeverity"),
    };
    for pd in player_db::load_local_players(&save.join("players.db"))? {
        let _ = write!(
            sb,
            "PL\t{}\t{}\t{},{},{}\t{}\t{}",
            pd.sql_id,
            pd.name.as_deref().unwrap_or("null"),
            save_dump::fh(pd.x),
            save_dump::fh(pd.y),
            save_dump::fh(pd.z),
            pd.world_version,
            u8::from(pd.is_dead)
        );
        let mut b = ByteBuffer::wrap(&pd.data);
        match IsoPlayer::load(
            &mut b,
            pd.world_version,
            &factory,
            &dictionary,
            &body_sandbox,
        ) {
            Ok(p) => save_dump::player(&mut sb, &dictionary, &p, b.remaining()),
            Err(e) => {
                let _ = writeln!(sb, "\tFAIL\t{e}");
            }
        }
    }
    Ok(sb)
}

/// java.util.UUID.toString of (most, least) significant bits.
fn uuid_string(id: (i64, i64)) -> String {
    let (m, l) = (id.0 as u64, id.1 as u64);
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        m >> 32,
        (m >> 16) & 0xffff,
        m & 0xffff,
        l >> 48,
        l & 0xffff_ffff_ffff
    )
}

/// The save's zones (pairs with MapZoneProbe.java): the meta grid's cells from `IsoMetaGrid.Create`, then
/// `IsoMetaGrid.loadZone` of the save's `map_zone.bin`; every registered zone, the outline of each wide polyline
/// (its points in the native order; only with `outlines`), with `triangles` every zone's `Zone.getTotalArea` and the
/// triangles and triangle areas behind it, the zones of every meta chunk per cell, and the
/// spawned-zombie zones.
pub fn mapzone(
    root: &Path,
    save: &Path,
    outlines: bool,
    triangles: bool,
) -> Result<String, String> {
    use zomboid_engine::java_nio::ByteBuffer;
    let media = root.join("media");
    let mut g = IsoMetaGrid::default();
    let dirs = IsoMetaGrid::lot_directories(&media, "Muldraugh, KY");
    g.create_step1(&media, &dirs, (-250, -250, 250, 250))?;
    for e in g.load_all_cells(&mut |_| 1, &mut |_, _| Vec::new()) {
        eprintln!("{e}");
    }
    let p = save.join("map_zone.bin");
    let data = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut bb = ByteBuffer::wrap(&data);
    let mut sb = String::new();
    let wv = data
        .get(4..8)
        .map_or(0, |s| i32::from_be_bytes([s[0], s[1], s[2], s[3]]));
    let _ = writeln!(
        sb,
        "MZ {wv} {} {} {} {}",
        g.min_x, g.min_y, g.max_x, g.max_y
    );
    let m = g.load_zone(&mut bb, -1)?;
    for l in &m.log {
        eprintln!("{l}");
    }
    let _ = writeln!(sb, "REST {}", bb.remaining());
    let _ = writeln!(sb, "ZN {}", g.zones.len());
    let null = |s: &Option<String>| s.clone().unwrap_or_else(|| "null".to_string());
    for (i, z) in g.zones.iter().enumerate() {
        let mut h: u64 = 0xcbf29ce484222325;
        for &v in &z.points {
            mix(&mut h, v);
        }
        let _ = writeln!(
            sb,
            "Z {i} {} {} {} {} {} {} {} {} {} {} {h:x} {} {} {} {} {} {}",
            null(&z.type_),
            null(&z.name),
            z.x,
            z.y,
            z.z,
            z.w,
            z.h,
            z.geometry_type.name(),
            z.polyline_width,
            z.points.len(),
            z.hour_last_seen,
            u8::from(z.have_construction),
            z.last_action_timestamp,
            null(&z.original_name),
            z.id.map_or_else(|| "random".to_string(), uuid_string),
            u8::from(z.is_preferred_zone_for_square)
        );
        if let Some(o) = z.polyline_outline_points.as_ref().filter(|_| outlines) {
            let n = o.len() / 2;
            let _ = write!(sb, "O {i} {n}");
            for p in o.chunks_exact(2) {
                let _ = write!(sb, " {},{}", p[0].to_bits() as i32, p[1].to_bits() as i32);
            }
            sb.push('\n');
        }
    }
    if triangles {
        // Zone.clipper, shared by every zone
        let mut clipper = zomboid_engine::zombie::vehicles::clipper::Clipper::new();
        for (i, z) in g.zones.iter_mut().enumerate() {
            let area = match z.get_total_area(&mut clipper) {
                Ok(a) => a,
                Err(e) => {
                    let _ = writeln!(sb, "AX {i} {}", e.0.replace('/', "."));
                    continue;
                }
            };
            let _ = writeln!(sb, "A {i} {}", area.to_bits() as i32);
            let Some(t) = &z.triangles else {
                continue;
            };
            let _ = write!(sb, "T {i} {}", t.len());
            for v in t {
                let _ = write!(sb, " {}", v.to_bits() as i32);
            }
            let areas = z.triangle_areas.as_deref().unwrap_or(&[]);
            let _ = write!(sb, "\nTA {i} {}", areas.len());
            for v in areas {
                let _ = write!(sb, " {}", v.to_bits() as i32);
            }
            sb.push('\n');
        }
    }
    let mut total = 0;
    for x in g.min_x..=g.max_x {
        for y in g.min_y..=g.max_y {
            let Some(cell) = g.get_cell_data(x, y) else {
                continue;
            };
            let mut chunks = 0;
            let mut h: u64 = 0xcbf29ce484222325;
            for i in 0..1024 {
                let Some(c) = cell.chunk(i) else {
                    continue;
                };
                if c.get_zones_size() == 0 {
                    continue;
                }
                chunks += 1;
                mix(&mut h, i as i32);
                mix(&mut h, c.get_zones_size() as i32);
                for zr in c.zones() {
                    mix(&mut h, zr.0 as i32);
                }
                total += c.get_zones_size();
            }
            if chunks > 0 {
                let _ = writeln!(
                    sb,
                    "ZC {x} {y} {} {chunks} {h:x}",
                    u8::from(cell.info_priority.is_some())
                );
            }
        }
    }
    let _ = writeln!(sb, "ZT {total}");
    let _ = writeln!(sb, "SZ {}", m.spawned_zombie_zone.len());
    for (k, ids) in &m.spawned_zombie_zone {
        let ids: Vec<String> = ids
            .iter()
            .map(|id| id.map_or_else(|| "random".to_string(), uuid_string))
            .collect();
        let _ = writeln!(sb, "S {k} [{}]", ids.join(", "));
    }
    Ok(sb)
}

/// `metagrid/metacell_*.bin` through `IsoMetaCell.load`, then `IsoMetaGrid.loadAnimalZones` of the save's
/// `map_animals.bin` into a meta grid built from the lot headers; every animal zone ordered by id (`UUID.compareTo`:
/// signed most, then least significant bits) with its junctions, and the zones of each cell (pairs with
/// AnimalZonesProbe.java).
pub fn animalzones(root: &Path, save: &Path) -> Result<String, String> {
    use zomboid_engine::java_nio::ByteBuffer;
    let media = root.join("media");
    let mut g = IsoMetaGrid::default();
    let dirs = IsoMetaGrid::lot_directories(&media, "Muldraugh, KY");
    g.create_step1(&media, &dirs, (-250, -250, 250, 250))?;
    for e in g.load_all_cells(&mut |_| 1, &mut |_, _| Vec::new()) {
        eprintln!("{e}");
    }
    let mut sb = String::new();
    for l in g.load_metacells(save) {
        eprintln!("{l}");
    }
    let mut names: Vec<String> = std::fs::read_dir(save.join("metagrid"))
        .map(|rd| {
            rd.filter_map(|e| e.ok().and_then(|e| e.file_name().into_string().ok()))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    for name in names {
        let Some(rest) = name
            .strip_prefix("metacell_")
            .and_then(|r| r.strip_suffix(".bin"))
        else {
            continue;
        };
        let Some((cx, cy)) = rest
            .split_once('_')
            .and_then(|(a, b)| Some((a.parse::<i32>().ok()?, b.parse::<i32>().ok()?)))
        else {
            continue;
        };
        let generated = g
            .get_cell_data(cx, cy)
            .is_some_and(|c| c.animal_zones_generated);
        let _ = writeln!(sb, "MC {cx} {cy} {}", u8::from(generated));
    }
    let p = save.join("map_animals.bin");
    let data = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut bb = ByteBuffer::wrap(&data);
    let wv = data
        .get(4..8)
        .map_or(0, |s| i32::from_be_bytes([s[0], s[1], s[2], s[3]]));
    let _ = writeln!(sb, "MA {wv}");
    let m = g.load_animal_zones(&mut bb, -1)?;
    for l in &m.log {
        eprintln!("{l}");
    }
    let _ = writeln!(sb, "REST {}", bb.remaining());
    // the handler's zones ordered by id; arena index -> that order
    let mut handler: Vec<((i64, i64), u32)> = g
        .animal_zone_handler
        .iter()
        .map(|(id, r)| (*id, r.0))
        .collect();
    handler.sort();
    let mut index = vec![-1i32; g.animal_zones.len()];
    for (i, (_, r)) in handler.iter().enumerate() {
        index[*r as usize] = i as i32;
    }
    let _ = writeln!(sb, "AZN {}", handler.len());
    let null = |s: &Option<String>| s.clone().unwrap_or_else(|| "null".to_string());
    for (i, (id, r)) in handler.iter().enumerate() {
        let a = &g.animal_zones[*r as usize];
        let z = &a.zone;
        let mut h: u64 = 0xcbf29ce484222325;
        for &v in &z.points {
            mix(&mut h, v);
        }
        let nj = a.junctions.as_ref().map_or(-1, |j| j.len() as i64);
        let _ = writeln!(
            sb,
            "AZ {i} {} {} {} {} {} {} {} {} {} {} {} {h:x} {} {} {} {} {} {nj}",
            uuid_string(*id),
            null(&z.type_),
            null(&z.name),
            z.x,
            z.y,
            z.z,
            z.w,
            z.h,
            z.geometry_type.name(),
            z.polyline_width,
            z.points.len(),
            null(&z.original_name),
            null(&a.action),
            null(&a.animal_type),
            u8::from(a.spawn_animal),
            u8::from(a.spawned_animals)
        );
        for j in a.junctions.iter().flatten() {
            let other = j.zone_other.map_or(-1, |o| index[o.0 as usize]);
            let _ = writeln!(
                sb,
                "AJ {i} {} {other} {} {}",
                j.point_index_self,
                j.point_index_other,
                j.distance_from_start.to_bits() as i32
            );
        }
    }
    for x in g.min_x..=g.max_x {
        for y in g.min_y..=g.max_y {
            let Some(cell) = g.get_cell_data(x, y) else {
                continue;
            };
            if cell.animal_zones.is_empty() {
                continue;
            }
            let mut h: u64 = 0xcbf29ce484222325;
            for r in &cell.animal_zones {
                mix(&mut h, index[r.0 as usize]);
            }
            let _ = writeln!(sb, "AC {x} {y} {} {h:x}", cell.animal_zones.len());
        }
    }
    Ok(sb)
}

/// ClimateProbe.java's scenarios on the ported ClimateManager: the same clock core, season timer and
/// `ClimateManager.update` per tick, printing the same lines (floats as `Float.toString`).
///
/// The inputs the probe reads from the booted original dedicated server and prints in its SETUP line (the sandbox's
/// Rain, climate and fog cycles, day/night cycle, fog quality, the meta grid bounds of the harness's world, the game
/// mode) are fixed here to what that line shows; the diff checks them against the original.
pub fn climate(root: &Path, scenarios: &[String]) -> Result<String, String> {
    use zomboid_engine::java::{double_to_string as d, float_to_string as f};
    use zomboid_engine::zombie::erosion::season::erosion_season::erosion_main_season;
    use zomboid_engine::zombie::game_time::GameTime;
    use zomboid_engine::zombie::iso::weather::climate_color_info::ClimateColorInfo;
    use zomboid_engine::zombie::iso::weather::climate_forecaster::ForecastValue;
    use zomboid_engine::zombie::iso::weather::climate_main_lua::apply_climate_main;
    use zomboid_engine::zombie::iso::weather::climate_manager::{
        AirFront, ClimateEnv, ClimateManager, BOOL_IS_SNOW, COLOR_GLOBAL_LIGHT, COLOR_NEW_FOG,
        FLOAT_MAX,
    };
    use zomboid_engine::zombie::iso::weather::weather_period::WeatherPeriod;

    let lua_path = root.join("media/lua/server/Climate/ClimateMain.lua");
    let lua = std::fs::read_to_string(&lua_path).map_err(|e| format!("{}: {e}", lua_path.display()))?;
    let b = |v: bool| u8::from(v);
    let col = |c: &ClimateColorInfo| {
        format!(
            "{} {} {} {} {} {} {} {}",
            f(c.interior[0]),
            f(c.interior[1]),
            f(c.interior[2]),
            f(c.interior[3]),
            f(c.exterior[0]),
            f(c.exterior[1]),
            f(c.exterior[2]),
            f(c.exterior[3])
        )
    };
    let front = |fr: &AirFront| {
        format!(
            "{} {} {} {} {} {} {} {}",
            fr.type_,
            f(fr.strength),
            f(fr.days),
            f(fr.max_noise),
            f(fr.total_noise),
            f(fr.noise_cache[0]),
            f(fr.noise_cache[1]),
            f(fr.front_wind_angle_degrees)
        )
    };
    let fv = |v: &ForecastValue| {
        [
            v.day_min,
            v.day_max,
            v.day_mean,
            v.night_min,
            v.night_max,
            v.night_mean,
            v.total_min,
            v.total_max,
            v.total_mean,
        ]
        .iter()
        .map(|x| f(*x))
        .collect::<Vec<_>>()
        .join(",")
    };
    let stages = |wp: &WeatherPeriod| {
        let l: Vec<String> = wp.weather_stages.iter().map(|s| s.stage_id.to_string()).collect();
        format!("[{}]", l.join(","))
    };
    let forecasts = |sb: &mut String, cm: &ClimateManager| {
        let fc = &cm.climate_forecaster;
        for (i, &id) in fc.forecasts.iter().enumerate() {
            let df = &fc.days[id];
            let wp = &df.weather_period;
            let list: Vec<String> = df.weather_stages.iter().map(|x| x.to_string()).collect();
            let _ = writeln!(
                sb,
                "FC {i} offset={} starts={} start={} end={} snow={} front={} fog={} fogStr={} fogDur={} overlap={} heavy={} storm={} tropical={} blizzard={} dawn={} dusk={} daylight={} stages=[{}]",
                df.index_offset,
                b(df.weather_starts),
                f(df.weather_start_time),
                f(df.weather_end_time),
                b(df.chance_on_snow),
                df.air_front_string,
                b(df.has_fog),
                f(df.fog_strength),
                f(df.fog_duration),
                fc.index_of(df.weather_overlap),
                b(df.has_heavy_rain),
                b(df.has_storm),
                b(df.has_tropical_storm),
                b(df.has_blizzard),
                f(df.dawn),
                f(df.dusk),
                f(df.day_light_hours),
                list.join(", ")
            );
            let _ = writeln!(
                sb,
                " FCA {} wp={} dur={} str={} angle={} {}",
                front(&df.air_front),
                b(wp.is_running),
                d(wp.duration),
                f(wp.get_total_strength()),
                f(wp.get_wind_angle_degrees()),
                stages(wp)
            );
            let _ = writeln!(
                sb,
                " FCV t={} h={} wd={} wp={} c={}",
                fv(&df.temperature),
                fv(&df.humidity),
                fv(&df.wind_direction),
                fv(&df.wind_power),
                fv(&df.cloudiness)
            );
        }
    };

    let mut sb = String::new();
    for spec in scenarios {
        let p: Vec<&str> = spec.split(':').collect();
        if p.len() != 6 {
            return Err(format!("scenario {spec}: seed:year:month:day:hour:days"));
        }
        let num = |s: &str| s.parse::<i64>().map_err(|e| format!("{s}: {e}"));
        let seed = num(p[0])?;
        let (year, month, day) = (num(p[1])? as i32, num(p[2])? as i32, num(p[3])? as i32);
        let hour: f32 = p[4].parse().map_err(|e| format!("{}: {e}", p[4]))?;
        let days = num(p[5])?;
        let _ = writeln!(sb, "SCENARIO {spec}");

        let mut rand = RandAbstract::new(java::Random::new(seed));
        let mut gt = GameTime::default();
        gt.year = year;
        gt.month = month;
        gt.day = day;
        gt.time_of_day = hour;
        gt.nights_survived = 0;
        gt.minutes_per_day = 60.0;
        gt.multiplier = 1.0;
        gt.fps_multiplier = 2.0;
        gt.multiplier_bias = 1.0;
        gt.per_object_multiplier = 1.0;
        gt.set_minutes_stamp();

        let env = ClimateEnv {
            server: true,
            client: false,
            debug: false,
            rain_modifier: 3,
            climate_cycle: 1,
            fog_cycle: 1,
            day_night_cycle: 1,
            fog_quality: 0,
            weather_shader: false,
            no_players: true,
            disable_lightning_during_storms: false,
        };
        let grid = [-250, 250, -250, 250];

        // ErosionMain's season (temperature option 3 = normal), set to the date as ErosionMain.mainTimer does.
        let mut season = erosion_main_season(3, env.day_night_cycle);
        let mut e_date = [gt.day, gt.month, gt.year];
        season.set_day(e_date[0], e_date[1], e_date[2]);
        let _ = writeln!(
            sb,
            "SETUP server=1 fogQuality={} multiplier={} unmodded={} rain={} climateCycle={} fogCycle={} dayNight={} gameMode=Multiplayer weatherShader={} offscreen=0 debug={} forceSnow=0 grid={},{},{},{}",
            env.fog_quality,
            f(gt.get_multiplier()),
            f(gt.get_unmodded_multiplier()),
            env.rain_modifier,
            env.climate_cycle,
            env.fog_cycle,
            env.day_night_cycle,
            b(env.weather_shader),
            b(env.debug),
            grid[0],
            grid[1],
            grid[2],
            grid[3]
        );
        let _ = writeln!(
            sb,
            "SEASONCFG {} {} {} {} {} {} {} {} {} {}",
            season.get_lat(),
            season.get_temp_max(),
            season.get_temp_min(),
            season.get_temp_diff(),
            season.get_season_lag(),
            f(season.get_high_noon()),
            season.get_seed_a(),
            season.get_seed_b(),
            season.get_seed_c(),
            f(season.get_rain_year_average())
        );

        let mut cm = ClimateManager::new(&mut rand);
        apply_climate_main(&mut cm, &lua)?;
        cm.init(
            season,
            grid[0],
            grid[1],
            grid[2],
            grid[3],
            "Multiplayer",
            &gt,
            &env,
        );
        let ts = &cm.thunder_storm;
        let _ = writeln!(
            sb,
            "OFFSETS {} {} {} {} map={},{},{},{} rand={}",
            d(cm.simplex_offset_a),
            d(cm.simplex_offset_b),
            d(cm.simplex_offset_c),
            d(cm.simplex_offset_d),
            ts.map_min_x,
            ts.map_max_x,
            ts.map_min_y,
            ts.map_max_y,
            rand.id()
        );
        let _ = writeln!(
            sb,
            "COL night {} | noMoon {} | moon {}",
            col(&cm.col_night),
            col(&cm.col_night_no_moon),
            col(&cm.col_night_moon)
        );
        let _ = writeln!(
            sb,
            "COL fog {} | legacy {} | new {}",
            col(&cm.col_fog),
            col(&cm.col_fog_legacy),
            col(&cm.col_fog_new)
        );
        let _ = writeln!(
            sb,
            "COL storm {} | tropical {}",
            col(&cm.fog_tint_storm),
            col(&cm.fog_tint_tropical)
        );
        let wpm = &cm.weather_period;
        let _ = writeln!(
            sb,
            "COL clouds {} | {} | {} | {} | {} | {}",
            col(&wpm.cloud_color_reddish),
            col(&wpm.cloud_color_greenish),
            col(&wpm.cloud_color_blueish),
            col(&wpm.cloud_color_purplish),
            col(&wpm.cloud_color_tropical),
            col(&wpm.cloud_color_blizzard)
        );
        for seg in 0..3i32 {
            for temp in 0..3usize {
                for s in 0..4usize {
                    let c = cm
                        .get_season_color(seg, temp, s)
                        .ok_or("season colour out of range")?;
                    let _ = writeln!(sb, "SEASONCOL {seg} {temp} {s} {}", col(c));
                }
            }
        }
        forecasts(&mut sb, &cm);

        let mut minutes_mod =
            ((gt.time_of_day - gt.time_of_day as i32 as f32) * 60.0) as i32 / 10;
        let mut last_start = f64::NAN;
        let mut last_running = false;
        let ticks = days * 108_000;
        for t in 1..=ticks {
            if gt.update_clock(&mut minutes_mod) {
                if gt.day != e_date[0] || gt.month != e_date[1] || gt.year != e_date[2] {
                    e_date = [gt.day, gt.month, gt.year];
                    cm.season.set_day(e_date[0], e_date[1], e_date[2]);
                }
                cm.update_every_ten_mins();
            }
            cm.update(&mut gt, &env, &mut rand);
            if !cm.tick_is_climate_tick {
                continue;
            }
            let _ = writeln!(
                sb,
                "T {t} {}-{}-{} {} age={} stamp={} hour={} day={} rand={}",
                gt.year,
                gt.month,
                gt.day,
                f(gt.time_of_day),
                d(gt.get_world_age_hours()),
                gt.minutes_stamp,
                b(cm.tick_is_hour_change),
                b(cm.tick_is_day_change),
                rand.id()
            );
            let mut v = String::from("V");
            let mut i_ = String::from("I");
            for i in 0..FLOAT_MAX {
                let cf = cm.get_climate_float(i);
                v.push(' ');
                v.push_str(&f(cf.final_value));
                i_.push(' ');
                i_.push_str(&f(cf.internal_value));
            }
            let _ = writeln!(sb, "{v}");
            let _ = writeln!(sb, "{i_}");
            let _ = writeln!(
                sb,
                "G {} | {} | {} | snow={}{}",
                col(&cm.get_climate_color(COLOR_GLOBAL_LIGHT).final_value),
                col(&cm.get_climate_color(COLOR_GLOBAL_LIGHT).internal_value),
                col(&cm.get_climate_color(COLOR_NEW_FOG).final_value),
                b(cm.get_climate_bool(BOOL_IS_SNOW).final_value),
                b(cm.get_climate_bool(BOOL_IS_SNOW).internal_value)
            );
            let cs = cm.get_season();
            let _ = writeln!(
                sb,
                "X {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} season={},{} moon={} view={},{} front={}",
                f(cm.air_mass),
                f(cm.air_mass_daily),
                f(cm.air_mass_temperature),
                f(cm.base_temperature),
                f(cm.snow_fall),
                f(cm.snow_strength),
                f(cm.snow_melt_strength),
                f(cm.snow_frac_now),
                b(cm.can_do_winter_sprites),
                b(cm.day_do_fog),
                f(cm.day_fog_strength),
                f(cm.fog_lerp_value),
                f(cm.wind_power),
                f(cm.day_light_lagged),
                f(cm.night_lagged),
                cs.get_season(),
                f(cs.get_season_progression()),
                cm.moon.get_current_moon_phase(),
                f(gt.view_dist_min),
                f(gt.view_dist_max),
                front(&cm.current_front)
            );
            let wp = &cm.weather_period;
            let start = wp.start_time;
            if wp.is_running != last_running || start != last_start {
                last_running = wp.is_running;
                last_start = start;
                let _ = writeln!(
                    sb,
                    "WP start={} running={} dur={} front={} dirMod={} cloud={}",
                    d(start),
                    b(wp.is_running),
                    d(wp.duration),
                    front(&wp.front_cache),
                    f(wp.wind_angle_dir_mod),
                    col(wp.cloud_color())
                );
                for (i, s) in wp.weather_stages.iter().enumerate() {
                    let _ = writeln!(
                        sb,
                        " WS {i} {} {} {} {} {} {} {} {} {} {}",
                        s.stage_id,
                        d(s.stage_start),
                        d(s.stage_end),
                        d(s.stage_duration),
                        f(s.entry_strength),
                        f(s.exit_strength),
                        f(s.target_strength),
                        s.lerp_mid_val.map_or("null", |v| v.name()),
                        s.lerp_end_val.map_or("null", |v| v.name()),
                        f(s.fog_strength)
                    );
                }
            }
            if wp.is_running {
                let cur = wp.current_stage.map(|i| &wp.weather_stages[i]);
                let stage = match cur {
                    None => "null".to_string(),
                    Some(s) => format!(
                        "{} {} {} {} {} {} {}",
                        s.stage_id,
                        f(s.linear_t),
                        f(s.parabolic_t),
                        b(s.is_cycle_first_half),
                        b(s.has_started_cloud),
                        f(s.entry_strength),
                        f(s.exit_strength)
                    ),
                };
                let _ = writeln!(
                    sb,
                    "W {} {} str={} rain={} noise={} total={} stage={} precip={} tinf={} flags={}{}{} cloud={}",
                    wp.weather_stage_index,
                    stage,
                    f(wp.current_strength),
                    f(wp.rain_threshold),
                    d(f64::from(wp.weather_noise)),
                    f(wp.total_progress),
                    f(wp.stage_progress),
                    f(wp.precipitation_final),
                    f(wp.temperature_influence),
                    b(wp.is_thunder_storm),
                    b(wp.is_tropical_storm),
                    b(wp.is_blizzard),
                    col(wp.cloud_color())
                );
            }
            let ts = &cm.thunder_storm;
            for (i, c) in ts.clouds.iter().enumerate() {
                if !c.is_running && c.start_time == 0.0 {
                    continue;
                }
                let st = &c.suspend_timer;
                let _ = writeln!(
                    sb,
                    "TC {i} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} timer={},{},{},{} strike={}",
                    b(c.is_running),
                    c.current_x,
                    c.current_y,
                    c.start_x,
                    c.start_y,
                    c.end_x,
                    c.end_y,
                    d(c.start_time),
                    d(c.end_time),
                    d(c.duration),
                    f(c.strength),
                    f(c.angle),
                    f(c.radius),
                    f(c.event_frequency),
                    f(c.thunder_ratio),
                    f(c.percentage_offset),
                    f(st.elapsed),
                    f(st.duration),
                    b(st.finished),
                    st.ticks,
                    ts.strike_radius
                );
            }
            if cm.tick_is_day_change {
                forecasts(&mut sb, &cm);
            }
        }
        let _ = writeln!(sb, "END {spec} rand={}", rand.id());
    }
    Ok(sb)
}

/// The save's `map_t.bin` through `GameTime.load` as a single-player game reads it, into a new game time and climate;
/// what the load sets, in GameTimeProbe.java's format.
pub fn gametime(save: &Path) -> Result<String, String> {
    use save_dump::{dh, fh, table};
    use zomboid_engine::zombie::game_time::{CorePoisonous, GameTime};
    use zomboid_engine::zombie::iso::map_meta::MetaSide;
    use zomboid_engine::zombie::iso::weather::climate_color_info::ClimateColorInfo;
    use zomboid_engine::zombie::iso::weather::climate_manager::{AirFront, ClimateManager};
    let p = save.join("map_t.bin");
    let data = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut gt = GameTime::default();
    let mut core = CorePoisonous::default();
    let mut cm = ClimateManager::default();
    let mut log = Vec::new();
    gt.load(
        &data,
        -1,
        MetaSide::SinglePlayer,
        &mut core,
        &mut cm,
        &mut log,
    )?;
    for l in &log {
        eprintln!("{l}");
    }
    let b = |v: bool| u8::from(v);
    let null = |s: &Option<String>| s.clone().unwrap_or_else(|| "null".to_string());
    let front = |f: &AirFront| {
        format!(
            "{} {} {} {} {} {} {} {}",
            fh(f.days),
            fh(f.max_noise),
            fh(f.total_noise),
            f.type_,
            fh(f.strength),
            fh(f.front_wind_angle_degrees),
            fh(f.noise_cache[0]),
            fh(f.noise_cache[1])
        )
    };
    let color = |c: &ClimateColorInfo| {
        c.interior
            .iter()
            .chain(c.exterior.iter())
            .map(|v| format!(" {}", fh(*v)))
            .collect::<String>()
    };
    let idx = |i: Option<usize>| i.map_or(-1, |i| i as i64);
    let mut sb = String::new();
    let _ = writeln!(
        sb,
        "GT {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
        fh(gt.multiplier),
        gt.nights_survived,
        gt.target_zombies,
        fh(gt.last_time_of_day),
        fh(gt.time_of_day),
        gt.day,
        gt.month,
        gt.year,
        gt.helicopter_day1,
        gt.helicopter_time1_start,
        gt.helicopter_time1_end,
        gt.minutes_stamp,
        gt.get_hour(),
        gt.get_minutes(),
        dh(gt.get_world_age_hours())
    );
    let _ = writeln!(
        sb,
        "GTT {}",
        gt.table.as_ref().map_or("null".to_string(), table)
    );
    let _ = writeln!(
        sb,
        "CORE {} {}",
        null(&core.poisonous_berry),
        null(&core.poisonous_mushroom)
    );
    let _ = writeln!(
        sb,
        "CM {} {} {} {} {} {} {} {} {}",
        dh(cm.simplex_offset_a),
        dh(cm.simplex_offset_b),
        dh(cm.simplex_offset_c),
        dh(cm.simplex_offset_d),
        fh(cm.snow_frac_now),
        fh(cm.snow_strength),
        b(cm.can_do_winter_sprites),
        b(cm.day_do_fog),
        fh(cm.day_fog_strength)
    );
    let _ = writeln!(sb, "CF {}", front(&cm.current_front));
    let wp = &cm.weather_period;
    let _ = writeln!(sb, "WP {}", b(wp.is_running));
    if wp.is_running {
        let _ = writeln!(
            sb,
            "WPR {} {} {} {} {} {} {} {} {} {}",
            wp.weather_stage_index,
            fh(wp.current_strength),
            fh(wp.rain_threshold),
            b(wp.is_thunder_storm),
            b(wp.is_tropical_storm),
            b(wp.is_blizzard),
            fh(wp.wind_angle_dir_mod),
            dh(wp.duration),
            wp.weather_stages.len(),
            idx(wp.current_stage)
        );
        let _ = writeln!(sb, "WPF {}", front(&wp.front_cache));
        for (i, s) in wp.weather_stages.iter().enumerate() {
            let lerp = |v: Option<_>| {
                v.map_or(
                    "null",
                    |v: zomboid_engine::zombie::iso::weather::weather_period::StrLerpVal| v.name(),
                )
            };
            let _ = writeln!(
                sb,
                "WS {i} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
                s.stage_id,
                dh(s.stage_start),
                dh(s.stage_end),
                dh(s.stage_duration),
                fh(s.entry_strength),
                fh(s.exit_strength),
                fh(s.target_strength),
                lerp(s.lerp_mid_val),
                lerp(s.lerp_end_val),
                b(s.has_started_cloud),
                null(&s.mod_id),
                fh(s.fog_strength),
                idx(s.previous_stage),
                idx(s.next_stage),
                b(s.creation_finished)
            );
        }
        let _ = writeln!(sb, "WPC{}", color(wp.cloud_color()));
    }
    for (i, c) in cm.thunder_storm.clouds.iter().enumerate() {
        if c.is_running {
            let _ = writeln!(
                sb,
                "TS {i} 1 {} {} {} {} {} {} {} {} {} {} {} {}",
                c.start_x,
                c.start_y,
                c.end_x,
                c.end_y,
                fh(c.radius),
                fh(c.angle),
                fh(c.strength),
                fh(c.thunder_ratio),
                dh(c.start_time),
                dh(c.end_time),
                dh(c.duration),
                fh(c.percentage_offset)
            );
        } else {
            let _ = writeln!(sb, "TS {i} 0");
        }
    }
    let _ = writeln!(
        sb,
        "CMT {}",
        cm.mod_data_table.as_ref().map_or("null".to_string(), table)
    );
    Ok(sb)
}

/// The save's `global_mod_data.bin` through `GlobalModData.load` and `iTrack.bin` through `InstanceTracker.load`;
/// the tables in `collectTableNames` order, the groups in their HashMap order, each exported as text and CSV with keys
/// sorted (pairs with ModDataProbe.java).
pub fn moddata(save: &Path) -> Result<String, String> {
    use zomboid_engine::java::hash_map_order;
    use zomboid_engine::zombie::iso::instance_tracker::{Format, InstanceTracker, Sort};
    use zomboid_engine::zombie::world::moddata::global_mod_data::GlobalModData;
    let read = |name: &str| {
        let p = save.join(name);
        std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))
    };
    let gmd_data = read("global_mod_data.bin")?;
    let it_data = read("iTrack.bin")?;
    let mut sb = String::new();
    let mut gmd = GlobalModData::default();
    gmd.load(Some(&gmd_data))?;
    let tags = gmd.collect_table_names();
    let _ = writeln!(sb, "GMD {}", tags.len());
    for t in &tags {
        let table = gmd.get(t).map_or("null".to_string(), save_dump::table);
        let _ = writeln!(sb, "GMT {t} {table}");
    }
    let mut it = InstanceTracker::default();
    it.load(Some(&it_data))?;
    let _ = writeln!(sb, "IG {}", it.instance_groups.len());
    let names: Vec<&str> = it.instance_groups.iter().map(|(g, _)| g.as_str()).collect();
    for i in hash_map_order(&names) {
        let g = names[i];
        let _ = writeln!(sb, "IGN {g}");
        sb.push_str(&it.export_group(g, Format::Text, Sort::Key, "\n"));
        sb.push_str(&it.export_group(g, Format::Csv, Sort::Key, "\n"));
    }
    Ok(sb)
}

/// The outfits of `media/clothing/clothing.xml` (OutfitManager.init) sorted by name, and PersistentOutfits built from
/// them with the save's `z_outfits.bin` loaded (pairs with OutfitsProbe.java).
pub fn outfits(root: &Path, save: &Path) -> Result<String, String> {
    use save_dump::fh;
    use zomboid_engine::java::{equals_ignore_case, string_compare_to};
    use zomboid_engine::zombie::core::skinnedmodel::population::clothing_item_reference::ClothingItemReference;
    use zomboid_engine::zombie::core::skinnedmodel::population::outfit::Outfit;
    use zomboid_engine::zombie::core::skinnedmodel::population::outfit_manager::OutfitManager;
    use zomboid_engine::zombie::persistent_outfits::{PersistentOutfits, FEMALE_BIT, NO_HAT_BIT};
    fn item(sb: &mut String, r: &ClothingItemReference, depth: usize) {
        let _ = writeln!(
            sb,
            "I {depth} {} {} {} {} {}",
            fh(r.probability),
            r.item_guid.as_deref().unwrap_or("null"),
            u8::from(r.randomized),
            u8::from(r.immutable),
            r.sub_items.len()
        );
        for s in &r.sub_items {
            item(sb, s, depth + 1);
        }
    }
    let om = OutfitManager::init(root)?.ok_or("no media/clothing/clothing.xml")?;
    let mut sb = String::new();
    let mut names: Vec<String> = Vec::new();
    for female in [true, false] {
        let mut l: Vec<&Outfit> = if female {
            om.female_outfits.iter().collect()
        } else {
            om.male_outfits.iter().collect()
        };
        l.sort_by(|a, b| string_compare_to(&a.name, &b.name));
        let map_len = if female {
            om.female_outfit_map_len()
        } else {
            om.male_outfit_map_len()
        };
        let _ = writeln!(
            sb,
            "OM {} {} {map_len}",
            if female { "F" } else { "M" },
            l.len()
        );
        for o in l {
            if !names.iter().any(|n| equals_ignore_case(n, &o.name)) {
                names.push(o.name.clone());
            }
            let same = om.get_specific_outfit(female, &o.name);
            let upper = om.get_specific_outfit(female, &o.name.to_uppercase());
            let _ = writeln!(
                sb,
                "O {} {} {} {} {} {} {} {} {} [{}] [{}] {} {} {}",
                o.name,
                u8::from(o.top),
                u8::from(o.pants),
                u8::from(o.allow_pants_hue),
                u8::from(o.allow_pants_tint),
                u8::from(o.allow_top_tint),
                u8::from(o.allow_tshirt_decal),
                o.mod_id.as_deref().unwrap_or("null"),
                u8::from(o.immutable),
                o.top_textures.join(","),
                o.pants_textures.join(","),
                o.items.len(),
                u8::from(same.is_some_and(|s| std::ptr::eq(s, o))),
                u8::from(upper.is_some_and(|s| std::ptr::eq(s, o)))
            );
            for r in &o.items {
                item(&mut sb, r, 0);
            }
        }
    }
    let p = save.join("z_outfits.bin");
    let data = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut po = PersistentOutfits::default();
    po.init(&om, false, &mut |_| 0);
    po.load(&data)?;
    for (i, s) in po.seeds.iter().enumerate() {
        let _ = writeln!(sb, "PS {i} {s}");
    }
    let k = names.len();
    let _ = writeln!(sb, "PA {k}");
    for i in 0..k {
        let d = &po.all[i];
        let _ = writeln!(
            sb,
            "PD {i} {} {} {} {}",
            d.index,
            d.outfit_name,
            u8::from(d.use_seed),
            po.outfit_names[i]
        );
    }
    for (sex, l, n) in [
        ("F", &po.female, om.female_outfits.len()),
        ("M", &po.male, om.male_outfits.len()),
    ] {
        let _ = writeln!(sb, "PL {sex} {n}");
        let line: Vec<String> = l[..n]
            .iter()
            .map(|&i| po.all[i].index.to_string())
            .collect();
        let _ = writeln!(sb, "{}", line.join(" "));
    }
    let mut no_draw = |_: i32| -> i32 { panic!("getOutfit drew a random variant") };
    for i in [0i32, 1, k as i32 - 1, 32767] {
        for v in [1, 250, 500] {
            for bits in [0, FEMALE_BIT, NO_HAT_BIT] {
                let id = bits | (i << 16) | v;
                let _ = writeln!(sb, "GO {id} {}", po.get_outfit(id, &mut no_draw));
            }
        }
    }
    let _ = writeln!(sb, "GO 0 {}", po.get_outfit(0, &mut no_draw));
    Ok(sb)
}

/// The file guid table (`media/fileGuidTable.xml`) with its lookups in other letter case, then
/// `OutfitManager.getClothingItem` for every guid in table order (pairs with ClothingItemsProbe.java).
pub fn clothingitems(root: &Path) -> Result<String, String> {
    use zomboid_engine::zombie::core::skinnedmodel::population::outfit_manager::OutfitManager;
    use zomboid_engine::zombie::file_guid_table::FileGuidTable;
    let t = FileGuidTable::load_game(root)?;
    let mut om = OutfitManager::init(root)?.ok_or("no media/clothing/clothing.xml")?;
    let b = |v: bool| u8::from(v);
    let s = |v: &Option<String>| v.clone().unwrap_or_else(|| "null".to_string());
    let l = |v: &[String]| format!("[{}]", v.join(","));
    let mut sb = String::new();
    let _ = writeln!(sb, "FGT {}", t.files.len());
    for p in &t.files {
        let (path, guid) = (s(&p.path), s(&p.guid));
        let _ = writeln!(
            sb,
            "FG {path} {guid} {} {}",
            b(t.get_file_path_from_guid(&guid.to_uppercase()) == Some(path.as_str())),
            b(t.get_guid_from_file_path(&path.to_lowercase()) == Some(guid.as_str()))
        );
    }
    for (i, p) in t.files.iter().enumerate() {
        let Some(ci) = om.get_clothing_item(&s(&p.guid), &t, root) else {
            let _ = writeln!(sb, "CI {i} null");
            continue;
        };
        let masks: Vec<String> = ci.masks.iter().map(|m| m.to_string()).collect();
        let _ = writeln!(
            sb,
            "CI {i} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
            s(&ci.guid),
            s(&ci.mame),
            b(ci.failed),
            s(&ci.male_model),
            s(&ci.female_model),
            s(&ci.alt_male_model),
            s(&ci.alt_female_model),
            b(ci.is_static),
            l(&ci.base_textures),
            s(&ci.attach_bone),
            l(&masks),
            s(&ci.masks_folder),
            s(&ci.underlay_masks_folder),
            l(&ci.texture_choices),
            b(ci.allow_random_hue),
            b(ci.allow_random_tint),
            s(&ci.decal_group),
            s(&ci.shader),
            s(&ci.hat_category),
            l(&ci.spawn_with),
            b(ci.is_hat()),
            b(ci.is_mask()),
            b(ci.has_model())
        );
    }
    Ok(sb)
}

/// Seeded draws that dress a character, in the format of `tests/original_probe/OutfitRandomProbe.java`.
pub fn outfitrandom(root: &Path) -> Result<String, String> {
    use save_dump::fh;
    use zomboid_engine::java::string_compare_to;
    use zomboid_engine::zombie::core::color::Color;
    use zomboid_engine::zombie::core::immutable_color::ImmutableColor;
    use zomboid_engine::zombie::core::skinnedmodel::population::clothing_decals::ClothingDecals;
    use zomboid_engine::zombie::core::skinnedmodel::population::clothing_item_reference::{
        ClothingItemReference, RandomizeContext,
    };
    use zomboid_engine::zombie::core::skinnedmodel::population::outfit::Outfit;
    use zomboid_engine::zombie::core::skinnedmodel::population::outfit_manager::OutfitManager;
    use zomboid_engine::zombie::core::skinnedmodel::population::outfit_rng::OutfitRng;
    use zomboid_engine::zombie::file_guid_table::FileGuidTable;
    use zomboid_engine::zombie::util::location_rng::LocationRng;
    let b = |v: bool| u8::from(v);
    let c = |c: &ImmutableColor| format!("{},{},{},{}", fh(c.r), fh(c.g), fh(c.b), fh(c.a));
    let s = |v: &Option<String>| v.clone().unwrap_or_else(|| "null".to_string());
    fn item(sb: &mut String, r: &ClothingItemReference, depth: usize) {
        let d = &r.random_data;
        let picked = match &d.picked_item_ref {
            Some(path) => r
                .picked(path)
                .and_then(|p| p.item_guid.as_deref())
                .unwrap_or("null"),
            None => "null",
        };
        let t = &d.tint;
        let _ = writeln!(
            sb,
            "R {depth} {} {} {} {} {},{},{},{} {} {picked} {} {}",
            r.item_guid.as_deref().unwrap_or("null"),
            u8::from(r.randomized),
            u8::from(d.active),
            fh(d.hue),
            fh(t.r),
            fh(t.g),
            fh(t.b),
            fh(t.a),
            d.base_texture.as_deref().unwrap_or("null"),
            d.texture_choice.as_deref().unwrap_or("null"),
            d.decal.as_deref().unwrap_or("null"),
        );
        for sub in &r.sub_items {
            item(sb, sub, depth + 1);
        }
    }
    let mut sb = String::new();
    let seeds: [i64; 7] = [
        0,
        1,
        -1,
        12345,
        i64::MIN,
        i64::MAX,
        0x9E37_79B9_7F4A_7C15_u64 as i64,
    ];
    let bounds: [i32; 9] = [1, 2, 7, 100, 1000, 65536, i32::MAX, 0, -5];
    let mut rng = LocationRng::default();
    for seed in seeds {
        rng.set_seed(seed);
        let _ = write!(sb, "LR {seed} {}", rng.get_seed());
        for _ in 0..4 {
            let _ = write!(sb, " {}", fh(rng.next_float()));
        }
        for n in bounds {
            let _ = write!(sb, " {}", rng.next_int(n));
        }
        sb.push('\n');
    }
    for [x, y, z] in [[0, 0, 0], [10500, 9800, 0], [-3, 7, 1], [300000, 200000, 7]] {
        let v = rng.next_int_at(1000, x, y, z);
        let _ = writeln!(sb, "LRS {x} {y} {z} {v} {}", rng.get_seed());
    }
    let hs: [f32; 12] = [
        -1.25,
        -0.5,
        -1.0e-9,
        0.0,
        0.1,
        0.166_666_67,
        0.25,
        0.5,
        0.75,
        0.9999,
        1.0,
        2.3,
    ];
    let ss: [f32; 4] = [0.0, 0.3, 1.0, 1.5];
    let vs: [f32; 4] = [0.0, 0.5, 1.0, -0.2];
    for h in hs {
        for sat in ss {
            for v in vs {
                let col = Color::hsb_to_rgb(h, sat, v);
                let _ = writeln!(
                    sb,
                    "HSB {} {} {} {},{},{},{}",
                    fh(h),
                    fh(sat),
                    fh(v),
                    fh(col.r),
                    fh(col.g),
                    fh(col.b),
                    fh(col.a)
                );
            }
        }
    }
    let mut orng = OutfitRng::default();
    for nbc in [false, true] {
        for seed in 0..20i64 {
            orng.set_seed(seed);
            let first = orng.random_immutable_color(nbc, false);
            let second = orng.random_immutable_color(nbc, true);
            let _ = writeln!(sb, "COL {} {seed} {} {}", b(nbc), c(&first), c(&second));
        }
    }
    let decals = ClothingDecals::init(root).ok_or("no media/clothing/clothingDecals.xml")?;
    let _ = writeln!(sb, "DECALS {}", decals.groups.len());
    for (gi, g) in decals.groups.iter().enumerate() {
        let name = s(&g.name);
        let same = decals
            .find_group(&name.to_uppercase())?
            .is_some_and(|f| std::ptr::eq(f, &decals.groups[gi]));
        let _ = write!(sb, "DG {name} {}", b(same));
        for seed in 0..6i64 {
            orng.set_seed(seed * 31 + 7);
            let _ = write!(sb, " {}", s(&decals.get_random_decal(&name, &mut orng)?));
        }
        sb.push('\n');
    }
    let _ = writeln!(
        sb,
        "DGX {} {}",
        s(&decals.get_random_decal("NoSuchGroup", &mut orng)?),
        s(&decals.get_random_decal(" ", &mut orng)?)
    );
    let table = FileGuidTable::load_game(root)?;
    let mut om = OutfitManager::init(root)?.ok_or("no media/clothing/clothing.xml")?;
    for nbc in [true, false] {
        for female in [true, false] {
            let mut l: Vec<Outfit> = if female {
                om.female_outfits.clone()
            } else {
                om.male_outfits.clone()
            };
            l.sort_by(|a, b| string_compare_to(&a.name, &b.name));
            for (i, base) in l.iter().enumerate() {
                let mut o = base.clone_outfit();
                orng.set_seed(
                    i as i64 * 7919 + if female { 1 } else { 2 } + if nbc { 0 } else { 1_000_003 },
                );
                let mut cx = RandomizeContext {
                    rng: &mut orng,
                    outfits: &mut om,
                    table: &table,
                    game_root: root,
                    decals: Some(&decals),
                    no_black_clothes: nbc,
                };
                for r in &mut o.items {
                    r.randomize(&mut cx)?;
                }
                let after = orng.next(1_000_000);
                let _ = writeln!(
                    sb,
                    "O {} {} {i} {} {} {after}",
                    b(nbc),
                    if female { "F" } else { "M" },
                    o.name,
                    o.items.len()
                );
                for r in &o.items {
                    item(&mut sb, r, 0);
                }
            }
        }
    }
    Ok(sb)
}

/// Every animation set of `media/AnimSets` and every action group of `media/actiongroups`, in the format of
/// `tests/original_probe/AnimSetsProbe.java`.
pub fn animsets(root: &Path) -> Result<String, String> {
    use zomboid_engine::java::{float_to_string, string_compare_to};
    use zomboid_engine::zombie::characters::action::action_group::ActionGroups;
    use zomboid_engine::zombie::characters::action::conditions::character_variable_condition::Value;
    use zomboid_engine::zombie::characters::action::i_action_condition::IActionCondition;
    use zomboid_engine::zombie::core::math::i_interpolator::IInterpolator;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::anim_condition::AnimCondition;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::anim_event::AnimEventKind;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::anim_node::AnimNode;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::animation_set::AnimationSets;
    use zomboid_engine::zombie::core::skinnedmodel::advancedanimation::animation_variable_reference::AnimationVariableReference;
    use zomboid_engine::zombie::zomboid_file_system::{list_files, ZomboidFileSystem};
    fn q(s: Option<&str>) -> String {
        let Some(s) = s else {
            return "null".into();
        };
        let mut b = String::from("\"");
        for c in s.encode_utf16() {
            if (0x20..0x7f).contains(&c) && c != u16::from(b'"') && c != u16::from(b'\\') {
                b.push(char::from(c as u8));
            } else {
                let _ = write!(b, "\\u{c:04x}");
            }
        }
        b.push('"');
        b
    }
    let f = float_to_string;
    let b = |v: bool| u8::from(v);
    fn e(v: Option<&str>) -> &str {
        v.unwrap_or("null")
    }
    fn reference(r: Option<&AnimationVariableReference>) -> String {
        match r {
            None => "null".into(),
            Some(r) => format!(
                "({},{})",
                q(r.sub_variable_source_name.as_deref()),
                q(Some(&r.name))
            ),
        }
    }
    fn idx(v: Option<usize>) -> String {
        v.map_or("null".into(), |i| i.to_string())
    }
    fn cond(sb: &mut String, indent: &str, tag: &str, i: usize, c: &AnimCondition) {
        let _ = writeln!(
            sb,
            "{indent}{tag} {i} name={} type={} value={} f={} b={} s={} ref={}",
            q(Some(&c.name)),
            e(c.type_.map(|t| t.name())),
            q(Some(&c.value)),
            float_to_string(c.float_value),
            u8::from(c.bool_value),
            q(Some(&c.string_value)),
            reference(c.variable_reference.as_ref())
        );
    }
    let node = |sb: &mut String, tag: &str, n: &AnimNode| {
        let _ = writeln!(
            sb,
            "  {tag} {} idle={} priority={} conditionPriority={} abstract={} parent=1",
            q(Some(&n.name)),
            b(n.is_idle_anim),
            n.priority,
            n.condition_priority,
            b(n.is_abstract())
        );
        let alternates: Vec<String> = n.alternate_anims.iter().map(|s| q(Some(s))).collect();
        let _ = writeln!(
            sb,
            "   anim={} alternates=[{}]",
            q(Some(&n.anim_name)),
            alternates.join(",")
        );
        let _ = writeln!(
            sb,
            "   grapple matching={} fwd={} yaw={} tweenIn={} behaviour={}",
            q(Some(&n.matching_grappled_anim_node)),
            f(n.grapple_offset_forward),
            f(n.grapple_offset_yaw),
            f(n.grapple_tween_in_time),
            e(n.grappler_offset_behaviour.map(|v| v.name()))
        );
        let _ = writeln!(
            sb,
            "   ragdoll={} chance={} startMin={} startMax={} maxTime={}",
            b(n.is_ragdoll),
            f(n.chance_to_ragdoll),
            f(n.ragdoll_start_time_min),
            f(n.ragdoll_start_time_max),
            f(n.ragdoll_max_time)
        );
        let _ = writeln!(
            sb,
            "   deferred bone={} axis={} rotation={} movement={} rotationScale={}",
            q(Some(&n.deferred_bone_name)),
            e(n.deferred_bone_axis.map(|v| v.name())),
            b(n.use_defered_rotation),
            b(n.use_deferred_movement),
            f(n.deferred_rotation_scale)
        );
        let _ = writeln!(
            sb,
            "   looped={} blendTime={} blendOutTime={} stopOnExit={} earlyTransitionOut={}",
            b(n.is_looped),
            f(n.blend_time),
            f(n.blend_out_time),
            b(n.stop_anim_on_exit),
            b(n.early_transition_out)
        );
        let _ = writeln!(
            sb,
            "   speedScale={} variable={} randomMin={} randomMax={} parsed={}",
            q(Some(&n.speed_scale)),
            q(n.speed_scale_variable.as_deref()),
            f(n.speed_scale_random_multiplier_min),
            f(n.speed_scale_random_multiplier_max),
            f(n.speed_scale_f)
        );
        let _ = writeln!(
            sb,
            "   randomAdvance={} maxTorsoTwist={} scalar={} scalar2={} reverse={} sync={} trackTimeTo={}",
            f(n.random_advance_fraction),
            f(n.max_torso_twist),
            q(Some(&n.scalar)),
            q(Some(&n.scalar2)),
            b(n.is_anim_reverse),
            b(n.sync_tracking_enabled),
            q(n.track_time_to_variable.as_deref())
        );
        sb.push_str("   blendCurve ");
        match n.blend_curve.as_ref().and_then(|s| s.interpolator.as_ref()) {
            None => sb.push_str("null"),
            Some(i) => {
                sb.push_str(i.simple_name());
                match i {
                    IInterpolator::Sequence(s) => {
                        let raw: Vec<String> = s
                            .sequence_raw
                            .iter()
                            .map(|p| {
                                format!(
                                    "{}:{}:{}",
                                    f(p.x),
                                    f(p.y),
                                    e(p.lerp_type.map(|t| t.name()))
                                )
                            })
                            .collect();
                        let sorted: Vec<String> = s
                            .sequence_entries
                            .iter()
                            .flatten()
                            .map(usize::to_string)
                            .collect();
                        let _ = write!(
                            sb,
                            " n={} min={} max={} raw=[{}] sorted=[{}]",
                            s.num_entries,
                            f(s.min_x),
                            f(s.max_x),
                            raw.join(","),
                            sorted.join(",")
                        );
                    }
                    IInterpolator::Polyline(p) => {
                        let raw: Vec<String> = p
                            .points_raw
                            .iter()
                            .map(|p| format!("{}:{}", f(p.x), f(p.y)))
                            .collect();
                        let pts: Vec<String> = p
                            .points
                            .iter()
                            .flatten()
                            .map(|p| format!("{}:{}", f(p[0]), f(p[1])))
                            .collect();
                        let _ = write!(
                            sb,
                            " n={} min={} max={} raw=[{}] points=[{}]",
                            p.num_points,
                            f(p.min_x),
                            f(p.max_x),
                            raw.join(","),
                            pts.join(",")
                        );
                    }
                    IInterpolator::Linear(l) => {
                        let _ = write!(sb, " y0={} y1={}", f(l.start_value), f(l.end_value));
                    }
                    IInterpolator::EaseInQuad(l) => {
                        let _ = write!(sb, " y0={} y1={}", f(l.start_value), f(l.end_value));
                    }
                    IInterpolator::EaseOutInQuad(l) => {
                        let _ = write!(sb, " y0={} y1={}", f(l.start_value), f(l.end_value));
                    }
                    IInterpolator::EaseOutQuad(l) => {
                        let _ = write!(sb, " y0={} y1={}", f(l.start_value), f(l.end_value));
                    }
                    IInterpolator::One | IInterpolator::Zero => {}
                }
            }
        }
        sb.push('\n');
        if n.blend_curve
            .as_ref()
            .is_some_and(|s| s.interpolator.is_none())
        {
            sb.push_str("   blendCurveSlot empty\n");
        }
        for (i, bl) in n.blends2d.iter().enumerate() {
            let _ = writeln!(
                sb,
                "   blend {i} anim={} x={} y={} speed={} id={}",
                q(Some(&bl.anim_name)),
                f(bl.pos_x),
                f(bl.pos_y),
                f(bl.speed_scale),
                q(bl.reference_id.as_deref())
            );
        }
        for (i, c) in n.conditions.iter().enumerate() {
            cond(sb, "   ", "cond", i, c);
        }
        for (i, ev) in n.events.iter().enumerate() {
            let class = match ev.kind {
                AnimEventKind::Plain => "AnimEvent",
                AnimEventKind::SetVariable { .. } => "AnimEventSetVariable",
                AnimEventKind::FlagWhileAlive { .. } => "AnimEventFlagWhileAlive",
            };
            let _ = write!(
                sb,
                "   event {i} {class} name={} time={} pc={} param={} parent=1",
                q(ev.event_name.as_deref()),
                e(ev.time.map(|t| t.name())),
                f(ev.time_pc),
                q(ev.parameter_value.as_deref())
            );
            match &ev.kind {
                AnimEventKind::Plain => {}
                AnimEventKind::SetVariable {
                    variable_reference,
                    set_variable_value,
                } => {
                    let _ = write!(
                        sb,
                        " ref={} set={}",
                        reference(Some(variable_reference)),
                        q(Some(set_variable_value))
                    );
                }
                AnimEventKind::FlagWhileAlive {
                    variable_reference,
                    flag_value,
                } => {
                    let _ = write!(
                        sb,
                        " ref={} flag={}",
                        reference(Some(variable_reference)),
                        b(*flag_value)
                    );
                }
            }
            sb.push('\n');
        }
        for (i, t) in n.blend_tris.iter().enumerate() {
            let _ = writeln!(
                sb,
                "   tri {i} {} {} {}",
                idx(t.node1),
                idx(t.node2),
                idx(t.node3)
            );
        }
        match &n.blend2d_picker {
            None => sb.push_str("   picker null\n"),
            Some(p) => {
                let mut hull = p.hull.clone();
                hull.sort_unstable();
                let hull: Vec<String> = hull.iter().map(usize::to_string).collect();
                let _ = writeln!(sb, "   picker tris=1 hull=[{}]", hull.join(","));
            }
        }
        for (i, t) in n.transitions.iter().enumerate() {
            let _ = writeln!(
                sb,
                "   trans {i} source={} target={} anim={} bone={} axis={} rotation={} movement={} rotationScale={} syncAdjust={} in={} out={} speed={} parsed={}",
                q(t.source.as_deref()),
                q(t.target.as_deref()),
                q(t.anim_name.as_deref()),
                q(t.deferred_bone_name.as_deref()),
                e(t.deferred_bone_axis.map(|v| v.name())),
                b(t.use_defered_rotation),
                b(t.use_deferred_movement),
                f(t.deferred_rotation_scale),
                f(t.sync_adjust_time),
                f(t.blend_in_time),
                f(t.blend_out_time),
                f(t.speed_scale),
                b(t.is_parsed)
            );
            for (j, c) in t.conditions.iter().enumerate() {
                cond(sb, "    ", "tcond", j, c);
            }
        }
        for (i, w) in n.sub_state_bone_weights.iter().enumerate() {
            let _ = writeln!(
                sb,
                "   bone {i} {} weight={} descendants={}",
                q(w.bone_name.as_deref()),
                f(w.weight),
                b(w.include_descendants)
            );
        }
        let _ = writeln!(sb, "   transitionOut={}", idx(n.transition_out));
    };

    let fs = ZomboidFileSystem::init(root)?;
    let mut sb = String::new();
    let dir_names = |dir: &Path| -> Vec<String> {
        let mut names: Vec<String> = list_files(dir)
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.is_dir())
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        names.sort_by(|a, b| string_compare_to(a, b));
        names
    };
    let mut sets = AnimationSets::default();
    for name in dir_names(&fs.base().join("media/AnimSets")) {
        if let Err(err) = sets.get_animation_set(&fs, &name, false) {
            let _ = writeln!(sb, "SETERROR {} {err}", q(Some(&name)));
        }
    }
    let mut set_keys: Vec<&String> = sets.set_map.keys().collect();
    set_keys.sort_by(|a, b| string_compare_to(a, b));
    for key in set_keys {
        let set = &sets.set_map[key];
        let _ = writeln!(
            sb,
            "ANIMSET {} name={} states={}",
            q(Some(key)),
            q(Some(&set.name)),
            set.states.len()
        );
        let mut state_keys: Vec<&String> = set.states.keys().collect();
        state_keys.sort_by(|a, b| string_compare_to(a, b));
        for sk in state_keys {
            let st = &set.states[sk];
            let _ = writeln!(
                sb,
                " STATE {} name={} set=1 default={} nodes={} abstract={}",
                q(Some(sk)),
                q(Some(&st.name)),
                st.default_index,
                st.nodes.len(),
                st.abstract_nodes.len()
            );
            for (i, n) in st.nodes.iter().enumerate() {
                node(&mut sb, &format!("NODE {i}"), n);
            }
            for (i, n) in st.abstract_nodes.iter().enumerate() {
                node(&mut sb, &format!("ABSTRACT {i}"), n);
            }
        }
    }
    let base = format!("{}/", fs.base().to_string_lossy());
    let mut failed: Vec<String> = sets
        .asset_manager
        .assets
        .iter()
        .filter(|(_, n)| n.is_none())
        .map(|(p, _)| format!("{} failed", p.strip_prefix(&base).unwrap_or(p)))
        .collect();
    failed.sort_by(|a, b| string_compare_to(a, b));
    let _ = writeln!(sb, "ASSETS {}", sets.asset_manager.assets.len());
    for s in failed {
        let _ = writeln!(sb, "ASSET {}", q(Some(&s)));
    }
    let mut groups = ActionGroups::default();
    for name in dir_names(&fs.base().join("media/actiongroups")) {
        groups.get_action_group(&fs, &name);
    }
    fn strings(v: Option<&Vec<String>>) -> String {
        match v {
            None => "null".into(),
            Some(v) => {
                let s: Vec<String> = v.iter().map(|s| q(Some(s))).collect();
                format!("[{}]", s.join(","))
            }
        }
    }
    fn value(v: Option<&Value>) -> String {
        match v {
            None => "null".into(),
            Some(Value::String(s)) => format!("s{}", q(Some(s))),
            Some(Value::Integer(i)) => format!("i{i}"),
            Some(Value::Float(x)) => format!("f{}", float_to_string(*x)),
            Some(Value::Boolean(x)) => format!("b{x}"),
            Some(Value::Lookup(r)) => format!("lookup{}", reference(Some(r))),
        }
    }
    let mut group_keys: Vec<&String> = groups.action_group_map.keys().collect();
    group_keys.sort_by(|a, b| string_compare_to(a, b));
    for key in group_keys {
        let g = &groups.action_group_map[key];
        let _ = writeln!(
            sb,
            "ACTIONGROUP {} name={} initial={} initialState={} states={}",
            q(Some(key)),
            q(Some(&g.name)),
            q(g.initial_state_name.as_deref()),
            g.get_initial_state()
                .map_or("null".into(), |s| q(Some(&s.name))),
            g.states.len()
        );
        for (i, s) in g.states.iter().enumerate() {
            let vars: Vec<_> = s.state_variables.get_game_variables().collect();
            let _ = writeln!(
                sb,
                " ASTATE {i} {} group=1 grappler={} tags={} childTags={} vars={} transitions={}",
                q(Some(&s.name)),
                b(s.is_grappler_state),
                strings(s.tags.as_ref()),
                strings(s.child_tags.as_ref()),
                vars.len(),
                s.transitions.len()
            );
            for slot in vars {
                let _ = writeln!(
                    sb,
                    "  var {} {} {} value={} readOnly={}",
                    q(Some(&slot.key)),
                    slot.class_name(),
                    slot.type_name(),
                    q(slot.get_value_string().as_deref()),
                    b(slot.read_only)
                );
            }
            for (j, t) in s.transitions.iter().enumerate() {
                let _ = writeln!(
                    sb,
                    "  atrans {j} to={} asSubstate={} transitionOut={} forceParent={} priority={} conditions={}",
                    q(t.transition_to.as_deref()),
                    b(t.as_substate),
                    b(t.transition_out),
                    b(t.force_parent),
                    t.condition_priority,
                    t.conditions.len()
                );
                for c in &t.conditions {
                    sb.push_str("    acond ");
                    match c {
                        IActionCondition::CharacterVariable(cv) => {
                            let _ = write!(
                                sb,
                                "CharacterVariableCondition op={} lhs={} rhs={}",
                                e(cv.op.map(|o| o.name())),
                                value(cv.lhs_value.as_ref()),
                                value(cv.rhs_value.as_ref())
                            );
                        }
                        IActionCondition::EventOccurred(n) => {
                            let _ = write!(sb, "EventOccurred {}", q(Some(n)));
                        }
                        IActionCondition::EventNotOccurred(n) => {
                            let _ = write!(sb, "EventNotOccurred {}", q(Some(n)));
                        }
                        IActionCondition::LuaCall => sb.push_str("LuaCall"),
                    }
                    sb.push('\n');
                }
            }
        }
        let mut lookup: Vec<(&String, &usize)> = g.state_lookup.iter().collect();
        lookup.sort_by(|a, b| string_compare_to(a.0, b.0));
        for (k, i) in lookup {
            let _ = writeln!(sb, " LOOKUP {} {i}", q(Some(k)));
        }
        let mut names: Vec<(&i32, &String)> = g.state_name_lookup.iter().collect();
        names.sort_by_key(|(h, _)| **h);
        for (h, n) in names {
            let _ = writeln!(sb, " NAMELOOKUP {h} {}", q(Some(n)));
        }
    }
    Ok(sb)
}

/// The Kahlua compiler over every `media/lua/**/*.lua` and the cases file, in the format of
/// `tests/original_probe/LuaCompileProbe.java`.
pub fn luacompile(root: &Path, cases: &Path) -> Result<String, String> {
    use zomboid_engine::java::{string_compare_to, utf8_decode_jdk};
    use zomboid_engine::org::luaj::kahluafork::compiler::lex_state::compile;
    use zomboid_engine::se::krka::kahlua::vm::prototype::{Constant, Prototype};
    fn esc(sb: &mut String, s: Option<&str>) {
        let Some(s) = s else {
            sb.push_str("null");
            return;
        };
        sb.push('"');
        for c in s.encode_utf16() {
            if (0x20..0x7f).contains(&c) && c != u16::from(b'"') && c != u16::from(b'\\') {
                sb.push(char::from(c as u8));
            } else {
                let _ = write!(sb, "\\u{c:04x}");
            }
        }
        sb.push('"');
    }
    fn proto(sb: &mut String, p: &Prototype, depth: usize) {
        let _ = write!(sb, "P {depth} ");
        esc(sb, p.name.as_deref());
        sb.push(' ');
        esc(sb, p.file.as_deref());
        sb.push(' ');
        esc(sb, p.filename.as_deref());
        let _ = writeln!(
            sb,
            " {} {} {} {} {} {} {} {}",
            p.num_params,
            u8::from(p.is_vararg),
            p.max_stacksize,
            p.num_upvalues,
            p.code.len(),
            p.constants.len(),
            p.prototypes.len(),
            p.locvars.len()
        );
        sb.push('C');
        for c in &p.code {
            let _ = write!(sb, " {c}");
        }
        sb.push_str("\nL");
        for l in &p.lines {
            let _ = write!(sb, " {l}");
        }
        sb.push_str("\nK");
        for k in &p.constants {
            sb.push(' ');
            match k {
                Constant::Nil => sb.push_str("nil"),
                Constant::Boolean(b) => sb.push(if *b { 'T' } else { 'F' }),
                Constant::Number(d) => {
                    let _ = write!(sb, "d{:x}", d.to_bits());
                }
                Constant::String(s) => {
                    sb.push('s');
                    esc(sb, Some(s));
                }
            }
        }
        sb.push_str("\nV");
        for v in &p.locvars {
            sb.push(' ');
            esc(sb, Some(v));
        }
        sb.push('\n');
        for c in &p.prototypes {
            proto(sb, c, depth + 1);
        }
    }
    fn run(sb: &mut String, chars: Vec<u16>, name: &str, file: &str, full: &str) {
        match compile(chars, Some(name), None, Some(file), Some(full)) {
            Ok(p) => proto(sb, &p, 0),
            Err(e) => {
                sb.push_str("ERR ");
                let msg = if e.starts_with("java.") {
                    e
                } else {
                    format!("se.krka.kahlua.vm.KahluaException: {e}")
                };
                esc(sb, Some(&msg));
                sb.push('\n');
            }
        }
    }
    /// The bytes as the original's UTF-8 InputStreamReader yields them (valid UTF-8 exactly; malformed input with
    /// the String decoder's replacement rules, which is not measured).
    fn chars(bytes: &[u8]) -> Vec<u16> {
        match std::str::from_utf8(bytes) {
            Ok(s) => s.encode_utf16().collect(),
            Err(_) => utf8_decode_jdk(bytes),
        }
    }
    fn gen(kind: &str, n: usize) -> String {
        let mut sb = String::new();
        match kind {
            "bigtable" => {
                sb.push_str("local t = {");
                for i in 0..n {
                    let _ = write!(sb, "{},", i % 7);
                }
                sb.push_str("}\n");
            }
            "locals" => {
                for i in 0..n {
                    let _ = writeln!(sb, "local a{i} = {i}");
                }
            }
            "nest" => {
                sb.push_str("x = ");
                sb.push_str(&"(".repeat(n));
                sb.push('1');
                sb.push_str(&")".repeat(n));
                sb.push('\n');
            }
            "upvals" => {
                sb.push_str("local function f()\n");
                for i in 0..n {
                    let _ = writeln!(sb, "local u{i} = {i}");
                }
                sb.push_str("return function() return ");
                for i in 0..n {
                    let _ = write!(sb, "{}u{i}", if i == 0 { "" } else { " + " });
                }
                sb.push_str(" end\nend\n");
            }
            "jump" => {
                sb.push_str("while x do\n");
                for i in 0..n {
                    let _ = writeln!(sb, "y = y + {i}");
                }
                sb.push_str("end\n");
            }
            _ => sb.push_str("error('unknown generator')\n"),
        }
        sb
    }
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
        let rd = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in rd {
            let p = entry.map_err(|e| e.to_string())?.path();
            if p.is_dir() {
                walk(&p, out)?;
            } else if p.extension().is_some_and(|x| x == "lua") {
                out.push(p);
            }
        }
        Ok(())
    }
    let base = root.join("media/lua");
    let mut paths = Vec::new();
    walk(&base, &mut paths)?;
    let mut files: Vec<String> = paths
        .iter()
        .filter_map(|p| p.strip_prefix(&base).ok())
        .map(|r| format!("media/lua/{}", r.to_string_lossy().replace('\\', "/")))
        .collect();
    files.sort_by(|a, b| string_compare_to(a, b));
    let mut sb = String::new();
    for rel in &files {
        let _ = writeln!(sb, "F {rel}");
        let name = rel.rsplit('/').next().unwrap_or(rel);
        let bytes = std::fs::read(root.join(rel)).map_err(|e| format!("{rel}: {e}"))?;
        run(&mut sb, chars(&bytes), name, name, rel);
    }
    let all = std::fs::read_to_string(cases).map_err(|e| format!("{}: {e}", cases.display()))?;
    for (i, case) in all.split("\n--@@case\n").enumerate().skip(1) {
        let src = match case.strip_prefix("--@@gen ") {
            Some(g) => {
                let mut it = g.split_whitespace();
                let kind = it.next().unwrap_or("");
                let n = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
                gen(kind, n)
            }
            None => case.to_string(),
        };
        let _ = writeln!(sb, "F case{i}");
        run(
            &mut sb,
            src.encode_utf16().collect(),
            &format!("case{i}"),
            "cases",
            "tests/lua_compile_cases.txt",
        );
    }
    Ok(sb)
}

/// The Kahlua VM over each case of the cases file, in the format of `tests/original_probe/LuaVmProbe.java`.
pub fn luavm(root: &Path, cases: &Path) -> Result<String, String> {
    use zomboid_engine::se::krka::kahlua::j2se::j2se_platform::J2SEPlatform;
    use zomboid_engine::se::krka::kahlua::luaj::compiler::lua_compiler::{
        loadstring, CompileFiles,
    };
    use zomboid_engine::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
    use zomboid_engine::se::krka::kahlua::vm::object::{Object, Throwable};
    fn esc(sb: &mut String, s: &str) {
        sb.push('"');
        for c in s.encode_utf16() {
            if (0x20..0x7f).contains(&c) && c != u16::from(b'"') && c != u16::from(b'\\') {
                sb.push(char::from(c as u8));
            } else {
                let _ = write!(sb, "\\u{c:04x}");
            }
        }
        sb.push('"');
    }
    fn esc_units(sb: &mut String, s: &[u16]) {
        sb.push('"');
        for &c in s {
            if (0x20..0x7f).contains(&c) && c != u16::from(b'"') && c != u16::from(b'\\') {
                sb.push(char::from(c as u8));
            } else {
                let _ = write!(sb, "\\u{c:04x}");
            }
        }
        sb.push('"');
    }
    fn render(sb: &mut String, o: &Object) {
        match o {
            Object::Nil => sb.push_str("nil"),
            Object::Boolean(b) => sb.push_str(if *b { "true" } else { "false" }),
            Object::Number(d) => {
                let _ = write!(sb, "d{:x}", d.to_bits());
            }
            Object::String(s) => {
                sb.push('s');
                esc_units(sb, s);
            }
            Object::Table(_) => sb.push_str("table"),
            Object::Closure(_) => sb.push_str("closure"),
            Object::Function(_) => sb.push_str("jfunction"),
            Object::Coroutine(_) => sb.push_str("coroutine"),
            Object::Userdata(u) => {
                let any = u.as_any();
                match any
                    .downcast_ref::<zomboid_engine::se::krka::kahlua::vm::object::ThrowableObject>()
                {
                    Some(t) => {
                        let _ = write!(sb, "exc:{}", t.0.class_name());
                    }
                    None => {
                        let _ = write!(sb, "userdata:{}", u.class().name);
                    }
                }
            }
        }
    }
    fn exc_text(e: &Throwable) -> String {
        format!(
            "{}: {}",
            e.class_name(),
            e.get_message().unwrap_or_else(|| "null".into())
        )
    }
    let all = std::fs::read_to_string(cases).map_err(|e| format!("{}: {e}", cases.display()))?;
    let mut sb = String::new();
    for (i, case) in all.split("\n--@@case\n").enumerate().skip(1) {
        let _ = writeln!(sb, "F case{i}");
        let platform = J2SEPlatform::new();
        let env = platform
            .new_environment(root)
            .map_err(|e| format!("environment: {}", exc_text(&e)))?;
        let mut thread = KahluaThread::new(platform.clone(), env.clone());
        let units: Vec<u16> = case.encode_utf16().collect();
        let closure = match loadstring(
            &units,
            Some(&format!("case{i}")),
            env,
            &CompileFiles::default(),
        ) {
            Ok(c) => c,
            Err(e) => {
                sb.push_str("ERR ");
                esc(&mut sb, &exc_text(&e));
                sb.push('\n');
                continue;
            }
        };
        let r = match thread.pcall_args(&Object::Closure(closure), &[]) {
            Ok(r) => r,
            Err(e) => {
                sb.push_str("THROW ");
                esc(&mut sb, &exc_text(&e));
                sb.push('\n');
                continue;
            }
        };
        for line in thread.out.split('\n') {
            if line.is_empty() {
                continue;
            }
            sb.push_str("O ");
            esc(&mut sb, line);
            sb.push('\n');
        }
        let _ = write!(sb, "R {}", r.len());
        for o in &r {
            sb.push(' ');
            render(&mut sb, o);
        }
        sb.push('\n');
    }
    Ok(sb)
}

/// `pz-dump javachars <root>`: java.lang.Character.isDigit/isLowerCase/isUpperCase of every UTF-16 unit, as
/// `JavaCharsProbe` prints them.
pub fn javachars() -> Result<String, String> {
    use std::fmt::Write as _;
    use zomboid_engine::java_chars::{is_digit, is_lower_case, is_upper_case};
    let mut out = String::new();
    for c in 0..=u16::MAX {
        let f = u8::from(is_digit(c))
            | (u8::from(is_lower_case(c)) << 1)
            | (u8::from(is_upper_case(c)) << 2);
        if f != 0 {
            let _ = writeln!(out, "{c:x} {f}");
        }
    }
    Ok(out)
}

/// `tests/original_probe/FdLibmProbe.java` on the port: java.lang.Math / StrictMath results as raw bits.
pub fn fdlibm() -> Result<String, String> {
    use std::fmt::Write as _;
    use zomboid_engine::java::Random;
    use zomboid_engine::java_fdlibm as m;
    const SPECIAL: [f64; 40] = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        -0.5,
        2.0,
        -2.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        5e-324,
        -5e-324,
        f64::MAX,
        -f64::MAX,
        f64::MIN_POSITIVE,
        std::f64::consts::PI,
        std::f64::consts::PI / 2.0,
        std::f64::consts::PI / 4.0,
        -std::f64::consts::PI / 2.0,
        3.0,
        10.0,
        0.1,
        709.782712893384,
        -745.1332191019411,
        710.4758600739439,
        22.0,
        1e-300,
        1e300,
        1e22,
        1e-8,
        0.9999995,
        1.0000009536743162,
        2.1474856959999995E9,
        1e-320,
        100.0,
        -100.0,
        1e6,
        7.0,
        -3.5,
    ];
    /// `Double.doubleToLongBits`: every NaN is 0x7ff8000000000000.
    fn bits(d: f64) -> u64 {
        if d.is_nan() {
            0x7ff8_0000_0000_0000
        } else {
            d.to_bits()
        }
    }
    fn input(r: &mut Random, i: usize) -> f64 {
        if i < SPECIAL.len() {
            return SPECIAL[i];
        }
        match i % 6 {
            0 => f64::from_bits(r.next_long() as u64),
            1 => r.next_double() * 2.0 - 1.0,
            2 => (r.next_double() * 2.0 - 1.0) * 10.0,
            3 => (r.next_double() * 2.0 - 1.0) * 1000.0,
            4 => (r.next_double() * 2.0 - 1.0) * 1e6,
            _ => {
                let d = r.next_double() * 2.0 - 1.0;
                d * m::scalb(1.0, r.next_int_bound(2100) - 1075)
            }
        }
    }
    fn one(f: &str, x: f64, n: i32) -> f64 {
        match f {
            "tan" => m::tan(x),
            "asin" => m::asin(x),
            "acos" => m::acos(x),
            "atan" => m::atan(x),
            "exp" => m::exp(x),
            "log" => m::log(x),
            "log10" => m::log10(x),
            "sinh" => m::sinh(x),
            "cosh" => m::cosh(x),
            "tanh" => m::tanh(x),
            "sin" => m::sin(x),
            "cos" => m::cos(x),
            "expm1" => m::expm1(x),
            _ => m::scalb(x, n),
        }
    }
    let mut out = String::new();
    let mut r = Random::new(42);
    for f in [
        "tan", "asin", "acos", "atan", "exp", "log", "log10", "sinh", "cosh", "tanh", "sin", "cos",
        "expm1", "scalb",
    ] {
        let mut h: i64 = 0;
        for i in 0..204_000 {
            let x = input(&mut r, i);
            let n = r.next_int_bound(4300) - 2150;
            let b = bits(one(f, x, n));
            if i < 4000 {
                let _ = write!(out, "{f} {:x}", bits(x));
                if f == "scalb" {
                    let _ = write!(out, " {n}");
                }
                let _ = writeln!(out, " {b:x}");
            } else {
                h = h.wrapping_mul(31).wrapping_add(b as i64);
            }
        }
        let _ = writeln!(out, "{f} hash {:x}", h as u64);
    }
    for f in ["pow", "atan2"] {
        let mut h: i64 = 0;
        for i in 0..204_000 {
            let x = input(&mut r, i % 1600);
            let mut y = input(&mut r, i / 40 % 40 + if i >= 1600 { 40 } else { 0 });
            if f == "pow" && i % 3 == 0 {
                y = (y * 4.0).round_ties_even() / 4.0;
            }
            let b = bits(if f == "pow" {
                m::pow(x, y)
            } else {
                m::atan2(x, y)
            });
            if i < 4000 {
                let _ = writeln!(out, "{f} {:x} {:x} {b:x}", bits(x), bits(y));
            } else {
                h = h.wrapping_mul(31).wrapping_add(b as i64);
            }
        }
        let _ = writeln!(out, "{f} hash {:x}", h as u64);
    }
    Ok(out)
}

/// `tests/original_probe/LuaLoadEnvProbe.java` on the port: the game's own Lua loaded by the ported LuaManager.
/// LuaLoadEnvProbe (`boot` false) and LuaBootProbe (`boot` true) on the port.
pub fn luaload(
    root: &Path,
    cache_dir: &str,
    dirs: &[String],
    boot: bool,
) -> Result<String, String> {
    use std::collections::{HashMap, HashSet};
    use std::rc::Rc;
    use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
    use zomboid_engine::se::krka::kahlua::vm::object::{Object, TableRef};
    use zomboid_engine::zombie::lua::exposer::JavaEnum;
    use zomboid_engine::zombie::lua::lua_manager::LuaManager;
    fn esc(sb: &mut String, s: &[u16]) {
        sb.push('"');
        for &c in s {
            if (0x20..0x7f).contains(&c) && c != u16::from(b'"') && c != u16::from(b'\\') {
                sb.push(char::from(c as u8));
            } else {
                let _ = write!(sb, "\\u{c:04x}");
            }
        }
        sb.push('"');
    }
    fn key(k: &Object) -> String {
        if let Object::String(s) = k {
            let mut b = String::new();
            esc(&mut b, s);
            return b;
        }
        value(k)
    }
    fn value(o: &Object) -> String {
        match o {
            Object::Nil => "nil".into(),
            Object::Boolean(b) => b.to_string(),
            Object::Number(d) => format!("d{:x}", d.to_bits()),
            Object::String(s) => {
                let mut b = String::from("s");
                esc(&mut b, s);
                b
            }
            Object::Closure(c) => {
                let f = c.prototype.filename.as_deref().unwrap_or("null");
                let f = f.find("/media/lua/").map_or(f, |i| &f[i + 1..]);
                format!(
                    "closure {f}:{}",
                    c.prototype.lines.first().copied().unwrap_or(-1)
                )
            }
            Object::Function(f) => format!("jfunction {}", f.class().name),
            Object::Coroutine(_) => "coroutine".into(),
            Object::Table(_) => "table".into(),
            Object::Userdata(u) => match u.as_any().downcast_ref::<JavaEnum>() {
                Some(e) => format!("userdata {}.{}", e.class.name, e.name),
                None => format!("userdata {}", u.class().name),
            },
        }
    }
    struct Dumper {
        sb: String,
        seen: HashMap<usize, String>,
        init_tables: HashMap<usize, String>,
        /// The closures printed so far, in the order first printed, with where (LuaLoadEnvProbe.closures).
        closures: Vec<(
            Rc<zomboid_engine::se::krka::kahlua::vm::lua_closure::LuaClosure>,
            String,
        )>,
        closure_index: HashSet<usize>,
    }
    fn addr(t: &TableRef) -> usize {
        Rc::as_ptr(t) as usize
    }
    impl Dumper {
        fn dump(&mut self, path: &str, v: &Object) {
            if let Object::Table(t) = v {
                if let Some(init) = self.init_tables.get(&addr(t)) {
                    let _ = writeln!(self.sb, "{path} = init {init}");
                    return;
                }
                if let Some(before) = self.seen.get(&addr(t)) {
                    let _ = writeln!(self.sb, "{path} = ref {before}");
                    return;
                }
                self.seen.insert(addr(t), path.to_string());
                let _ = writeln!(self.sb, "{path} = table");
                for (k, v) in KahluaTableImpl::iterate_snapshot(t) {
                    self.dump(&format!("{path}[{}]", key(&k)), &v);
                }
            } else {
                let _ = writeln!(self.sb, "{path} = {}", value(v));
                if let Object::Closure(c) = v {
                    self.closure(c, path);
                }
            }
        }
        fn closure(
            &mut self,
            c: &Rc<zomboid_engine::se::krka::kahlua::vm::lua_closure::LuaClosure>,
            path: &str,
        ) {
            if self.closure_index.insert(Rc::as_ptr(c) as usize) {
                self.closures.push((c.clone(), path.to_string()));
            }
        }
    }
    let mut lm = LuaManager::init(root).map_err(|e| format!("init: {e:?}"))?;
    // the probe's ZomboidFileSystem.instance.setCacheDir(<scratch>)
    *lm.statics.cache_dir.borrow_mut() = Some(cache_dir.to_string());
    let env = lm.env.clone();
    let mut init_globals: Vec<(Object, Object)> = Vec::new();
    let mut init_keys: HashMap<usize, HashSet<String>> = HashMap::new();
    let mut d = Dumper {
        sb: String::new(),
        seen: HashMap::new(),
        init_tables: HashMap::new(),
        closures: Vec::new(),
        closure_index: HashSet::new(),
    };
    for (k, v) in KahluaTableImpl::iterate_snapshot(&env) {
        if let (Object::Table(t), Object::String(name)) = (&v, &k) {
            if !Rc::ptr_eq(t, &env) {
                let keys = KahluaTableImpl::iterate_snapshot(t)
                    .iter()
                    .map(|(k, _)| key(k))
                    .collect();
                init_keys.insert(addr(t), keys);
                d.init_tables
                    .insert(addr(t), String::from_utf16_lossy(name));
            }
        }
        init_globals.push((k, v));
    }
    d.init_tables.insert(addr(&env), "_G".into());
    let init_collections: HashMap<String, String> = luaload_static_collections(&env)
        .into_values()
        .map(|(name, v)| (name, static_collection_text(&v, &value).unwrap_or_default()))
        .collect();
    for dir in dirs {
        lm.load_dir_base(dir)
            .map_err(|e| format!("LoadDirBase {dir}: {e:?}"))?;
    }
    let boot_state = if boot {
        Some(luaboot_events(&mut lm, &value).map_err(|e| format!("boot: {e:?}"))?)
    } else {
        None
    };
    for line in &lm.loader.borrow().log {
        eprintln!("LOG {line}");
    }
    for f in &lm.loader.borrow().loaded {
        let i = f.find("/media/lua/").map_or(0, |i| i + 1);
        let _ = writeln!(d.sb, "LOADED {}", &f[i..]);
    }
    let mut globals: Vec<(String, Object)> = KahluaTableImpl::iterate_snapshot(&env)
        .into_iter()
        .map(|(k, _)| (key(&k), k))
        .collect();
    globals.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    let init_value = |k: &Object| -> Option<Object> {
        init_globals
            .iter()
            .find(|(ik, _)| ik.same(k) || key(ik) == key(k))
            .map(|(_, v)| v.clone())
    };
    for (gk, k) in &globals {
        let Some(Object::Table(t)) = init_value(k) else {
            continue;
        };
        let now = KahluaTableImpl::rawget(&env, k).map_err(|e| format!("{e:?}"))?;
        let Object::Table(now) = now else {
            continue;
        };
        if !Rc::ptr_eq(&now, &t) || !init_keys.contains_key(&addr(&t)) {
            continue;
        }
        let had = &init_keys[&addr(&t)];
        let added: Vec<(Object, Object)> = KahluaTableImpl::iterate_snapshot(&t)
            .into_iter()
            .filter(|(k, _)| !had.contains(&key(k)))
            .collect();
        let name = d.init_tables.remove(&addr(&t)).unwrap_or_default();
        for (k2, v2) in added {
            d.dump(&format!("ADDED G[{gk}][{}]", key(&k2)), &v2);
        }
        d.init_tables.insert(addr(&t), name);
    }
    for (gk, k) in &globals {
        let v = KahluaTableImpl::rawget(&env, k).map_err(|e| format!("{e:?}"))?;
        if let Some(iv) = init_value(k) {
            if iv.same(&v) {
                continue;
            }
        }
        d.dump(&format!("G[{gk}]"), &v);
    }
    for e in lm.events.event_list.borrow().iter() {
        let callbacks = e.callbacks.borrow();
        if callbacks.is_empty() {
            continue;
        }
        let _ = writeln!(d.sb, "EVENT {} {}", e.name, callbacks.len());
        for (i, c) in callbacks.iter().enumerate() {
            let _ = writeln!(d.sb, "  {}", value(&Object::Closure(c.clone())));
            d.closure(c, &format!("EVENT {} {i}", e.name));
        }
    }
    luaload_java_state(&lm, &init_collections, &mut d.sb, &value);
    if let Some((boot_sb, tos)) = boot_state {
        d.sb.push_str(&boot_sb);
        luaboot_java_state(&lm, &tos, &mut d.sb, &value);
        // LuaBootProbe's upvalue section: each closure printed with upvalues, in the order first printed
        let mut n = 0;
        while n < d.closures.len() {
            let (c, path) = d.closures[n].clone();
            if !c.upvalues.is_empty() {
                let _ = writeln!(d.sb, "CLOSURE U{n} {path} {}", c.upvalues.len());
                for (j, u) in c.upvalues.iter().enumerate() {
                    let v = u.borrow().get_value();
                    d.dump(&format!("U{n}[{j}]"), &v);
                }
            }
            n += 1;
        }
    }
    Ok(d.sb)
}

/// LuaBootProbe's steps after LoadDirBase on the port: ZomboidGlobals.Load, then the events the original fires on
/// its way to the main menu, each through Event.trigger; returns the BOOT lines and the TermsOfServiceState given
/// to OnGameStateEnter.
fn luaboot_events(
    lm: &mut zomboid_engine::zombie::lua::lua_manager::LuaManager,
    value: &dyn Fn(&zomboid_engine::se::krka::kahlua::vm::object::Object) -> String,
) -> Result<
    (
        String,
        std::rc::Rc<zomboid_engine::zombie::lua::exposed_boot::TermsOfServiceStateObject>,
    ),
    zomboid_engine::se::krka::kahlua::vm::object::Throwable,
> {
    use std::rc::Rc;
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::lua::exposed_boot::TermsOfServiceStateObject;
    let mut sb = String::new();
    let mut fire = |lm: &mut zomboid_engine::zombie::lua::lua_manager::LuaManager,
                    name: &str,
                    params: &[Object]|
     -> Result<(), zomboid_engine::se::krka::kahlua::vm::object::Throwable> {
        let e = lm.events.add_event(name, &lm.env)?;
        let n = e.callbacks.borrow().len();
        let _ = writeln!(sb, "BOOT {name} {n}");
        for (c, r) in e.trigger(&mut lm.thread, params)? {
            let ok = matches!(r.first(), Some(Object::Boolean(true)));
            let _ = write!(sb, "  {} {ok}", value(&Object::Closure(c)));
            if !ok {
                let _ = write!(sb, " {}", value(r.get(1).unwrap_or(&Object::Nil)));
            }
            sb.push('\n');
        }
        Ok(())
    };
    lm.statics
        .state
        .zomboid_globals
        .borrow_mut()
        .load(&lm.env)?;
    fire(lm, "OnLoadSoundBanks", &[])?;
    fire(lm, "OnGameBoot", &[])?;
    let tos = Rc::new(TermsOfServiceStateObject::default());
    fire(lm, "OnGameStateEnter", &[Object::Userdata(tos.clone())])?;
    // the rest of TermsOfServiceState.enter
    tos.0.borrow_mut().after_enter_event();
    fire(lm, "OnMainMenuEnter", &[])?;
    Ok((sb, tos))
}

/// The rest of LuaBootProbe's report: ZomboidGlobals, ObservationFactory, SurvivorDesc's colours and the
/// TermsOfServiceState.
fn luaboot_java_state(
    lm: &zomboid_engine::zombie::lua::lua_manager::LuaManager,
    tos: &zomboid_engine::zombie::lua::exposed_boot::TermsOfServiceStateObject,
    sb: &mut String,
    value: &dyn Fn(&zomboid_engine::se::krka::kahlua::vm::object::Object) -> String,
) {
    let state = &lm.statics.state;
    for (name, bits) in state.zomboid_globals.borrow().fields() {
        let _ = writeln!(sb, "ZOMBOIDGLOBALS {name} {bits}");
    }
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::lua::exposed_boot::{
        ColorObject, ImmutableColorObject, ObservationMapObject,
    };
    use zomboid_engine::zombie::lua::exposed_methods::ArrayListObject;
    let s = |t: &Option<String>| {
        t.as_deref()
            .map_or("nil".to_string(), |x| value(&Object::str(x)))
    };
    if let Object::Userdata(u) = lm.statics.field(
        "zombie.characters.traits.ObservationFactory",
        "observationMap",
    ) {
        if let Some(m) = u.as_any().downcast_ref::<ObservationMapObject>() {
            let m = m.0.borrow();
            // TreeMap<String, ...>: UTF-16 order (a null key would throw there; none is added)
            let mut keys: Vec<&Option<String>> = m.observation_map.keys().collect();
            keys.sort_by(|a, b| {
                let a: Vec<u16> = a.as_deref().unwrap_or("").encode_utf16().collect();
                let b: Vec<u16> = b.as_deref().unwrap_or("").encode_utf16().collect();
                a.cmp(&b)
            });
            for k in keys {
                let o = &m.observation_map[k];
                let me: Vec<String> = o.mutually_exclusive.iter().map(s).collect();
                let _ = writeln!(
                    sb,
                    "OBSERVATION {} {} {} {} [{}]",
                    s(k),
                    s(&o.trait_id),
                    s(&o.name),
                    s(&o.description),
                    me.join(",")
                );
            }
        }
    }
    let list = |field: &str| -> Vec<Object> {
        match lm.statics.field("zombie.characters.SurvivorDesc", field) {
            Object::Userdata(u) => u
                .as_any()
                .downcast_ref::<ArrayListObject>()
                .map(|l| l.0.borrow().clone())
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    };
    for o in list("TrouserCommonColors") {
        if let Object::Userdata(u) = o {
            if let Some(c) = u.as_any().downcast_ref::<ColorObject>() {
                let c = *c.0.borrow();
                let _ = writeln!(
                    sb,
                    "TROUSERCOLOR {:x} {:x} {:x} {:x}",
                    c.r.to_bits(),
                    c.g.to_bits(),
                    c.b.to_bits(),
                    c.a.to_bits()
                );
            }
        }
    }
    for o in list("HairCommonColors") {
        if let Object::Userdata(u) = o {
            if let Some(c) = u.as_any().downcast_ref::<ImmutableColorObject>() {
                let c = c.0;
                let _ = writeln!(
                    sb,
                    "HAIRCOLOR {:x} {:x} {:x} {:x}",
                    c.r.to_bits(),
                    c.g.to_bits(),
                    c.b.to_bits(),
                    c.a.to_bits()
                );
            }
        }
    }
    if let Object::Userdata(u) = zomboid_engine::zombie::lua::exposed_methods::singleton(
        &lm.statics,
        zomboid_engine::zombie::lua::exposer::class_named("zombie.scripting.ScriptManager"),
    ) {
        if let Some(m) = u
            .as_any()
            .downcast_ref::<zomboid_engine::zombie::lua::exposed_methods::ScriptManagerObject>()
        {
            // TreeMap<String, Integer> of String.valueOf(tag)
            let mut tags: Vec<(Vec<u16>, String, usize)> = m
                .tag_to_item_map
                .borrow()
                .iter()
                .map(|(k, v)| {
                    let name = if k.is_nil() {
                        "null".to_string()
                    } else {
                        k.java_to_string()
                    };
                    let n = match v {
                        Object::Userdata(l) => l
                            .as_any()
                            .downcast_ref::<ArrayListObject>()
                            .map_or(0, |l| l.0.borrow().len()),
                        _ => 0,
                    };
                    (name.encode_utf16().collect(), name, n)
                })
                .collect();
            tags.sort_by(|a, b| a.0.cmp(&b.0));
            for (_, name, n) in tags {
                let _ = writeln!(sb, "ITEMSTAG {} {n}", value(&Object::str(&name)));
            }
        }
    }
    let t = *tos.0.borrow();
    let _ = writeln!(sb, "TOSSTATE {} {}", t.created, t.exit);
}

/// LuaLoadEnvProbe's text of a static collection: a list's elements, a map's size (the port's other maps are
/// opaque and never change, so they are left out on both sides of the comparison).
fn static_collection_text(
    v: &zomboid_engine::se::krka::kahlua::vm::object::Object,
    value: &dyn Fn(&zomboid_engine::se::krka::kahlua::vm::object::Object) -> String,
) -> Option<String> {
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::lua::exposed_boot::ObservationMapObject;
    use zomboid_engine::zombie::lua::exposed_methods::ArrayListObject;
    let Object::Userdata(u) = v else { return None };
    if let Some(l) = u.as_any().downcast_ref::<ArrayListObject>() {
        return Some(format!(
            "[{}]",
            l.0.borrow().iter().map(value).collect::<Vec<_>>().join(",")
        ));
    }
    u.as_any()
        .downcast_ref::<ObservationMapObject>()
        .map(|m| format!("size {}", m.0.borrow().observation_map.len()))
}

/// The ArrayLists (and the maps the port keeps) public static fields hold, by "<class>.<field>", from the statics
/// tables globals hold (LuaLoadEnvProbe.staticCollections).
fn luaload_static_collections(
    env: &zomboid_engine::se::krka::kahlua::vm::object::TableRef,
) -> std::collections::BTreeMap<
    Vec<u16>,
    (String, zomboid_engine::se::krka::kahlua::vm::object::Object),
> {
    use zomboid_engine::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
    use zomboid_engine::se::krka::kahlua::vm::object::{ClassObject, Object};
    use zomboid_engine::zombie::lua::exposed_methods::ArrayListObject;
    let mut r = std::collections::BTreeMap::new();
    for (_, v) in KahluaTableImpl::iterate_snapshot(env) {
        let Object::Table(t) = v else { continue };
        let Ok(Object::Userdata(c)) = KahluaTableImpl::rawget(&t, &Object::str("class")) else {
            continue;
        };
        let Some(c) = c.as_any().downcast_ref::<ClassObject>() else {
            continue;
        };
        for (k, v) in KahluaTableImpl::iterate_snapshot(&t) {
            if let Object::Userdata(u) = &v {
                if u.as_any().is::<ArrayListObject>()
                    || u.as_any()
                        .is::<zomboid_engine::zombie::lua::exposed_boot::ObservationMapObject>()
                {
                    let name = format!("{}.{}", c.0.name, k.java_to_string());
                    r.insert(name.encode_utf16().collect(), (name, v.clone()));
                }
            }
        }
    }
    r
}

/// LuaLoadEnvProbe.javaState on the port: the Java state the scripts filled while loading.
fn luaload_java_state(
    lm: &zomboid_engine::zombie::lua::lua_manager::LuaManager,
    init_collections: &std::collections::HashMap<String, String>,
    sb: &mut String,
    value: &dyn Fn(&zomboid_engine::se::krka::kahlua::vm::object::Object) -> String,
) {
    use std::collections::BTreeMap;
    use std::rc::Rc;
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::lua::exposed_methods::{singleton, ArrayListObject};
    use zomboid_engine::zombie::lua::exposed_table::RANDOMIZED_WORLD_BASE_LISTS;
    use zomboid_engine::zombie::lua::exposed_world::{
        BentFencesObject, BrokenFencesObject, ContainerOverlaysObject, TileOverlaysObject,
        WorldGenUtilsObject,
    };
    use zomboid_engine::zombie::lua::exposer::class_named;
    let s = |t: &str| value(&Object::str(t));
    let opt = |t: &Option<String>| t.as_deref().map_or("nil".to_string(), s);
    let list = |l: &[String]| format!("[{}]", l.iter().map(|x| s(x)).collect::<Vec<_>>().join(","));
    let opt_list = |l: &Option<Vec<String>>| l.as_deref().map_or("nil".to_string(), list);
    let objects = |l: &ArrayListObject| {
        format!(
            "[{}]",
            l.0.borrow().iter().map(value).collect::<Vec<_>>().join(",")
        )
    };
    // TreeMap<String, V>: String.compareTo is UTF-16 order
    fn sorted<'a, V>(
        m: impl IntoIterator<Item = (&'a String, V)>,
    ) -> BTreeMap<Vec<u16>, (&'a String, V)> {
        m.into_iter()
            .map(|(k, v)| (k.encode_utf16().collect(), (k, v)))
            .collect()
    }
    let events = lm.events.event_list.borrow();
    let _ = write!(sb, "EVENTS {}", events.len());
    for e in events.iter() {
        let _ = write!(sb, " {}", e.name);
    }
    sb.push('\n');
    for (_, (name, v)) in luaload_static_collections(&lm.env) {
        if name.ends_with("Texture.nullTextures") {
            continue;
        }
        let Some(is) = static_collection_text(&v, value) else {
            continue;
        };
        if init_collections.get(&name) != Some(&is) {
            let _ = writeln!(sb, "STATIC {name} {is}");
        }
    }
    let state = &lm.statics.state;
    let map_objects = state.map_objects.borrow();
    for (which, m) in [
        ("onNew", &map_objects.on_new),
        ("onLoad", &map_objects.on_load),
    ] {
        for (_, (sprite, cb)) in sorted(m) {
            let _ = write!(sb, "MAPOBJECTS {which} {}", s(sprite));
            for (p, f) in cb.priority.iter().zip(&cb.functions) {
                let _ = write!(sb, " {p}={}", value(f));
            }
            sb.push('\n');
        }
    }
    let the = |class: &str| singleton(&lm.statics, class_named(class));
    let o = the("zombie.iso.TileOverlays");
    if let Object::Userdata(u) = &o {
        if let Some(to) = u.as_any().downcast_ref::<TileOverlaysObject>() {
            let to = to.0.borrow();
            for (_, (k, o)) in sorted(&to.overlay_map) {
                let _ = writeln!(sb, "TILEOVERLAY {} {}", s(k), s(&o.tile));
                for e in &o.entries {
                    let u = &e.usage;
                    let _ = writeln!(
                        sb,
                        "  {} {} {} {} {} {:x} {} {}",
                        opt(&e.room),
                        e.chance,
                        opt(&u.usage),
                        u.z_only,
                        u.z_greater_than,
                        u.alpha.to_bits(),
                        u.table_top,
                        list(&e.tiles)
                    );
                }
            }
            for (_, (k, v)) in sorted(&to.overlay_name_to_underlying_name) {
                let _ = writeln!(sb, "TILEUNDER {} {}", s(k), list(v));
            }
        }
    }
    let o = the("zombie.iso.ContainerOverlays");
    if let Object::Userdata(u) = &o {
        if let Some(co) = u.as_any().downcast_ref::<ContainerOverlaysObject>() {
            let co = co.0.borrow();
            for (_, (k, o)) in sorted(&co.overlay_map) {
                let _ = writeln!(sb, "CONTAINEROVERLAY {} {}", s(k), s(&o.name));
                for e in &o.entries {
                    let _ = writeln!(
                        sb,
                        "  {} {} {}",
                        opt(&e.room),
                        opt(&e.many_items),
                        opt(&e.few_items)
                    );
                }
            }
            for (_, (k, v)) in sorted(&co.overlay_name_to_underlying_name) {
                let _ = writeln!(sb, "CONTAINERUNDER {} {}", s(k), list(v));
            }
        }
    }
    let o = the("zombie.iso.BentFences");
    if let Object::Userdata(u) = &o {
        if let Some(bf) = u.as_any().downcast_ref::<BentFencesObject>() {
            let bf = bf.0.borrow();
            for (i, e) in bf.entries.iter().enumerate() {
                let _ = writeln!(
                    sb,
                    "BENTFENCE {i} {} {} {} {} {} {} {}",
                    e.dir.name(),
                    e.health,
                    e.length,
                    e.collapsed_offset,
                    e.collapsed_size_x,
                    e.collapsed_size_y,
                    e.do_smash
                );
                let stages: BTreeMap<i32, &Vec<String>> =
                    e.stages.iter().map(|(k, v)| (*k, v)).collect();
                for (k, v) in stages {
                    let _ = writeln!(sb, "  stage {k} {}", list(v));
                }
                let _ = writeln!(sb, "  collapsed {}", list(&e.collapsed));
                let _ = writeln!(sb, "  debris {}", list(&e.debris));
            }
            let set = |h: &std::collections::HashSet<String>| {
                let v: Vec<String> = sorted(h.iter().map(|x| (x, ())))
                    .into_values()
                    .map(|(x, _)| x.clone())
                    .collect();
                list(&v)
            };
            let _ = writeln!(sb, "BENTCOLLAPSED {}", set(&bf.collapsed_tiles));
            let _ = writeln!(sb, "BENTDEBRIS {}", set(&bf.debris_tiles));
            for (_, (k, v)) in sorted(&bf.fence_map) {
                let _ = write!(sb, "BENTMAP {}", s(k));
                for e in v {
                    let n = bf
                        .entries
                        .iter()
                        .rposition(|x| Rc::ptr_eq(x, e))
                        .map_or(-1, |i| i as i64);
                    let _ = write!(sb, " {n}");
                }
                sb.push('\n');
            }
        }
    }
    let o = the("zombie.iso.BrokenFences");
    if let Object::Userdata(u) = &o {
        if let Some(brf) = u.as_any().downcast_ref::<BrokenFencesObject>() {
            let brf = brf.0.borrow();
            let unbroken = sorted(&brf.unbroken_map);
            for (k, t) in unbroken.values() {
                let t = t.borrow();
                let _ = write!(sb, "BROKENTILE {}", s(k));
                for l in [
                    &t.self_,
                    &t.left,
                    &t.right,
                    &t.debris_n,
                    &t.debris_s,
                    &t.debris_w,
                    &t.debris_e,
                ] {
                    let _ = write!(sb, " {}", opt_list(l));
                }
                sb.push('\n');
            }
            for (which, m) in [
                ("brokenLeftMap", &brf.broken_left_map),
                ("brokenRightMap", &brf.broken_right_map),
                ("allMap", &brf.all_map),
            ] {
                for (_, (k, t)) in sorted(m) {
                    let label = unbroken
                        .values()
                        .find(|(_, u)| Rc::ptr_eq(u, t))
                        .map_or("orphan".to_string(), |(uk, _)| s(uk));
                    let _ = writeln!(sb, "BROKENMAP {which} {} {label}", s(k));
                }
            }
        }
    }
    let mut lists: Vec<&str> = RANDOMIZED_WORLD_BASE_LISTS.to_vec();
    lists.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    let clutter = state.clutter.borrow();
    for f in lists {
        let l = match clutter.get(f) {
            Some(Object::Userdata(u)) => u
                .as_any()
                .downcast_ref::<ArrayListObject>()
                .map_or("[]".to_string(), &objects),
            _ => "[]".to_string(),
        };
        let _ = writeln!(sb, "CLUTTER {f} {l}");
    }
    let o = the("zombie.iso.worldgen.WorldGenUtils");
    let files = match &o {
        Object::Userdata(u) => u
            .as_any()
            .downcast_ref::<WorldGenUtilsObject>()
            .map_or_else(Vec::new, |w| w.0.borrow().files.clone()),
        _ => Vec::new(),
    };
    let _ = writeln!(sb, "WORLDGENFILES {}", list(&files));
    let _ = writeln!(sb, "SGLOBALSYSTEMS 0");
    luaload_client_state(lm, sb, &s);
}

/// The client part of LuaLoadEnvProbe.javaState: the music configs, the controller bindings, the loaded binding
/// sets.
fn luaload_client_state(
    lm: &zomboid_engine::zombie::lua::lua_manager::LuaManager,
    sb: &mut String,
    s: &dyn Fn(&str) -> String,
) {
    use std::rc::Rc;
    use zomboid_engine::se::krka::kahlua::vm::object::Object;
    use zomboid_engine::zombie::characters::character_joypad_axis2d_binding as axis2d;
    use zomboid_engine::zombie::characters::character_joypad_button_binding::{
        IsDownBinding, CONSTANTS,
    };
    use zomboid_engine::zombie::lua::exposed_client::{
        MusicIntensityConfigObject, MusicThreatConfigObject,
    };
    // TreeMap<String, V>: String.compareTo is UTF-16 order
    fn sorted<'a, V>(
        m: impl IntoIterator<Item = (&'a String, V)>,
    ) -> std::collections::BTreeMap<Vec<u16>, (&'a String, V)> {
        m.into_iter()
            .map(|(k, v)| (k.encode_utf16().collect(), (k, v)))
            .collect()
    }
    let made = |class: &str| match lm.statics.singleton_if_made(class) {
        Some(Object::Userdata(u)) => Some(u),
        _ => None,
    };
    match made("zombie.audio.MusicIntensityConfig") {
        None => sb.push_str("MUSICINTENSITY nil\n"),
        Some(u) => {
            if let Some(o) = u.as_any().downcast_ref::<MusicIntensityConfigObject>() {
                let c = o.0.borrow();
                let _ = writeln!(sb, "MUSICINTENSITY {}", c.events.len());
                for e in &c.events {
                    let _ = writeln!(
                        sb,
                        "  {} {:x} {} {}",
                        s(&e.id),
                        e.intensity.to_bits(),
                        e.duration,
                        e.multiple
                    );
                }
                for (_, (k, e)) in sorted(&c.event_by_id) {
                    let i = c
                        .events
                        .iter()
                        .position(|x| Rc::ptr_eq(x, e))
                        .map_or(-1, |i| i as i64);
                    let _ = writeln!(sb, "MUSICINTENSITYID {} {i}", s(k));
                }
            }
        }
    }
    match made("zombie.audio.MusicThreatConfig") {
        None => sb.push_str("MUSICTHREAT nil\n"),
        Some(u) => {
            if let Some(o) = u.as_any().downcast_ref::<MusicThreatConfigObject>() {
                let c = o.0.borrow();
                let _ = writeln!(sb, "MUSICTHREAT {}", c.status_list.len());
                for st in &c.status_list {
                    let _ = writeln!(
                        sb,
                        "  {} {:x} {:x}",
                        s(&st.id),
                        st.intensity.to_bits(),
                        st.intensity_override.get().to_bits()
                    );
                }
                for (_, (k, st)) in sorted(&c.status_by_id) {
                    let i = c
                        .status_list
                        .iter()
                        .position(|x| Rc::ptr_eq(x, st))
                        .map_or(-1, |i| i as i64);
                    let _ = writeln!(sb, "MUSICTHREATID {} {i}", s(k));
                }
            }
        }
    }
    let b = lm.statics.state.input_bindings.borrow();
    let name = |n: Option<&str>| n.unwrap_or("nil").to_string();
    for (i, (n, _)) in CONSTANTS.iter().enumerate() {
        let d = match b.button_bindings.binding[i] {
            None => "nil".to_string(),
            Some(IsDownBinding::JoypadButton(x)) => format!("button {}", name(x)),
            Some(IsDownBinding::Axis1dMinMax { axis1d, min, max }) => {
                format!(
                    "axis1d {} {:x} {:x}",
                    name(axis1d),
                    min.to_bits(),
                    max.to_bits()
                )
            }
            Some(IsDownBinding::Axis2dMinMax { axis2d, min, max }) => {
                format!(
                    "axis2d {} {:x} {:x}",
                    name(axis2d),
                    min.to_bits(),
                    max.to_bits()
                )
            }
        };
        let _ = writeln!(sb, "JOYPADBUTTONBINDING {n} {d}");
    }
    for (i, (n, _)) in axis2d::CONSTANTS.iter().enumerate() {
        let _ = writeln!(
            sb,
            "JOYPADAXISBINDING {n} {}",
            name(b.axis2d_bindings.binding[i])
        );
    }
    match &b.loaded_binding_sets {
        None => sb.push_str("INPUTBINDINGSETS nil\n"),
        Some(sets) => {
            let names: Vec<String> = sets.iter().map(|x| s(&x.name)).collect();
            let _ = writeln!(sb, "INPUTBINDINGSETS [{}]", names.join(","));
        }
    }
}

/// TranslatorProbe's output for the port: `zombie.core.Translator` over the game folder, driven by the same command
/// file (see the probe's header). The scratch dir is unused: `json` texts go straight to the part of
/// `tryFillMapFromFile` after its `File.exists` (the probe writes them as files first).
fn translator(root: &Path, commands: &Path) -> Result<String, String> {
    use std::collections::HashMap;
    use zomboid_engine::java_formatter::FormatArg;
    use zomboid_engine::zombie::core::translator::{self, Translator, BY_NAME};

    fn esc(s: &str) -> String {
        let mut b = String::from("\"");
        for c in s.encode_utf16() {
            if (0x20..0x7f).contains(&c) && c != 0x22 && c != 0x5c {
                b.push(c as u8 as char);
            } else {
                let _ = write!(b, "\\u{c:04x}");
            }
        }
        b.push('"');
        b
    }
    fn esc_opt(s: Option<&str>) -> String {
        s.map_or_else(|| "null".to_string(), esc)
    }
    // the command file's escapes, as bytes (\xHH a raw byte, the rest UTF-8; \u a UTF-16 unit in 3-byte form)
    fn unescape(s: &str) -> Vec<u8> {
        let mut out = Vec::new();
        let mut it = s.chars();
        while let Some(c) = it.next() {
            if c != '\\' {
                let mut buf = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                continue;
            }
            match it.next() {
                Some('n') => out.push(b'\n'),
                Some('t') => out.push(b'\t'),
                Some('r') => out.push(b'\r'),
                Some('\\') => out.push(b'\\'),
                Some('x') => {
                    let h: String = it.by_ref().take(2).collect();
                    out.push(u8::from_str_radix(&h, 16).unwrap_or(0));
                }
                Some('u') => {
                    let h: String = it.by_ref().take(4).collect();
                    let u = u32::from_str_radix(&h, 16).unwrap_or(0);
                    if u < 0x80 {
                        out.push(u as u8);
                    } else if u < 0x800 {
                        out.extend_from_slice(&[0xc0 | (u >> 6) as u8, 0x80 | (u & 0x3f) as u8]);
                    } else {
                        out.extend_from_slice(&[
                            0xe0 | (u >> 12) as u8,
                            0x80 | ((u >> 6) & 0x3f) as u8,
                            0x80 | (u & 0x3f) as u8,
                        ]);
                    }
                }
                Some(e) => {
                    out.push(b'\\');
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(e.encode_utf8(&mut buf).as_bytes());
                }
                None => out.push(b'\\'),
            }
        }
        out
    }
    fn text(field: &str) -> String {
        String::from_utf8_lossy(&unescape(field)).into_owned()
    }
    fn arg(a: &str) -> Result<FormatArg, String> {
        if a == "n" {
            return Ok(FormatArg::Null);
        }
        let v = text(a.get(2..).unwrap_or(""));
        match a.as_bytes().first() {
            Some(b's') => Ok(FormatArg::Str(v)),
            Some(b'd') => java::parse_double(&v)
                .map(FormatArg::Double)
                .ok_or_else(|| format!("bad double {v}")),
            Some(b'b') => Ok(FormatArg::Bool(java::equals_ignore_case(&v, "true"))),
            _ => Err(format!("bad argument {a}")),
        }
    }
    fn arg_text(o: &FormatArg) -> String {
        match o {
            FormatArg::Null => "n".into(),
            FormatArg::Double(d) => format!("d:{:x}", d.to_bits()),
            FormatArg::Bool(b) => format!("b:{b}"),
            FormatArg::Str(s) => format!("s:{}", esc(s)),
            FormatArg::Object { .. } => "?".into(),
        }
    }
    fn fnv(mut h: u64, line: &str) -> u64 {
        for &x in line.as_bytes() {
            h ^= u64::from(x);
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }
    fn sorted(map: &HashMap<String, String>) -> Vec<(&String, &String)> {
        let mut v: Vec<_> = map.iter().collect();
        v.sort_by(|a, b| java::string_compare_to(a.0, b.0));
        v
    }
    fn dump_map(sb: &mut String, name: &str, map: &HashMap<String, String>, full: bool) {
        let entries = sorted(map);
        if full {
            let _ = writeln!(sb, "MAP {name} {}", entries.len());
            for (k, v) in &entries {
                let _ = writeln!(sb, "{} = {}", esc(k), esc(v));
            }
        } else {
            let mut h: u64 = 0xcbf29ce484222325;
            for (k, v) in &entries {
                h = fnv(h, &format!("{} = {}\n", esc(k), esc(v)));
            }
            let _ = writeln!(sb, "MAP {name} {} {h:016x}", entries.len());
        }
    }
    fn call(t: &mut Translator, method: &str, a: &[String]) -> Result<String, String> {
        let s0 = a.first().map(|f| text(f)).unwrap_or_default();
        let r: Result<Option<String>, String> = match method {
            "getText" | "getTextOrNull" => {
                let args = a[1..]
                    .iter()
                    .map(|x| arg(x))
                    .collect::<Result<Vec<_>, _>>()?;
                let r = if method == "getText" {
                    t.get_text(&s0, &args).map(Some)
                } else {
                    t.get_text_or_null(&s0, &args)
                };
                // a formatting the port does not print stops the dump (it would differ anyway)
                return r.map(|v| esc_opt(v.as_deref()));
            }
            "getDisplayItemName" => Ok(Some(t.get_display_item_name(&s0))),
            "getItemNameFromFullType" => t.get_item_name_from_full_type(&s0, None).map(Some),
            "getItemEvolvedRecipeName" => t.get_item_evolved_recipe_name(&s0, None).map(Some),
            "setDefaultItemEvolvedRecipeName" => t
                .set_default_item_evolved_recipe_name(&s0, &text(&a[1]))
                .map(|()| Some("(void)".to_string())),
            "getMoveableDisplayName" => Ok(Some(t.get_moveable_display_name(&s0))),
            "getMoveableDisplayNameOrNull" => Ok(t.get_moveable_display_name_or_null(&s0)),
            "getRecipeName" => Ok(Some(t.get_recipe_name(&s0))),
            "getRecipeGroupName" => Ok(Some(t.get_recipe_group_name(&s0))),
            "getTextMediaEN" => Ok(Some(t.get_text_media_en(&s0))),
            "getAttributeText" => Ok(t.get_attribute_text(&s0, false)),
            "getAttributeTextOrNull" => Ok(t.get_attribute_text(&s0, true)),
            "getFluidText" => Ok(Some(t.get_fluid_text(&s0))),
            "getEntityText" => Ok(Some(t.get_entity_text(&s0))),
            "getMapLabelText" => Ok(Some(t.get_map_label_text(&s0))),
            _ => return Err(format!("unknown method {method}")),
        };
        Ok(match r {
            Ok(v) => esc_opt(v.as_deref()),
            Err(e) => format!("ERROR {e}"),
        })
    }
    fn call_text(method: &str, a: &[String]) -> Result<String, String> {
        let mut b = format!("CALL {method}(");
        for (i, f) in a.iter().enumerate() {
            if i > 0 {
                b.push_str(", ");
            }
            if i == 0 || !method.starts_with("getText") {
                b.push_str(&esc(&text(f)));
            } else {
                b.push_str(&arg_text(&arg(f)?));
            }
        }
        b.push(')');
        Ok(b)
    }

    let mut t = Translator::new(root);
    t.languages.init(root);
    let mut sb = String::new();
    let mut langs = t.languages.languages.clone();
    langs.sort_by(|a, b| java::string_compare_to(&a.name, &b.name));
    let _ = writeln!(sb, "LANGUAGES {}", langs.len());
    for l in &langs {
        let _ = writeln!(
            sb,
            "LANG {} {} {} {}",
            l.name,
            esc_opt(l.text.as_deref()),
            esc_opt(l.base.as_deref()),
            l.azerty
        );
    }
    let _ = writeln!(sb, "DEFAULT {}", t.languages.get_default_language().name);

    let file =
        std::fs::read_to_string(commands).map_err(|e| format!("{}: {e}", commands.display()))?;
    let mut calls: Vec<Vec<String>> = Vec::new();
    for line in file.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut f: Vec<String> = line.split('\t').map(str::to_string).collect();
        let cmd = f.remove(0);
        match cmd.as_str() {
            "userlang" => {
                t.user_language = text(&f[0]);
                let _ = writeln!(sb, "USERLANG {}", esc(&text(&f[0])));
            }
            "call" => calls.push(f),
            "load" => {
                let option = if f[0] == "_" {
                    String::new()
                } else {
                    text(&f[0])
                };
                let full = f[1] == "full";
                t.option_language_name = option.clone();
                let _ = writeln!(sb, "LOAD {}", esc(&option));
                if let Err(e) = t.load_files() {
                    let _ = writeln!(sb, "ERROR {}", e.cause.java_text());
                    continue;
                }
                let _ = writeln!(sb, "LANGUAGE {}", t.get_language().name);
                let mut stack = String::from("STACK");
                for l in t.for_language_stack() {
                    let _ = write!(stack, " {}", l.name);
                }
                let _ = writeln!(sb, "{stack}");
                for name in BY_NAME {
                    dump_map(&mut sb, name, t.map(name).ok_or("no map")?, full);
                }
                dump_map(&mut sb, "Recorded_Media_EN", &t.recorded_media_en, full);
                for c in &calls {
                    let args = &c[1..];
                    let _ = writeln!(
                        sb,
                        "{} = {}",
                        call_text(&c[0], args)?,
                        call(&mut t, &c[0], args)?
                    );
                }
                let mut missing: Vec<&String> = t.missing.iter().collect();
                missing.sort_by(|a, b| java::string_compare_to(a, b));
                let _ = writeln!(sb, "MISSING {}", missing.len());
                for m in missing {
                    let _ = writeln!(sb, "  {}", esc(m));
                }
                let br = sorted(&t.br_replacements);
                let _ = writeln!(sb, "BR {}", br.len());
                for (k, v) in br {
                    let _ = writeln!(sb, "  {} = {}", esc(k), esc(v));
                }
            }
            "fmt" => {
                let template = text(&f[0]);
                let mut args = f.clone();
                t.get_ui().insert("UI_Probe".into(), template.clone());
                args[0] = "UI_Probe".into();
                let _ = writeln!(sb, "FMT {}", esc(&template));
                for m in ["getText", "getTextOrNull"] {
                    let _ = writeln!(
                        sb,
                        "  {} = {}",
                        call_text(m, &args)?,
                        call(&mut t, m, &args)?
                    );
                }
                args[0] = f[0].clone();
                let _ = writeln!(
                    sb,
                    "  {} = {}",
                    call_text("getText", &args)?,
                    call(&mut t, "getText", &args)?
                );
                t.get_ui().remove("UI_Probe");
            }
            "json" => {
                let id = &f[0];
                let mut map = HashMap::new();
                let mut result = None;
                for field in &f[1..] {
                    if let Err(e) = translator::fill_map_from_bytes(&unescape(field), &mut map) {
                        result = Some(format!("ERROR {}", e.java_text()));
                        break;
                    }
                }
                match result {
                    Some(r) => {
                        let _ = writeln!(sb, "JSON {id} {r}");
                    }
                    None => {
                        let _ = writeln!(sb, "JSON {id} ok {}", map.len());
                        for (k, v) in sorted(&map) {
                            let _ = writeln!(sb, "  {} = {}", esc(k), esc(v));
                        }
                    }
                }
            }
            _ => return Err(format!("unknown command {cmd}")),
        }
    }
    Ok(sb)
}
