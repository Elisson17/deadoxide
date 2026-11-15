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
        out.print(sb);
        out.flush();
        Runtime.getRuntime().halt(0);
    }
}
