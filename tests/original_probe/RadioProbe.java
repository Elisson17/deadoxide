import java.io.File;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Random;
import java.util.TreeMap;
import se.krka.kahlua.vm.KahluaTable;
import zombie.GameTime;
import zombie.SandboxOptions;
import zombie.ZomboidFileSystem;
import zombie.Lua.LuaManager;
import zombie.core.Core;
import zombie.core.Translator;
import zombie.core.random.Rand;
import zombie.core.random.RandAbstract;
import zombie.core.random.RandStandard;
import zombie.iso.weather.ClimateManager;
import zombie.radio.RadioData;
import zombie.radio.RadioTranslationData;
import zombie.radio.ZomboidRadio;
import zombie.radio.scripting.DynamicRadioChannel;
import zombie.radio.scripting.RadioBroadCast;
import zombie.radio.scripting.RadioChannel;
import zombie.radio.scripting.RadioLine;
import zombie.radio.scripting.RadioScript;
import zombie.radio.scripting.RadioScriptManager;

// Inside the booted original (ServerHarness): the radio. A commands file drives the ORIGINAL classes (ZomboidRadio,
// RadioScriptManager, RadioChannel, RadioScript, RadioBroadCast, RadioLine, RadioData, RadioTranslationData) and the
// state is printed after each step; the Rust side runs the same commands on the port. The global Rand is replaced by a
// java.util.Random(seed) with a draw count (as ClimateProbe does) so that every draw is reproducible and counted ("id").
//   seed n | sandbox n | gamemode text | nights n | hour h | weather running strength thunder tropical blizzard
//   flags disableBroadcasting louisvilleObfuscation | multiplier f | dynamic reset | dynamic saved freq
//   radiosave <file>|none            put crafted/<file> where RADIO_SAVE.txt is (or remove it)
//   init v                           ZomboidRadio.Reset(), getInstance(), Init(v): the whole radio of a world
//   state | fulldump                 the radio's fields and its channels / and every script, broadcast and line
//   updatescripts h m | listen f 0|1 tv 0|1 | tick n | saveto name | randcheck
//   scramble n bb custom text | computerize text | bzzt | randfreq a b | addname name f cat ow | removename f
//   xml file | xchan i time ts day | xchan i listen 0|1 | xchan i update n | xtrans key=value | trans file
// Probe-side shortcuts, all named in docs/COMPATIBILITY.md: the Lua handlers of OnLoadRadioScripts (ISDynamicRadio.lua,
// ISWeatherChannel.lua) run for real here, and the port stands in for them; the weather interference is set through the
// weather period's own fields.
// Args: <scratch dir> <commands file> <crafted dir>
public class RadioProbe {
    final PrintStream out;
    final File crafted;
    int step = 0;
    RadioData xmlData;
    List<RadioChannel> xmlChannels = new ArrayList<>();

    RadioProbe(PrintStream out, File crafted) {
        this.out = out;
        this.crafted = crafted;
    }

    static Object g(Object o, String name) throws Exception {
        Class<?> c = o.getClass();
        while (c != null) {
            try {
                Field f = c.getDeclaredField(name);
                f.setAccessible(true);
                return f.get(o);
            } catch (NoSuchFieldException e) {
                c = c.getSuperclass();
            }
        }
        throw new NoSuchFieldException(name);
    }

    static void set(Object o, Class<?> c, String name, Object v) throws Exception {
        Field f = c.getDeclaredField(name);
        f.setAccessible(true);
        f.set(o, v);
    }

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

    static String hx(float f) { return Integer.toHexString(Float.floatToRawIntBits(f)); }

    static String id(RadioBroadCast b) { return b == null ? "null" : b.getID(); }

    static int randId() throws Exception { return (Integer) g(RandStandard.INSTANCE, "id"); }

    static File saveFile() { return new File(ZomboidFileSystem.instance.getFileNameInCurrentSave("radio", "data", "RADIO_SAVE.txt")); }

    static String exc(Throwable t) {
        if (t instanceof InvocationTargetException && t.getCause() != null) t = t.getCause();
        return t.toString();
    }

    // ---- dumps ----

    void line(String tag, String label, String rest) { out.println(tag + " " + label + " " + rest); }

    void dumpBroadcast(String label, String where, RadioBroadCast b, int depth) throws Exception {
        List<RadioLine> lines = b.getLines();
        line("BC", label, where + " id=" + b.getID() + " start=" + b.getStartStamp() + " end=" + b.getEndStamp()
            + " pre=" + id((RadioBroadCast) g(b, "preSegment")) + " post=" + id((RadioBroadCast) g(b, "postSegment"))
            + " lines=" + lines.size() + " lc=" + b.getCurrentLineNumber()
            + " donepre=" + g(b, "hasDonePreSegment") + " donepost=" + g(b, "hasDonePostPause"));
        for (int j = 0; j < lines.size(); j++) {
            RadioLine l = lines.get(j);
            line("LN", label, where + " " + j + " r=" + hx(l.getR()) + " g=" + hx(l.getG()) + " b=" + hx(l.getB())
                + " air=" + hx(l.getAirTime()) + " fx=" + esc(l.getEffectsString()) + " text=" + esc(l.getText()));
        }
        if (depth < 2) {
            RadioBroadCast pre = (RadioBroadCast) g(b, "preSegment");
            RadioBroadCast post = (RadioBroadCast) g(b, "postSegment");
            if (pre != null) dumpBroadcast(label, where + "/pre", pre, depth + 1);
            if (post != null) dumpBroadcast(label, where + "/post", post, depth + 1);
        }
    }

    void channelLine(String label, RadioChannel c) throws Exception {
        RadioScript cur = c.getCurrentScript();
        RadioBroadCast air = c.getAiringBroadcast();
        Object rd = c.getRadioData();
        line("CH", label, c.GetFrequency() + " id=" + c.getGUID() + " name=" + esc(c.GetName()) + " cat=" + c.GetCategory().name()
            + " tv=" + c.IsTv() + " vanilla=" + c.isVanilla() + " rd=" + (rd == null ? "null" : String.valueOf(((RadioData) rd).isVanilla()))
            + " dynamic=" + (c instanceof DynamicRadioChannel) + " obf=" + g(c, "louisvilleObfuscate") + " synced=" + c.isTimeSynced()
            + " listening=" + c.GetPlayerIsListening() + " cur=" + (cur == null ? "null" : cur.GetName())
            + " loop=" + c.getCurrentScriptLoop() + " max=" + c.getCurrentScriptMaxLoops()
            + " airing=" + id(air) + "@" + (air == null ? -1 : air.getCurrentLineNumber())
            + " counter=" + hx((Float) g(c, "airCounter")) + " lastline=" + esc(c.getLastAiredLine()) + " lastbc=" + c.getLastBroadcastID()
            + " mult=" + hx(c.getAirCounterMultiplier()) + " minmax=" + hx((Float) g(c, "minmod")) + "," + hx((Float) g(c, "maxmod")));
    }

    void scriptLine(String label, RadioChannel c, RadioScript s) throws Exception {
        StringBuilder ex = new StringBuilder("[");
        for (RadioScript.ExitOption o : s.getExitOptions()) {
            if (ex.length() > 1) ex.append(",");
            ex.append(o.getScriptname()).append(":").append(o.getChance()).append(":").append(o.getStartDelay());
        }
        ex.append("]");
        line("SC", label, c.GetFrequency() + " " + s.GetName() + " guid=" + s.GetGUID() + " loops=" + s.getLoopMin() + "-" + s.getLoopMax()
            + " startday=" + s.getStartDay() + " stamp=" + s.getStartDayStamp() + " internal=" + g(s, "internalStamp")
            + " curbc=" + id(s.getCurrentBroadcast()) + " aired=" + g(s, "currentHasAired") + " exits=" + ex + " bcs=" + s.getBroadcastList().size());
    }

    void dumpChannel(String label, RadioChannel c) throws Exception {
        channelLine(label, c);
        Map<String, RadioScript> scripts = new TreeMap<>((Map<String, RadioScript>) g(c, "scripts"));
        for (RadioScript s : scripts.values()) {
            scriptLine(label, c, s);
            List<RadioBroadCast> bcs = s.getBroadcastList();
            for (int i = 0; i < bcs.size(); i++) dumpBroadcast(label, c.GetFrequency() + " " + s.GetName() + " " + i, bcs.get(i), 0);
        }
    }

    void compactChannel(String label, RadioChannel c) throws Exception {
        channelLine(label, c);
        RadioScript s = c.getCurrentScript();
        if (s != null) scriptLine(label, c, s);
    }

    void state(String label) throws Exception {
        ZomboidRadio zr = ZomboidRadio.getInstance();
        RadioScriptManager m = zr.getScriptManager();
        line("ST", label, "days=" + g(zr, "daysSinceStart") + " lastHour=" + g(zr, "lastRecordedHour") + " silence=" + ZomboidRadio.postRadioSilence
            + " disable=" + ZomboidRadio.disableBroadcasting + " louis=" + ZomboidRadio.louisvilleObfuscation + " mode=" + zr.getGameMode()
            + " mgr=" + (m != null) + " time=" + (m == null ? -1 : m.getCurrentTimeStamp()) + " rand=" + randId());
        line("KN", label, String.valueOf(g(zr, "knownFrequencies")));
        Map<Integer, String> names = new TreeMap<>((Map<Integer, String>) g(zr, "channelNames"));
        StringBuilder sb = new StringBuilder();
        for (Map.Entry<Integer, String> e : names.entrySet()) sb.append(e.getKey()).append(":").append(esc(e.getValue())).append(" ");
        line("NM", label, sb.toString());
        Map<String, Map<Integer, String>> cats = new TreeMap<>((Map<String, Map<Integer, String>>) g(zr, "categorizedChannels"));
        for (Map.Entry<String, Map<Integer, String>> e : cats.entrySet()) {
            StringBuilder cb = new StringBuilder();
            for (Map.Entry<Integer, String> n : new TreeMap<>(e.getValue()).entrySet()) cb.append(n.getKey()).append(":").append(esc(n.getValue())).append(" ");
            line("CT", label, e.getKey() + " " + cb);
        }
        if (m != null) for (RadioChannel c : m.getChannels().values()) compactChannel(label, c);
    }

    void fulldump(String label) throws Exception {
        state(label);
        RadioScriptManager m = ZomboidRadio.getInstance().getScriptManager();
        if (m != null) for (RadioChannel c : m.getChannels().values()) dumpChannel(label, c);
    }

    // ---- commands ----

    void dynamicReset(boolean reset, int saved) throws Exception {
        KahluaTable dyn = (KahluaTable) LuaManager.env.rawget("DynamicRadio");
        KahluaTable chans = (KahluaTable) dyn.rawget("channels");
        KahluaTable c1 = (KahluaTable) chans.rawget(1);
        KahluaTable range = LuaManager.platform.newTable();
        range.rawset(1, (Object) Double.valueOf(88000.0));
        range.rawset(2, (Object) Double.valueOf(108000.0));
        c1.rawset("freq", range);
        ((KahluaTable) dyn.rawget("cache")).wipe();
        KahluaTable md = GameTime.getInstance().getModData();
        if (reset) {
            md.rawset("dynamicRadio", null);
        } else {
            KahluaTable list = LuaManager.platform.newTable();
            KahluaTable e = LuaManager.platform.newTable();
            e.rawset("uuid", "EMRG-711984");
            e.rawset("freq", (double) saved);
            list.rawset(1, (Object) e);
            md.rawset("dynamicRadio", list);
        }
    }

    void init(int v) throws Exception {
        ZomboidRadio.getInstance().Reset();
        ZomboidRadio zr = ZomboidRadio.getInstance();
        zr.Init(v);
        line("INIT", "" + step, "v=" + v + " rand=" + randId());
    }

    void readSave(String label) throws Exception {
        File f = saveFile();
        if (!f.exists()) {
            line("SAVE", label, "unchanged");
            return;
        }
        byte[] b = Files.readAllBytes(f.toPath());
        String text = new String(b, "UTF-8");
        line("SAVE", label, "bytes=" + b.length + " hash=" + Integer.toHexString(java.util.Arrays.hashCode(b)));
        int n = 0;
        for (String l : text.split("\n", -1)) line("SL", label, (n++) + " " + esc(l));
    }

    void xml(String name) throws Exception {
        File f = new File(crafted, name);
        Method rf = RadioData.class.getDeclaredMethod("ReadFile", String.class);
        rf.setAccessible(true);
        RadioData rd = (RadioData) rf.invoke(null, f.getAbsolutePath());
        xmlData = rd;
        xmlChannels = new ArrayList<>();
        if (rd == null) {
            line("XML", name, "read=null rand=" + randId());
            return;
        }
        line("XML", name, "read=" + g(rd, "guid") + "," + g(rd, "version") + " rand=" + randId());
        Method lr = RadioData.class.getDeclaredMethod("loadRadioScripts");
        lr.setAccessible(true);
        String res;
        try {
            res = String.valueOf(lr.invoke(rd));
        } catch (InvocationTargetException e) {
            res = "exception " + exc(e);
        }
        line("XML", name, "load=" + res + " rand=" + randId());
        xmlChannels = new ArrayList<>(rd.getRadioChannels());
        for (int i = 0; i < xmlChannels.size(); i++) dumpChannel(name + "#" + i, xmlChannels.get(i));
    }

    void trans(String name) throws Exception {
        File f = new File(crafted, name);
        RadioTranslationData d = RadioTranslationData.ReadFile(f.getAbsolutePath());
        if (d == null) {
            line("TR", name, "read=null");
            return;
        }
        line("TR", name, "guid=" + esc(d.getGuid()) + " language=" + esc(d.getLanguage()) + " enum=" + d.getLanguageEnum() + " version=" + d.getVersion()
            + " translators=" + d.getTranslators() + " valid=" + d.validate());
        boolean ok = d.loadTranslations();
        line("TR", name, "load=" + ok + " count=" + d.getTranslationCount());
        for (Map.Entry<String, String> e : new TreeMap<>((Map<String, String>) g(d, "translations")).entrySet())
            line("TT", name, esc(e.getKey()) + "=" + esc(e.getValue()));
    }

    void run(String cmds) throws Exception {
        for (String raw : Files.readAllLines(new File(cmds).toPath())) {
            String l = raw.trim();
            if (l.isEmpty() || l.startsWith("#")) continue;
            step++;
            out.println("CMD " + step + " " + l);
            String[] p = l.split("\\s+");
            String rest = l.contains(" ") ? l.substring(l.indexOf(' ') + 1) : "";
            try {
                switch (p[0]) {
                    case "seed" -> {
                        set(RandStandard.INSTANCE, RandAbstract.class, "rand", new Random(Long.parseLong(p[1])));
                        set(RandStandard.INSTANCE, RandAbstract.class, "id", 0);
                    }
                    case "sandbox" -> SandboxOptions.instance.timeSinceApo.setValue(Integer.parseInt(p[1]));
                    case "gamemode" -> Core.gameMode = rest;
                    case "nights" -> GameTime.getInstance().setNightsSurvived(Integer.parseInt(p[1]));
                    case "hour" -> GameTime.getInstance().setTimeOfDay(Float.parseFloat(p[1]));
                    case "weather" -> {
                        Object wp = ClimateManager.getInstance().getWeatherPeriod();
                        set(wp, wp.getClass(), "isRunning", p[1].equals("1"));
                        set(wp, wp.getClass(), "currentStrength", Float.parseFloat(p[2]));
                        set(wp, wp.getClass(), "isThunderStorm", p[3].equals("1"));
                        set(wp, wp.getClass(), "isTropicalStorm", p[4].equals("1"));
                        set(wp, wp.getClass(), "isBlizzard", p[5].equals("1"));
                        line("WI", "" + step, hx(ClimateManager.getInstance().getWeatherInterference()));
                    }
                    case "flags" -> {
                        ZomboidRadio.disableBroadcasting = p[1].equals("1");
                        ZomboidRadio.louisvilleObfuscation = p[2].equals("1");
                    }
                    case "multiplier" -> {
                        GameTime gt = GameTime.getInstance();
                        gt.setMultiplier(Float.parseFloat(p[1]));
                        gt.fpsMultiplier = 1.0F;
                        gt.multiplierBias = 1.0F;
                        gt.perObjectMultiplier = 1.0F;
                        line("MULT", "" + step, hx(gt.getMultiplier()));
                    }
                    case "dynamic" -> {
                        if (p[1].equals("reset")) dynamicReset(true, 0);
                        else dynamicReset(false, Integer.parseInt(p[2]));
                    }
                    case "radiosave" -> {
                        File f = saveFile();
                        f.getParentFile().mkdirs();
                        f.delete();
                        if (!p[1].equals("none")) Files.copy(new File(crafted, p[1]).toPath(), f.toPath(), StandardCopyOption.REPLACE_EXISTING);
                    }
                    case "init" -> init(Integer.parseInt(p[1]));
                    case "state" -> state("" + step);
                    case "fulldump" -> fulldump("" + step);
                    case "updatescripts" -> {
                        saveFile().delete();
                        ZomboidRadio.getInstance().UpdateScripts(Integer.parseInt(p[1]), Integer.parseInt(p[2]));
                        readSave("" + step);
                        line("US", "" + step, "rand=" + randId());
                    }
                    case "listen" -> ZomboidRadio.getInstance().PlayerListensChannel(Integer.parseInt(p[1]), p[2].equals("1"), p[3].equals("1"));
                    case "tick" -> {
                        ZomboidRadio zr = ZomboidRadio.getInstance();
                        int n = Integer.parseInt(p[1]);
                        for (int i = 0; i < n; i++) {
                            zr.update();
                            for (RadioChannel c : zr.getScriptManager().getChannels().values()) {
                                if (c.getAiringBroadcast() != null)
                                    line("TK", "" + step, i + " " + c.GetFrequency() + " counter=" + hx((Float) g(c, "airCounter")) + " lc="
                                        + c.getAiringBroadcast().getCurrentLineNumber() + " last=" + esc(c.getLastAiredLine()));
                            }
                        }
                        line("TKEND", "" + step, "rand=" + randId() + " silence=" + ZomboidRadio.postRadioSilence);
                    }
                    case "saveto" -> {
                        saveFile().delete();
                        ZomboidRadio.getInstance().Save();
                        readSave("" + step);
                        if (saveFile().exists())
                            Files.copy(saveFile().toPath(), new File(crafted, p[1]).toPath(), StandardCopyOption.REPLACE_EXISTING);
                    }
                    case "randcheck" -> line("RC", "" + step, Rand.Next(1000000) + " rand=" + randId());
                    case "scramble" -> {
                        String text = l.split("\\s+", 5)[4];
                        String custom = p[3].equals("-") ? null : p[3];
                        String r = ZomboidRadio.getInstance().scrambleString(text, Integer.parseInt(p[1]), p[2].equals("1"), custom);
                        line("SCR", "" + step, esc(r) + " rand=" + randId());
                    }
                    case "computerize" -> line("CMP", "" + step, esc(ZomboidRadio.getInstance().computerize(rest)) + " rand=" + randId());
                    case "bzzt" -> line("BZ", "" + step, ZomboidRadio.getInstance().getRandomBzztFzzt() + " rand=" + randId());
                    case "randfreq" -> line("RF", "" + step, ZomboidRadio.getInstance().getRandomFrequency(Integer.parseInt(p[1]), Integer.parseInt(p[2])) + " rand=" + randId());
                    case "addname" -> ZomboidRadio.getInstance().addChannelName(p[1].replace('_', ' '), Integer.parseInt(p[2]), p[3], p[4].equals("1"));
                    case "removename" -> ZomboidRadio.getInstance().removeChannelName(Integer.parseInt(p[1]));
                    case "xml" -> xml(p[1]);
                    case "xtrans" -> {
                        String kv = rest;
                        int eq = kv.indexOf('=');
                        Field rdm = Translator.class.getDeclaredField("radioData");
                        rdm.setAccessible(true);
                        ((Map<String, String>) rdm.get(null)).put(kv.substring(0, eq), kv.substring(eq + 1));
                    }
                    case "xchan" -> {
                        RadioChannel c = xmlChannels.get(Integer.parseInt(p[1]));
                        switch (p[2]) {
                            case "time" -> c.UpdateScripts(Integer.parseInt(p[3]), Integer.parseInt(p[4]));
                            case "listen" -> c.SetPlayerIsListening(p[3].equals("1"));
                            case "update" -> {
                                int n = Integer.parseInt(p[3]);
                                for (int i = 0; i < n; i++) {
                                    c.update();
                                    line("XK", "" + step, i + " counter=" + hx((Float) g(c, "airCounter")) + " last=" + esc(c.getLastAiredLine())
                                        + " airing=" + id(c.getAiringBroadcast()));
                                }
                            }
                            default -> throw new IllegalArgumentException(l);
                        }
                        compactChannel("x" + p[1], c);
                        line("XR", "" + step, "rand=" + randId());
                    }
                    case "trans" -> trans(p[1]);
                    default -> throw new IllegalArgumentException("unknown command " + l);
                }
            } catch (Throwable t) {
                out.println("EXC " + step + " " + esc(exc(t)) + " rand=" + randId());
            }
        }
    }

    public static void main(String[] a) throws Exception {
        File crafted = new File(a[2]);
        ServerHarness.run(a[0], out -> {
            crafted.mkdirs();
            new RadioProbe(out, crafted).run(a[1]);
        });
    }
}
