import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import java.util.Random;
import java.util.TimeZone;
import zombie.GameTime;
import zombie.SandboxOptions;
import zombie.core.Core;
import zombie.core.PerformanceSettings;
import zombie.core.random.RandAbstract;
import zombie.core.random.RandStandard;
import zombie.erosion.ErosionMain;
import zombie.erosion.season.ErosionSeason;
import zombie.iso.IsoMetaGrid;
import zombie.iso.IsoWorld;
import zombie.iso.weather.ClimateColorInfo;
import zombie.iso.weather.ClimateForecaster;
import zombie.iso.weather.ClimateManager;
import zombie.iso.weather.ClimateMoon;
import zombie.iso.weather.ThunderStorm;
import zombie.iso.weather.WeatherPeriod;
import zombie.network.GameServer;

// Inside the booted original dedicated server (ServerHarness), with the JVM's default zone pinned to UTC: the ORIGINAL
// zombie.iso.weather.ClimateManager stepped over simulated days.
//
// Per scenario "seed:year:month:day:hour:days" (month and day 0-based, as GameTime holds them):
//  - the global Rand (RandStandard) draws from a java.util.Random(seed) instead of its seeded-by-time generator;
//  - GameTime is set to the date and hour with 0 nights survived, 60 minutes per day, multiplier 1, fpsMultiplier 2;
//  - the erosion season (ErosionMain's, the one ClimateManager.init takes) is set to the date the way
//    ErosionMain.mainTimer does when the date changes;
//  - a new ClimateManager is made (its constructor draws the simplex offsets from Rand and triggers
//    OnClimateManagerInit, which the server's ClimateMain.lua handles), set as the instance and init'ed with the world's
//    meta grid;
//  - then, per tick, the clock part of GameTime.update (time of day += 1/minutesPerDay/60 * getMultiplier()/2, nights
//    at 7:00, the calendar at midnight, the minute stamp; at each ten-minute mark the season part of
//    ErosionMain.mainTimer and ClimateManager.updateEveryTenMins), then ClimateManager.update().
// The server stays a server (GameServer.server true): its climate is the authoritative one. Printed: the set-up, the
// colours the Lua set, the 40 forecasts after init and after each day change, and at every climate tick (each game
// minute) every climate float, colour and bool, the internal state, the weather period, the thunder clouds and (on a
// change) the thunder event the server would send (TE: x y strike/lightning/rumble).
// Floats with Float.toString, doubles with Double.toString.
// Args: <scratch dir> <scenario>...
public class ClimateProbe {
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

    static Object call(Object o, String name) throws Exception {
        Method m = o.getClass().getDeclaredMethod(name);
        m.setAccessible(true);
        return m.invoke(o);
    }

    static String f(Object v) { return Float.toString((Float) v); }
    static String d(Object v) { return Double.toString((Double) v); }
    static int b(Object v) { return (Boolean) v ? 1 : 0; }

    static String col(ClimateColorInfo c) {
        return c.getInterior().r + " " + c.getInterior().g + " " + c.getInterior().b + " " + c.getInterior().a + " "
            + c.getExterior().r + " " + c.getExterior().g + " " + c.getExterior().b + " " + c.getExterior().a;
    }

    static String front(Object fr) throws Exception {
        float[] nc = (float[]) g(fr, "noiseCache");
        return g(fr, "type") + " " + f(g(fr, "strength")) + " " + f(g(fr, "days")) + " " + f(g(fr, "maxNoise")) + " "
            + f(g(fr, "totalNoise")) + " " + nc[0] + " " + nc[1] + " " + f(g(fr, "frontWindAngleDegrees"));
    }

    static String fv(ClimateForecaster.ForecastValue v) {
        return v.getDayMin() + "," + v.getDayMax() + "," + v.getDayMean() + "," + v.getNightMin() + "," + v.getNightMax() + ","
            + v.getNightMean() + "," + v.getTotalMin() + "," + v.getTotalMax() + "," + v.getTotalMean();
    }

    static String stages(WeatherPeriod wp) {
        List<String> l = new ArrayList<>();
        for (WeatherPeriod.WeatherStage s : wp.getWeatherStages()) l.add(Integer.toString(s.getStageID()));
        return "[" + String.join(",", l) + "]";
    }

    static void forecasts(PrintStream out, ClimateManager cm) throws Exception {
        ClimateForecaster fc = cm.getClimateForecaster();
        Object[] arr = (Object[]) g(fc, "forecasts");
        List<Object> list = java.util.Arrays.asList(arr);
        for (int i = 0; i < arr.length; i++) {
            ClimateForecaster.DayForecast df = (ClimateForecaster.DayForecast) arr[i];
            WeatherPeriod wp = df.getWeatherPeriod();
            out.println("FC " + i + " offset=" + df.getIndexOffset() + " starts=" + b(df.isWeatherStarts()) + " start=" + df.getWeatherStartTime()
                + " end=" + df.getWeatherEndTime() + " snow=" + b(df.isChanceOnSnow()) + " front=" + df.getAirFrontString() + " fog=" + b(df.isHasFog())
                + " fogStr=" + df.getFogStrength() + " fogDur=" + df.getFogDuration() + " overlap=" + list.indexOf(df.getWeatherOverlap())
                + " heavy=" + b(df.isHasHeavyRain()) + " storm=" + b(df.isHasStorm()) + " tropical=" + b(df.isHasTropicalStorm())
                + " blizzard=" + b(df.isHasBlizzard()) + " dawn=" + df.getDawn() + " dusk=" + df.getDusk() + " daylight=" + df.getDayLightHours()
                + " stages=" + df.getWeatherStages());
            out.println(" FCA " + front(df.getAirFront()) + " wp=" + b(wp.isRunning()) + " dur=" + wp.getDuration() + " str=" + wp.getTotalStrength()
                + " angle=" + wp.getWindAngleDegrees() + " " + stages(wp));
            out.println(" FCV t=" + fv(df.getTemperature()) + " h=" + fv(df.getHumidity()) + " wd=" + fv(df.getWindDirection()) + " wp="
                + fv(df.getWindPower()) + " c=" + fv(df.getCloudiness()));
        }
    }

    public static void main(String[] a) throws Exception {
        TimeZone.setDefault(TimeZone.getTimeZone("UTC"));
        ServerHarness.run(a[0], out -> {
            for (int i = 1; i < a.length; i++) scenario(out, a[i]);
        });
    }

    static void scenario(PrintStream out, String spec) throws Exception {
        String[] p = spec.split(":");
        long seed = Long.parseLong(p[0]);
        int year = Integer.parseInt(p[1]), month = Integer.parseInt(p[2]), day = Integer.parseInt(p[3]);
        float hour = Float.parseFloat(p[4]);
        int days = Integer.parseInt(p[5]);
        out.println("SCENARIO " + spec);

        // Optional 7th field: comma-separated options, set before the manager is made and put back to the defaults
        // by the next scenario: rain=1..5 (default 3), climate=1..6 (1), fog=1..3 (1), daynight=1..3 (1), mode=<game mode>.
        int rainOpt = 3, climateOpt = 1, fogOpt = 1, dayNightOpt = 1;
        String modeOpt = "Multiplayer";
        if (p.length > 6) {
            for (String kv : p[6].split(",")) {
                String[] e = kv.split("=", 2);
                switch (e[0]) {
                    case "rain" -> rainOpt = Integer.parseInt(e[1]);
                    case "climate" -> climateOpt = Integer.parseInt(e[1]);
                    case "fog" -> fogOpt = Integer.parseInt(e[1]);
                    case "daynight" -> dayNightOpt = Integer.parseInt(e[1]);
                    case "mode" -> modeOpt = e[1];
                    default -> throw new IllegalArgumentException(kv);
                }
            }
        }
        SandboxOptions.instance.rain.setValue(rainOpt);
        SandboxOptions.instance.climateCycle.setValue(climateOpt);
        SandboxOptions.instance.fogCycle.setValue(fogOpt);
        SandboxOptions.instance.dayNightCycle.setValue(dayNightOpt);
        IsoWorld.instance.setGameMode(modeOpt);
        Core.getInstance().setForceSnow(false);

        set(RandStandard.INSTANCE, RandAbstract.class, "rand", new Random(seed));
        set(RandStandard.INSTANCE, RandAbstract.class, "id", 0);

        GameTime gt = GameTime.getInstance();
        gt.setYear(year);
        gt.setMonth(month);
        gt.setDay(day);
        gt.setTimeOfDay(hour);
        gt.setNightsSurvived(0);
        gt.setMinutesPerDay(60.0F);
        gt.setMultiplier(1.0F);
        gt.fpsMultiplier = 2.0F;
        gt.multiplierBias = 1.0F;
        gt.perObjectMultiplier = 1.0F;
        call(gt, "setMinutesStamp");

        ErosionMain em = ErosionMain.getInstance();
        ErosionSeason season = em.getSeasons();
        int[] eDate = {(Integer) g(em, "day"), (Integer) g(em, "month"), (Integer) g(em, "year")};
        Runnable seasonTimer = () -> {
            if (gt.getDay() != eDate[0] || gt.getMonth() != eDate[1] || gt.getYear() != eDate[2]) {
                eDate[0] = gt.getDay();
                eDate[1] = gt.getMonth();
                eDate[2] = gt.getYear();
                season.setDay(eDate[0], eDate[1], eDate[2]);
            }
        };
        seasonTimer.run();

        SandboxOptions so = SandboxOptions.instance;
        IsoMetaGrid mg = IsoWorld.instance.getMetaGrid();
        out.println("SETUP server=" + b(GameServer.server) + " fogQuality=" + PerformanceSettings.fogQuality + " multiplier=" + gt.getMultiplier()
            + " unmodded=" + gt.getUnmoddedMultiplier() + " rain=" + so.getRainModifier() + " climateCycle=" + so.climateCycle.getValue()
            + " fogCycle=" + so.fogCycle.getValue() + " dayNight=" + so.dayNightCycle.getValue() + " gameMode=" + IsoWorld.instance.getGameMode()
            + " weatherShader=" + b(zombie.core.SceneShaderStore.weatherShader != null) + " offscreen=" + b(Core.getInstance().getOffscreenBuffer() != null)
            + " debug=" + b(Core.debug) + " forceSnow=" + b(Core.getInstance().isForceSnow()) + " grid=" + mg.minX + "," + mg.maxX + "," + mg.minY + "," + mg.maxY);
        out.println("SEASONCFG " + season.getLat() + " " + season.getTempMax() + " " + season.getTempMin() + " " + season.getTempDiff() + " "
            + season.getSeasonLag() + " " + season.getHighNoon() + " " + season.getSeedA() + " " + season.getSeedB() + " " + season.getSeedC() + " "
            + season.getRainYearAverage());

        ClimateManager cm = new ClimateManager();
        ClimateManager.setInstance(cm);
        cm.init(mg);
        out.println("OFFSETS " + cm.getSimplexOffsetA() + " " + cm.getSimplexOffsetB() + " " + cm.getSimplexOffsetC() + " " + cm.getSimplexOffsetD()
            + " map=" + ThunderStorm.mapMinX + "," + ThunderStorm.mapMaxX + "," + ThunderStorm.mapMinY + "," + ThunderStorm.mapMaxY
            + " rand=" + g(RandStandard.INSTANCE, "id"));
        out.println("COL night " + col(cm.getColNight()) + " | noMoon " + col(cm.getColNightNoMoon()) + " | moon " + col(cm.getColNightMoon()));
        out.println("COL fog " + col(cm.getColFog()) + " | legacy " + col(cm.getColFogLegacy()) + " | new " + col(cm.getColFogNew()));
        out.println("COL storm " + col(cm.getFogTintStorm()) + " | tropical " + col(cm.getFogTintTropical()));
        WeatherPeriod wpm = cm.getWeatherPeriod();
        out.println("COL clouds " + col(wpm.getCloudColorReddish()) + " | " + col(wpm.getCloudColorGreenish()) + " | " + col(wpm.getCloudColorBlueish())
            + " | " + col(wpm.getCloudColorPurplish()) + " | " + col(wpm.getCloudColorTropical()) + " | " + col(wpm.getCloudColorBlizzard()));
        for (int seg = 0; seg < 3; seg++)
            for (int temp = 0; temp < 3; temp++)
                for (int s = 0; s < 4; s++) out.println("SEASONCOL " + seg + " " + temp + " " + s + " " + col(cm.getSeasonColor(seg, temp, s)));
        forecasts(out, cm);

        int minutesMod = (int) ((gt.getTimeOfDay() - (int) gt.getTimeOfDay()) * 60.0F) / 10;
        double lastStart = Double.NaN;
        boolean lastRunning = false;
        String[] lastEvent = {"0 0 000"};
        long ticks = (long) days * 108000L;
        for (long t = 1; t <= ticks; t++) {
            float last = gt.getTimeOfDay();
            float time = 1.0F / gt.getMinutesPerDay() / 60.0F * gt.getMultiplier() / 2.0F;
            gt.setTimeOfDay(gt.getTimeOfDay() + time);
            if (last <= 7.0F && gt.getTimeOfDay() > 7.0F) gt.setNightsSurvived(gt.getNightsSurvived() + 1);
            if (gt.getTimeOfDay() >= 24.0F) {
                gt.setTimeOfDay(gt.getTimeOfDay() - 24.0F);
                gt.setDay(gt.getDay() + 1);
                if (gt.getDay() >= gt.daysInMonth(gt.getYear(), gt.getMonth())) {
                    gt.setDay(0);
                    gt.setMonth(gt.getMonth() + 1);
                    if (gt.getMonth() >= 12) {
                        gt.setMonth(0);
                        gt.setYear(gt.getYear() + 1);
                    }
                }
            }
            call(gt, "setMinutesStamp");
            int now = (int) ((gt.getTimeOfDay() - (int) gt.getTimeOfDay()) * 60.0F);
            if (now / 10 != minutesMod) {
                seasonTimer.run();
                cm.updateEveryTenMins();
                minutesMod = now / 10;
            }
            cm.update();
            if (!(Boolean) g(cm, "tickIsClimateTick")) continue;

            out.println("T " + t + " " + gt.getYear() + "-" + gt.getMonth() + "-" + gt.getDay() + " " + gt.getTimeOfDay() + " age=" + gt.getWorldAgeHours()
                + " stamp=" + gt.getMinutesStamp() + " hour=" + b(g(cm, "tickIsHourChange")) + " day=" + b(g(cm, "tickIsDayChange"))
                + " rand=" + g(RandStandard.INSTANCE, "id"));
            StringBuilder v = new StringBuilder("V");
            StringBuilder in = new StringBuilder("I");
            for (int i = 0; i < cm.getFloatMax(); i++) {
                ClimateManager.ClimateFloat cf = cm.getClimateFloat(i);
                v.append(' ').append(cf.getFinalValue());
                in.append(' ').append(cf.getInternalValue());
            }
            out.println(v);
            out.println(in);
            out.println("G " + col(cm.getClimateColor(0).getFinalValue()) + " | " + col(cm.getClimateColor(0).getInternalValue()) + " | "
                + col(cm.getClimateColor(1).getFinalValue()) + " | snow=" + b(g(cm.getClimateBool(0), "finalValue")) + b(cm.getClimateBool(0).getInternalValue()));
            ErosionSeason cs = cm.getSeason();
            out.println("X " + cm.getAirMass() + " " + cm.getAirMassDaily() + " " + cm.getAirMassTemperature() + " " + cm.getBaseTemperature() + " "
                + f(g(cm, "snowFall")) + " " + cm.getSnowStrength() + " " + f(g(cm, "snowMeltStrength")) + " " + cm.getSnowFracNow() + " "
                + b(g(cm, "canDoWinterSprites")) + " " + b(g(cm, "dayDoFog")) + " " + f(g(cm, "dayFogStrength")) + " " + f(g(cm, "fogLerpValue")) + " "
                + cm.getWindPower() + " " + f(g(cm, "dayLightLagged")) + " " + f(g(cm, "nightLagged")) + " season=" + cs.getSeason() + ","
                + cs.getSeasonProgression() + " moon=" + ClimateMoon.getInstance().getCurrentMoonPhase() + " view=" + gt.getViewDistMin() + ","
                + gt.getViewDistMax() + " front=" + front(g(cm, "currentFront")));
            WeatherPeriod wp = cm.getWeatherPeriod();
            double start = (Double) g(wp, "startTime");
            if (wp.isRunning() != lastRunning || start != lastStart) {
                lastRunning = wp.isRunning();
                lastStart = start;
                out.println("WP start=" + start + " running=" + b(wp.isRunning()) + " dur=" + wp.getDuration() + " front=" + front(wp.getFrontCache())
                    + " dirMod=" + f(g(wp, "windAngleDirMod")) + " cloud=" + col(wp.getCloudColor()));
                List<WeatherPeriod.WeatherStage> ws = wp.getWeatherStages();
                for (int i = 0; i < ws.size(); i++) {
                    WeatherPeriod.WeatherStage s = ws.get(i);
                    out.println(" WS " + i + " " + s.getStageID() + " " + s.getStageStart() + " " + s.getStageEnd() + " " + s.getStageDuration() + " "
                        + f(g(s, "entryStrength")) + " " + f(g(s, "exitStrength")) + " " + f(g(s, "targetStrength")) + " " + g(s, "lerpMidVal") + " "
                        + g(s, "lerpEndVal") + " " + f(g(s, "fogStrength")));
                }
            }
            if (wp.isRunning()) {
                WeatherPeriod.WeatherStage s = wp.getCurrentStage();
                out.println("W " + g(wp, "weatherStageIndex") + " " + (s == null ? "null" : s.getStageID() + " " + s.getLinearT() + " " + s.getParabolicT()
                    + " " + b(g(s, "isCycleFirstHalf")) + " " + b(s.getHasStartedCloud()) + " " + f(g(s, "entryStrength")) + " " + f(g(s, "exitStrength")))
                    + " str=" + wp.getCurrentStrength() + " rain=" + wp.getRainThreshold() + " noise=" + wp.getWeatherNoise() + " total=" + wp.getTotalProgress()
                    + " stage=" + wp.getStageProgress() + " precip=" + wp.getPrecipitationFinal() + " tinf=" + f(g(wp, "temperatureInfluence"))
                    + " flags=" + b(wp.isThunderStorm()) + b(wp.isTropicalStorm()) + b(wp.isBlizzard()) + " cloud=" + col(wp.getCloudColor()));
            }
            ThunderStorm ts = cm.getThunderStorm();
            Object[] clouds = (Object[]) g(ts, "clouds");
            for (int i = 0; i < clouds.length; i++) {
                Object c = clouds[i];
                if (!(Boolean) g(c, "isRunning") && (Double) g(c, "startTime") == 0.0) continue;
                GameTime.AnimTimer st = (GameTime.AnimTimer) g(c, "suspendTimer");
                out.println("TC " + i + " " + b(g(c, "isRunning")) + " " + g(c, "currentX") + " " + g(c, "currentY") + " " + g(c, "startX") + " "
                    + g(c, "startY") + " " + g(c, "endX") + " " + g(c, "endY") + " " + d(g(c, "startTime")) + " " + d(g(c, "endTime")) + " "
                    + d(g(c, "duration")) + " " + f(g(c, "strength")) + " " + f(g(c, "angle")) + " " + f(g(c, "radius")) + " "
                    + f(g(c, "eventFrequency")) + " " + f(g(c, "thunderRatio")) + " " + f(g(c, "percentageOffset")) + " timer=" + st.elapsed + ","
                    + st.duration + "," + b(st.finished) + "," + st.ticks + " strike=" + g(ts, "strikeRadius"));
            }
            Object ne = g(ts, "networkThunderEvent");
            String te = g(ne, "eventX") + " " + g(ne, "eventY") + " " + b(g(ne, "doStrike")) + b(g(ne, "doLightning")) + b(g(ne, "doRumble"));
            if (!te.equals(lastEvent[0])) {
                lastEvent[0] = te;
                out.println("TE " + te);
            }
            if ((Boolean) g(cm, "tickIsDayChange")) forecasts(out, cm);
        }
        out.println("END " + spec + " rand=" + g(RandStandard.INSTANCE, "id"));
    }
}
