import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Map;
import java.util.Set;
import zombie.core.Color;
import zombie.core.Colors;
import zombie.core.textures.Texture;
import zombie.scripting.ScriptManager;
import zombie.scripting.objects.BaseScriptObject;
import zombie.scripting.objects.XuiColorsScript;
import zombie.scripting.objects.XuiLayoutScript;
import zombie.scripting.objects.XuiSkinScript;
import zombie.scripting.ui.XuiLuaStyle;
import zombie.scripting.ui.XuiManager;
import zombie.scripting.ui.XuiScript;
import zombie.scripting.ui.XuiSkin;

// Inside the booted original: the XUI scripts as ScriptManager loaded them (getAllXuiConfigScripts, getAllXuiLayouts,
// getAllXuiStyles, getAllXuiDefaultStyles, getAllXuiColors, getAllXuiSkinScripts, each sorted by full type), then the
// static state XuiManager.ParseScripts left: XuiLuaStyle's ALLOWED_VAR_TYPES and var registry, every static field of
// XuiManager with every XuiSkin it holds, and the colours of zombie.core.Colors by name. Every instance field is
// printed by reflection in declaration order, one line per field: "<path>\t<field>\t<value>"; a nested object as
// "object" and a block under "<path>\t<field>"; a map as "size N" and one entry per key in sorted order (path
// "<field>[key]", a plain value as "=\t<value>"); a list of objects as "size N" and one block per element. Strings:
// '\\', tab, CR, LF and every UTF-16 unit outside 0x20..0x7e escaped (\\uXXXX); floats and doubles with
// Float/Double.toString; colours as rgba(r, g, b, a); references to a skin, script, style or XuiScript by name; a
// var's parent as "owner" when it is the style that holds it. XuiLuaStyle.varsMap is printed as the index of each
// var in `vars`. Args: <scratch dir>
public class XuiProbe {
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

    static String rgba(Color c) {
        return "rgba(" + Float.toString(c.r) + ", " + Float.toString(c.g) + ", " + Float.toString(c.b) + ", " + Float.toString(c.a) + ")";
    }

    // The objects printed as blocks: the nested classes of XuiSkin and XuiSkinScript, styles, vars, colour scripts.
    static boolean complex(Object v) {
        if (v == null) return false;
        String n = v.getClass().getName();
        return v instanceof XuiLuaStyle || v instanceof XuiLuaStyle.XuiVar || v instanceof XuiColorsScript
            || n.startsWith("zombie.scripting.ui.XuiSkin$") || n.startsWith("zombie.scripting.objects.XuiSkinScript$");
    }

    static String val(Object v) {
        if (v == null) return "null";
        if (v instanceof String s) return esc(s);
        if (v instanceof Float f) return Float.toString(f);
        if (v instanceof Double d) return Double.toString(d);
        if (v instanceof Enum<?> e) return e.name();
        if (v instanceof Color c) return rgba(c);
        if (v instanceof Texture t) return "texture:" + esc(t.getName());
        if (v instanceof XuiSkin s) return "skin:" + esc(s.getName());
        if (v instanceof BaseScriptObject b) return "script:" + esc(b.getScriptObjectFullType());
        if (v instanceof XuiLuaStyle s) return "style:" + esc(s.getXuiLuaClass()) + "/" + esc(s.getXuiStyleName());
        if (v instanceof XuiScript x) return "xuiscript:" + esc(x.getXuiLayoutName());
        if (v instanceof Set<?> s) {
            ArrayList<String> l = new ArrayList<>();
            for (Object o : s) l.add(val(o));
            l.sort(null);
            return "[" + String.join(", ", l) + "]";
        }
        if (v instanceof Collection<?> c) {
            ArrayList<String> l = new ArrayList<>();
            for (Object o : c) l.add(val(o));
            return "[" + String.join(", ", l) + "]";
        }
        return esc(v);
    }

    static void base(StringBuilder sb, String path, BaseScriptObject s) {
        sb.append(path).append("\tmodule\t").append(s.getModule() == null ? "null" : esc(s.getModule().getName()))
            .append("\tenabled\t").append(s.isEnabled()).append("\tdebugOnly\t").append(s.isDebugOnly())
            .append("\tscriptVersion\t").append(Long.toHexString(s.getScriptVersion()))
            .append("\tbodies\t").append(s.getLoadedScriptBodyCount()).append('\n');
    }

    // One map: "size N", then its entries by sorted key.
    static void map(StringBuilder sb, String path, Map<?, ?> m, Object owner) throws Exception {
        sb.append(path).append("\tsize ").append(m.size()).append('\n');
        ArrayList<Object> keys = new ArrayList<>(m.keySet());
        keys.sort((x, y) -> val(x).compareTo(val(y)));
        for (Object k : keys) {
            String p = path + "[" + val(k) + "]";
            Object x = m.get(k);
            if (x instanceof Map<?, ?> inner) map(sb, p, inner, owner);
            else if (owner instanceof XuiLuaStyle st && x instanceof XuiLuaStyle.XuiVar<?, ?> var) {
                int idx = -1;
                for (int i = 0; i < st.getVars().size(); i++) if (st.getVars().get(i) == var) idx = i;
                sb.append(p).append("\t=\t#").append(idx).append('\n');
            } else if (complex(x)) dump(sb, p, x);
            else sb.append(p).append("\t=\t").append(val(x)).append('\n');
        }
    }

    static void field(StringBuilder sb, String path, Object owner, String name, Object v) throws Exception {
        String head = path + "\t" + name + "\t";
        if (name.equals("parent") && owner instanceof XuiLuaStyle.XuiVar && v instanceof XuiLuaStyle st) {
            sb.append(head).append(st == currentStyle ? "owner" : "other:" + val(st)).append('\n');
        } else if (name.equals("totalFile") && v instanceof String s) {
            sb.append(head).append(s.length()).append('\t').append(s.hashCode()).append('\n');
        } else if (name.equals("block") && owner instanceof XuiLayoutScript) {
            sb.append(head).append(v == null ? "null" : "set").append('\n');
        } else if (v instanceof Map<?, ?> m) {
            map(sb, path + "\t" + name, m, owner);
        } else if (v instanceof List<?> l && !l.isEmpty() && complex(l.get(0))) {
            sb.append(head).append("size ").append(l.size()).append('\n');
            for (int i = 0; i < l.size(); i++) dump(sb, path + "\t" + name + "[" + i + "]", l.get(i));
        } else if (complex(v)) {
            sb.append(head).append("object\n");
            dump(sb, path + "\t" + name, v);
        } else {
            sb.append(head).append(val(v)).append('\n');
        }
    }

    static XuiLuaStyle currentStyle;

    static void dump(StringBuilder sb, String path, Object o) throws Exception {
        if (o instanceof BaseScriptObject b) base(sb, path, b);
        if (o instanceof XuiLuaStyle.XuiVar) sb.append(path).append("\tclass\t").append(o.getClass().getSimpleName()).append('\n');
        XuiLuaStyle saved = currentStyle;
        if (o instanceof XuiLuaStyle st) currentStyle = st;
        for (Class<?> c = o.getClass(); c != null && c != Object.class && c != BaseScriptObject.class; c = c.getSuperclass()) {
            for (Field f : c.getDeclaredFields()) {
                if (Modifier.isStatic(f.getModifiers())) continue;
                f.setAccessible(true);
                field(sb, path, o, f.getName(), f.get(o));
            }
        }
        currentStyle = saved;
    }

    static <T extends BaseScriptObject> void scripts(StringBuilder sb, String tag, List<T> list) throws Exception {
        ArrayList<T> l = new ArrayList<>(list);
        l.sort((x, y) -> x.getScriptObjectFullType().compareTo(y.getScriptObjectFullType()));
        sb.append(tag).append("\tsize ").append(l.size()).append('\n');
        for (T s : l) dump(sb, tag + ":" + esc(s.getScriptObjectFullType()), s);
    }

    static void statics(StringBuilder sb, String path, Class<?> c) throws Exception {
        for (Field f : c.getDeclaredFields()) {
            if (!Modifier.isStatic(f.getModifiers())) continue;
            f.setAccessible(true);
            Object v = f.get(null);
            if (f.getName().equals("xuiSkins")) {
                Map<?, ?> m = (Map<?, ?>) v;
                sb.append(path).append('\t').append(f.getName()).append("\tsize ").append(m.size()).append('\n');
                ArrayList<String> keys = new ArrayList<>();
                for (Object k : m.keySet()) keys.add((String) k);
                keys.sort(null);
                for (String k : keys) dump(sb, "skin:" + esc(k), m.get(k));
            } else {
                field(sb, path, null, f.getName(), v);
            }
        }
    }

    public static void main(String[] a) throws Exception {
        ServerHarness.run(a[0], out -> {
            StringBuilder sb = new StringBuilder();
            ScriptManager sm = ScriptManager.instance;
            scripts(sb, "xuiConfig", sm.getAllXuiConfigScripts());
            scripts(sb, "xuiLayout", sm.getAllXuiLayouts());
            scripts(sb, "xuiStyle", sm.getAllXuiStyles());
            scripts(sb, "xuiDefaultStyle", sm.getAllXuiDefaultStyles());
            scripts(sb, "xuiGlobalColors", sm.getAllXuiColors());
            scripts(sb, "xuiSkin", sm.getAllXuiSkinScripts());
            statics(sb, "XuiLuaStyle", XuiLuaStyle.class);
            statics(sb, "XuiManager", XuiManager.class);
            XuiSkin def = XuiManager.GetDefaultSkin();
            sb.append("XuiManager\tGetDefaultSkin\t").append(val(def)).append("\tsame\t")
                .append(def != null && def == XuiManager.GetSkin(def.getName())).append('\n');
            ArrayList<String> names = Colors.GetColorNames();
            sb.append("Colors\tsize ").append(names.size()).append('\t').append(Colors.GetColorsCount()).append('\n');
            for (int i = 0; i < names.size(); i++) {
                String n = names.get(i);
                Colors.ColNfo nfo = Colors.GetColorInfo(n);
                sb.append("Colors[").append(i).append("]\t").append(esc(n)).append('\t').append(val(Colors.GetColorByName(n)))
                    .append('\t').append(nfo == null ? "null" : nfo.getColorSet().name()).append('\n');
            }
            ArrayList<String> cb = Colors.CB_GetColorNames();
            sb.append("CB\tsize ").append(cb.size()).append('\n');
            for (int i = 0; i < cb.size(); i++) {
                String n = cb.get(i);
                sb.append("CB[").append(i).append("]\t").append(esc(n)).append('\t').append(val(Colors.CB_GetColorByName(n))).append('\n');
            }
            out.print(sb);
        });
    }
}
