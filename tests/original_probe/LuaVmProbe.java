import java.io.ByteArrayOutputStream;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import se.krka.kahlua.j2se.J2SEPlatform;
import se.krka.kahlua.luaj.compiler.LuaCompiler;
import se.krka.kahlua.vm.Coroutine;
import se.krka.kahlua.vm.JavaFunction;
import se.krka.kahlua.vm.KahluaTable;
import se.krka.kahlua.vm.KahluaThread;
import se.krka.kahlua.vm.LuaClosure;

// The ORIGINAL Kahlua VM (se.krka.kahlua.vm.KahluaThread over J2SEPlatform.newEnvironment(), the environment
// LuaManager.init starts from, stdlib.lua included) running each case of tests/lua_vm_cases.txt in a fresh
// environment and thread: the case is compiled with LuaCompiler.loadstring (chunk name "case<N>"), called with
// KahluaThread.pcall, and the probe prints what `print` wrote, then every result of the pcall (numbers as raw
// bits). Game logging goes to a null stream. No server needed.
// Args: <cases file>; run from the game root (stdlib.lua is read from the working directory).
public class LuaVmProbe {
    static PrintStream out;

    static void esc(StringBuilder sb, String s) {
        sb.append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c >= 0x20 && c < 0x7f && c != '"' && c != '\\') sb.append(c);
            else sb.append(String.format("\\u%04x", (int) c));
        }
        sb.append('"');
    }

    static String render(Object o) {
        if (o == null) return "nil";
        if (o instanceof Boolean b) return b ? "true" : "false";
        if (o instanceof Double d) return "d" + Long.toHexString(Double.doubleToRawLongBits(d));
        if (o instanceof String s) {
            StringBuilder sb = new StringBuilder("s");
            esc(sb, s);
            return sb.toString();
        }
        if (o instanceof KahluaTable) return "table";
        if (o instanceof LuaClosure) return "closure";
        if (o instanceof JavaFunction) return "jfunction";
        if (o instanceof Coroutine) return "coroutine";
        if (o instanceof Throwable t) return "exc:" + t.getClass().getName();
        return "userdata:" + o.getClass().getName();
    }

    public static void main(String[] a) throws Exception {
        out = new PrintStream(new FileOutputStream(FileDescriptor.out), false, StandardCharsets.UTF_8);
        PrintStream nul = new PrintStream(OutputStream.nullOutputStream());
        System.setOut(nul);
        System.setErr(nul);
        String all = Files.readString(Paths.get(a[0]), StandardCharsets.UTF_8);
        String[] cases = all.split("\n--@@case\n", -1);
        for (int i = 1; i < cases.length; i++) {
            out.println("F case" + i);
            J2SEPlatform platform = new J2SEPlatform();
            KahluaTable env = platform.newEnvironment();
            ByteArrayOutputStream printed = new ByteArrayOutputStream();
            PrintStream ps = new PrintStream(printed, true, StandardCharsets.UTF_8);
            KahluaThread thread = new KahluaThread(ps, platform, env);
            thread.debugOwnerThread = Thread.currentThread();
            LuaClosure c;
            try {
                c = LuaCompiler.loadstring(cases[i], "case" + i, env);
            } catch (Throwable e) {
                StringBuilder sb = new StringBuilder("ERR ");
                esc(sb, e.getClass().getName() + ": " + e.getMessage());
                out.println(sb);
                continue;
            }
            Object[] r;
            try {
                r = thread.pcall(c, new Object[0]);
            } catch (Throwable e) {
                StringBuilder sb = new StringBuilder("THROW ");
                esc(sb, e.getClass().getName() + ": " + e.getMessage());
                out.println(sb);
                continue;
            }
            ps.flush();
            String text = printed.toString(StandardCharsets.UTF_8);
            for (String line : text.split("\n", -1)) {
                if (line.isEmpty()) continue;
                StringBuilder sb = new StringBuilder("O ");
                esc(sb, line);
                out.println(sb);
            }
            StringBuilder sb = new StringBuilder("R ").append(r.length);
            for (Object o : r) sb.append(' ').append(render(o));
            out.println(sb);
        }
        out.flush();
    }
}
