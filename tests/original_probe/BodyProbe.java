import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.Random;
import java.util.TimeZone;
import zombie.GameTime;
import zombie.characters.BodyDamage.BodyDamage;
import zombie.characters.BodyDamage.BodyPart;
import zombie.characters.CharacterStat;
import zombie.characters.IsoGameCharacter;
import zombie.characters.IsoPlayer;
import zombie.core.random.RandAbstract;
import zombie.core.random.RandStandard;
import zombie.iso.CellLoader;
import zombie.iso.IsoCell;
import zombie.iso.IsoChunk;
import zombie.iso.IsoGridSquare;
import zombie.iso.IsoWorld;
import zombie.network.ServerMap;
import java.util.ArrayList;
import java.util.List;
import zombie.SandboxOptions;
import zombie.scripting.objects.CharacterTrait;
import zombie.scripting.objects.ResourceLocation;

// Exploration of the original zombie.characters.BodyDamage inside the booted server (ServerHarness): a headless IsoPlayer
// whose BodyDamage.Update and the stat updates are run for N ticks.
public class BodyProbe {
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

    static String f(float v) { return Integer.toHexString(Float.floatToRawIntBits(v)); }

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
            // the scenarios: the lines of the file a[1] (blank lines and '#' comments skipped)
            for (String line : java.nio.file.Files.readAllLines(java.nio.file.Path.of(a[1]))) {
                if (line.isBlank() || line.startsWith("#")) continue;
                scenario(out, line.trim(), cell);
            }
        });
    }

    static void setField(Object o, String name, Object v) throws Exception {
        Class<?> c = o.getClass();
        while (c != null) {
            try {
                Field f = c.getDeclaredField(name);
                f.setAccessible(true);
                f.set(o, v);
                return;
            } catch (NoSuchFieldException e) { c = c.getSuperclass(); }
        }
        throw new NoSuchFieldException(name);
    }

    // Per scenario "seed:ticks:run:every:show[:options]":
    //  - run: the updates one tick makes, joined by '+', in that order (nutrition, calc, moodles, body)
    //  - every: a TICK line is printed on ticks 1..3 and on every `every`-th tick; show: its sections joined by '+'
    //    (stats, nut, traits, draws)
    //  - options: name=value pairs separated by ',': mpd (GameTime minutesPerDay), mult (GameTime multiplier),
    //    sb.<Name> (a sandbox option), stat.<Name>, weight, cal, carb, lip, prot (Nutrition), trait (ids joined by '+'),
    //    moving, running, sprint, sneak, asleep, sit (0/1 flags)
    static void scenario(PrintStream out, String spec, IsoCell cell) throws Exception {
        String[] p = spec.split(":", 6);
        long seed = Long.parseLong(p[0]);
        int ticks = Integer.parseInt(p[1]);
        String[] run = p[2].split("\\+");
        int every = Integer.parseInt(p[3]);
        List<String> show = java.util.Arrays.asList(p[4].split("\\+"));
        java.util.Map<String, String> o = new java.util.LinkedHashMap<>();
        if (p.length > 5 && !p[5].isEmpty()) for (String kv : p[5].split(",")) o.put(kv.substring(0, kv.indexOf('=')), kv.substring(kv.indexOf('=') + 1));
        out.println("SCENARIO " + spec);
        IsoPlayer pl = new IsoPlayer(cell, null, 8280, 11670, 0);
        IsoGridSquare sq = cell.getGridSquare(8280, 11670, 0);
        pl.setCurrent(sq);
        pl.setX(8280.5f);
        pl.setY(11670.5f);
        pl.setZ(0);
        // the world: the clock multiplier and the day length, the sandbox options of the scenario
        GameTime gt = GameTime.instance;
        float oldMult = gt.getTrueMultiplier();
        float oldMpd = gt.getMinutesPerDay();
        gt.setMultiplier(Float.parseFloat(o.getOrDefault("mult", "1.0")));
        setField(gt, "minutesPerDay", Float.parseFloat(o.getOrDefault("mpd", "30.0")));
        List<Runnable> restore = new ArrayList<>();
        restore.add(() -> { gt.setMultiplier(oldMult); try { setField(gt, "minutesPerDay", oldMpd); } catch (Exception e) { throw new RuntimeException(e); } });
        SandboxOptions so = SandboxOptions.instance;
        for (java.util.Map.Entry<String, String> e : o.entrySet()) {
            if (e.getKey().startsWith("sb.")) {
                String name = e.getKey().substring(3);
                Object old = so.getOptionByName(name).asConfigOption().getValueAsObject();
                so.set(name, Double.valueOf(e.getValue()));
                restore.add(() -> so.set(name, old));
            }
        }
        for (java.util.Map.Entry<String, String> e : o.entrySet()) {
            String k = e.getKey(), v = e.getValue();
            if (k.startsWith("stat.")) pl.getStats().set(CharacterStat.getById(k.substring(5)), Float.parseFloat(v));
            else if (k.equals("weight")) pl.getNutrition().setWeight(Double.parseDouble(v));
            else if (k.equals("cal")) pl.getNutrition().setCalories(Float.parseFloat(v));
            else if (k.equals("carb")) pl.getNutrition().setCarbohydrates(Float.parseFloat(v));
            else if (k.equals("lip")) pl.getNutrition().setLipids(Float.parseFloat(v));
            else if (k.equals("prot")) pl.getNutrition().setProteins(Float.parseFloat(v));
            else if (k.equals("trait")) for (String t : v.split("\\+")) pl.getCharacterTraits().set(CharacterTrait.get(ResourceLocation.of(t)), true);
            else if (k.equals("moving")) setField(pl, "isPlayerMoving", v.equals("1"));
            else if (k.equals("running")) pl.setRunning(v.equals("1"));
            else if (k.equals("sprint")) pl.setSprinting(v.equals("1"));
            else if (k.equals("sneak")) pl.setSneaking(v.equals("1"));
            else if (k.equals("asleep")) pl.setAsleep(v.equals("1"));
            else if (k.equals("sit")) pl.setSitOnGround(v.equals("1"));
        }
        set(RandStandard.INSTANCE, RandAbstract.class, "rand", new Random(seed));
        set(RandStandard.INSTANCE, RandAbstract.class, "id", 0);
        out.println("ENV mult=" + f(gt.getMultiplier()) + " delta=" + f(gt.getTimeDelta()) + " gwsslu=" + f(gt.getGameWorldSecondsSinceLastUpdate())
            + " mpd=" + f(gt.getMinutesPerDay()) + " dmpd=" + f(gt.getDeltaMinutesPerDay()));
        BodyDamage bd = pl.getBodyDamage();
        Method calc = IsoGameCharacter.class.getDeclaredMethod("calculateStats");
        calc.setAccessible(true);
        for (int n = 1; n <= ticks; n++) {
            for (String r : run) {
                switch (r) {
                    case "nutrition" -> pl.getNutrition().update();
                    case "calc" -> calc.invoke(pl);
                    case "moodles" -> pl.getMoodles().Update();
                    case "body" -> bd.Update();
                    default -> throw new IllegalArgumentException("run " + r);
                }
            }
            if (n <= 3 || n % every == 0 || n == ticks) {
                StringBuilder sb = new StringBuilder("TICK " + n);
                for (String sec : show) {
                    switch (sec) {
                        case "stats" -> {
                            sb.append(" stats");
                            for (CharacterStat st : CharacterStat.ORDERED_STATS) sb.append(' ').append(f(pl.getStats().get(st)));
                        }
                        case "nut" -> {
                            zombie.characters.BodyDamage.Nutrition nu = pl.getNutrition();
                            sb.append(" nut ").append(f(nu.getCalories())).append(' ').append(f(nu.getProteins())).append(' ').append(f(nu.getLipids()))
                                .append(' ').append(f(nu.getCarbohydrates())).append(' ').append(Long.toHexString(Double.doubleToRawLongBits(nu.getWeight())))
                                .append(' ').append(nu.isIncWeight() ? 1 : 0).append(nu.isIncWeightLot() ? 1 : 0).append(nu.isDecWeight() ? 1 : 0)
                                .append(' ').append(g(nu, "updatedWeight")).append(' ').append(f((Float) g(nu, "caloriesMax"))).append(' ').append(f((Float) g(nu, "caloriesMin")));
                        }
                        case "traits" -> {
                            sb.append(" traits");
                            for (CharacterTrait t : pl.getCharacterTraits().getKnownTraits()) sb.append(' ').append(t);
                        }
                        case "draws" -> sb.append(" draws=").append(g(RandStandard.INSTANCE, "id"));
                        default -> throw new IllegalArgumentException("show " + sec);
                    }
                }
                out.println(sb);
            }
        }
        for (Runnable r : restore) r.run();
    }
}
