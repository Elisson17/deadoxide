import java.io.PrintStream;
import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.Collection;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import zombie.characters.skills.PerkFactory;
import zombie.scripting.ScriptManager;
import zombie.scripting.entity.components.crafting.CraftRecipe;
import zombie.scripting.entity.components.crafting.InputScript;
import zombie.scripting.entity.components.crafting.OutputScript;
import zombie.scripting.objects.BaseScriptObject;
import zombie.scripting.objects.Item;

// Inside the booted original: every craftRecipe script (ScriptManager.getAllCraftRecipes, the craftRecipe buckets of
// all modules), sorted by full type (String.compareTo), with the fields Load, InitLoadPP, OnScriptsLoaded and
// OnPostWorldDictionaryInit set, read by reflection (not printed: iconTexture, categoryBits, and the inputs' resolved
// items, fluids and energies). Each line starts with the recipe's full type. IO lines (inputs, outputs and their '+'/'-'
// lines) are printed in the recipe's ioLines order and referred to by their index there. Strings: '\\', tab, CR, LF
// and every char outside 0x20..0x7e escaped (\\uXXXX); floats with Float.toString; null as "null".
// Args: <scratch dir>
public class RecipesProbe {
    static Object f(Object o, String name) throws Exception {
        for (Class<?> c = o.getClass(); c != null; c = c.getSuperclass()) {
            try {
                Field fl = c.getDeclaredField(name);
                fl.setAccessible(true);
                return fl.get(o);
            } catch (NoSuchFieldException e) {
                // superclass
            }
        }
        throw new NoSuchFieldException(name);
    }

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

    static String val(Object v) {
        if (v instanceof Float fl) return Float.toString(fl);
        if (v instanceof Collection<?> c) {
            StringBuilder sb = new StringBuilder("[");
            boolean first = true;
            for (Object e : c) {
                if (!first) sb.append(", ");
                first = false;
                sb.append(val(e));
            }
            return sb.append(']').toString();
        }
        if (v instanceof PerkFactory.Perk p) return esc(p.getId());
        if (v instanceof Item it) return esc(it.getFullName());
        if (v instanceof BaseScriptObject b) return esc(b.getScriptObjectFullType());
        return esc(v);
    }

    static String skills(Object list, boolean xp) throws Exception {
        if (list == null) return "null";
        StringBuilder sb = new StringBuilder("[");
        boolean first = true;
        for (Object e : (List<?>) list) {
            if (!first) sb.append(", ");
            first = false;
            sb.append(val(f(e, "perk"))).append(':').append(f(e, xp ? "amount" : "level"));
        }
        return sb.append(']').toString();
    }

    static String io(List<?> ioLines, Object script) {
        if (script == null) return "null";
        for (int i = 0; i < ioLines.size(); i++) if (ioLines.get(i) == script) return Integer.toString(i);
        return "?";
    }

    static String ios(List<?> ioLines, Collection<?> scripts) {
        StringBuilder sb = new StringBuilder("[");
        boolean first = true;
        for (Object s : scripts) {
            if (!first) sb.append(", ");
            first = false;
            sb.append(io(ioLines, s));
        }
        return sb.append(']').toString();
    }

    public static void main(String[] a) throws Exception {
        ServerHarness.run(a[0], out -> dump(out));
    }

    static void dump(PrintStream out) throws Exception {
        ArrayList<CraftRecipe> recipes = new ArrayList<>(ScriptManager.instance.getAllCraftRecipes());
        recipes.sort((x, y) -> x.getScriptObjectFullType().compareTo(y.getScriptObjectFullType()));
        StringBuilder sb = new StringBuilder();
        for (CraftRecipe r : recipes) {
            String ft = esc(r.getScriptObjectFullType());
            List<?> ioLines = (List<?>) f(r, "ioLines");
            ArrayList<String> lines = new ArrayList<>();
            lines.add("name\t" + val(f(r, "name")));
            lines.add("translationName\t" + val(f(r, "translationName")));
            lines.add("enabled\t" + f(r, "enabled") + "\tdebugOnly\t" + f(r, "debugOnly"));
            lines.add("modId\t" + val(f(r, "modId")) + "\texistsAsVanilla\t" + f(r, "existsAsVanilla"));
            lines.add("scriptVersion\t" + Long.toHexString(r.getScriptVersion()));
            lines.add("iconName\t" + val(f(r, "iconName")));
            lines.add("recipeGroup\t" + val(f(r, "recipeGroup")));
            lines.add("needToBeLearn\t" + f(r, "needToBeLearn"));
            lines.add("skillRequired\t" + skills(f(r, "skillRequired"), false));
            lines.add("autoLearnAny\t" + skills(f(r, "autoLearnAny"), false));
            lines.add("autoLearnAll\t" + skills(f(r, "autoLearnAll"), false));
            lines.add("xpAward\t" + skills(f(r, "xpAward"), true));
            lines.add("metaRecipe\t" + val(f(r, "metaRecipe")));
            lines.add("time\t" + f(r, "time"));
            lines.add("loadedTimedActionScript\t" + val(f(r, "loadedTimedActionScript")));
            lines.add("timedActionScript\t" + val(f(r, "timedActionScript")));
            HashMap<?, ?> luaCalls = (HashMap<?, ?>) f(r, "luaCalls");
            StringBuilder lc = new StringBuilder("luaCalls");
            for (CraftRecipe.LuaCall c : CraftRecipe.LuaCall.values()) {
                if (luaCalls.containsKey(c)) lc.append('\t').append(c).append('=').append(val(luaCalls.get(c)));
            }
            lines.add(lc.toString());
            lines.add("onAddToMenu\t" + val(f(r, "onAddToMenu")));
            lines.add("category\t" + val(f(r, "category")));
            lines.add("categoryTags\t" + val(f(r, "categoryTags")));
            lines.add("animation\t" + val(f(r, "animation")));
            lines.add("tooltip\t" + val(f(r, "tooltip")));
            lines.add("allowBatchCraft\t" + f(r, "allowBatchCraft") + "\tcanWalk\t" + f(r, "canWalk"));
            lines.add("researchSkillLevel\t" + f(r, "researchSkillLevel"));
            lines.add("researchAll\t" + val(f(r, "researchAll")) + "\tresearchAny\t" + val(f(r, "researchAny")));
            lines.add("inputs\t" + ios(ioLines, (Collection<?>) f(r, "inputs")) + "\toutputs\t" + ios(ioLines, (Collection<?>) f(r, "outputs")));
            for (int i = 0; i < ioLines.size(); i++) {
                Object s = ioLines.get(i);
                StringBuilder l = new StringBuilder("io\t").append(i);
                if (s instanceof InputScript in) {
                    l.append("\tin\t").append(f(in, "type"));
                    l.append("\tline\t").append(val(f(in, "originalLine")));
                    l.append("\tamount\t").append(val(f(in, "amount"))).append('\t').append(val(f(in, "maxamount")));
                    l.append("\tamounts\t").append(val(f(in, "amounts"))).append('\t').append(val(f(in, "maxamounts")));
                    l.append("\tloadedItems\t").append(val(f(in, "loadedItems")));
                    l.append("\tloadedFluids\t").append(val(f(in, "loadedFluids")));
                    l.append("\tloadedEnergies\t").append(val(f(in, "loadedEnergies")));
                    ArrayList<String> tags = new ArrayList<>();
                    for (Object t : (Collection<?>) f(in, "itemTags")) tags.add(t == null ? "null" : t.toString());
                    tags.sort(null);
                    l.append("\titemTags\t").append(val(tags));
                    l.append("\tcategories\t").append(val(f(in, "categories")));
                    l.append("\tany\t").append(f(in, "acceptsAnyItem")).append('\t').append(f(in, "acceptsAnyFluid")).append('\t').append(f(in, "acceptsAnyEnergy"));
                    l.append("\tapplyMode\t").append(f(in, "itemApplyMode")).append("\tfluidMode\t").append(f(in, "fluidMatchMode"));
                    l.append("\tapplyOnTick\t").append(f(in, "applyOnTick")).append("\tshapedIndex\t").append(f(in, "shapedIndex"));
                    l.append("\tflags\t").append(val(f(in, "flags")));
                    l.append("\tcreateTo\t").append(io(ioLines, f(in, "createToItemScript")));
                    l.append("\tconsumeFrom\t").append(io(ioLines, f(in, "consumeFromItemScript")));
                    l.append("\tparent\t").append(io(ioLines, f(in, "parentScript")));
                } else {
                    OutputScript o = (OutputScript) s;
                    l.append("\tout\t").append(f(o, "type"));
                    l.append("\tline\t").append(val(f(o, "originalLine")));
                    l.append("\tamount\t").append(val(f(o, "amount"))).append('\t').append(val(f(o, "maxamount")));
                    l.append("\tchance\t").append(val(f(o, "chance")));
                    l.append("\tloadedFluid\t").append(val(f(o, "loadedFluid"))).append("\tloadedEnergy\t").append(val(f(o, "loadedEnergy")));
                    l.append("\tapplyMode\t").append(f(o, "itemApplyMode")).append("\tfluidMode\t").append(f(o, "fluidMatchMode"));
                    l.append("\tapplyOnTick\t").append(f(o, "applyOnTick")).append("\tshapedIndex\t").append(f(o, "shapedIndex"));
                    l.append("\tflags\t").append(val(f(o, "flags")));
                    l.append("\tcreateTo\t").append(io(ioLines, f(o, "createToItemScript")));
                    Object m = f(o, "outputMapper");
                    l.append("\tmapper\t").append(m == null ? "null" : val(f(m, "name")));
                    l.append("\tresults\t").append(m == null ? "null" : val(f(m, "resultItems")));
                }
                lines.add(l.toString());
            }
            lines.add("prop1\t" + io(ioLines, f(r, "prop1")) + "\tprop2\t" + io(ioLines, f(r, "prop2")));
            lines.add("toolLeft\t" + io(ioLines, f(r, "toolLeft")) + "\ttoolRight\t" + io(ioLines, f(r, "toolRight")) + "\tusesTools\t" + f(r, "usesTools"));
            lines.add("hasOnTick\t" + f(r, "hasOnTickInputs") + "\t" + f(r, "hasOnTickOutputs"));
            HashMap<?, ?> mappers = (HashMap<?, ?>) f(r, "outputMappers");
            ArrayList<String> names = new ArrayList<>();
            for (Object k : mappers.keySet()) names.add((String) k);
            names.sort(null);
            for (String n : names) {
                Object m = mappers.get(n);
                StringBuilder l = new StringBuilder("outputMapper\t").append(esc(n));
                Object d = f(m, "defaultOutputEntree");
                l.append("\tdefault\t").append(d == null ? "null" : val(f(d, "fullType")) + "=" + val(f(d, "result")));
                l.append("\tentrees\t[");
                boolean first = true;
                for (Object e : (List<?>) f(m, "entrees")) {
                    if (!first) l.append(", ");
                    first = false;
                    l.append(val(f(e, "fullType"))).append('=').append(val(f(e, "result")));
                    l.append(" <- ").append(val(f(e, "patternFullTypes"))).append('=').append(val(f(e, "pattern")));
                }
                ArrayList<String> map = new ArrayList<>();
                for (Map.Entry<?, ?> e : ((HashMap<?, ?>) f(m, "entreeMap")).entrySet()) {
                    map.add(val(e.getKey()) + "=" + ((List<?>) f(m, "entrees")).indexOf(e.getValue()));
                }
                map.sort(null);
                l.append("]\tentreeMap\t").append(val(map));
                l.append("\tresults\t").append(val(f(m, "resultItems")));
                l.append("\tinputs\t").append(ios(ioLines, (Collection<?>) f(m, "inputScripts")));
                lines.add(l.toString());
            }
            Object om = f(r, "overlayMapper");
            StringBuilder l = new StringBuilder("overlayMapper\tdefault\t").append(val(f(om, "defaultStyles")));
            ArrayList<String> styles = new ArrayList<>();
            for (Map.Entry<?, ?> e : ((HashMap<?, ?>) f(om, "styleMap")).entrySet()) {
                styles.add(((Item) e.getKey()).getFullName() + "=" + e.getValue());
            }
            styles.sort(null);
            l.append("\tstyles\t").append(val(styles)).append("\tinput\t").append(io(ioLines, f(om, "inputScript")));
            lines.add(l.toString());
            for (String line : lines) sb.append(ft).append('\t').append(line).append('\n');
        }
        out.print(sb);
    }
}
