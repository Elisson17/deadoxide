#!/bin/sh
# Differential check of the direct port: run a Java probe on the ORIGINAL jar and `pz-dump` on the Rust port over the
# same install, and compare the two outputs byte for byte. Nothing game-derived is committed; only sha256 of the
# oracle output is kept in tests/golden/engine/<case>.sha256 so `cargo test` can check the port without Java.
#   sh tools/oracle.sh            all cases
#   sh tools/oracle.sh tiledefs   one case
cd "$(dirname "$0")/.." || exit 2
export PATH="$HOME/.cargo/bin:$PATH"
ROOT=$(cd .. && pwd)
HERE=$(pwd)
mkdir -p target/oracle target/original-probe tests/golden/engine
cargo build -q --release -p zomboid-engine || exit 1
fail=0
# case <name> <ProbeClass> [probe args...]   (pz-dump gets: <name> <root> [args...])
run_case() {
  name=$1; probe=$2; shift 2
  javac -nowarn -encoding UTF-8 -cp "$ROOT/projectzomboid.jar" -d target/original-probe "tests/original_probe/$probe.java" 2>target/oracle/$name.javac \
    || { cat target/oracle/$name.javac; echo "FAIL $name (javac)"; fail=1; return; }
  (cd "$ROOT" && java -cp "$HERE/target/original-probe:projectzomboid.jar" "$probe" "$@" >"$HERE/target/oracle/$name.java.txt" 2>"$HERE/target/oracle/$name.java.err") \
    || { tail -5 target/oracle/$name.java.err; echo "FAIL $name (java probe)"; fail=1; return; }
  ./target/release/pz-dump "$name" "$ROOT" "$@" >target/oracle/$name.rust.txt 2>target/oracle/$name.rust.err \
    || { tail -5 target/oracle/$name.rust.err; echo "FAIL $name (pz-dump)"; fail=1; return; }
  if cmp -s target/oracle/$name.java.txt target/oracle/$name.rust.txt; then
    shasum -a 256 target/oracle/$name.java.txt | cut -d' ' -f1 >tests/golden/engine/$name.sha256
    echo "OK   $name: $(wc -l <target/oracle/$name.java.txt | tr -d ' ') lines identical"
  else
    echo "FAIL $name: first differences:"; diff target/oracle/$name.java.txt target/oracle/$name.rust.txt | head -6; fail=1
  fi
}
# jre_case <name> <ProbeClass> [probe args...]: like run_case, on the game's own bundled JRE (the JDK library
# behaviour the game really has: UTF-8 decoding, Double.parseDouble), the probe compiled for it.
PZ_JRE=${PZ_JRE:-"$HOME/Library/Application Support/Steam/steamapps/common/ProjectZomboid/Project Zomboid.app/Contents/PlugIns/jre-$( [ "$(uname -m)" = arm64 ] && echo aarch64 || echo x86_64 )/Contents/Home/bin/java"}
jre_case() {
  name=$1; probe=$2; shift 2
  [ -x "$PZ_JRE" ] || { echo "FAIL $name: no game JRE at $PZ_JRE (set PZ_JRE)"; fail=1; return; }
  javac -nowarn --release 25 -encoding UTF-8 -cp "$ROOT/projectzomboid.jar" -d target/original-probe-jre "tests/original_probe/$probe.java" 2>target/oracle/$name.javac \
    || { cat target/oracle/$name.javac; echo "FAIL $name (javac)"; fail=1; return; }
  (cd "$ROOT" && "$PZ_JRE" -cp "$HERE/target/original-probe-jre:projectzomboid.jar" "$probe" "$@" >"$HERE/target/oracle/$name.java.txt" 2>"$HERE/target/oracle/$name.java.err") \
    || { tail -5 target/oracle/$name.java.err; echo "FAIL $name (java probe)"; fail=1; return; }
  ./target/release/pz-dump "$name" "$ROOT" "$@" >target/oracle/$name.rust.txt 2>target/oracle/$name.rust.err \
    || { tail -5 target/oracle/$name.rust.err; echo "FAIL $name (pz-dump)"; fail=1; return; }
  if cmp -s target/oracle/$name.java.txt target/oracle/$name.rust.txt; then
    shasum -a 256 target/oracle/$name.java.txt | cut -d' ' -f1 >tests/golden/engine/$name.sha256
    echo "OK   $name: $(wc -l <target/oracle/$name.java.txt | tr -d ' ') lines identical"
  else
    echo "FAIL $name: first differences:"; diff target/oracle/$name.java.txt target/oracle/$name.rust.txt | head -6; fail=1
  fi
}
# server_case <name> <ProbeClass> [args...]: the probe runs INSIDE the fully booted original (ServerHarness.java): a
# headless dedicated server from the pinned jar, natives for this OS from $PZ_NATIVES, fresh scratch cache dir, fixed seed.
PZ_NATIVES=${PZ_NATIVES:-"$HOME/Library/Application Support/Steam/steamapps/common/ProjectZomboid/Project Zomboid.app/Contents/Java"}
server_case() {
  name=$1; probe=$2; shift 2
  [ -d "$PZ_NATIVES" ] || { echo "FAIL $name: no natives in $PZ_NATIVES (set PZ_NATIVES)"; fail=1; return; }
  javac -nowarn -encoding UTF-8 -cp "$ROOT/projectzomboid.jar" -d target/original-probe tests/original_probe/ServerHarness.java "tests/original_probe/$probe.java" 2>target/oracle/$name.javac \
    || { cat target/oracle/$name.javac; echo "FAIL $name (javac)"; fail=1; return; }
  (cd "$ROOT" && java -Djava.awt.headless=true --enable-final-field-mutation=ALL-UNNAMED -Djava.library.path="$PZ_NATIVES" -Xmx4g -cp "$HERE/target/original-probe:projectzomboid.jar" "$probe" "$HERE/target/oracle/pz-oracle-$name" "$@" >"$HERE/target/oracle/$name.java.txt" 2>"$HERE/target/oracle/$name.java.err") \
    || { grep -n 'ORACLE\|Exception' target/oracle/$name.java.err | tail -5; echo "FAIL $name (server probe)"; fail=1; return; }
  ./target/release/pz-dump "$name" "$ROOT" "$@" >target/oracle/$name.rust.txt 2>target/oracle/$name.rust.err \
    || { tail -5 target/oracle/$name.rust.err; echo "FAIL $name (pz-dump)"; fail=1; return; }
  if cmp -s target/oracle/$name.java.txt target/oracle/$name.rust.txt; then
    shasum -a 256 target/oracle/$name.java.txt | cut -d' ' -f1 >tests/golden/engine/$name.sha256
    echo "OK   $name: $(wc -l <target/oracle/$name.java.txt | tr -d ' ') lines identical"
  else
    echo "FAIL $name: first differences:"; diff target/oracle/$name.java.txt target/oracle/$name.rust.txt | head -6; fail=1
  fi
}
# game_case <name> <ProbeClass> [args...]: the probe runs parts of the ORIGINAL game standalone (no server): the pinned
# jar with this OS's natives, from the game root; its first argument is a fresh scratch cache dir, then the args.
# A probe may use another probe's code (javac finds it in tests/original_probe).
game_case() {
  name=$1; probe=$2; shift 2
  [ -d "$PZ_NATIVES" ] || { echo "FAIL $name: no natives in $PZ_NATIVES (set PZ_NATIVES)"; fail=1; return; }
  javac -nowarn -encoding UTF-8 -cp "$ROOT/projectzomboid.jar" -sourcepath tests/original_probe -d target/original-probe "tests/original_probe/$probe.java" 2>target/oracle/$name.javac \
    || { cat target/oracle/$name.javac; echo "FAIL $name (javac)"; fail=1; return; }
  scratch="$HERE/target/oracle/pz-oracle-$name"; rm -rf "$scratch"
  (cd "$ROOT" && java -Djava.awt.headless=true --enable-final-field-mutation=ALL-UNNAMED -Djava.library.path="$PZ_NATIVES" -Xmx4g -cp "$HERE/target/original-probe:projectzomboid.jar" "$probe" "$scratch" "$@" >"$HERE/target/oracle/$name.java.txt" 2>"$HERE/target/oracle/$name.java.err") \
    || { tail -5 target/oracle/$name.java.err; echo "FAIL $name (game probe)"; fail=1; return; }
  ./target/release/pz-dump "$name" "$ROOT" "$scratch" "$@" >target/oracle/$name.rust.txt 2>target/oracle/$name.rust.err \
    || { tail -5 target/oracle/$name.rust.err; echo "FAIL $name (pz-dump)"; fail=1; return; }
  if cmp -s target/oracle/$name.java.txt target/oracle/$name.rust.txt; then
    shasum -a 256 target/oracle/$name.java.txt | cut -d' ' -f1 >tests/golden/engine/$name.sha256
    echo "OK   $name: $(wc -l <target/oracle/$name.java.txt | tr -d ' ') lines identical"
  else
    echo "FAIL $name: first differences:"; diff target/oracle/$name.java.txt target/oracle/$name.rust.txt | head -6; fail=1
  fi
}
# game_measure <name> <ProbeClass> [args...]: a game probe whose output is input data for a generator (no Rust side).
game_measure() {
  name=$1; probe=$2; shift 2
  javac -nowarn -encoding UTF-8 -cp "$ROOT/projectzomboid.jar" -d target/original-probe "tests/original_probe/$probe.java" 2>target/oracle/$name.javac \
    || { cat target/oracle/$name.javac; echo "FAIL $name (javac)"; fail=1; return; }
  scratch="$HERE/target/oracle/pz-oracle-$name"; rm -rf "$scratch"
  (cd "$ROOT" && java -Djava.awt.headless=true --enable-final-field-mutation=ALL-UNNAMED -Djava.library.path="$PZ_NATIVES" -Xmx4g -cp "$HERE/target/original-probe:projectzomboid.jar" "$probe" "$scratch" "$@" >"$HERE/target/oracle/$name.txt" 2>"$HERE/target/oracle/$name.err") \
    || { tail -5 target/oracle/$name.err; echo "FAIL $name (game probe)"; fail=1; return; }
  echo "MEASURED $name: target/oracle/$name.txt ($(wc -l <target/oracle/$name.txt | tr -d ' ') lines)"
}
want=${1:-all}
sel() { [ "$want" = all ] || [ "$want" = "$1" ]; }
sel tiledefs && run_case tiledefs TileDefsProbe
# every media/fonts/**/*.fnt (and media/*.fnt) through AngelCodeFont, and TextManager.Init per language and font size
sel fonts && game_case fonts FontsProbe
sel sandbox && run_case sandbox SandboxProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13/map_sand.bin"
sel lotheaders && run_case lotheaders LotHeadersProbe "Muldraugh, KY"
sel worlddictionary && run_case worlddictionary WorldDictionaryProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13/WorldDictionary.bin"
sel lotpack && run_case lotpack LotPackProbe "Muldraugh, KY" 30 43 34 47
# the whole map (4.2 GB of lotpacks, minutes): sh tools/oracle.sh lotpack-all
[ "$want" = lotpack-all ] && run_case lotpack LotPackProbe "Muldraugh, KY" 0 0 99 99
sel metagrid && server_case metagrid MetaGridProbe "Muldraugh, KY" 30 43 34 47
# not in the default run until it matches (first gap: one Rand draw for crafted_01_29, a water-collector entity)
sel chunkobjects && server_case chunkobjects ChunkObjectsProbe 1036 1460 1039 1463
# a whole cell (1,024 chunks): sh tools/oracle.sh chunkobjects-cell
[ "$want" = chunkobjects-cell ] && server_case chunkobjects ChunkObjectsProbe 1024 1440 1055 1471
sel collide && server_case collide CollideProbe 1036 1460 1039 1463
sel entitysprites && server_case entitysprites EntitySpritesProbe
sel specialcollide && server_case specialcollide SpecialCollideProbe 129 182
sel basements && server_case basements BasementsProbe 30 43 34 47
# every chunk of the isolated profile's save, loaded by the original server and by the port
sel chunksave && server_case chunksave ChunkSaveProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13"
# the save's basements (map_basements.bin, seeded into the server's save) and map_meta.bin, read by IsoMetaGrid.load
sel mapmeta && server_case mapmeta MapMetaProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13"
# the save's zones (map_zone.bin) read by IsoMetaGrid.loadZone into the booted world's meta grid
sel mapzone && server_case mapzone MapZoneProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13"
# the same plus the outline of every wide polyline zone (native ClipperOffset + the Clipper64 union, ported Clipper2)
sel zoneoutlines && server_case zoneoutlines MapZoneProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13" outlines
# the same plus Zone.getTotalArea of every zone: the native Clipper union and earcut triangles of polygons and outlines
sel zonetriangles && server_case zonetriangles MapZoneProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13" triangles
# the save's metagrid/metacell_*.bin (IsoMetaCell.load) and map_animals.bin (IsoMetaGrid.loadAnimalZones)
sel animalzones && server_case animalzones AnimalZonesProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13"
# the save's map_t.bin (GameTime.load with ClimateManager.load) as a single-player game reads it
sel gametime && server_case gametime GameTimeProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13"
# the save's global_mod_data.bin (GlobalModData.load) and iTrack.bin (InstanceTracker.load, exported as text and CSV)
sel moddata && server_case moddata ModDataProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13"
# the outfits of media/clothing/clothing.xml (OutfitManager) and PersistentOutfits with the save's z_outfits.bin
sel outfits && server_case outfits OutfitsProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13"
# media/fileGuidTable.xml and every clothing item file through OutfitManager.getClothingItem
sel clothingitems && server_case clothingitems ClothingItemsProbe
sel outfitrandom && server_case outfitrandom OutfitRandomProbe
sel animsets && game_case animsets AnimSetsProbe
sel luacompile && jre_case luacompile LuaCompileProbe "$HERE/tests/lua_compile_cases.txt"
sel luavm && jre_case luavm LuaVmProbe "$HERE/tests/lua_vm_cases.txt"
sel luastring && jre_case luastring LuaVmProbe "$HERE/tests/lua_string_cases.txt"
sel luaos && jre_case luaos LuaVmProbe "$HERE/tests/lua_os_cases.txt"
sel javachars && jre_case javachars JavaCharsProbe
# zombie.core.Translator: Languages.init + loadFiles for every language, getText & co., the formatter and org.json
sel translator && game_case translator TranslatorProbe "$HERE/tests/translator_cases.txt"
sel fdlibm && jre_case fdlibm FdLibmProbe
sel luaload && game_case luaload LuaLoadEnvProbe shared
sel luaserver && game_case luaserver LuaLoadEnvProbe shared server
sel luaclient && game_case luaclient LuaLoadEnvProbe shared client
sel luaboot && game_case luaboot LuaBootProbe shared client
# measurement only (no Rust side): what the original LuaManager.init exposes, the input of tools/gen_lua_expose.py
[ "$want" = luaexpose ] && game_measure luaexpose LuaExposeProbe
sel doortoggle && { cp tests/golden/engine/specialcollide.sha256 target/oracle/sc.keep 2>/dev/null; server_case specialcollide SpecialCollideProbe 129 182 toggle; mv tests/golden/engine/specialcollide.sha256 tests/golden/engine/doortoggle.sha256 2>/dev/null; cp target/oracle/sc.keep tests/golden/engine/specialcollide.sha256 2>/dev/null; }
sel los && server_case los LosProbe 129 182
sel losdoors && { server_case los LosProbe 129 182 toggle; cp tests/golden/engine/los.sha256 tests/golden/engine/losdoors.sha256 2>/dev/null; server_case los LosProbe 129 182; }
sel stairs && server_case stairs StairsProbe 126 181
# mover: a character walked over two staircases; the Rust side is the scene crate's example, not pz-dump.
mover_case() {
  args="8118.5 11615.5 0 -0.1 45 8077.5 11612.5 -0.1 0 45"
  javac -nowarn -encoding UTF-8 -cp "$ROOT/projectzomboid.jar" -d target/original-probe tests/original_probe/ServerHarness.java tests/original_probe/MoverProbe.java 2>target/oracle/mover.javac || { echo "FAIL mover (javac)"; fail=1; return; }
  (cd "$ROOT" && java -Djava.awt.headless=true --enable-final-field-mutation=ALL-UNNAMED -Djava.library.path="$PZ_NATIVES" -Xmx4g -cp "$HERE/target/original-probe:projectzomboid.jar" MoverProbe "$HERE/target/oracle/pz-oracle-mover" 126 181 $args >"$HERE/target/oracle/mover.java.txt" 2>"$HERE/target/oracle/mover.java.err") || { echo "FAIL mover (server probe)"; fail=1; return; }
  cargo run -q --release -p zomboid-scene --example mover_walk -- "$ROOT" 31 45 $args >target/oracle/mover.rust.txt 2>target/oracle/mover.rust.err || { echo "FAIL mover (rust)"; fail=1; return; }
  if cmp -s target/oracle/mover.java.txt target/oracle/mover.rust.txt; then
    cp target/oracle/mover.java.txt tests/golden/mover_stairs.txt
    echo "OK   mover: $(wc -l <target/oracle/mover.java.txt | tr -d ' ') lines identical"
  else
    echo "FAIL mover: first differences:"; diff target/oracle/mover.java.txt target/oracle/mover.rust.txt | head -6; fail=1
  fi
}
sel mover && mover_case
# movercollide: objects pushed against the walls, doors and furniture of the spawn house (and up a staircase).
movercollide_case() {
  cell=$1; rc=$2; shift 2
  javac -nowarn -encoding UTF-8 -cp "$ROOT/projectzomboid.jar" -d target/original-probe tests/original_probe/ServerHarness.java tests/original_probe/MoverCollideProbe.java 2>target/oracle/mc.javac || { echo "FAIL movercollide (javac)"; fail=1; return; }
  (cd "$ROOT" && java -Djava.awt.headless=true --enable-final-field-mutation=ALL-UNNAMED -Djava.library.path="$PZ_NATIVES" -Xmx4g -cp "$HERE/target/original-probe:projectzomboid.jar" MoverCollideProbe "$HERE/target/oracle/pz-oracle-mc" $cell "$@" >"$HERE/target/oracle/mc.java.txt" 2>"$HERE/target/oracle/mc.java.err") || { echo "FAIL movercollide (server probe)"; fail=1; return; }
  cargo run -q --release -p zomboid-scene --example mover_collide -- "$ROOT" $rc "$@" >target/oracle/mc.rust.txt 2>target/oracle/mc.rust.err || { echo "FAIL movercollide (rust)"; fail=1; return; }
  if cmp -s target/oracle/mc.java.txt target/oracle/mc.rust.txt; then
    cp target/oracle/mc.java.txt "tests/golden/mover_collide_$(echo $rc | tr ' ' '_').txt"
    echo "OK   movercollide $cell: $(wc -l <target/oracle/mc.java.txt | tr -d ' ') lines identical"
  else
    echo "FAIL movercollide $cell: first differences:"; diff target/oracle/mc.java.txt target/oracle/mc.rust.txt | head -6; fail=1
  fi
}
sel movercollide && movercollide_case "129 182" "32 45" 8303.5 11689.5 0.0697 0.0070 80 8303.5 11689.5 0.0617 0.0331 80 8303.5 11689.5 0.0443 0.0542 80 8303.5 11689.5 0.0202 0.0670 80 8303.5 11689.5 -0.0070 0.0697 80 8303.5 11689.5 -0.0331 0.0617 80 8303.5 11689.5 -0.0542 0.0443 80 8303.5 11689.5 -0.0670 0.0202 80 8303.5 11689.5 -0.0697 -0.0070 80 8303.5 11689.5 -0.0617 -0.0331 80 8303.5 11689.5 -0.0443 -0.0542 80 8303.5 11689.5 -0.0202 -0.0670 80 8303.5 11689.5 0.0070 -0.0697 80 8303.5 11689.5 0.0331 -0.0617 80 8303.5 11689.5 0.0542 -0.0443 80 8303.5 11689.5 0.0670 -0.0202 80 8288.5 11702.5 -0.05 0.01 40 8290.3 11698.2 0.06 0.06 80
sel movercollide && movercollide_case "126 181" "31 45" 8118.5 11615.5 0.001 -0.08 70
sel items && { python3 tools/gen_item_params.py >/dev/null; server_case items ItemsProbe "$HERE/target/oracle/item_fields.txt"; }
# every craftRecipe script (ScriptManager craftRecipe buckets: CraftRecipe.Load, OnScriptsLoaded, the ported part of
# OnPostWorldDictionaryInit) as the booted server holds it
sel recipes && server_case recipes RecipesProbe
# every vehicle script (VehicleScript.Load with its templates, Loaded) and vehicle template as the booted server holds it
sel vehicles && server_case vehicles VehiclesProbe
# the XUI scripts (xuiConfig, xuiSkin, ...) as ScriptManager loaded them, and XuiManager.ParseScripts' skins, styles, vars
sel xui && server_case xui XuiProbe
# mod loading over crafted mod folders in the probe's cache dir: ChooseGameInfo (mod.info, availability), the mod folders
# and their 42/42.x/common versions, ActiveMods, loadMods' order and the file map the mods' files shadow
sel mods && game_case mods ModsProbe "$HERE/tests/mods_cases.txt"
# every sound script (GameSoundScript.Load) and the GameSounds registry as the booted server holds them, then crafted
# Load bodies and GameSounds calls (tests/sounds_cases.txt); the second argument is the Rust side's scratch folder for
# the sounds.ini those commands write (the probe writes into the server's own cache folder)
sel sounds && server_case sounds SoundsProbe "$HERE/tests/sounds_cases.txt" "$HERE/target/oracle/pz-oracle-sounds-rust"
# Global object systems (gos_*.bin): the player's five real files, a file the ORIGINAL writer makes and hand-made broken
# ones; the second argument is the folder the probe (re)writes those crafted files into (tests/gos_crafted, our own
# data, committed) for the Rust side to read
sel gos && server_case gos GosProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13" "$HERE/tests/gos_crafted"
# Small world-level files (map_worldgen.bin, map_symbols.bin, statistics.bin, metadata.bin): the real files, the ones the
# ORIGINAL writers make, hand-made broken ones and the writer file cut at every length (tests/savemeta_crafted)
sel savemeta && server_case savemeta SaveMetaProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13" "$HERE/tests/savemeta_crafted"
# map_visited.bin (WorldMapVisited): the real file, the original's mutators and writer, and hand-made files of the current
# and the legacy formats (tests/visited_cases.txt); args: the commands, then the save folder for the real file
sel visited && server_case visited VisitedProbe "$HERE/tests/visited_cases.txt" "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13"
# isoregiondata/datachunk_*.bin and RegionHeader.bin (the saved part of IsoRegions): IsoRegionWorker.loadSaveMap on the real folder
# and on the scenario folders of tests/isoregion_crafted (the ORIGINAL writer's files and hand-made ones, written by the probe)
sel isoregion && server_case isoregion IsoRegionProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13" "$HERE/tests/isoregion_crafted"
# erosion.ini (ErosionConfig.readFile/writeFile): the real file and the hand-made ones of tests/erosionini_crafted
sel erosionini && server_case erosionini ErosionIniProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13" "$HERE/tests/erosionini_crafted"
# map_t.bin's GMTM header and map_ver.bin (IsoWorld.readWorldVersion, GameWindow.ReadString(DataInputStream)): the real files and the pairs the probe writes to tests/mapver_crafted
sel mapver && server_case mapver MapVerProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13" "$HERE/tests/mapver_crafted"
# zombie.scripting.ScriptParser (readBlock, stripComments, parseTokens) over crafted text
sel scriptparser && jre_case scriptparser ScriptParserProbe "$HERE/tests/script_parser_cases.txt"
exit $fail
