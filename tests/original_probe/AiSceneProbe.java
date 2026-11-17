import java.io.File;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.EnumSet;
import java.util.List;
import jassimp.AiAnimation;
import jassimp.AiBone;
import jassimp.AiBoneWeight;
import jassimp.AiBuiltInWrapperProvider;
import jassimp.AiMaterial;
import jassimp.AiMatrix4f;
import jassimp.AiMesh;
import jassimp.AiNode;
import jassimp.AiNodeAnim;
import jassimp.AiPostProcessSteps;
import jassimp.AiScene;
import jassimp.Jassimp;
import zombie.core.skinnedmodel.model.jassimp.JAssImpImporter;

/** Oracle probe (tools/oracle.sh xscene, fbxscene, animscene): the ORIGINAL native assimp (the game's libjassimp through
 * jassimp.Jassimp.importFile, the post-process flags of FileTask_LoadMesh / FileTask_LoadAnimation) on every file of
 * the given extension under the given directories, in name order, and the imported AiScene printed section by section
 * (see TxtModelsProbe: one digest line per section, `full <file>...` prints the sections in full).
 * Args: <flags: mesh | anim | none | STEP,STEP,...> <extensions, comma separated, lower case> <dir>... ;
 *       or full <flags> <file>... Run from the game root. */
public class AiSceneProbe {
    static final PrintStream out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, StandardCharsets.UTF_8);
    static boolean full = false;
    /** a flag spec ending in `+mat` also prints every material's properties (key, semantic, index, type, data) */
    static boolean mats = false;

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

    static EnumSet<AiPostProcessSteps> flags(String spec) {
        EnumSet<AiPostProcessSteps> s = EnumSet.noneOf(AiPostProcessSteps.class);
        if (spec.equals("mesh")) {
            s.addAll(EnumSet.of(AiPostProcessSteps.FIND_INSTANCES, AiPostProcessSteps.MAKE_LEFT_HANDED, AiPostProcessSteps.LIMIT_BONE_WEIGHTS,
                AiPostProcessSteps.TRIANGULATE, AiPostProcessSteps.OPTIMIZE_MESHES, AiPostProcessSteps.REMOVE_REDUNDANT_MATERIALS,
                AiPostProcessSteps.JOIN_IDENTICAL_VERTICES));
        } else if (spec.equals("anim")) {
            s.addAll(EnumSet.of(AiPostProcessSteps.MAKE_LEFT_HANDED, AiPostProcessSteps.REMOVE_REDUNDANT_MATERIALS));
        } else if (!spec.equals("none")) {
            for (String n : spec.split(",")) s.add(AiPostProcessSteps.valueOf(n));
        }
        return s;
    }

    static String mat(AiMatrix4f m) {
        StringBuilder sb = new StringBuilder();
        for (int r = 0; r < 4; r++) for (int c = 0; c < 4; c++) sb.append(r + c == 0 ? "" : " ").append(m.get(r, c));
        return sb.toString();
    }

    static String data(Object d) {
        if (d instanceof float[] fa) { StringBuilder sb = new StringBuilder("floats"); for (float x : fa) sb.append(' ').append(x); return sb.toString(); }
        if (d instanceof int[] ia) { StringBuilder sb = new StringBuilder("ints"); for (int x : ia) sb.append(' ').append(x); return sb.toString(); }
        if (d instanceof Float || d instanceof Integer) return d.getClass().getSimpleName() + " " + d;
        if (d instanceof String str) return "string " + str;
        if (d instanceof java.nio.ByteBuffer bb) { StringBuilder sb = new StringBuilder("buffer"); for (int i = 0; i < bb.limit(); i++) sb.append(String.format(" %02x", bb.get(i))); return sb.toString(); }
        if (d instanceof byte[] ba) { StringBuilder sb = new StringBuilder("bytes"); for (byte x : ba) sb.append(String.format(" %02x", x)); return sb.toString(); }
        return d == null ? "null" : d.getClass().getName() + " " + d;
    }

    static void dumpScene(AiScene sc) {
        AiBuiltInWrapperProvider wp = new AiBuiltInWrapperProvider();
        out.println("scene meshes=" + sc.getNumMeshes() + " materials=" + sc.getNumMaterials() + " animations=" + sc.getNumAnimations()
            + " cameras=" + sc.getNumCameras() + " lights=" + sc.getNumLights());
        for (int i = 0; i < sc.getNumMeshes(); i++) {
            AiMesh m = sc.getMeshes().get(i);
            StringBuilder h = new StringBuilder();
            h.append("mesh ").append(i).append(" name=").append(m.getName()).append(" material=").append(m.getMaterialIndex())
                .append(" vertices=").append(m.getNumVertices()).append(" faces=").append(m.getNumFaces()).append(" ptypes=").append(m.getPrimitiveTypes())
                .append(" bones=").append(m.getBones().size()).append(" normals=").append(m.hasNormals()).append(" tangents=").append(m.hasTangentsAndBitangents());
            for (int u = 0; u < 8; u++) if (m.hasTexCoords(u)) h.append(" uv").append(u).append('=').append(m.getNumUVComponents(u));
            for (int u = 0; u < 8; u++) if (m.hasColors(u)) h.append(" color").append(u);
            out.println(h);
            List<String> l = new ArrayList<>();
            for (int v = 0; v < m.getNumVertices(); v++) l.add(m.getPositionX(v) + " " + m.getPositionY(v) + " " + m.getPositionZ(v));
            section("positions", l);
            if (m.hasNormals()) {
                l = new ArrayList<>();
                for (int v = 0; v < m.getNumVertices(); v++) l.add(m.getNormalX(v) + " " + m.getNormalY(v) + " " + m.getNormalZ(v));
                section("normals", l);
            }
            if (m.hasTangentsAndBitangents()) {
                l = new ArrayList<>();
                for (int v = 0; v < m.getNumVertices(); v++) l.add(m.getTangentX(v) + " " + m.getTangentY(v) + " " + m.getTangentZ(v) + " "
                    + m.getBitangentX(v) + " " + m.getBitangentY(v) + " " + m.getBitangentZ(v));
                section("tangents", l);
            }
            for (int u = 0; u < 8; u++) {
                if (!m.hasTexCoords(u)) continue;
                l = new ArrayList<>();
                for (int v = 0; v < m.getNumVertices(); v++) l.add(m.getTexCoordU(v, u) + " " + m.getTexCoordV(v, u) + (m.getNumUVComponents(u) == 3 ? " " + m.getTexCoordW(v, u) : ""));
                section("uv" + u, l);
            }
            for (int u = 0; u < 8; u++) {
                if (!m.hasColors(u)) continue;
                l = new ArrayList<>();
                for (int v = 0; v < m.getNumVertices(); v++) l.add(m.getColorR(v, u) + " " + m.getColorG(v, u) + " " + m.getColorB(v, u) + " " + m.getColorA(v, u));
                section("colors" + u, l);
            }
            l = new ArrayList<>();
            for (int f = 0; f < m.getNumFaces(); f++) {
                StringBuilder sb = new StringBuilder();
                int n = m.getFaceNumIndices(f);
                for (int k = 0; k < n; k++) sb.append(k == 0 ? "" : " ").append(m.getFaceVertex(f, k));
                l.add(sb.toString());
            }
            section("faces", l);
            l = new ArrayList<>();
            for (AiBone b : m.getBones()) {
                l.add("bone " + b.getName() + " weights=" + b.getNumWeights() + " offset=" + mat((AiMatrix4f) b.getOffsetMatrix(wp0)));
                for (AiBoneWeight w : b.getBoneWeights()) l.add(w.getVertexId() + " " + w.getWeight());
            }
            section("bones", l);
        }
        if (mats) {
            List<String> l = new ArrayList<>();
            for (int i = 0; i < sc.getNumMaterials(); i++) {
                AiMaterial mt = sc.getMaterials().get(i);
                l.add("material " + i + " properties=" + mt.getProperties().size());
                for (AiMaterial.Property p : mt.getProperties())
                    l.add(p.getKey() + " " + p.getSemantic() + " " + p.getIndex() + " " + p.getType() + " " + data(p.getData()));
            }
            section("materials", l);
        }
        List<String> nodes = new ArrayList<>();
        walk(sc.getSceneRoot(wp), 0, nodes);
        section("nodes", nodes);
        for (int a = 0; a < sc.getNumAnimations(); a++) {
            AiAnimation an = sc.getAnimations().get(a);
            out.println("animation " + a + " name=" + an.getName() + " duration=" + an.getDuration() + " tps=" + an.getTicksPerSecond()
                + " channels=" + an.getNumChannels());
            List<String> l = new ArrayList<>();
            for (AiNodeAnim c : an.getChannels()) {
                l.add("channel " + c.getNodeName() + " pos=" + c.getNumPosKeys() + " rot=" + c.getNumRotKeys() + " scale=" + c.getNumScaleKeys()
                    + " pre=" + c.getPreState() + " post=" + c.getPostState());
                for (int k = 0; k < c.getNumPosKeys(); k++) l.add("p " + c.getPosKeyTime(k) + " " + c.getPosKeyX(k) + " " + c.getPosKeyY(k) + " " + c.getPosKeyZ(k));
                for (int k = 0; k < c.getNumRotKeys(); k++) l.add("r " + c.getRotKeyTime(k) + " " + c.getRotKeyW(k) + " " + c.getRotKeyX(k) + " " + c.getRotKeyY(k) + " " + c.getRotKeyZ(k));
                for (int k = 0; k < c.getNumScaleKeys(); k++) l.add("s " + c.getScaleKeyTime(k) + " " + c.getScaleKeyX(k) + " " + c.getScaleKeyY(k) + " " + c.getScaleKeyZ(k));
            }
            section("channels", l);
        }
    }

    static final AiBuiltInWrapperProvider wp0 = new AiBuiltInWrapperProvider();

    static void walk(AiNode n, int depth, List<String> lines) {
        StringBuilder sb = new StringBuilder();
        sb.append(depth).append(' ').append(n.getName()).append(" meshes=").append(Arrays.toString(n.getMeshes())).append(" transform=")
            .append(mat((AiMatrix4f) n.getTransform(wp0)));
        lines.add(sb.toString());
        for (AiNode c : n.getChildren()) walk(c, depth + 1, lines);
    }

    static void one(String rel, String flagSpec) {
        out.println("file " + rel + " flags=" + flagSpec);
        try {
            mats = flagSpec.endsWith("+mat");
            AiScene sc = Jassimp.importFile(rel, flags(mats ? flagSpec.substring(0, flagSpec.length() - 4) : flagSpec));
            dumpScene(sc);
        } catch (Throwable t) {
            out.println("error " + t.getClass().getName() + ": " + t.getMessage());
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
        JAssImpImporter.Init();
        if (args[0].equals("full")) {
            full = true;
            for (int i = 2; i < args.length; i++) one(args[i], args[1]);
        } else {
            List<String> files = new ArrayList<>();
            List<String> exts = Arrays.asList(args[1].split(","));
            for (int i = 2; i < args.length; i++) collect(new File(args[i]), args[i], exts, files);
            for (String f : files) one(f, args[0]);
        }
        out.flush();
    }
}
