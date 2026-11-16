import java.io.PrintStream;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import se.krka.kahlua.vm.Coroutine;
import se.krka.kahlua.vm.JavaFunction;
import se.krka.kahlua.vm.KahluaTable;
import se.krka.kahlua.vm.KahluaTableIterator;
import se.krka.kahlua.vm.LuaClosure;
import zombie.Lua.Event;
import zombie.Lua.LuaEventManager;
import zombie.Lua.LuaManager;
import zombie.ZomboidFileSystem;
import zombie.core.random.RandStandard;

/** Oracle probe (tools/oracle.sh luaload): the ORIGINAL LuaManager.init, then LoadDirBase for each directory given
 * (the game's own media/lua, standalone: no world, no UI, no mods). Printed, in this order:
 *   LOADED <file>            LuaManager.loaded, in order
 *   ADDED G["x"]["k"] = ...  keys the scripts added to a table that a global held after init (globals by name)
 *   G["x"] = ...             globals the scripts added or replaced (by name)
 *   EVENT <name> <n>         every event with callbacks, in registration order, then its callbacks
 *   then the Java state the scripts filled (javaState): every event name, the non-empty collections of public static
 *   fields, MapObjects callbacks, TileOverlays, ContainerOverlays, BentFences, BrokenFences, RandomizedWorldBase's
 *   static lists, WorldGenUtils' file list, SGlobalObjects' system count, then the client's registries:
 *   MusicIntensityConfig, MusicThreatConfig, the CharacterJoypadButtonBinding / CharacterJoypadAxis2dBinding
 *   enum bindings and CharacterInputBindingSet.loadedBindingSets
 * Tables are walked in their own iteration order; a table met before prints as `ref <path>`, a table a global held
 * after init as `init <name>`. Values: numbers as raw bits, strings escaped, closures by file (from media/lua/) and
 * first line, Java
 * objects by class (enum constants with their name). Compared with `pz-dump luaload`.
 * Args: <scratch cache dir> <dir>... ; run from the game root. */
public class LuaLoadEnvProbe {
    static final StringBuilder sb = new StringBuilder();
    static final IdentityHashMap<Object, String> seen = new IdentityHashMap<>();
    static final IdentityHashMap<Object, String> initTables = new IdentityHashMap<>();

    static void esc(StringBuilder b, String s) {
        b.append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c >= 0x20 && c < 0x7f && c != '"' && c != '\\') b.append(c);
            else b.append(String.format("\\u%04x", (int) c));
        }
        b.append('"');
    }

    static String key(Object k) {
        if (k instanceof String s) {
            StringBuilder b = new StringBuilder();
            esc(b, s);
            return b.toString();
        }
        return value(k);
    }

    static String value(Object o) {
        if (o == null) return "nil";
        if (o instanceof Boolean b) return b ? "true" : "false";
        if (o instanceof Double d) return "d" + Long.toHexString(Double.doubleToRawLongBits(d));
        if (o instanceof String s) {
            StringBuilder b = new StringBuilder("s");
            esc(b, s);
            return b.toString();
        }
        if (o instanceof LuaClosure c) {
            int line = c.prototype.lines != null && c.prototype.lines.length > 0 ? c.prototype.lines[0] : -1;
            String f = c.prototype.filename;
            int i = f == null ? -1 : f.indexOf("/media/lua/");
            return "closure " + (i < 0 ? f : f.substring(i + 1)) + ":" + line;
        }
        if (o instanceof JavaFunction) return "jfunction " + o.getClass().getName();
        if (o instanceof Coroutine) return "coroutine";
        if (o instanceof KahluaTable) return "table";
        if (o instanceof Enum<?> e) return "userdata " + e.getDeclaringClass().getName() + "." + e.name();
        return "userdata " + o.getClass().getName();
    }

    static void dump(String path, Object v) {
        if (v instanceof KahluaTable t) {
            String init = initTables.get(t);
            if (init != null) {
                sb.append(path).append(" = init ").append(init).append('\n');
                return;
            }
            String before = seen.get(t);
            if (before != null) {
                sb.append(path).append(" = ref ").append(before).append('\n');
                return;
            }
            seen.put(t, path);
            sb.append(path).append(" = table\n");
            KahluaTableIterator it = t.iterator();
            while (it.advance()) dump(path + "[" + key(it.getKey()) + "]", it.getValue());
        } else {
            sb.append(path).append(" = ").append(value(v)).append('\n');
        }
    }

    static Object field(Object owner, Class<?> c, String name) throws Exception {
        java.lang.reflect.Field f = c.getDeclaredField(name);
        f.setAccessible(true);
        return f.get(owner);
    }

    static Object field(Object owner, String name) throws Exception {
        return field(owner, owner.getClass(), name);
    }

    static String list(Object l) {
        if (l == null) return "nil";
        StringBuilder b = new StringBuilder("[");
        boolean first = true;
        for (Object o : (Iterable<?>) l) {
            if (!first) b.append(',');
            first = false;
            b.append(value(o));
        }
        return b.append(']').toString();
    }

    static String sortedList(java.util.Collection<String> c) {
        return list(new java.util.TreeSet<>(c));
    }

    @SuppressWarnings("unchecked")
    static <V> TreeMap<String, V> sorted(Object m) {
        return new TreeMap<>((Map<String, V>) m);
    }

    static final HashMap<String, String> initCollections = new HashMap<>();

    /** The collections and maps public static fields hold, by "<class>.<field>", from the statics tables globals
     * hold. */
    static TreeMap<String, Object> staticCollections() {
        TreeMap<String, Object> r = new TreeMap<>();
        KahluaTableIterator it = LuaManager.env.iterator();
        while (it.advance()) {
            if (!(it.getValue() instanceof KahluaTable t && t.rawget("class") instanceof Class<?> c)) continue;
            KahluaTableIterator ti = t.iterator();
            while (ti.advance()) {
                Object v = ti.getValue();
                if (v instanceof java.util.Collection<?> || v instanceof Map<?, ?>) r.put(c.getName() + "." + ti.getKey(), v);
            }
        }
        return r;
    }

    /** The Java state the scripts filled while loading: private fields read by reflection, hash containers in key
     * order (their own order is not observable through the classes). */
    static void javaState(ArrayList<Event> events) throws Exception {
        sb.append("EVENTS ").append(events.size());
        for (Event e : events) sb.append(' ').append(e.name);
        sb.append('\n');
        // every collection of a public static field (as the exposer published it in a statics table of _G) that the
        // scripts changed. Texture.nullTextures (and VideoTexture's, the same set) is left out: it is the cache of
        // failed Texture.getSharedTexture lookups, which the scripts' getTexture calls fill in this renderer-less
        // state, and the port has no texture loading.
        TreeMap<String, Object> now = staticCollections();
        for (Map.Entry<String, Object> e : now.entrySet()) {
            if (e.getKey().endsWith("Texture.nullTextures")) continue;
            Object v = e.getValue();
            String was = initCollections.get(e.getKey());
            String is = v instanceof Map<?, ?> m ? "size " + m.size() : list(v);
            if (!is.equals(was)) sb.append("STATIC ").append(e.getKey()).append(' ').append(is).append('\n');
        }
        for (String which : new String[]{"onNew", "onLoad"}) {
            for (Map.Entry<String, Object> e : LuaLoadEnvProbe.<Object>sorted(field(null, zombie.Lua.MapObjects.class, which)).entrySet()) {
                List<?> fns = (List<?>) field(e.getValue(), "functions");
                gnu.trove.list.array.TShortArrayList pr = (gnu.trove.list.array.TShortArrayList) field(e.getValue(), "priority");
                sb.append("MAPOBJECTS ").append(which).append(' ').append(value(e.getKey()));
                for (int i = 0; i < fns.size(); i++) sb.append(' ').append(pr.get(i)).append('=').append(value(fns.get(i)));
                sb.append('\n');
            }
        }
        Object to = zombie.iso.TileOverlays.instance;
        for (Map.Entry<String, Object> e : LuaLoadEnvProbe.<Object>sorted(field(to, "overlayMap")).entrySet()) {
            sb.append("TILEOVERLAY ").append(value(e.getKey())).append(' ').append(value(field(e.getValue(), "tile"))).append('\n');
            for (Object en : (List<?>) field(e.getValue(), "entries")) {
                Object u = field(en, "usage");
                sb.append("  ").append(value(field(en, "room"))).append(' ').append(field(en, "chance")).append(' ')
                    .append(value(field(u, "usage"))).append(' ').append(field(u, "zOnly")).append(' ').append(field(u, "zGreaterThan")).append(' ')
                    .append(Integer.toHexString(Float.floatToRawIntBits((Float) field(u, "alpha")))).append(' ').append(field(u, "tableTop")).append(' ')
                    .append(list(field(en, "tiles"))).append('\n');
            }
        }
        for (Map.Entry<String, Object> e : LuaLoadEnvProbe.<Object>sorted(field(to, "overlayNameToUnderlyingName")).entrySet())
            sb.append("TILEUNDER ").append(value(e.getKey())).append(' ').append(list(e.getValue())).append('\n');
        Object co = zombie.iso.ContainerOverlays.instance;
        for (Map.Entry<String, Object> e : LuaLoadEnvProbe.<Object>sorted(field(co, "overlayMap")).entrySet()) {
            sb.append("CONTAINEROVERLAY ").append(value(e.getKey())).append(' ').append(value(field(e.getValue(), "name"))).append('\n');
            for (Object en : (List<?>) field(e.getValue(), "entries"))
                sb.append("  ").append(value(field(en, "room"))).append(' ').append(value(field(en, "manyItems"))).append(' ').append(value(field(en, "fewItems"))).append('\n');
        }
        for (Map.Entry<String, Object> e : LuaLoadEnvProbe.<Object>sorted(field(co, "overlayNameToUnderlyingName")).entrySet())
            sb.append("CONTAINERUNDER ").append(value(e.getKey())).append(' ').append(list(e.getValue())).append('\n');
        Object bf = zombie.iso.BentFences.getInstance();
        List<?> bentEntries = (List<?>) field(bf, "entries");
        for (int i = 0; i < bentEntries.size(); i++) {
            Object en = bentEntries.get(i);
            sb.append("BENTFENCE ").append(i).append(' ').append(field(en, "dir")).append(' ').append(field(en, "health")).append(' ')
                .append(field(en, "length")).append(' ').append(field(en, "collapsedOffset")).append(' ').append(field(en, "collapsedSizeX")).append(' ')
                .append(field(en, "collapsedSizeY")).append(' ').append(field(en, "doSmash")).append('\n');
            @SuppressWarnings("unchecked") Map<Integer, Object> stages = (Map<Integer, Object>) field(en, "stages");
            for (Map.Entry<Integer, Object> st : new TreeMap<>(stages).entrySet()) sb.append("  stage ").append(st.getKey()).append(' ').append(list(st.getValue())).append('\n');
            sb.append("  collapsed ").append(list(field(en, "collapsed"))).append('\n');
            sb.append("  debris ").append(list(field(en, "debris"))).append('\n');
        }
        @SuppressWarnings("unchecked") java.util.Collection<String> collapsedTiles = (java.util.Collection<String>) field(bf, "collapsedTiles");
        @SuppressWarnings("unchecked") java.util.Collection<String> debrisTiles = (java.util.Collection<String>) field(bf, "debrisTiles");
        sb.append("BENTCOLLAPSED ").append(sortedList(collapsedTiles)).append('\n');
        sb.append("BENTDEBRIS ").append(sortedList(debrisTiles)).append('\n');
        for (Map.Entry<String, List<?>> e : LuaLoadEnvProbe.<List<?>>sorted(field(bf, "fenceMap")).entrySet()) {
            sb.append("BENTMAP ").append(value(e.getKey()));
            for (Object en : e.getValue()) {
                int n = -1;
                for (int i = 0; i < bentEntries.size(); i++) if (bentEntries.get(i) == en) n = i;
                sb.append(' ').append(n);
            }
            sb.append('\n');
        }
        Object brf = zombie.iso.BrokenFences.getInstance();
        TreeMap<String, Object> unbroken = sorted(field(brf, "unbrokenMap"));
        for (Map.Entry<String, Object> e : unbroken.entrySet()) {
            Object t = e.getValue();
            sb.append("BROKENTILE ").append(value(e.getKey()));
            for (String f : new String[]{"self", "left", "right", "debrisN", "debrisS", "debrisW", "debrisE"}) sb.append(' ').append(list(field(t, f)));
            sb.append('\n');
        }
        for (String which : new String[]{"brokenLeftMap", "brokenRightMap", "allMap"}) {
            for (Map.Entry<String, Object> e : LuaLoadEnvProbe.<Object>sorted(field(brf, which)).entrySet()) {
                String label = "orphan";
                for (Map.Entry<String, Object> u : unbroken.entrySet()) if (u.getValue() == e.getValue()) { label = value(u.getKey()); break; }
                sb.append("BROKENMAP ").append(which).append(' ').append(value(e.getKey())).append(' ').append(label).append('\n');
            }
        }
        TreeMap<String, java.lang.reflect.Field> lists = new TreeMap<>();
        for (java.lang.reflect.Field f : zombie.randomizedWorld.RandomizedWorldBase.class.getDeclaredFields())
            if (java.lang.reflect.Modifier.isStatic(f.getModifiers()) && f.getType() == ArrayList.class) lists.put(f.getName(), f);
        for (Map.Entry<String, java.lang.reflect.Field> e : lists.entrySet()) {
            e.getValue().setAccessible(true);
            sb.append("CLUTTER ").append(e.getKey()).append(' ').append(list(e.getValue().get(null))).append('\n');
        }
        sb.append("WORLDGENFILES ").append(list(field(zombie.iso.worldgen.WorldGenUtils.INSTANCE, "files"))).append('\n');
        sb.append("SGLOBALSYSTEMS ").append(zombie.globalObjects.SGlobalObjects.getSystemCount()).append('\n');
        // the client scripts' registries: the music configs (null until getInstance), the controller bindings
        // (enum statics) and the loaded binding sets (null until getLoadedBindingSets)
        Object mic = field(null, zombie.audio.MusicIntensityConfig.class, "instance");
        if (mic == null) sb.append("MUSICINTENSITY nil\n");
        else {
            List<?> evs = (List<?>) field(mic, "events");
            sb.append("MUSICINTENSITY ").append(evs.size()).append('\n');
            for (Object e : evs)
                sb.append("  ").append(value(field(e, "id"))).append(' ').append(floatBits(field(e, "intensity"))).append(' ')
                    .append(field(e, "duration")).append(' ').append(field(e, "multiple")).append('\n');
            for (Map.Entry<String, Object> e : LuaLoadEnvProbe.<Object>sorted(field(mic, "eventById")).entrySet())
                sb.append("MUSICINTENSITYID ").append(value(e.getKey())).append(' ').append(indexOf(evs, e.getValue())).append('\n');
        }
        Object mtc = field(null, zombie.audio.MusicThreatConfig.class, "instance");
        if (mtc == null) sb.append("MUSICTHREAT nil\n");
        else {
            List<?> sts = (List<?>) field(mtc, "statusList");
            sb.append("MUSICTHREAT ").append(sts.size()).append('\n');
            for (Object st : sts)
                sb.append("  ").append(value(field(st, "id"))).append(' ').append(floatBits(field(st, "intensity"))).append(' ')
                    .append(floatBits(field(st, "intensityOverride"))).append('\n');
            for (Map.Entry<String, Object> e : LuaLoadEnvProbe.<Object>sorted(field(mtc, "statusById")).entrySet())
                sb.append("MUSICTHREATID ").append(value(e.getKey())).append(' ').append(indexOf(sts, e.getValue())).append('\n');
        }
        for (zombie.characters.CharacterJoypadButtonBinding b : zombie.characters.CharacterJoypadButtonBinding.values()) {
            Object d = field(b, zombie.characters.CharacterJoypadButtonBinding.class, "binding");
            sb.append("JOYPADBUTTONBINDING ").append(b.name()).append(' ');
            if (d == null) sb.append("nil");
            else if (d instanceof zombie.characters.CharacterJoypadButtonBinding.JoypadButtonBinding) sb.append("button ").append(enumName(field(d, "binding")));
            else sb.append(d instanceof zombie.characters.CharacterJoypadButtonBinding.Axis1dMinMaxBinding ? "axis1d " : "axis2d ")
                .append(enumName(field(d, "binding"))).append(' ').append(floatBits(field(d, "min"))).append(' ').append(floatBits(field(d, "max")));
            sb.append('\n');
        }
        for (zombie.characters.CharacterJoypadAxis2dBinding b : zombie.characters.CharacterJoypadAxis2dBinding.values())
            sb.append("JOYPADAXISBINDING ").append(b.name()).append(' ').append(enumName(b.getBinding())).append('\n');
        Object[] sets = (Object[]) field(null, zombie.characters.CharacterInputBindingSet.class, "loadedBindingSets");
        sb.append("INPUTBINDINGSETS ");
        if (sets == null) sb.append("nil");
        else {
            List<Object> names = new ArrayList<>();
            for (Object s : sets) names.add(((zombie.characters.CharacterInputBindingSet) s).getName());
            sb.append(list(names));
        }
        sb.append('\n');
    }

    static String floatBits(Object f) {
        return Integer.toHexString(Float.floatToRawIntBits((Float) f));
    }

    static String enumName(Object e) {
        return e == null ? "nil" : ((Enum<?>) e).name();
    }

    static int indexOf(List<?> l, Object o) {
        for (int i = 0; i < l.size(); i++) if (l.get(i) == o) return i;
        return -1;
    }

    public static void main(String[] a) throws Exception {
        PrintStream out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, "UTF-8");
        System.setOut(System.err);
        ZomboidFileSystem.instance.setCacheDir(a[0]);
        ZomboidFileSystem.instance.init();
        RandStandard.INSTANCE.init();
        LuaManager.init();
        HashMap<Object, Object> initGlobals = new HashMap<>();
        IdentityHashMap<Object, HashSet<Object>> initKeys = new IdentityHashMap<>();
        KahluaTableIterator it = LuaManager.env.iterator();
        while (it.advance()) {
            initGlobals.put(it.getKey(), it.getValue());
            if (it.getValue() instanceof KahluaTable t && t != LuaManager.env && it.getKey() instanceof String name) {
                HashSet<Object> keys = new HashSet<>();
                KahluaTableIterator ti = t.iterator();
                while (ti.advance()) keys.add(ti.getKey());
                initKeys.put(t, keys);
                initTables.put(t, name);
            }
        }
        initTables.put(LuaManager.env, "_G");
        for (Map.Entry<String, Object> e : staticCollections().entrySet())
            initCollections.put(e.getKey(), e.getValue() instanceof Map<?, ?> m ? "size " + m.size() : list(e.getValue()));
        for (int i = 1; i < a.length; i++) LuaManager.LoadDirBase(a[i]);
        for (Object f : LuaManager.loaded) {
            String s = f.toString();
            sb.append("LOADED ").append(s.substring(s.indexOf("/media/lua/") + 1)).append('\n');
        }
        TreeMap<String, Object> globals = new TreeMap<>();
        it = LuaManager.env.iterator();
        while (it.advance()) globals.put(key(it.getKey()), it.getKey());
        for (Map.Entry<String, Object> g : globals.entrySet()) {
            Object v = initGlobals.get(g.getValue());
            if (!(v instanceof KahluaTable t) || LuaManager.env.rawget(g.getValue()) != t || !initKeys.containsKey(t)) continue;
            HashSet<Object> had = initKeys.get(t);
            KahluaTableIterator ti = t.iterator();
            List<Object[]> added = new ArrayList<>();
            while (ti.advance()) if (!had.contains(ti.getKey())) added.add(new Object[]{ti.getKey(), ti.getValue()});
            initTables.remove(t);
            for (Object[] kv : added) dump("ADDED G[" + g.getKey() + "][" + key(kv[0]) + "]", kv[1]);
            initTables.put(t, (String) g.getValue());
        }
        for (Map.Entry<String, Object> g : globals.entrySet()) {
            Object k = g.getValue();
            Object v = LuaManager.env.rawget(k);
            if (initGlobals.containsKey(k) && initGlobals.get(k) == v) continue;
            dump("G[" + g.getKey() + "]", v);
        }
        ArrayList<Event> events = new ArrayList<>();
        LuaEventManager.getEvents(events, new HashMap<>());
        for (Event e : events) {
            if (e.callbacks.isEmpty()) continue;
            sb.append("EVENT ").append(e.name).append(" ").append(e.callbacks.size()).append('\n');
            for (LuaClosure c : e.callbacks) sb.append("  ").append(value(c)).append('\n');
        }
        javaState(events);
        out.print(sb);
        out.flush();
        Runtime.getRuntime().halt(0);
    }
}
