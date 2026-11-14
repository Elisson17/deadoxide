import java.io.File;
import java.lang.reflect.Field;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;
import zombie.basements.Basements;
import zombie.characters.Faction;
import zombie.core.stash.StashBuilding;
import zombie.core.stash.StashSystem;
import zombie.iso.*;
import zombie.iso.areas.DesignationZone;
import zombie.iso.areas.NonPvpZone;
import zombie.iso.areas.SafeHouse;
import zombie.network.GameServer;
import zombie.randomizedWorld.randomizedBuilding.RBBasic;

// Inside the booted original (ServerHarness) whose save folder was seeded with the player's map_basements.bin, so the
// world's meta grid has the save's basements (Basements.loadSavefile). Then the original IsoMetaGrid.load(ByteBuffer)
// reads the save's map_meta.bin as a single-player game does (GameServer.server false during the call), and the probe
// prints the rooms' and buildings' saved state per cell, plus the lists stored after them.
// Args: <scratch dir> <save dir>
public class MapMetaProbe {
    static long h;
    static void mix(int v) { for (int i = 0; i < 4; i++) { h ^= (v >>> (8 * i)) & 255; h *= 0x100000001b3L; } }
    static void mixLong(long v) { mix((int) v); mix((int) (v >> 32)); }
    static int b(boolean v) { return v ? 1 : 0; }

    static Object g(Object o, String name) throws Exception {
        Class<?> c = o instanceof Class ? (Class<?>) o : o.getClass();
        while (c != null) {
            try {
                Field f = c.getDeclaredField(name);
                f.setAccessible(true);
                return f.get(o instanceof Class ? null : o);
            } catch (NoSuchFieldException e) { c = c.getSuperclass(); }
        }
        throw new NoSuchFieldException(name);
    }

    static int roomFlags(RoomDef r) {
        return b(r.explored) | b(r.lightsActive) << 1 | b(r.doneSpawn) << 2 | b(r.isRoofFixed()) << 3;
    }

    public static void main(String[] a) throws Exception {
        File save = new File(a[1]);
        ServerHarness.run(a[0], new File[]{new File(save, "map_basements.bin")}, out -> {
            IsoMetaGrid g = IsoWorld.instance.getMetaGrid();
            List<?> placements = (List<?>) g(Basements.getInstance(), "basementPlacements");
            h = 0xcbf29ce484222325L;
            for (Object p : placements) {
                mix((int) g(p, "x")); mix((int) g(p, "y")); mix((int) g(p, "z")); mix((int) g(p, "w")); mix((int) g(p, "h"));
                for (char ch : ((String) g(p, "name")).toCharArray()) mix(ch);
            }
            out.println("BP " + placements.size() + " " + Long.toHexString(h));
            @SuppressWarnings("unchecked")
            ArrayList<BuildingDef> buildings = (ArrayList<BuildingDef>) g(g, "buildings");
            h = 0xcbf29ce484222325L;
            for (BuildingDef d : buildings) { mixLong(d.id); mixLong(d.metaId); }
            out.println("B " + buildings.size() + " " + Long.toHexString(h));

            byte[] data = Files.readAllBytes(new File(save, "map_meta.bin").toPath());
            ByteBuffer bb = ByteBuffer.wrap(data);
            out.println("MM " + bb.getInt(4) + " " + bb.getInt(8) + " " + bb.getInt(12) + " " + bb.getInt(16) + " " + bb.getInt(20));
            // Rooms and buildings the file does not cover keep what IsoMetaGrid.Create drew from the global Rand at
            // boot (lightsActive, alarmed, keyId), which the port cannot reproduce: reset them to the field defaults
            // first, so the comparison is about what load() applies.
            for (int x = g.minX; x <= g.maxX; x++) for (int y = g.minY; y <= g.maxY; y++) {
                IsoMetaCell cell = g.getCellData(x, y);
                if (cell == null || cell.info == null) continue;
                for (RoomDef r : cell.roomList) { r.explored = false; r.lightsActive = false; r.doneSpawn = false; r.setRoofFixed(false); }
                for (BuildingDef d : cell.buildings) { d.alarmed = false; d.setKeyId(-1); d.seen = false; d.hasBeenVisited = false; d.lootRespawnHour = -1; d.alarmDecay = 10000000; }
            }
            boolean server = GameServer.server;
            GameServer.server = false;
            try { g.load(bb); } finally { GameServer.server = server; }
            out.println("REST " + bb.remaining());

            int[] t = new int[9];
            for (int x = g.minX; x <= g.maxX; x++) for (int y = g.minY; y <= g.maxY; y++) {
                IsoMetaCell cell = g.getCellData(x, y);
                if (cell == null || cell.info == null) continue;
                StringBuilder detail = new StringBuilder();
                h = 0xcbf29ce484222325L;
                for (RoomDef r : cell.roomList) {
                    int f = roomFlags(r);
                    mixLong(r.metaId); mix(f);
                    t[0]++;
                    for (int i = 0; i < 4; i++) if ((f & 1 << i) != 0) t[2 + i]++;
                    if ((f & ~2) != 0) detail.append("MR ").append(r.metaId).append(' ').append(f).append('\n');
                }
                for (BuildingDef d : cell.buildings) {
                    mixLong(d.metaId); mix(b(d.alarmed)); mix(d.getKeyId()); mix(b(d.seen)); mix(b(d.hasBeenVisited)); mix(d.lootRespawnHour); mix(d.alarmDecay);
                    t[1]++;
                    if (d.alarmed) t[6]++;
                    if (d.seen) t[7]++;
                    if (d.hasBeenVisited) t[8]++;
                    if (d.alarmed || d.seen || d.hasBeenVisited)
                        detail.append("MD ").append(d.metaId).append(' ').append(b(d.alarmed)).append(' ').append(d.getKeyId()).append(' ').append(b(d.seen)).append(' ')
                            .append(b(d.hasBeenVisited)).append(' ').append(d.lootRespawnHour).append(' ').append(d.alarmDecay).append('\n');
                }
                out.println("MC " + x + " " + y + " " + cell.roomList.size() + " " + cell.buildings.size() + " " + Long.toHexString(h));
                out.print(detail);
            }
            out.println("MT " + t[0] + " " + t[1] + " " + t[2] + " " + t[3] + " " + t[4] + " " + t[5] + " " + t[6] + " " + t[7] + " " + t[8]);

            ArrayList<SafeHouse> sh = SafeHouse.getSafehouseList();
            out.println("SH " + sh.size());
            for (SafeHouse s : sh)
                out.println("S " + s.getX() + " " + s.getY() + " " + s.getW() + " " + s.getH() + " " + s.getOwner() + " " + s.getHitPoints() + " " + s.getPlayers() + " "
                    + s.getLastVisited() + " " + s.getTitle() + " " + s.getDatetimeCreated() + " " + s.getLocation() + " " + g(s, "playersRespawn"));
            out.println("NP " + NonPvpZone.getAllZones().size());
            for (NonPvpZone z : NonPvpZone.getAllZones())
                out.println("N " + z.getX() + " " + z.getY() + " " + z.getX2() + " " + z.getY2() + " " + z.getSize() + " " + z.getTitle());
            out.println("FA " + Faction.getFactions().size());
            for (Faction f : Faction.getFactions())
                out.println("F " + f.getName() + " " + f.getOwner() + " " + f.getTag() + " " + Float.floatToIntBits(f.getTagColor().r) + " "
                    + Float.floatToIntBits(f.getTagColor().g) + " " + Float.floatToIntBits(f.getTagColor().b) + " " + f.getPlayers());
            out.println("DZ " + DesignationZone.allZones.size());

            @SuppressWarnings("unchecked") ArrayList<StashBuilding> possible = (ArrayList<StashBuilding>) g(StashSystem.class, "possibleStashes");
            @SuppressWarnings("unchecked") ArrayList<StashBuilding> todo = (ArrayList<StashBuilding>) g(StashSystem.class, "buildingsToDo");
            @SuppressWarnings("unchecked") ArrayList<String> read = (ArrayList<String>) g(StashSystem.class, "alreadyReadMap");
            out.println("ST " + possible.size() + " " + todo.size() + " " + read.size());
            for (StashBuilding s : possible) out.println("SP " + s.stashName + " " + s.buildingX + " " + s.buildingY);
            for (StashBuilding s : todo) out.println("SB " + s.stashName + " " + s.buildingX + " " + s.buildingY);
            for (String s : read) out.println("SR " + s);
            ArrayList<String> rds = RBBasic.getUniqueRDSSpawned();
            out.println("RDS " + rds.size());
            for (String s : rds) out.println("R " + s);
        });
    }
}
