import java.io.ByteArrayOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.util.Arrays;
import java.util.TreeMap;
import java.util.Map;
import zombie.GameWindow;
import zombie.ZomboidFileSystem;
import zombie.iso.IsoWorld;
import zombie.network.GameServer;

// Inside the booted original (ServerHarness): the files that say which world version and which map a save was made
// with, map_t.bin's "GMTM" header and map_ver.bin (an int world version, then GameWindow.WriteString(Core.gameMap)).
// For the save's real files and the hand-made ones the probe puts into the current save: IsoWorld.readWorldVersion(),
// once on the server branch (map_t.bin; the booted original is a server) and once on the other branch (map_ver.bin;
// GameServer.server is switched off around that call only), and what IsoWorld.init does with map_ver.bin on a
// client: readInt, then the original GameWindow.ReadString(DataInputStream) (the three lines of init are repeated here,
// init itself cannot run). The hand-made pairs are written to tests/mapver_crafted/<label>.map_t and .map_ver (a
// missing file means no such file in the save). Args: <scratch dir> <save dir> <crafted dir>
public class MapVerProbe {
    static String esc(String s) {
        StringBuilder sb = new StringBuilder("\"");
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c >= 0x20 && c < 0x7f && c != '"' && c != '\\') sb.append(c);
            else sb.append("\\").append(String.format("u%04x", (int) c));
        }
        return sb.append('"').toString();
    }

    static File cur(String name) { return new File(ZomboidFileSystem.instance.getFileNameInCurrentSave(name)); }

    static void place(String name, byte[] data) throws Exception {
        File f = cur(name);
        f.delete();
        if (data != null) Files.write(f.toPath(), data);
    }

    static void run(PrintStream out, String label, byte[] mapT, byte[] mapVer) throws Exception {
        place("map_t.bin", mapT);
        place("map_ver.bin", mapVer);
        Method m = IsoWorld.class.getDeclaredMethod("readWorldVersion");
        m.setAccessible(true);
        int srv = (Integer) m.invoke(IsoWorld.instance);
        Field sf = GameServer.class.getDeclaredField("server");
        sf.setAccessible(true);
        sf.setBoolean(null, false);
        int cli;
        try {
            cli = (Integer) m.invoke(IsoWorld.instance);
        } finally {
            sf.setBoolean(null, true);
        }
        out.println("MV " + label + " srv=" + srv + " cli=" + cli);
        File inFile = cur("map_ver.bin");
        String r;
        try (FileInputStream fis = new FileInputStream(inFile); DataInputStream input = new DataInputStream(fis)) {
            int worldVersion = input.readInt();
            String mapDir = GameWindow.ReadString(input);
            r = "version=" + worldVersion + " map=" + esc(mapDir);
        } catch (java.io.FileNotFoundException e) {
            r = "missing";
        } catch (Throwable e) {
            r = "exception " + e.getClass().getName();
        }
        out.println("MD " + label + " " + r);
    }

    static byte[] ws(int version, String s, boolean lenOnly) throws Exception {
        ByteArrayOutputStream bo = new ByteArrayOutputStream();
        DataOutputStream o = new DataOutputStream(bo);
        o.writeInt(version);
        GameWindow.WriteString(o, s);
        return bo.toByteArray();
    }

    static byte[] gmtm(int version) throws Exception {
        ByteArrayOutputStream bo = new ByteArrayOutputStream();
        DataOutputStream o = new DataOutputStream(bo);
        o.write(new byte[] {71, 77, 84, 77});
        o.writeInt(version);
        o.writeInt(7);
        return bo.toByteArray();
    }

    static byte[] ints(int... v) throws Exception {
        ByteArrayOutputStream bo = new ByteArrayOutputStream();
        DataOutputStream o = new DataOutputStream(bo);
        for (int x : v) o.writeInt(x);
        return bo.toByteArray();
    }

    public static void main(String[] a) throws Exception {
        File save = new File(a[1]);
        File crafted = new File(a[2]);
        ServerHarness.run(a[0], out -> {
            crafted.mkdirs();
            for (File f : crafted.listFiles()) f.delete();
            Map<String, byte[][]> sc = new TreeMap<>();
            StringBuilder big = new StringBuilder();
            for (int i = 0; i < 65536; i++) big.append((char) ('a' + i % 26));
            sc.put("plain", new byte[][] {gmtm(249), ws(249, "Muldraugh, KY", false)});
            sc.put("list", new byte[][] {gmtm(100), ws(248, "Brandenburg, KY;Echo Creek, KY;Muldraugh, KY", false)});
            sc.put("emptystr", new byte[][] {gmtm(0), ws(1, "", false)});
            sc.put("nullstr", new byte[][] {gmtm(-5), ws(-1, null, false)});
            sc.put("unicode", new byte[][] {gmtm(2147483647), ws(-2147483648, "Café € 😀 \u0000 x", false)});
            sc.put("max65536", new byte[][] {null, ws(5, big.toString(), false)});
            sc.put("over65536", new byte[][] {null, ints(5, 65537)});
            sc.put("negative_len", new byte[][] {null, ints(5, -1)});
            sc.put("truncated_str", new byte[][] {null, cat(ints(5, 10), new byte[] {0, 65, 0, 66, 0})});
            sc.put("truncated_len", new byte[][] {null, new byte[] {0, 0, 0, 5, 0, 0}});
            sc.put("only_version", new byte[][] {null, ints(249)});
            sc.put("short_version", new byte[][] {null, new byte[] {0, 0, 1}});
            sc.put("empty_files", new byte[][] {new byte[0], new byte[0]});
            sc.put("gmtm_bad_magic", new byte[][] {new byte[] {71, 77, 84, 78, 0, 0, 0, 9}, ints(9)});
            sc.put("gmtm_short_magic", new byte[][] {new byte[] {71, 77, 84}, null});
            sc.put("gmtm_no_version", new byte[][] {new byte[] {71, 77, 84, 77}, null});
            sc.put("gmtm_short_version", new byte[][] {new byte[] {71, 77, 84, 77, 0, 0, 1}, null});
            sc.put("trailing", new byte[][] {cat(gmtm(33), new byte[] {1, 2, 3}), cat(ws(34, "AB", false), new byte[] {9, 9, 9})});
            for (Map.Entry<String, byte[][]> e : sc.entrySet()) {
                if (e.getValue()[0] != null) Files.write(new File(crafted, e.getKey() + ".map_t").toPath(), e.getValue()[0]);
                if (e.getValue()[1] != null) Files.write(new File(crafted, e.getKey() + ".map_ver").toPath(), e.getValue()[1]);
            }
            File t = new File(save, "map_t.bin"), v = new File(save, "map_ver.bin");
            run(out, "none", null, null);
            run(out, "real", t.exists() ? Files.readAllBytes(t.toPath()) : null, v.exists() ? Files.readAllBytes(v.toPath()) : null);
            for (Map.Entry<String, byte[][]> e : sc.entrySet()) run(out, e.getKey(), e.getValue()[0], e.getValue()[1]);
        });
    }

    static byte[] cat(byte[] x, byte[] y) {
        byte[] r = Arrays.copyOf(x, x.length + y.length);
        System.arraycopy(y, 0, r, x.length, y.length);
        return r;
    }
}
