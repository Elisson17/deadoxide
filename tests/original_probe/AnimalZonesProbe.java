import java.io.File;
import java.lang.reflect.Field;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import zombie.characters.animals.AnimalZone;
import zombie.characters.animals.AnimalZoneJunction;
import zombie.iso.IsoMetaCell;
import zombie.iso.IsoMetaGrid;
import zombie.iso.IsoWorld;

// Inside the booted original (ServerHarness): IsoWorld.init's next two steps after map_zone.bin, on the player's save.
// Every metagrid/metacell_X_Y.bin goes through the ORIGINAL IsoMetaCell.load into getCellOrCreate's cell (file names
// sorted), then the ORIGINAL IsoMetaGrid.loadAnimalZones(ByteBuffer, -1) reads map_animals.bin (dropping the animal
// zones the server registered at boot). The probe prints the loaded cells' flag, every animal zone ordered by id with
// its junctions, and which zones each cell holds.
// Args: <scratch dir> <save dir>
public class AnimalZonesProbe {
    static long h;
    static void mix(int v) { for (int i = 0; i < 4; i++) { h ^= (v >>> (8 * i)) & 255; h *= 0x100000001b3L; } }
    static int b(boolean v) { return v ? 1 : 0; }

    static Object g(Object o, String name) throws Exception {
        Class<?> c = o.getClass();
        while (c != null) {
            try {
                Field f = c.getDeclaredField(name);
                f.setAccessible(true);
                return f.get(o);
            } catch (NoSuchFieldException e) { c = c.getSuperclass(); }
        }
        throw new NoSuchFieldException(name);
    }

    public static void main(String[] a) throws Exception {
        File save = new File(a[1]);
        ServerHarness.run(a[0], out -> {
            IsoMetaGrid g = IsoWorld.instance.getMetaGrid();
            Pattern p = Pattern.compile("metacell_(-?[0-9]+)_(-?[0-9]+)\\.bin");
            String[] files = new File(save, "metagrid").list();
            if (files == null) files = new String[0];
            Arrays.sort(files);
            for (String f : files) {
                Matcher m = p.matcher(f);
                if (!m.matches()) continue;
                int cx = Integer.parseInt(m.group(1)), cy = Integer.parseInt(m.group(2));
                IsoMetaCell cell = g.getCellOrCreate(cx - g.minX, cy - g.minY);
                cell.load(g, ByteBuffer.wrap(Files.readAllBytes(new File(new File(save, "metagrid"), f).toPath())), -1);
                out.println("MC " + cx + " " + cy + " " + b((Boolean) g(cell, "animalZonesGenerated")));
            }
            byte[] data = Files.readAllBytes(new File(save, "map_animals.bin").toPath());
            ByteBuffer bb = ByteBuffer.wrap(data);
            out.println("MA " + bb.getInt(4));
            g.loadAnimalZones(bb, -1);
            out.println("REST " + bb.remaining());
            List<AnimalZone> zones = new ArrayList<>(g.animalZoneHandler.getZones());
            zones.sort((x, y) -> x.id.compareTo(y.id));
            IdentityHashMap<AnimalZone, Integer> index = new IdentityHashMap<>();
            for (int i = 0; i < zones.size(); i++) index.put(zones.get(i), i);
            out.println("AZN " + zones.size());
            for (int i = 0; i < zones.size(); i++) {
                AnimalZone z = zones.get(i);
                h = 0xcbf29ce484222325L;
                for (int k = 0; k < z.points.size(); k++) mix(z.points.get(k));
                int nj = z.junctions == null ? -1 : z.junctions.size();
                out.println("AZ " + i + " " + z.id + " " + z.type + " " + z.name + " " + z.x + " " + z.y + " " + z.z + " " + z.w + " " + z.h + " "
                    + z.geometryType + " " + z.polylineWidth + " " + z.points.size() + " " + Long.toHexString(h) + " " + z.getOriginalName() + " "
                    + z.action + " " + g(z, "animalType") + " " + b((Boolean) g(z, "spawnAnimal")) + " " + b((Boolean) g(z, "spawnedAnimals")) + " " + nj);
                if (z.junctions == null) continue;
                for (AnimalZoneJunction j : z.junctions) {
                    Integer other = j.zoneOther == null ? null : index.get(j.zoneOther);
                    out.println("AJ " + i + " " + j.pointIndexSelf + " " + (other == null ? -1 : other) + " " + j.pointIndexOther + " "
                        + Float.floatToIntBits(j.distanceFromStart));
                }
            }
            for (int x = g.minX; x <= g.maxX; x++) for (int y = g.minY; y <= g.maxY; y++) {
                IsoMetaCell cell = g.getCellData(x, y);
                if (cell == null || cell.getAnimalZonesSize() == 0) continue;
                h = 0xcbf29ce484222325L;
                for (int k = 0; k < cell.getAnimalZonesSize(); k++) {
                    Integer i = index.get(cell.getAnimalZone(k));
                    mix(i == null ? -1 : i);
                }
                out.println("AC " + x + " " + y + " " + cell.getAnimalZonesSize() + " " + Long.toHexString(h));
            }
        });
    }
}
