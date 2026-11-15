import java.io.PrintStream;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import zombie.FileGuidPair;
import zombie.FileGuidTable;
import zombie.GameWindow;
import zombie.SandboxOptions;
import zombie.ZomboidFileSystem;
import zombie.core.Color;
import zombie.core.ImmutableColor;
import zombie.core.skinnedmodel.population.ClothingDecalGroup;
import zombie.core.skinnedmodel.population.ClothingDecals;
import zombie.core.skinnedmodel.population.ClothingItem;
import zombie.core.skinnedmodel.population.ClothingItemReference;
import zombie.core.skinnedmodel.population.Outfit;
import zombie.core.skinnedmodel.population.OutfitManager;
import zombie.core.skinnedmodel.population.OutfitRNG;
import zombie.util.LocationRNG;

// Inside the booted original (ServerHarness): the ORIGINAL seeded draws that dress a character. LocationRNG (seeds,
// floats, bounded ints, the per-square form), Color.HSBtoRGB over a grid that includes out-of-range inputs,
// OutfitRNG.randomImmutableColor with and without the NoBlackClothes sandbox option, ClothingDecals.getRandomDecal for
// every group of media/clothing/clothingDecals.xml, then ClothingItemReference.randomize for every item of a clone of
// every outfit of media/clothing/clothing.xml (sorted by name: OutfitManager shuffles the lists with an unseeded
// Random), each outfit under its own seed. The clothing item assets are loaded first (the probe pumps
// GameWindow.fileSystem on the main thread until every asset is ready), as OutfitManager.loaded() does in the game.
// Args: <scratch dir>
public class OutfitRandomProbe {
    static String fh(float f) { return Integer.toHexString(Float.floatToRawIntBits(f)); }
    static int b(boolean v) { return v ? 1 : 0; }
    static String c(ImmutableColor c) { return c == null ? "null" : fh(c.r) + "," + fh(c.g) + "," + fh(c.b) + "," + fh(c.a); }

    static void item(PrintStream out, ClothingItemReference r, int depth) {
        ClothingItemReference.RandomData d = r.randomData;
        out.println("R " + depth + " " + r.itemGuid + " " + b(r.randomized) + " " + b(d.active) + " " + fh(d.hue) + " " + c(d.tint) + " "
            + d.baseTexture + " " + (d.pickedItemRef == null ? "null" : d.pickedItemRef.itemGuid) + " " + d.textureChoice + " " + d.decal);
        for (ClothingItemReference s : r.subItems) item(out, s, depth + 1);
    }

    public static void main(String[] a) throws Exception {
        ServerHarness.run(a[0], out -> {
            long[] seeds = {0L, 1L, -1L, 12345L, Long.MIN_VALUE, Long.MAX_VALUE, 0x9E3779B97F4A7C15L};
            int[] bounds = {1, 2, 7, 100, 1000, 65536, Integer.MAX_VALUE, 0, -5};
            LocationRNG rng = new LocationRNG();
            for (long s : seeds) {
                rng.setSeed(s);
                StringBuilder sb = new StringBuilder("LR " + s + " " + rng.getSeed());
                for (int i = 0; i < 4; i++) sb.append(' ').append(fh(rng.nextFloat()));
                for (int n : bounds) sb.append(' ').append(rng.nextInt(n));
                out.println(sb);
            }
            int[][] squares = {{0, 0, 0}, {10500, 9800, 0}, {-3, 7, 1}, {300000, 200000, 7}};
            for (int[] q : squares) out.println("LRS " + q[0] + " " + q[1] + " " + q[2] + " " + rng.nextInt(1000, q[0], q[1], q[2]) + " " + rng.getSeed());

            float[] hs = {-1.25F, -0.5F, -1.0E-9F, 0.0F, 0.1F, 0.16666667F, 0.25F, 0.5F, 0.75F, 0.9999F, 1.0F, 2.3F};
            float[] ss = {0.0F, 0.3F, 1.0F, 1.5F};
            float[] vs = {0.0F, 0.5F, 1.0F, -0.2F};
            for (float h : hs) for (float s : ss) for (float v : vs) {
                Color col = Color.HSBtoRGB(h, s, v);
                out.println("HSB " + fh(h) + " " + fh(s) + " " + fh(v) + " " + fh(col.r) + "," + fh(col.g) + "," + fh(col.b) + "," + fh(col.a));
            }

            SandboxOptions.BooleanSandboxOption nbc = SandboxOptions.instance.noBlackClothes;
            boolean nbcWas = nbc.getValue();
            for (boolean v : new boolean[]{false, true}) {
                nbc.setValue(v);
                for (long s = 0; s < 20; s++) {
                    OutfitRNG.setSeed(s);
                    out.println("COL " + b(v) + " " + s + " " + c(OutfitRNG.randomImmutableColor()) + " " + c(OutfitRNG.randomImmutableColor(true)));
                }
            }

            ClothingDecals decals = ClothingDecals.instance;
            out.println("DECALS " + decals.groups.size());
            for (ClothingDecalGroup g : decals.groups) {
                StringBuilder sb = new StringBuilder("DG " + g.name + " " + b(decals.FindGroup(g.name.toUpperCase()) == g));
                for (long s = 0; s < 6; s++) {
                    OutfitRNG.setSeed(s * 31L + 7L);
                    sb.append(' ').append(decals.getRandomDecal(g.name));
                }
                out.println(sb);
            }
            out.println("DGX " + decals.getRandomDecal("NoSuchGroup") + " " + decals.getRandomDecal(" "));

            FileGuidTable t = ZomboidFileSystem.instance.getFileGuidTable();
            OutfitManager om = OutfitManager.instance;
            List<ClothingItem> items = new ArrayList<>();
            for (FileGuidPair p : t.files) items.add(om.getClothingItem(p.guid));
            long deadline = System.nanoTime() + 60_000_000_000L;
            while (true) {
                boolean pending = false;
                for (ClothingItem ci : items) if (ci != null && ci.isEmpty()) pending = true;
                if (!pending) break;
                if (System.nanoTime() > deadline) throw new IllegalStateException("clothing items still loading");
                GameWindow.fileSystem.updateAsyncTransactions();
                Thread.sleep(2);
            }

            for (boolean v : new boolean[]{true, false}) {
                nbc.setValue(v);
                for (String sex : new String[]{"F", "M"}) {
                    boolean female = sex.equals("F");
                    List<Outfit> l = new ArrayList<>(female ? om.femaleOutfits : om.maleOutfits);
                    l.sort(Comparator.comparing(o -> o.name));
                    for (int i = 0; i < l.size(); i++) {
                        Outfit o = l.get(i).clone();
                        OutfitRNG.setSeed(i * 7919L + (female ? 1L : 2L) + (v ? 0L : 1000003L));
                        for (ClothingItemReference r : o.items) r.randomize();
                        out.println("O " + b(v) + " " + sex + " " + i + " " + o.name + " " + o.items.size() + " " + OutfitRNG.Next(1000000));
                        for (ClothingItemReference r : o.items) item(out, r, 0);
                    }
                }
            }
            nbc.setValue(nbcWas);
        });
    }
}
