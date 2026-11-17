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
import zombie.iso.weather.ClimateManager;
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

    static String d(double v) { return Long.toHexString(Double.doubleToRawLongBits(v)); }

    // the value of a (private) field as the probe prints it: floats and doubles as hex bits, booleans as 0/1
    static String fv(Object o, String name) throws Exception {
        Object v = g(o, name);
        if (v instanceof Float x) return f(x);
        if (v instanceof Double x) return d(x);
        if (v instanceof Boolean x) return x ? "1" : "0";
        return String.valueOf(v);
    }

    // sets a (private) field of a scenario object from text; the type of the field says how the text is read
    static void setTyped(Object o, String name, String v) throws Exception {
        Class<?> c = o.getClass();
        while (c != null) {
            try {
                Field fd = c.getDeclaredField(name);
                fd.setAccessible(true);
                Class<?> t = fd.getType();
                if (t == float.class) fd.setFloat(o, Float.parseFloat(v));
                else if (t == double.class) fd.setDouble(o, Double.parseDouble(v));
                else if (t == int.class) fd.setInt(o, Integer.parseInt(v));
                else if (t == byte.class) fd.setByte(o, Byte.parseByte(v));
                else if (t == boolean.class) fd.setBoolean(o, v.equals("1"));
                else throw new IllegalArgumentException("field type " + t + " of " + name);
                return;
            } catch (NoSuchFieldException e) { c = c.getSuperclass(); }
        }
        throw new NoSuchFieldException(name);
    }

    static final String[] PART_FIELDS = {"health", "bandaged", "bitten", "bleeding", "isBleedingStemmed", "isCauterized", "scratched", "stitched",
        "deepWounded", "isInfected", "isFakeInfected", "bandageLife", "scratchTime", "biteTime", "alcoholicBandage", "stiffness", "woundInfectionLevel",
        "infectedWound", "bleedingTime", "deepWoundTime", "haveGlass", "stitchTime", "alcoholLevel", "additionalPain", "getBandageXp", "getStitchXp",
        "getSplintXp", "fractureTime", "splint", "splintFactor", "haveBullet", "burnTime", "needBurnWash", "lastTimeBurnWash", "plantainFactor",
        "comfreyFactor", "garlicFactor", "cutTime", "cut", "wetness"};
    static final String[] BODY_FIELDS = {"overallBodyHealth", "catchACold", "hasACold", "coldStrength", "timeToSneezeOrCough", "healthFromFoodTimer",
        "painReduction", "coldReduction", "infectionTime", "infectionMortalityDuration", "coldDamageStage", "isInfected", "isFakeInfected",
        "reduceFakeInfection", "oldNumZombiesVisible", "sneezeCoughActive", "sneezeCoughTime", "wasDraggingCorpse", "startedDraggingCorpse",
        "burntToDeath"};
    static final String[] CHAR_FIELDS = {"bleedingLevel", "painEffect", "painDelta", "reduceInfectionPower", "timeSinceLastSmoke", "forceWakeUp",
        "moveSpeed", "health", "maxWeight", "corpseSicknessRate"};
    static final String[] THERMO_FIELDS = {"setPoint", "metabolicRate", "metabolicRateReal", "metabolicTarget", "bodyHeatDelta", "coreHeatDelta",
        "thermalDamage", "damageCounter", "thermalDuration", "totalHeatRaw", "totalHeat", "primTotal", "secTotal", "externalAirTemperature",
        "airTemperature", "airAndWindTemp", "rateOfChangeCounter", "coreCelciusCache", "coreRateOfChange", "fluidsMultiplier", "energyMultiplier",
        "fatigueMultiplier", "thermalChevronUp"};
    static final String[] NODE_FIELDS = {"celcius", "skinCelcius", "heatDelta", "primaryDelta", "secondaryDelta", "insulation", "windresist",
        "bodyWetness", "clothingWetness"};

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
        for (int wx = 1034; wx <= 1037; wx++) {
            for (int wy = 1458; wy <= 1461; wy++) {
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
            // ZomboidGlobals, as the booted server loaded them (the port loads defines.lua itself)
            java.util.TreeMap<String, String> gl = new java.util.TreeMap<>();
            for (Field fd : zombie.ZomboidGlobals.class.getDeclaredFields()) {
                if (!java.lang.reflect.Modifier.isStatic(fd.getModifiers())) continue;
                fd.setAccessible(true);
                if (fd.getType() == double.class) gl.put(fd.getName(), Long.toHexString(Double.doubleToRawLongBits(fd.getDouble(null))));
                else if (fd.getType() == float.class) gl.put(fd.getName(), Integer.toHexString(Float.floatToRawIntBits(fd.getFloat(null))));
            }
            for (java.util.Map.Entry<String, String> e : gl.entrySet()) out.println("GLOBALS " + e.getKey() + " " + e.getValue());
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
        // room=1: the first square of the loaded chunks that is in a room; room=2: the first square on floor 1
        if (o.containsKey("room")) {
            IsoGridSquare found = null;
            int rz = o.get("room").equals("2") ? 1 : 0;
            for (int y = 1458 * 8; y < 1462 * 8 && found == null; y++) {
                for (int x = 1034 * 8; x < 1038 * 8 && found == null; x++) {
                    IsoGridSquare c = cell.getGridSquare(x, y, rz);
                    if (c != null && (rz == 1 || c.isInARoom())) found = c;
                }
            }
            if (found == null) throw new IllegalStateException("no square for room=" + o.get("room"));
            sq = found;
        }
        pl.setCurrent(sq);
        pl.setX(sq.getX() + 0.5f);
        pl.setY(sq.getY() + 0.5f);
        pl.setZ(sq.getZ());
        if (o.getOrDefault("sqfield", "0").equals("1")) setField(pl, "square", sq);
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
        // the climate: the values the character reads are set directly on the manager (the world's own are restored after)
        ClimateManager clim = ClimateManager.getInstance();
        for (String cf : new String[] {"temp", "wind", "daylight", "night"}) {
            if (!o.containsKey(cf)) continue;
            float v = Float.parseFloat(o.get(cf));
            switch (cf) {
                case "temp" -> { Object c = clim.getClimateFloat(ClimateManager.FLOAT_TEMPERATURE); float old = (Float) g(c, "finalValue"); setField(c, "finalValue", v); restore.add(() -> { try { setField(c, "finalValue", old); } catch (Exception e) { throw new RuntimeException(e); } }); }
                case "wind" -> { float old = (Float) g(clim, "windPower"); setField(clim, "windPower", v); restore.add(() -> { try { setField(clim, "windPower", old); } catch (Exception e) { throw new RuntimeException(e); } }); }
                case "daylight" -> { float old = (Float) g(clim, "dayLightLagged"); setField(clim, "dayLightLagged", v); restore.add(() -> { try { setField(clim, "dayLightLagged", old); } catch (Exception e) { throw new RuntimeException(e); } }); }
                default -> { float old = (Float) g(clim, "nightLagged"); setField(clim, "nightLagged", v); restore.add(() -> { try { setField(clim, "nightLagged", old); } catch (Exception e) { throw new RuntimeException(e); } }); }
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
            else if (k.equals("bed")) pl.setBedType(v);
            else if (k.equals("delay")) pl.setDelayToSleep(Float.parseFloat(v));
            else if (k.equals("fitness")) pl.setPerkLevelDebug(zombie.characters.skills.PerkFactory.Perks.Fitness, Integer.parseInt(v));
            else if (k.equals("strength")) pl.setPerkLevelDebug(zombie.characters.skills.PerkFactory.Perks.Strength, Integer.parseInt(v));
            else if (k.equals("zombies")) pl.getStats().numVisibleZombies = Integer.parseInt(v);
            else if (k.startsWith("bp.")) {
                String[] kp = k.split("\\.");
                setTyped(pl.getBodyDamage().getBodyParts().get(Integer.parseInt(kp[1])), kp[2], v);
            }
            else if (k.startsWith("bd.")) setTyped(pl.getBodyDamage(), k.substring(3), v);
            else if (k.startsWith("ch.")) setTyped(pl, k.substring(3), v);
            else if (k.startsWith("th.")) setTyped(pl.getBodyDamage().getThermoregulator(), k.substring(3), v);
            else if (k.startsWith("node.")) {
                String[] kp = k.split("\\.");
                setTyped(pl.getBodyDamage().getThermoregulator().getNode(Integer.parseInt(kp[1])), kp[2], v);
            }
            else if (k.equals("sleep")) {
                zombie.network.ServerOptions so2 = zombie.network.ServerOptions.instance;
                boolean oa = so2.sleepAllowed.getValue(), on = so2.sleepNeeded.getValue();
                so2.sleepAllowed.setValue(v.equals("1"));
                so2.sleepNeeded.setValue(v.equals("1"));
                restore.add(() -> { so2.sleepAllowed.setValue(oa); so2.sleepNeeded.setValue(on); });
            }
        }
        set(RandStandard.INSTANCE, RandAbstract.class, "rand", new Random(seed));
        set(RandStandard.INSTANCE, RandAbstract.class, "id", 0);
        out.println("ENV mult=" + f(gt.getMultiplier()) + " delta=" + f(gt.getTimeDelta()) + " gwsslu=" + f(gt.getGameWorldSecondsSinceLastUpdate())
            + " mpd=" + f(gt.getMinutesPerDay()) + " dmpd=" + f(gt.getDeltaMinutesPerDay()));
        // the facts of the headless player the port takes as inputs: its square, its inventory and the stats it was born with
        out.println("FACTS inroom=" + (sq.isInARoom() ? 1 : 0) + " z=" + sq.getZ() + " hydro=" + (IsoWorld.instance.isHydroPowerOn() ? 1 : 0)
            + " capw=" + f(pl.getInventory().getCapacityWeight()) + " maxw=" + pl.getMaxWeight() + " lastsq=" + (pl.getLastSquare() == null ? "null" : pl.getLastSquare() == sq ? "same" : "other")
            + " vehicle=" + (pl.getVehicle() == null ? 0 : 1) + " building=" + (pl.getCurrentBuilding() == null ? 0 : 1) + " toxic=" + (pl.getCurrentBuilding() != null && pl.getCurrentBuilding().isToxic() ? 1 : 0)
            + " sleepblock=" + ((zombie.network.GameServer.server && (!zombie.network.ServerOptions.instance.sleepAllowed.getValue() || !zombie.network.ServerOptions.instance.sleepNeeded.getValue())) ? 1 : 0)
            + " allasleep=" + (IsoPlayer.allPlayersAsleep() ? 1 : 0)
            + " instance=" + (IsoPlayer.getInstance() == pl ? 1 : 0)
            + " heatsources=" + ((java.util.List<?>) g(cell, "heatSources")).size()
            + " airtemp=" + f(clim.getAirTemperatureForCharacter(pl, false)) + " airwind=" + f(clim.getAirTemperatureForCharacter(pl, true))
            + " windchill=" + f(zombie.iso.weather.Temperature.getWindChillAmountForPlayer(pl))
            + " fitnessperk=" + pl.getPerkLevel(zombie.characters.skills.PerkFactory.Perks.Fitness) + " strperk=" + pl.getPerkLevel(zombie.characters.skills.PerkFactory.Perks.Strength)
            + " hours=" + d(pl.getHoursSurvived()) + " corpses=" + zombie.iso.CorpseCount.instance.getCorpseCount(pl) + " cdm=" + f(pl.getClothingDiscomfortModifier())
            + " mwbase=" + pl.getMaxWeightBase() + " mwdelta=" + f(pl.getMaxWeightDelta()) + " roof=" + (sq.haveRoof ? 1 : 0) + " health=" + f(pl.getHealth())
            + " allowsprint=" + (pl.isAllowSprint() ? 1 : 0) + " movespeed=" + f((Float) g(pl, "moveSpeed")));
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
                        case "moodles" -> {
                            sb.append(" moodles");
                            for (zombie.scripting.objects.MoodleType mt : zombie.scripting.objects.Registries.MOODLE_TYPE.values()) {
                                sb.append(' ').append(pl.getMoodles().getMoodleLevel(mt));
                                if (mt == zombie.scripting.objects.MoodleType.HYPERTHERMIA || mt == zombie.scripting.objects.MoodleType.HYPOTHERMIA) {
                                    zombie.characters.Moodles.Moodle m = (zombie.characters.Moodles.Moodle) ((java.util.Map<?, ?>) g(pl.getMoodles(), "moodles")).get(mt);
                                    float r = m.getChevronColor().r;
                                    sb.append('/').append(m.getChevronCount()).append(m.isChevronIsUp() ? 1 : 0).append(r == 1.0f ? 0 : r == 0.88235295f ? 1 : 2);
                                }
                            }
                        }
                        case "traits" -> {
                            sb.append(" traits");
                            for (CharacterTrait t : pl.getCharacterTraits().getKnownTraits()) sb.append(' ').append(t);
                        }
                        case "draws" -> sb.append(" draws=").append(g(RandStandard.INSTANCE, "id"));
                        case "thermo" -> {
                            zombie.characters.BodyDamage.Thermoregulator t = pl.getBodyDamage().getThermoregulator();
                            sb.append(" thermo");
                            for (String fn : THERMO_FIELDS) sb.append(' ').append(fv(t, fn));
                            for (int ni = 0; ni < t.getNodeSize(); ni++) {
                                sb.append(" |");
                                for (String fn : NODE_FIELDS) sb.append(' ').append(fv(t.getNode(ni), fn));
                            }
                        }
                        case "parts" -> {
                            sb.append(" parts");
                            for (BodyPart bp : pl.getBodyDamage().getBodyParts()) {
                                sb.append(" | ").append(f(bp.getPain()));
                                for (String fn : PART_FIELDS) sb.append(' ').append(fv(bp, fn));
                            }
                        }
                        case "bodyst" -> {
                            sb.append(" bodyst");
                            for (String fn : BODY_FIELDS) sb.append(' ').append(fv(pl.getBodyDamage(), fn));
                            sb.append(" |");
                            for (String fn : CHAR_FIELDS) sb.append(' ').append(fv(pl, fn));
                        }
                        default -> throw new IllegalArgumentException("show " + sec);
                    }
                }
                out.println(sb);
            }
        }
        for (Runnable r : restore) r.run();
    }
}
