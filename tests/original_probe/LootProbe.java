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

    public static void main(String[] a) throws Exception {
        out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, "UTF-8");
        System.setOut(System.err);
        LuaGameProbe.init(new String[]{a[0], "shared", "server"});
        parse();
        tables();
        out.flush();
        Runtime.getRuntime().halt(0);
    }
}
