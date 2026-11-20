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
// Scenario "pgraph:<cx1>:<cy1>:<cx2>:<cy2>": the pathfinder's own cache of the chunk rectangle (zombie.pathfind.Square bits,
// cost, sloped surface of every square; the high-level regions, edges and level transitions of every chunk level), JAVA mode
// (PathfindNative.useNativeCode = false), the chunks added with PolygonalMap2.addChunkToWorld.
// Scenario "psearch:<cx1>:<cy1>:<cx2>:<cy2>:<n>:<seed>:<mover>": n seeded path requests answered by the Java
// PolygonalMap2 (findPathHighLevelThenLowLevel, the high-level A* over chunk regions and staircases, the low-level A*
// over squares, cleanPath, smoothPath); mover is zombie, crawler or player. With the extra field ":sq" the ends are walkable
// squares of the cache (start anywhere, target within 24 squares and 2 levels) instead of random points.
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
                if (ignore) ch.ignorePathfind();
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


    // Java mode: PathfindNative.useNativeCode = false, PolygonalMap2 initialised on the meta grid, the chunks of the rectangle
    // (plus a one-chunk ring) loaded with ignorePathfind() and then added with addChunkToWorld, as the game adds them.
    static void javaWorld(IsoCell cell, int cx1, int cy1, int cx2, int cy2) throws Exception {
        zombie.pathfind.nativeCode.PathfindNative.useNativeCode = false;
        if (!pmInit) {
            PolygonalMap2.instance.init(IsoWorld.instance.getMetaGrid());
            pmInit = true;
        }
        int before = loaded.size();
        ignore = true;
        loadChunks(cell, cx1 - 1, cy1 - 1, cx2 + 1, cy2 + 1);
        ignore = false;
        for (int i = before; i < loaded.size(); i++) PolygonalMap2.instance.addChunkToWorld(loaded.get(i));
        waitPm();
    }

    // native mode (PathfindNative.useNativeCode = true, libPZPathFind): the chunks loaded since `from` are added to the native
    // world and its thread is waited for, as PathExplore did (the Java world above only enumerates the walkable squares)
    static void nativeWorld(int from) throws Exception {
        zombie.pathfind.nativeCode.PathfindNative.useNativeCode = true;
        for (int i = from; i < loaded.size(); i++) zombie.pathfind.nativeCode.PathfindNative.instance.addChunkToWorld(loaded.get(i));
        Object th = zombie.pathfind.nativeCode.PathfindNativeThread.instance;
        long t0 = System.currentTimeMillis();
        while (!((java.util.Collection<?>) g(th, "chunkTaskQueue")).isEmpty() && System.currentTimeMillis() - t0 < 120000) Thread.sleep(50);
        Thread.sleep(1800);
    }

    static boolean pmInit = false;
    static boolean ignore = false;

    // wait for the PolyPathThread to drain its queues and finish the rebuild after the last task
    static void waitPm() throws Exception {
        PolygonalMap2 pm = PolygonalMap2.instance;
        long t0 = System.currentTimeMillis();
        while (System.currentTimeMillis() - t0 < 120000) {
            boolean busy = !((java.util.Collection<?>) g(pm, "chunkTaskQueue")).isEmpty()
                || !((java.util.Collection<?>) g(pm, "squareTaskQueue")).isEmpty()
                || !((java.util.Collection<?>) g(pm, "requestTaskQueue")).isEmpty();
            if (!busy) {
                Thread.sleep(400);
                if (!(Boolean) g(pm, "rebuild")) break;
            }
            Thread.sleep(50);
        }
    }

    static Object call(Object o, String n, Object... args) throws Exception {
        for (Method m : (o instanceof Class ? (Class<?>) o : o.getClass()).getDeclaredMethods()) {
            if (m.getName().equals(n) && m.getParameterCount() == args.length) {
                m.setAccessible(true);
                return m.invoke(o instanceof Class ? null : o, args);
            }
        }
        throw new NoSuchMethodException(n);
    }

    static void pgraph(PrintStream out, String spec, IsoCell cell) throws Exception {
        String[] p = spec.split(":");
        int cx1 = Integer.parseInt(p[1]), cy1 = Integer.parseInt(p[2]), cx2 = Integer.parseInt(p[3]), cy2 = Integer.parseInt(p[4]);
        javaWorld(cell, cx1, cy1, cx2, cy2);
        out.println("SCENARIO " + spec);
        PolygonalMap2 pm = PolygonalMap2.instance;
        for (int wy = cy1; wy <= cy2; wy++) {
            for (int wx = cx1; wx <= cx2; wx++) {
                zombie.pathfind.Chunk chunk = pm.getChunkFromChunkPos(wx, wy);
                if (chunk == null) {
                    out.println("C " + wx + "," + wy + " none");
                    continue;
                }
                int minL = chunk.getMinLevel(), maxL = chunk.getMaxLevel();
                out.println("C " + wx + "," + wy + " levels=" + minL + ".." + maxL);
                for (int z = minL; z <= maxL; z++) {
                    zombie.pathfind.ChunkLevel cl = chunk.getLevelData(z);
                    for (int y = 0; y < 8; y++) {
                        for (int x = 0; x < 8; x++) {
                            zombie.pathfind.Square sq = cl.getSquare(wx * 8 + x, wy * 8 + y);
                            if (sq == null) continue;
                            out.println("B " + sq.getX() + "," + sq.getY() + "," + (sq.getZ() - 32) + " bits=" + g(sq, "bits") + " cost=" + g(sq, "cost")
                                + " slope=" + sq.getSlopedSurfaceDirection() + "," + f(sq.getSlopedSurfaceHeightMin()) + "," + f(sq.getSlopedSurfaceHeightMax()));
                        }
                    }
                    zombie.pathfind.highLevel.HLChunkLevel hl = cl.getHighLevelData();
                    call(hl, "initStairsIfNeeded");
                    java.util.List<?> regions = (java.util.List<?>) g(hl, "regionList");
                    for (Object r : regions) {
                        StringBuilder mask = new StringBuilder();
                        for (int y = 0; y < 8; y++)
                            for (int x = 0; x < 8; x++) mask.append(((Boolean) call(r, "containsSquareLocal", x, y)) ? '1' : '0');
                        StringBuilder edges = new StringBuilder();
                        for (String en : new String[] {"edgeN", "edgeS", "edgeW", "edgeE"}) {
                            int[] e = (int[]) g(r, en);
                            for (int v : e) edges.append(v).append(',');
                            edges.append(';');
                        }
                        out.println("R " + wx + "," + wy + "," + (z - 32) + " " + g(r, "minX") + "," + g(r, "minY") + "," + g(r, "maxX") + "," + g(r, "maxY")
                            + " " + mask + " " + edges);
                    }
                    for (Object st : (java.util.List<?>) g(hl, "stairs")) {
                        zombie.pathfind.highLevel.HLStaircase s = (zombie.pathfind.highLevel.HLStaircase) st;
                        out.println("S " + wx + "," + wy + "," + (z - 32) + " " + s.getDir() + " " + s.getBottomFloorX() + "," + s.getBottomFloorY() + "," + (s.getBottomFloorZ() - 32));
                    }
                    for (Object st : (java.util.List<?>) g(hl, "slopedSurfaces")) {
                        zombie.pathfind.highLevel.HLSlopedSurface s = (zombie.pathfind.highLevel.HLSlopedSurface) st;
                        out.println("K " + wx + "," + wy + "," + (z - 32) + " " + s.getDir() + " " + s.getBottomFloorX() + "," + s.getBottomFloorY() + "," + (s.getBottomFloorZ() - 32));
                    }
                }
            }
        }
        out.println("END " + spec);
    }

    static String pathStr(zombie.pathfind.Path path) throws Exception {
        @SuppressWarnings("unchecked")
        ArrayList<zombie.pathfind.PathNode> nodes = (ArrayList<zombie.pathfind.PathNode>) g(path, "nodes");
        StringBuilder sb = new StringBuilder();
        sb.append(nodes.size());
        for (zombie.pathfind.PathNode n : nodes) sb.append(' ').append(f(n.x)).append(',').append(f(n.y)).append(',').append(f(n.z)).append(',').append(g(n, "flags"));
        return sb.toString();
    }

    static void psearch(PrintStream out, String spec, IsoCell cell) throws Exception {
        String[] p = spec.split(":");
        int cx1 = Integer.parseInt(p[1]), cy1 = Integer.parseInt(p[2]), cx2 = Integer.parseInt(p[3]), cy2 = Integer.parseInt(p[4]);
        int n = Integer.parseInt(p[5]);
        long seed = Long.parseLong(p[6]);
        String kind = p[7];
        boolean onSquares = p.length > 8 && p[8].equals("sq");
        boolean nat = p.length > 8 && p[p.length - 1].equals("native");
        int before = loaded.size();
        javaWorld(cell, cx1, cy1, cx2, cy2);
        if (nat) nativeWorld(before);
        ArrayList<int[]> walk = new ArrayList<>();
        if (onSquares) {
            for (int wy = cy1; wy <= cy2; wy++) {
                for (int wx = cx1; wx <= cx2; wx++) {
                    zombie.pathfind.Chunk chunk = PolygonalMap2.instance.getChunkFromChunkPos(wx, wy);
                    if (chunk == null) continue;
                    for (int z = chunk.getMinLevel(); z <= chunk.getMaxLevel(); z++) {
                        zombie.pathfind.ChunkLevel cl = chunk.getLevelData(z);
                        for (int y = 0; y < 8; y++) {
                            for (int x = 0; x < 8; x++) {
                                zombie.pathfind.Square sq = cl.getSquare(wx * 8 + x, wy * 8 + y);
                                if (sq == null) continue;
                                int bits = (Integer) g(sq, "bits");
                                if ((bits & 1) == 0 && (bits & (512 | 504)) != 0) walk.add(new int[] {sq.getX(), sq.getY(), sq.getZ() - 32});
                            }
                        }
                    }
                }
            }
        }
        zombie.ai.astar.Mover mover;
        zombie.iso.IsoMovingObject.setIDCount(0);
        if (kind.equals("player")) {
            mover = new zombie.characters.IsoPlayer(cell, new zombie.characters.SurvivorDesc(), cx1 * 8 + 1, cy1 * 8 + 1, 0);
        } else {
            zombie.characters.IsoZombie z = new zombie.characters.IsoZombie(cell);
            if (kind.equals("crawler")) z.crawling = true;
            mover = z;
        }
        out.println("SCENARIO " + spec);
        Random r = new Random(seed);
        int minX = cx1 * 8, minY = cy1 * 8, w = (cx2 - cx1 + 1) * 8, h = (cy2 - cy1 + 1) * 8;
        PolygonalMap2 pm = PolygonalMap2.instance;
        for (int i = 0; i < n; i++) {
            if (onSquares) {
                int[] a = walk.get(r.nextInt(walk.size()));
                int[] b = a;
                for (int k = 0; k < 30; k++) {
                    int[] c = walk.get(r.nextInt(walk.size()));
                    b = c;
                    if (Math.abs(c[0] - a[0]) <= 24 && Math.abs(c[1] - a[1]) <= 24 && Math.abs(c[2] - a[2]) <= 2) break;
                }
                float sx = a[0] + 0.5f, sy = a[1] + 0.5f;
                if (r.nextInt(4) == 0) {
                    sx = a[0] + r.nextFloat();
                    sy = a[1] + r.nextFloat();
                }
                float tx = b[0] + 0.5f, ty = b[1] + 0.5f;
                if (r.nextInt(4) == 0) {
                    tx = b[0] + r.nextFloat();
                    ty = b[1] + r.nextFloat();
                }
                searchOne(out, pm, mover, kind, i, sx, sy, a[2], tx, ty, b[2], nat);
                continue;
            }
            int sz = r.nextInt(4) == 0 ? 1 + r.nextInt(2) : 0;
            float sx = minX + r.nextFloat() * w, sy = minY + r.nextFloat() * h;
            float tx, ty;
            int tk = r.nextInt(10);
            if (tk < 5) {
                tx = sx + (r.nextFloat() - 0.5f) * 12.0f;
                ty = sy + (r.nextFloat() - 0.5f) * 12.0f;
            } else if (tk < 9) {
                tx = sx + (r.nextFloat() - 0.5f) * 40.0f;
                ty = sy + (r.nextFloat() - 0.5f) * 40.0f;
            } else {
                tx = minX + r.nextFloat() * w;
                ty = minY + r.nextFloat() * h;
            }
            int tz = r.nextInt(5) == 0 ? sz + (r.nextInt(2) == 0 ? 1 : -1) : sz;
            if (tz < 0) tz = 0;
            if (r.nextInt(3) != 0) {
                sx = (float) Math.floor(sx) + 0.5f;
                sy = (float) Math.floor(sy) + 0.5f;
            }
            if (r.nextInt(3) != 0) {
                tx = (float) Math.floor(tx) + 0.5f;
                ty = (float) Math.floor(ty) + 0.5f;
            }
            searchOne(out, pm, mover, kind, i, sx, sy, sz, tx, ty, tz, nat);
        }
        out.println("END " + spec);
    }

    // preq:<cx1>:<cy1>:<cx2>:<cy2>:<kind>[:native]:<sx,sy,sz,tx,ty,tz>/...: the listed requests only
    static void preq(PrintStream out, String spec, IsoCell cell) throws Exception {
        String[] p = spec.split(":");
        int cx1 = Integer.parseInt(p[1]), cy1 = Integer.parseInt(p[2]), cx2 = Integer.parseInt(p[3]), cy2 = Integer.parseInt(p[4]);
        String kind = p[5];
        boolean nat = p.length > 7 && p[6].equals("native");
        String list = p[p.length - 1];
        int before = loaded.size();
        javaWorld(cell, cx1, cy1, cx2, cy2);
        if (nat) nativeWorld(before);
        zombie.ai.astar.Mover mover;
        zombie.iso.IsoMovingObject.setIDCount(0);
        if (kind.equals("player")) {
            mover = new zombie.characters.IsoPlayer(cell, new zombie.characters.SurvivorDesc(), cx1 * 8 + 1, cy1 * 8 + 1, 0);
        } else {
            zombie.characters.IsoZombie z = new zombie.characters.IsoZombie(cell);
            if (kind.equals("crawler")) z.crawling = true;
            mover = z;
        }
        out.println("SCENARIO " + spec);
        int i = 0;
        for (String r : list.split("/")) {
            String[] v = r.split(",");
            searchOne(out, PolygonalMap2.instance, mover, kind, i++, Float.parseFloat(v[0]), Float.parseFloat(v[1]), Integer.parseInt(v[2]),
                Float.parseFloat(v[3]), Float.parseFloat(v[4]), Integer.parseInt(v[5]), nat);
        }
        out.println("END " + spec);
    }

    static void searchOne(PrintStream out, PolygonalMap2 pm, zombie.ai.astar.Mover mover, String kind, int i, float sx, float sy, int sz,
                          float tx, float ty, int tz, boolean nat) throws Exception {
        zombie.pathfind.TestRequest tr = new zombie.pathfind.TestRequest();
        if (nat) {
            zombie.pathfind.nativeCode.PathfindNative.instance.addRequest(tr, mover, sx, sy, (float) sz, tx, ty, (float) tz);
        } else {
            pm.addRequest(tr, mover, sx, sy, (float) sz, tx, ty, (float) tz);
        }
        long t0 = System.currentTimeMillis();
        while (!tr.done && System.currentTimeMillis() - t0 < 60000) {
            if (nat) zombie.pathfind.nativeCode.PathfindNative.instance.updateMain();
            else pm.updateMain();
            Thread.sleep(5);
        }
        out.println("P " + i + " " + kind + " " + f(sx) + "," + f(sy) + "," + sz + " -> " + f(tx) + "," + f(ty) + "," + tz
            + " done=" + (tr.done ? 1 : 0) + " " + pathStr(tr.path));
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
                else if (a[i].startsWith("pgraph:")) pgraph(out, a[i], cell);
                else if (a[i].startsWith("psearch:")) psearch(out, a[i], cell);
                else if (a[i].startsWith("preq:")) preq(out, a[i], cell);
                else throw new IllegalArgumentException("unknown scenario " + a[i]);
            }
        });
    }
}
