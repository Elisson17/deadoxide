import java.io.File;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Random;
import java.util.TimeZone;
import zombie.GameTime;
import zombie.SandboxOptions;
import zombie.ZomboidFileSystem;
import zombie.core.random.RandAbstract;
import zombie.core.random.RandStandard;
import zombie.erosion.ErosionData;
import zombie.erosion.ErosionMain;
import zombie.erosion.season.ErosionSeason;
import zombie.iso.CellLoader;
import zombie.iso.IsoCell;
import zombie.iso.IsoChunk;
import zombie.iso.IsoGridSquare;
import zombie.iso.IsoMetaGrid;
import zombie.iso.IsoObject;
import zombie.iso.IsoWorld;
import zombie.iso.objects.IsoTree;
import zombie.iso.sprite.IsoSpriteInstance;
import zombie.network.GameServer;
import zombie.network.ServerMap;

// Inside the booted original dedicated server (ServerHarness), with the JVM's default zone pinned to UTC: the ORIGINAL
// zombie.erosion.* run over chunks of the real map.
//
// Per scenario "x1,y1,x2,y2:year:month:day:days:dumps[:options]" (chunk coordinates x1..x2 by y1..y2, month and day
// 0-based as GameTime holds them, dumps a list of day counts separated by "/"; 0 is straight after the load):
//  - the erosion.ini files (the save's and the cache's copy) are deleted, the sandbox options are set (options:
//    speed=1..5 ErosionSpeed, edays=N ErosionDays, apo=1..13 TimeSinceApo, temp=1..5 Temperature; the defaults of a new
//    server are speed 3, edays 0, apo 1, temp 3; keep=1 leaves the erosion.ini the previous scenario wrote in place, so
//    that the new ErosionMain reads it), GameTime is set to the date, and a new ErosionMain is made (it reads the
//    config, builds the noise, season, ice queen and the four regions with their categories);
//  - the chunks are loaded by CellLoader.LoadCellBinaryChunk with the global Rand on java.util.Random(12345), then
//    through IsoChunk.loadInWorldStreamerThread, and put into a ServerMap cell the way the server's loader leaves
//    them (so IsoWorld.currentCell.getGridSquare finds them); loaded = true as ServerCell.RecalcAll2 sets it;
//  - then, in the order of ServerCell.RecalcAll2 and IsoChunk.doLoadGridsquare (chunks x outer, y inner; per chunk
//    levels, then x outer, y inner), every square that has objects goes through ErosionMain.LoadGridsquare, and every
//    chunk through ErosionMain.ChunkLoaded afterwards;
//  - then, per simulated day: the date advances (as the game clock does at midnight), the original
//    ErosionMain.EveryTenMinutes (mainTimer) runs 144 times (ten-minute marks of the day).
// On a server ErosionData.Square.rand reads the static table, so the global Rand must not be drawn from by erosion:
// its draw counter is printed before and after. Printed: the set-up, ErosionMain's own state, the erosion.ini the
// original wrote, and at every dump the ErosionMain state, the season, the per-chunk data (ErosionData.Chunk.save),
// and for every square that has objects, sorted by x, y, z: the erosion data as ErosionData.Square.save wrote it
// (hex), the kudzu noise, the property flags, and the objects (class:name:sprite+attached sprites).
// Args: <scratch dir> <scenario>...
public class ErosionProbe {
    static Object g(Object o, String name) throws Exception {
        Class<?> c = o instanceof Class<?> k ? k : o.getClass();
        Object target = o instanceof Class<?> ? null : o;
        while (c != null) {
            try {
                Field f = c.getDeclaredField(name);
                f.setAccessible(true);
                return f.get(target);
            } catch (NoSuchFieldException e) { c = c.getSuperclass(); }
        }
        throw new NoSuchFieldException(name);
    }

    static void set(Object o, Class<?> c, String name, Object v) throws Exception {
        Field f = c.getDeclaredField(name);
        f.setAccessible(true);
        f.set(o, v);
    }

    static String f(Object v) { return Float.toString((Float) v); }
    static int b(Object v) { return (Boolean) v ? 1 : 0; }

    static String hex(ByteBuffer bb) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < bb.position(); i++) sb.append(String.format("%02x", bb.get(i) & 0xff));
        return sb.toString();
    }

    public static void main(String[] a) throws Exception {
        TimeZone.setDefault(TimeZone.getTimeZone("UTC"));
        ServerHarness.run(a[0], out -> {
            for (int i = 1; i < a.length; i++) scenario(out, a[i]);
        });
    }

    static void deleteIni() throws Exception {
        for (String p : new String[]{ZomboidFileSystem.instance.getFileNameInCurrentSave("erosion.ini"),
                ZomboidFileSystem.instance.getCacheDir() + File.separator + "erosion.ini"}) new File(p).delete();
    }

    static String describe(ErosionMain em) throws Exception {
        ErosionSeason s = em.getSeasons();
        return "ticks=" + g(em, "ticks") + " eTicks=" + g(em, "eTicks") + " epoch=" + g(em, "epoch") + " tickUnit=" + g(em, "tickUnit")
            + " date=" + g(em, "year") + "-" + g(em, "month") + "-" + g(em, "day") + " season=" + s.getSeason() + " seasonDay=" + f(s.getSeasonDay())
            + " seasonDays=" + f(s.getSeasonDays()) + " progression=" + f(s.getSeasonProgression()) + " snow=" + b(g(em, "isSnow"));
    }

    static void scenario(PrintStream out, String spec) throws Exception {
        String[] p = spec.split(":");
        String[] rc = p[0].split(",");
        int x1 = Integer.parseInt(rc[0]), y1 = Integer.parseInt(rc[1]), x2 = Integer.parseInt(rc[2]), y2 = Integer.parseInt(rc[3]);
        int year = Integer.parseInt(p[1]), month = Integer.parseInt(p[2]), day = Integer.parseInt(p[3]);
        int days = Integer.parseInt(p[4]);
        List<Integer> dumps = new ArrayList<>();
        for (String d : p[5].split("/")) dumps.add(Integer.parseInt(d));
        int speed = 3, edays = 0, apo = 1, temp = 3;
        boolean keep = false;
        if (p.length > 6) {
            for (String kv : p[6].split(",")) {
                String[] e = kv.split("=", 2);
                switch (e[0]) {
                    case "speed" -> speed = Integer.parseInt(e[1]);
                    case "edays" -> edays = Integer.parseInt(e[1]);
                    case "apo" -> apo = Integer.parseInt(e[1]);
                    case "temp" -> temp = Integer.parseInt(e[1]);
                    case "keep" -> keep = Integer.parseInt(e[1]) != 0;
                    default -> throw new IllegalArgumentException(kv);
                }
            }
        }
        out.println("SCENARIO " + spec);

        if (!keep) deleteIni();
        SandboxOptions so = SandboxOptions.instance;
        so.erosionSpeed.setValue(speed);
        so.erosionDays.setValue(edays);
        so.timeSinceApo.setValue(apo);
        so.temperature.setValue(temp);
        GameTime gt = GameTime.getInstance();
        gt.setYear(year);
        gt.setMonth(month);
        gt.setDay(day);
        gt.setTimeOfDay(12.0F);
        gt.setNightsSurvived(0);
        int spritesBefore = zombie.iso.sprite.IsoSpriteManager.instance.namedMap.size();
        ErosionMain em = new ErosionMain(zombie.iso.sprite.IsoSpriteManager.instance, false);
        out.println("SETUP server=" + b(GameServer.server) + " speed=" + so.getErosionSpeed() + " edays=" + so.erosionDays.getValue()
            + " apo=" + so.timeSinceApo.getValue() + " temp=" + so.getTemperatureModifier() + " loadedCells=" + ServerMap.instance.loadedCells.size()
            + " " + describe(em) + " newSprites=" + (zombie.iso.sprite.IsoSpriteManager.instance.namedMap.size() - spritesBefore));

        // chunks as the server's loader leaves them
        IsoCell cell = IsoWorld.instance.currentCell;
        Field rf = RandAbstract.class.getDeclaredField("rand");
        rf.setAccessible(true);
        Method gp = IsoChunk.class.getDeclaredMethod("getFromPool");
        gp.setAccessible(true);
        ServerMap sm = ServerMap.instance;
        HashMap<Long, ServerMap.ServerCell> cells = new HashMap<>();
        Field cellMapF = ServerMap.class.getDeclaredField("cellMap");
        cellMapF.setAccessible(true);
        ServerMap.ServerCell[] cellMap = (ServerMap.ServerCell[]) cellMapF.get(sm);
        Field widthF = ServerMap.class.getDeclaredField("width");
        widthF.setAccessible(true);
        int width = (Integer) widthF.get(sm);
        List<IsoChunk> chunks = new ArrayList<>();
        for (int wx = x1; wx <= x2; wx++) for (int wy = y1; wy <= y2; wy++) {
            set(RandStandard.INSTANCE, RandAbstract.class, "rand", new Random(12345L));
            IsoChunk ch = (IsoChunk) gp.invoke(null);
            ch.wx = wx; ch.wy = wy;
            CellLoader.LoadCellBinaryChunk(cell, wx, wy, ch);
            ch.loadInWorldStreamerThread();
            int cx = wx / 8, cy = wy / 8;
            ServerMap.ServerCell sc = cells.get(((long) cx << 32) | cy);
            if (sc == null) {
                sc = new ServerMap.ServerCell();
                sc.wx = cx; sc.wy = cy;
                cells.put(((long) cx << 32) | cy, sc);
                int idx = (cy - sm.getMinY()) * width + (cx - sm.getMinX());
                if (cellMap[idx] != null) throw new IllegalStateException("cell already loaded " + cx + "," + cy);
                cellMap[idx] = sc;
                sm.loadedCells.add(sc);
            }
            sc.chunks[wx - cx * 8][wy - cy * 8] = ch;
            chunks.add(ch);
        }
        for (ServerMap.ServerCell sc : cells.values()) sc.isLoaded = true;
        for (IsoChunk ch : chunks) ch.loaded = true;

        set(RandStandard.INSTANCE, RandAbstract.class, "rand", new Random(12345L));
        set(RandStandard.INSTANCE, RandAbstract.class, "id", 0);
        out.println("RAND before=" + g(RandStandard.INSTANCE, "id"));
        // ServerMap.ServerCell.RecalcAll2 order: chunks x outer, y inner; IsoChunk.doLoadGridsquare: levels, x outer, y inner
        for (ServerMap.ServerCell sc : sortedCells(cells)) {
            for (int x = 0; x < 8; x++) for (int y = 0; y < 8; y++) {
                IsoChunk ch = sc.chunks[x][y];
                if (ch == null) continue;
                for (int zz = ch.getMinLevel(); zz <= ch.getMaxLevel(); zz++) {
                    for (int sx = 0; sx < 8; sx++) for (int sy = 0; sy < 8; sy++) {
                        IsoGridSquare sq = ch.getGridSquare(sx, sy, zz);
                        if (sq != null && !sq.getObjects().isEmpty()) ErosionMain.LoadGridsquare(sq);
                    }
                }
                ErosionMain.ChunkLoaded(ch);
            }
        }
        out.println("RAND after load=" + g(RandStandard.INSTANCE, "id"));
        int dumped = 0;
        if (dumps.contains(0)) { dump(out, em, chunks, 0, spritesBefore); dumped++; }
        for (int d = 1; d <= days; d++) {
            gt.setDay(gt.getDay() + 1);
            if (gt.getDay() >= gt.daysInMonth(gt.getYear(), gt.getMonth())) {
                gt.setDay(0);
                gt.setMonth(gt.getMonth() + 1);
                if (gt.getMonth() >= 12) {
                    gt.setMonth(0);
                    gt.setYear(gt.getYear() + 1);
                }
            }
            for (int m = 0; m < 144; m++) ErosionMain.EveryTenMinutes();
            if (dumps.contains(d)) { dump(out, em, chunks, d, spritesBefore); dumped++; }
        }
        out.println("RAND end=" + g(RandStandard.INSTANCE, "id") + " dumps=" + dumped);
        File ini = new File(ZomboidFileSystem.instance.getFileNameInCurrentSave("erosion.ini"));
        out.println("INI exists=" + (ini.exists() ? 1 : 0));
        if (ini.exists()) for (String line : java.nio.file.Files.readAllLines(ini.toPath())) out.println("INI " + line);

        // take the chunks out of the server map again for the next scenario
        for (ServerMap.ServerCell sc : cells.values()) {
            int idx = (sc.wy - sm.getMinY()) * width + (sc.wx - sm.getMinX());
            cellMap[idx] = null;
            sm.loadedCells.remove(sc);
        }
        out.println("END " + spec);
    }

    static List<ServerMap.ServerCell> sortedCells(HashMap<Long, ServerMap.ServerCell> cells) {
        List<ServerMap.ServerCell> l = new ArrayList<>(cells.values());
        l.sort((p, q) -> p.wx != q.wx ? Integer.compare(p.wx, q.wx) : Integer.compare(p.wy, q.wy));
        return l;
    }

    static void dump(PrintStream out, ErosionMain em, List<IsoChunk> chunks, int day, int spritesBefore) throws Exception {
        out.println("DUMP " + day + " " + describe(em) + " newSprites=" + (zombie.iso.sprite.IsoSpriteManager.instance.namedMap.size() - spritesBefore));
        List<IsoChunk> cs = new ArrayList<>(chunks);
        cs.sort((p, q) -> p.wx != q.wx ? Integer.compare(p.wx, q.wx) : Integer.compare(p.wy, q.wy));
        ByteBuffer bb = ByteBuffer.allocate(1 << 16);
        List<IsoGridSquare> all = new ArrayList<>();
        for (IsoChunk ch : cs) {
            bb.clear();
            ErosionData.Chunk cd = ch.getErosionData();
            cd.save(bb);
            out.println("C " + ch.wx + " " + ch.wy + " " + hex(bb) + " moisture=" + f(g(cd, "moisture")) + " minerals=" + f(g(cd, "minerals")));
            for (int z = ch.getMinLevel(); z <= ch.getMaxLevel(); z++)
                for (int y = 0; y < 8; y++) for (int x = 0; x < 8; x++) {
                    IsoGridSquare sq = ch.getGridSquare(x, y, z);
                    if (sq != null && !sq.getObjects().isEmpty()) all.add(sq);
                }
        }
        all.sort((p, q) -> p.x != q.x ? Integer.compare(p.x, q.x) : p.y != q.y ? Integer.compare(p.y, q.y) : Integer.compare(p.z, q.z));
        for (IsoGridSquare sq : all) {
            ErosionData.Square ed = sq.getErosionData();
            bb.clear();
            ed.save(bb);
            StringBuilder sb = new StringBuilder("S " + sq.x + " " + sq.y + " " + sq.z + " " + hex(bb) + " kudzu=" + Integer.toHexString(Float.floatToRawIntBits(ed.noiseKudzu)));
            sb.append(' ').append(sq.haveRoof ? 1 : 0).append(' ');
            boolean first = true;
            for (zombie.iso.SpriteDetails.IsoFlagType fl : sq.getProperties().getFlagsList()) { if (!first) sb.append(','); first = false; sb.append(fl.name()); }
            sb.append(' ').append(Long.toHexString(sq.hasTypes));
            for (int oi = 0; oi < sq.getObjects().size(); oi++) {
                IsoObject o = sq.getObjects().get(oi);
                sb.append(' ').append(o.getClass().getSimpleName()).append(':').append(o.getName()).append(':')
                    .append(o.getSprite() == null ? "null" : o.getSprite().getName());
                if (o.attachedAnimSprite != null) for (int ai = 0; ai < o.attachedAnimSprite.size(); ai++) sb.append('+').append(o.attachedAnimSprite.get(ai).parentSprite.getName());
                if (o instanceof IsoTree t) sb.append("{s").append(g(t, "size")).append('}');
            }
            out.println(sb);
        }
    }
}
