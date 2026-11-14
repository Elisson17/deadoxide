import java.io.ByteArrayInputStream;
import java.io.DataInputStream;
import java.io.File;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import se.krka.kahlua.vm.KahluaTable;
import se.krka.kahlua.vm.KahluaTableIterator;
import zombie.GameTime;
import zombie.core.Core;
import zombie.iso.weather.ClimateManager;
import zombie.network.GameServer;

// Inside the booted original (ServerHarness): the ORIGINAL GameTime.load(DataInputStream) over the player's save's
// map_t.bin, as a single-player game runs it from IsoWorld.init (GameServer.server false during the call). The game
// time's and the climate's mod data tables are set to null first, as in a freshly started game. The probe prints
// what the load sets: the clock and calendar, the mod data, the poisonous items, the helicopter, the minute stamp,
// and the climate (offsets, air front, snow and fog, weather period, thunder clouds, climate mod data).
// Args: <scratch dir> <save dir>
public class GameTimeProbe {
    static Object g(Object o, String name) throws Exception {
        Class<?> c = o.getClass();
        while (c != null) {
            try {
                Field f = c.getDeclaredField(name);
                f.setAccessible(true);
                return f.get(o);
            } catch (NoSuchFieldException e) { c = c.getSuperclass(); }
        }
        throw new NoSuchFieldException(name);
    }

    static void set(Object o, String name, Object v) throws Exception {
        Field f = o.getClass().getDeclaredField(name);
        f.setAccessible(true);
        f.set(o, v);
    }

    static String fh(Object f) { return Integer.toHexString(Float.floatToRawIntBits((Float) f)); }
    static String dh(Object d) { return Long.toHexString(Double.doubleToRawLongBits((Double) d)); }
    static int b(Object v) { return (Boolean) v ? 1 : 0; }

    static String lua(Object v) {
        if (v == null) return "null";
        if (v instanceof String x) return "s:" + x;
        if (v instanceof Double x) return "n:" + dh(x);
        if (v instanceof Boolean x) return "b:" + x;
        if (v instanceof KahluaTable t) {
            List<String> l = new ArrayList<>();
            KahluaTableIterator it = t.iterator();
            while (it.advance()) l.add(lua(it.getKey()) + "=" + lua(it.getValue()));
            Collections.sort(l);
            return "{" + String.join(",", l) + "}";
        }
        return "?" + v;
    }

    static String front(Object f) throws Exception {
        float[] nc = (float[]) g(f, "noiseCache");
        return fh(g(f, "days")) + " " + fh(g(f, "maxNoise")) + " " + fh(g(f, "totalNoise")) + " " + g(f, "type") + " "
            + fh(g(f, "strength")) + " " + fh(g(f, "frontWindAngleDegrees")) + " " + fh(nc[0]) + " " + fh(nc[1]);
    }

    static String color(Object c) throws Exception {
        StringBuilder sb = new StringBuilder();
        for (String side : new String[]{"interior", "exterior"}) {
            zombie.core.Color col = (zombie.core.Color) g(c, side);
            sb.append(" ").append(fh(col.r)).append(" ").append(fh(col.g)).append(" ").append(fh(col.b)).append(" ").append(fh(col.a));
        }
        return sb.toString();
    }

    public static void main(String[] a) throws Exception {
        File save = new File(a[1]);
        ServerHarness.run(a[0], out -> {
            byte[] data = Files.readAllBytes(new File(save, "map_t.bin").toPath());
            GameTime gt = GameTime.getInstance();
            ClimateManager cm = ClimateManager.getInstance();
            set(gt, "table", null);
            set(cm, "modDataTable", null);
            boolean server = GameServer.server;
            GameServer.server = false;
            try {
                gt.load(new DataInputStream(new ByteArrayInputStream(data)));
            } finally {
                GameServer.server = server;
            }
            out.println("GT " + fh(g(gt, "multiplier")) + " " + gt.getNightsSurvived() + " " + g(gt, "targetZombies") + " "
                + fh(g(gt, "lastTimeOfDay")) + " " + fh(gt.getTimeOfDay()) + " " + gt.getDay() + " " + gt.getMonth() + " " + gt.getYear() + " "
                + g(gt, "helicopterDay1") + " " + g(gt, "helicopterTime1Start") + " " + g(gt, "helicopterTime1End") + " "
                + gt.getMinutesStamp() + " " + gt.getHour() + " " + gt.getMinutes() + " " + dh(gt.getWorldAgeHours()));
            out.println("GTT " + lua(g(gt, "table")));
            out.println("CORE " + Core.getInstance().getPoisonousBerry() + " " + Core.getInstance().getPoisonousMushroom());
            out.println("CM " + dh(g(cm, "simplexOffsetA")) + " " + dh(g(cm, "simplexOffsetB")) + " " + dh(g(cm, "simplexOffsetC")) + " "
                + dh(g(cm, "simplexOffsetD")) + " " + fh(g(cm, "snowFracNow")) + " " + fh(g(cm, "snowStrength")) + " "
                + b(g(cm, "canDoWinterSprites")) + " " + b(g(cm, "dayDoFog")) + " " + fh(g(cm, "dayFogStrength")));
            out.println("CF " + front(g(cm, "currentFront")));
            Object wp = g(cm, "weatherPeriod");
            boolean running = (Boolean) g(wp, "isRunning");
            out.println("WP " + (running ? 1 : 0));
            if (running) {
                List<?> stages = (List<?>) g(wp, "weatherStages");
                out.println("WPR " + g(wp, "weatherStageIndex") + " " + fh(g(wp, "currentStrength")) + " " + fh(g(wp, "rainThreshold")) + " "
                    + b(g(wp, "isThunderStorm")) + " " + b(g(wp, "isTropicalStorm")) + " " + b(g(wp, "isBlizzard")) + " "
                    + fh(g(wp, "windAngleDirMod")) + " " + dh(g(wp, "duration")) + " " + stages.size() + " " + stages.indexOf(g(wp, "currentStage")));
                out.println("WPF " + front(g(wp, "frontCache")));
                for (int i = 0; i < stages.size(); i++) {
                    Object s = stages.get(i);
                    out.println("WS " + i + " " + g(s, "stageId") + " " + dh(g(s, "stageStart")) + " " + dh(g(s, "stageEnd")) + " "
                        + dh(g(s, "stageDuration")) + " " + fh(g(s, "entryStrength")) + " " + fh(g(s, "exitStrength")) + " "
                        + fh(g(s, "targetStrength")) + " " + g(s, "lerpMidVal") + " " + g(s, "lerpEndVal") + " " + b(g(s, "hasStartedCloud")) + " "
                        + g(s, "modId") + " " + fh(g(s, "fogStrength")) + " " + stages.indexOf(g(s, "previousStage")) + " "
                        + stages.indexOf(g(s, "nextStage")) + " " + b(g(s, "creationFinished")));
                }
                out.println("WPC" + color(g(wp, "cloudColor")));
            }
            Object[] clouds = (Object[]) g(g(cm, "thunderStorm"), "clouds");
            for (int i = 0; i < clouds.length; i++) {
                Object c = clouds[i];
                boolean r = (Boolean) g(c, "isRunning");
                out.println("TS " + i + " " + (r ? 1 : 0) + (!r ? "" : " " + g(c, "startX") + " " + g(c, "startY") + " " + g(c, "endX") + " "
                    + g(c, "endY") + " " + fh(g(c, "radius")) + " " + fh(g(c, "angle")) + " " + fh(g(c, "strength")) + " " + fh(g(c, "thunderRatio")) + " "
                    + dh(g(c, "startTime")) + " " + dh(g(c, "endTime")) + " " + dh(g(c, "duration")) + " " + fh(g(c, "percentageOffset"))));
            }
            out.println("CMT " + lua(g(cm, "modDataTable")));
        });
    }
}
