import java.io.File;
import java.io.PrintStream;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Queue;
import java.util.Random;
import zombie.iso.areas.isoregion.IsoRegionWorker;
import zombie.iso.areas.isoregion.IsoRegions;
import zombie.iso.areas.isoregion.data.DataChunk;
import zombie.iso.areas.isoregion.data.DataRoot;
import zombie.iso.areas.isoregion.jobs.JobChunkUpdate;
import zombie.iso.areas.isoregion.jobs.RegionJobManager;

// Inside the booted original (ServerHarness): the saved part of IsoRegions. A scenario is a folder of files (the player's
// isoregiondata, files the ORIGINAL writer makes, or hand-made ones); the probe points IsoRegions at a scratch copy and
// runs the original IsoRegionWorker.loadSaveMap() on a new worker (no thread: the worker is only constructed), then does
// what the worker thread does with the queued jobs, JobChunkUpdate.readChunksPacket(rootBuffer, ...), and prints
// every chunk of the resulting DataRoot: its level count, flags and active levels, and what DataRoot.getSquareFlags says
// for every square of its levels. The region data derived from the flags (recalculation) is not printed.
// Each scenario starts with the worker's static 2076 byte file buffer and the job pool emptied, so a short block reads
// zeros and not what an earlier load left.
//   * "real": the save's isoregiondata folder
//   * writer scenarios: DataRoot + DataChunk + JobChunkUpdate.addChunkFromDataChunk/saveChunksToDisk write the files
//   * hand-made scenarios: single files, so that the (unspecified) directory order cannot matter
// The scenarios other than "real" are committed under tests/isoregion_crafted/<scenario>/ and read by the Rust side.
// Args: <scratch dir> <save dir> <crafted dir>
public class IsoRegionProbe {
    static Object g(Object o, String name) throws Exception {
        Field f = o.getClass().getDeclaredField(name);
        f.setAccessible(true);
        return f.get(o);
    }

    static void sset(Class<?> c, String name, Object v) throws Exception {
        Field f = c.getDeclaredField(name);
        f.setAccessible(true);
        f.set(null, v);
    }

    static Object sget(Class<?> c, String name) throws Exception {
        Field f = c.getDeclaredField(name);
        f.setAccessible(true);
        return f.get(null);
    }

    static void delete(File f) {
        File[] l = f.listFiles();
        if (l != null) for (File c : l) delete(c);
        f.delete();
    }

    static byte[] bytes(int... v) {
        byte[] b = new byte[v.length];
        for (int i = 0; i < v.length; i++) b[i] = (byte) v[i];
        return b;
    }
    static byte[] cat(byte[]... parts) {
        int n = 0;
        for (byte[] p : parts) n += p.length;
        byte[] r = new byte[n];
        int o = 0;
        for (byte[] p : parts) { System.arraycopy(p, 0, r, o, p.length); o += p.length; }
        return r;
    }
    static byte[] i32(int v) { return bytes(v >>> 24, v >>> 16, v >>> 8, v); }

    // one chunk file as the original writes it: packlen, world version, x, y, then the chunk block
    static byte[] chunkFile(int ver, int x, int y, int length, int hz, int maxBytes, byte[] flags) {
        byte[] block = cat(i32(length), i32(hz), i32(maxBytes), flags);
        return cat(i32(16 + block.length - 4 + 4), i32(ver), i32(x), i32(y), block);
    }

    static byte[] pattern(int n, int seed) {
        byte[] b = new byte[n];
        for (int i = 0; i < n; i++) b[i] = (byte) ((i * 7 + seed * 13) ^ (i >> 3));
        return b;
    }

    static String hex(int v) { return Integer.toHexString(v); }

    static void run(PrintStream out, String label, File srcDir, File work) throws Exception {
        delete(work);
        work.mkdirs();
        if (srcDir != null) for (File f : srcDir.listFiles()) Files.copy(f.toPath(), new File(work, f.getName()).toPath());
        sset(IsoRegions.class, "cacheDir", work.getAbsolutePath() + File.separator);
        sset(IsoRegions.class, "cacheDirFile", work);
        sset(IsoRegions.class, "headDataFile", new File(work, "RegionHeader.bin"));
        ((Map<?, ?>) sget(IsoRegions.class, "chunkFileNames")).clear();
        Arrays.fill(((java.nio.ByteBuffer) sget(IsoRegionWorker.class, "byteBuffer")).array(), (byte) 0);
        ((Queue<?>) sget(RegionJobManager.class, "poolChunkUpdate")).clear();
        Constructor<IsoRegionWorker> ctor = IsoRegionWorker.class.getDeclaredConstructor();
        ctor.setAccessible(true);
        IsoRegionWorker w = ctor.newInstance();
        Method load = IsoRegionWorker.class.getDeclaredMethod("loadSaveMap");
        load.setAccessible(true);
        load.invoke(w);
        DataRoot root = (DataRoot) g(w, "rootBuffer");
        List<Integer> thread = (List<Integer>) g(w, "threadDiscoveredChunks");
        for (Object job : new ArrayList<>((Queue<?>) g(w, "jobQueue"))) {
            if (job instanceof JobChunkUpdate j) j.readChunksPacket(root, thread);
        }
        List<Integer> disc = new ArrayList<>((List<Integer>) g(w, "discoveredChunks"));
        Collections.sort(disc);
        List<Integer> th = new ArrayList<>(thread);
        Collections.sort(th);
        List<DataChunk> chunks = new ArrayList<>();
        root.getAllChunks(chunks);
        chunks.sort((a, b) -> a.getChunkX() != b.getChunkX() ? Integer.compare(a.getChunkX(), b.getChunkX()) : Integer.compare(a.getChunkY(), b.getChunkY()));
        out.println("ISO " + label + " chunks=" + chunks.size() + " disc=" + disc + " thread=" + th);
        for (DataChunk c : chunks) {
            byte[] flags = (byte[]) g(c, "squareFlags");
            boolean[] active = (boolean[]) g(c, "activeZLayers");
            StringBuilder act = new StringBuilder();
            for (boolean b : active) act.append(b ? '1' : '0');
            int h = 0;
            for (int z = 0; z < 32; z++) for (int y = 0; y < 8; y++) for (int x = 0; x < 8; x++)
                h = h * 31 + root.getSquareFlags(c.getChunkX() * 8 + x, c.getChunkY() * 8 + y, z);
            out.println("CH " + label + " " + c.getChunkX() + " " + c.getChunkY() + " id=" + hex((Integer) g(c, "hashId")) + " hz=" + g(c, "highestZ")
                + " flags=" + (flags == null ? "null" : flags.length + "," + hex(Arrays.hashCode(flags))) + " active=" + act + " sq=" + hex(h)
                + " found=" + (root.getDataChunk(c.getChunkX(), c.getChunkY()) == c ? 1 : 0));
        }
    }

    // build chunks through DataRoot's public API and write them with the original JobChunkUpdate writer
    static void writer(File dir, int n, int hzMax, long seed, boolean negative) throws Exception {
        delete(dir);
        dir.mkdirs();
        sset(IsoRegions.class, "cacheDir", dir.getAbsolutePath() + File.separator);
        sset(IsoRegions.class, "cacheDirFile", dir);
        ((Map<?, ?>) sget(IsoRegions.class, "chunkFileNames")).clear();
        DataRoot root = new DataRoot();
        Random r = new Random(seed);
        List<Integer> ids = new ArrayList<>();
        for (int i = 0; i < n; i++) {
            int cx = (negative ? -40 : 1000) + r.nextInt(80), cy = (negative ? -40 : 1400) + r.nextInt(80);
            int hz = hzMax < 0 ? r.nextInt(8) : hzMax;
            for (int z = hz; z >= 0; z--) {
                root.select.reset(cx * 8, cy * 8, z, true);
                for (int y = 0; y < 8; y++) for (int x = 0; x < 8; x++) root.updateExistingSquare(cx * 8 + x, cy * 8 + y, z, (byte) r.nextInt(256));
            }
            if (!ids.contains(IsoRegions.hash(cx, cy))) ids.add(IsoRegions.hash(cx, cy));
        }
        List<DataChunk> chunks = new ArrayList<>();
        root.getAllChunks(chunks);
        chunks.sort((a, b) -> a.getChunkX() != b.getChunkX() ? Integer.compare(a.getChunkX(), b.getChunkX()) : Integer.compare(a.getChunkY(), b.getChunkY()));
        JobChunkUpdate j = RegionJobManager.allocChunkUpdate();
        for (DataChunk c : chunks) {
            if (!j.canAddChunk()) { j.saveChunksToDisk(); j = RegionJobManager.allocChunkUpdate(); }
            j.addChunkFromDataChunk(c);
        }
        j.saveChunksToDisk();
        // the header the worker thread writes: version, count, ids
        java.io.DataOutputStream o = new java.io.DataOutputStream(new java.io.FileOutputStream(new File(dir, "RegionHeader.bin")));
        o.writeInt(249);
        o.writeInt(ids.size());
        for (int id : ids) o.writeInt(id);
        o.close();
    }

    static void one(File crafted, String name, byte[]... files) throws Exception {
        File d = new File(crafted, name);
        d.mkdirs();
        int i = 0;
        for (byte[] f : files) {
            Files.write(new File(d, i == 0 ? "datachunk_1_1.bin" : "datachunk_1_" + (i + 1) + ".bin").toPath(), f);
            i++;
        }
    }

    public static void main(String[] a) throws Exception {
        File save = new File(a[1]);
        File crafted = new File(a[2]);
        File work = new File(a[0] + "-work");
        ServerHarness.run(a[0], out -> {
            delete(crafted);
            crafted.mkdirs();
            writer(new File(crafted, "writer"), 12, -1, 1, false);
            writer(new File(crafted, "writerneg"), 8, -1, 2, true);
            writer(new File(crafted, "many"), 40, 31, 3, false);
            // hand-made single files
            byte[] flags64 = pattern(64, 1);
            byte[] good = chunkFile(249, 100, 200, 4 + 4 + 4 + 64, 0, 64, flags64);
            one(crafted, "single", good);
            one(crafted, "version0", chunkFile(0, 100, 200, 76, 0, 64, flags64));
            one(crafted, "empty", new byte[0]);
            one(crafted, "short", bytes(1, 2, 3, 4, 5, 6, 7, 8, 9, 10));
            one(crafted, "short19", cat(i32(19), i32(249), i32(5), i32(5), bytes(0, 0, 0)));
            one(crafted, "lenhuge", chunkFile(249, 100, 200, 3000, 0, 64, flags64));
            one(crafted, "lenneg", chunkFile(249, 100, 200, -4, 0, 64, flags64));
            one(crafted, "lenzero", chunkFile(249, 100, 200, 0, 0, 64, flags64));
            one(crafted, "hz3", chunkFile(249, 7, 8, 12 + 256, 3, 256, pattern(256, 2)));
            one(crafted, "hz31", chunkFile(249, 7, 8, 12 + 2048, 31, 2048, pattern(2048, 3)));
            one(crafted, "hz32", chunkFile(249, 7, 8, 12 + 2048, 32, 2048, pattern(2048, 4)));
            one(crafted, "hz40", chunkFile(249, 7, 8, 12 + 2048, 40, 2048, pattern(2048, 5)));
            one(crafted, "hzneg", chunkFile(249, 7, 8, 12 + 64, -1, 64, flags64));
            one(crafted, "maxbig", chunkFile(249, 7, 8, 12 + 100, 0, 100, pattern(100, 6)));
            one(crafted, "maxsmall", chunkFile(249, 7, 8, 12 + 10, 0, 10, pattern(10, 7)));
            one(crafted, "maxzero", chunkFile(249, 7, 8, 12, 2, 0, new byte[0]));
            one(crafted, "maxneg", chunkFile(249, 7, 8, 12, 2, -5, new byte[0]));
            one(crafted, "bigfile", cat(chunkFile(249, 7, 8, 12 + 64, 0, 64, flags64), pattern(2500, 8)));
            one(crafted, "bigblock", chunkFile(249, 7, 8, 2060, 31, 2048, pattern(2048, 9)));
            one(crafted, "negcoords", chunkFile(249, -3, -33, 12 + 64, 0, 64, flags64));
            one(crafted, "bigcoords", chunkFile(249, 2000000000, -2000000000, 12 + 64, 1, 64, flags64));
            one(crafted, "twoz", chunkFile(249, 7, 8, 12 + 128, 1, 128, pattern(128, 10)));
            // other names are ignored; a header with unknown and with missing chunks; a short header
            File d = new File(crafted, "names");
            d.mkdirs();
            Files.write(new File(d, "datachunk_7_8.bin").toPath(), good);
            Files.write(new File(d, "datachunk_7_9.txt").toPath(), good);
            Files.write(new File(d, "other_7_9.bin").toPath(), good);
            Files.write(new File(d, "datachunk_7_9").toPath(), good);
            d = new File(crafted, "headerextra");
            d.mkdirs();
            Files.write(new File(d, "datachunk_1_1.bin").toPath(), good);
            Files.write(new File(d, "RegionHeader.bin").toPath(), cat(i32(249), i32(3), i32(IsoRegions.hash(100, 200)), i32(5), i32(6)));
            d = new File(crafted, "headeronly");
            d.mkdirs();
            Files.write(new File(d, "RegionHeader.bin").toPath(), cat(i32(249), i32(1), i32(IsoRegions.hash(100, 200))));
            d = new File(crafted, "headershort");
            d.mkdirs();
            Files.write(new File(d, "datachunk_1_1.bin").toPath(), good);
            Files.write(new File(d, "RegionHeader.bin").toPath(), cat(i32(249), i32(5), i32(1)));
            d = new File(crafted, "headerempty");
            d.mkdirs();
            Files.write(new File(d, "datachunk_1_1.bin").toPath(), good);
            Files.write(new File(d, "RegionHeader.bin").toPath(), new byte[0]);

            run(out, "none", null, work);
            run(out, "real", new File(save, "isoregiondata"), work);
            File[] scenarios = crafted.listFiles(File::isDirectory);
            Arrays.sort(scenarios);
            for (File s : scenarios) run(out, s.getName(), s, work);
        });
    }
}
