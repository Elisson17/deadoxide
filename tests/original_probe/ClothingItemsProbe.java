import java.util.ArrayList;
import java.util.List;
import zombie.FileGuidPair;
import zombie.FileGuidTable;
import zombie.GameWindow;
import zombie.ZomboidFileSystem;
import zombie.core.skinnedmodel.population.ClothingItem;
import zombie.core.skinnedmodel.population.OutfitManager;

// Inside the booted original (ServerHarness): the ORIGINAL file guid table (media/fileGuidTable.xml) with its lookups
// both ways (in other letter case), then OutfitManager.getClothingItem for every guid in table order. The clothing
// item assets load asynchronously; the probe runs on the main thread and pumps GameWindow.fileSystem itself until every
// asset is ready or failed, then prints each item's fields.
// Args: <scratch dir>
public class ClothingItemsProbe {
    static String l(List<?> v) {
        StringBuilder sb = new StringBuilder("[");
        for (int i = 0; i < v.size(); i++) sb.append(i == 0 ? "" : ",").append(v.get(i));
        return sb.append("]").toString();
    }

    static int b(boolean v) { return v ? 1 : 0; }

    public static void main(String[] a) throws Exception {
        ServerHarness.run(a[0], out -> {
            FileGuidTable t = ZomboidFileSystem.instance.getFileGuidTable();
            out.println("FGT " + t.files.size());
            for (FileGuidPair p : t.files) {
                out.println("FG " + p.path + " " + p.guid + " " + b(p.path.equals(t.getFilePathFromGuid(p.guid.toUpperCase()))) + " "
                    + b(p.guid.equals(t.getGuidFromFilePath(p.path.toLowerCase()))));
            }
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
            for (int i = 0; i < items.size(); i++) {
                ClothingItem ci = om.getClothingItem(t.files.get(i).guid);
                if (ci == null) {
                    out.println("CI " + i + " null");
                    continue;
                }
                out.println("CI " + i + " " + ci.guid + " " + ci.mame + " " + b(ci.isFailure()) + " " + ci.maleModel + " " + ci.femaleModel + " " + ci.altMaleModel
                    + " " + ci.altFemaleModel + " " + b(ci.isStatic) + " " + l(ci.baseTextures) + " " + ci.attachBone + " " + l(ci.masks) + " "
                    + ci.masksFolder + " " + ci.underlayMasksFolder + " " + l(ci.textureChoices) + " " + b(ci.allowRandomHue) + " "
                    + b(ci.allowRandomTint) + " " + ci.decalGroup + " " + ci.shader + " " + ci.hatCategory + " " + l(ci.spawnWith) + " "
                    + b(ci.isHat()) + " " + b(ci.isMask()) + " " + b(ci.hasModel()));
            }
        });
    }
}
