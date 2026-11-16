import java.io.File;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.Arrays;
import java.util.List;
import zombie.SandboxOptions;
import zombie.ZomboidFileSystem;
import zombie.iso.IsoWorld;
import zombie.iso.SliceY;
import zombie.worldMap.WorldMapVisited;

// Inside the booted original (ServerHarness): the ORIGINAL WorldMapVisited (the map's visited and known units) is made
// by getInstance(), which loads map_visited.bin from the server's current save, optionally marks the whole map
// known (the MapAllKnown sandbox option) and saves the file again; the instance's own mutators and save() are called.
// The commands come from a text file (# lines are comments):
//   mapallknown 0|1                         SandboxOptions Map.MapAllKnown
//   open real|none|keep                     the player's real map_visited.bin / no file / the file as it is, then
//                                           Reset() and getInstance(); prints OPEN, DUMP
//   openhex <hex>                           the file is these bytes, then as open
//   openbody <delta> <hex>                  a file with the current grid's header and (the array's length + delta) body
//                                           bytes made of the hex repeated, then as open
//   setVisitedSquares|setKnownSquares|clearVisitedSquares|clearKnownSquares a b c d
//   setVisitedCells|setKnownCells|clearVisitedCells|clearKnownCells a b c d
//   save                                    instance.save(); prints SAVE (the file's length and hash)
//   dump                                    prints DUMP
// The slice buffer is zeroed before every getInstance(): the file is read through it and a short row of the legacy
// format keeps what the buffer held, so the result would otherwise depend on every earlier use of the buffer.
// Args: <scratch dir> <commands file> <save dir>
public class VisitedProbe {
    static String hex(int v) { return Integer.toHexString(v); }

    static byte[] unhex(String s) {
        byte[] b = new byte[s.length() / 2];
        for (int i = 0; i < b.length; i++) b[i] = (byte) Integer.parseInt(s.substring(2 * i, 2 * i + 2), 16);
        return b;
    }

    static File cur() { return new File(ZomboidFileSystem.instance.getFileNameInCurrentSave("map_visited.bin")); }

    static String file() throws Exception {
        File f = cur();
        if (!f.exists()) return "none";
        byte[] b = Files.readAllBytes(f.toPath());
        return b.length + "," + hex(Arrays.hashCode(b));
    }

    static void dump(PrintStream out, int n, WorldMapVisited v) throws Exception {
        Field f = WorldMapVisited.class.getDeclaredField("visited");
        f.setAccessible(true);
        byte[] a = (byte[]) f.get(v);
        long[] c = new long[4];
        for (byte x : a) for (int k = 0; k < 4; k++) c[x >> 2 * k & 3]++;
        out.println("DUMP " + n + " len=" + a.length + " hash=" + hex(Arrays.hashCode(a)) + " units=" + c[0] + "," + c[1] + "," + c[2] + "," + c[3]
            + " bounds=" + v.getMinX() + "," + v.getMinY());
    }

    public static void main(String[] a) throws Exception {
        List<String> lines = Files.readAllLines(new File(a[1]).toPath());
        File save = new File(a[2]);
        ServerHarness.run(a[0], out -> {
            WorldMapVisited v = null;
            int n = 0;
            for (String line : lines) {
                if (line.isEmpty() || line.startsWith("#")) continue;
                String[] t = line.split(" ");
                n++;
                switch (t[0]) {
                    case "mapallknown" -> SandboxOptions.getInstance().map.mapAllKnown.setValue(t[1].equals("1"));
                    case "open", "openhex", "openbody" -> {
                        if (t[0].equals("open")) {
                            if (t[1].equals("real")) Files.copy(new File(save, "map_visited.bin").toPath(), cur().toPath(), StandardCopyOption.REPLACE_EXISTING);
                            else if (t[1].equals("none")) cur().delete();
                        } else if (t[0].equals("openhex")) {
                            Files.write(cur().toPath(), unhex(t[1]));
                        } else {
                            IsoWorld w = IsoWorld.instance;
                            int len = WorldMapVisited.getVisitedLength() + Integer.parseInt(t[1]);
                            byte[] pat = unhex(t[2]);
                            java.io.ByteArrayOutputStream bo = new java.io.ByteArrayOutputStream();
                            java.io.DataOutputStream d = new java.io.DataOutputStream(bo);
                            d.writeInt(249); d.writeInt(2);
                            d.writeInt(w.getMetaGrid().minX); d.writeInt(w.getMetaGrid().minY); d.writeInt(w.getMetaGrid().maxX); d.writeInt(w.getMetaGrid().maxY);
                            d.writeInt(8);
                            for (int i = 0; i < len; i++) d.writeByte(pat[i % pat.length]);
                            Files.write(cur().toPath(), bo.toByteArray());
                        }
                        Arrays.fill(SliceY.SliceBuffer.array(), (byte) 0);
                        WorldMapVisited.Reset();
                        v = WorldMapVisited.getInstance();
                        out.println("OPEN " + n + " file=" + file());
                        dump(out, n, v);
                    }
                    case "setVisitedSquares" -> v.setVisitedInSquares(Integer.parseInt(t[1]), Integer.parseInt(t[2]), Integer.parseInt(t[3]), Integer.parseInt(t[4]));
                    case "setKnownSquares" -> v.setKnownInSquares(Integer.parseInt(t[1]), Integer.parseInt(t[2]), Integer.parseInt(t[3]), Integer.parseInt(t[4]));
                    case "clearVisitedSquares" -> v.clearVisitedInSquares(Integer.parseInt(t[1]), Integer.parseInt(t[2]), Integer.parseInt(t[3]), Integer.parseInt(t[4]));
                    case "clearKnownSquares" -> v.clearKnownInSquares(Integer.parseInt(t[1]), Integer.parseInt(t[2]), Integer.parseInt(t[3]), Integer.parseInt(t[4]));
                    case "setVisitedCells" -> v.setVisitedInCells(Integer.parseInt(t[1]), Integer.parseInt(t[2]), Integer.parseInt(t[3]), Integer.parseInt(t[4]));
                    case "setKnownCells" -> v.setKnownInCells(Integer.parseInt(t[1]), Integer.parseInt(t[2]), Integer.parseInt(t[3]), Integer.parseInt(t[4]));
                    case "clearVisitedCells" -> v.clearVisitedInCells(Integer.parseInt(t[1]), Integer.parseInt(t[2]), Integer.parseInt(t[3]), Integer.parseInt(t[4]));
                    case "clearKnownCells" -> v.clearKnownInCells(Integer.parseInt(t[1]), Integer.parseInt(t[2]), Integer.parseInt(t[3]), Integer.parseInt(t[4]));
                    case "save" -> {
                        v.save();
                        out.println("SAVE " + n + " file=" + file());
                    }
                    case "dump" -> dump(out, n, v);
                    default -> throw new IllegalArgumentException("unknown command: " + line);
                }
            }
        });
    }
}
