import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;
import zombie.GameTime;
import zombie.SandboxOptions;
import zombie.inventory.InventoryItem;
import zombie.inventory.InventoryItemFactory;
import zombie.inventory.ItemContainer;
import zombie.inventory.ItemUser;
import zombie.inventory.types.DrainableComboItem;
import zombie.inventory.types.Food;
import zombie.inventory.types.Literature;
import zombie.scripting.ScriptManager;
import zombie.scripting.objects.Item;
import zombie.scripting.objects.ItemTag;
import zombie.scripting.objects.ItemType;
import zombie.scripting.objects.ResourceLocation;

// Inside the booted original (dedicated server): inventory items made by InventoryItemFactory.CreateItem, the
// ItemContainer operations on them and the time-driven behaviour of the item classes. Floats print as raw bits and
// Float.toString. Args: <scratch dir> <case file>. Case lines (blank and # lines ignored), run in order:
//   create                       every item script sorted by full name: the created item's class, category, weights,
//                                condition, count and tags. Scripts whose creation reaches state this port does not model
//                                are listed as SKIP with the reason (class, component, oncreate): not created, so nothing
//                                about them is claimed.
//   reset <seed> [k=v,...]       a new scenario: the generator is a java.util.Random(seed) (RandAbstract.id counts the
//                                draws), no containers or variables; k is tod, nights, mult or sb.<SandboxOption>
//   container <name> <type> <capacity> [k=v,...]     new ItemContainer(type, null, null); k is accept (OnlyAcceptCategory), temp
//   new <var> <type>             InventoryItemFactory.CreateItem(type)
//   add <container> <var> <type> ItemContainer.AddItem(String)
//   put <container> <var>        ItemContainer.AddItem(InventoryItem)
//   remove <container> <var>     ItemContainer.Remove(InventoryItem)
//   set <var> <field> <value>    cond uses usesf age lastaged count cooked burnt cooktime freeze heat hung off offmax
//   show <var>                   every field the port models
//   items <container>            the items, in order
//   q <container> <query> [arg]  the ItemContainer queries, see query()
//   removeall <container> <type> [count] | removeone <container> <type> | removetype <container> <type>
//   tick <var> <n> <dtod>        n times: the clock advances dtod hours, then the item's update()
//   ageupdate <var>              Food.updateAge(false)
//   pick <var> <container> <index>   the item at that place of the container, by a name
//   use <var>                    InventoryItem.Use() (DrainableComboItem.Use() for a drainable)
//   useitem <var> [crafting inContainer consumes keep destroy]   ItemUser.UseItem
// The server flag is cleared while the probe runs, so that the items' update() reads the game clock and not the wall
// clock (InventoryItem.calculateTimeMultiplier).
public class InventoryProbe {
    static String hx(float f) {
        return Integer.toHexString(Float.floatToRawIntBits(f));
    }

    static String f(float v) {
        return hx(v) + "(" + v + ")";
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

    // ---- scenarios ----
    static final Map<String, ItemContainer> containers = new LinkedHashMap<>();
    static final Map<String, InventoryItem> vars = new LinkedHashMap<>();
    static final List<Runnable> undo = new ArrayList<>();

    static void setField(Object o, String name, Object v) throws Exception {
        Class<?> c = o.getClass();
        while (c != null) {
            try {
                Field fd = c.getDeclaredField(name);
                fd.setAccessible(true);
                fd.set(o, v);
                return;
            } catch (NoSuchFieldException e) { c = c.getSuperclass(); }
        }
        throw new NoSuchFieldException(name);
    }

    static Object getField(Object o, Class<?> c, String name) throws Exception {
        Field fd = c.getDeclaredField(name);
        fd.setAccessible(true);
        return fd.get(o);
    }

    static void swapRand(long seed) throws Exception {
        Field rf = zombie.core.random.RandAbstract.class.getDeclaredField("rand");
        rf.setAccessible(true);
        rf.set(zombie.core.random.RandStandard.INSTANCE, new java.util.Random(seed));
        Field idf = zombie.core.random.RandAbstract.class.getDeclaredField("id");
        idf.setAccessible(true);
        idf.set(zombie.core.random.RandStandard.INSTANCE, 0);
    }

    static int draws() throws Exception {
        return (Integer) getField(zombie.core.random.RandStandard.INSTANCE, zombie.core.random.RandAbstract.class, "id");
    }

    static String name(ItemContainer c) {
        for (Map.Entry<String, ItemContainer> e : containers.entrySet()) if (e.getValue() == c) return e.getKey();
        return c == null ? "-" : "?";
    }

    static String ref(InventoryItem i) {
        return i == null ? "null" : i.getFullType() + "@" + i.id;
    }

    static String refs(List<InventoryItem> l) {
        StringBuilder b = new StringBuilder("[");
        for (int n = 0; n < l.size(); n++) b.append(n == 0 ? "" : ",").append(ref(l.get(n)));
        return b.append(']').toString();
    }

    static String show(InventoryItem it) {
        StringBuilder b = new StringBuilder();
        b.append(it.getClass().getSimpleName()).append(' ').append(ref(it)).append(" in=").append(name(it.getContainer()));
        b.append(" cond=").append(it.getCondition()).append('/').append(it.getConditionMax());
        b.append(" uses=").append(it.getCurrentUses()).append('/').append(it.getMaxUses());
        b.append(" count=").append(it.getCount());
        b.append(" age=").append(f(it.getAge())).append(" lastAged=").append(f(it.getLastAged()));
        b.append(" aw=").append(f(it.getActualWeight())).append(" w=").append(f(it.getWeight()));
        b.append(" uw=").append(f(it.getUnequippedWeight())).append(" ew=").append(f(it.getEquippedWeight()));
        b.append(" customW=").append(it.isCustomWeight() ? 1 : 0);
        b.append(" cook=").append(f(it.getCookingTime())).append(it.isCooked() ? " cooked" : "").append(it.isBurnt() ? " burnt" : "");
        b.append(" off=").append(it.getOffAge()).append('/').append(it.getOffAgeMax());
        b.append(" wet=").append(it.isWet() ? 1 : 0).append(" wetCd=").append(f(it.getWetCooldown()));
        if (it instanceof Food fd) {
            b.append(" heat=").append(f(fd.getHeat()));
            b.append(" hung=").append(f(fd.getHungChange())).append(" hunger=").append(f(fd.getHungerChange())).append(" base=").append(f(fd.getBaseHunger()));
            b.append(" thirst=").append(f(fd.getThirstChange())).append(" unhappy=").append(f(fd.getUnhappyChange())).append(" boredom=").append(f(fd.getBoredomChange()));
            b.append(" stress=").append(f(fd.getStressChange())).append(" endur=").append(f(fd.getEnduranceChange()));
            b.append(" rotten=").append(fd.isRotten() ? 1 : 0).append(" fresh=").append(fd.isFresh() ? 1 : 0);
            b.append(" frozen=").append(fd.isFrozen() ? 1 : 0).append(" freezing=").append(f(fd.getFreezingTime())).append(" lastFrozen=").append(f(getFloat(fd, Food.class, "lastFrozenUpdate")));
            b.append(" lastCook=").append(fd.getLastCookMinute()).append(" tempAccum=").append(f(getFloat(fd, Food.class, "temperatureTimeAccum")));
            b.append(" cal=").append(f(fd.getCalories())).append(" poison=").append(fd.getPoisonPower()).append(" tainted=").append(fd.isTainted() ? 1 : 0);
        } else if (it instanceof DrainableComboItem dr) {
            b.append(" useDelta=").append(f(dr.getUseDelta())).append(" ticks=").append(f(dr.getTicks())).append(" heat=").append(f(dr.getHeat()));
            b.append(" weightEmpty=").append(f(dr.getWeightEmpty()));
            b.append(" lastUpd=").append(getInt(dr, DrainableComboItem.class, "lastUpdateMinutes")).append(" lastCook=").append(getInt(dr, DrainableComboItem.class, "lastCookMinute"));
        } else if (it instanceof Literature li) {
            b.append(" pages=").append(li.getNumberOfPages());
        }
        return b.toString();
    }

    static float getFloat(Object o, Class<?> c, String name) {
        try {
            return (Float) getField(o, c, name);
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }

    static int getInt(Object o, Class<?> c, String name) {
        try {
            return (Integer) getField(o, c, name);
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }

    static InventoryItem item(String v) {
        InventoryItem i = vars.get(v);
        if (i == null) throw new IllegalArgumentException("no variable " + v);
        return i;
    }

    static ItemContainer cont(String c) {
        ItemContainer x = containers.get(c);
        if (x == null) throw new IllegalArgumentException("no container " + c);
        return x;
    }

    static ItemTag tag(String t) {
        return ItemTag.get(ResourceLocation.of(t));
    }

    static void clock(String[] kv) throws Exception {
        GameTime gt = GameTime.instance;
        for (String e : kv) {
            String k = e.substring(0, e.indexOf('=')), v = e.substring(e.indexOf('=') + 1);
            switch (k) {
                case "tod" -> setField(gt, "timeOfDay", Float.parseFloat(v));
                case "nights" -> setField(gt, "nightsSurvived", Integer.parseInt(v));
                case "mult" -> gt.setMultiplier(Float.parseFloat(v));
                default -> {
                    if (!k.startsWith("sb.")) throw new IllegalArgumentException("option " + e);
                    SandboxOptions so = SandboxOptions.instance;
                    String n = k.substring(3);
                    Object old = so.getOptionByName(n).asConfigOption().getValueAsObject();
                    so.set(n, Double.valueOf(v));
                    undo.add(() -> so.set(n, old));
                }
            }
        }
    }

    static void reset(PrintStream out, String[] p) throws Exception {
        for (Runnable r : undo) r.run();
        undo.clear();
        containers.clear();
        vars.clear();
        GameTime gt = GameTime.instance;
        // the clock of a booted server, whatever an earlier scenario set
        setField(gt, "timeOfDay", 9.0f);
        setField(gt, "nightsSurvived", 0);
        gt.setMultiplier(1.0f);
        swapRand(Long.parseLong(p[1]));
        if (p.length > 2) clock(p[2].split(","));
        SandboxOptions so = SandboxOptions.instance;
        out.println("RESET " + String.join(" ", p) + " mult=" + f(gt.getMultiplier()) + " tod=" + f(gt.getTimeOfDay()) + " nights=" + gt.getNightsSurvived()
            + " ageHours=" + gt.getWorldAgeHours() + " foodRot=" + so.foodRotSpeed.getValue() + " fridge=" + so.fridgeFactor.getValue() + " elecShut=" + so.getElecShutModifier()
            + " removal=" + so.daysForRottenFoodRemoval.getValue() + " apo=" + so.getTimeSinceApo());
    }

    static void query(PrintStream out, String[] p) throws Exception {
        ItemContainer c = cont(p[1]);
        String q = p[2];
        String a = p.length > 3 ? p[3] : null;
        String r;
        switch (q) {
            case "contains" -> r = String.valueOf(c.contains(item(a)));
            case "has" -> r = String.valueOf(c.contains(a));
            case "hasr" -> r = String.valueOf(c.containsTypeRecurse(a));
            case "hasnb" -> r = String.valueOf(c.contains(a, false, true));
            case "count" -> r = String.valueOf(c.getCountType(a));
            case "alltype" -> r = refs(c.getAllType(a));
            case "alltag" -> r = refs(c.getAllTag(tag(a)));
            case "allcat" -> r = refs(c.getAllCategory(a));
            case "counttag" -> r = String.valueOf(c.getCountTag(tag(a)));
            case "firsttype" -> r = ref(c.getFirstType(a));
            case "firstcat" -> r = ref(c.getFirstCategory(a));
            case "firsttag" -> r = ref(c.getFirstTag(tag(a)));
            case "some" -> r = refs(c.getSomeType(a, Integer.parseInt(p[4])));
            case "best" -> r = ref(c.getBestCondition(a));
            case "find" -> r = ref(c.findItem(a, false, false));
            case "numberof" -> r = String.valueOf(c.getNumberOfItem(a));
            case "numberofdepl" -> r = String.valueOf(c.getNumberOfItem(a, true));
            case "withmodule" -> r = String.valueOf(c.containsWithModule(a));
            case "withmoduledelta" -> r = String.valueOf(c.containsWithModule(a, true));
            case "stack" -> r = ref(c.FindAndReturnStack(a));
            case "room" -> r = String.valueOf(c.hasRoomFor(null, item(a)));
            case "roomw" -> r = String.valueOf(c.hasRoomFor(null, Float.parseFloat(a)));
            case "allowed" -> r = String.valueOf(c.isItemAllowed(item(a)));
            case "weight" -> r = "capacityWeight=" + f(c.getCapacityWeight()) + " contents=" + f(c.getContentsWeight()) + " max=" + f(c.getMaxWeight()) + " capacity=" + c.getCapacity()
                + " effective=" + c.getEffectiveCapacity(null) + " free=" + f(c.getFreeCapacity(null)) + " full=" + c.isFull(null) + " available=" + f(c.getAvailableWeightCapacity())
                + " empty=" + c.isEmpty() + " size=" + c.getItems().size() + " temp=" + f(c.getTemperature());
            default -> throw new IllegalArgumentException("query " + q);
        }
        out.println("Q " + String.join(" ", p) + " = " + r);
    }

    static void set(String[] p) throws Exception {
        InventoryItem it = item(p[1]);
        String v = p[3];
        switch (p[2]) {
            case "cond" -> it.setConditionNoSound(Integer.parseInt(v));
            case "uses" -> it.setCurrentUses(Integer.parseInt(v));
            case "usesf" -> it.setCurrentUsesFloat(Float.parseFloat(v));
            case "age" -> it.setAge(Float.parseFloat(v));
            case "lastaged" -> it.setLastAged(Float.parseFloat(v));
            case "count" -> it.setCount(Integer.parseInt(v));
            case "cooked" -> it.setCooked(Boolean.parseBoolean(v));
            case "burnt" -> it.setBurnt(Boolean.parseBoolean(v));
            case "cooktime" -> it.setCookingTime(Float.parseFloat(v));
            case "freeze" -> ((Food) it).setFreezingTime(Float.parseFloat(v));
            case "heat" -> ((Food) it).setHeat(Float.parseFloat(v));
            case "hung" -> ((Food) it).setHungChange(Float.parseFloat(v));
            case "off" -> it.setOffAge(Integer.parseInt(v));
            case "offmax" -> it.setOffAgeMax(Integer.parseInt(v));
            case "taint" -> ((Food) it).setTainted(Boolean.parseBoolean(v));
            case "lastcook" -> ((Food) it).setLastCookMinute(Integer.parseInt(v));
            case "wet" -> it.setWet(Boolean.parseBoolean(v));
            case "wetcd" -> it.setWetCooldown(Float.parseFloat(v));
            case "dheat" -> ((DrainableComboItem) it).setHeat(Float.parseFloat(v));
            default -> throw new IllegalArgumentException("field " + p[2]);
        }
    }

    static void advance(float dtod) throws Exception {
        GameTime gt = GameTime.instance;
        float t = gt.getTimeOfDay() + dtod;
        int nights = gt.getNightsSurvived();
        while (t >= 24.0f) {
            t -= 24.0f;
            nights++;
        }
        setField(gt, "timeOfDay", t);
        setField(gt, "nightsSurvived", nights);
    }

    static void scenario(PrintStream out, String[] p) throws Exception {
        switch (p[0]) {
            case "reset" -> reset(out, p);
            case "container" -> {
                ItemContainer c = new ItemContainer(p[2], null, null);
                c.setCapacity(Integer.parseInt(p[3]));
                if (p.length > 4) for (String e : p[4].split(",")) {
                    String k = e.substring(0, e.indexOf('=')), v = e.substring(e.indexOf('=') + 1);
                    if (k.equals("accept")) c.setOnlyAcceptCategory(v);
                    else if (k.equals("temp")) c.setCustomTemperature(Float.parseFloat(v));
                    else throw new IllegalArgumentException("container option " + e);
                }
                containers.put(p[1], c);
                out.println("CONTAINER " + p[1] + " type=" + c.getType() + " capacity=" + c.getCapacity());
            }
            case "new" -> {
                InventoryItem i = InventoryItemFactory.CreateItem(p[2]);
                if (i != null) vars.put(p[1], i);
                out.println("NEW " + p[1] + " " + (i == null ? "null" : show(i)) + " draws=" + draws());
            }
            case "add" -> {
                InventoryItem i = cont(p[1]).AddItem(p[3]);
                if (i != null) vars.put(p[2], i);
                out.println("ADD " + p[1] + " " + p[2] + " " + (i == null ? "null" : show(i)) + " draws=" + draws());
            }
            case "put" -> {
                InventoryItem i = cont(p[1]).AddItem(item(p[2]));
                out.println("PUT " + p[1] + " " + ref(i) + " draws=" + draws());
            }
            case "remove" -> {
                cont(p[1]).Remove(item(p[2]));
                out.println("REMOVE " + p[1] + " " + p[2] + " in=" + name(item(p[2]).getContainer()));
            }
            case "removeall" -> out.println("REMOVEALL " + p[1] + " " + p[2] + " = " + refs(cont(p[1]).RemoveAll(p[2], p.length > 3 ? Integer.parseInt(p[3]) : cont(p[1]).getItems().size())));
            case "removeone" -> out.println("REMOVEONE " + p[1] + " " + p[2] + " = " + ref(cont(p[1]).RemoveOneOf(p[2], true)));
            case "removetype" -> {
                cont(p[1]).Remove(p[2]);
                out.println("REMOVETYPE " + p[1] + " " + p[2]);
            }
            case "set" -> {
                set(p);
                out.println("SET " + String.join(" ", p) + " -> " + show(item(p[1])));
            }
            case "show" -> out.println("SHOW " + p[1] + " " + show(item(p[1])));
            case "items" -> out.println("ITEMS " + p[1] + " " + refs(cont(p[1]).getItems()));
            case "q" -> query(out, p);
            case "clock" -> {
                clock(p[1].split(","));
                out.println("CLOCK " + p[1] + " tod=" + f(GameTime.instance.getTimeOfDay()) + " nights=" + GameTime.instance.getNightsSurvived());
            }
            case "tick" -> {
                InventoryItem it = item(p[1]);
                int n = Integer.parseInt(p[2]);
                float dtod = Float.parseFloat(p[3]);
                for (int k = 0; k < n; k++) {
                    advance(dtod);
                    it.update();
                }
                out.println("TICK " + String.join(" ", p) + " -> " + show(it) + " tod=" + f(GameTime.instance.getTimeOfDay()) + " tm=" + f(getFloat(it, InventoryItem.class, "timeMultiplier")) + " draws=" + draws());
            }
            case "pick" -> {
                InventoryItem i = cont(p[2]).getItems().get(Integer.parseInt(p[3]));
                vars.put(p[1], i);
                out.println("PICK " + p[1] + " " + ref(i));
            }
            case "use" -> {
                InventoryItem it = item(p[1]);
                it.Use();
                out.println("USE " + p[1] + " -> " + show(it) + " draws=" + draws());
            }
            case "useitem" -> {
                InventoryItem it = item(p[1]);
                int consumed = ItemUser.UseItem(it, p.length > 2 && Boolean.parseBoolean(p[2]), p.length > 3 && Boolean.parseBoolean(p[3]),
                    p.length > 4 ? Integer.parseInt(p[4]) : 1, p.length > 5 && Boolean.parseBoolean(p[5]), p.length > 6 && Boolean.parseBoolean(p[6]));
                out.println("USEITEM " + String.join(" ", p) + " = " + consumed + " -> " + show(it) + " draws=" + draws());
            }
            case "ageupdate" -> {
                ((Food) item(p[1])).updateAge(false);
                out.println("AGEUPDATE " + p[1] + " -> " + show(item(p[1])));
            }
            default -> throw new IllegalArgumentException("case " + String.join(" ", p));
        }
    }

    public static void main(String[] a) throws Exception {
        List<String> cases = Files.readAllLines(Paths.get(a[1]));
        ServerHarness.run(a[0], out -> {
            // the items' update() reads the wall clock on a server: run as the single-player game does
            zombie.network.GameServer.server = false;
            for (String line : cases) {
                String l = line.trim();
                if (l.isEmpty() || l.startsWith("#")) continue;
                String[] p = l.split("\\s+");
                if (p[0].equals("create")) create(out);
                else scenario(out, p);
            }
        });
    }
}
