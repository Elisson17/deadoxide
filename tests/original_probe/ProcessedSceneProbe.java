import java.io.File;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.EnumSet;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import jassimp.AiNode;
import jassimp.AiPostProcessSteps;
import jassimp.AiScene;
import jassimp.Jassimp;
import org.lwjgl.util.vector.Matrix4f;
import zombie.core.skinnedmodel.animation.AnimationClip;
import zombie.core.skinnedmodel.animation.Keyframe;
import zombie.core.skinnedmodel.model.SkinningData;
import zombie.core.skinnedmodel.model.VertexBufferObject;
import zombie.core.skinnedmodel.model.jassimp.JAssImpImporter;
import zombie.core.skinnedmodel.model.jassimp.ProcessedAiScene;
import zombie.core.skinnedmodel.model.jassimp.ProcessedAiSceneParams;

/** Oracle probe (tools/oracle.sh xmodels, animclips): the ORIGINAL ProcessedAiScene.process on the scene the native
 * assimp imports (the post-process flags of FileTask_LoadMesh / FileTask_LoadAnimation) for every file of the given
 * extension under the given directories, in name order, in the given LoadMode. What the processing builds
 * (ImportedSkeleton: bone indices and hierarchy, bind pose, inverse bind pose, skin offsets, the animation clips with
 * their keyframes; ImportedSkinnedMesh and ImportedStaticMesh: the vertex array, the elements, the bounds and the
 * transform) is read by reflection and printed section by section like TxtModelsProbe (one digest line per section;
 * `full <mode> <flags> <file>...` prints the sections in full). Floats are Float.toString.
 * Args: <passes> <flags: mesh | anim> <extensions> <dir>... ; or full <passes> <flags> <file>... (the passes are
 * comma separated, `Normal | StaticMesh | AnimationOnly` with an optional `:last`, `:all`, `:fbx` or `:skin=<file>`). Run from the
 * game root. */
public class ProcessedSceneProbe {
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
        if (m == null) return "null";
        return m.m00 + " " + m.m01 + " " + m.m02 + " " + m.m03 + " " + m.m10 + " " + m.m11 + " " + m.m12 + " " + m.m13 + " "
            + m.m20 + " " + m.m21 + " " + m.m22 + " " + m.m23 + " " + m.m30 + " " + m.m31 + " " + m.m32 + " " + m.m33;
    }

    static String v3(org.lwjgl.util.vector.Vector3f v) {
        return v == null ? "null" : v.x + "," + v.y + "," + v.z;
    }

    static String q4(org.lwjgl.util.vector.Quaternion q) {
        return q == null ? "null" : q.x + "," + q.y + "," + q.z + "," + q.w;
    }

    static void dumpVertices(Object va0, Object el0) {
        VertexBufferObject.VertexArray va = (VertexBufferObject.VertexArray) va0;
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
        int[] el = (int[]) el0;
        List<String> es = new ArrayList<>();
        for (int i = 0; i + 2 < el.length; i += 3) es.add(el[i] + " " + el[i + 1] + " " + el[i + 2]);
        if (el.length % 3 != 0) es.add("tail " + el.length);
        section("elements", es);
    }

    @SuppressWarnings("unchecked")
    static void dumpSkeleton(Object sk) throws Exception {
        AiNode root = (AiNode) get(sk, "rootBoneNode");
        out.println("skeleton root=" + (root == null ? "null" : root.getName()));
        List<String> bones = new ArrayList<>();
        ArrayList<Integer> hier = (ArrayList<Integer>) get(sk, "skeletonHierarchy");
        for (int i = 0; i < hier.size(); i++) bones.add("parent " + i + " " + hier.get(i));
        HashMap<String, Integer> bi = (HashMap<String, Integer>) get(sk, "boneIndices");
        for (Map.Entry<String, Integer> e : new TreeMap<>(bi).entrySet()) bones.add("index " + e.getKey() + " " + e.getValue());
        section("bones", bones);
        for (String name : new String[] {"bindPose", "invBindPose", "skinOffsetMatrices"}) {
            List<String> ms = new ArrayList<>();
            for (Matrix4f m : (ArrayList<Matrix4f>) get(sk, name)) ms.add(mat(m));
            section(name, ms);
        }
        HashMap<String, AnimationClip> clips = (HashMap<String, AnimationClip>) get(sk, "clips");
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

    static String jv3(org.joml.Vector3f v) {
        return v.x + "," + v.y + "," + v.z;
    }

    static void dumpProcessed(ProcessedAiScene ps) throws Exception {
        Object sk = get(ps, "skeleton");
        Object sm = get(ps, "skinnedMesh");
        List<?> st = (List<?>) get(ps, "staticMeshes");
        out.println("result skeleton=" + (sk != null) + " skinned=" + (sm != null) + " static=" + st.size());
        if (sk != null) dumpSkeleton(sk);
        if (sm != null) {
            out.println("skinned name=" + get(sm, "name") + " transform=" + mat((Matrix4f) get(sm, "transform")));
            dumpVertices(get(sm, "vertices"), get(sm, "elements"));
        }
        for (int i = 0; i < st.size(); i++) {
            Object s = st.get(i);
            out.println("static " + i + " min=" + jv3((org.joml.Vector3f) get(s, "minXyz")) + " max=" + jv3((org.joml.Vector3f) get(s, "maxXyz"))
                + " transform=" + mat((Matrix4f) get(s, "transform")));
            dumpVertices(get(s, "verticesUnskinned"), get(s, "elements"));
        }
    }

    static EnumSet<AiPostProcessSteps> flags(String spec) {
        if (spec.equals("mesh")) {
            return EnumSet.of(AiPostProcessSteps.FIND_INSTANCES, AiPostProcessSteps.MAKE_LEFT_HANDED, AiPostProcessSteps.LIMIT_BONE_WEIGHTS,
                AiPostProcessSteps.TRIANGULATE, AiPostProcessSteps.OPTIMIZE_MESHES, AiPostProcessSteps.REMOVE_REDUNDANT_MATERIALS,
                AiPostProcessSteps.JOIN_IDENTICAL_VERTICES);
        }
        return EnumSet.of(AiPostProcessSteps.MAKE_LEFT_HANDED, AiPostProcessSteps.REMOVE_REDUNDANT_MATERIALS);
    }

    /** The SkinningData of the skeleton of `rel` (processed as a Normal mesh), as FileTask_LoadMesh passes the
     * `animationsMesh`'s skinning data as `skinnedTo`. */
    @SuppressWarnings("unchecked")
    static SkinningData skinOf(String rel) throws Exception {
        AiScene sc = Jassimp.importFile(rel, flags("mesh"));
        ProcessedAiSceneParams p = ProcessedAiSceneParams.create();
        p.scene = sc;
        ProcessedAiScene ps = ProcessedAiScene.process(p);
        Object sk = get(ps, "skeleton");
        return new SkinningData((HashMap<String, AnimationClip>) get(sk, "clips"), (ArrayList<Matrix4f>) get(sk, "bindPose"),
            (ArrayList<Matrix4f>) get(sk, "invBindPose"), (ArrayList<Matrix4f>) get(sk, "skinOffsetMatrices"),
            (ArrayList<Integer>) get(sk, "skeletonHierarchy"), (HashMap<String, Integer>) get(sk, "boneIndices"));
    }

    /** One pass: `Mode[:variant]`; variants: `last` (meshName = the name of the last mesh), `all` (allMeshes), `fbx` (the
     * scale 0.01 and the -90 degree rotation about x that FileTask_LoadMesh/LoadAnimation give an .fbx), `skin=<file>`
     * (skinnedTo = the skeleton of that file). */
    static void one(String rel, String passSpec, String flagSpec) {
        out.println("file " + rel + " pass=" + passSpec + " flags=" + flagSpec);
        try {
            String[] parts = passSpec.split(":");
            AiScene sc = Jassimp.importFile(rel, flags(flagSpec));
            ProcessedAiSceneParams p = ProcessedAiSceneParams.create();
            p.scene = sc;
            p.mode = JAssImpImporter.LoadMode.valueOf(parts[0]);
            String variant = parts.length > 1 ? parts[1] : "";
            if (variant.equals("last") && sc.getNumMeshes() > 0) p.meshName = sc.getMeshes().get(sc.getNumMeshes() - 1).getName();
            if (variant.equals("all")) p.allMeshes = true;
            if (variant.startsWith("skin=")) p.skinnedTo = skinOf(variant.substring(5));
            if (variant.equals("fbx")) {
                org.lwjgl.util.vector.Quaternion rotateModifier = new org.lwjgl.util.vector.Quaternion();
                rotateModifier.setFromAxisAngle(new org.lwjgl.util.vector.Vector4f(1.0F, 0.0F, 0.0F, (float) (-Math.PI / 2)));
                p.animBonesScaleModifier = 0.01F;
                p.animBonesRotateModifier = rotateModifier;
            }
            ProcessedAiScene ps = ProcessedAiScene.process(p);
            dumpProcessed(ps);
        } catch (Throwable t) {
            out.println("error " + t.getClass().getName());
        }
    }

    static void collect(File dir, String rel, List<String> exts, List<String> acc) {
        String[] names = dir.list();
        if (names == null) return;
        Arrays.sort(names);
        for (String n : names) {
            File f = new File(dir, n);
            if (f.isDirectory()) {
                collect(f, rel + "/" + n, exts, acc);
            } else {
                String low = n.toLowerCase();
                for (String e : exts) if (low.endsWith("." + e)) acc.add(rel + "/" + n);
            }
        }
    }

    public static void main(String[] args) throws Exception {
        System.setOut(System.err);
        JAssImpImporter.Init();
        if (args[0].equals("full")) {
            full = true;
            for (int i = 3; i < args.length; i++) for (String pass : args[1].split(",")) one(args[i], pass, args[2]);
        } else {
            List<String> files = new ArrayList<>();
            List<String> exts = Arrays.asList(args[2].split(","));
            for (int i = 3; i < args.length; i++) collect(new File(args[i]), args[i], exts, files);
            for (String f : files) for (String pass : args[0].split(",")) one(f, pass, args[1]);
        }
        out.flush();
    }
}
