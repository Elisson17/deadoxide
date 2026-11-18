import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.Random;
import zombie.iso.CellLoader;
import zombie.iso.IsoCell;
import zombie.iso.IsoChunk;
import zombie.iso.IsoWorld;
import zombie.network.ServerMap;
import zombie.pathfind.PolygonalMap2;

// The ORIGINAL pathfinding (zombie.pathfind) run inside the booted dedicated server over real chunks of Rosewood
// (Muldraugh, KY): the chunks [cx1..cx2] x [cy1..cy2] are loaded with CellLoader.LoadCellBinaryChunk +
// loadInWorldStreamerThread, as the server's chunk loader does, and served by ServerMap (GameServer.server stays true).
// Scenario "lcc:<cx1>:<cy1>:<cx2>:<cy2>:<n>:<seed>": n random segments of the region, each answered by
// PolygonalMap2.lineClearCollide (true = blocked) and PolygonalMap2.canStandAt for both ends, with the flags 0..3.
public class PathProbe {
    static Object g(Object o, String n) throws Exception {
        Class<?> c = o instanceof Class ? (Class<?>) o : o.getClass();
        while (c != null) {
            try {
                Field f = c.getDeclaredField(n);
                f.setAccessible(true);
                return f.get(o instanceof Class ? null : o);
            } catch (NoSuchFieldException e) {
                c = c.getSuperclass();
            }
        }
        throw new NoSuchFieldException(n);
    }

    static ArrayList<IsoChunk> loaded = new ArrayList<>();
    static java.util.HashSet<Long> have = new java.util.HashSet<>();
    static java.util.HashMap<Long, ServerMap.ServerCell> cells = new java.util.HashMap<>();

    // CellLoader.LoadCellBinaryChunk + loadInWorldStreamerThread for every chunk of the rectangle not loaded yet, put into
    // the server's cells (8 x 8 chunks per ServerCell), as ActionContextProbe.loadChunks does for four chunks.
    static void loadChunks(IsoCell cell, int wx1, int wy1, int wx2, int wy2) throws Exception {
        Method gp = IsoChunk.class.getDeclaredMethod("getFromPool");
        gp.setAccessible(true);
        Field rf = zombie.core.random.RandAbstract.class.getDeclaredField("rand");
        rf.setAccessible(true);
        ServerMap sm = ServerMap.instance;
        ServerMap.ServerCell[] cellMap = (ServerMap.ServerCell[]) g(sm, "cellMap");
        int width = (Integer) g(sm, "width");
        for (int wx = wx1; wx <= wx2; wx++) {
            for (int wy = wy1; wy <= wy2; wy++) {
                long ck = ((long) wx << 32) | (wy & 0xffffffffL);
                if (!have.add(ck)) continue;
                int cx = Math.floorDiv(wx, 8), cy = Math.floorDiv(wy, 8);
                long key = ((long) cx << 32) | cy;
                ServerMap.ServerCell sc = cells.get(key);
                if (sc == null) {
                    sc = new ServerMap.ServerCell();
                    sc.wx = cx;
                    sc.wy = cy;
                    cellMap[(cy - sm.getMinY()) * width + (cx - sm.getMinX())] = sc;
                    sm.loadedCells.add(sc);
                    sc.isLoaded = true;
                    cells.put(key, sc);
                }
                // the draws of the loader (random door and window states) from a fixed seed, as ChunkObjectsProbe does
                rf.set(zombie.core.random.RandStandard.INSTANCE, new java.util.Random(12345L));
                IsoChunk ch = (IsoChunk) gp.invoke(null);
                ch.wx = wx;
                ch.wy = wy;
                CellLoader.LoadCellBinaryChunk(cell, wx, wy, ch);
                ch.loadInWorldStreamerThread();
                sc.chunks[wx - cx * 8][wy - cy * 8] = ch;
                ch.loaded = true;
                loaded.add(ch);
            }
        }
        // the game's neighbour links (nav) of every loaded square: chunk interiors from RecalcAllWithNeighbours, chunk borders
        // from IsoChunk.doLoadGridsquare; both end as a lookup in the loaded grid
        for (IsoChunk ch : loaded) {
            for (int z = ch.getMinLevel(); z <= ch.getMaxLevel(); z++) {
                for (int i = 0; i < 64; i++) {
                    zombie.iso.IsoGridSquare sq = ch.getGridSquare(i % 8, i / 8, z);
                    if (sq != null) sq.doGridNav(zombie.iso.IsoGridSquare.cellGetSquare);
                }
            }
        }
    }

    static String f(float v) {
        return Float.toString(v);
    }

    static void lcc(PrintStream out, String spec, IsoCell cell) throws Exception {
        String[] p = spec.split(":");
        int cx1 = Integer.parseInt(p[1]), cy1 = Integer.parseInt(p[2]), cx2 = Integer.parseInt(p[3]), cy2 = Integer.parseInt(p[4]);
        int n = Integer.parseInt(p[5]);
        long seed = Long.parseLong(p[6]);
        loadChunks(cell, cx1 - 1, cy1 - 1, cx2 + 1, cy2 + 1);
        out.println("SCENARIO " + spec);
        Random r = new Random(seed);
        int minX = cx1 * 8, minY = cy1 * 8, w = (cx2 - cx1 + 1) * 8, h = (cy2 - cy1 + 1) * 8;
        PolygonalMap2 map = PolygonalMap2.instance;
        for (int i = 0; i < n; i++) {
            int z = r.nextInt(4) == 0 ? 1 + r.nextInt(2) : 0;
            int flags = r.nextInt(4);
            float fx = minX + r.nextFloat() * w, fy = minY + r.nextFloat() * h;
            float tx, ty;
            int kind = r.nextInt(10);
            if (kind < 4) {
                tx = fx + (r.nextFloat() - 0.5f) * 5.0f;
                ty = fy + (r.nextFloat() - 0.5f) * 5.0f;
            } else if (kind < 8) {
                tx = fx + (r.nextFloat() - 0.5f) * 16.0f;
                ty = fy + (r.nextFloat() - 0.5f) * 16.0f;
            } else {
                tx = minX + r.nextFloat() * w;
                ty = minY + r.nextFloat() * h;
            }
            if (r.nextInt(3) == 0) {
                fx = (float) Math.floor(fx) + 0.5f;
                fy = (float) Math.floor(fy) + 0.5f;
            }
            if (r.nextInt(3) == 0) {
                tx = (float) Math.floor(tx) + 0.5f;
                ty = (float) Math.floor(ty) + 0.5f;
            }
            boolean blocked = map.lineClearCollide(fx, fy, tx, ty, z, null, flags);
            boolean s1 = map.canStandAt(fx, fy, z, (zombie.vehicles.BaseVehicle) null, flags);
            boolean s2 = map.canStandAt(tx, ty, z, (zombie.vehicles.BaseVehicle) null, flags);
            out.println("L " + i + " z=" + z + " f=" + flags + " " + f(fx) + "," + f(fy) + " " + f(tx) + "," + f(ty)
                + " blocked=" + (blocked ? 1 : 0) + " stand=" + (s1 ? 1 : 0) + (s2 ? 1 : 0));
        }
        out.println("END " + spec);
    }

    public static void main(String[] a) throws Exception {
        java.util.TimeZone.setDefault(java.util.TimeZone.getTimeZone("UTC"));
        ServerHarness.run(a[0], out -> {
            // the world's meta grid is swapped for one built by Create() alone (no basements), as ChunkObjectsProbe does
            zombie.iso.IsoMetaGrid mgrid = new zombie.iso.IsoMetaGrid();
            mgrid.Create();
            Field mg = IsoWorld.class.getDeclaredField("metaGrid");
            mg.setAccessible(true);
            mg.set(IsoWorld.instance, mgrid);
            IsoCell cell = IsoWorld.instance.currentCell;
            for (int i = 1; i < a.length; i++) {
                if (a[i].startsWith("lcc:")) lcc(out, a[i], cell);
                else throw new IllegalArgumentException("unknown scenario " + a[i]);
            }
        });
    }
}
