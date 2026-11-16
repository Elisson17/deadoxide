import java.io.File;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import zombie.ZomboidFileSystem;
import zombie.asset.Asset;
import zombie.characters.action.ActionGroup;
import zombie.characters.action.ActionState;
import zombie.characters.action.ActionTransition;
import zombie.characters.action.IActionCondition;
import zombie.characters.action.conditions.CharacterVariableCondition;
import zombie.characters.action.conditions.EventNotOccurred;
import zombie.characters.action.conditions.EventOccurred;
import zombie.core.math.IInterpolator;
import zombie.core.math.interpolators.LerpEaseInQuad;
import zombie.core.math.interpolators.LerpEaseOutInQuad;
import zombie.core.math.interpolators.LerpEaseOutQuad;
import zombie.core.math.interpolators.LerpLinear;
import zombie.core.math.interpolators.LerpPolyline;
import zombie.core.math.interpolators.LerpSequence;
import zombie.core.skinnedmodel.advancedanimation.Anim2DBlend;
import zombie.core.skinnedmodel.advancedanimation.Anim2DBlendPicker;
import zombie.core.skinnedmodel.advancedanimation.Anim2DBlendTriangle;
import zombie.core.skinnedmodel.advancedanimation.AnimBoneWeight;
import zombie.core.skinnedmodel.advancedanimation.AnimCondition;
import zombie.core.skinnedmodel.advancedanimation.AnimEvent;
import zombie.core.skinnedmodel.advancedanimation.AnimEventFlagWhileAlive;
import zombie.core.skinnedmodel.advancedanimation.AnimEventSetVariable;
import zombie.core.skinnedmodel.advancedanimation.AnimNode;
import zombie.core.skinnedmodel.advancedanimation.AnimNodeAssetManager;
import zombie.core.skinnedmodel.advancedanimation.AnimState;
import zombie.core.skinnedmodel.advancedanimation.AnimTransition;
import zombie.core.skinnedmodel.advancedanimation.AnimationSet;
import zombie.core.skinnedmodel.advancedanimation.AnimationVariableReference;
import zombie.core.skinnedmodel.advancedanimation.IAnimationVariableSlot;

/** Oracle probe (tools/oracle.sh animsets): the ORIGINAL AnimationSet.GetAnimationSet for every directory of
 * media/AnimSets and ActionGroup.getActionGroup for every directory of media/actiongroups (in name order), standalone
 * (no world, no mods; ActionContext's class initialiser registers the condition factories first, as a character does
 * in the game). Printed: the sets in key order (AnimationSet.setMap is a HashMap), each set's states in key order
 * (AnimationSet.states is a HashMap), each state's nodes and abstract nodes in list order with every field (private
 * ones by reflection), then the AnimNode assets that failed to load; then the action groups in key order with their
 * states in list order, every transition and condition, and both lookups in key order. Strings are quoted with
 * non-ASCII as \\uXXXX, null unquoted; floats are Float.toString. The 2D blend picker's hull is printed as a set (its
 * order comes from a HashSet of identity hashes). Compared with `pz-dump animsets`.
 * Args: <scratch cache dir>; run from the game root. */
public class AnimSetsProbe {
    static final StringBuilder sb = new StringBuilder();

    static String q(String s) {
        if (s == null) return "null";
        StringBuilder b = new StringBuilder("\"");
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c >= 0x20 && c < 0x7f && c != '"' && c != '\\') b.append(c);
            else b.append(String.format("\\u%04x", (int) c));
        }
        return b.append('"').toString();
    }

    static String f(float v) {
        return Float.toString(v);
    }

    static int b(boolean v) {
        return v ? 1 : 0;
    }

    static String e(Enum<?> v) {
        return v == null ? "null" : v.name();
    }

    static Object field(Object owner, Class<?> c, String name) throws Exception {
        Field fd = c.getDeclaredField(name);
        fd.setAccessible(true);
        return fd.get(owner);
    }

    static Object field(Object owner, String name) throws Exception {
        return field(owner, owner.getClass(), name);
    }

    static String strings(List<String> l) {
        StringBuilder r = new StringBuilder("[");
        for (int i = 0; i < l.size(); i++) r.append(i == 0 ? "" : ",").append(q(l.get(i)));
        return r.append(']').toString();
    }

    static String strings(String[] a) {
        return a == null ? "null" : strings(Arrays.asList(a));
    }

    static String ref(AnimationVariableReference r) throws Exception {
        if (r == null) return "null";
        return "(" + q((String) field(r, "subVariableSourceName")) + "," + q((String) field(r, "name")) + ")";
    }

    static <T> String indexOf(List<T> l, T o) {
        if (o == null) return "null";
        for (int i = 0; i < l.size(); i++) if (l.get(i) == o) return Integer.toString(i);
        return "?";
    }

    static void cond(String indent, String tag, int i, AnimCondition c) throws Exception {
        sb.append(indent).append(tag).append(' ').append(i).append(" name=").append(q(c.name)).append(" type=").append(e(c.type))
            .append(" value=").append(q(c.value)).append(" f=").append(f(c.floatValue)).append(" b=").append(b(c.boolValue))
            .append(" s=").append(q(c.stringValue)).append(" ref=").append(ref((AnimationVariableReference) field(c, "variableReference")))
            .append('\n');
    }

    static void interpolator(IInterpolator in) throws Exception {
        if (in == null) {
            sb.append("null\n");
            return;
        }
        sb.append(in.getClass().getSimpleName());
        if (in instanceof LerpSequence s) {
            sb.append(" n=").append(field(s, "numEntries")).append(" min=").append(f((Float) field(s, "minX"))).append(" max=")
                .append(f((Float) field(s, "maxX"))).append(" raw=[");
            for (int i = 0; i < s.sequenceRaw.size(); i++) {
                LerpSequence.LerpSequenceEntry en = s.sequenceRaw.get(i);
                sb.append(i == 0 ? "" : ",").append(f(en.x)).append(':').append(f(en.y)).append(':').append(e(en.lerpType));
            }
            sb.append("] sorted=[");
            LerpSequence.LerpSequenceEntry[] sorted = (LerpSequence.LerpSequenceEntry[]) field(s, "sequenceEntries");
            for (int i = 0; sorted != null && i < sorted.length; i++) sb.append(i == 0 ? "" : ",").append(indexOf(s.sequenceRaw, sorted[i]));
            sb.append(']');
        } else if (in instanceof LerpPolyline p) {
            sb.append(" n=").append(field(p, "numPoints")).append(" min=").append(f((Float) field(p, "minX"))).append(" max=")
                .append(f((Float) field(p, "maxX"))).append(" raw=[");
            for (int i = 0; i < p.pointsRaw.size(); i++) sb.append(i == 0 ? "" : ",").append(f(p.pointsRaw.get(i).x)).append(':').append(f(p.pointsRaw.get(i).y));
            sb.append("] points=[");
            float[][] pts = (float[][]) field(p, "points");
            for (int i = 0; pts != null && i < pts.length; i++) sb.append(i == 0 ? "" : ",").append(f(pts[i][0])).append(':').append(f(pts[i][1]));
            sb.append(']');
        } else if (in instanceof LerpLinear l) {
            sb.append(" y0=").append(f(l.startValue)).append(" y1=").append(f(l.endValue));
        } else if (in instanceof LerpEaseInQuad l) {
            sb.append(" y0=").append(f(l.startValue)).append(" y1=").append(f(l.endValue));
        } else if (in instanceof LerpEaseOutInQuad l) {
            sb.append(" y0=").append(f(l.startValue)).append(" y1=").append(f(l.endValue));
        } else if (in instanceof LerpEaseOutQuad l) {
            sb.append(" y0=").append(f(l.startValue)).append(" y1=").append(f(l.endValue));
        }
        sb.append('\n');
    }

    @SuppressWarnings("unchecked")
    static void node(String tag, AnimNode n, AnimState st) throws Exception {
        sb.append("  ").append(tag).append(' ').append(q(n.name)).append(" idle=").append(b(n.isIdleAnim())).append(" priority=").append(n.priority)
            .append(" conditionPriority=").append(n.conditionPriority).append(" abstract=").append(b(n.isAbstract())).append(" parent=")
            .append(n.parentState == st ? "1" : n.parentState == null ? "null" : "?").append('\n');
        sb.append("   anim=").append(q(n.animName)).append(" alternates=").append(strings(n.alternateAnims)).append('\n');
        sb.append("   grapple matching=").append(q(n.matchingGrappledAnimNode)).append(" fwd=").append(f(n.grappleOffsetForward)).append(" yaw=")
            .append(f(n.grappleOffsetYaw)).append(" tweenIn=").append(f(n.grappleTweenInTime)).append(" behaviour=").append(e(n.grapplerOffsetBehaviour))
            .append('\n');
        sb.append("   ragdoll=").append(b(n.isRagdoll)).append(" chance=").append(f(n.chanceToRagdoll)).append(" startMin=").append(f(n.ragdollStartTimeMin))
            .append(" startMax=").append(f(n.ragdollStartTimeMax)).append(" maxTime=").append(f(n.ragdollMaxTime)).append('\n');
        sb.append("   deferred bone=").append(q(n.deferredBoneName)).append(" axis=").append(e(n.deferredBoneAxis)).append(" rotation=")
            .append(b(n.useDeferedRotation)).append(" movement=").append(b(n.useDeferredMovement)).append(" rotationScale=")
            .append(f(n.deferredRotationScale)).append('\n');
        sb.append("   looped=").append(b(n.isLooped)).append(" blendTime=").append(f(n.blendTime)).append(" blendOutTime=").append(f(n.blendOutTime))
            .append(" stopOnExit=").append(b(n.stopAnimOnExit)).append(" earlyTransitionOut=").append(b(n.earlyTransitionOut)).append('\n');
        sb.append("   speedScale=").append(q(n.speedScale)).append(" variable=").append(q(n.speedScaleVariable)).append(" randomMin=")
            .append(f(n.speedScaleRandomMultiplierMin)).append(" randomMax=").append(f(n.speedScaleRandomMultiplierMax)).append(" parsed=")
            .append(f((Float) field(n, "speedScaleF"))).append('\n');
        sb.append("   randomAdvance=").append(f(n.randomAdvanceFraction)).append(" maxTorsoTwist=").append(f(n.maxTorsoTwist)).append(" scalar=")
            .append(q(n.scalar)).append(" scalar2=").append(q(n.scalar2)).append(" reverse=").append(b(n.isAnimReverse)).append(" sync=")
            .append(b(n.syncTrackingEnabled)).append(" trackTimeTo=").append(q(n.trackTimeToVariable)).append('\n');
        sb.append("   blendCurve ");
        interpolator(n.blendCurve == null ? null : n.blendCurve.interpolator);
        if (n.blendCurve != null && n.blendCurve.interpolator == null) sb.append("   blendCurveSlot empty\n");
        for (int i = 0; i < n.blends2d.size(); i++) {
            Anim2DBlend bl = n.blends2d.get(i);
            sb.append("   blend ").append(i).append(" anim=").append(q(bl.animName)).append(" x=").append(f(bl.posX)).append(" y=").append(f(bl.posY))
                .append(" speed=").append(f(bl.speedScale)).append(" id=").append(q(bl.referenceId)).append('\n');
        }
        for (int i = 0; i < n.conditions.length; i++) cond("   ", "cond", i, n.conditions[i]);
        for (int i = 0; i < n.events.size(); i++) {
            AnimEvent ev = n.events.get(i);
            sb.append("   event ").append(i).append(' ').append(ev.getClass().getSimpleName()).append(" name=").append(q(ev.eventName)).append(" time=")
                .append(e(ev.time)).append(" pc=").append(f(ev.timePc)).append(" param=").append(q(ev.parameterValue)).append(" parent=")
                .append(ev.parentAnimNode == n ? "1" : "?");
            if (ev instanceof AnimEventSetVariable sv) sb.append(" ref=").append(ref(sv.variableReference)).append(" set=").append(q(sv.setVariableValue));
            if (ev instanceof AnimEventFlagWhileAlive fw) sb.append(" ref=").append(ref(fw.variableReference)).append(" flag=").append(b(fw.flagValue));
            sb.append('\n');
        }
        for (int i = 0; i < n.blendTris.size(); i++) {
            Anim2DBlendTriangle t = n.blendTris.get(i);
            sb.append("   tri ").append(i).append(' ').append(indexOf(n.blends2d, t.node1)).append(' ').append(indexOf(n.blends2d, t.node2)).append(' ')
                .append(indexOf(n.blends2d, t.node3)).append('\n');
        }
        if (n.blend2dPicker == null) {
            sb.append("   picker null\n");
        } else {
            Anim2DBlendPicker p = n.blend2dPicker;
            sb.append("   picker tris=").append(field(p, "tris") == n.blendTris ? "1" : "?").append(" hull=");
            List<Anim2DBlend> hull = (List<Anim2DBlend>) field(p, "hull");
            List<Integer> idx = new ArrayList<>();
            for (Anim2DBlend h : hull) idx.add(Integer.valueOf(indexOf(n.blends2d, h)));
            idx.sort(null);
            sb.append(idx.toString().replace(" ", "")).append('\n');
        }
        for (int i = 0; i < n.transitions.size(); i++) {
            AnimTransition t = n.transitions.get(i);
            sb.append("   trans ").append(i).append(" source=").append(q(t.source)).append(" target=").append(q(t.target)).append(" anim=")
                .append(q(t.animName)).append(" bone=").append(q(t.deferredBoneName)).append(" axis=").append(e(t.deferredBoneAxis)).append(" rotation=")
                .append(b(t.useDeferedRotation)).append(" movement=").append(b(t.useDeferredMovement)).append(" rotationScale=")
                .append(f(t.deferredRotationScale)).append(" syncAdjust=").append(f(t.syncAdjustTime)).append(" in=").append(f(t.blendInTime))
                .append(" out=").append(f(t.blendOutTime)).append(" speed=").append(f(t.speedScale)).append(" parsed=")
                .append(b((Boolean) field(t, "isParsed"))).append('\n');
            for (int j = 0; j < t.conditions.length; j++) cond("    ", "tcond", j, t.conditions[j]);
        }
        for (int i = 0; i < n.subStateBoneWeights.size(); i++) {
            AnimBoneWeight w = n.subStateBoneWeights.get(i);
            sb.append("   bone ").append(i).append(' ').append(q(w.boneName)).append(" weight=").append(f(w.weight)).append(" descendants=")
                .append(b(w.includeDescendants)).append('\n');
        }
        sb.append("   transitionOut=").append(indexOf(n.transitions, (AnimTransition) field(n, "transitionOut"))).append('\n');
    }

    @SuppressWarnings("unchecked")
    static void animSets(File root) throws Exception {
        File[] dirs = new File(root, "media/AnimSets").listFiles(File::isDirectory);
        List<String> names = new ArrayList<>();
        if (dirs != null) for (File d : dirs) names.add(d.getName());
        names.sort(null);
        for (String name : names) {
            try {
                AnimationSet.GetAnimationSet(name, false);
            } catch (Throwable t) {
                sb.append("SETERROR ").append(q(name)).append(' ').append(t.getClass().getName()).append('\n');
            }
        }
        Map<String, AnimationSet> setMap = (Map<String, AnimationSet>) field(null, AnimationSet.class, "setMap");
        for (Map.Entry<String, AnimationSet> se : new TreeMap<>(setMap).entrySet()) {
            AnimationSet set = se.getValue();
            sb.append("ANIMSET ").append(q(se.getKey())).append(" name=").append(q(set.name)).append(" states=").append(set.states.size()).append('\n');
            for (Map.Entry<String, AnimState> ste : new TreeMap<>(set.states).entrySet()) {
                AnimState st = ste.getValue();
                sb.append(" STATE ").append(q(ste.getKey())).append(" name=").append(q(st.name)).append(" set=").append(st.set == set ? "1" : "?")
                    .append(" default=").append(st.defaultIndex).append(" nodes=").append(st.nodes.size()).append(" abstract=")
                    .append(st.abstractNodes.size()).append('\n');
                for (int i = 0; i < st.nodes.size(); i++) node("NODE " + i, st.nodes.get(i), st);
                for (int i = 0; i < st.abstractNodes.size(); i++) node("ABSTRACT " + i, st.abstractNodes.get(i), st);
            }
        }
        String base = root.getCanonicalPath() + File.separator;
        List<String> failed = new ArrayList<>();
        int[] total = new int[1];
        AnimNodeAssetManager.instance.getAssetTable().forEachValue(a -> {
            total[0]++;
            if (!a.isReady()) {
                String p = a.getPath().getPath();
                failed.add((p.startsWith(base) ? p.substring(base.length()) : p) + (a.isFailure() ? " failed" : " not-ready"));
            }
            return true;
        });
        failed.sort(null);
        sb.append("ASSETS ").append(total[0]).append('\n');
        for (String s : failed) sb.append("ASSET ").append(q(s)).append('\n');
    }

    static String value(Object v) throws Exception {
        if (v == null) return "null";
        if (v instanceof String s) return "s" + q(s);
        if (v instanceof Integer i) return "i" + i;
        if (v instanceof Float fl) return "f" + f(fl);
        if (v instanceof Boolean bo) return "b" + bo;
        if (v.getClass().getSimpleName().equals("CharacterVariableLookup"))
            return "lookup" + ref((AnimationVariableReference) field(v, "variableReference"));
        return "?" + v.getClass().getName();
    }

    static void actionCondition(IActionCondition c) throws Exception {
        sb.append("    acond ");
        if (c instanceof CharacterVariableCondition cv) {
            sb.append("CharacterVariableCondition op=").append(e((Enum<?>) field(cv, "op"))).append(" lhs=").append(value(field(cv, "lhsValue")))
                .append(" rhs=").append(value(field(cv, "rhsValue")));
        } else if (c instanceof EventOccurred eo) {
            sb.append("EventOccurred ").append(q(eo.eventName));
        } else if (c instanceof EventNotOccurred en) {
            sb.append("EventNotOccurred ").append(q(en.eventName));
        } else {
            sb.append(c.getClass().getSimpleName());
        }
        sb.append('\n');
    }

    @SuppressWarnings("unchecked")
    static void actionGroups(File root) throws Exception {
        File[] dirs = new File(root, "media/actiongroups").listFiles(File::isDirectory);
        List<String> names = new ArrayList<>();
        if (dirs != null) for (File d : dirs) names.add(d.getName());
        names.sort(null);
        for (String name : names) ActionGroup.getActionGroup(name);
        Map<String, ActionGroup> groups = (Map<String, ActionGroup>) field(null, ActionGroup.class, "s_actionGroupMap");
        for (Map.Entry<String, ActionGroup> ge : new TreeMap<>(groups).entrySet()) {
            ActionGroup g = ge.getValue();
            List<ActionState> states = (List<ActionState>) field(g, "states");
            ActionState initial = g.getInitialState();
            sb.append("ACTIONGROUP ").append(q(ge.getKey())).append(" name=").append(q(g.getName())).append(" initial=")
                .append(q((String) field(g, "initialStateName"))).append(" initialState=").append(initial == null ? "null" : q(initial.getName()))
                .append(" states=").append(states.size()).append('\n');
            for (int i = 0; i < states.size(); i++) {
                ActionState s = states.get(i);
                List<IAnimationVariableSlot> vars = new ArrayList<>();
                for (IAnimationVariableSlot slot : s.getGameVariablesInternal().getGameVariables()) vars.add(slot);
                sb.append(" ASTATE ").append(i).append(' ').append(q(s.getName())).append(" group=").append(s.getParentActionGroup() == g ? "1" : "?")
                    .append(" grappler=").append(b(s.isGrapplerState())).append(" tags=").append(strings((String[]) field(s, "tags")))
                    .append(" childTags=").append(strings((String[]) field(s, "childTags"))).append(" vars=").append(vars.size())
                    .append(" transitions=").append(s.transitions.size()).append('\n');
                for (IAnimationVariableSlot slot : vars) {
                    sb.append("  var ").append(q(slot.getKey())).append(' ').append(slot.getClass().getSimpleName()).append(' ')
                        .append(e(slot.getType())).append(" value=").append(q(slot.getValueString())).append(" readOnly=").append(b(slot.isReadOnly()))
                        .append('\n');
                }
                for (int j = 0; j < s.transitions.size(); j++) {
                    ActionTransition t = s.transitions.get(j);
                    List<IActionCondition> conds = (List<IActionCondition>) field(t, "conditions");
                    sb.append("  atrans ").append(j).append(" to=").append(q(t.getTransitionTo())).append(" asSubstate=")
                        .append(b((Boolean) field(t, "asSubstate"))).append(" transitionOut=").append(b((Boolean) field(t, "transitionOut")))
                        .append(" forceParent=").append(b((Boolean) field(t, "forceParent"))).append(" priority=")
                        .append(field(t, "conditionPriority")).append(" conditions=").append(conds.size()).append('\n');
                    for (IActionCondition c : conds) actionCondition(c);
                }
            }
            for (Map.Entry<String, ActionState> le : new TreeMap<>((Map<String, ActionState>) field(g, "stateLookup")).entrySet())
                sb.append(" LOOKUP ").append(q(le.getKey())).append(' ').append(indexOf(states, le.getValue())).append('\n');
            for (Map.Entry<Integer, String> le : new TreeMap<>((Map<Integer, String>) field(g, "stateNameLookup")).entrySet())
                sb.append(" NAMELOOKUP ").append(le.getKey()).append(' ').append(q(le.getValue())).append('\n');
        }
    }

    public static void main(String[] a) throws Exception {
        PrintStream out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, "UTF-8");
        System.setOut(System.err);
        ZomboidFileSystem.instance.setCacheDir(a[0]);
        ZomboidFileSystem.instance.init();
        Class.forName("zombie.characters.action.ActionContext");
        File root = new File(".").getCanonicalFile();
        animSets(root);
        actionGroups(root);
        out.print(sb);
        out.flush();
        Runtime.getRuntime().halt(0);
    }
}
