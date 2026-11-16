import java.io.File;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import zombie.ZomboidFileSystem;
import zombie.erosion.ErosionConfig;

// Inside the booted original (ServerHarness): ErosionConfig.readFile on the save's erosion.ini and on the hand-made
// files of tests/erosionini_crafted (comments, lines without or with several "=", unknown keys, numbers the parsers
// refuse, every float and boolean form, line terminators), then ErosionConfig.writeFile of what was read. Per file: the
// result, every field (ints in decimal, floats as raw bits, the boolean) and every line the writer wrote.
// Args: <scratch dir> <save dir> <crafted dir>
public class ErosionIniProbe {
    static String esc(String s) {
        StringBuilder sb = new StringBuilder("\"");
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c >= 0x20 && c < 0x7f && c != '"' && c != '\\') sb.append(c);
            else sb.append("\\").append(String.format("u%04x", (int) c));
        }
        return sb.append('"').toString();
    }

    static void fields(PrintStream out, String label, String section, Object o) throws Exception {
        List<Field> fs = new ArrayList<>(Arrays.asList(o.getClass().getDeclaredFields()));
        fs.sort((a, b) -> a.getName().compareTo(b.getName()));
        for (Field f : fs) {
            f.setAccessible(true);
            Object v = f.get(o);
            String s = v instanceof Float ? String.format("%08x", Float.floatToRawIntBits((Float) v)) : String.valueOf(v);
            out.println("F " + label + " " + section + "." + f.getName() + "=" + s);
        }
    }

    static void one(PrintStream out, String label, File file, int n) throws Exception {
        ErosionConfig cfg = new ErosionConfig();
        boolean ret = cfg.readFile(file.getAbsolutePath());
        out.println("ERO " + label + " ret=" + ret);
        for (String sec : new String[] {"seeds", "time", "debug", "season"}) {
            Field f = ErosionConfig.class.getDeclaredField(sec);
            f.setAccessible(true);
            fields(out, label, sec, f.get(cfg));
        }
        File w = new File(ZomboidFileSystem.instance.getCacheDir() + File.separator + "erosion_probe_" + n + ".ini");
        w.delete();
        cfg.writeFile(w.getAbsolutePath());
        if (!w.exists()) {
            out.println("W " + label + " nofile");
            return;
        }
        String text = new String(Files.readAllBytes(w.toPath()), "UTF-8");
        int ln = 0;
        for (String line : text.split("\n", -1)) out.println("W " + label + " " + (ln++) + " " + esc(line));
    }

    public static void main(String[] a) throws Exception {
        File save = new File(a[1]);
        File crafted = new File(a[2]);
        ServerHarness.run(a[0], out -> {
            int n = 0;
            one(out, "none", new File(a[0] + "-no-such-file.ini"), n++);
            one(out, "real", new File(save, "erosion.ini"), n++);
            File[] fs = crafted.listFiles((d, name) -> name.endsWith(".ini"));
            Arrays.sort(fs);
            for (File f : fs) one(out, f.getName().replace(".ini", ""), f, n++);
        });
    }
}
