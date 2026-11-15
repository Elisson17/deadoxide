import java.io.PrintStream;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import se.krka.kahlua.vm.JavaFunction;
import se.krka.kahlua.vm.KahluaTable;
import se.krka.kahlua.vm.KahluaTableIterator;
import se.krka.kahlua.vm.KahluaUtil;
import se.krka.kahlua.vm.LuaCallFrame;
import zombie.Lua.LuaManager;
import zombie.ZomboidFileSystem;
import zombie.core.random.RandStandard;

/** Measurement (not an oracle): which Java functions the game's own Lua calls while LuaManager.LoadDirBase loads
 * media/lua/<dirs>. Every JavaFunction in the environment, in exposed class tables and in the class metatables is
 * wrapped with a counter. Prints "CALL <count> <name>" sorted by name, then the files loaded.
 * Args: <scratch cache dir> <dir>... ; run from the game root. */
public class LuaLoadCallsProbe {
    static final Map<String, int[]> counts = new TreeMap<>();

    static final class Counting implements JavaFunction {
        final JavaFunction inner;
        final String name;
        Counting(JavaFunction inner, String name) { this.inner = inner; this.name = name; }
        @Override public int call(LuaCallFrame cf, int n) {
            counts.computeIfAbsent(name, k -> new int[1])[0]++;
            return inner.call(cf, n);
        }
        @Override public String toString() { return inner.toString(); }
    }

    static void wrapTable(KahluaTable t, String prefix, int depth) {
        List<Object[]> todo = new ArrayList<>();
        KahluaTableIterator it = t.iterator();
        while (it.advance()) todo.add(new Object[]{it.getKey(), it.getValue()});
        for (Object[] kv : todo) {
            if (kv[1] instanceof JavaFunction f && !(f instanceof Counting)) {
                t.rawset(kv[0], new Counting(f, prefix + kv[0]));
            } else if (kv[1] instanceof KahluaTable sub && depth > 0 && kv[0] instanceof String s && !s.equals("_G")) {
                wrapTable(sub, prefix + s + ".", depth - 1);
            }
        }
    }

    public static void main(String[] a) throws Exception {
        PrintStream out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, "UTF-8");
        System.setOut(System.err);
        ZomboidFileSystem.instance.setCacheDir(a[0]);
        ZomboidFileSystem.instance.init();
        RandStandard.INSTANCE.init();
        LuaManager.init();
        wrapTable(LuaManager.env, "", 1);
        KahluaTable mts = KahluaUtil.getClassMetatables(LuaManager.platform, LuaManager.env);
        KahluaTableIterator it = mts.iterator();
        while (it.advance()) {
            String cls = it.getKey() instanceof Class<?> c ? c.getSimpleName() : String.valueOf(it.getKey());
            if (it.getValue() instanceof KahluaTable mt) {
                wrapTable(mt, cls + ":", 0);
                if (mt.rawget("__index") instanceof KahluaTable idx) wrapTable(idx, cls + ":", 0);
            }
        }
        for (int i = 1; i < a.length; i++) {
            try {
                LuaManager.LoadDirBase(a[i]);
            } catch (Throwable t) {
                out.println("LOADDIR " + a[i] + " threw " + t);
            }
        }
        for (Map.Entry<String, int[]> e : counts.entrySet()) out.println("CALL " + e.getValue()[0] + " " + e.getKey());
        for (Object f : LuaManager.loaded) out.println("LOADED " + f);
        out.flush();
        Runtime.getRuntime().halt(0);
    }
}
