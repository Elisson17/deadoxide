import java.io.File;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import zombie.AchievementManager;
import zombie.AmbientStreamManager;
import zombie.ZomboidFileSystem;
import zombie.inventory.types.MapItem;
import zombie.iso.Helicopter;
import zombie.iso.IsoWorld;
import zombie.iso.MetaTracker;
import zombie.iso.worldgen.WorldGenParams;
import zombie.statistics.Achievement;
import zombie.statistics.Statistic;
import zombie.statistics.StatisticCategory;
import zombie.statistics.StatisticType;
import zombie.statistics.StatisticsManager;
import zombie.worldMap.symbols.WorldMapBaseSymbol;
import zombie.worldMap.symbols.WorldMapSymbols;
import zombie.worldMap.symbols.WorldMapTextSymbol;

// Inside the booted original (ServerHarness): four small world-level save files, each read by the ORIGINAL loader
// from the server's current save folder, and the state it leaves printed.
//   WGEN  map_worldgen.bin   WorldGenParams.INSTANCE.load()    (seed text, cell bounds)
//   MSYM  map_symbols.bin    MapItem.LoadWorldMap()            (the world map's symbols)
//   STAT  statistics.bin     StatisticsManager.load()          (statistics and the achievements they unlock)
//   META  metadata.bin       Helicopter.load + AmbientStreamManager.load, the body of MetaTracker.load (the class's
//                            own load() is called too, on the real file, for the dedicated server's helicopter)
// Each family has (1) the player's real file (the first argument after the scratch dir is the save folder), (2) files
// the ORIGINAL writers make from values chosen here (WorldGenParams.save, MapItem.SaveWorldMap, StatisticsManager.save,
// Helicopter.save + AmbientStreamManager.save), copied to <crafted dir>/<tag>_craft*.bin, (3) hand-made files
// <tag>_<name>.bin (bad magic, unknown versions and types, wrong enum names, malformed text), and (4) the first writer
// file cut at every length (printed as "cut" lines). The Rust side lists the crafted dir and reads it by file name.
// Args: <scratch dir> <save dir> <crafted dir>
public class SaveMetaProbe {
    static Object g(Object o, String name) throws Exception {
        for (Class<?> c = o.getClass(); c != null; c = c.getSuperclass()) {
            try {
                Field f = c.getDeclaredField(name);
                f.setAccessible(true);
                return f.get(o);
            } catch (NoSuchFieldException e) {
                // try the superclass
            }
        }
        throw new NoSuchFieldException(o.getClass().getName() + "." + name);
    }

    static String fh(float f) { return Integer.toHexString(Float.floatToRawIntBits(f)); }
    static String fh(Object f) { return fh(((Float) f).floatValue()); }
    static String b(Object o) { return ((Boolean) o) ? "1" : "0"; }

    // printable ASCII except quote and backslash stays; everything else is a backslash-u escape of its UTF-16 unit
    static String esc(String s) {
        if (s == null) return "null";
        StringBuilder sb = new StringBuilder("\"");
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c >= 0x20 && c < 0x7f && c != '"' && c != '\\') sb.append(c);
            else sb.append("\\").append(String.format("u%04x", (int) c));
        }
        return sb.append('"').toString();
    }

    static File cur(String name) { return new File(ZomboidFileSystem.instance.getFileNameInCurrentSave(name)); }
    static void put(String name, byte[] data) throws Exception { Files.write(cur(name).toPath(), data); }
    static byte[] read(File f) throws Exception { return Files.readAllBytes(f.toPath()); }

    static byte[] bytes(int... v) {
        byte[] r = new byte[v.length];
        for (int i = 0; i < v.length; i++) r[i] = (byte) v[i];
        return r;
    }
    static byte[] cat(byte[]... parts) {
        int n = 0;
        for (byte[] p : parts) n += p.length;
        byte[] r = new byte[n];
        int o = 0;
        for (byte[] p : parts) { System.arraycopy(p, 0, r, o, p.length); o += p.length; }
        return r;
    }
    static byte[] i32(int v) { return bytes(v >>> 24, v >>> 16, v >>> 8, v); }
    static byte[] i16(int v) { return bytes(v >>> 8, v); }
    static byte[] f32(float v) { return i32(Float.floatToRawIntBits(v)); }
    static byte[] str(String s) { byte[] u = s.getBytes(java.nio.charset.StandardCharsets.UTF_8); return cat(i16(u.length), u); }
    static byte[] utf(String s) { try { java.io.ByteArrayOutputStream o = new java.io.ByteArrayOutputStream(); new java.io.DataOutputStream(o).writeUTF(s); return o.toByteArray(); } catch (Exception e) { throw new RuntimeException(e); } }

    static void write(File dir, String name, byte[] data) throws Exception { Files.write(new File(dir, name).toPath(), data); }

    static String hash(String s) { return Integer.toHexString(s.hashCode()); }

    // ---- WGEN ----
    static void wgenLoad(PrintStream out, String label, byte[] data) throws Exception {
        cur("map_worldgen.bin").delete();
        if (data != null) put("map_worldgen.bin", data);
        WorldGenParams p = WorldGenParams.INSTANCE;
        p.setSeedString("");
        p.setMinXCell(-250); p.setMinYCell(-250); p.setMaxXCell(250); p.setMaxYCell(250);
        WorldGenParams.Result r = p.load();
        out.println("WGEN " + label + " " + r + " seed=" + esc(p.getSeedString()) + " " + p.getSeed() + " cells=" + p.getMinXCell() + "," + p.getMinYCell()
            + "," + p.getMaxXCell() + "," + p.getMaxYCell());
    }

    static byte[] wgenWrite(String seed, int a, int bb2, int c, int d) throws Exception {
        WorldGenParams p = WorldGenParams.INSTANCE;
        p.setSeedString(seed);
        p.setMinXCell(a); p.setMinYCell(bb2); p.setMaxXCell(c); p.setMaxYCell(d);
        cur("map_worldgen.bin").delete();
        p.save();
        return read(cur("map_worldgen.bin"));
    }

    static void wgen(PrintStream out, File save, File crafted) throws Exception {
        wgenLoad(out, "missing", null);
        wgenLoad(out, "real", read(new File(save, "map_worldgen.bin")));
        write(crafted, "wgen_craft.bin", wgenWrite("DeadOxide-Seed 42", -3, -4, 5, 6));
        write(crafted, "wgen_craft2.bin", wgenWrite("", Integer.MIN_VALUE, Integer.MAX_VALUE, 0, -1));
        write(crafted, "wgen_craft3.bin", wgenWrite("Séed 中文 😀 end", 10, 20, 30, 40));
        write(crafted, "wgen_badmagic.bin", cat(bytes('W', 'G', 'E', 'X'), i32(249), str("x"), i32(1), i32(2), i32(3), i32(4)));
        write(crafted, "wgen_empty.bin", new byte[0]);
        write(crafted, "wgen_magic.bin", bytes('W', 'G', 'E', 'N'));
        write(crafted, "wgen_ver0.bin", cat(bytes('W', 'G', 'E', 'N'), i32(0), str("abc"), i32(1), i32(2), i32(3), i32(4)));
        write(crafted, "wgen_newer.bin", cat(bytes('W', 'G', 'E', 'N'), i32(9999), str("abc"), i32(1), i32(2), i32(3), i32(4)));
        write(crafted, "wgen_badutf8.bin", cat(bytes('W', 'G', 'E', 'N'), i32(249), i16(5), bytes(0xc3, 0x28, 0xff, 0xe4, 0xb8), i32(1), i32(2), i32(3), i32(4)));
        write(crafted, "wgen_longseed.bin", cat(bytes('W', 'G', 'E', 'N'), i32(249), i16(500), bytes('a', 'b', 'c'), i32(1), i32(2), i32(3), i32(4)));
        write(crafted, "wgen_negseed.bin", cat(bytes('W', 'G', 'E', 'N'), i32(249), i16(-7), i32(1), i32(2), i32(3), i32(4)));
        write(crafted, "wgen_trailing.bin", cat(bytes('W', 'G', 'E', 'N'), i32(249), str("abc"), i32(1), i32(2), i32(3), i32(4), bytes(1, 2, 3, 4, 5)));
        File[] files = crafted.listFiles((d, n) -> n.startsWith("wgen_"));
        Arrays.sort(files);
        for (File f : files) wgenLoad(out, f.getName().substring(5, f.getName().length() - 4), read(f));
        byte[] craft = read(new File(crafted, "wgen_craft.bin"));
        for (int k = 0; k <= craft.length; k++) wgenLoad(out, "cut" + k, Arrays.copyOf(craft, k));
    }

    // ---- MSYM ----
    static void msymDump(PrintStream out, String label, MapItem m) throws Exception {
        WorldMapSymbols s = m.getSymbols();
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < s.getSymbolCount(); i++) {
            WorldMapBaseSymbol sy = s.getSymbolByIndex(i);
            sb.append('\t').append(sy.getClass().getSimpleName());
            for (String n : new String[]{"x", "y", "anchorX", "anchorY", "scale", "rotation", "r", "g", "b", "a"}) sb.append(',').append(fh(g(sy, n)));
            sb.append(',').append(b(g(sy, "collide"))).append(b(g(sy, "matchPerspective"))).append(b(g(sy, "applyZoom"))).append(',')
                .append(fh(g(sy, "minZoom"))).append(',').append(fh(g(sy, "maxZoom")));
            if (sy instanceof WorldMapTextSymbol) sb.append(',').append(esc((String) g(sy, "text"))).append(',').append(b(g(sy, "translated")))
                .append(',').append(esc((String) g(sy, "layerId")));
            else sb.append(',').append(esc((String) g(sy, "symbolId")));
        }
        out.println("MSYM " + label + " n=" + s.getSymbolCount() + sb);
    }

    static void msymLoad(PrintStream out, String label, byte[] data, boolean summary) throws Exception {
        MapItem m = MapItem.getSingleton();
        m.getSymbols().clear();
        cur("map_symbols.bin").delete();
        if (data != null) put("map_symbols.bin", data);
        // the file is read into the shared slice buffer: a string whose count runs past a truncated file reads the bytes
        // beyond it, which are whatever earlier loads left; zero them so that the result does not depend on history
        Arrays.fill(zombie.iso.SliceY.SliceBuffer.array(), (byte) 0);
        MapItem.LoadWorldMap();
        if (!summary) { msymDump(out, label, m); return; }
        java.io.ByteArrayOutputStream bo = new java.io.ByteArrayOutputStream();
        msymDump(new PrintStream(bo, true, "UTF-8"), label, m);
        String text = bo.toString("UTF-8");
        out.println("MSYMCUT " + label + " n=" + m.getSymbols().getSymbolCount() + " " + hash(text));
    }

    static void sym(WorldMapBaseSymbol y, float rot, boolean match, boolean zoom, float min, float max, boolean collide) {
        y.setRotation(rot);
        y.setMatchPerspective(match);
        y.setApplyZoom(zoom);
        if (!Float.isNaN(min)) y.setMinZoom(min);
        if (!Float.isNaN(max)) y.setMaxZoom(max);
        y.setCollide(collide);
    }

    static void msym(PrintStream out, File save, File crafted) throws Exception {
        MapItem m = MapItem.getSingleton();
        msymLoad(out, "missing", null, false);
        msymLoad(out, "real", read(new File(save, "map_symbols.bin")), false);
        // the original writer
        WorldMapSymbols s = m.getSymbols();
        s.clear();
        sym(s.addTranslatedText("Hello é 中 😀", "text-note", 8303.5f, 11689.25f, 1.0f, 0.5f, 0.25f, 1.0f), 0.0f, false, true, Float.NaN, Float.NaN, false);
        sym(s.addUntranslatedText("second note", "text-note", -1.5f, 0.0f, 0.3f, 0.6f, 0.9f, 0.1f), 400.0f, true, false, 2.5f, 30.0f, true);
        sym(s.addUntranslatedText("other font", "text-title", 100.0f, 200.0f, 0.0f, 0.0f, 0.0f, 0.0f), -45.5f, true, true, -3.0f, 12.0f, false);
        sym(s.addText("anchored", true, "text-note", 1.0f, 2.0f, 2.0f, -1.0f, 3.5f, 1.0f, 1.0f, 1.0f, 1.0f), 360.0f, false, false, 0.0f, 24.0f, true);
        sym(s.addTranslatedText("blank layer", " ", 5.0f, 6.0f, 0.2f, 0.4f, 0.8f, 1.0f), 90.0f, false, true, Float.NaN, 0.5f, false);
        sym(s.addTexture("Ambulance", 8310.0f, 11700.0f, 1.0f, 0.0f, 0.0f, 1.0f), 0.0f, false, true, Float.NaN, Float.NaN, false);
        sym(s.addTexture("NoSuchSymbolId", 1.0f, 2.0f, 0.5f, 0.5f, 1.25f, 0.1f, 0.2f, 0.3f, 0.4f), 123.456f, true, true, 5.0f, Float.NaN, true);
        sym(s.addTexture("", 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f), 0.0f, false, false, Float.NaN, Float.NaN, false);
        WorldMapBaseSymbol notUser = s.addTexture("NotSaved", 9.0f, 9.0f, 1.0f, 1.0f, 1.0f, 1.0f);
        notUser.setUserDefined(false);
        MapItem.SaveWorldMap();
        write(crafted, "msym_craft.bin", read(cur("map_symbols.bin")));
        s.clear();
        MapItem.SaveWorldMap();
        write(crafted, "msym_craft2.bin", read(cur("map_symbols.bin")));
        // hand-made
        byte[] magic = bytes('W', 'M', 'S', 'Y');
        byte[] base = cat(f32(1.0f), f32(2.0f), f32(0.0f), f32(1.0f), f32(0.5f));
        byte[] color = bytes(255, 128, 0, 64, 1);
        // version 1 text and texture symbols: no rotation, no flags, no font table, no font index
        write(crafted, "msym_v1.bin", cat(magic, i32(249), i16(1), i32(2),
            bytes(0), base, color, str("old note"), bytes(1),
            bytes(1), base, color, str("OldStamp")));
        // version 2: font table of two, a text symbol using index 1, one using index 7 (missing: the default layer)
        write(crafted, "msym_v2fonts.bin", cat(magic, i32(249), i16(2), bytes(2), str("fontA"), str("fontB"), i32(3),
            bytes(0), base, f32(30.0f), color, bytes(0b1111), f32(-5.0f), f32(99.0f), str("n1"), bytes(0), bytes(1),
            bytes(0), base, f32(-30.0f), color, bytes(0), str("n2"), bytes(1), bytes(7),
            bytes(1), base, f32(720.0f), color, bytes(2), str("Stamp")));
        write(crafted, "msym_badmagic.bin", cat(bytes('W', 'M', 'S', 'X'), i32(249), i16(2), bytes(0), i32(0)));
        write(crafted, "msym_ver0.bin", cat(magic, i32(249), i16(0), i32(0)));
        write(crafted, "msym_ver3.bin", cat(magic, i32(249), i16(3), bytes(0), i32(0)));
        write(crafted, "msym_badtype.bin", cat(magic, i32(249), i16(2), bytes(0), i32(2), bytes(1), base, f32(0.0f), color, bytes(0), str("ok"), bytes(5), base));
        write(crafted, "msym_negtype.bin", cat(magic, i32(249), i16(2), bytes(0), i32(1), bytes(0xff)));
        write(crafted, "msym_count.bin", cat(magic, i32(249), i16(2), bytes(0), i32(1000), bytes(1), base, f32(0.0f), color, bytes(0), str("only one")));
        write(crafted, "msym_negcount.bin", cat(magic, i32(249), i16(2), bytes(0), i32(-3)));
        write(crafted, "msym_header.bin", cat(magic, i32(249)));
        write(crafted, "msym_empty.bin", new byte[0]);
        write(crafted, "msym_nan.bin", cat(magic, i32(249), i16(2), bytes(0), i32(1), bytes(1), f32(Float.NaN), f32(Float.POSITIVE_INFINITY), f32(-0.0f), f32(1.0e30f), f32(Float.MIN_VALUE),
            f32(Float.NaN), color, bytes(12), f32(Float.NaN), f32(Float.POSITIVE_INFINITY), str("NaN")));
        File[] files = crafted.listFiles((d, n) -> n.startsWith("msym_"));
        Arrays.sort(files);
        for (File f : files) msymLoad(out, f.getName().substring(5, f.getName().length() - 4), read(f), false);
        byte[] craft = read(new File(crafted, "msym_craft.bin"));
        for (int k = 0; k <= craft.length; k++) msymLoad(out, "cut" + k, Arrays.copyOf(craft, k), true);
    }

    // ---- STAT ----
    static void statDump(PrintStream out, String label, String tag) throws Exception {
        StatisticsManager sm = StatisticsManager.getInstance();
        TreeMap<String, Statistic> sorted = new TreeMap<>(sm.getStatistics());
        StringBuilder sb = new StringBuilder();
        for (Map.Entry<String, Statistic> e : sorted.entrySet()) {
            Statistic st = e.getValue();
            sb.append('\t').append(esc(e.getKey())).append(',').append(esc(st.getName())).append(',').append(st.getStatisticType().name()).append(',')
                .append(st.getStatisticCategory().name()).append(',').append(fh(st.getValue()));
        }
        TreeMap<String, Achievement> ach = new TreeMap<>(AchievementManager.getInstance().getAchievements());
        StringBuilder ab = new StringBuilder();
        for (Map.Entry<String, Achievement> e : ach.entrySet()) ab.append(e.getValue().isUnlocked() ? '1' : '0');
        if (tag.equals("STAT")) out.println("STAT " + label + " n=" + sorted.size() + " ach=" + ab + sb);
        else out.println("STATCUT " + label + " n=" + sorted.size() + " ach=" + ab + " " + hash(sb.toString()));
    }

    static void statLoad(PrintStream out, String label, byte[] data, String tag) throws Exception {
        cur("statistics.bin").delete();
        if (data != null) put("statistics.bin", data);
        StatisticsManager.getInstance().load();
        statDump(out, label, tag);
    }

    static void stat(PrintStream out, File save, File crafted) throws Exception {
        statLoad(out, "missing", null, "STAT");
        statLoad(out, "real", read(new File(save, "statistics.bin")), "STAT");
        StatisticsManager sm = StatisticsManager.getInstance();
        // the original writer
        sm.getStatistics().clear();
        sm.setStatistic(StatisticType.Player, StatisticCategory.Travel, "Distance Walked", 1234.5f);
        sm.setStatistic(StatisticType.Player, StatisticCategory.Travel, "Distance Ran", 9.99f);
        sm.setStatistic(StatisticType.Zombie, StatisticCategory.Combat, "Zombie Kills é中", 0.0f);
        sm.setStatistic(StatisticType.World, StatisticCategory.PVP, "nan", Float.NaN);
        sm.setStatistic(StatisticType.Server, StatisticCategory.Undefined, "", -0.0f);
        sm.setStatistic(StatisticType.Undefined, StatisticCategory.Events, "inf", Float.POSITIVE_INFINITY);
        sm.setStatistic(StatisticType.Player, StatisticCategory.General, "Shots Fired", 10.0f);
        sm.setStatistic(StatisticType.Player, StatisticCategory.Vehicle, "Distance Driven", Float.NEGATIVE_INFINITY);
        sm.setStatistic(StatisticType.Player, StatisticCategory.Items, "emoji 😀 nul \u0000 end", 1.0e-30f);
        cur("statistics.bin").delete();
        sm.save();
        write(crafted, "stat_craft.bin", read(cur("statistics.bin")));
        sm.getStatistics().clear();
        cur("statistics.bin").delete();
        sm.save();
        write(crafted, "stat_craft2.bin", read(cur("statistics.bin")));
        // hand-made
        byte[] one = cat(utf("Distance Walked"), utf("Player"), utf("Travel"), f32(10.0f));
        write(crafted, "stat_exact10.bin", cat(i32(1), one));
        write(crafted, "stat_badtype.bin", cat(i32(2), one, utf("x"), utf("Bogus"), utf("Travel"), f32(1.0f)));
        write(crafted, "stat_badcat.bin", cat(i32(2), one, utf("x"), utf("Player"), utf("travel"), f32(1.0f)));
        write(crafted, "stat_dupes.bin", cat(i32(3), utf("a"), utf("Player"), utf("Travel"), f32(1.0f), utf("b"), utf("World"), utf("Items"), f32(2.0f),
            utf("a"), utf("Zombie"), utf("Health"), f32(3.0f)));
        write(crafted, "stat_count.bin", cat(i32(1000), one));
        write(crafted, "stat_negcount.bin", cat(i32(-1), one));
        write(crafted, "stat_empty.bin", new byte[0]);
        write(crafted, "stat_zero.bin", i32(0));
        write(crafted, "stat_trailing.bin", cat(i32(1), one, bytes(1, 2, 3)));
        write(crafted, "stat_badutf.bin", cat(i32(1), i16(2), bytes(0xc3, 0x28), utf("Player"), utf("Travel"), f32(1.0f)));
        write(crafted, "stat_modutf.bin", cat(i32(1), i16(4), bytes(0xc0, 0x80, 0x41, 0x42), utf("Player"), utf("Travel"), f32(1.0f)));
        write(crafted, "stat_achieve.bin", cat(i32(4), utf("Distance Ran"), utf("Player"), utf("Travel"), f32(9.999999f), utf("Distance Driven"), utf("Player"), utf("Vehicle"), f32(100.0f),
            utf("Shots Fired"), utf("Player"), utf("Combat"), f32(Float.NaN), utf("Distance Walked"), utf("Player"), utf("Travel"), f32(10.0f)));
        File[] files = crafted.listFiles((d, n) -> n.startsWith("stat_"));
        Arrays.sort(files);
        for (File f : files) statLoad(out, f.getName().substring(5, f.getName().length() - 4), read(f), "STAT");
        byte[] craft = read(new File(crafted, "stat_craft.bin"));
        for (int k = 0; k <= craft.length; k++) statLoad(out, "cut" + k, Arrays.copyOf(craft, k), "STATCUT");
    }

    // ---- META ----
    static String heli(Helicopter h) throws Exception {
        Object st = g(h, "state");
        return "active=" + b(g(h, "active")) + " state=" + (st == null ? "null" : ((Enum<?>) st).name()) + " x=" + fh(g(h, "x")) + " y=" + fh(g(h, "y"));
    }

    static String alarms(AmbientStreamManager a) throws Exception {
        StringBuilder sb = new StringBuilder();
        sb.append("alarms=").append(a.alarmList.size());
        for (zombie.iso.Alarm al : a.alarmList) sb.append(' ').append(g(al, "x")).append(',').append(g(al, "y")).append(',').append(fh(g(al, "endGameTime")));
        return sb.toString();
    }

    static void metaLoad(PrintStream out, String label, byte[] data, boolean summary) throws Exception {
        Helicopter h = new Helicopter();
        AmbientStreamManager a = new AmbientStreamManager();
        if (data != null) {
            try {
                ByteBuffer bb = ByteBuffer.wrap(data);
                int worldVersion = bb.getInt();
                h.load(bb, worldVersion);
                a.load(bb, worldVersion);
            } catch (Throwable t) {
                // MetaTracker.load logs it
            }
        }
        String text = heli(h) + " " + alarms(a);
        out.println((summary ? "METACUT " : "META ") + label + " " + (summary ? hash(text) : text));
    }

    static void meta(PrintStream out, File save, File crafted) throws Exception {
        metaLoad(out, "missing", null, false);
        metaLoad(out, "real", read(new File(save, "metadata.bin")), false);
        // MetaTracker.load itself, on the real file, into the server's own helicopter (the dedicated server's ambient manager is a no-op)
        put("metadata.bin", read(new File(save, "metadata.bin")));
        MetaTracker.load();
        out.println("METATRACKER real " + heli(IsoWorld.instance.helicopter));
        // the original writers (MetaTracker.save is skipped on a server, so its body is run here)
        Helicopter h = new Helicopter();
        h.load(wrap(cat(bytes(1), i32(2), f32(8303.5f), f32(-11689.25f))), 249);
        AmbientStreamManager a = new AmbientStreamManager();
        a.alarmList.add(new zombie.iso.Alarm(8300, 11690));
        a.alarmList.add(new zombie.iso.Alarm(-5, Integer.MAX_VALUE));
        ByteBuffer bb = ByteBuffer.allocate(10000);
        bb.putInt(249);
        h.save(bb);
        a.save(bb);
        bb.flip();
        write(crafted, "meta_craft.bin", Arrays.copyOf(bb.array(), bb.limit()));
        bb = ByteBuffer.allocate(10000);
        bb.putInt(249);
        new Helicopter().save(bb);
        new AmbientStreamManager().save(bb);
        bb.flip();
        write(crafted, "meta_craft2.bin", Arrays.copyOf(bb.array(), bb.limit()));
        // hand-made
        byte[] alarm = cat(i32(1), i32(2), f32(3.5f));
        write(crafted, "meta_state4.bin", cat(i32(249), bytes(1), i32(4), f32(1.0f), f32(2.0f), i16(0)));
        write(crafted, "meta_stateneg.bin", cat(i32(249), bytes(1), i32(-1), f32(1.0f), f32(2.0f), i16(0)));
        write(crafted, "meta_flag7.bin", cat(i32(249), bytes(7), i32(3), f32(1.0f), f32(2.0f), i16(1), alarm));
        write(crafted, "meta_count.bin", cat(i32(249), bytes(0), i32(0), f32(1.0f), f32(2.0f), i16(5), alarm, alarm));
        write(crafted, "meta_negcount.bin", cat(i32(249), bytes(0), i32(0), f32(1.0f), f32(2.0f), i16(-4), alarm));
        write(crafted, "meta_nan.bin", cat(i32(249), bytes(1), i32(1), f32(Float.NaN), f32(Float.NEGATIVE_INFINITY), i16(1), i32(0), i32(0), f32(Float.NaN)));
        write(crafted, "meta_noalarms.bin", cat(i32(249), bytes(0), i32(0), f32(1.0f), f32(2.0f)));
        write(crafted, "meta_empty.bin", new byte[0]);
        write(crafted, "meta_version.bin", i32(249));
        write(crafted, "meta_oldver.bin", cat(i32(1), bytes(0), i32(0), f32(1.0f), f32(2.0f), i16(1), alarm));
        write(crafted, "meta_trailing.bin", cat(i32(249), bytes(0), i32(0), f32(1.0f), f32(2.0f), i16(0), bytes(9, 9, 9)));
        File[] files = crafted.listFiles((d, n) -> n.startsWith("meta_"));
        Arrays.sort(files);
        for (File f : files) metaLoad(out, f.getName().substring(5, f.getName().length() - 4), read(f), false);
        byte[] craft = read(new File(crafted, "meta_craft.bin"));
        for (int k = 0; k <= craft.length; k++) metaLoad(out, "cut" + k, Arrays.copyOf(craft, k), true);
    }

    static ByteBuffer wrap(byte[] d) { return ByteBuffer.wrap(d); }

    public static void main(String[] a) throws Exception {
        File save = new File(a[1]);
        File crafted = new File(a[2]);
        ServerHarness.run(a[0], out -> {
            crafted.mkdirs();
            for (File f : crafted.listFiles()) f.delete();
            wgen(out, save, crafted);
            msym(out, save, crafted);
            stat(out, save, crafted);
            meta(out, save, crafted);
        });
    }
}
