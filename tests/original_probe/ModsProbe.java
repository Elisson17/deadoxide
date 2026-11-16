import java.io.File;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;
import zombie.ZomboidFileSystem;
import zombie.core.Core;
import zombie.core.GameVersion;
import zombie.core.Language;
import zombie.core.Languages;
import zombie.core.Translator;
import zombie.core.textures.TextureID;
import zombie.gameStates.ChooseGameInfo;
import zombie.modding.ActiveMods;

/** Oracle probe (tools/oracle.sh mods): the ORIGINAL mod loading standalone, started the way GameServer.main starts
 * the file system (setCacheDir, ZomboidFileSystem.init, Languages.init). The cache dir is <scratch>/java. It runs the
 * commands of tests/mods_cases.txt (one per line, TAB-separated fields, `\n \t \r \\ \xHH` and backslash-u escapes,
 * `#` comments; "$S" in an argument is the cache dir; a missing argument is ""):
 *   lang L          Translator.setLanguage(Languages.getByName(L))
 *   gv S / vint S   GameVersion.parse(S) / ZomboidFileSystem.getGameVersionIntFromName(S)
 *   file P TEXT     write TEXT (bytes) to <cache>/P, parents created; dir P: create <cache>/P
 *   order S         setModFoldersOrder(S), resetModFolders()
 *   folders         getAllModFolders
 *   details ID / available ID / read DIR   ChooseGameInfo.getModDetails / getAvailableModDetails / readModInfo
 *   verdir DIR      getModVersionDirName(Paths.get(DIR))
 *   active LIST ID B / isactive LIST ID / remove LIST ID / checkmissing LIST   ActiveMods.getById(LIST)...
 *   lists           every ActiveMods (s_activeMods) and s_loaded
 *   load LIST       ZomboidFileSystem.loadMods(ActiveMods.getById(LIST).getMods()); loadstr ID: loadMods(String)
 *   get / dirget / known / abs PATH   getString / getDirectoryString / isKnownFile / getAbsolutePath
 *   map             activeFileMap's size, allAbsolutePaths' size, and the entries whose value is in the cache dir
 *   state           ChooseGameInfo's Mods, MissingMods, tempStrings; ZomboidFileSystem's modIdToDir, modDirToMod,
 *                   mods, modFolders, modFoldersOrder
 *   mods            every ChooseGameInfo.Mod of modDirToMod, all fields
 * Strings print escaped (printable ASCII kept, other UTF-16 units as \\uXXXX), the cache dir as $S, the game folder as
 * $G. Compared with `pz-dump mods`. Args: <scratch dir> <command file>; run from the game root. */
public class ModsProbe {
    static final StringBuilder sb = new StringBuilder();
    static String cache;
    static String cacheCanon;
    static String base;

    static void line(String s) {
        sb.append(s).append('\n');
    }

    static String esc(String s) {
        StringBuilder b = new StringBuilder("\"");
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c >= 0x20 && c < 0x7f && c != '"' && c != '\\') b.append(c);
            else b.append(String.format("\\u%04x", (int) c));
        }
        return b.append('"').toString();
    }

    static String p(String s) {
        if (s == null) return "null";
        s = s.replace(cache, "$S");
        if (!cacheCanon.equals(cache)) s = s.replace(cacheCanon, "$S");
        return esc(s.replace(base, "$G"));
    }

    static String list(Collection<String> l) {
        if (l == null) return "null";
        StringBuilder b = new StringBuilder("[");
        for (String s : l) {
            if (b.length() > 1) b.append(", ");
            b.append(p(s));
        }
        return b.append(']').toString();
    }

    static String sorted(Collection<String> l) {
        return list(new TreeSet<>(l));
    }

    static Object get(Object o, Class<?> c, String name) throws Exception {
        Field f = c.getDeclaredField(name);
        f.setAccessible(true);
        return f.get(o);
    }

    static byte[] unescape(String s) {
        java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream();
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
            } else {
                utf8(out, c);
            }
        }
        return out.toByteArray();
    }

    static void utf8(java.io.ByteArrayOutputStream out, char c) {
        if (c < 0x80) out.write(c);
        else if (c < 0x800) { out.write(0xc0 | (c >> 6)); out.write(0x80 | (c & 0x3f)); }
        else { out.write(0xe0 | (c >> 12)); out.write(0x80 | ((c >> 6) & 0x3f)); out.write(0x80 | (c & 0x3f)); }
    }

    static String arg(String field) {
        return new String(unescape(field), StandardCharsets.UTF_8).replace("$S", cache);
    }

    static String modDir(ChooseGameInfo.Mod m) {
        return m == null ? "null" : p(m.getDir());
    }

    static String version(GameVersion v) {
        return v == null ? "null" : p(v.toString()) + " " + v.getInt();
    }

    static String folder(ZomboidFileSystem.PZFolder f) {
        return p(f.absoluteFile == null ? null : f.absoluteFile.getPath()) + " " + p(f.canonicalFile == null ? null : f.canonicalFile.getPath());
    }

    static void modFolder(String label, ZomboidFileSystem.PZModFolder f) {
        line(" " + label + " common " + folder(f.common) + " version " + folder(f.version));
    }

    @SuppressWarnings("unchecked")
    static void dumpMod(ChooseGameInfo.Mod m) throws Exception {
        Class<?> c = ChooseGameInfo.Mod.class;
        line("MOD " + p(m.dir));
        line(" commonDir " + p(m.commonDir));
        line(" versionDir " + p(m.versionDir));
        modFolder("baseFile", m.baseFile);
        modFolder("mediaFile", m.mediaFile);
        modFolder("actionGroupsFile", m.actionGroupsFile);
        modFolder("animSetsFile", m.animSetsFile);
        modFolder("animsXFile", m.animsXFile);
        line(" posters " + list((List<String>) get(m, c, "posters")));
        line(" require " + list(m.getRequire()));
        line(" incompatible " + list(m.getIncompatible()));
        line(" loadAfter " + list(m.getLoadAfter()));
        line(" loadBefore " + list(m.getLoadBefore()));
        for (String f : new String[] {"name", "desc", "id", "url", "author", "modVersion", "icon", "category", "workshopId", "source"}) {
            line(" " + f + " " + p((String) get(m, c, f)));
        }
        line(" availableDone " + get(m, c, "availableDone"));
        line(" available " + get(m, c, "available"));
        line(" versionMin " + version(m.getVersionMin()));
        line(" versionMax " + version(m.getVersionMax()));
        StringBuilder packs = new StringBuilder();
        for (ChooseGameInfo.PackFile pf : m.getPacks()) packs.append(packs.length() == 0 ? "" : ", ").append(p(pf.name)).append(' ').append(pf.flags);
        line(" packs [" + packs + "]");
        StringBuilder defs = new StringBuilder();
        for (ChooseGameInfo.TileDef td : m.getTileDefs()) defs.append(defs.length() == 0 ? "" : ", ").append(p(td.name)).append(' ').append(td.fileNumber);
        line(" tileDefs [" + defs + "]");
        line(" read " + get(m, c, "read"));
        line(" valid " + get(m, c, "valid"));
    }

    static String activeText(ActiveMods a) throws Exception {
        return p((String) get(a, ActiveMods.class, "id")) + " mods " + list(a.getMods()) + " maps " + list(a.getMapOrder());
    }

    @SuppressWarnings("unchecked")
    static void state() throws Exception {
        ZomboidFileSystem fs = ZomboidFileSystem.instance;
        line("STATE");
        Map<String, ChooseGameInfo.Mod> mods = (Map<String, ChooseGameInfo.Mod>) get(null, ChooseGameInfo.class, "Mods");
        line(" Mods " + mods.size());
        for (Map.Entry<String, ChooseGameInfo.Mod> e : new TreeMap<>(mods).entrySet()) line("  " + p(e.getKey()) + " -> " + modDir(e.getValue()));
        line(" MissingMods " + sorted((Set<String>) get(null, ChooseGameInfo.class, "MissingMods")));
        line(" tempStrings " + list((List<String>) get(null, ChooseGameInfo.class, "tempStrings")));
        Map<String, String> idToDir = (Map<String, String>) get(fs, ZomboidFileSystem.class, "modIdToDir");
        line(" modIdToDir " + idToDir.size());
        for (Map.Entry<String, String> e : new TreeMap<>(idToDir).entrySet()) line("  " + p(e.getKey()) + " -> " + p(e.getValue()));
        Map<String, ChooseGameInfo.Mod> dirToMod = (Map<String, ChooseGameInfo.Mod>) get(fs, ZomboidFileSystem.class, "modDirToMod");
        line(" modDirToMod " + sorted(dirToMod.keySet()));
        line(" mods " + list(fs.getModIDs()));
        line(" modFolders " + list((List<String>) get(fs, ZomboidFileSystem.class, "modFolders")));
        line(" modFoldersOrder " + list((List<String>) get(fs, ZomboidFileSystem.class, "modFoldersOrder")));
    }

    @SuppressWarnings("unchecked")
    static void run(String[] f) throws Exception {
        ZomboidFileSystem fs = ZomboidFileSystem.instance;
        // a missing field is the empty string (the commands file keeps no trailing whitespace)
        String a1 = arg(f.length > 1 ? f[1] : "");
        String a2 = arg(f.length > 2 ? f[2] : "");
        switch (f[0]) {
            case "lang" -> {
                Translator.setLanguage(Languages.instance.getByName(a1));
                line("LANG " + p(a1) + " -> " + p(Translator.getLanguage().name()));
            }
            case "gv" -> {
                GameVersion v = GameVersion.parse(a1);
                line("GV " + p(a1) + " -> " + version(v));
            }
            case "vint" -> line("VINT " + p(a1) + " -> " + fs.getGameVersionIntFromName(a1));
            case "file" -> {
                Path path = Paths.get(cache, f[1]);
                Files.createDirectories(path.getParent());
                Files.write(path, unescape(f.length > 2 ? f[2] : ""));
            }
            case "dir" -> Files.createDirectories(Paths.get(cache, f[1]));
            case "order" -> {
                fs.setModFoldersOrder(a1);
                fs.resetModFolders();
                line("ORDER " + p(a1));
            }
            case "folders" -> {
                List<String> out = new ArrayList<>();
                fs.getAllModFolders(out);
                line("FOLDERS " + out.size());
                for (String s : out) line(" " + p(s));
            }
            case "details" -> line("DETAILS " + p(a1) + " -> " + modDir(ChooseGameInfo.getModDetails(a1)));
            case "available" -> line("AVAILABLE " + p(a1) + " -> " + modDir(ChooseGameInfo.getAvailableModDetails(a1)));
            case "read" -> line("READ " + p(a1) + " -> " + modDir(ChooseGameInfo.readModInfo(a1)));
            case "verdir" -> line("VERDIR " + p(a1) + " -> " + p(fs.getModVersionDirName(Paths.get(a1))));
            case "active" -> {
                ActiveMods a = ActiveMods.getById(a1);
                a.setModActive(a2, Boolean.parseBoolean(arg(f[3])));
                line("ACTIVE " + p(a1) + " " + p(a2) + " " + f[3] + " -> " + activeText(a));
            }
            case "isactive" -> line("ISACTIVE " + p(a1) + " " + p(a2) + " -> " + ActiveMods.getById(a1).isModActive(a2));
            case "remove" -> {
                ActiveMods a = ActiveMods.getById(a1);
                a.removeMod(a2);
                line("REMOVE " + p(a1) + " " + p(a2) + " -> " + activeText(a));
            }
            case "checkmissing" -> {
                ActiveMods a = ActiveMods.getById(a1);
                a.checkMissingMods();
                line("CHECKMISSING " + p(a1) + " -> " + activeText(a));
            }
            case "lists" -> {
                List<ActiveMods> all = (List<ActiveMods>) get(null, ActiveMods.class, "s_activeMods");
                line("LISTS " + all.size());
                for (ActiveMods a : all) line(" " + activeText(a));
                line(" loaded " + activeText((ActiveMods) get(null, ActiveMods.class, "s_loaded")));
            }
            case "load" -> {
                fs.loadMods(ActiveMods.getById(a1).getMods());
                line("LOAD " + p(a1) + " -> " + list(fs.getModIDs()));
            }
            case "loadstr" -> {
                fs.loadMods(a1);
                line("LOADSTR " + p(a1) + " -> " + list(fs.getModIDs()));
            }
            case "get" -> {
                String r = fs.getString(a1);
                line("GET " + p(a1) + (r == a1 ? " unchanged" : " -> " + p(r)));
            }
            case "dirget" -> line("DIRGET " + p(a1) + " -> " + p(fs.getDirectoryString(a1)));
            case "known" -> line("KNOWN " + p(a1) + " " + fs.isKnownFile(a1));
            case "abs" -> line("ABS " + p(a1) + " -> " + p(fs.getAbsolutePath(a1)));
            case "map" -> {
                Set<String> all = (Set<String>) get(fs, ZomboidFileSystem.class, "allAbsolutePaths");
                line("MAP size " + fs.activeFileMap.size() + " abs " + all.size());
                for (Map.Entry<String, String> e : new TreeMap<>(fs.activeFileMap).entrySet()) {
                    if (e.getValue().startsWith(cache)) line(" " + p(e.getKey()) + " -> " + p(e.getValue()));
                }
            }
            case "state" -> state();
            case "mods" -> {
                Map<String, ChooseGameInfo.Mod> dirToMod = (Map<String, ChooseGameInfo.Mod>) get(fs, ZomboidFileSystem.class, "modDirToMod");
                line("MODS " + dirToMod.size());
                for (ChooseGameInfo.Mod m : new TreeMap<>(dirToMod).values()) dumpMod(m);
            }
            default -> throw new IllegalArgumentException("unknown command " + f[0]);
        }
    }

    public static void main(String[] a) throws Exception {
        PrintStream out = System.out;
        System.setOut(System.err);
        new File(a[0] + "/java").mkdirs();
        ZomboidFileSystem.instance.setCacheDir(a[0] + "/java");
        ZomboidFileSystem.instance.init();
        Languages.instance.init();
        // loadMods(String) reads GameClient.client: GameClient's class init builds ServerOptions, which draws from Rand
        zombie.core.random.RandStandard.INSTANCE.init();
        cache = ZomboidFileSystem.instance.getCacheDir();
        cacheCanon = new File(cache).getCanonicalPath();
        base = ZomboidFileSystem.instance.base.canonicalFile.getPath();
        line("GAME version " + version(Core.getInstance().getGameVersion()) + " min " + version(ChooseGameInfo.getMinRequiredVersion())
            + " compression " + TextureID.useCompressionOption + " modsEnabled " + Core.optionModsEnabled);
        for (String l : Files.readAllLines(Paths.get(a[1]), StandardCharsets.UTF_8)) {
            if (l.isEmpty() || l.startsWith("#")) continue;
            String[] f = l.split("\t", -1);
            try {
                run(f);
            } catch (Exception e) {
                line("EXC " + f[0] + " " + e.getClass().getSimpleName() + ": " + p(e.getMessage()));
            }
        }
        out.print(sb);
        out.flush();
        System.exit(0);
    }
}
