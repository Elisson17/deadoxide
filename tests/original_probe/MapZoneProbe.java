import java.io.File;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.TreeMap;
import java.util.UUID;
import zombie.iso.IsoMetaCell;
import zombie.iso.IsoMetaChunk;
import zombie.iso.IsoMetaGrid;
import zombie.iso.IsoWorld;
import zombie.iso.zones.Zone;

// Inside the booted original (ServerHarness): the ORIGINAL IsoMetaGrid.loadZone(ByteBuffer, -1) reads the save's
// map_zone.bin into the world's meta grid (dropping the zones the server registered at boot), then the probe prints
// every registered zone, the outline the native ClipperOffset made for each wide polyline (its points in the
// native order; only with the "outlines" or "triangles" argument), with "triangles" every zone's Zone.getTotalArea
// and the triangles and triangle areas the native Clipper/earcut made, which zones each meta chunk holds per cell,
// and IsoWorld.spawnedZombieZone.
// Args: <scratch dir> <save dir> [outlines|triangles]
public class MapZoneProbe {
    static long h;
    static void mix(int v) { for (int i = 0; i < 4; i++) { h ^= (v >>> (8 * i)) & 255; h *= 0x100000001b3L; } }
    static int b(boolean v) { return v ? 1 : 0; }

    public static void main(String[] a) throws Exception {
        File save = new File(a[1]);
        boolean triangles = a.length > 2 && a[2].equals("triangles");
        boolean outlines = triangles || a.length > 2 && a[2].equals("outlines");
        ServerHarness.run(a[0], out -> {
            IsoMetaGrid g = IsoWorld.instance.getMetaGrid();
            byte[] data = Files.readAllBytes(new File(save, "map_zone.bin").toPath());
            ByteBuffer bb = ByteBuffer.wrap(data);
            out.println("MZ " + bb.getInt(4) + " " + g.minX + " " + g.minY + " " + g.maxX + " " + g.maxY);
            g.loadZone(bb, -1);
            out.println("REST " + bb.remaining());
            out.println("ZN " + g.zones.size());
            IdentityHashMap<Zone, Integer> index = new IdentityHashMap<>();
            for (int i = 0; i < g.zones.size(); i++) {
                Zone z = g.zones.get(i);
                index.put(z, i);
                h = 0xcbf29ce484222325L;
                for (int k = 0; k < z.points.size(); k++) mix(z.points.get(k));
                out.println("Z " + i + " " + z.type + " " + z.name + " " + z.x + " " + z.y + " " + z.z + " " + z.w + " " + z.h + " " + z.geometryType + " "
                    + z.polylineWidth + " " + z.points.size() + " " + Long.toHexString(h) + " " + z.hourLastSeen + " " + b(z.haveConstruction) + " "
                    + z.lastActionTimestamp + " " + z.getOriginalName() + " " + z.id + " " + b(z.isPreferredZoneForSquare));
                float[] o = z.polylineOutlinePoints;
                if (o != null && outlines) {
                    int n = o.length / 2;
                    StringBuilder sb = new StringBuilder("O " + i + " " + n);
                    for (int k = 0; k < n; k++) {
                        sb.append(' ').append(Float.floatToIntBits(o[k * 2])).append(',').append(Float.floatToIntBits(o[k * 2 + 1]));
                    }
                    out.println(sb);
                }
            }
            if (triangles) {
                for (int i = 0; i < g.zones.size(); i++) {
                    Zone z = g.zones.get(i);
                    float area;
                    try {
                        area = z.getTotalArea();
                    } catch (Throwable e) {
                        out.println("AX " + i + " " + e.getClass().getName());
                        continue;
                    }
                    out.println("A " + i + " " + Float.floatToIntBits(area));
                    float[] t = z.triangles;
                    if (t == null) continue;
                    StringBuilder sb = new StringBuilder("T " + i + " " + t.length);
                    for (float v : t) sb.append(' ').append(Float.floatToIntBits(v));
                    out.println(sb);
                    sb = new StringBuilder("TA " + i + " " + z.triangleAreas.length);
                    for (float v : z.triangleAreas) sb.append(' ').append(Float.floatToIntBits(v));
                    out.println(sb);
                }
            }
            int total = 0;
            for (int x = g.minX; x <= g.maxX; x++) for (int y = g.minY; y <= g.maxY; y++) {
                IsoMetaCell cell = g.getCellData(x, y);
                if (cell == null) continue;
                int chunks = 0;
                h = 0xcbf29ce484222325L;
                for (int i = 0; i < 1024; i++) {
                    if (!cell.hasChunk(i)) continue;
                    IsoMetaChunk c = cell.getChunk(i);
                    if (c.getZonesSize() == 0) continue;
                    chunks++;
                    mix(i);
                    mix(c.getZonesSize());
                    for (int k = 0; k < c.getZonesSize(); k++) mix(index.get(c.getZone(k)));
                    total += c.getZonesSize();
                }
                if (chunks > 0) out.println("ZC " + x + " " + y + " " + b(cell.info != null) + " " + chunks + " " + Long.toHexString(h));
            }
            out.println("ZT " + total);
            HashMap<String, ArrayList<UUID>> sz = IsoWorld.instance.getSpawnedZombieZone();
            out.println("SZ " + sz.size());
            for (String k : new TreeMap<>(sz).keySet()) out.println("S " + k + " " + sz.get(k));
        });
    }
}
