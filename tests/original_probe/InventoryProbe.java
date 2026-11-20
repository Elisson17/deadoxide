import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.nio.ByteBuffer;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;
import zombie.GameTime;
import zombie.SandboxOptions;
import zombie.core.Core;
import zombie.inventory.InventoryItem;
import zombie.inventory.InventoryItemFactory;
import zombie.inventory.ItemContainer;
import zombie.inventory.ItemUser;
import se.krka.kahlua.j2se.KahluaTableImpl;
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
//   container <name> <type> <capacity> [k=v,...]     new ItemContainer(type, null, null); k is accept (OnlyAcceptCategory), temp, explored, looted
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
//   regs                         WorldDictionary.getItemRegistryID of every item script
//   field <var> <name> <value>   a field of the item set directly (by its Java type), see FIELDS
//   list <var> <name> a,b,c      a list field (spices, extraItems, learnedRecipes, customPages)
//   bytes <var> <hex>            byteData
//   mod <var> <path> <s|d|b|t> <value>   modData: a dotted path of string keys (#n a number key), a value of that type
//   fields <var>                 the fields the save writes that show does not print
//   save <var>                   InventoryItem.save: the bytes
//   roundtrip <var> <newvar>     saveWithSize, InventoryItem.loadItem of those bytes into <newvar>, then its bytes again
//   csave <container>            ItemContainer.save: the bytes
//   cload <container> <newcontainer>   a new container that loads those bytes, then its items and its bytes again
//   createsave                   every modelled script made, saved, loaded and saved again
//   recordedmedia <hex of a save dir>   the media ids of that save's recorded_media.bin become the indexes of the world's RecordedMedia (no output)
//   rawitem <name> <hex>         a block of a real save (InventoryItem.saveWithSize format): loadItem, the state, saved again, equal or not
//   rawcont <name> <hex>         the bytes of an ItemContainer.save of a real save: load, the items' state, save again, equal or not
//                                (both print SKIPPED when an item is of a class, script or kind this probe does not model)
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
        // a world draws these two with Core.initPoisonousBerry/Mushroom; fixed here so a scenario does not depend on the boot
        Core.getInstance().setPoisonousBerry("Base.BerryGeneric2");
        Core.getInstance().setPoisonousMushroom("Base.MushroomGeneric3");
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


    // ---- save and load ----
    static final int WORLD_VERSION = zombie.iso.IsoWorld.getWorldVersion();

    static String hex(byte[] a, int from, int to) {
        StringBuilder b = new StringBuilder();
        for (int i = from; i < to; i++) b.append(String.format("%02x", a[i]));
        return b.toString();
    }

    static byte[] unhex(String h) {
        byte[] r = new byte[h.length() / 2];
        for (int i = 0; i < r.length; i++) r[i] = (byte) Integer.parseInt(h.substring(2 * i, 2 * i + 2), 16);
        return r;
    }

    // InventoryItem.load ends with setCustomName(), which rewrites modData customName with the item's name in the language
    // of the loading game; a save written by a game in another language would not save again byte for byte. The name the
    // save held is found in the block's table (key type 0, WriteString "customName", value type 0, WriteString name) and
    // put back before the item is saved again, so that "same" tells the writers, not the language.
    static final byte[] NAME_KEY = {0, 0, 10, 'c', 'u', 's', 't', 'o', 'm', 'N', 'a', 'm', 'e', 0};

    static String origName(byte[] raw, int from, int to) {
        outer:
        for (int i = from; i + NAME_KEY.length + 2 <= to; i++) {
            for (int k = 0; k < NAME_KEY.length; k++) if (raw[i + k] != NAME_KEY[k]) continue outer;
            int p = i + NAME_KEY.length;
            int len = ((raw[p] & 0xff) << 8) | (raw[p + 1] & 0xff);
            if (p + 2 + len > to) return null;
            return new String(raw, p + 2, len, java.nio.charset.StandardCharsets.UTF_8);
        }
        return null;
    }

    static int int32(byte[] r, int p) {
        return ((r[p] & 0xff) << 24) | ((r[p + 1] & 0xff) << 16) | ((r[p + 2] & 0xff) << 8) | (r[p + 3] & 0xff);
    }

    static void restoreName(InventoryItem it, String name) {
        if (it != null && name != null) ((KahluaTableImpl) it.getModData()).rawset("customName", name);
    }

    static String bits(ByteBuffer bb) {
        return "len=" + bb.position() + " " + hex(bb.array(), 0, bb.position());
    }

    static ByteBuffer saved(InventoryItem it) throws Exception {
        ByteBuffer bb = ByteBuffer.allocate(1 << 20);
        it.save(bb, false);
        return bb;
    }

    static ByteBuffer savedSized(InventoryItem it) throws Exception {
        ByteBuffer bb = ByteBuffer.allocate(1 << 20);
        it.saveWithSize(bb, false);
        return bb;
    }

    static InventoryItem reloaded(ByteBuffer bb) throws Exception {
        ByteBuffer in = ByteBuffer.wrap(bb.array(), 0, bb.position());
        return InventoryItem.loadItem(in, WORLD_VERSION);
    }

    static String tableText(KahluaTableImpl t) {
        StringBuilder b = new StringBuilder("{");
        se.krka.kahlua.vm.KahluaTableIterator it = t.iterator();
        boolean first = true;
        while (it.advance()) {
            if (!first) b.append(',');
            first = false;
            Object k = it.getKey(), v = it.getValue();
            b.append(k instanceof String ? "s:" + k : k instanceof Double ? "d:" + Double.doubleToRawLongBits((Double) k) : "?:" + k).append('=');
            b.append(v instanceof String ? "s:" + v : v instanceof Double ? "d:" + Double.doubleToRawLongBits((Double) v) : v instanceof Boolean ? "b:" + v : v instanceof KahluaTableImpl tt ? tableText(tt) : "?:" + v);
        }
        return b.append('}').toString();
    }

    static Object peek(Object o, String name) {
        try {
            return getField(o, o.getClass(), name);
        } catch (NoSuchFieldException e) {
            Class<?> c = o.getClass().getSuperclass();
            while (c != null) {
                try {
                    return getField(o, c, name);
                } catch (Exception e2) { c = c.getSuperclass(); }
            }
            throw new RuntimeException(e);
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }

    static String pv(Object v) {
        if (v == null) return "null";
        if (v instanceof Float x) return f(x);
        if (v instanceof ByteBuffer bb) return "bytes:" + hex(bb.array(), 0, bb.limit());
        if (v instanceof zombie.core.Color c) return "rgba:" + hx(c.r) + "," + hx(c.g) + "," + hx(c.b) + "," + hx(c.a);
        if (v instanceof KahluaTableImpl t) return tableText(t);
        if (v instanceof java.util.Map<?, ?> m) return new java.util.TreeMap<>(m).toString();
        return String.valueOf(v);
    }

    static final String[] COMMON_FIELDS = {"table", "customName", "customColor", "col", "itemCapacity", "keyId", "remoteControlId", "remoteRange", "colorRed", "colorGreen", "colorBlue",
        "worker", "stashMap", "zombieInfected", "attachedSlot", "attachedSlotType", "attachedToModel", "maxCapacity", "recordedMediaIndex", "worldXRotation", "worldYRotation",
        "worldZRotation", "worldScale", "isInitialised", "modelIndex", "activated", "haveBeenRepaired", "favorite", "byteData", "extraItems", "name", "originalName", "registryId",
        "currentAmmoCount", "customWeight", "isCookable"};
    static final String[] FOOD_FIELDS = {"calories", "proteins", "lipids", "carbohydrates", "dangerousUncooked", "poisonDetectionLevel", "spices", "poisonPower", "chef", "painReduction",
        "fluReduction", "foodSicknessChange", "poison", "useForPoison", "rottenTime", "compostTime", "cookedInMicrowave", "fatigueChange", "endChange", "milkQty", "milkType",
        "stressChange", "frozen", "isCookable"};
    static final String[] LITERATURE_FIELDS = {"numberOfPages", "alreadyReadPages", "canBeWrite", "customPages", "lockedBy", "learnedRecipes", "alreadyRead"};

    static String fields(InventoryItem it) {
        StringBuilder b = new StringBuilder();
        for (String n : COMMON_FIELDS) b.append(' ').append(n).append('=').append(pv(peek(it, n)));
        if (it instanceof Food) for (String n : FOOD_FIELDS) b.append(" F.").append(n).append('=').append(pv(peek(it, n)));
        if (it instanceof Literature) for (String n : LITERATURE_FIELDS) b.append(" L.").append(n).append('=').append(pv(peek(it, n)));
        return b.toString();
    }

    static void field(String[] p) throws Exception {
        InventoryItem it = item(p[1]);
        Class<?> c = it.getClass();
        java.lang.reflect.Field fd = null;
        while (c != null && fd == null) {
            try { fd = c.getDeclaredField(p[2]); } catch (NoSuchFieldException e) { c = c.getSuperclass(); }
        }
        if (fd == null) throw new NoSuchFieldException(p[2]);
        fd.setAccessible(true);
        Class<?> t = fd.getType();
        String v = p[3];
        if (t == int.class) fd.setInt(it, Integer.parseInt(v));
        else if (t == short.class) fd.setShort(it, Short.parseShort(v));
        else if (t == byte.class) fd.setByte(it, Byte.parseByte(v));
        else if (t == float.class) fd.setFloat(it, Float.parseFloat(v));
        else if (t == boolean.class) fd.setBoolean(it, Boolean.parseBoolean(v));
        else if (t == String.class) fd.set(it, v.equals("null") ? null : v);
        else if (t == zombie.core.Color.class) {
            String[] x = v.split(",");
            fd.set(it, new zombie.core.Color(Float.parseFloat(x[0]), Float.parseFloat(x[1]), Float.parseFloat(x[2]), Float.parseFloat(x[3])));
        } else throw new IllegalArgumentException("field type " + t + " of " + p[2]);
    }

    static void list(String[] p) throws Exception {
        InventoryItem it = item(p[1]);
        String[] x = p[3].equals("null") ? null : p[3].equals("empty") ? new String[0] : p[3].split(",");
        switch (p[2]) {
            case "spices" -> ((Food) it).setSpices(x == null ? null : new ArrayList<>(java.util.Arrays.asList(x)));
            case "extraItems" -> setField(it, "extraItems", x == null ? null : new ArrayList<>(java.util.Arrays.asList(x)));
            case "learnedRecipes" -> ((Literature) it).setLearnedRecipes(x == null ? null : new ArrayList<>(java.util.Arrays.asList(x)));
            case "customPages" -> {
                java.util.HashMap<Integer, String> m = null;
                if (x != null) {
                    m = new java.util.HashMap<>();
                    for (int i = 0; i < x.length; i++) m.put(i + 1, x[i]);
                }
                ((Literature) it).setCustomPages(m);
            }
            default -> throw new IllegalArgumentException("list " + p[2]);
        }
    }

    static Object modKey(String k) {
        return k.startsWith("#") ? (Object) Double.valueOf(k.substring(1)) : k;
    }

    static void mod(String[] p) throws Exception {
        InventoryItem it = item(p[1]);
        KahluaTableImpl t = (KahluaTableImpl) it.getModData();
        String[] path = p[2].split("\\.");
        for (int i = 0; i < path.length - 1; i++) {
            Object n = t.rawget(modKey(path[i]));
            if (!(n instanceof KahluaTableImpl)) {
                n = zombie.Lua.LuaManager.platform.newTable();
                t.rawset(modKey(path[i]), n);
            }
            t = (KahluaTableImpl) n;
        }
        Object key = modKey(path[path.length - 1]);
        switch (p[3]) {
            case "s" -> t.rawset(key, p[4]);
            case "d" -> t.rawset(key, Double.valueOf(p[4]));
            case "b" -> t.rawset(key, Boolean.valueOf(p[4]));
            case "t" -> t.rawset(key, zombie.Lua.LuaManager.platform.newTable());
            default -> throw new IllegalArgumentException("mod type " + p[3]);
        }
    }

    static void createsave(PrintStream out) throws Exception {
        ArrayList<Item> items = new ArrayList<>(ScriptManager.instance.getAllItems());
        items.sort((x, y) -> x.getFullName().compareTo(y.getFullName()));
        for (Item sc : items) {
            if (skip(sc) != null) continue;
            if (sc.getFullName().equals("Base.CorpseAnimal")) {
                // Food.load -> initAnimalIcons -> loadCorpseFromByteData: builds a dead animal body
                out.println("SKIP " + sc.getFullName() + " load builds an animal corpse");
                continue;
            }
            InventoryItem item = InventoryItemFactory.CreateItem(sc.getFullName());
            if (item == null) {
                out.println("SAVED " + sc.getFullName() + " null");
                continue;
            }
            ByteBuffer bb = savedSized(item);
            InventoryItem back = reloaded(bb);
            String again = back == null ? "null" : bits(savedSized(back));
            out.println("SAVED " + sc.getFullName() + " " + bits(bb) + " back=" + again + " draws=" + draws());
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
                    else if (k.equals("explored")) c.setExplored(Boolean.parseBoolean(v));
                    else if (k.equals("looted")) c.setHasBeenLooted(Boolean.parseBoolean(v));
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
            case "field" -> {
                field(p);
                out.println("FIELD " + String.join(" ", p) + " -> " + show(item(p[1])) + fields(item(p[1])));
            }
            case "list" -> {
                list(p);
                out.println("LIST " + String.join(" ", p) + " -> " + fields(item(p[1])));
            }
            case "bytes" -> {
                setField(item(p[1]), "byteData", p[2].equals("null") ? null : ByteBuffer.wrap(unhex(p[2])));
                out.println("BYTES " + p[1] + " " + p[2]);
            }
            case "mod" -> {
                mod(p);
                out.println("MOD " + String.join(" ", p) + " -> " + pv(peek(item(p[1]), "table")));
            }
            case "fields" -> out.println("FIELDS " + p[1] + fields(item(p[1])));
            case "save" -> out.println("SAVE " + p[1] + " " + bits(saved(item(p[1]))));
            case "roundtrip" -> {
                InventoryItem it = item(p[1]);
                ByteBuffer bb = savedSized(it);
                InventoryItem back = reloaded(bb);
                if (back != null) vars.put(p[2], back);
                out.println("ROUNDTRIP " + p[1] + " " + p[2] + " " + bits(bb) + " draws=" + draws());
                if (back == null) out.println("BACK " + p[2] + " null");
                else out.println("BACK " + p[2] + " " + show(back) + fields(back) + " again=" + bits(savedSized(back)));
            }
            case "csave" -> {
                ByteBuffer bb = ByteBuffer.allocate(1 << 20);
                cont(p[1]).save(bb);
                out.println("CSAVE " + p[1] + " " + bits(bb));
            }
            case "cload" -> {
                ByteBuffer bb = ByteBuffer.allocate(1 << 20);
                cont(p[1]).save(bb);
                ByteBuffer in = ByteBuffer.wrap(bb.array(), 0, bb.position());
                ItemContainer n = new ItemContainer();
                n.load(in, WORLD_VERSION);
                containers.put(p[2], n);
                ByteBuffer again = ByteBuffer.allocate(1 << 20);
                n.save(again);
                out.println("CLOAD " + p[1] + " " + p[2] + " type=" + n.getType() + " capacity=" + n.getCapacity() + " explored=" + n.isExplored() + " looted=" + n.isHasBeenLooted()
                    + " read=" + in.position() + "/" + in.limit() + " draws=" + draws());
                for (InventoryItem i : n.getItems()) out.println("LOADED " + p[2] + " " + show(i) + fields(i));
                out.println("CAGAIN " + p[2] + " " + bits(again));
            }
            case "recordedmedia" -> {
                // the media ids of a save (recorded_media.bin of the folder p[1]) become the indexes of the world's RecordedMedia, so
                // that a recorded-media index of the save names the media the game made it for (no output: the Rust side has no media)
                Object rm = zombie.radio.ZomboidRadio.getInstance().getRecordedMedia();
                Field idxf = rm.getClass().getDeclaredField("indexes");
                idxf.setAccessible(true);
                @SuppressWarnings("unchecked") ArrayList<String> idx = (ArrayList<String>) idxf.get(rm);
                ByteBuffer mb = ByteBuffer.wrap(Files.readAllBytes(Paths.get(new String(unhex(p[1]), java.nio.charset.StandardCharsets.UTF_8), "recorded_media.bin")));
                mb.getInt();
                ArrayList<String> ids = new ArrayList<>();
                for (int n = mb.getInt(); n > 0; n--) ids.add(zombie.GameWindow.ReadString(mb));
                for (String id : idx) if (!ids.contains(id)) ids.add(id);
                idx.clear();
                idx.addAll(ids);
            }
            case "rawitem" -> {
                // a block of a real save (the length, the registry id, the save type, the body): loaded, its state, saved again
                byte[] raw = unhex(p[2]);
                ByteBuffer in = ByteBuffer.wrap(raw);
                InventoryItem back = InventoryItem.loadItem(in, WORLD_VERSION);
                if (back == null) { out.println("RAWITEM " + p[1] + " null"); return; }
                String why = skip(back.getScriptItem());
                if (why != null || back.getFullType().equals("Base.CorpseAnimal") || back.isRecordedMedia()) { out.println("SKIPPED " + p[1]); return; }
                restoreName(back, origName(raw, 0, raw.length));
                ByteBuffer again = savedSized(back);
                boolean same = java.util.Arrays.equals(raw, java.util.Arrays.copyOf(again.array(), again.position()));
                out.println("RAWITEM " + p[1] + " " + show(back) + fields(back) + " read=" + in.position() + "/" + in.limit() + " same=" + (same ? 1 : 0)
                    + (same ? "" : " again=" + bits(again)));
            }
            case "rawcont" -> {
                // the bytes of an ItemContainer.save of a real save: loaded, every item's state, saved again
                byte[] raw = unhex(p[2]);
                ByteBuffer in = ByteBuffer.wrap(raw);
                ItemContainer n = new ItemContainer();
                ArrayList<InventoryItem> savedItems = n.load(in, WORLD_VERSION);
                for (InventoryItem i : n.getItems()) {
                    if (skip(i.getScriptItem()) != null || i.getFullType().equals("Base.CorpseAnimal") || i.isRecordedMedia()) { out.println("SKIPPED " + p[1]); return; }
                }
                // the groups of CompressIdenticalItems: the name each block holds goes back to the items it made
                int pos = 2 + (((raw[0] & 0xff) << 8) | (raw[1] & 0xff)) + 1;
                int groups = ((raw[pos] & 0xff) << 8) | (raw[pos + 1] & 0xff);
                pos += 2;
                for (int g = 0, k = 0; g < groups; g++) {
                    int identical = int32(raw, pos), len = int32(raw, pos + 4);
                    String nm = origName(raw, pos + 8, pos + 8 + len);
                    for (int j = 0; j < identical; j++) restoreName(savedItems.get(k + j), nm);
                    k += identical;
                    pos += 8 + len + (identical > 1 ? (identical - 1) * 4 : 0);
                }
                ByteBuffer again = ByteBuffer.allocate(1 << 20);
                n.save(again);
                boolean same = java.util.Arrays.equals(raw, java.util.Arrays.copyOf(again.array(), again.position()));
                out.println("RAWCONT " + p[1] + " type=" + n.getType() + " capacity=" + n.getCapacity() + " explored=" + n.isExplored() + " looted=" + n.isHasBeenLooted()
                    + " items=" + n.getItems().size() + " read=" + in.position() + "/" + in.limit() + " same=" + (same ? 1 : 0) + (same ? "" : " again=" + bits(again)));
                for (InventoryItem i : n.getItems()) out.println("LOADED " + p[1] + " " + show(i) + fields(i));
            }
            case "createsave" -> createsave(out);
            case "regs" -> {
                ArrayList<Item> all = new ArrayList<>(ScriptManager.instance.getAllItems());
                all.sort((x, y) -> x.getFullName().compareTo(y.getFullName()));
                for (Item it : all) out.println("REG " + it.getFullName() + " " + zombie.world.WorldDictionary.getItemRegistryID(it.getFullName()) + " " + it.getRegistry_id());
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
