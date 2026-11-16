import java.io.File;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Random;
import java.util.TreeMap;
import zombie.GameSounds;
import zombie.SystemDisabler;
import zombie.ZomboidFileSystem;
import zombie.audio.BaseSoundBank;
import zombie.audio.GameSound;
import zombie.audio.GameSoundClip;
import zombie.core.random.RandAbstract;
import zombie.core.random.RandStandard;
import zombie.scripting.ScriptManager;
import zombie.scripting.objects.BaseScriptObject;
import zombie.scripting.objects.GameSoundScript;
import zombie.scripting.objects.SoundTimelineScript;
import zombie.vehicles.EngineRPMData;
import zombie.vehicles.VehicleEngineRPM;

// Inside the booted original (a dedicated server): the `sound` scripts as ScriptManager loaded them
// (getAllGameSounds, in its order), then the GameSounds registry GameSounds.ScriptsLoaded filled (the `sounds` list in
// order and `soundByName` by sorted key, each entry as the index of its sound in the list), the categories and the
// sounds of each, then the `soundTimeline` scripts (getAllSoundTimelines) and the `vehicleEngineRPM` scripts
// (getAllVehicleEngineRPMs). Then, when a command file is given (a[1]), crafted Load bodies of the three script types
// and GameSounds calls (see tests/sounds_cases.txt), run on the emptied registry after the real state was printed; a[2] (the Rust side's
// scratch folder) is not used here, the original writes sounds.ini into its own cache folder.
// Every field of GameSound and GameSoundClip is printed by reflection in declaration order, one line per field:
// "<path>\t<field>\t<value>"; clips as "size N" and one block per clip; the values derived from them follow as
// "derived" lines. Strings: '\\', tab, CR, LF and every UTF-16 unit outside 0x20..0x7e escaped (backslash-u escapes); floats with
// Float.toString; a GameSoundClip's owner as "sound:<name>".
public class SoundsProbe {
    static String esc(Object v) {
        if (v == null) return "null";
        String s = v.toString();
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c == '\\') sb.append("\\\\");
            else if (c == '\t') sb.append("\\t");
            else if (c == '\n') sb.append("\\n");
            else if (c == '\r') sb.append("\\r");
            else if (c < 0x20 || c > 0x7e) sb.append(String.format("\\u%04x", (int) c));
            else sb.append(c);
        }
        return sb.toString();
    }

    static String val(Object v) {
        if (v == null) return "null";
        if (v instanceof String s) return esc(s);
        if (v instanceof Float f) return Float.toString(f);
        if (v instanceof Enum<?> e) return e.name();
        if (v instanceof GameSound g) return "sound:" + esc(g.name);
        return esc(v);
    }

    static String unescape(String s) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c != '\\' || i + 1 >= s.length()) { sb.append(c); continue; }
            char n = s.charAt(++i);
            if (n == 'n') sb.append('\n');
            else if (n == 't') sb.append('\t');
            else if (n == 'r') sb.append('\r');
            else if (n == '\\') sb.append('\\');
            else if (n == 'u') { sb.append((char) Integer.parseInt(s.substring(i + 1, i + 5), 16)); i += 4; }
            else sb.append('\\').append(n);
        }
        return sb.toString();
    }

    static void fields(StringBuilder sb, String path, Object o, Class<?> c) throws Exception {
        for (Field f : c.getDeclaredFields()) {
            if (Modifier.isStatic(f.getModifiers())) continue;
            f.setAccessible(true);
            Object v = f.get(o);
            if (v instanceof List<?> l) {
                sb.append(path).append('\t').append(f.getName()).append("\tsize ").append(l.size()).append('\n');
                for (int i = 0; i < l.size(); i++) dumpClip(sb, path + ".clips[" + i + "]", (GameSoundClip) l.get(i));
            } else {
                sb.append(path).append('\t').append(f.getName()).append('\t').append(val(v)).append('\n');
            }
        }
    }

    static void dumpClip(StringBuilder sb, String path, GameSoundClip clip) throws Exception {
        fields(sb, path, clip, GameSoundClip.class);
        sb.append(path).append("\tderived\thasMin=").append(clip.hasMinDistance()).append(" hasMax=").append(clip.hasMaxDistance())
            .append(" stop=").append(clip.isStopImmediate()).append(" effective=").append(clip.getEffectiveVolume())
            .append(" effectiveMenu=").append(clip.getEffectiveVolumeInMenu()).append('\n');
    }

    static void dumpSound(StringBuilder sb, String path, GameSound g) throws Exception {
        fields(sb, path, g, GameSound.class);
        sb.append(path).append("\tderived\tmaxDistance=").append(g.getMaxDistanceOfClips()).append(" master=").append(g.getMasterName())
            .append(" looped=").append(g.isLooped()).append(" userVolume=").append(g.getUserVolume()).append('\n');
    }

    static void base(StringBuilder sb, String path, BaseScriptObject s) {
        sb.append(path).append("\tmodule\t").append(s.getModule() == null ? "null" : esc(s.getModule().getName()))
            .append("\tenabled\t").append(s.isEnabled()).append("\tdebugOnly\t").append(s.isDebugOnly())
            .append("\tscriptVersion\t").append(Long.toHexString(s.getScriptVersion()))
            .append("\tbodies\t").append(s.getLoadedScriptBodyCount()).append('\n');
    }

    static void dumpScript(StringBuilder sb, String path, GameSoundScript s) throws Exception {
        base(sb, path, s);
        dumpSound(sb, path + ".gameSound", s.gameSound);
    }

    // SoundTimelineScript: eventName and positionByName (by sorted key).
    @SuppressWarnings("unchecked")
    static void dumpTimeline(StringBuilder sb, String path, SoundTimelineScript s) throws Exception {
        base(sb, path, s);
        Field e = SoundTimelineScript.class.getDeclaredField("eventName");
        e.setAccessible(true);
        sb.append(path).append("\teventName\t").append(val(e.get(s))).append('\n');
        Field m = SoundTimelineScript.class.getDeclaredField("positionByName");
        m.setAccessible(true);
        TreeMap<String, Integer> sorted = new TreeMap<>((Map<String, Integer>) m.get(s));
        sb.append(path).append("\tpositionByName\tsize ").append(sorted.size()).append('\n');
        for (Map.Entry<String, Integer> x : sorted.entrySet())
            sb.append(path).append(".positionByName[").append(esc(x.getKey())).append("]\t=\t").append(x.getValue()).append('\n');
    }

    // VehicleEngineRPM: name and the eight rpmData slots.
    static void dumpRpm(StringBuilder sb, String path, VehicleEngineRPM s) throws Exception {
        base(sb, path, s);
        Field n = VehicleEngineRPM.class.getDeclaredField("name");
        n.setAccessible(true);
        sb.append(path).append("\tname\t").append(val(n.get(s))).append('\n');
        sb.append(path).append("\trpmData\tsize ").append(s.rpmData.length).append('\n');
        for (int i = 0; i < s.rpmData.length; i++) {
            EngineRPMData d = s.rpmData[i];
            sb.append(path).append(".rpmData[").append(i).append("]\t");
            if (d == null) sb.append("null\n");
            else sb.append("gearChange\t").append(d.gearChange).append("\tafterGearChange\t").append(d.afterGearChange).append('\n');
        }
    }

    @SuppressWarnings("unchecked")
    static List<GameSound> sounds() throws Exception {
        Field f = GameSounds.class.getDeclaredField("sounds");
        f.setAccessible(true);
        return (List<GameSound>) f.get(null);
    }

    @SuppressWarnings("unchecked")
    static Map<String, GameSound> byName() throws Exception {
        Field f = GameSounds.class.getDeclaredField("soundByName");
        f.setAccessible(true);
        return (Map<String, GameSound>) f.get(null);
    }

    static int position(GameSound g) throws Exception {
        List<GameSound> l = sounds();
        for (int i = 0; i < l.size(); i++) if (l.get(i) == g) return i;
        return -1;
    }

    static String names(List<GameSound> l) throws Exception {
        ArrayList<String> p = new ArrayList<>();
        for (GameSound g : l) p.add(Integer.toString(position(g)));
        return "[" + String.join(", ", p) + "]";
    }

    // The registry: the sounds list in order, soundByName by sorted key as the position of its sound in the list, the
    // pause flag, the categories and the sounds of each (as positions).
    static void registry(StringBuilder sb, String path, boolean full) throws Exception {
        List<GameSound> l = sounds();
        sb.append(path).append("\tsounds\tsize ").append(l.size()).append('\n');
        if (full) for (int i = 0; i < l.size(); i++) dumpSound(sb, path + ".sounds[" + i + "]", l.get(i));
        TreeMap<String, GameSound> sorted = new TreeMap<>(byName());
        sb.append(path).append("\tsoundByName\tsize ").append(sorted.size()).append('\n');
        for (Map.Entry<String, GameSound> e : sorted.entrySet())
            sb.append(path).append(".soundByName[").append(esc(e.getKey())).append("]\t=\tsounds[").append(position(e.getValue())).append("]\n");
        sb.append(path).append("\tsoundIsPaused\t").append(GameSounds.soundIsPaused).append('\n');
        ArrayList<String> cats = new ArrayList<>();
        for (String c : GameSounds.getCategories()) cats.add(esc(c));
        sb.append(path).append("\tgetCategories\t[").append(String.join(", ", cats)).append("]\n");
        for (String c : GameSounds.getCategories())
            sb.append(path).append(".getSoundsInCategory[").append(esc(c)).append("]\t").append(names(GameSounds.getSoundsInCategory(c))).append('\n');
    }

    static void call(StringBuilder sb, String what, ThrowingRunnable r) throws Exception {
        try {
            r.run();
        } catch (Throwable t) {
            sb.append(what).append("\texception\t").append(esc(t.toString())).append('\n');
        }
    }

    interface ThrowingRunnable { void run() throws Exception; }

    // The commands of tests/sounds_cases.txt: one per line, '#' lines and blank lines skipped, text arguments with \n,
    // \t, \r, \\ and backslash-u with four hex digits as escapes.
    static void crafted(StringBuilder sb, String file) throws Exception {
        HashMap<String, GameSoundScript> scripts = new HashMap<>();
        HashMap<String, SoundTimelineScript> timelines = new HashMap<>();
        HashMap<String, VehicleEngineRPM> rpms = new HashMap<>();
        File cache = new File(ZomboidFileSystem.instance.getCacheDir());
        int n = 0;
        for (String line : Files.readAllLines(new File(file).toPath(), StandardCharsets.UTF_8)) {
            if (line.isEmpty() || line.startsWith("#")) continue;
            n++;
            String[] p = line.split(" ", 2);
            String cmd = p[0];
            String rest = p.length > 1 ? p[1] : "";
            String at = "cmd[" + n + "]";
            sb.append(at).append("\t> ").append(esc(line)).append('\n');
            switch (cmd) {
                case "load" -> {
                    String[] a = rest.split(" ", 3);
                    GameSoundScript script = scripts.get(a[0]);
                    if (script == null) {
                        script = new GameSoundScript();
                        scripts.put(a[0], script);
                    }
                    GameSoundScript s = script;
                    String body = a.length > 2 ? unescape(a[2]) : "";
                    call(sb, at, () -> s.Load(a[1], body));
                    dumpScript(sb, at + ".script", s);
                }
                case "timeline" -> {
                    String[] a = rest.split(" ", 3);
                    SoundTimelineScript t = timelines.computeIfAbsent(a[0], k -> new SoundTimelineScript());
                    String body = a.length > 2 ? unescape(a[2]) : "";
                    call(sb, at, () -> t.Load(a[1], body));
                    dumpTimeline(sb, at + ".timeline", t);
                }
                case "timelinepos" -> {
                    String[] a = rest.split(" ");
                    sb.append(at).append("\tgetPosition\t").append(timelines.get(a[0]).getPosition(a[1])).append('\n');
                }
                case "timelinereset" -> {
                    SoundTimelineScript t = timelines.get(rest);
                    t.reset();
                    dumpTimeline(sb, at + ".timeline", t);
                }
                case "rpm" -> {
                    String[] a = rest.split(" ", 3);
                    VehicleEngineRPM r = rpms.computeIfAbsent(a[0], k -> new VehicleEngineRPM());
                    String body = a.length > 2 ? unescape(a[2]) : "";
                    call(sb, at, () -> r.Load(a[1], body));
                    dumpRpm(sb, at + ".rpm", r);
                }
                case "rpmreset" -> {
                    VehicleEngineRPM r = rpms.get(rest);
                    r.reset();
                    dumpRpm(sb, at + ".rpm", r);
                }
                case "scriptreset" -> {
                    GameSoundScript s = scripts.get(rest);
                    if (s == null) sb.append(at).append("\tno such script\n");
                    else {
                        s.reset();
                        dumpScript(sb, at + ".script", s);
                    }
                }
                case "add" -> {
                    GameSoundScript s = scripts.remove(rest);
                    if (s == null) sb.append(at).append("\tno such script\n");
                    else call(sb, at, () -> GameSounds.addSound(s.gameSound));
                    registry(sb, at, false);
                }
                case "get", "getnull" -> {
                    GameSound[] g = new GameSound[1];
                    String name = cmd.equals("getnull") ? null : rest;
                    call(sb, at, () -> g[0] = GameSounds.getSound(name));
                    if (g[0] == null) sb.append(at).append("\tresult\tnull\n");
                    else {
                        sb.append(at).append("\tresult\tsounds[").append(position(g[0])).append("]\n");
                        dumpSound(sb, at + ".sound", g[0]);
                    }
                    registry(sb, at, false);
                }
                case "known" -> sb.append(at).append("\tisKnownSound\t").append(GameSounds.isKnownSound(rest)).append('\n');
                case "knownnull" -> sb.append(at).append("\tisKnownSound\t").append(GameSounds.isKnownSound(null)).append('\n');
                case "cat" -> sb.append(at).append("\tgetSoundsInCategory\t").append(names(GameSounds.getSoundsInCategory(rest))).append('\n');
                case "volume" -> {
                    String[] a = rest.split(" ");
                    GameSound g = GameSounds.getSound(a[0]);
                    g.setUserVolume(Float.parseFloat(a[1]));
                    sb.append(at).append("\tuserVolume\t");
                    Field f = GameSound.class.getDeclaredField("userVolume");
                    f.setAccessible(true);
                    sb.append(f.get(g)).append("\tgetUserVolume\t").append(g.getUserVolume()).append('\n');
                    dumpSound(sb, at + ".sound", g);
                }
                case "advanced" -> SystemDisabler.setEnableAdvancedSoundOptions(Boolean.parseBoolean(rest));
                case "rand" -> {
                    String[] a = rest.split(" ");
                    GameSound g = GameSounds.getSound(a[0]);
                    Field rf = RandAbstract.class.getDeclaredField("rand");
                    rf.setAccessible(true);
                    rf.set(RandStandard.INSTANCE, new Random(Long.parseLong(a[1])));
                    for (int i = 0; i < Integer.parseInt(a[2]); i++) {
                        try {
                            GameSoundClip clip = g.getRandomClip();
                            sb.append(at).append("\tgetRandomClip\t").append(g.clips.indexOf(clip)).append('\n');
                        } catch (Throwable t) {
                            sb.append(at).append("\tgetRandomClip\texception\t").append(esc(t.toString())).append('\n');
                        }
                    }
                }
                case "ini" -> {
                    Files.write(new File(cache, "sounds.ini").toPath(), unescape(rest).getBytes(StandardCharsets.UTF_8));
                    GameSounds.loadINI();
                    Field f = GameSound.class.getDeclaredField("userVolume");
                    f.setAccessible(true);
                    List<GameSound> l = sounds();
                    for (int i = 0; i < l.size(); i++)
                        sb.append(at).append(".sounds[").append(i).append("]\tuserVolume\t").append(f.get(l.get(i))).append('\n');
                }
                case "reset" -> GameSounds.Reset();
                case "state" -> registry(sb, at, true);
                default -> sb.append(at).append("\tunknown command\n");
            }
        }
    }

    public static void main(String[] a) throws Exception {
        ServerHarness.run(a[0], out -> {
            StringBuilder sb = new StringBuilder();
            sb.append("advancedSoundOptions\t").append(SystemDisabler.getEnableAdvancedSoundOptions()).append('\n');
            sb.append("soundBank\t").append(BaseSoundBank.instance == null ? "null" : BaseSoundBank.instance.getClass().getName()).append('\n');
            ArrayList<GameSoundScript> scripts = ScriptManager.instance.getAllGameSounds();
            sb.append("scripts\tsize ").append(scripts.size()).append('\n');
            for (int i = 0; i < scripts.size(); i++) dumpScript(sb, "script[" + i + "]", scripts.get(i));
            registry(sb, "registry", true);
            ArrayList<SoundTimelineScript> timelines = ScriptManager.instance.getAllSoundTimelines();
            sb.append("soundTimelines\tsize ").append(timelines.size()).append('\n');
            for (int i = 0; i < timelines.size(); i++) dumpTimeline(sb, "timeline[" + i + "]", timelines.get(i));
            ArrayList<VehicleEngineRPM> rpms = ScriptManager.instance.getAllVehicleEngineRPMs();
            sb.append("vehicleEngineRPMs\tsize ").append(rpms.size()).append('\n');
            for (int i = 0; i < rpms.size(); i++) dumpRpm(sb, "rpm[" + i + "]", rpms.get(i));
            if (a.length > 1) {
                GameSounds.Reset();
                crafted(sb, a[1]);
            }
            out.print(sb);
        });
    }
}
