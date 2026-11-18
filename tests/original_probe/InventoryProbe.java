import java.io.PrintStream;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.TreeSet;
import zombie.inventory.InventoryItem;
import zombie.inventory.InventoryItemFactory;
import zombie.scripting.ScriptManager;
import zombie.scripting.objects.Item;
import zombie.scripting.objects.ItemType;

// Inside the booted original (dedicated server): inventory items made by InventoryItemFactory.CreateItem from every item
// script and what the item classes report about them. Floats as raw bits. Args: <scratch dir> <case file>
// Case lines (blank and # lines ignored):
//   create    every item script sorted by full name: the created item's class, category, weights, condition, count and
//             tags. Scripts whose creation reaches state this port does not model are listed as SKIP with the reason
//             (class, component, oncreate): they are not created, so nothing about them is claimed.
public class InventoryProbe {
    static String hx(float f) {
        return Integer.toHexString(Float.floatToRawIntBits(f));
    }

    static boolean modelled(ItemType t) {
        return t == ItemType.NORMAL || t == ItemType.DRAINABLE || t == ItemType.FOOD || t == ItemType.LITERATURE;
    }

    // why the script is not created in this report, or null
    static String skip(Item it) {
        if (!modelled(it.getItemType())) return "class " + it.getItemType();
        if (!it.getComponentScripts().isEmpty()) return "component";
        if (it.getLuaCreate() != null) return "oncreate";
        return null;
    }

    static void create(PrintStream out) {
        ArrayList<Item> items = new ArrayList<>(ScriptManager.instance.getAllItems());
        items.sort((x, y) -> x.getFullName().compareTo(y.getFullName()));
        for (Item it : items) {
            String why = skip(it);
            if (why != null) {
                out.println("SKIP " + it.getFullName() + " " + why);
                continue;
            }
            InventoryItem item = InventoryItemFactory.CreateItem(it.getFullName());
            if (item == null) {
                out.println("NULL " + it.getFullName());
                continue;
            }
            TreeSet<String> tags = new TreeSet<>();
            for (Object t : item.getTags()) tags.add(String.valueOf(t));
            out.println("ITEM " + it.getFullName() + " " + item.getClass().getSimpleName() + " cat=" + item.getCategory() + " aw=" + hx(item.getActualWeight()) + " w=" + hx(item.getWeight())
                + " uw=" + hx(item.getUnequippedWeight()) + " ew=" + hx(item.getEquippedWeight()) + " cond=" + item.getCondition() + "/" + item.getConditionMax() + " count=" + item.getCount()
                + " tags=" + tags);
        }
    }

    public static void main(String[] a) throws Exception {
        List<String> cases = Files.readAllLines(Paths.get(a[1]));
        ServerHarness.run(a[0], out -> {
            for (String line : cases) {
                String l = line.trim();
                if (l.isEmpty() || l.startsWith("#")) continue;
                String[] p = l.split("\\s+");
                switch (p[0]) {
                    case "create" -> create(out);
                    default -> throw new IllegalArgumentException("case " + l);
                }
            }
        });
    }
}
