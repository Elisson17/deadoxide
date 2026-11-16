import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Random;
import java.util.TimeZone;
import java.util.TreeMap;
import java.util.TreeSet;
import zombie.characters.action.ActionContext;
import zombie.characters.action.ActionGroup;
import zombie.characters.action.ActionState;
import zombie.core.skinnedmodel.advancedanimation.AnimationVariableHandle;
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

    public static void main(String[] a) throws Exception {
        TimeZone.setDefault(TimeZone.getTimeZone("UTC"));
        ServerHarness.run(a[0], out -> {
            for (int i = 1; i < a.length; i++) {
                if (a[i].startsWith("ctx:")) ctxScenario(out, a[i]);
                else if (a[i].equals("clips")) clipsScenario(out);
                else throw new IllegalArgumentException("unknown scenario " + a[i]);
            }
        });
    }
}
