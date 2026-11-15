import java.io.PrintStream;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.List;
import se.krka.kahlua.integration.expose.LuaJavaInvoker;
import se.krka.kahlua.integration.expose.MultiLuaJavaInvoker;
import se.krka.kahlua.vm.JavaFunction;
import se.krka.kahlua.vm.KahluaTable;
import se.krka.kahlua.vm.KahluaTableIterator;
import se.krka.kahlua.vm.LuaClosure;
import zombie.Lua.LuaManager;
import zombie.ZomboidFileSystem;
import zombie.core.random.RandStandard;

/** Measurement probe (tools/gen_lua_expose.py): what the ORIGINAL LuaManager.init leaves in the Lua environment,
 * standalone (no world, no UI). Printed in iteration order:
 *   G <key> <value>                 every global
 *   T <path> <key> <value>          the entries of every table reachable from the globals (each table once)
 *   C <class> <super> <ifaces,...>  every class in __classmetatables, then its method table:
 *   M <class> <name> <value>
 * Values: numbers as raw bits, strings escaped, booleans, `table <path>`, `jfunction <class>` (an exposed method
 * `invoker <class.method>`, overloads `multi <n>`), `class <name>`, Java objects by class (enum constants with their
 * name). Args: <scratch cache dir>; run from the game root. */
public class LuaExposeProbe {
    static final StringBuilder sb = new StringBuilder();
    static final IdentityHashMap<Object, String> seen = new IdentityHashMap<>();

    static String esc(String s) {
        StringBuilder b = new StringBuilder("\"");
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c >= 0x20 && c < 0x7f && c != '"' && c != '\\') b.append(c);
            else b.append(String.format("\\u%04x", (int) c));
        }
        return b.append('"').toString();
    }

    static String value(Object o) {
        if (o == null) return "nil";
        if (o instanceof Boolean b) return b ? "true" : "false";
        if (o instanceof Double d) return "d" + Long.toHexString(Double.doubleToRawLongBits(d));
        if (o instanceof String s) return "s" + esc(s);
        if (o instanceof LuaClosure c) return "closure " + c.prototype.filename;
        if (o instanceof MultiLuaJavaInvoker m) return "multi " + m.getInvokers().size();
        if (o instanceof LuaJavaInvoker i) return "invoker " + clazz(i).getName() + "." + i;
        if (o instanceof JavaFunction) return "jfunction " + o.getClass().getName();
        if (o instanceof KahluaTable) return "table";
        if (o instanceof Class<?> c) return "class " + c.getName();
        if (o instanceof Enum<?> e) return "enum " + e.getDeclaringClass().getName() + " " + e.name() + " " + e.ordinal();
        return "object " + o.getClass().getName();
    }

    static Class<?> clazz(LuaJavaInvoker i) {
        try {
            java.lang.reflect.Field f = LuaJavaInvoker.class.getDeclaredField("clazz");
            f.setAccessible(true);
            return (Class<?>) f.get(i);
        } catch (ReflectiveOperationException e) {
            throw new RuntimeException(e);
        }
    }

    static void walk(String path, KahluaTable t) {
        List<Object[]> sub = new ArrayList<>();
        KahluaTableIterator it = t.iterator();
        while (it.advance()) {
            Object k = it.getKey(), v = it.getValue();
            String ks = k instanceof String s ? esc(s) : value(k);
            if (v instanceof KahluaTable vt) {
                String before = seen.get(vt);
                if (before != null) {
                    sb.append("T ").append(path).append(' ').append(ks).append(" ref ").append(before).append('\n');
                    continue;
                }
                String p = path + "." + (k instanceof String s ? s : ks);
                seen.put(vt, p);
                sb.append("T ").append(path).append(' ').append(ks).append(" table ").append(p).append('\n');
                sub.add(new Object[]{p, vt});
            } else {
                sb.append("T ").append(path).append(' ').append(ks).append(' ').append(value(v)).append('\n');
            }
        }
        for (Object[] s : sub) walk((String) s[0], (KahluaTable) s[1]);
    }

    public static void main(String[] a) throws Exception {
        PrintStream out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, "UTF-8");
        System.setOut(System.err);
        ZomboidFileSystem.instance.setCacheDir(a[0]);
        ZomboidFileSystem.instance.init();
        RandStandard.INSTANCE.init();
        LuaManager.init();
        KahluaTable metatables = (KahluaTable) LuaManager.env.rawget("__classmetatables");
        seen.put(LuaManager.env, "_G");
        seen.put(metatables, "__classmetatables");
        walk("_G", LuaManager.env);
        KahluaTableIterator it = metatables.iterator();
        while (it.advance()) {
            if (!(it.getKey() instanceof Class<?> c) || !(it.getValue() instanceof KahluaTable mt)) {
                sb.append("X ").append(value(it.getKey())).append(' ').append(value(it.getValue())).append('\n');
                continue;
            }
            StringBuilder ifs = new StringBuilder();
            for (Class<?> i : c.getInterfaces()) ifs.append(ifs.length() == 0 ? "" : ",").append(i.getName());
            sb.append("C ").append(c.getName()).append(' ')
                .append(c.getSuperclass() == null ? "-" : c.getSuperclass().getName()).append(' ')
                .append(ifs.length() == 0 ? "-" : ifs).append(' ')
                .append(c.isEnum() ? "enum" : c.isInterface() ? "interface" : "class").append('\n');
            KahluaTableIterator mi = mt.iterator();
            while (mi.advance()) {
                Object k = mi.getKey(), v = mi.getValue();
                if (k.equals("__index") && v instanceof KahluaTable idx) {
                    KahluaTableIterator ii = idx.iterator();
                    while (ii.advance()) sb.append("M ").append(c.getName()).append(' ').append(ii.getKey()).append(' ').append(value(ii.getValue())).append('\n');
                    KahluaTable imt = idx.getMetatable();
                    sb.append("I ").append(c.getName()).append(" meta ").append(imt == null ? "nil" : "set").append('\n');
                } else {
                    sb.append("K ").append(c.getName()).append(' ').append(k instanceof String s ? esc(s) : value(k)).append(' ').append(value(v)).append('\n');
                }
            }
        }
        out.print(sb);
        out.flush();
        Runtime.getRuntime().halt(0);
    }
}
