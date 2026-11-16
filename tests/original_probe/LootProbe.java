import java.io.PrintStream;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.TreeSet;
import zombie.Lua.LuaEventManager;
import zombie.inventory.ItemPickerJava;
import zombie.inventory.ItemPickerJava.ItemPickerContainer;
import zombie.inventory.ItemPickerJava.ItemPickerItem;
import zombie.inventory.ItemPickerJava.ItemPickerRoom;
import zombie.inventory.ItemPickerJava.ProceduralItem;
import zombie.inventory.ItemPickerJava.VehicleDistribution;
import zombie.scripting.ScriptManager;
import zombie.scripting.objects.Item;

/** Oracle probe (tools/oracle.sh loot): the ORIGINAL ItemPickerJava on the distributions the game's own Lua defines.
 * The setup is LuaGameProbe's (the client's startup up to LoadDirBase, headless: item scripts, the Lua state) with
 * LoadDirBase of shared and server (the Lua the dedicated server loads: the Distributions tables), then
 * IsoWorld.init's order: OnPreDistributionMerge, OnDistributionMerge, OnPostDistributionMerge, ItemPickerJava.Parse(),
 * InitSandboxLootSettings(). Printed (floats as raw bits, strings escaped, tables sorted by key as Java Strings
 * compare; ArrayLists, arrays and the Lua-ordered lists in their own order):
 *   LOOTMOD <name> <bits>          the loot modifiers InitSandboxLootSettings left (and zombieDensityCap)
 *   NOFILL <n> <names...>          NoContainerFillRooms
 *   UPGRADE <name> <n> <types...>  WeaponUpgrades (in list order), then WEAPONUPGRADEMAP <n> <sorted names>
 *   ROOM <key> <fields>            each room of `rooms` (Suburbs distributions), then its containers: RC <key> <name>
 *   CONTAINER <key>                `containers` (the bag and zombie-outfit containers with their own rolls)
 *   PROCDIST <key>                 ProceduralDistributions
 *   VEHDIST <key> / VEHNORMAL / VEHSPECIFIC   VehicleDistributions
 *   ... each ItemPickerContainer prints a line of its flags, then `items`, `proc` (the procedural list), `junk` and
 *       `bags` (nested, one deeper)
 *   CANSPAWN <n>                   the item scripts Parse marked canSpawnAsLoot, then CANSPAWNITEM <full name> each
 * Args: <scratch cache dir> <scenario>... ; run from the game root. */
public class LootProbe {
    static PrintStream out;

    static String esc(String s) {
        if (s == null) return "nil";
        StringBuilder b = new StringBuilder();
        LuaLoadEnvProbe.esc(b, s);
        return b.toString();
    }

    static String fbits(float f) {
        return Integer.toHexString(Float.floatToRawIntBits(f));
    }

    static String list(List<String> l) {
        if (l == null) return "nil";
        StringBuilder b = new StringBuilder("[");
        for (int i = 0; i < l.size(); i++) b.append(i == 0 ? "" : ",").append(esc(l.get(i)));
        return b.append(']').toString();
    }

    static String flag(boolean b) {
        return b ? "1" : "0";
    }

    static void container(String prefix, String label, ItemPickerContainer c) {
        if (c == null) {
            out.println(prefix + label + " nil");
            return;
        }
        out.println(prefix + label + " rolls=" + fbits(c.rolls) + " flags=" + flag(c.noAutoAge) + flag(c.isShop) + flag(c.procedural)
            + flag(c.dontSpawnAmmo) + flag(c.gunStorage) + flag(c.ignoreZombieDensity) + flag(c.cookFood) + flag(c.canBurn) + flag(c.isTrash)
            + flag(c.isWorn) + flag(c.isRotten) + flag(c.onlyOne) + flag(c.defaultInventoryLoot) + " fillRand=" + c.fillRand + " maxMap=" + c.maxMap
            + " stashChance=" + c.stashChance + " items=" + c.items.length);
        StringBuilder b = new StringBuilder(prefix + " ITEMS");
        for (ItemPickerItem i : c.items) b.append(' ').append(esc(i.itemName)).append('=').append(fbits(i.chance));
        out.println(b);
        if (c.proceduralItems == null) out.println(prefix + " PROC nil");
        else {
            out.println(prefix + " PROC " + c.proceduralItems.size());
            for (ProceduralItem p : c.proceduralItems)
                out.println(prefix + "  " + esc(p.name) + " min=" + p.min + " max=" + p.max + " weight=" + p.weightChance + " items=" + list(p.forceForItems)
                    + " zones=" + list(p.forceForZones) + " tiles=" + list(p.forceForTiles) + " rooms=" + list(p.forceForRooms));
        }
        if (c.junk != null) container(prefix + " ", "JUNK", c.junk);
        if (c.bags != null) container(prefix + " ", "BAGS", c.bags);
    }

    static void room(String label, ItemPickerRoom r) {
        out.println(label + " fillRand=" + r.fillRand + " isShop=" + flag(r.isShop) + " specificId=" + esc(r.specificId) + " professionChance=" + r.professionChance
            + " professionChanceInt=" + r.professionChanceInt + " outfit=" + esc(r.outfit) + " outfitFemale=" + esc(r.outfitFemale) + " outfitMale=" + esc(r.outfitMale)
            + " outfitChance=" + esc(r.outfitChance) + " vehicle=" + esc(r.vehicle) + " vehicles=" + list(r.vehicles) + " vehicleChance=" + esc(r.vehicleChance)
            + " vehicleDistribution=" + esc(r.vehicleDistribution) + " vehicleSkin=" + r.vehicleSkin + " femaleChance=" + esc(r.femaleChance)
            + " roomTypes=" + esc(r.roomTypes) + " zoneRequires=" + esc(r.zoneRequires) + " zoneDisallows=" + esc(r.zoneDisallows)
            + " containerChance=" + esc(r.containerChance) + " femaleOdds=" + esc(r.femaleOdds) + " bagType=" + esc(r.bagType) + " bagTable=" + esc(r.bagTable)
            + " containers=" + r.containers.size());
        for (Map.Entry<String, ItemPickerContainer> e : new TreeMap<>(r.containers).entrySet()) container(" ", "RC " + esc(e.getKey()), e.getValue());
    }

    /** The tables Parse and InitSandboxLootSettings left. */
    static void tables() throws Exception {
        for (String n : new String[]{"otherLootModifier", "foodLootModifier", "cannedFoodLootModifier", "weaponLootModifier", "rangedWeaponLootModifier",
            "ammoLootModifier", "literatureLootModifier", "survivalGearsLootModifier", "medicalLootModifier", "bagLootModifier", "mechanicsLootModifier",
            "clothingLootModifier", "containerLootModifier", "keyLootModifier", "keyLootModifierD100", "mediaLootModifier", "mementoLootModifier",
            "cookwareLootModifier", "materialLootModifier", "farmingLootModifier", "toolLootModifier", "skillBookLootModifier", "recipeResourceLootModifier"})
            out.println("LOOTMOD " + n + " " + fbits((Float) LuaLoadEnvProbe.field(null, ItemPickerJava.class, n)));
        out.println("LOOTMOD zombieDensityCap " + fbits(ItemPickerJava.zombieDensityCap));
        StringBuilder b = new StringBuilder("NOFILL " + ItemPickerJava.NoContainerFillRooms.size());
        for (String s : ItemPickerJava.NoContainerFillRooms) b.append(' ').append(esc(s));
        out.println(b);
        for (ItemPickerJava.ItemPickerUpgradeWeapons w : ItemPickerJava.WeaponUpgrades)
            out.println("UPGRADE " + esc(w.name) + " " + w.upgrades.size() + " " + list(w.upgrades));
        out.println("WEAPONUPGRADEMAP " + ItemPickerJava.WeaponUpgradeMap.size() + " " + list(new ArrayList<>(new TreeSet<>(ItemPickerJava.WeaponUpgradeMap.keySet()))));
        out.println("ADDEDINVALID " + ((List<?>) LuaLoadEnvProbe.field(null, ItemPickerJava.class, "addedInvalidAlready")).size());
        out.println("ROOMS " + ItemPickerJava.rooms.size());
        for (Map.Entry<String, ItemPickerRoom> e : new TreeMap<>(ItemPickerJava.rooms).entrySet()) room("ROOM " + esc(e.getKey()), e.getValue());
        out.println("CONTAINERS " + ItemPickerJava.containers.size());
        for (Map.Entry<String, ItemPickerContainer> e : new TreeMap<>(ItemPickerJava.containers).entrySet()) container("", "CONTAINER " + esc(e.getKey()), e.getValue());
        out.println("PROCDISTS " + ItemPickerJava.ProceduralDistributions.size());
        for (Map.Entry<String, ItemPickerContainer> e : new TreeMap<>(ItemPickerJava.ProceduralDistributions).entrySet())
            container("", "PROCDIST " + esc(e.getKey()), e.getValue());
        out.println("VEHDISTS " + ItemPickerJava.VehicleDistributions.size());
        for (Map.Entry<String, VehicleDistribution> e : new TreeMap<>(ItemPickerJava.VehicleDistributions).entrySet()) {
            VehicleDistribution v = e.getValue();
            out.println("VEHDIST " + esc(e.getKey()) + " specific=" + v.specific.size());
            if (v.normal != null) room(" VEHNORMAL", v.normal);
            for (int i = 0; i < v.specific.size(); i++) room(" VEHSPECIFIC " + i, v.specific.get(i));
        }
        TreeSet<String> can = new TreeSet<>();
        for (Item i : ScriptManager.instance.getAllItems()) if (i.canSpawnAsLoot) can.add(i.getFullName());
        out.println("CANSPAWN " + can.size());
        for (String s : can) out.println("CANSPAWNITEM " + s);
    }

    /** IsoWorld.init: the distribution merge events, then Parse and InitSandboxLootSettings. */
    static void parse() {
        LuaEventManager.triggerEvent("OnPreDistributionMerge");
        LuaEventManager.triggerEvent("OnDistributionMerge");
        LuaEventManager.triggerEvent("OnPostDistributionMerge");
        ItemPickerJava.Parse();
        ItemPickerJava.InitSandboxLootSettings();
    }

    /** Draw sites seen while rolling (loot.trace=1): the first frames outside java.util / zombie.core.random of each draw. */
    static final TreeMap<String, Integer> sites = new TreeMap<>();

    /** A java.util.Random that counts nothing itself (RandAbstract.id does) but, with loot.trace, notes who drew. */
    static class TracedRandom extends java.util.Random {
        TracedRandom(long seed) { super(seed); }
        void note() {
            if (!Boolean.getBoolean("loot.trace")) return;
            StringBuilder b = new StringBuilder();
            int n = 0;
            for (StackTraceElement e : new Throwable().getStackTrace()) {
                String c = e.getClassName();
                if (c.startsWith("java.") || c.startsWith("zombie.core.random.") || c.equals("LootProbe$TracedRandom")) continue;
                b.append(n++ == 0 ? "" : " < ").append(c).append('.').append(e.getMethodName()).append(':').append(e.getLineNumber());
                if (n == 3) break;
            }
            sites.merge(b.toString(), 1, Integer::sum);
        }
        @Override public int nextInt(int bound) { note(); return super.nextInt(bound); }
        @Override public float nextFloat() { note(); return super.nextFloat(); }
        @Override public int nextInt() { note(); return super.nextInt(); }
    }

    static int draws() throws Exception {
        return (Integer) LuaLoadEnvProbe.field(zombie.core.random.RandStandard.INSTANCE, zombie.core.random.RandAbstract.class, "id");
    }

    static void swapRand(long seed) throws Exception {
        java.lang.reflect.Field rf = zombie.core.random.RandAbstract.class.getDeclaredField("rand");
        rf.setAccessible(true);
        rf.set(zombie.core.random.RandStandard.INSTANCE, new TracedRandom(seed));
        java.lang.reflect.Field idf = zombie.core.random.RandAbstract.class.getDeclaredField("id");
        idf.setAccessible(true);
        idf.set(zombie.core.random.RandStandard.INSTANCE, 0);
    }

    /** The room distribution a scenario names: a SuburbsDistributions room, or `veh/<VehicleDistributions key>` (its
     * normal distribution). */
    static ItemPickerRoom roomDist(String room) {
        if (room.startsWith("veh/")) return ItemPickerJava.VehicleDistributions.get(room.substring(4)).normal;
        return ItemPickerJava.rooms.get(room);
    }

    /** The ITEM lines of a container's items (a bag's contents follow it, path `n.m`). */
    static void itemLines(String prefix, zombie.inventory.ItemContainer c) {
        int n = 0;
        for (zombie.inventory.InventoryItem i : c.getItems()) {
            String path = prefix.isEmpty() ? String.valueOf(n) : prefix + "." + n;
            n++;
            StringBuilder b = new StringBuilder("ITEM " + path + " " + i.getFullType() + " id=" + i.id + " uses=" + i.getCurrentUses() + " cond=" + i.getCondition()
                    + " weight=" + fbits(i.getActualWeight()) + " sharp=" + fbits(i.getSharpness()));
            if (i instanceof zombie.inventory.types.InventoryContainer ic) {
                b.append(" cap=" + ic.getCapacity() + " icap=" + ic.getItemContainer().getCapacity() + " red=" + ic.getWeightReduction()
                        + " nin=" + ic.getItemContainer().getItems().size() + " cw=" + fbits(ic.getContentsWeight()) + " uw=" + fbits(ic.getUnequippedWeight())
                        + " exp=" + (ic.getItemContainer().isExplored() ? 1 : 0));
            }
            out.println(b);
            if (i instanceof zombie.inventory.types.InventoryContainer ic) itemLines(path, ic.getItemContainer());
        }
    }

    /** One scenario `seed:room:container:times[:capacity[:mode[:building]]]`: `times` containers of that type, each
     * filled by ItemPickerJava.fillContainer from one seeded generator. The container sits on a square (an IsoGridSquare
     * made without a cell) in an IsoRoom named `room` of an IsoBuilding with the rooms `building` (`+` separated, default
     * just `room`); `mode` is `same` (every container in one room: its RoomDef's procedural counts persist) or `fresh`
     * (a new room each time). No player and no map exist headless. Printed per container the items in order with the
     * draws it took, then the generator's draw counter. `veh/<key>` as room fills from that vehicle distribution's
     * normal room with fillContainerType (no square). */
    static void scenario(String spec) throws Exception {
        String[] p = spec.split(":");
        long seed = Long.parseLong(p[0]);
        String room = p[1], type = p[2];
        int times = Integer.parseInt(p[3]);
        int capacity = p.length > 4 ? Integer.parseInt(p[4]) : 50;
        boolean same = p.length > 5 && p[5].equals("same");
        String[] bld = p.length > 6 ? p[6].split("\\+") : new String[]{room};
        out.println("SCENARIO " + spec);
        swapRand(seed);
        zombie.iso.RoomDef def = null;
        zombie.iso.areas.IsoRoom isoRoom = null;
        zombie.iso.IsoGridSquare sq = null;
        for (int t = 0; t < times; t++) {
            zombie.inventory.ItemContainer c;
            if (room.startsWith("veh/")) {
                c = new zombie.inventory.ItemContainer(type, null, null);
            } else {
                if (sq == null || !same) {
                    // IsoBuilding's constructor draws (a probe artifact: the game builds them outside any loot roll), so it
                    // runs on a scratch generator and the seeded one is put back untouched
                    Object keep = LuaLoadEnvProbe.field(zombie.core.random.RandStandard.INSTANCE, zombie.core.random.RandAbstract.class, "rand");
                    int keepId = draws();
                    swapRand(0);
                    zombie.iso.areas.IsoBuilding building = new zombie.iso.areas.IsoBuilding();
                    isoRoom = null;
                    for (String name : bld) {
                        zombie.iso.areas.IsoRoom r = new zombie.iso.areas.IsoRoom();
                        r.roomDef = name;
                        r.def = new zombie.iso.RoomDef(building.rooms.size(), name);
                        r.building = building;
                        building.rooms.add(r);
                        if (isoRoom == null && name.equals(room)) isoRoom = r;
                    }
                    sq = new zombie.iso.IsoGridSquare(null, null, 100 + t, 200, 0);
                    sq.setRoom(isoRoom);
                    LuaLoadEnvProbe.setField(sq, zombie.iso.IsoGridSquare.class, "roomId", isoRoom.def.getID() + 1L);
                    LuaLoadEnvProbe.setField(zombie.core.random.RandStandard.INSTANCE, zombie.core.random.RandAbstract.class, "rand", keep);
                    LuaLoadEnvProbe.setField(zombie.core.random.RandStandard.INSTANCE, zombie.core.random.RandAbstract.class, "id", keepId);
                }
                c = new zombie.inventory.ItemContainer(type, sq, null);
            }
            c.setCapacity(capacity);
            int before = draws();
            String err = "";
            try {
                if (room.startsWith("veh/")) ItemPickerJava.fillContainerType(roomDist(room), c, "all", null);
                else ItemPickerJava.fillContainer(c, null);
            } catch (Throwable e) {
                err = " EXC " + e.getClass().getName();
                if (Boolean.getBoolean("loot.trace")) {
                    StringBuilder tb = new StringBuilder("EXCTRACE " + e);
                    for (int k = 0; k < 6 && k < e.getStackTrace().length; k++) tb.append(" < ").append(e.getStackTrace()[k]);
                    System.err.println(tb);
                }
            }
            StringBuilder b = new StringBuilder("FILL " + t + " draws=" + ((draws() - before + 10000) % 10000) + " n=" + c.getItems().size() + err);
            for (zombie.inventory.InventoryItem i : c.getItems()) b.append(' ').append(i.getFullType());
            out.println(b);
            itemLines("", c);
        }
        out.println("END " + spec + " rand=" + draws());
    }

    public static void main(String[] a) throws Exception {
        out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, "UTF-8");
        System.setOut(System.err);
        LuaGameProbe.init(new String[]{a[0], "shared", "server"});
        parse();
        tables();
        // IsoWorld.init: StashSystem.init() (the stash descriptions the Lua defined; possibleStashes starts with every stash building)
        zombie.core.stash.StashSystem.init();
        // a cell with no map: IsoWorld.instance.currentCell (AddItem adds each item to its process list; a car key
        // looks for a vehicle in its vehicle set, which is empty)
        try { new zombie.iso.IsoCell(300, 300); System.err.println("LOOT cell " + (zombie.iso.IsoWorld.instance.currentCell != null)); }
        catch (Throwable e) { System.err.println("LOOT cell FAILED " + e); e.printStackTrace(); }
        { zombie.SandboxOptions so = zombie.SandboxOptions.instance; System.err.println("LOOT gametime tod=" + zombie.GameTime.getInstance().getTimeOfDay() + " nights=" + zombie.GameTime.getInstance().getNightsSurvived() + " ageDays=" + zombie.GameTime.getInstance().getWorldAgeDaysSinceBegin() + " timeSinceApo=" + so.getTimeSinceApo()); System.err.println("LOOT sandbox MaximumDiminishedLoot=" + so.maximumDiminishedLoot.getValue() + " DaysUntil=" + so.daysUntilMaximumDiminishedLoot.getValue() + " Zombies=" + so.zombies.getValue() + " AnnotatedMapChance=" + so.annotatedMapChance.getValue() + " ZombiePopLootEffect=" + so.zombiePopLootEffect.getValue() + " RollsMultiplier=" + so.rollsMultiplier.getValue() + " reloadDifficulty=" + zombie.core.Core.getInstance().getOptionReloadDifficulty()); }
        if (Boolean.getBoolean("loot.trace")) {
            zombie.inventory.ItemContainer dc = new zombie.inventory.ItemContainer("fridge", null, null);
            int n = 0;
            for (ItemPickerJava.ItemPickerItem it : ItemPickerJava.ProceduralDistributions.get("FridgeGeneric").items) {
                if (n++ < 12) System.err.println("CHANCE " + it.itemName + " " + it.chance + " -> " + ItemPickerJava.getActualSpawnChance(it, null, dc, 0.0F, false) + " mod=" + ItemPickerJava.getLootModifier(it.itemName));
            }
        }
        for (int i = 1; i < a.length; i++) scenario(a[i]);
        for (Map.Entry<String, Integer> e : sites.entrySet()) System.err.println("SITE " + e.getValue() + " " + e.getKey());
        out.flush();
        Runtime.getRuntime().halt(0);
    }
}
