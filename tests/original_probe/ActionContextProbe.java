import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Random;
import java.util.TimeZone;
import java.util.TreeMap;
import java.util.TreeSet;
import zombie.GameTime;
import zombie.characters.IsoZombie;
import zombie.characters.SurvivorDesc;
import zombie.characters.action.ActionContext;
import zombie.characters.action.ActionGroup;
import zombie.characters.action.ActionState;
import zombie.core.Core;
import zombie.core.random.RandAbstract;
import zombie.core.random.RandStandard;
import zombie.core.skinnedmodel.advancedanimation.AdvancedAnimator;
import zombie.core.skinnedmodel.advancedanimation.AnimCondition;
import zombie.core.skinnedmodel.advancedanimation.AnimEvent;
import zombie.core.skinnedmodel.advancedanimation.AnimLayer;
import zombie.core.skinnedmodel.advancedanimation.AnimNode;
import zombie.core.skinnedmodel.advancedanimation.AnimState;
import zombie.core.skinnedmodel.advancedanimation.AnimTransition;
import zombie.core.skinnedmodel.advancedanimation.AnimationSet;
import zombie.core.skinnedmodel.advancedanimation.AnimationVariableHandle;
import zombie.core.skinnedmodel.advancedanimation.AnimationVariableReference;
import zombie.core.skinnedmodel.advancedanimation.IAnimationVariableMap;
import zombie.core.skinnedmodel.advancedanimation.IAnimationVariableSource;
import zombie.core.skinnedmodel.advancedanimation.LiveAnimNode;
import zombie.core.skinnedmodel.advancedanimation.events.IAnimEventCallback;
import zombie.core.skinnedmodel.animation.AnimationPlayer;
import zombie.core.skinnedmodel.animation.AnimationTrack;
import zombie.core.skinnedmodel.population.OutfitRNG;
import zombie.debug.DebugOptions;
import zombie.iso.IsoCell;
import zombie.iso.IsoGridSquare;
import zombie.iso.IsoWorld;
import zombie.iso.IsoChunk;
import zombie.iso.objects.RainManager;
import zombie.network.ServerMap;
import zombie.iso.CellLoader;
import zombie.SandboxOptions;
import zombie.core.skinnedmodel.advancedanimation.AnimationVariableSource;
import zombie.core.skinnedmodel.advancedanimation.IAnimatable;
import zombie.core.skinnedmodel.advancedanimation.IAnimationVariableSlot;

// Inside the booted original dedicated server (ServerHarness): the ORIGINAL zombie.characters.action.ActionContext run
// for N ticks.
//
// Scenario "ctx:<group>:<seed>:<ticks>": an ActionContext of the action group (media/actiongroups/<group>) whose owner
// is a scripted character (a java.lang.reflect.Proxy of IAnimatable): its variables are a real AnimationVariableSource
// the script sets, consulted after the context's own state variables as IsoGameCharacter.getVariable does. The script
// is a java.util.Random(seed) walk: the variables the group's conditions read (found by reflection into the group's
// transitions, sorted by name; a variable compared with booleans is a Bool slot, with numbers a Float slot, with
// strings a String slot) and the events its conditions test are changed at random each tick; the options of a variable
// are the constants it is compared with plus a few more. Per tick, after ActionContext.update(): the root state, the
// child states, the previous state name, the state peekNextState() gives, the history depth is not printed (only what
// the original exposes), whether the states-changed callback fired, and canTransitionToState for one random state.
//
// Args: <scratch dir> <scenario>...
public class ActionContextProbe {
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

    // The scripted owner: IAnimatable as a Proxy; getVariable consults the context's state variables, then the script's.
    static final class Scripted implements InvocationHandler {
        final AnimationVariableSource vars = new AnimationVariableSource();
        ActionContext ctx;
        int changed;

        @Override public Object invoke(Object proxy, Method m, Object[] args) {
            switch (m.getName()) {
                case "getVariable":
                    if (args[0] instanceof AnimationVariableHandle h) {
                        IAnimationVariableSlot s = ctx != null ? ctx.getVariable(h) : null;
                        return s != null ? s : vars.getVariable(h);
                    }
                    return null;
                case "getUID": return "scripted";
                case "toString": return "ScriptedCharacter";
                case "hashCode": return System.identityHashCode(proxy);
                case "equals": return proxy == args[0];
                case "getActionContext": return ctx;
                case "getSubVariableSource": return null;
                default:
                    Class<?> r = m.getReturnType();
                    if (r == boolean.class) return false;
                    if (r == int.class) return 0;
                    if (r == short.class) return (short) -1;
                    if (r == float.class) return 0f;
                    return null;
            }
        }
    }

    record Var(String name, String kind, List<Object> options) {}

    static final class Domain {
        final TreeMap<String, TreeSet<String>> strs = new TreeMap<>();
        final TreeMap<String, TreeSet<Float>> nums = new TreeMap<>();
        final TreeMap<String, Boolean> bools = new TreeMap<>();
        final TreeSet<String> events = new TreeSet<>();
        final TreeSet<String> names = new TreeSet<>();
        final List<String> stateNames = new ArrayList<>();
        final List<Var> vars = new ArrayList<>();
    }

    static Domain domain(ActionGroup group) throws Exception {
        Domain d = new Domain();
        List<?> states = (List<?>) g(group, "states");
        for (Object so : states) {
            ActionState st = (ActionState) so;
            d.stateNames.add(st.getName());
            for (Object to : st.transitions) {
                List<?> conds = (List<?>) g(to, "conditions");
                for (Object c : conds) {
                    String cn = c.getClass().getSimpleName();
                    if (cn.equals("EventOccurred") || cn.equals("EventNotOccurred")) {
                        d.events.add((String) g(c, "eventName"));
                    } else if (cn.equals("CharacterVariableCondition")) {
                        Object lhs = g(c, "lhsValue");
                        Object rhs = g(c, "rhsValue");
                        if (lhs == null || !lhs.getClass().getSimpleName().equals("CharacterVariableLookup")) continue;
                        Object ref = g(lhs, "variableReference");
                        if ((String) ref.getClass().getMethod("getSubVariableSourceName").invoke(ref) != null) continue;
                        String name = (String) ref.getClass().getMethod("getName").invoke(ref);
                        d.names.add(name);
                        if (rhs instanceof Boolean) d.bools.putIfAbsent(name, true);
                        else if (rhs instanceof Integer i) d.nums.computeIfAbsent(name, k -> new TreeSet<>()).add((float) i);
                        else if (rhs instanceof Float f) d.nums.computeIfAbsent(name, k -> new TreeSet<>()).add(f);
                        else if (rhs instanceof String s) d.strs.computeIfAbsent(name, k -> new TreeSet<>()).add(s);
                    }
                }
            }
        }
        for (String name : d.names) {
            List<Object> opts = new ArrayList<>();
            if (d.strs.containsKey(name)) {
                TreeSet<String> o = new TreeSet<>(d.strs.get(name));
                o.add("");
                o.add("x");
                opts.addAll(o);
                d.vars.add(new Var(name, "string", opts));
            } else if (d.nums.containsKey(name)) {
                TreeSet<Float> o = new TreeSet<>(d.nums.get(name));
                for (Float f : new ArrayList<>(o)) o.add(f + 1.0F);
                o.add(0.0F);
                o.add(1.0F);
                opts.addAll(o);
                d.vars.add(new Var(name, "float", opts));
            } else {
                opts.add(true);
                opts.add(false);
                d.vars.add(new Var(name, "bool", opts));
            }
        }
        return d;
    }

    static String list(List<ActionState> l) {
        StringBuilder sb = new StringBuilder("[");
        for (int i = 0; i < l.size(); i++) sb.append(i > 0 ? "," : "").append(l.get(i).getName());
        return sb.append("]").toString();
    }

    static void ctxScenario(PrintStream out, String spec) throws Exception {
        String[] p = spec.split(":");
        String groupName = p[1];
        long seed = Long.parseLong(p[2]);
        int ticks = Integer.parseInt(p[3]);
        ActionGroup group = ActionGroup.getActionGroup(groupName);
        Domain d = domain(group);
        out.println("SCENARIO " + spec);
        out.println("GROUP " + groupName + " states=" + d.stateNames.size() + " vars=" + d.vars.size() + " events=" + d.events.size());
        StringBuilder sb = new StringBuilder("VARS");
        for (Var v : d.vars) sb.append(' ').append(v.name()).append(':').append(v.kind()).append('/').append(v.options().size());
        out.println(sb);
        out.println("EVENTS " + d.events);
        Scripted sc = new Scripted();
        IAnimatable owner = (IAnimatable) Proxy.newProxyInstance(ActionContextProbe.class.getClassLoader(), new Class<?>[]{IAnimatable.class}, sc);
        ActionContext ctx = new ActionContext(owner);
        sc.ctx = ctx;
        ctx.addOnStateChanged(c -> sc.changed++);
        ctx.setGroup(group);
        out.println("INIT root=" + ctx.getCurrentStateName() + " prev=" + ctx.peekPreviousStateName());
        Random r = new Random(seed);
        int nv = d.vars.size();
        int ne = d.events.size();
        List<String> events = new ArrayList<>(d.events);
        for (int t = 1; t <= ticks; t++) {
            int changes = 1 + r.nextInt(2);
            for (int c = 0; c < changes && nv > 0; c++) {
                Var v = d.vars.get(r.nextInt(nv));
                Object o = v.options().get(r.nextInt(v.options().size()));
                switch (v.kind()) {
                    case "bool" -> sc.vars.setVariable(v.name(), (Boolean) o);
                    case "float" -> sc.vars.setVariable(v.name(), (Float) o);
                    default -> sc.vars.setVariable(v.name(), (String) o);
                }
            }
            if (ne > 0 && r.nextInt(4) == 0) {
                String e = events.get(r.nextInt(ne));
                if (r.nextInt(3) == 0) ctx.reportEvent(ctx.getCurrentStateName(), e);
                else ctx.reportEvent(e);
            }
            if (r.nextInt(64) == 0) {
                String forced = d.stateNames.get(r.nextInt(d.stateNames.size()));
                ctx.setCurrentState(group.findState(forced));
            }
            String can = d.stateNames.get(r.nextInt(d.stateNames.size()));
            sc.changed = 0;
            ctx.update();
            ActionState next = ctx.peekNextState();
            out.println("T " + t + " root=" + ctx.getCurrentStateName() + " child=" + list(ctx.getChildStates()) + " prev=" + ctx.peekPreviousStateName()
                + " next=" + (next == null ? "-" : next.getName()) + " changed=" + sc.changed
                + " can(" + can + ")=" + (ctx.canTransitionToState(can) ? 1 : 0) + (ctx.canTransitionToState(can, false) ? 1 : 0)
                + " ev=" + (ne > 0 ? (ctx.hasEventOccurred(events.get(t % ne)) ? 1 : 0) : 0));
        }
        out.println("END " + spec);
    }

    // Scenario "clips": the clips of a zombie's AnimationPlayer (the human skeleton's SkinningData.animationClips), sorted
    // by name: name and duration. The zombie is only made for its player; it is not put in the world.
    static void clipsScenario(PrintStream out) throws Exception {
        zombie.characters.IsoZombie z = new zombie.characters.IsoZombie(zombie.iso.IsoWorld.instance.currentCell);
        zombie.core.skinnedmodel.animation.AnimationPlayer p = z.getAnimationPlayer();
        out.println("SCENARIO clips");
        out.println("READY " + p.isReady());
        TreeMap<String, zombie.core.skinnedmodel.animation.AnimationClip> m = new TreeMap<>(p.getSkinningData().animationClips);
        out.println("CLIPS " + m.size());
        for (Map.Entry<String, zombie.core.skinnedmodel.animation.AnimationClip> e : m.entrySet()) {
            out.println("CLIP " + e.getKey() + " " + e.getValue().name + " " + e.getValue().getDuration());
        }
        out.println("END clips");
    }

    static String f(Object v) { return Float.toString((Float) v); }
    static int b(Object v) { return (Boolean) v ? 1 : 0; }

    // The animator's state as the animation variables and the draws leave it: per layer (the root, then the sub layers in
    // the order the animator keeps them) its depth, state and live nodes; then the tracks of the player's multi track.
    static void dumpAnimator(PrintStream out, AdvancedAnimator aa, AnimationPlayer p) throws Exception {
        List<AnimLayer> layers = new ArrayList<>();
        layers.add((AnimLayer) g(aa, "rootLayer"));
        for (Object slot : (List<?>) g(aa, "subLayers")) layers.add((AnimLayer) g(slot, "animLayer"));
        for (AnimLayer l : layers) {
            out.println(" L " + l.getDepth() + " state=" + l.getCurrentStateName() + " nodes=" + l.getLiveAnimNodes().size());
            for (LiveAnimNode n : l.getLiveAnimNodes()) {
                out.println("  N " + n.getName() + " anim=" + n.getAnimName() + " w=" + f(n.getWeight()) + " raw=" + f(g(n, "rawWeight"))
                    + " act=" + b(g(n, "active")) + " in=" + b(n.isBlendingIn()) + " out=" + b(n.isBlendingOut()) + " tout=" + b(g(n, "transitioningOut"))
                    + " new=" + b(n.isNew()) + " bf=" + b(n.isBlendField) + " t=" + f(g(n, "nodeAnimTime")) + " tin=" + b(n.isTransitioningIn())
                    + " tw=" + f(n.getTransitionInWeight()) + " from=" + n.getTransitionFrom()
                    + " mt=" + ((List<?>) g(n, "animationTracks")).size() + " flags=" + n.getWhileAliveFlags().size());
            }
        }
        int i = 0;
        for (AnimationTrack t : p.getMultiTrack().getTracks()) {
            out.println(" R " + (i++) + " " + t.getName() + " time=" + f(t.getCurrentTimeValue()) + " speed=" + f(t.getSpeedDelta()) + " bw=" + f(t.getBlendWeight())
                + " fw=" + f(t.getBlendFieldWeight()) + " play=" + (t.isPlaying ? 1 : 0) + " loop=" + (t.isLooping() ? 1 : 0) + " prim=" + (t.isPrimary ? 1 : 0)
                + " prio=" + t.getPriority());
        }
    }

    static void options(PrintStream out) {
        out.println("OPTS debug=" + Core.debug + " earlyOut=" + (DebugOptions.instance.animation.allowEarlyTransitionOut.getValue() ? 1 : 0)
            + " noBlends=" + (DebugOptions.instance.animation.disableAnimationBlends.getValue() ? 1 : 0)
            + " physicsHit=" + (DebugOptions.instance.character.debug.ragdoll.physics.physicsHitReaction.getValue() ? 1 : 0)
            + " logStates=" + (DebugOptions.instance.animation.animLayer.logStateChanges.getValue() ? 1 : 0)
            + " override=" + (DebugOptions.instance.animation.animLayer.allowAnimNodeOverride.getValue() ? 1 : 0));
    }

    // ---- the scripted animator character -------------------------------------------------------------------------
    // IAnimatable + IAnimationVariableMap + IAnimEventCallback as a Proxy: the variables are a real AnimationVariableSource
    // (the animator's while-alive flags and maxTwist write into it), the events are logged, the player is a zombie's.
    static final class AnimScript implements InvocationHandler {
        final AnimationVariableSource vars = new AnimationVariableSource();
        AdvancedAnimator animator;
        AnimationPlayer player;
        String setName;
        final StringBuilder events = new StringBuilder();

        @Override public Object invoke(Object proxy, Method m, Object[] args) throws Throwable {
            Class<?> dc = m.getDeclaringClass();
            if (dc == IAnimationVariableSource.class || dc == IAnimationVariableMap.class) {
                try { return m.invoke(vars, args); } catch (InvocationTargetException e) { throw e.getCause(); }
            }
            switch (m.getName()) {
                case "OnAnimEvent": {
                    AnimLayer layer = (AnimLayer) args[0];
                    AnimationTrack track = (AnimationTrack) args[1];
                    AnimEvent e = (AnimEvent) args[2];
                    events.append(" EV ").append(AnimLayer.getDepth(layer)).append(' ').append(AnimLayer.getCurrentStateName(layer)).append(' ')
                        .append(e.eventName).append(' ').append(e.parameterValue).append(' ').append(track == null ? "null" : track.getName()).append('\n');
                    return null;
                }
                case "getAnimationPlayer": return player;
                case "getAdvancedAnimator": return animator;
                case "GetAnimSetName": return setName;
                case "hasAnimationPlayer": return true;
                case "getUID": return "scripted";
                case "toString": return "ScriptedAnimatable";
                case "hashCode": return System.identityHashCode(proxy);
                case "equals": return proxy == args[0];
                default:
                    Class<?> r = m.getReturnType();
                    if (r == boolean.class) return false;
                    if (r == int.class) return 0;
                    if (r == short.class) return (short) -1;
                    if (r == float.class) return 0f;
                    return null;
            }
        }
    }

    static int cmpStr(String a, String b) { return a.compareTo(b); }

    static final float[] DTS = {1.0F / 60.0F, 1.0F / 30.0F, 0.083333336F, 0.05F, 0.016666668F, 0.2F, 0.5F, 0.033333335F};

    // Scenario "anim:<set>:<seed>:<ticks>[:state,state,...]": the real AdvancedAnimator of the animation set <set> on the scripted character.
    // The script is a java.util.Random(seed) walk over the variables the set's conditions read (every condition of every
    // node and transition; a variable compared with a string is a String slot, with a number a Float slot, else a Bool
    // slot) and over the states that need no ragdoll or node blend curve. The global Rand is seeded with the seed, so the
    // draws of the animator (random anims, random advance, blend field starts) are checked by the draw counter.
    static void animScenario(PrintStream out, String spec, IsoZombie carrier) throws Exception {
        String[] p = spec.split(":");
        String setName = p[1];
        long seed = Long.parseLong(p[2]);
        int ticks = Integer.parseInt(p[3]);
        AnimationSet set = AnimationSet.GetAnimationSet(setName, false);
        // the domain
        TreeMap<String, TreeSet<String>> strs = new TreeMap<>(ActionContextProbe::cmpStr);
        TreeMap<String, TreeSet<Float>> nums = new TreeMap<>(ActionContextProbe::cmpStr);
        TreeSet<String> bools = new TreeSet<>();
        TreeSet<String> names = new TreeSet<>();
        TreeSet<String> stateKeys = new TreeSet<>();
        HashMapView states = new HashMapView((java.util.Map<String, AnimState>) g(set, "states"));
        for (String key : states.keys()) {
            AnimState st = states.get(key);
            boolean ok = true;
            for (AnimNode n : st.nodes) {
                if (n.isRagdoll || n.blendCurve != null) ok = false;
                List<AnimCondition> conds = new ArrayList<>(List.of(n.conditions));
                for (AnimTransition t : n.transitions) conds.addAll(List.of(t.conditions));
                for (AnimCondition c : conds) {
                    if (c.type == null || c.type == AnimCondition.Type.OR) continue;
                    AnimationVariableReference ref = AnimationVariableReference.fromRawVariableName(c.name);
                    if (ref.isSubVariableSourceReference() || ref.getName().isBlank()) continue;
                    names.add(c.name);
                    switch (c.type) {
                        case STRING, STRNEQ -> strs.computeIfAbsent(c.name, k -> new TreeSet<>(ActionContextProbe::cmpStr)).add(c.stringValue);
                        case BOOL -> bools.add(c.name);
                        default -> nums.computeIfAbsent(c.name, k -> new TreeSet<>()).add(c.floatValue);
                    }
                }
            }
            if (ok) stateKeys.add(key);
        }
        List<Var> vars = new ArrayList<>();
        for (String name : names) {
            List<Object> opts = new ArrayList<>();
            if (strs.containsKey(name)) {
                TreeSet<String> o = new TreeSet<>(ActionContextProbe::cmpStr);
                o.addAll(strs.get(name));
                o.add("");
                o.add("x");
                opts.addAll(o);
                vars.add(new Var(name, "string", opts));
            } else if (nums.containsKey(name)) {
                TreeSet<Float> o = new TreeSet<>(nums.get(name));
                for (Float fv : new ArrayList<>(o)) o.add(fv + 1.0F);
                o.add(0.0F);
                o.add(1.0F);
                opts.addAll(o);
                vars.add(new Var(name, "float", opts));
            } else {
                opts.add(true);
                opts.add(false);
                vars.add(new Var(name, "bool", opts));
            }
        }
        List<String> stateList = new ArrayList<>(stateKeys);
        if (p.length > 4) {
            // "...:state,state": only these states (lower case, as the set keys them) are chosen
            List<String> only = List.of(p[4].split(","));
            stateList.removeIf(k -> !only.contains(k));
        }
        out.println("SCENARIO " + spec);
        out.println("SET " + setName + " states=" + states.keys().size() + " usable=" + stateList.size() + " vars=" + vars.size());
        options(out);
        AnimScript sc = new AnimScript();
        sc.setName = setName;
        sc.player = carrier.getAnimationPlayer();
        sc.player.reset();
        sc.animator = new AdvancedAnimator();
        IAnimatable owner = (IAnimatable) Proxy.newProxyInstance(ActionContextProbe.class.getClassLoader(),
            new Class<?>[]{IAnimatable.class, IAnimationVariableMap.class, IAnimEventCallback.class}, sc);
        sc.animator.init(owner, setName);
        if (seed % 2 == 0) sc.vars.setVariable("maxTwist", 0.0F);
        Class<?> randClass = RandAbstract.class;
        java.lang.reflect.Field rf = randClass.getDeclaredField("rand");
        rf.setAccessible(true);
        rf.set(RandStandard.INSTANCE, new Random(seed));
        java.lang.reflect.Field idf = randClass.getDeclaredField("id");
        idf.setAccessible(true);
        idf.set(RandStandard.INSTANCE, 0);
        Random r = new Random(seed);
        int nv = vars.size();
        int ns = stateList.size();
        int prev = 0;
        for (int t = 1; t <= ticks; t++) {
            int changes = 1 + r.nextInt(3);
            for (int c = 0; c < changes && nv > 0; c++) {
                Var v = vars.get(r.nextInt(nv));
                Object o = v.options().get(r.nextInt(v.options().size()));
                switch (v.kind()) {
                    case "bool" -> sc.vars.setVariable(v.name(), (Boolean) o);
                    case "float" -> sc.vars.setVariable(v.name(), (Float) o);
                    default -> sc.vars.setVariable(v.name(), (String) o);
                }
            }
            if (nv > 0 && r.nextInt(16) == 0) sc.vars.clearVariable(vars.get(r.nextInt(nv)).name());
            String did = "-";
            if (ns > 0 && r.nextInt(5) == 0) {
                String root = r.nextInt(25) == 0 ? "bogus" : stateList.get(r.nextInt(ns));
                int nsub = r.nextInt(3);
                List<String> subs = new ArrayList<>();
                for (int i = 0; i < nsub; i++) subs.add(r.nextInt(25) == 0 ? "bogus" : stateList.get(r.nextInt(ns)));
                sc.animator.setState(root, subs);
                did = root + subs;
            }
            if (r.nextInt(97) == 0) {
                sc.animator.reset();
                did = "reset";
            }
            float dt = DTS[r.nextInt(DTS.length)];
            sc.events.setLength(0);
            sc.animator.update(dt);
            sc.player.Update(dt);
            int draws = (Integer) g(RandStandard.INSTANCE, "id");
            out.println("T " + t + " dt=" + dt + " draws=" + (draws - prev) + " set=" + did + " state=" + sc.animator.getCurrentStateName()
                + (sc.vars.getVariable("maxTwist") != null ? " maxTwist=" + sc.vars.getVariable("maxTwist").getValueFloat() : ""));
            prev = draws;
            out.print(sc.events);
            dumpAnimator(out, sc.animator, sc.player);
        }
        out.println("END " + spec + " draws=" + g(RandStandard.INSTANCE, "id"));
    }

    record HashMapView(java.util.Map<String, AnimState> m) {
        List<String> keys() { return new ArrayList<>(new TreeSet<>(m.keySet())); }
        AnimState get(String k) { return m.get(k); }
    }

    // ---- the real zombie ---------------------------------------------------------------------------------------
    static final java.util.LinkedHashMap<String, Integer> BASE = new java.util.LinkedHashMap<>();
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

    static void loadChunks(IsoCell cell) throws Exception {
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

    static boolean chunksLoaded;

    // Scenario "zanim:<seed>:<ticks>": the ZombieProbe zombie of that seed (the base lore options), alone on the square
    // (8280, 11670, 0), preupdate/update/postupdate per tick as the server's world does; per tick the animation time
    // delta, the draws of the global Rand, the action context's state and the animator's layers, nodes and tracks.
    static void zombieAnimScenario(PrintStream out, String spec, IsoCell cell) throws Exception {
        String[] p = spec.split(":");
        long seed = Long.parseLong(p[1]);
        int ticks = Integer.parseInt(p[2]);
        if (!chunksLoaded) {
            loadChunks(cell);
            chunksLoaded = true;
        }
        java.util.Map<String, Integer> saved = new java.util.LinkedHashMap<>();
        for (java.util.Map.Entry<String, Integer> e : BASE.entrySet()) {
            Object opt = g(SandboxOptions.instance.lore, Character.toLowerCase(e.getKey().charAt(0)) + e.getKey().substring(1));
            saved.put(e.getKey(), (Integer) opt.getClass().getMethod("getValue").invoke(opt));
            setOption(opt, e.getValue());
        }
        out.println("SCENARIO " + spec);
        options(out);
        java.lang.reflect.Field rf = RandAbstract.class.getDeclaredField("rand");
        rf.setAccessible(true);
        rf.set(RandStandard.INSTANCE, new Random(seed));
        java.lang.reflect.Field idf = RandAbstract.class.getDeclaredField("id");
        idf.setAccessible(true);
        idf.set(RandStandard.INSTANCE, 0);
        OutfitRNG.setSeed(seed);
        IsoZombie z = new IsoZombie(cell);
        out.println("ZOMBIE draws=" + g(RandStandard.INSTANCE, "id") + " group=" + z.getActionContext().getGroup().getName() + " animSet=" + z.getAdvancedAnimator().animSet.name);
        IsoGridSquare sq = cell.getGridSquare(8280, 11670, 0);
        z.setCurrent(sq);
        z.setMovingSquareNow();
        z.setX(8280.5f);
        z.setY(11670.5f);
        z.setZ(0);
        cell.getObjectList().add(z);
        int prev = (Integer) g(RandStandard.INSTANCE, "id");
        for (int n = 1; n <= ticks; n++) {
            z.preupdate();
            z.update();
            z.postupdate();
            int draws = (Integer) g(RandStandard.INSTANCE, "id");
            StringBuilder sb = new StringBuilder("T " + n + " dt=" + f(z.getAnimationTimeDelta()) + " upd=" + (z.isAnimationUpdatingThisFrame() ? 1 : 0)
                + " draws=" + (draws - prev) + " ctx=" + z.getActionContext().getCurrentStateName() + " child=" + list(z.getActionContext().getChildStates())
                + " ai=" + (z.getCurrentState() == null ? "null" : z.getCurrentState().getClass().getSimpleName())
                + " vars=" + gameVars(z));
            out.println(sb);
            dumpAnimator(out, z.getAdvancedAnimator(), z.getAnimationPlayer());
            prev = draws;
        }
        out.println("END " + spec + " draws=" + g(RandStandard.INSTANCE, "id"));
        cell.getObjectList().remove(z);
        sq.getMovingObjects().remove(z);
        for (java.util.Map.Entry<String, Integer> e : saved.entrySet()) {
            setOption(g(SandboxOptions.instance.lore, Character.toLowerCase(e.getKey().charAt(0)) + e.getKey().substring(1)), e.getValue());
        }
    }

    // The variables the zombie has set (its game variables and the action context's), by name: count of the game variables.
    static String gameVars(IsoZombie z) {
        int n = 0;
        for (Object o : z.getGameVariables()) n++;
        return Integer.toString(n);
    }

    public static void main(String[] a) throws Exception {
        TimeZone.setDefault(TimeZone.getTimeZone("UTC"));
        ServerHarness.run(a[0], out -> {
            for (int i = 1; i < a.length; i++) {
                if (a[i].startsWith("ctx:")) ctxScenario(out, a[i]);
                else if (a[i].equals("clips")) clipsScenario(out);
                else if (a[i].startsWith("anim:")) animScenario(out, a[i], new zombie.characters.IsoZombie(zombie.iso.IsoWorld.instance.currentCell));
                else if (a[i].startsWith("zanim:")) zombieAnimScenario(out, a[i], zombie.iso.IsoWorld.instance.currentCell);
                else throw new IllegalArgumentException("unknown scenario " + a[i]);
            }
        });
    }
}
