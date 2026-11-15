import java.io.BufferedReader;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.FileDescriptor;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.io.Reader;
import java.io.StringReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.stream.Stream;
import org.luaj.kahluafork.compiler.FuncState;
import org.luaj.kahluafork.compiler.LexState;
import se.krka.kahlua.vm.Prototype;

// The ORIGINAL Kahlua compiler (org.luaj.kahluafork.compiler.LexState.compile, as LuaCompiler.loadis calls it) over
// every media/lua/**/*.lua of the game, in path order, read as LuaManager.RunLuaInternal reads them (a UTF-8
// InputStreamReader; FuncState.currentFile = the file name, currentfullFile = the path; chunk name = the file name);
// then over the cases of tests/lua_compile_cases.txt (from a StringReader, chunk name "case<N>"). Prints every
// prototype: name, file statics, params, vararg, stack, upvalues, code, lines, constants (doubles as raw bits), local
// names, nested prototypes; or the exception. Core.debug stays false. No server needed.
// Args: <cases file>; run from the game root (oracle.sh does).
public class LuaCompileProbe {
    static PrintStream out;

    static void esc(StringBuilder sb, String s) {
        if (s == null) {
            sb.append("null");
            return;
        }
        sb.append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c >= 0x20 && c < 0x7f && c != '"' && c != '\\') sb.append(c);
            else sb.append(String.format("\\u%04x", (int) c));
        }
        sb.append('"');
    }

    static void proto(Prototype p, int depth) {
        StringBuilder sb = new StringBuilder("P ").append(depth).append(' ');
        esc(sb, p.name);
        sb.append(' ');
        esc(sb, p.file);
        sb.append(' ');
        esc(sb, p.filename);
        sb.append(' ').append(p.numParams).append(' ').append(p.isVararg ? 1 : 0).append(' ').append(p.maxStacksize).append(' ')
            .append(p.numUpvalues).append(' ').append(p.code.length).append(' ').append(p.constants.length).append(' ')
            .append(p.prototypes.length).append(' ').append(p.locvars.length);
        out.println(sb);
        sb = new StringBuilder("C");
        for (int c : p.code) sb.append(' ').append(c);
        out.println(sb);
        sb = new StringBuilder("L");
        for (int l : p.lines) sb.append(' ').append(l);
        out.println(sb);
        sb = new StringBuilder("K");
        for (Object o : p.constants) {
            sb.append(' ');
            if (o == null) sb.append("nil");
            else if (o instanceof Boolean b) sb.append(b ? "T" : "F");
            else if (o instanceof Double d) sb.append('d').append(Long.toHexString(Double.doubleToRawLongBits(d)));
            else if (o instanceof String s) {
                sb.append('s');
                esc(sb, s);
            } else sb.append('?').append(o.getClass().getName());
        }
        out.println(sb);
        sb = new StringBuilder("V");
        for (String v : p.locvars) {
            sb.append(' ');
            esc(sb, v);
        }
        out.println(sb);
        for (Prototype c : p.prototypes) proto(c, depth + 1);
    }

    static void compile(Reader r, String name) {
        try {
            proto(LexState.compile(r.read(), r, name, null), 0);
        } catch (Throwable e) {
            StringBuilder sb = new StringBuilder("ERR ");
            esc(sb, e.getClass().getName() + ": " + e.getMessage());
            out.println(sb);
        }
    }

    // The generated cases: "--@@gen <kind> <n>" stands for a source built the same way by the port.
    static String gen(String kind, int n) {
        StringBuilder sb = new StringBuilder();
        switch (kind) {
            case "bigtable" -> {
                sb.append("local t = {");
                for (int i = 0; i < n; i++) sb.append(i % 7).append(',');
                sb.append("}\n");
            }
            case "locals" -> {
                for (int i = 0; i < n; i++) sb.append("local a").append(i).append(" = ").append(i).append('\n');
            }
            case "nest" -> {
                sb.append("x = ");
                for (int i = 0; i < n; i++) sb.append('(');
                sb.append('1');
                for (int i = 0; i < n; i++) sb.append(')');
                sb.append('\n');
            }
            case "upvals" -> {
                sb.append("local function f()\n");
                for (int i = 0; i < n; i++) sb.append("local u").append(i).append(" = ").append(i).append('\n');
                sb.append("return function() return ");
                for (int i = 0; i < n; i++) sb.append(i == 0 ? "" : " + ").append('u').append(i);
                sb.append(" end\nend\n");
            }
            case "jump" -> {
                sb.append("while x do\n");
                for (int i = 0; i < n; i++) sb.append("y = y + ").append(i).append('\n');
                sb.append("end\n");
            }
            default -> sb.append("error('unknown generator')\n");
        }
        return sb.toString();
    }

    public static void main(String[] a) throws Exception {
        out = new PrintStream(new FileOutputStream(FileDescriptor.out), false, StandardCharsets.UTF_8);
        Path root = Paths.get("").toAbsolutePath();
        Path base = root.resolve("media/lua");
        List<String> files = new ArrayList<>();
        try (Stream<Path> s = Files.walk(base)) {
            s.filter(p -> Files.isRegularFile(p) && p.getFileName().toString().endsWith(".lua"))
                .forEach(p -> files.add("media/lua/" + base.relativize(p).toString().replace('\\', '/')));
        }
        Collections.sort(files);
        for (String rel : files) {
            out.println("F " + rel);
            String name = rel.substring(rel.lastIndexOf('/') + 1);
            FuncState.currentFile = name;
            FuncState.currentfullFile = rel;
            try (BufferedReader r = new BufferedReader(new InputStreamReader(new FileInputStream(root.resolve(rel).toFile()), "UTF-8"))) {
                compile(r, name);
            }
        }
        String all = Files.readString(Paths.get(a[0]), StandardCharsets.UTF_8);
        String[] cases = all.split("\n--@@case\n", -1);
        FuncState.currentFile = "cases";
        FuncState.currentfullFile = "tests/lua_compile_cases.txt";
        for (int i = 1; i < cases.length; i++) {
            String src = cases[i];
            if (src.startsWith("--@@gen ")) {
                String[] g = src.substring(8).trim().split(" ");
                src = gen(g[0], Integer.parseInt(g[1]));
            }
            out.println("F case" + i);
            compile(new StringReader(src), "case" + i);
        }
        out.flush();
    }
}
