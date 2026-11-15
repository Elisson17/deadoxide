import java.io.File;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.TreeMap;
import zombie.PersistentOutfits;
import zombie.ZomboidFileSystem;
import zombie.core.skinnedmodel.population.ClothingItemReference;
import zombie.core.skinnedmodel.population.Outfit;
import zombie.core.skinnedmodel.population.OutfitManager;

// Inside the booted original (ServerHarness): the outfits the ORIGINAL OutfitManager read from media/clothing/clothing.xml
// at boot (sorted by name: init shuffles both lists with an unseeded Random), and PersistentOutfits after the ORIGINAL
// load() of the player's save's z_outfits.bin (copied over the server's own): the 500 seeds, and the outfit data built
// from the two outfit lists. Only the part made from clothing.xml is printed (the first K entries of all, the first F
// and M of the female and male lists); custom outfits that ZombiesZoneDefinition (Lua) registers come after them.
// Then getOutfit for ids whose variant needs no random draw.
// Args: <scratch dir> <save dir>
public class OutfitsProbe {
    static Object g(Object o, String name) throws Exception {
        Class<?> c = o instanceof Class<?> k ? k : o.getClass();
        Field f = c.getDeclaredField(name);
        f.setAccessible(true);
        return f.get(o instanceof Class<?> ? null : o);
    }

    static String fh(float f) { return Integer.toHexString(Float.floatToRawIntBits(f)); }
    static int b(boolean v) { return v ? 1 : 0; }

    static void item(java.io.PrintStream out, ClothingItemReference r, int depth) {
        out.println("I " + depth + " " + fh(r.probability) + " " + r.itemGuid + " " + b(r.randomized) + " " + b(r.immutable) + " " + r.subItems.size());
        for (ClothingItemReference s : r.subItems) item(out, s, depth + 1);
    }

    public static void main(String[] a) throws Exception {
        File save = new File(a[1]);
        ServerHarness.run(a[0], out -> {
            OutfitManager om = OutfitManager.instance;
            TreeMap<String, Object> names = new TreeMap<>(String.CASE_INSENSITIVE_ORDER);
            for (String sex : new String[]{"F", "M"}) {
                boolean female = sex.equals("F");
                List<Outfit> l = new ArrayList<>(female ? om.femaleOutfits : om.maleOutfits);
                l.sort(Comparator.comparing(o -> o.name));
                out.println("OM " + sex + " " + l.size() + " " + ((TreeMap<?, ?>) g(om, female ? "femaleOutfitMap" : "maleOutfitMap")).size());
                for (Outfit o : l) {
                    names.put(o.name, o);
                    Outfit same = female ? om.FindFemaleOutfit(o.name) : om.FindMaleOutfit(o.name);
                    Outfit upper = om.GetSpecificOutfit(female, o.name.toUpperCase());
                    out.println("O " + o.name + " " + b(o.top) + " " + b(o.pants) + " " + b(o.allowPantsHue) + " " + b(o.allowPantsTint) + " "
                        + b(o.allowTopTint) + " " + b(o.allowTshirtDecal) + " " + o.modId + " " + b(o.immutable) + " [" + String.join(",", o.topTextures)
                        + "] [" + String.join(",", o.pantsTextures) + "] " + o.items.size() + " " + b(same == o) + " " + b(upper == o));
                    for (ClothingItemReference r : o.items) item(out, r, 0);
                }
            }
            Files.copy(new File(save, "z_outfits.bin").toPath(), new File(ZomboidFileSystem.instance.getFileNameInCurrentSave("z_outfits.bin")).toPath(),
                StandardCopyOption.REPLACE_EXISTING);
            PersistentOutfits po = PersistentOutfits.instance;
            po.load();
            long[] seeds = (long[]) g(po, "seeds");
            for (int i = 0; i < seeds.length; i++) out.println("PS " + i + " " + seeds[i]);
            int k = names.size();
            List<?> all = (List<?>) g(po, "all");
            out.println("PA " + k);
            for (int i = 0; i < k; i++) {
                Object d = all.get(i);
                out.println("PD " + i + " " + g(d, "index") + " " + g(d, "outfitName") + " " + b((Boolean) g(d, "useSeed")) + " " + po.getOutfitNames().get(i));
            }
            for (String sex : new String[]{"F", "M"}) {
                List<?> l = (List<?>) g(po, sex.equals("F") ? "female" : "male");
                int n = (sex.equals("F") ? om.femaleOutfits : om.maleOutfits).size();
                out.println("PL " + sex + " " + n);
                StringBuilder sb = new StringBuilder();
                for (int i = 0; i < n; i++) sb.append(i == 0 ? "" : " ").append(g(l.get(i), "index"));
                out.println(sb);
            }
            int[] idx = {0, 1, k - 1, 32767};
            int[] var = {1, 250, 500};
            for (int i : idx) for (int v : var) for (int bits : new int[]{0, PersistentOutfits.FEMALE_BIT, PersistentOutfits.NO_HAT_BIT}) {
                int id = bits | i << 16 | v;
                out.println("GO " + id + " " + po.getOutfit(id));
            }
            out.println("GO 0 " + po.getOutfit(0));
        });
    }
}
