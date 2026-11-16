import java.io.File;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.stream.Stream;
import zombie.GameWindow;
import zombie.ZomboidFileSystem;
import zombie.core.Core;
import zombie.core.Language;
import zombie.core.Languages;
import zombie.core.Translator;
import zombie.core.bucket.BucketManager;
import zombie.core.fonts.AngelCodeFont;
import zombie.core.textures.Texture;
import zombie.ui.FontsFile;
import zombie.ui.FontsFileFont;
import zombie.ui.TextManager;
import zombie.ui.UIFont;

/** Oracle probe (tools/oracle.sh fonts): the ORIGINAL zombie.core.fonts.AngelCodeFont and zombie.ui.TextManager,
 * standalone (no renderer: every texture lookup fails, as it does headless). Printed, in this order:
 *   FONT <path>   every .fnt under media/fonts and in media/ (sorted), loaded as TextManager loads one:
 *                 new AngelCodeFont("media/...fnt", null); then its file, sdf flag, line height, chars length and
 *                 count, the texture names parseFnt asked Texture.getSharedTexture for (TEX, in order), every
 *                 defined char in id order with its sorted kerning arrays (C), and the measuring methods on the
 *                 SAMPLES (M: getWidth, getWidth(xadvance), getHeight, actual height, offset, getYOffset, a
 *                 sub-range width)
 *   FSR           Core.getOptionFontSizeReal for every font size option and a range of screen heights
 *   LANG          the languages (Languages.instance, by name)
 *   FONTSFILE     every fonts*.txt read by FontsFile.read into a TreeMap
 *   PATH          TextManager.getFontFilePath (private, by reflection) for every language, size folder and file
 *                 name the fonts files mention
 *   INIT          TextManager.Init on a fresh TextManager for every language, font size option 1..5 and dyslexic
 *                 flag: the font of every UIFont (shared objects by index), normal[], the public font fields,
 *                 isUsingNonEnglishFonts, getFontHeight/MeasureFont/isSdf, MeasureStringX/Y/YReal/YOffset and
 *                 CentreStringYOffset on samples, WrapText cases
 * The texture names are recorded through the shared Bucket's map (Texture.getSharedTextureInternal asks
 * BucketManager.Shared().HasTexture(name) for every name no earlier table knows; Texture.nullTextures and the
 * shared table are cleared before each font so none is skipped). Paths under the game dir print as "$/...".
 * Strings escape everything outside printable ASCII as \\uXXXX. Compared with `pz-dump fonts`.
 * Args: <scratch cache dir>; run from the game root. */
public class FontsProbe {
    static final StringBuilder sb = new StringBuilder();
    static String base;
    static final ArrayList<String> texRequests = new ArrayList<>();

    static final String LONG = "This is a long sentence that should wrap across several lines when the width is small"
        + " enough, with averyveryverylongwordthatcannotfitonasingleline inside.";
    static final String[] SAMPLES = {
        "",
        "Hello, World!",
        "The quick brown fox jumps over the lazy dog. 0123456789",
        "Line one\nLine two\n\nfourth line",
        "\n",
        "AV AW Ta To Wa Yo ff fi LT",
        "  two  spaces  ",
        "Tab\there\u0000nul",
        "a\r\nb\rc",
        "Über naïve café àéîõü ß ©®€™",
        "Привет, мир! Ёё",
        "你好世界，测试。",
        "こんにちは、カタカナ漢字",
        "안녕하세요 한국어",
        "สวัสดีครับ ก่ำ",
        "\u0001\u007f�￿😀",
        "<RGB:1,0,0> Red <LINE> <SIZE:large> [img=music] <IMAGE:media/ui/x.png>",
        "Zażółć gęślą jaźń",
        LONG,
    };
    /** The samples MeasureString* runs on for every font of every TextManager.Init. */
    static final int[] INIT_SAMPLES = {0, 1, 3, 10, 11, 15};
    static final Object[][] WRAPS = {
        {LONG, 200, -1, ""},
        {LONG, 50, -1, ""},
        {LONG, 50, 2, "..."},
        {LONG, 120, 3, "…"},
        {"first line\r\nsecond line is longer than the first\nthird", 80, -1, ""},
        {"", 100, -1, ""},
        {"   ", 0, -1, ""},
        {"short", 1000, 1, "..."},
        {"你好世界你好世界你好世界你好世界", 60, -1, ""},
        {"word  double  spaces here", 40, -1, ""},
        {"\n\nlead", 100, -1, ""},
        {"trail\n\n", 100, -1, ""},
        {" x", -1, -1, ""},
        {LONG, 90, 2, "a much longer suffix than the room"},
        {"Supercalifragilisticexpialidociousandsomemore end", 60, -1, ""},
        {"Antidisestablishmentarianism\r\n\r\nx", 25, 4, "~"},
    };
    static final UIFont[] WRAP_FONTS = {UIFont.Small, UIFont.Medium, UIFont.Large, UIFont.Code, UIFont.Dialogue};
    static final int[] HEIGHTS = {0, 720, 1079, 1080, 1211, 1212, 1343, 1344, 1475, 1476, 1607, 1608, 1739, 1740,
        1871, 1872, 2003, 2004, 2135, 2136, 2267, 2268, 2399, 2400, 2531, 2532, 3000, 4320, -5000};

    static void esc(StringBuilder b, String s) {
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c >= 0x20 && c < 0x7f && c != '\\') b.append(c);
            else b.append(String.format("\\u%04x", (int) c));
        }
    }

    static String esc(String s) {
        if (s == null) return "null";
        StringBuilder b = new StringBuilder();
        esc(b, s);
        return b.toString();
    }

    static String rel(String abs) {
        if (abs == null) return "null";
        return abs.startsWith(base) ? "$/" + abs.substring(base.length()) : abs;
    }

    static Object field(Object owner, String name) throws Exception {
        Field f = owner.getClass().getDeclaredField(name);
        f.setAccessible(true);
        return f.get(owner);
    }

    static String exc(Throwable t) {
        while (t instanceof java.lang.reflect.InvocationTargetException && t.getCause() != null) t = t.getCause();
        return "E:" + t.getClass().getSimpleName();
    }

    /** Records every name Texture.getSharedTextureInternal looks up in the shared bucket. */
    static final class RecordingMap extends HashMap<Path, Texture> {
        @Override
        public boolean containsKey(Object key) {
            texRequests.add(String.valueOf(key));
            return super.containsKey(key);
        }
    }

    @SuppressWarnings("unchecked")
    static void resetTextureTables() throws Exception {
        Texture.nullTextures.clear();
        Field f = Texture.class.getDeclaredField("s_sharedTextureTable");
        f.setAccessible(true);
        ((Map<String, Texture>) f.get(null)).clear();
        texRequests.clear();
    }

    static void font(String path) throws Exception {
        sb.append("FONT ").append(path).append('\n');
        resetTextureTables();
        AngelCodeFont f;
        try {
            f = new AngelCodeFont(path, (String) null);
        } catch (Throwable t) {
            sb.append("FAIL ").append(exc(t)).append('\n');
            return;
        }
        int defined = 0;
        for (AngelCodeFont.CharDef d : f.chars) if (d != null) defined++;
        sb.append("FILE ").append(rel(((File) field(f, "fntFile")).getPath())).append(" sdf=").append(f.isSdf() ? 1 : 0)
            .append(" lh=").append(f.getLineHeight()).append(" chars=").append(f.chars.length).append(" defined=")
            .append(defined).append(" pages=").append(((Map<?, ?>) field(f, "pages")).size()).append('\n');
        for (String t : texRequests) sb.append("TEX ").append(esc(rel(t))).append('\n');
        for (AngelCodeFont.CharDef d : f.chars) {
            if (d == null) continue;
            sb.append("C ").append(d.id).append(' ').append(d.x).append(' ').append(d.y).append(' ').append(d.width)
                .append(' ').append(d.height).append(' ').append(d.xoffset).append(' ').append(d.yoffset).append(' ')
                .append(d.xadvance).append(' ').append(d.page);
            if (d.kerningSecond != null) {
                sb.append(" K");
                for (int i = 0; i < d.kerningSecond.length; i++)
                    sb.append(' ').append(d.kerningSecond[i]).append(':').append(d.kerningAmount[i]);
            }
            sb.append('\n');
        }
        for (int s = 0; s < SAMPLES.length; s++) {
            String t = SAMPLES[s];
            sb.append("M ").append(s).append(' ').append(f.getWidth(t)).append(' ').append(f.getWidth(t, true))
                .append(' ').append(f.getHeight(t)).append(' ').append(f.getHeight(t, true, false)).append(' ')
                .append(f.getHeight(t, false, true)).append(' ');
            try {
                sb.append(f.getYOffset(t));
            } catch (Throwable e) {
                sb.append(exc(e));
            }
            if (t.length() >= 3) sb.append(' ').append(f.getWidth(t, 1, t.length() - 2)).append(' ')
                .append(f.getWidth(t, 1, t.length() - 2, true));
            sb.append('\n');
        }
    }

    static String fsName(UIFont f) {
        return f == null ? "null" : f.name();
    }

    public static void main(String[] a) throws Exception {
        PrintStream out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, "UTF-8");
        System.setOut(System.err);
        ZomboidFileSystem.instance.setCacheDir(a[0]);
        ZomboidFileSystem.instance.init();
        base = ZomboidFileSystem.instance.base.canonicalFile.getPath() + "/";
        // Texture.getSharedTextureInternal asks the shared bucket last; its map records what was asked for.
        Field bt = zombie.core.bucket.Bucket.class.getDeclaredField("textures");
        bt.setAccessible(true);
        bt.set(BucketManager.Shared(), new RecordingMap());

        // Part A: every font file.
        ArrayList<String> files = new ArrayList<>();
        try (Stream<Path> w = Files.walk(Paths.get("media/fonts"))) {
            w.filter(p -> p.toString().endsWith(".fnt") && Files.isRegularFile(p)).forEach(p -> files.add(p.toString()));
        }
        try (Stream<Path> w = Files.list(Paths.get("media"))) {
            w.filter(p -> p.toString().endsWith(".fnt") && Files.isRegularFile(p)).forEach(p -> files.add(p.toString()));
        }
        files.sort(null);
        sb.append("FONTS ").append(files.size()).append('\n');
        for (String p : files) font(p);

        // Part B: the TextManager tables.
        Core core = Core.getInstance();
        for (int o = 1; o <= 6; o++) {
            core.setOptionFontSize(o);
            for (int h : HEIGHTS) {
                Core.height = h;
                sb.append("FSR ").append(o).append(' ').append(h).append(' ').append(core.getOptionFontSizeReal()).append('\n');
            }
        }
        Core.height = 720;

        Languages.instance.init();
        TreeSet<String> langs = new TreeSet<>();
        for (Language l : Languages.instance.getLanguages()) langs.add(l.name());
        sb.append("LANG ").append(langs.size());
        for (String l : langs) sb.append(' ').append(l);
        sb.append('\n');

        TreeSet<String> names = new TreeSet<>();
        ArrayList<String> fontsFiles = new ArrayList<>();
        try (Stream<Path> w = Files.walk(Paths.get("media/fonts"))) {
            w.filter(p -> p.getFileName().toString().matches("fonts.*\\.txt")).forEach(p -> fontsFiles.add(p.toString()));
        }
        fontsFiles.sort(null);
        for (String ff : fontsFiles) {
            HashMap<String, FontsFileFont> m = new HashMap<>();
            boolean ok = new FontsFile().read(ZomboidFileSystem.instance.getString(ff), m);
            sb.append("FONTSFILE ").append(ff).append(' ').append(ok).append(' ').append(m.size()).append('\n');
            for (Map.Entry<String, FontsFileFont> e : new TreeMap<>(m).entrySet()) {
                FontsFileFont v = e.getValue();
                sb.append("FF ").append(esc(e.getKey())).append(" id=").append(esc(v.id)).append(" fnt=").append(esc(v.fnt))
                    .append(" img=").append(esc(v.img)).append(" type=").append(esc(v.type)).append('\n');
                names.add(v.fnt);
                if (v.img != null) names.add(v.img);
            }
        }
        Method gffp = TextManager.class.getDeclaredMethod("getFontFilePath", String.class, String.class, String.class);
        gffp.setAccessible(true);
        String[] sizeDirs = {null, "1x", "2x", "3x", "4x"};
        TextManager probe = new TextManager();
        for (String l : langs)
            for (String sd : sizeDirs)
                for (String n : names)
                    sb.append("PATH ").append(l).append(' ').append(sd).append(' ').append(n).append(' ')
                        .append(gffp.invoke(probe, l, sd, n)).append('\n');

        GameWindow.gameThreadExited = true; // TextManager.Init ends by queueing an SDF shader for the render thread
        UIFont[] fonts = UIFont.values();
        for (String l : langs) {
            Translator.setLanguage(Languages.instance.getByName(l));
            for (int o = 1; o <= 5; o++) {
                for (int dys = 0; dys <= 1; dys++) {
                    core.setOptionFontSize(o);
                    core.setOptionEnableDyslexicFont(dys == 1);
                    TextManager tm = new TextManager();
                    sb.append("INIT ").append(l).append(" size=").append(o).append(" dyslexic=").append(dys);
                    try {
                        tm.Init();
                        sb.append(" ok\n");
                    } catch (Throwable t) {
                        sb.append(' ').append(exc(t)).append('\n');
                    }
                    List<AngelCodeFont> distinct = new ArrayList<>();
                    java.util.function.Function<AngelCodeFont, String> id = f -> {
                        if (f == null) return "null";
                        for (int i = 0; i < distinct.size(); i++) if (distinct.get(i) == f) return "#" + i;
                        distinct.add(f);
                        try {
                            return "#" + (distinct.size() - 1) + " " + rel(((File) field(f, "fntFile")).getPath()) + " sdf="
                                + (f.isSdf() ? 1 : 0) + " lh=" + f.getLineHeight() + " chars=" + f.chars.length;
                        } catch (Exception e) {
                            throw new RuntimeException(e);
                        }
                    };
                    for (UIFont u : fonts) sb.append("U ").append(u.name()).append(' ').append(id.apply(tm.enumToFont[u.ordinal()])).append('\n');
                    for (int n = 0; n < tm.normal.length; n++) sb.append("N ").append(n + 11).append(' ').append(id.apply(tm.getNormalFromFontSize(n + 11))).append('\n');
                    AngelCodeFont[] fields = {tm.font, tm.font2, tm.font3, tm.font4, tm.main1, tm.main2, tm.zombiefontcredits1,
                        tm.zombiefontcredits2, tm.zombienew1, tm.zombienew2, tm.zombienew3, tm.zomboidDialogue, tm.codetext,
                        tm.codeSmall, tm.codeMedium, tm.codeLarge, tm.debugConsole, tm.intro, tm.handwritten};
                    sb.append("FIELDS");
                    for (AngelCodeFont f : fields) sb.append(' ').append(id.apply(f));
                    sb.append('\n');
                    sb.append("NONEN ").append(tm.isUsingNonEnglishFonts()).append(" code=").append(tm.getCurrentCodeFont()).append('\n');
                    UIFont[] withNull = new UIFont[fonts.length + 1];
                    System.arraycopy(fonts, 0, withNull, 0, fonts.length);
                    for (UIFont u : withNull) {
                        sb.append("H ").append(fsName(u));
                        try {
                            sb.append(' ').append(tm.getFontHeight(u));
                        } catch (Throwable t) {
                            sb.append(' ').append(exc(t));
                        }
                        try {
                            sb.append(' ').append(tm.MeasureFont(u));
                        } catch (Throwable t) {
                            sb.append(' ').append(exc(t));
                        }
                        try {
                            sb.append(' ').append(tm.isSdf(u));
                        } catch (Throwable t) {
                            sb.append(' ').append(exc(t));
                        }
                        sb.append('\n');
                        for (int s : INIT_SAMPLES) {
                            String t = SAMPLES[s];
                            sb.append("MS ").append(fsName(u)).append(' ').append(s).append(' ').append(tm.MeasureStringX(u, t))
                                .append(' ').append(tm.MeasureStringY(u, t)).append(' ').append(tm.MeasureStringYReal(u, t))
                                .append(' ').append(tm.MeasureStringYOffset(u, t)).append(' ').append(tm.CentreStringYOffset(u, t))
                                .append('\n');
                        }
                    }
                    for (UIFont u : WRAP_FONTS) {
                        for (int w = 0; w < WRAPS.length; w++) {
                            Object[] c = WRAPS[w];
                            sb.append("WRAP ").append(u.name()).append(' ').append(w).append(' ');
                            try {
                                sb.append(esc(tm.WrapText(u, (String) c[0], (Integer) c[1], (Integer) c[2], (String) c[3])));
                            } catch (Throwable t) {
                                sb.append(exc(t));
                            }
                            sb.append('\n');
                        }
                        sb.append("WRAP2 ").append(u.name()).append(' ').append(esc(tm.WrapText(u, LONG, 150))).append('\n');
                    }
                }
            }
        }
        ArrayList<UIFont> all = new TextManager().getAllFonts(new ArrayList<>());
        sb.append("ALLFONTS");
        for (UIFont u : all) sb.append(' ').append(u.name());
        sb.append('\n');
        for (String s : new String[]{"Small", "small", "SdfCaveat", "", "Nope", "DebugConsole"})
            sb.append("FROMSTRING ").append(esc(s)).append(' ').append(fsName(UIFont.FromString(s))).append('\n');
        out.print(sb);
        out.flush();
        Runtime.getRuntime().halt(0);
    }
}
