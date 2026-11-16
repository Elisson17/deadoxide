import gnu.trove.list.array.TFloatArrayList;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.joml.Vector2f;
import org.joml.Vector3f;
import se.krka.kahlua.vm.KahluaTable;
import se.krka.kahlua.vm.KahluaTableIterator;
import zombie.core.ImmutableColor;
import zombie.core.textures.Texture;
import zombie.scripting.ScriptManager;
import zombie.scripting.objects.BaseScriptObject;
import zombie.scripting.objects.ModelAttachment;
import zombie.scripting.objects.VehicleScript;
import zombie.scripting.objects.VehicleTemplate;

// Inside the booted original: every vehicle script (ScriptManager.getAllVehicleScripts: VehicleScript.Load and
// Loaded) sorted by full name, then every vehicle template (getAllVehicleTemplates) sorted by full type, with the
// VehicleScript its getScript() built if it was asked for. Every instance field of VehicleScript and of its nested
// objects is printed by reflection in declaration order, one line per field: "<path>\t<field>\t<value>"; a list of
// objects as "size N" and one block per element (path "<field>[i]"), an object as "object" and a block. Strings:
// '\\', tab, CR, LF and every UTF-16 unit outside 0x20..0x7e escaped (\\uXXXX); floats with Float.toString; hash
// containers sorted (Lua tables by key, numbers "n:" + Double.toString, strings "s:"). Args: <scratch dir>
public class VehiclesProbe {
    static String esc(Object v) {
        if (v == null) return "null";
        String s = v.toString();
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c == '\\') sb.append("\\\\");
            else if (c == '\t') sb.append("\\t");
            else if (c == '\n') sb.append("\\n");
            else if (c == '\r') sb.append("\\r");
            else if (c < 0x20 || c > 0x7e) sb.append(String.format("\\u%04x", (int) c));
            else sb.append(c);
        }
        return sb.toString();
    }

    static boolean complex(Object v) {
        return v instanceof ModelAttachment || v != null && v.getClass().getName().startsWith("zombie.scripting.objects.VehicleScript$");
    }

    static String table(KahluaTable t) {
        ArrayList<String> entries = new ArrayList<>();
        KahluaTableIterator it = t.iterator();
        while (it.advance()) {
            Object k = it.getKey();
            String ks = k instanceof Double d ? "n:" + Double.toString(d) : "s:" + esc(k);
            entries.add(ks + "=" + val(it.getValue()));
        }
        entries.sort(null);
        return "{" + String.join(", ", entries) + "}";
    }

    static String val(Object v) {
        if (v == null) return "null";
        if (v instanceof Float f) return Float.toString(f);
        if (v instanceof Double d) return Double.toString(d);
        if (v instanceof Vector3f a) return "(" + Float.toString(a.x) + ", " + Float.toString(a.y) + ", " + Float.toString(a.z) + ")";
        if (v instanceof Vector2f a) return "(" + Float.toString(a.x) + ", " + Float.toString(a.y) + ")";
        if (v instanceof ImmutableColor c) return "rgba(" + Float.toString(c.r) + ", " + Float.toString(c.g) + ", " + Float.toString(c.b) + ", " + Float.toString(c.a) + ")";
        if (v instanceof float[] a) {
            ArrayList<String> l = new ArrayList<>();
            for (float f : a) l.add(Float.toString(f));
            return "[" + String.join(", ", l) + "]";
        }
        if (v instanceof TFloatArrayList a) {
            ArrayList<String> l = new ArrayList<>();
            for (int i = 0; i < a.size(); i++) l.add(Float.toString(a.get(i)));
            return "[" + String.join(", ", l) + "]";
        }
        if (v instanceof KahluaTable t) return table(t);
        if (v instanceof Texture t) return "texture:" + esc(t.getName());
        if (v instanceof Set<?> s) {
            ArrayList<String> l = new ArrayList<>();
            for (Object o : s) l.add(val(o));
            l.sort(null);
            return "[" + String.join(", ", l) + "]";
        }
        if (v instanceof Map<?, ?> m) {
            ArrayList<String> l = new ArrayList<>();
            for (Map.Entry<?, ?> e : m.entrySet()) l.add(val(e.getKey()) + "=" + val(e.getValue()));
            l.sort(null);
            return "{" + String.join(", ", l) + "}";
        }
        if (v instanceof Collection<?> c) {
            ArrayList<String> l = new ArrayList<>();
            for (Object o : c) l.add(val(o));
            return "[" + String.join(", ", l) + "]";
        }
        return esc(v);
    }

    static void dump(StringBuilder sb, String path, Object o, Object root) throws Exception {
        for (Field f : o.getClass().getDeclaredFields()) {
            if (Modifier.isStatic(f.getModifiers())) continue;
            f.setAccessible(true);
            Object v = f.get(o);
            String head = path + "\t" + f.getName() + "\t";
            if (f.getName().equals("owner") && o instanceof ModelAttachment) {
                sb.append(head).append(v == null ? "null" : v == root ? "self" : "other").append('\n');
            } else if (v instanceof List<?> l && !l.isEmpty() && complex(l.get(0))) {
                sb.append(head).append("size ").append(l.size()).append('\n');
                for (int i = 0; i < l.size(); i++) dump(sb, path + "\t" + f.getName() + "[" + i + "]", l.get(i), root);
            } else if (v instanceof Map<?, ?> m && !m.isEmpty() && m.values().iterator().next() instanceof KahluaTable) {
                sb.append(head).append("size ").append(m.size()).append('\n');
                ArrayList<String> keys = new ArrayList<>();
                for (Object k : m.keySet()) keys.add((String) k);
                keys.sort(null);
                for (String k : keys) sb.append(path).append('\t').append(f.getName()).append('[').append(esc(k)).append("]\t").append(val(m.get(k))).append('\n');
            } else if (complex(v)) {
                sb.append(head).append("object\n");
                dump(sb, path + "\t" + f.getName(), v, root);
            } else {
                sb.append(head).append(val(v)).append('\n');
            }
        }
    }

    static void base(StringBuilder sb, String path, BaseScriptObject s) {
        sb.append(path).append("\tmodule\t").append(s.getModule() == null ? "null" : esc(s.getModule().getName()))
            .append("\tenabled\t").append(s.isEnabled()).append("\tdebugOnly\t").append(s.isDebugOnly())
            .append("\tscriptVersion\t").append(Long.toHexString(s.getScriptVersion()))
            .append("\tbodies\t").append(s.getLoadedScriptBodyCount()).append('\n');
    }

    public static void main(String[] a) throws Exception {
        ServerHarness.run(a[0], out -> {
            StringBuilder sb = new StringBuilder();
            ArrayList<VehicleScript> vehicles = new ArrayList<>(ScriptManager.instance.getAllVehicleScripts());
            vehicles.sort((x, y) -> x.getFullName().compareTo(y.getFullName()));
            for (VehicleScript v : vehicles) {
                String p = esc(v.getFullName());
                base(sb, p, v);
                dump(sb, p, v, v);
            }
            ArrayList<VehicleTemplate> templates = new ArrayList<>(ScriptManager.instance.getAllVehicleTemplates());
            templates.sort((x, y) -> x.getScriptObjectFullType().compareTo(y.getScriptObjectFullType()));
            for (VehicleTemplate t : templates) {
                String p = "template:" + esc(t.getScriptObjectFullType());
                sb.append(p).append("\tname\t").append(esc(t.name)).append("\tbody\t").append(t.body.length()).append('\t')
                    .append(t.body.hashCode()).append("\tscript\t").append(t.script == null ? "null" : "built").append('\n');
                if (t.script != null) {
                    base(sb, p, t.script);
                    dump(sb, p, t.script, t.script);
                }
            }
            out.print(sb);
        });
    }
}
