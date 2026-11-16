import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Random;
import java.util.TimeZone;
import zombie.GameTime;
import zombie.SandboxOptions;
import zombie.characters.IsoZombie;
import zombie.characters.SurvivorDesc;
import zombie.characters.SurvivorFactory;
import zombie.core.random.RandAbstract;
import zombie.core.random.RandStandard;
import zombie.core.skinnedmodel.population.OutfitRNG;
import zombie.iso.CellLoader;
import zombie.iso.IsoCell;
import zombie.iso.IsoChunk;
import zombie.iso.IsoGridSquare;
import zombie.iso.IsoWorld;
import zombie.network.ServerMap;

// Inside the booted original dedicated server (ServerHarness), with the JVM's default zone pinned to UTC: the ORIGINAL
// zombie.characters.IsoZombie constructed (and, with ticks > 0, updated) headless over real loaded squares.
//
// Per scenario "seed:count:ticks[:options]":
//  - the global Rand (RandStandard) is swapped for java.util.Random(seed) with its draw counter (RandAbstract.id) at 0,
//    OutfitRNG is seeded with the same seed, and the lore sandbox options named in options (Key=value pairs separated
//    by commas, Key the option's name without the "ZombieLore." group, e.g. Speed=4) are set (and put back after);
//  - `count` zombies are made one after another with `new IsoZombie(IsoWorld.instance.currentCell)`; after each one
//    its state is printed: the draw counter, the descriptor (names, gender, torso, voice, the eight stats), health,
//    weight, patience, forward direction, palette and sprite name, tints, speed/strength/cognition/memory/sight/hearing,
//    walk type and variant, speed modifier, the flags and the human visual's blood.
// The chunks 1034..1035 x 1458..1459 of the real map are loaded as the erosion probe does so that a square exists.
// Args: <scratch dir> <scenario>...
public class ZombieProbe {
    static Object g(Object o, String name) throws Exception {
        Class<?> c = o instanceof Class<?> k ? k : o.getClass();
        Object target = o instanceof Class<?> ? null : o;
        while (c != null) {
            try {
                Field f = c.getDeclaredField(name);
                f.setAccessible(true);
                return f.get(target);
            } catch (NoSuchFieldException e) { c = c.getSuperclass(); }
        }
        throw new NoSuchFieldException(name);
    }

    static void set(Object o, Class<?> c, String name, Object v) throws Exception {
        Field f = c.getDeclaredField(name);
        f.setAccessible(true);
        f.set(o, v);
    }

    static String f(Object v) { return Float.toString((Float) v); }
    static int b(Object v) { return (Boolean) v ? 1 : 0; }

    static void load(IsoCell cell) throws Exception {
        Method gp = IsoChunk.class.getDeclaredMethod("getFromPool");
        gp.setAccessible(true);
        ServerMap sm = ServerMap.instance;
        ServerMap.ServerCell[] cellMap = (ServerMap.ServerCell[]) g(sm, "cellMap");
        int width = (Integer) g(sm, "width");
        ServerMap.ServerCell sc = new ServerMap.ServerCell();
        int cx = 1034 / 8, cy = 1458 / 8;
        sc.wx = cx;
        sc.wy = cy;
        cellMap[(cy - sm.getMinY()) * width + (cx - sm.getMinX())] = sc;
        sm.loadedCells.add(sc);
        for (int wx = 1034; wx <= 1035; wx++) {
            for (int wy = 1458; wy <= 1459; wy++) {
                IsoChunk ch = (IsoChunk) gp.invoke(null);
                ch.wx = wx;
                ch.wy = wy;
                CellLoader.LoadCellBinaryChunk(cell, wx, wy, ch);
                ch.loadInWorldStreamerThread();
                sc.chunks[wx - cx * 8][wy - cy * 8] = ch;
                ch.loaded = true;
            }
        }
        sc.isLoaded = true;
    }

    public static void main(String[] a) throws Exception {
        TimeZone.setDefault(TimeZone.getTimeZone("UTC"));
        ServerHarness.run(a[0], out -> {
            IsoCell cell = IsoWorld.instance.currentCell;
            load(cell);
            out.println("NAMES female=" + SurvivorFactory.FemaleForenames.size() + " male=" + SurvivorFactory.MaleForenames.size()
                + " surnames=" + SurvivorFactory.Surnames.size());
            for (ArrayListView v : new ArrayListView[]{
                new ArrayListView("female", SurvivorFactory.FemaleForenames), new ArrayListView("male", SurvivorFactory.MaleForenames),
                new ArrayListView("surname", SurvivorFactory.Surnames)}) {
                int n = v.list.size();
                out.println("NAME " + v.name + " 0=" + v.list.get(0) + " " + (n / 2) + "=" + v.list.get(n / 2) + " " + (n - 1) + "=" + v.list.get(n - 1));
            }
            for (int i = 1; i < a.length; i++) scenario(out, a[i], cell);
        });
    }

    record ArrayListView(String name, ArrayList<String> list) {}

    static final Map<String, Integer> BASE = new java.util.LinkedHashMap<>();
    static {
        BASE.put("Speed", 2);
        BASE.put("SprinterPercentage", 33);
        BASE.put("Strength", 2);
        BASE.put("Toughness", 2);
        BASE.put("Cognition", 3);
        BASE.put("DoorOpeningPercentage", 33);
        BASE.put("CrawlUnderVehicle", 5);
        BASE.put("Memory", 2);
        BASE.put("Sight", 2);
        BASE.put("Hearing", 2);
        BASE.put("ActiveOnly", 1);
        BASE.put("DisableFakeDead", 1);
    }

    static void setOption(Object opt, int v) throws Exception {
        for (Method m : opt.getClass().getMethods()) {
            if (m.getName().equals("setValue") && m.getParameterCount() == 1 && m.getParameterTypes()[0] == int.class) {
                m.invoke(opt, v);
                return;
            }
        }
        throw new NoSuchMethodException("setValue(int) on " + opt.getClass());
    }

    static void scenario(PrintStream out, String spec, IsoCell cell) throws Exception {
        String[] p = spec.split(":");
        long seed = Long.parseLong(p[0]);
        int count = Integer.parseInt(p[1]);
        int ticks = Integer.parseInt(p[2]);
        // the lore options are set to a fixed base first (the booted server's own preset is not what a new
        // SandboxOptions holds), then the scenario's overrides
        Map<String, Integer> want = new java.util.LinkedHashMap<>(BASE);
        if (p.length > 3 && !p[3].isEmpty()) {
            for (String kv : p[3].split(",")) {
                String[] e = kv.split("=", 2);
                want.put(e[0], Integer.parseInt(e[1]));
            }
        }
        Map<String, Integer> saved = new java.util.LinkedHashMap<>();
        for (Map.Entry<String, Integer> e : want.entrySet()) {
            Object opt = g(SandboxOptions.instance.lore, Character.toLowerCase(e.getKey().charAt(0)) + e.getKey().substring(1));
            saved.put(e.getKey(), (Integer) opt.getClass().getMethod("getValue").invoke(opt));
            setOption(opt, e.getValue());
        }
        out.println("SCENARIO " + spec);
        StringBuilder lore = new StringBuilder("LORE");
        for (String k : new String[]{"speed", "sprinterPercentage", "strength", "toughness", "cognition", "doorOpeningPercentage", "crawlUnderVehicle", "memory", "sight", "hearing", "activeOnly", "disableFakeDead"}) {
            Object opt = g(SandboxOptions.instance.lore, k);
            lore.append(' ').append(k).append('=').append(opt.getClass().getMethod("getValue").invoke(opt));
        }
        out.println(lore);
        set(RandStandard.INSTANCE, RandAbstract.class, "rand", new Random(seed));
        set(RandStandard.INSTANCE, RandAbstract.class, "id", 0);
        OutfitRNG.setSeed(seed);
        int firstId = -1;
        for (int i = 0; i < count; i++) {
            IsoZombie z = new IsoZombie(cell);
            SurvivorDesc d = z.getDescriptor();
            if (firstId < 0) firstId = d.getID();
            out.println("ZOMBIE " + i + " draws=" + g(RandStandard.INSTANCE, "id") + " outfitSeed=" + OutfitRNG.getSeed());
            dump(out, z, d, firstId);
        }
        for (Map.Entry<String, Integer> e : saved.entrySet()) {
            setOption(g(SandboxOptions.instance.lore, Character.toLowerCase(e.getKey().charAt(0)) + e.getKey().substring(1)), e.getValue());
        }
    }

    static void dump(PrintStream out, IsoZombie z, SurvivorDesc d, int firstId) throws Exception {
        out.println(" desc id+" + (d.getID() - firstId) + " forename=" + d.getForename() + " surname=" + d.getSurname() + " female=" + b(d.isFemale())
            + " torso=" + d.getTorso() + " voice=" + d.getVoicePrefix() + " type=" + g(d, "type"));
        out.println(" stats bravery=" + f(g(d, "bravery")) + " aggressiveness=" + f(g(d, "aggressiveness")) + " compassion=" + f(g(d, "compassion"))
            + " loner=" + f(g(d, "loner")) + " temper=" + f(g(d, "temper")) + " friendliness=" + f(g(d, "friendliness"))
            + " favourindoors=" + f(g(d, "favourindoors")) + " loyalty=" + f(g(d, "loyalty")));
        out.println(" body health=" + f(g(z, "health")) + " weight=" + f(g(z, "weight")) + " width=" + f(g(z, "width")) + " patience=" + g(z, "patience")
            + " dir=" + z.getForwardIsoDirection() + " fwd=" + f(z.getForwardDirectionX()) + "," + f(z.getForwardDirectionY()));
        Object def = g(g(z, "sprite"), "def");
        out.println(" look palette=" + g(z, "palette") + " sprite=" + g(z, "spriteName") + " tint=" + f(g(def, "tintr")) + "," + f(g(def, "tintg")) + "," + f(g(def, "tintb"))
            + " zombieId=" + g(z, "zombieId") + " hurt=" + g(z, "hurtSound"));
        out.println(" stats2 speedType=" + g(z, "speedType") + " strength=" + g(z, "strength") + " cognition=" + g(z, "cognition") + " memory=" + g(z, "memory")
            + " sight=" + g(z, "sight") + " hearing=" + g(z, "hearing") + " walkType=" + g(z, "walkType") + " walkVariant=" + g(z, "walkVariant")
            + " speedMod=" + f(g(z, "speedMod")) + " lunger=" + b(g(z, "lunger")) + " running=" + b(g(z, "running")) + " crawling=" + b(g(z, "crawling"))
            + " inactive=" + b(g(z, "inactive")) + " canCrawlUnder=" + b(g(z, "canCrawlUnderVehicle")) + " turnDelta=" + f(g(z, "turnDeltaNormal")));
        byte[] blood = (byte[]) g(g(z, "humanVisual"), "blood");
        StringBuilder sb = new StringBuilder(" blood");
        for (byte x : blood) sb.append(' ').append(x & 255);
        out.println(sb);
    }
}
