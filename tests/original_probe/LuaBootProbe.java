import java.io.PrintStream;
import java.util.Map;
import java.util.TreeMap;
import se.krka.kahlua.vm.LuaClosure;
import zombie.Lua.Event;
import zombie.Lua.LuaEventManager;
import zombie.Lua.LuaManager;

/** Oracle probe (tools/oracle.sh luaboot): LuaLoadEnvProbe's load (LuaManager.init, LoadDirBase of each directory
 * given), then what the ORIGINAL game runs next on its way to the main menu, in its order (GameWindow.init after
 * LoadDirBase, GameWindow.enter, the states GameWindow.initShared queues), as far as it runs standalone:
 *   ZomboidGlobals.Load()                              GameWindow.init
 *   triggerEvent("OnLoadSoundBanks")                   GameWindow.init
 *   triggerEvent("OnGameBoot")                         GameWindow.enter (after the texture packs and models)
 *   triggerEvent("OnGameStateEnter", <state>)          TermsOfServiceState.enter (TISLogoState.enter fires nothing),
 *                                                      then the rest of enter()
 *   triggerEvent("OnMainMenuEnter")                    MainScreenState.enter
 * Each event runs as Event.trigger does (every callback in order through LuaManager.caller on LuaManager.thread, the
 * index stepped back when a callback removed itself), through caller.pcall instead of protectedCallVoid so the probe
 * sees each callback's result. Then the same report as LuaLoadEnvProbe (globals, events, Java state), then:
 *   BOOT <event> <n>           each event fired, with its callbacks: `true` or `false` and the error message
 *   ZOMBOIDGLOBALS <field> <bits>   ZomboidGlobals' static numbers (double/float raw bits), by name
 *   OBSERVATION ...            ObservationFactory.observationMap, by key
 *   TROUSERCOLOR / HAIRCOLOR   SurvivorDesc's common colour lists (r g b a raw float bits)
 *   ITEMSTAG <tag> <n>         ScriptManager's tagToItemMap (the lists getItemsTag made), by the tag's toString
 *   TOSSTATE created exit      the TermsOfServiceState handed to OnGameStateEnter, after its enter()
 *   CLOSURE U<n> <path> <k>    each closure printed so far that has upvalues (n: the order first printed, path:
 *                              where), then its k upvalues as U<n>[j]; closures met there join the list
 * Args: <scratch cache dir> <dir>... ; run from the game root. */
public class LuaBootProbe {
    static final StringBuilder boot = new StringBuilder();

    /** zombie.Lua.Event.trigger's loop for the event `name` (LuaEventManager.checkEvent: no callbacks, no call). */
    static void fire(String name, Object... args) {
        Event e = LuaEventManager.AddEvent(name);
        boot.append("BOOT ").append(name).append(' ').append(e.callbacks.size()).append('\n');
        for (int n = 0; n < e.callbacks.size(); n++) {
            LuaClosure c = e.callbacks.get(n);
            Object[] r = LuaManager.caller.pcall(LuaManager.thread, c, args.clone());
            boot.append("  ").append(LuaLoadEnvProbe.value(c)).append(' ').append(r[0]);
            if (r[0] != Boolean.TRUE) boot.append(' ').append(LuaLoadEnvProbe.value(r[1]));
            boot.append('\n');
            if (!e.callbacks.contains(c)) n--;
        }
    }

    static String bits(Object o) {
        if (o instanceof Double d) return Long.toHexString(Double.doubleToRawLongBits(d));
        return Integer.toHexString(Float.floatToRawIntBits((Float) o));
    }

    public static void main(String[] a) throws Exception {
        PrintStream out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, "UTF-8");
        System.setOut(System.err);
        LuaLoadEnvProbe.load(a);
        zombie.gameStates.TermsOfServiceState tos = events(null);
        LuaLoadEnvProbe.report();
        report(tos);
        out.print(LuaLoadEnvProbe.sb);
        out.flush();
        Runtime.getRuntime().halt(0);
    }

    interface Hook { void run() throws Exception; }

    /** ZomboidGlobals.Load and the boot events, after LoadDirBase, with `enter` (GameWindow.enter's steps before its
     * OnGameBoot; null for none) before OnGameBoot; returns the TermsOfServiceState handed out. */
    static zombie.gameStates.TermsOfServiceState events(Hook enter) throws Exception {
        // a debugging aid (pz-dump reads the same variable): LUABOOT_STOP=<event> stops after that event,
        // LUABOOT_STOP=load before ZomboidGlobals.Load
        String stop = System.getenv("LUABOOT_STOP");
        zombie.gameStates.TermsOfServiceState tos = new zombie.gameStates.TermsOfServiceState();
        if ("load".equals(stop)) return tos;
        zombie.ZomboidGlobals.Load();
        fire("OnLoadSoundBanks");
        if ("OnLoadSoundBanks".equals(stop)) return tos;
        if (enter != null) enter.run();
        fire("OnGameBoot");
        if ("OnGameBoot".equals(stop)) return tos;
        fire("OnGameStateEnter", tos);
        // the rest of TermsOfServiceState.enter, the original's code: its triggerEvent finds no callbacks (checkEvent
        // gives null) while they are set aside, as fire ran them already
        Event tosEvent = LuaEventManager.AddEvent("OnGameStateEnter");
        java.util.ArrayList<LuaClosure> saved = new java.util.ArrayList<>(tosEvent.callbacks);
        tosEvent.callbacks.clear();
        tos.enter();
        tosEvent.callbacks.addAll(saved);
        if ("OnGameStateEnter".equals(stop)) return tos;
        fire("OnMainMenuEnter");
        return tos;
    }

    /** The boot lines and Java state after LuaLoadEnvProbe.report (see the class comment). */
    static void report(zombie.gameStates.TermsOfServiceState tos) throws Exception {
        StringBuilder sb = LuaLoadEnvProbe.sb;
        sb.append(boot);
        TreeMap<String, java.lang.reflect.Field> zg = new TreeMap<>();
        for (java.lang.reflect.Field f : zombie.ZomboidGlobals.class.getDeclaredFields())
            if (java.lang.reflect.Modifier.isStatic(f.getModifiers()) && (f.getType() == double.class || f.getType() == float.class)) zg.put(f.getName(), f);
        for (Map.Entry<String, java.lang.reflect.Field> e : zg.entrySet())
            sb.append("ZOMBOIDGLOBALS ").append(e.getKey()).append(' ').append(bits(e.getValue().get(null))).append('\n');
        for (Map.Entry<String, zombie.characters.traits.ObservationFactory.Observation> e
            : new TreeMap<>(zombie.characters.traits.ObservationFactory.observationMap).entrySet()) {
            zombie.characters.traits.ObservationFactory.Observation o = e.getValue();
            sb.append("OBSERVATION ").append(LuaLoadEnvProbe.value(e.getKey())).append(' ').append(LuaLoadEnvProbe.value(o.getTraitID())).append(' ')
                .append(LuaLoadEnvProbe.value(o.getName())).append(' ').append(LuaLoadEnvProbe.value(o.getDescription())).append(' ')
                .append(LuaLoadEnvProbe.list(o.mutuallyExclusive)).append('\n');
        }
        for (zombie.core.Color c : zombie.characters.SurvivorDesc.TrouserCommonColors)
            sb.append("TROUSERCOLOR ").append(bits(c.r)).append(' ').append(bits(c.g)).append(' ').append(bits(c.b)).append(' ').append(bits(c.a)).append('\n');
        for (zombie.core.ImmutableColor c : zombie.characters.SurvivorDesc.HairCommonColors)
            sb.append("HAIRCOLOR ").append(bits(c.r)).append(' ').append(bits(c.g)).append(' ').append(bits(c.b)).append(' ').append(bits(c.a)).append('\n');
        TreeMap<String, Integer> tags = new TreeMap<>();
        for (Map.Entry<?, ?> e : ((Map<?, ?>) LuaLoadEnvProbe.field(zombie.scripting.ScriptManager.instance, "tagToItemMap")).entrySet())
            tags.put(String.valueOf(e.getKey()), ((java.util.List<?>) e.getValue()).size());
        for (Map.Entry<String, Integer> e : tags.entrySet())
            sb.append("ITEMSTAG ").append(LuaLoadEnvProbe.value(e.getKey())).append(' ').append(e.getValue()).append('\n');
        sb.append("TOSSTATE ").append(LuaLoadEnvProbe.field(tos, "created")).append(' ').append(LuaLoadEnvProbe.field(tos, "exit")).append('\n');
        // the upvalues of every closure printed above (where the scripts keep local state: ISMoveableDefinitions'
        // `instance`, ...), then of the closures found in them, in the order first printed
        for (int n = 0; n < LuaLoadEnvProbe.closures.size(); n++) {
            se.krka.kahlua.vm.UpValue[] ups = LuaLoadEnvProbe.closures.get(n).upvalues;
            if (ups.length == 0) continue;
            sb.append("CLOSURE U").append(n).append(' ').append(LuaLoadEnvProbe.closurePaths.get(n)).append(' ').append(ups.length).append('\n');
            for (int j = 0; j < ups.length; j++) LuaLoadEnvProbe.dump("U" + n + "[" + j + "]", ups[j].getValue());
        }
    }
}
