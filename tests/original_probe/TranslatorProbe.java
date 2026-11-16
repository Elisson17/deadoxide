import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.function.Function;
import org.json.JSONException;
import zombie.ZomboidFileSystem;
import zombie.core.Core;
import zombie.core.Language;
import zombie.core.Languages;
import zombie.core.Translator;

/** Oracle probe (tools/oracle.sh translator): the ORIGINAL zombie.core.Translator standalone, started the way
 * GameServer.main starts it (ZomboidFileSystem.init, Languages.init, Translator.loadFiles; no mods). It reads the
 * command file tests/translator_cases.txt (one command per line, fields separated by TAB, `\n \t \r \\ \xHH`
 * escapes and backslash-u with 4 hex digits; `#` comments) and prints, in order:
 *   LANGUAGES n / LANG name text base azerty   the languages after Languages.init, sorted by name; DEFAULT name
 *   userlang <v>             System.setProperty("user.language", v) (getLanguage's last fallback before EN)
 *   call <method> <arg>...   stored; every stored call runs after each load, in file order
 *   load <option> full|digest  Core option "language" (`_` = empty), Translator.loadFiles, then:
 *       LOAD, LANGUAGE, STACK (forLanguageStack), MAP <name> <size> per BY_NAME map and the never-cleared
 *       recordedMedia_EN (full: every entry sorted by key; digest: FNV-1a 64 of those entry lines), the stored calls,
 *       MISSING (Translator.missing, sorted), BR (the brReplacements cache, sorted)
 *   fmt <template> <arg>...  getUI().put("UI_Probe", template) then getText/getTextOrNull("UI_Probe", args) and
 *                            getText(template, args) (the missing-key path: the key itself is formatted)
 *   json <id> <text>...      each text written as media/lua/shared/Translate/EN/J<id>_<i>.json under the scratch dir
 *                            and read into one fresh map by the private tryFillMapFromFile (EN, formatFixer)
 * Arguments: s:<text> String, d:<double> Double, b:true|false Boolean, n null. Strings print escaped like
 * LuaLoadEnvProbe (printable ASCII kept, other UTF-16 units as \\uXXXX). Compared with `pz-dump translator`.
 * Args: <scratch cache dir> <command file>; run from the game root. */
public class TranslatorProbe {
    static final StringBuilder sb = new StringBuilder();

    static void esc(StringBuilder b, String s) {
        b.append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c >= 0x20 && c < 0x7f && c != '"' && c != '\\') b.append(c);
            else b.append(String.format("\\u%04x", (int) c));
        }
        b.append('"');
    }

    static String esc(String s) {
        if (s == null) return "null";
        StringBuilder b = new StringBuilder();
        esc(b, s);
        return b.toString();
    }

    /** The command file's escapes, as bytes (\xHH is a raw byte, everything else UTF-8). */
    static byte[] unescape(String s) {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c == '\\' && i + 1 < s.length()) {
                char e = s.charAt(++i);
                switch (e) {
                    case 'n' -> out.write('\n');
                    case 't' -> out.write('\t');
                    case 'r' -> out.write('\r');
                    case '\\' -> out.write('\\');
                    case 'x' -> { out.write(Integer.parseInt(s.substring(i + 1, i + 3), 16)); i += 2; }
                    case 'u' -> { utf8(out, (char) Integer.parseInt(s.substring(i + 1, i + 5), 16)); i += 4; }
                    default -> { out.write('\\'); utf8(out, e); }
                }
            } else if (Character.isHighSurrogate(c) && i + 1 < s.length()) {
                byte[] b = s.substring(i, i + 2).getBytes(StandardCharsets.UTF_8);
                out.write(b, 0, b.length);
                i++;
            } else {
                utf8(out, c);
            }
        }
        return out.toByteArray();
    }

    static void utf8(ByteArrayOutputStream out, char c) {
        if (c < 0x80) out.write(c);
        else if (c < 0x800) { out.write(0xc0 | (c >> 6)); out.write(0x80 | (c & 0x3f)); }
        else { out.write(0xe0 | (c >> 12)); out.write(0x80 | ((c >> 6) & 0x3f)); out.write(0x80 | (c & 0x3f)); }
    }

    static String text(String field) {
        return new String(unescape(field), StandardCharsets.UTF_8);
    }

    static Object arg(String a) {
        if (a.equals("n")) return null;
        String v = text(a.substring(2));
        return switch (a.charAt(0)) {
            case 's' -> v;
            case 'd' -> Double.parseDouble(v);
            case 'b' -> Boolean.parseBoolean(v);
            default -> throw new IllegalArgumentException(a);
        };
    }

    static String argText(Object o) {
        if (o == null) return "n";
        if (o instanceof Double d) return "d:" + Long.toHexString(Double.doubleToRawLongBits(d));
        if (o instanceof Boolean b) return "b:" + b;
        return "s:" + esc((String) o);
    }

    static String error(Throwable t) {
        if (t instanceof InvocationTargetException ite) t = ite.getCause();
        if (t instanceof RuntimeException r && r.getMessage() != null && r.getMessage().startsWith("JSON Error in: ") && r.getCause() != null) {
            Throwable c = r.getCause();
            return "ERROR " + c.getClass().getName() + (c instanceof JSONException ? ": " + c.getMessage() : "");
        }
        if (t instanceof IllegalArgumentException) return "ERROR " + t.getClass().getName() + ": " + t.getMessage();
        return "ERROR " + t.getClass().getName();
    }

    static long fnv(long h, String line) {
        byte[] b = line.getBytes(StandardCharsets.UTF_8);
        for (byte x : b) {
            h ^= (x & 0xff);
            h *= 0x100000001b3L;
        }
        return h;
    }

    static void dumpMap(String name, Map<String, String> map, boolean full) {
        TreeMap<String, String> sorted = new TreeMap<>(map);
        if (full) {
            sb.append("MAP ").append(name).append(' ').append(sorted.size()).append('\n');
            for (Map.Entry<String, String> e : sorted.entrySet())
                sb.append(esc(e.getKey())).append(" = ").append(esc(e.getValue())).append('\n');
        } else {
            long h = 0xcbf29ce484222325L;
            for (Map.Entry<String, String> e : sorted.entrySet()) h = fnv(h, esc(e.getKey()) + " = " + esc(e.getValue()) + "\n");
            sb.append("MAP ").append(name).append(' ').append(sorted.size()).append(' ').append(String.format("%016x", h)).append('\n');
        }
    }

    static String call(String method, List<String> a) {
        String s0 = a.isEmpty() ? null : text(a.get(0));
        try {
            Object r = switch (method) {
                case "getText", "getTextOrNull" -> {
                    Object[] args = new Object[a.size() - 1];
                    for (int i = 1; i < a.size(); i++) args[i - 1] = arg(a.get(i));
                    yield method.equals("getText") ? Translator.getText(s0, args) : Translator.getTextOrNull(s0, args);
                }
                case "getDisplayItemName" -> Translator.getDisplayItemName(s0);
                case "getItemNameFromFullType" -> Translator.getItemNameFromFullType(s0);
                case "getItemEvolvedRecipeName" -> Translator.getItemEvolvedRecipeName(s0);
                case "setDefaultItemEvolvedRecipeName" -> { Translator.setDefaultItemEvolvedRecipeName(s0, text(a.get(1))); yield "(void)"; }
                case "getMoveableDisplayName" -> Translator.getMoveableDisplayName(s0);
                case "getMoveableDisplayNameOrNull" -> Translator.getMoveableDisplayNameOrNull(s0);
                case "getRecipeName" -> Translator.getRecipeName(s0);
                case "getRecipeGroupName" -> Translator.getRecipeGroupName(s0);
                case "getTextMediaEN" -> Translator.getTextMediaEN(s0);
                case "getAttributeText" -> Translator.getAttributeText(s0);
                case "getAttributeTextOrNull" -> Translator.getAttributeTextOrNull(s0);
                case "getFluidText" -> Translator.getFluidText(s0);
                case "getEntityText" -> Translator.getEntityText(s0);
                case "getMapLabelText" -> Translator.getMapLabelText(s0);
                default -> throw new IllegalStateException("unknown method " + method);
            };
            return esc((String) r);
        } catch (IllegalStateException e) {
            throw e;
        } catch (Throwable t) {
            return error(t);
        }
    }

    static String callText(String method, List<String> a) {
        StringBuilder b = new StringBuilder("CALL ").append(method).append('(');
        for (int i = 0; i < a.size(); i++) {
            if (i > 0) b.append(", ");
            b.append(i == 0 || !method.startsWith("getText") ? esc(text(a.get(i))) : argText(arg(a.get(i))));
        }
        return b.append(")").toString();
    }

    @SuppressWarnings("unchecked")
    public static void main(String[] a) throws Exception {
        PrintStream out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, "UTF-8");
        System.setOut(System.err);
        ZomboidFileSystem.instance.setCacheDir(a[0]);
        ZomboidFileSystem.instance.init();
        Languages.instance.init();
        System.err.println("ORACLE Core.IS_DEV=" + Core.IS_DEV + " Core.debug=" + Core.debug + " Translator.debug=" + Translator.debug
            + " mods=" + ZomboidFileSystem.instance.getModIDs() + " perks=" + zombie.characters.skills.PerkFactory.PerkList.size());

        Method fill = Translator.class.getDeclaredMethod("tryFillMapFromFile", String.class, String.class, Map.class, Language.class, Function.class);
        fill.setAccessible(true);
        Method fixer = Translator.class.getDeclaredMethod("formatFixer", String.class);
        fixer.setAccessible(true);
        Function<String, String> formatFixer = s -> {
            try {
                return (String) fixer.invoke(null, s);
            } catch (InvocationTargetException e) {
                throw (RuntimeException) e.getCause();
            } catch (IllegalAccessException e) {
                throw new IllegalStateException(e);
            }
        };
        Field fMissing = Translator.class.getDeclaredField("missing");
        fMissing.setAccessible(true);
        Field fBr = Translator.class.getDeclaredField("brReplacements");
        fBr.setAccessible(true);
        Field fMediaEn = Translator.class.getDeclaredField("recordedMedia_EN");
        fMediaEn.setAccessible(true);

        TreeMap<String, Language> langs = new TreeMap<>();
        for (Language l : Languages.instance.getLanguages()) langs.put(l.name(), l);
        sb.append("LANGUAGES ").append(langs.size()).append('\n');
        for (Language l : langs.values())
            sb.append("LANG ").append(l.name()).append(' ').append(esc(l.text())).append(' ').append(esc(l.base())).append(' ').append(l.isAzerty()).append('\n');
        sb.append("DEFAULT ").append(Languages.instance.getDefaultLanguage().name()).append('\n');

        List<List<String>> calls = new ArrayList<>();
        for (String line : Files.readAllLines(Path.of(a[1]), StandardCharsets.UTF_8)) {
            if (line.isEmpty() || line.startsWith("#")) continue;
            List<String> f = new ArrayList<>(List.of(line.split("\t", -1)));
            String cmd = f.remove(0);
            switch (cmd) {
                case "userlang" -> {
                    System.setProperty("user.language", text(f.get(0)));
                    sb.append("USERLANG ").append(esc(text(f.get(0)))).append('\n');
                }
                case "call" -> calls.add(f);
                case "load" -> {
                    String option = f.get(0).equals("_") ? "" : text(f.get(0));
                    boolean full = f.get(1).equals("full");
                    Core.getInstance().setOptionLanguageName(option);
                    sb.append("LOAD ").append(esc(option)).append('\n');
                    try {
                        Translator.loadFiles();
                    } catch (Throwable t) {
                        sb.append(error(t)).append('\n');
                        continue;
                    }
                    sb.append("LANGUAGE ").append(Translator.getLanguage().name()).append('\n');
                    StringBuilder stack = new StringBuilder("STACK");
                    Translator.forLanguageStack(l -> stack.append(' ').append(l.name()));
                    sb.append(stack).append('\n');
                    for (Map.Entry<String, Map<String, String>> m : Translator.BY_NAME.entrySet()) dumpMap(m.getKey(), m.getValue(), full);
                    dumpMap("Recorded_Media_EN", (Map<String, String>) fMediaEn.get(null), full);
                    for (List<String> c : calls) {
                        List<String> args = c.subList(1, c.size());
                        sb.append(callText(c.get(0), args)).append(" = ").append(call(c.get(0), args)).append('\n');
                    }
                    TreeSet<String> missing = new TreeSet<>((Set<String>) fMissing.get(null));
                    sb.append("MISSING ").append(missing.size()).append('\n');
                    for (String m : missing) sb.append("  ").append(esc(m)).append('\n');
                    TreeMap<String, String> br = new TreeMap<>((Map<String, String>) fBr.get(null));
                    sb.append("BR ").append(br.size()).append('\n');
                    for (Map.Entry<String, String> e : br.entrySet()) sb.append("  ").append(esc(e.getKey())).append(" = ").append(esc(e.getValue())).append('\n');
                }
                case "fmt" -> {
                    String template = text(f.get(0));
                    List<String> args = new ArrayList<>(f);
                    Translator.getUI().put("UI_Probe", template);
                    args.set(0, "UI_Probe");
                    sb.append("FMT ").append(esc(template)).append('\n');
                    sb.append("  ").append(callText("getText", args)).append(" = ").append(call("getText", args)).append('\n');
                    sb.append("  ").append(callText("getTextOrNull", args)).append(" = ").append(call("getTextOrNull", args)).append('\n');
                    args.set(0, f.get(0));
                    sb.append("  ").append(callText("getText", args)).append(" = ").append(call("getText", args)).append('\n');
                    Translator.getUI().remove("UI_Probe");
                }
                case "json" -> {
                    String id = f.get(0);
                    Map<String, String> map = new HashMap<>();
                    Path dir = Path.of(a[0], "media", "lua", "shared", "Translate", "EN");
                    Files.createDirectories(dir);
                    String result = null;
                    for (int i = 1; i < f.size() && result == null; i++) {
                        String name = "J" + id + "_" + i;
                        Files.write(dir.resolve(name + ".json"), unescape(f.get(i)));
                        try {
                            fill.invoke(null, a[0], name, map, Languages.instance.getDefaultLanguage(), formatFixer);
                        } catch (Throwable t) {
                            result = error(t);
                        }
                    }
                    if (result != null) {
                        sb.append("JSON ").append(id).append(' ').append(result).append('\n');
                    } else {
                        sb.append("JSON ").append(id).append(" ok ").append(map.size()).append('\n');
                        for (Map.Entry<String, String> e : new TreeMap<>(map).entrySet())
                            sb.append("  ").append(esc(e.getKey())).append(" = ").append(esc(e.getValue())).append('\n');
                    }
                }
                default -> throw new IllegalStateException("unknown command " + cmd);
            }
        }
        out.print(sb);
        out.flush();
        Runtime.getRuntime().halt(0);
    }
}
