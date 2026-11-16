import java.io.File;
import java.io.PrintStream;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.List;
import se.krka.kahlua.vm.JavaFunction;
import se.krka.kahlua.vm.KahluaTable;
import se.krka.kahlua.vm.KahluaTableIterator;
import zombie.Lua.LuaManager;
import zombie.ZomboidFileSystem;
import zombie.globalObjects.GlobalObject;
import zombie.globalObjects.SGlobalObjectSystem;
import zombie.globalObjects.SGlobalObjects;
import zombie.iso.IsoMetaGrid;
import zombie.iso.IsoWorld;

// Inside the booted original (ServerHarness): the global object systems of a save, read by the ORIGINAL
// SGlobalObjectSystem.load() (gos_<name>.bin: "GLOS", world version, mod data table, objects), and then everything
// the original's lookup says about them.
//  1. Every gos_*.bin of the player's save (farming, campfire, feedingTrough, rainbarrel, trap) is copied in as
//     gos_real_<name>.bin and loaded into a new system real_<name>.
//  2. The ORIGINAL writer (SGlobalObjectSystem.save) writes a system "craft" built here (tables of every type the
//     format has, negative and extreme coordinates, squares outside the map, several objects in one chunk, keys the
//     writer filters out); the file goes to <crafted dir>/file_craft.bin and is loaded back into craft2.
//  3. Hand-made files in <crafted dir>: file_* go through load() (bad magic, newer version, empty, version 249 with
//     objects, ...), payload_* (the bytes after the 8 byte header) go through load(ByteBuffer, int) at several world
//     versions with the exception printed, and the crafted file cut at every length goes through load(ByteBuffer, int).
// Both sides list the crafted dir and treat each file by its name, so the Rust side needs no side channel.
// Args: <scratch dir> <save dir> <crafted dir>
public class GosProbe {
    static String dh(Double d) { return Long.toHexString(Double.doubleToRawLongBits(d)); }

    static String lua(Object v) {
        if (v == null) return "null";
        if (v instanceof String x) return "s:" + x;
        if (v instanceof Double x) return "n:" + dh(x);
        if (v instanceof Boolean x) return "b:" + x;
        if (v instanceof KahluaTable t) {
            List<String> l = new ArrayList<>();
            KahluaTableIterator it = t.iterator();
            while (it.advance()) l.add(lua(it.getKey()) + "=" + lua(it.getValue()));
            Collections.sort(l);
            return "{" + String.join(",", l) + "}";
        }
        return "?" + v;
    }

    static KahluaTable table() { return LuaManager.platform.newTable(); }

    static KahluaTable keys(String... names) {
        KahluaTable t = table();
        for (int i = 0; i < names.length; i++) t.rawset((double) (i + 1), names[i]);
        return t;
    }

    static String list(List<Integer> l) {
        if (l.isEmpty()) return "-";
        StringBuilder sb = new StringBuilder();
        for (int i : l) { if (sb.length() > 0) sb.append(','); sb.append(i); }
        return sb.toString();
    }

    static List<Integer> indices(IdentityHashMap<GlobalObject, Integer> index, List<GlobalObject> objects) {
        List<Integer> l = new ArrayList<>();
        for (GlobalObject o : objects) l.add(index.get(o));
        return l;
    }

    static String hash(String s) { return Integer.toHexString(s.hashCode()); }

    // The whole state of a system, then for each object what the lookup finds.
    static void dump(PrintStream out, String label, SGlobalObjectSystem s) {
        IdentityHashMap<GlobalObject, Integer> index = new IdentityHashMap<>();
        for (int i = 0; i < s.getObjectCount(); i++) index.put(s.getObjectByIndex(i), i);
        out.println("SYS " + label + " v=" + s.loadedWorldVersion() + " n=" + s.getObjectCount() + " md=" + lua(s.getModData()));
        for (int i = 0; i < s.getObjectCount(); i++) {
            GlobalObject o = s.getObjectByIndex(i);
            int x = o.getX(), y = o.getY(), z = o.getZ();
            out.println("OBJ " + label + " " + i + " " + x + " " + y + " " + z + " md=" + lua(o.getModData()));
            GlobalObject at = s.getObjectAt(x, y, z);
            GlobalObject above = s.getObjectAt(x, y, z + 1);
            out.println("AT " + label + " " + i + " " + (at == null ? -1 : index.get(at)) + " " + (above == null ? -1 : index.get(above)));
            int wx = x / 8, wy = y / 8;
            out.println("CHK " + label + " " + i + " " + wx + " " + wy + " " + (s.hasObjectsInChunk(wx, wy) ? 1 : 0) + " "
                + list(indices(index, s.getObjectsInChunk(wx, wy))));
            out.println("ADJ " + label + " " + i + " " + list(indices(index, s.getObjectsAdjacentTo(x, y, z))));
        }
    }

    // The short form for the cut files: the same state folded into hashes.
    static void summary(PrintStream out, String label, String err, SGlobalObjectSystem s) {
        StringBuilder objs = new StringBuilder();
        for (int i = 0; i < s.getObjectCount(); i++) {
            GlobalObject o = s.getObjectByIndex(i);
            objs.append(o.getX()).append(',').append(o.getY()).append(',').append(o.getZ()).append(',').append(lua(o.getModData())).append(';');
        }
        out.println("TR " + label + " err=" + err + " v=" + s.loadedWorldVersion() + " n=" + s.getObjectCount()
            + " md=" + hash(lua(s.getModData())) + " objs=" + hash(objs.toString()));
    }

    static void obj(SGlobalObjectSystem s, int x, int y, int z, Object... kv) {
        zombie.globalObjects.GlobalObject o = s.newObject(x, y, z);
        for (int i = 0; i < kv.length; i += 2) o.getModData().rawset(kv[i], kv[i + 1]);
    }

    static SGlobalObjectSystem craft(IsoMetaGrid grid) {
        SGlobalObjectSystem c = SGlobalObjects.newSystem("craft");
        c.setModDataKeys(keys("alpha", "beta", "num", "flag", "uni", "tbl", "empty", "inf"));
        c.setObjectModDataKeys(keys("k1", "k2", "nested"));
        KahluaTable md = c.getModData();
        md.rawset("alpha", "hello");
        md.rawset("beta", 3.25);
        md.rawset("num", 1.0e300);
        md.rawset("skipme", "filtered out by the writer");
        md.rawset("flag", true);
        md.rawset("uni", "héllo 中文 😀 end");
        md.rawset("empty", "");
        KahluaTable tbl = table();
        tbl.rawset(1.0, "a");
        tbl.rawset(2.0, "b");
        tbl.rawset(3.0, 3.0);
        tbl.rawset(-0.0, "negative zero");
        tbl.rawset(0.5, false);
        tbl.rawset("x", true);
        tbl.rawset("", "empty key");
        tbl.rawset(Boolean.TRUE, "boolean key");
        tbl.rawset("nan", Double.NaN);
        tbl.rawset("pinf", Double.POSITIVE_INFINITY);
        tbl.rawset("min", Double.MIN_VALUE);
        tbl.rawset("fn", (JavaFunction) (frame, n) -> 0);
        KahluaTable sub = table();
        sub.rawset("deep", 1.0);
        sub.rawset("deeper", table());
        tbl.rawset("sub", sub);
        md.rawset("tbl", tbl);
        md.rawset("inf", Double.NEGATIVE_INFINITY);

        obj(c, 8303, 11689, 0, "k1", "plant", "k2", 12.0, "nested", sub, "other", "filtered");
        obj(c, 8304, 11689, 0);
        obj(c, 8304, 11689, 1, "k1", true);
        obj(c, 8304, 11690, -1, "k1", "");
        obj(c, 8304, 11690, 0, "other", "only a filtered key");
        for (int x = 8296; x < 8304; x++) obj(c, x, 11688, 0);
        obj(c, 8447, 11689, 0, "k2", -1.5);
        obj(c, 8448, 11689, 0, "k2", 1.5);
        obj(c, 0, 0, 0, "k1", "outside the map");
        obj(c, -5, -7, 2);
        obj(c, Integer.MAX_VALUE, Integer.MAX_VALUE, 127, "k1", "max");
        obj(c, Integer.MIN_VALUE, Integer.MIN_VALUE, -128, "k1", "min");
        obj(c, grid.minX * 256, grid.minY * 256, 0, "k1", "grid first square");
        obj(c, (grid.maxX + 1) * 256 - 1, (grid.maxY + 1) * 256 - 1, 0, "k1", "grid last square");
        obj(c, (grid.maxX + 1) * 256, (grid.maxY + 1) * 256, 0, "k1", "one past the grid");
        obj(c, grid.minX * 256 - 1, grid.minY * 256, 0, "k1", "one before the grid");
        return c;
    }

    static byte[] bytes(int... v) {
        byte[] b = new byte[v.length];
        for (int i = 0; i < v.length; i++) b[i] = (byte) v[i];
        return b;
    }

    static void write(File dir, String name, byte[] data) throws Exception { Files.write(new File(dir, name).toPath(), data); }

    static byte[] cat(byte[]... parts) {
        int n = 0;
        for (byte[] p : parts) n += p.length;
        byte[] r = new byte[n];
        int o = 0;
        for (byte[] p : parts) { System.arraycopy(p, 0, r, o, p.length); o += p.length; }
        return r;
    }

    static byte[] i32(int v) { return bytes(v >>> 24, v >>> 16, v >>> 8, v); }

    static final byte[] MAGIC = bytes('G', 'L', 'O', 'S');

    static void handMade(File dir) throws Exception {
        // load() header checks
        write(dir, "file_badmagic.bin", cat(bytes('G', 'L', 'O', 'X'), i32(249), bytes(0), i32(0)));
        write(dir, "file_newer.bin", cat(MAGIC, i32(250), bytes(0), i32(1), i32(8303), i32(11689), bytes(0, 0)));
        write(dir, "file_empty.bin", new byte[0]);
        write(dir, "file_short.bin", cat(MAGIC, bytes(0, 0)));
        write(dir, "file_nothing.bin", cat(MAGIC, i32(249)));
        write(dir, "file_minimal.bin", cat(MAGIC, i32(249), bytes(0), i32(0)));
        write(dir, "file_old248.bin", cat(MAGIC, i32(248), bytes(0), i32(1), i32(8303), i32(11689), bytes(0, 0)));
        write(dir, "file_negcount.bin", cat(MAGIC, i32(249), bytes(0), i32(-5)));
        write(dir, "file_v1.bin", cat(MAGIC, i32(1), bytes(0), i32(1), i32(8303), i32(11689), bytes(0, 0)));
        // load(ByteBuffer, int): table of every value type in the system's mod data, one object, flag byte 7 (any non-zero)
        byte[] oneKeyStr = cat(i32(1), bytes(0), bytes(0, 1, 'k'), bytes(0), bytes(0, 1, 'v'));
        write(dir, "payload_flag7.bin", cat(bytes(7), oneKeyStr, i32(1), i32(10), i32(20), bytes(5), bytes(9), oneKeyStr));
        // the same square twice: the second newObject throws
        write(dir, "payload_dup.bin", cat(bytes(0), i32(2), i32(8303), i32(11689), bytes(0), bytes(0), i32(8303), i32(11689), bytes(0), bytes(1), oneKeyStr));
        // the same x, y at other levels is fine, and so is the same square in another system (craft2 and the real ones
        // share the lookup)
        write(dir, "payload_levels.bin", cat(bytes(0), i32(3), i32(8303), i32(11689), bytes(0), bytes(0), i32(8303), i32(11689), bytes(1), bytes(0), i32(8303), i32(11689), bytes(0xff), bytes(1), oneKeyStr));
        // invalid table value type 9, and invalid key type 4
        write(dir, "payload_badvalue.bin", cat(bytes(1), i32(2), bytes(0), bytes(0, 1, 'a'), bytes(1), new byte[]{0x3f, (byte) 0xf0, 0, 0, 0, 0, 0, 0},
            bytes(0), bytes(0, 1, 'b'), bytes(9), i32(0)));
        write(dir, "payload_badkey.bin", cat(bytes(1), i32(1), bytes(4), bytes(0), bytes(0, 1, 'v'), i32(0)));
        // a string longer than the rest of the file, a negative string length, a boolean other than 0 and 1
        write(dir, "payload_longstr.bin", cat(bytes(1), i32(1), bytes(0), bytes(0x7f, 0xff, 'a', 'b'), bytes(0), bytes(0, 1, 'v')));
        write(dir, "payload_negstr.bin", cat(bytes(1), i32(1), bytes(0), bytes(0xff, 0xff), bytes(0), bytes(0, 1, 'v'), i32(0)));
        write(dir, "payload_bool2.bin", cat(bytes(1), i32(2), bytes(3), bytes(2), bytes(3), bytes(0), bytes(3), bytes(1), bytes(3), bytes(255), i32(0)));
        // a repeated key keeps the first position and the last value; an invalid UTF-8 string; nested count past the end
        write(dir, "payload_repeat.bin", cat(bytes(1), i32(3),
            bytes(0), bytes(0, 1, 'a'), bytes(0), bytes(0, 1, '1'),
            bytes(0), bytes(0, 1, 'b'), bytes(0), bytes(0, 1, '2'),
            bytes(0), bytes(0, 1, 'a'), bytes(0), bytes(0, 1, '3'), i32(0)));
        write(dir, "payload_badutf8.bin", cat(bytes(1), i32(1), bytes(0), bytes(0, 3, 0xc3, 0x28, 0xff), bytes(0), bytes(0, 2, 0xe4, 0xb8), i32(0)));
        write(dir, "payload_nestedend.bin", cat(bytes(1), i32(1), bytes(0), bytes(0, 1, 't'), bytes(2), i32(5), bytes(0), bytes(0, 1, 'a')));
    }

    static String loadBytes(SGlobalObjectSystem s, byte[] data, int version) {
        try {
            s.load(ByteBuffer.wrap(data), version);
            return "-";
        } catch (Throwable t) {
            return t.toString();
        }
    }

    public static void main(String[] a) throws Exception {
        File save = new File(a[1]);
        File crafted = new File(a[2]);
        ServerHarness.run(a[0], out -> {
            IsoMetaGrid grid = IsoWorld.instance.getMetaGrid();
            out.println("GRID " + grid.minX + " " + grid.minY + " " + grid.getWidth() + " " + grid.getHeight());

            // 1. the real files
            File[] real = save.listFiles((d, n) -> n.startsWith("gos_") && n.endsWith(".bin"));
            Arrays.sort(real);
            for (File f : real) {
                String name = f.getName().substring(4, f.getName().length() - 4);
                Files.copy(f.toPath(), new File(ZomboidFileSystem.instance.getFileNameInCurrentSave("gos_real_" + name + ".bin")).toPath(),
                    StandardCopyOption.REPLACE_EXISTING);
                SGlobalObjectSystem s = SGlobalObjects.newSystem("real_" + name);
                java.util.Arrays.fill(zombie.iso.SliceY.SliceBuffer.array(), (byte) 0);
                s.load();
                dump(out, "real_" + name, s);
            }

            // 2. the original writer
            crafted.mkdirs();
            for (File f : crafted.listFiles()) f.delete();
            SGlobalObjectSystem c = craft(grid);
            c.save();
            Files.copy(new File(ZomboidFileSystem.instance.getFileNameInCurrentSave("gos_craft.bin")).toPath(),
                new File(crafted, "file_craft.bin").toPath());
            handMade(crafted);

            // 3. every file of the crafted dir by its name
            File[] files = crafted.listFiles();
            Arrays.sort(files);
            byte[] craftBytes = null;
            for (File f : files) {
                String n = f.getName();
                byte[] data = Files.readAllBytes(f.toPath());
                String base = n.substring(n.indexOf('_') + 1, n.length() - 4);
                if (n.startsWith("file_")) {
                    if (n.equals("file_craft.bin")) craftBytes = data;
                    Files.copy(f.toPath(), new File(ZomboidFileSystem.instance.getFileNameInCurrentSave("gos_f_" + base + ".bin")).toPath(),
                        StandardCopyOption.REPLACE_EXISTING);
                    SGlobalObjectSystem s = SGlobalObjects.newSystem("f_" + base);
                    java.util.Arrays.fill(zombie.iso.SliceY.SliceBuffer.array(), (byte) 0);
                s.load();
                    dump(out, "f_" + base, s);
                } else if (n.startsWith("payload_")) {
                    for (int v : new int[]{249, 100, 25}) {
                        SGlobalObjectSystem s = SGlobalObjects.newSystem("p_" + base + "_" + v);
                        String err = loadBytes(s, data, v);
                        out.println("PAYLOAD " + base + " " + v + " err=" + err);
                        dump(out, "p_" + base + "_" + v, s);
                    }
                }
            }
            // the crafted file cut at every length (payload = after the 8 byte header)
            byte[] payload = Arrays.copyOfRange(craftBytes, 8, craftBytes.length);
            for (int k = 0; k <= payload.length; k++) {
                SGlobalObjectSystem s = SGlobalObjects.newSystem("t_" + k);
                String err = loadBytes(s, Arrays.copyOf(payload, k), 249);
                summary(out, String.valueOf(k), err, s);
            }
        });
    }
}
