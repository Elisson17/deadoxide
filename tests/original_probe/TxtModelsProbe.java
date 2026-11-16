import java.io.File;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import org.lwjgl.util.vector.Matrix4f;
import zombie.core.skinnedmodel.animation.AnimationClip;
import zombie.core.skinnedmodel.animation.Keyframe;
import zombie.core.skinnedmodel.model.ModelLoader;
import zombie.core.skinnedmodel.model.SkinningData;
import zombie.core.skinnedmodel.model.VertexBufferObject;

/** Oracle probe (tools/oracle.sh txtmodels): the ORIGINAL ModelLoader.loadTxt (protected, by reflection) on every
 * media/models/*.txt and media/animsold/*.txt, in name order, in several (bStatic, bReverse, skinnedTo) combinations:
 * the whole ModelTxt printed section by section (format, vertex floats, element indices, bone hierarchy and indices
 * in name order, the three matrix lists, each clip's keyframes and its per-bone-index tables). Each section is one
 * line in the default (digest) mode: `<label> lines=<n> fnv=<FNV-1a 64 of the section text>`; with the arguments
 * `full <file>...` the sections of those files are printed in full. Floats are Float.toString. An exception that
 * leaves loadTxt prints `error <class>`. Compared with `pz-dump txtmodels`. Run from the game root. */
public class TxtModelsProbe {
    static final PrintStream out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, StandardCharsets.UTF_8);
    static boolean full = false;

    static String fnv(String text) {
        long h = 0xcbf29ce484222325L;
        for (byte b : text.getBytes(StandardCharsets.UTF_8)) {
            h ^= (b & 0xff);
            h *= 0x100000001b3L;
        }
        return String.format("%016x", h);
    }

    static void section(String label, List<String> lines) {
        if (full) {
            out.println(label + " lines=" + lines.size());
            for (String l : lines) out.println("  " + l);
        } else {
            StringBuilder sb = new StringBuilder();
            for (String l : lines) sb.append(l).append('\n');
            out.println(label + " lines=" + lines.size() + " fnv=" + fnv(sb.toString()));
        }
    }

    static Object get(Object o, String name) throws Exception {
        Field f = o.getClass().getDeclaredField(name);
        f.setAccessible(true);
        return f.get(o);
    }

    static String mat(Matrix4f m) {
        return m.m00 + " " + m.m01 + " " + m.m02 + " " + m.m03 + " " + m.m10 + " " + m.m11 + " " + m.m12 + " " + m.m13 + " "
            + m.m20 + " " + m.m21 + " " + m.m22 + " " + m.m23 + " " + m.m30 + " " + m.m31 + " " + m.m32 + " " + m.m33;
    }

    static String v3(org.lwjgl.util.vector.Vector3f v) {
        return v == null ? "null" : v.x + "," + v.y + "," + v.z;
    }

    static String q4(org.lwjgl.util.vector.Quaternion q) {
        return q == null ? "null" : q.x + "," + q.y + "," + q.z + "," + q.w;
    }

    static ModelLoader loader = ModelLoader.instance;
    static Method loadTxt;

    @SuppressWarnings("unchecked")
    static void dump(Object mt) throws Exception {
        VertexBufferObject.VertexArray va = (VertexBufferObject.VertexArray) get(mt, "vertices");
        if (va == null) {
            out.println("vertices null");
        } else {
            List<String> fmt = new ArrayList<>();
            for (int i = 0; i < va.format.getNumElements(); i++) {
                VertexBufferObject.VertexElement e = va.format.getElement(i);
                fmt.add(i + " " + e.type + " size=" + e.byteSize + " offset=" + e.byteOffset);
            }
            fmt.add("stride=" + va.format.getStride() + " numVertices=" + va.numVertices);
            section("format", fmt);
            List<String> vs = new ArrayList<>();
            for (int v = 0; v < va.numVertices; v++) {
                StringBuilder sb = new StringBuilder();
                sb.append(v).append(':');
                for (int e = 0; e < va.format.getNumElements(); e++) {
                    sb.append(e == 0 ? " " : " | ");
                    int n = va.format.getElement(e).byteSize / 4;
                    for (int k = 0; k < n; k++) sb.append(k == 0 ? "" : " ").append(va.getElementFloat(v, e, k));
                }
                vs.add(sb.toString());
            }
            section("vertices", vs);
        }
        int[] el = (int[]) get(mt, "elements");
        if (el == null) {
            out.println("elements null");
        } else {
            List<String> es = new ArrayList<>();
            for (int i = 0; i + 2 < el.length; i += 3) es.add(el[i] + " " + el[i + 1] + " " + el[i + 2]);
            if (el.length % 3 != 0) es.add("tail " + el.length);
            section("elements", es);
        }
        List<String> bones = new ArrayList<>();
        ArrayList<Integer> hier = (ArrayList<Integer>) get(mt, "skeletonHierarchy");
        for (int i = 0; i < hier.size(); i++) bones.add("parent " + i + " " + hier.get(i));
        HashMap<String, Integer> bi = (HashMap<String, Integer>) get(mt, "boneIndices");
        for (Map.Entry<String, Integer> e : new TreeMap<>(bi).entrySet()) bones.add("index " + e.getKey() + " " + e.getValue());
        section("bones", bones);
        for (String name : new String[] {"bindPose", "invBindPose", "skinOffsetMatrices"}) {
            List<String> ms = new ArrayList<>();
            for (Matrix4f m : (ArrayList<Matrix4f>) get(mt, name)) ms.add(mat(m));
            section(name, ms);
        }
        HashMap<String, AnimationClip> clips = (HashMap<String, AnimationClip>) get(mt, "clips");
        out.println("clips " + clips.size());
        for (Map.Entry<String, AnimationClip> e : new TreeMap<>(clips).entrySet()) {
            AnimationClip c = e.getValue();
            out.println("clip " + e.getKey() + " name=" + c.name + " duration=" + c.getDuration() + " keepLast=" + c.keepLastFrame
                + " ragdoll=" + c.isRagdoll + " frames=" + c.getKeyframes().length);
            List<String> ks = new ArrayList<>();
            for (Keyframe k : c.getKeyframes()) {
                ks.add(k.bone + " " + k.boneName + " " + k.time + " " + v3(k.position) + " " + q4(k.rotation) + " " + v3(k.scale));
            }
            section("keyframes", ks);
            List<String> bb = new ArrayList<>();
            for (int b = 0; b < 60; b++) {
                StringBuilder sb = new StringBuilder("b" + b + " n=");
                Keyframe[] fr = c.getBoneFramesAt(b);
                sb.append(fr.length).append(':');
                for (Keyframe k : fr) sb.append(' ').append(k.bone).append('@').append(k.time);
                bb.add(sb.toString());
            }
            section("byBone", bb);
        }
    }

    static SkinningData skin(String rel) throws Exception {
        Object mt = loadTxt.invoke(loader, rel, false, false, null);
        return new SkinningData((HashMap<String, AnimationClip>) get(mt, "clips"), (ArrayList<Matrix4f>) get(mt, "bindPose"),
            (ArrayList<Matrix4f>) get(mt, "invBindPose"), (ArrayList<Matrix4f>) get(mt, "skinOffsetMatrices"),
            (ArrayList<Integer>) get(mt, "skeletonHierarchy"), (HashMap<String, Integer>) get(mt, "boneIndices"));
    }

    static void load(String rel, boolean bStatic, boolean bReverse, String skinName) throws Exception {
        out.println("load " + rel + " static=" + bStatic + " reverse=" + bReverse + " skin=" + (skinName == null ? "none" : skinName));
        try {
            SkinningData s = skinName == null ? null : skin(skinName);
            Object mt = loadTxt.invoke(loader, rel, bStatic, bReverse, s);
            dump(mt);
        } catch (InvocationTargetException e) {
            out.println("error " + e.getCause().getClass().getName());
        } catch (Exception e) {
            out.println("error " + e.getClass().getName());
        }
    }

    static String[] list(String dir) {
        String[] names = new File(dir).list();
        Arrays.sort(names);
        ArrayList<String> r = new ArrayList<>();
        for (String n : names) if (n.endsWith(".txt")) r.add(dir + "/" + n);
        return r.toArray(new String[0]);
    }

    public static void main(String[] args) throws Exception {
        loadTxt = ModelLoader.class.getDeclaredMethod("loadTxt", String.class, boolean.class, boolean.class, SkinningData.class);
        loadTxt.setAccessible(true);
        String kate = "media/models/kate.txt";
        String bob = "media/animsold/Bob_Asleep.txt";
        if (args.length > 0 && args[0].equals("full")) {
            full = true;
            for (int i = 1; i < args.length; i++) {
                load(args[i], false, false, null);
                load(args[i], true, false, null);
            }
            out.flush();
            return;
        }
        for (String rel : list("media/models")) {
            load(rel, true, false, null);
            load(rel, false, false, null);
            load(rel, false, true, null);
            load(rel, false, false, kate);
        }
        for (String rel : list("media/animsold")) {
            load(rel, false, false, null);
            load(rel, false, true, null);
            load(rel, false, false, kate);
            load(rel, false, false, bob);
            load(rel, true, false, null);
        }
        out.flush();
    }
}
