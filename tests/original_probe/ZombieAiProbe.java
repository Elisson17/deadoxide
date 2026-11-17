import java.io.PrintStream;
import java.util.Random;
import zombie.characters.IsoPlayer;
import zombie.characters.IsoZombie;
import zombie.characters.SurvivorDesc;
import zombie.core.random.RandAbstract;
import zombie.core.random.RandStandard;
import zombie.core.skinnedmodel.population.OutfitRNG;
import zombie.iso.IsoCamera;
import zombie.iso.IsoCell;
import zombie.iso.IsoChunk;
import zombie.iso.IsoChunkMap;
import zombie.iso.IsoGridSquare;
import zombie.network.GameServer;
import zombie.network.ServerLOS;
import zombie.network.ServerMap;
import zombie.SandboxOptions;

// The ORIGINAL zombie AI run headless: the zombie of a seed on the square (8280, 11670, 0) of the harness's world and a
// player stand-in a fixed offset away, ticked as the single-player game's world ticks its objects (preupdate, update,
// postupdate). `GameServer.server` is false from before the zombie is built until the scenario ends (the zombie's variable
// callbacks, its animation update cadence and the square lookups are the single-player ones then), the world has an
// IsoChunkMap around the player and ServerLOS.init() ran; the dedicated server's world is otherwise as the other probes have
// it. Scenario "zai:<seed>:<ticks>:<dx>:<dy>:<mode>": the player stands at (8280.5 + dx, 11670.5 + dy); `force` sets the
// zombie's target to the player (and timeSinceSeenFlesh to 0) before every tick, `none` leaves the zombie without a target.
// Per tick: the draws of the global Rand, the action context's state, the AI state, position, direction, health, target,
// the path behaviour and the animation variables the states read.
public class ZombieAiProbe {
    static final String[] VARS = {
        "bLunge", "bMoving", "bPathfind", "bHasTarget", "bCanSeeTarget", "bAttack", "isFacingTarget", "alerted", "bThump",
        "bKnockedDown", "bOnFloor", "bStaggerBack", "hashitreaction", "AttackType", "AttackOutcome", "AttackDidDamage", "ZombieBiteDone"
    };

    static String vars(IsoZombie z) {
        StringBuilder sb = new StringBuilder();
        for (String n : VARS) {
            var s = z.getVariable(n);
            sb.append(n).append('=').append(s == null ? "-" : s.getValueString()).append(' ');
        }
        return sb.toString().trim();
    }

    static void scenario(PrintStream out, String spec, IsoCell cell) throws Exception {
        String[] p = spec.split(":");
        long seed = Long.parseLong(p[1]);
        int ticks = Integer.parseInt(p[2]);
        float dx = Float.parseFloat(p[3]);
        float dy = Float.parseFloat(p[4]);
        boolean force = p[5].equals("force");
        if (!ActionContextProbe.chunksLoaded) {
            ActionContextProbe.loadChunks(cell);
            ActionContextProbe.chunksLoaded = true;
            ServerLOS.init();
        }
        java.util.Map<String, Integer> saved = new java.util.LinkedHashMap<>();
        for (java.util.Map.Entry<String, Integer> e : ActionContextProbe.BASE.entrySet()) {
            Object opt = ActionContextProbe.g(SandboxOptions.instance.lore, Character.toLowerCase(e.getKey().charAt(0)) + e.getKey().substring(1));
            saved.put(e.getKey(), (Integer) opt.getClass().getMethod("getValue").invoke(opt));
            ActionContextProbe.setOption(opt, e.getValue());
        }
        out.println("SCENARIO " + spec);
        ActionContextProbe.options(out);
        // single-player: no dedicated server, one local player with a chunk map around it
        GameServer.server = false;
        IsoPlayer.numPlayers = 1;
        IsoChunkMap cm = new IsoChunkMap(cell);
        cm.playerId = 0;
        cm.ignore = false;
        cm.worldX = 1035;
        cm.worldY = 1458;
        IsoChunk[] arr = (IsoChunk[]) ActionContextProbe.g(cm, "chunksSwapA");
        int w = IsoChunkMap.chunkGridWidth;
        ServerMap sm = ServerMap.instance;
        for (int wx = 1034; wx <= 1035; wx++) {
            for (int wy = 1458; wy <= 1459; wy++) {
                IsoChunk ch = null;
                for (Object o : sm.loadedCells) {
                    ServerMap.ServerCell sc = (ServerMap.ServerCell) o;
                    int cx = wx - sc.wx * 8, cy = wy - sc.wy * 8;
                    if (cx >= 0 && cx < 8 && cy >= 0 && cy < 8 && sc.chunks[cx][cy] != null) ch = sc.chunks[cx][cy];
                }
                arr[w * (wy - (cm.worldY - w / 2)) + (wx - (cm.worldX - w / 2))] = ch;
            }
        }
        cell.chunkMap[0] = cm;
        // the player stand-in first, so its construction draws are not in the zombie's sequence
        int px = (int) Math.floor(8280.5f + dx), py = (int) Math.floor(11670.5f + dy);
        IsoPlayer pl = new IsoPlayer(cell, new SurvivorDesc(), px, py, 0);
        IsoGridSquare psq = cell.getGridSquare(px, py, 0);
        pl.setCurrent(psq);
        pl.setMovingSquareNow();
        pl.setX(8280.5f + dx);
        pl.setY(11670.5f + dy);
        pl.setZ(0);
        IsoPlayer.players[0] = pl;
        cell.getObjectList().add(pl);
        IsoCamera.setCameraCharacter(pl);
        java.lang.reflect.Field rf = RandAbstract.class.getDeclaredField("rand");
        rf.setAccessible(true);
        rf.set(RandStandard.INSTANCE, new Random(seed));
        java.lang.reflect.Field idf = RandAbstract.class.getDeclaredField("id");
        idf.setAccessible(true);
        idf.set(RandStandard.INSTANCE, 0);
        OutfitRNG.setSeed(seed);
        IsoZombie z = new IsoZombie(cell);
        out.println("ZOMBIE draws=" + ActionContextProbe.g(RandStandard.INSTANCE, "id") + " vars=" + ActionContextProbe.gameVars(z));
        IsoGridSquare sq = cell.getGridSquare(8280, 11670, 0);
        z.setCurrent(sq);
        z.setMovingSquareNow();
        z.setX(8280.5f);
        z.setY(11670.5f);
        z.setZ(0);
        cell.getObjectList().add(z);
        Object pfb = z.getPathFindBehavior2();
        int prev = (Integer) ActionContextProbe.g(RandStandard.INSTANCE, "id");
        for (int n = 1; n <= ticks; n++) {
            if (force) {
                z.setTarget(pl);
                ZombieProbe.set(z, IsoZombie.class, "timeSinceSeenFlesh", 0.0f);
            }
            z.preupdate();
            z.update();
            z.postupdate();
            int draws = (Integer) ActionContextProbe.g(RandStandard.INSTANCE, "id");
            out.println("T " + n + " draws=" + (draws - prev) + " ctx=" + z.getActionContext().getCurrentStateName()
                + " child=" + ActionContextProbe.list(z.getActionContext().getChildStates())
                + " ai=" + (z.getCurrentState() == null ? "null" : z.getCurrentState().getClass().getSimpleName())
                + " x=" + z.getX() + " y=" + z.getY() + " dir=" + z.getDir()
                + " fwd=" + z.getForwardDirection().x + "," + z.getForwardDirection().y
                + " look=" + z.getLookAngleRadians() + " hp=" + z.getHealth()
                + " target=" + (z.getTarget() == null ? "-" : "player") + " php=" + pl.getHealth()
                + " goal=" + ActionContextProbe.g(pfb, "goal")
                + " " + vars(z));
            prev = draws;
        }
        out.println("END " + spec + " draws=" + ActionContextProbe.g(RandStandard.INSTANCE, "id"));
        cell.getObjectList().remove(z);
        sq.getMovingObjects().remove(z);
        cell.getObjectList().remove(pl);
        psq.getMovingObjects().remove(pl);
        IsoPlayer.players[0] = null;
        cell.chunkMap[0] = null;
        GameServer.server = true;
        for (java.util.Map.Entry<String, Integer> e : saved.entrySet()) {
            ActionContextProbe.setOption(ActionContextProbe.g(SandboxOptions.instance.lore, Character.toLowerCase(e.getKey().charAt(0)) + e.getKey().substring(1)), e.getValue());
        }
    }

    public static void main(String[] a) throws Exception {
        java.util.TimeZone.setDefault(java.util.TimeZone.getTimeZone("UTC"));
        ServerHarness.run(a[0], out -> {
            for (int i = 1; i < a.length; i++) {
                if (a[i].startsWith("zai:")) scenario(out, a[i], zombie.iso.IsoWorld.instance.currentCell);
                else throw new IllegalArgumentException("unknown scenario " + a[i]);
            }
        });
    }
}
