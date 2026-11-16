import java.io.PrintStream;
import java.util.ArrayList;
import java.util.List;
import zombie.GameWindow;
import zombie.Lua.LuaManager;
import zombie.ZomboidFileSystem;
import zombie.asset.AssetManagers;
import zombie.core.Core;
import zombie.ui.UIElement;
import zombie.ui.UIElementInterface;

/** Oracle probe (tools/oracle.sh luagame): what the ORIGINAL client runs on its way to the main menu, in its order,
 * with every step a run without a window or sound can do, then luaboot's LoadDirBase, ZomboidGlobals and boot events
 * and report. The client is the one `-nosound` starts (Core.soundDisabled: the Dummy sound managers, no FMOD); a step
 * that needs the window, OpenGL, Steam, the network or a native library the scripts do not see is left out (the list
 * is in docs/COMPATIBILITY.md). Steps, in the order of the original's code:
 *   MainScreenState.main      RandStandard/RandLua.init, Bullet.init (ScriptManager.Load's vehicles call it),
 *                             the `-nosound` argument, InitDisplay's language option (no options.ini on a fresh
 *                             cache: from user.language), the asset managers
 *   GameWindow.mainThreadInit DebugOptions.init, GameProfiler.init, the Dummy sound managers and bank,
 *                             VoiceManager.loadConfig, ZomboidFileSystem.init
 *   GameWindow.init           initFonts (TextManager.Init), the mods (resetModFolders, resetDefaultModsForNewRelease,
 *                             loadMods("default"), loadModPackFiles), Languages.init, Translator.language = null,
 *                             initFonts, Translator.loadFiles, LuaManager.init
 *   GameWindow.initShared     the UI texture packs and their lookup, IsoCursor, IsoReticle, PerkFactory.init,
 *                             CustomPerks, CustomSandboxOptions, ModRegistries, ScriptManager.Load, SpriteModelManager,
 *                             ClothingDecals, BeardStyles, HairStyles, OutfitManager, VoiceStyles, RandStandard/RandLua,
 *                             Mouse.initCustomCursor, TileGeometryManager, TileDepthTextureAssignmentManager,
 *                             SeamManager, SeatingManager, GameInput.initControllers
 *   GameWindow.init           LoadDirBase (shared, client), then LuaBootProbe's events (ZomboidGlobals.Load,
 *                             OnLoadSoundBanks, OnGameBoot, OnGameStateEnter + TermsOfServiceState.enter's rest,
 *                             OnMainMenuEnter)
 * A step that throws stops the probe (exit 1): every step listed runs headless. Output: LuaLoadEnvProbe's report,
 * LuaBootProbe's lines, then the UI the scripts built:
 *   UI <list> <i> <element>    each element of UIManager's pending lists (toAdd, toRemove: UIManager.update moves
 *                              them into UI, and no frame runs here), then its controls depth first as
 *                              UICHILD <depth> <element>; an element is its fields x y width height (raw bits)
 *                              visible anchors(T L R B) alwaysOnTop capture scrollChildren scrollWithParent yScroll
 *                              scrollHeight consumeMouseEvents wantKeyEvents wantExtraMouseEvents forceCursorVisible
 *                              and its table (as the report prints values; a table not printed yet is dumped as
 *                              UITABLE ...)
 * Args: <scratch cache dir> <dir>... ; run from the game root. */
public class LuaGameProbe {
    interface Step { void run() throws Throwable; }

    static void step(String name, Step s) throws Exception {
        try {
            s.run();
            System.err.println("LUAGAME step " + name);
        } catch (Throwable e) {
            System.err.println("LUAGAME step " + name + " FAILED");
            e.printStackTrace();
            System.exit(1);
        }
    }

    static void init(String[] a) throws Exception {
        // MainScreenState.main
        ZomboidFileSystem.instance.setCacheDir(a[0]);
        step("RandStandard.init", () -> zombie.core.random.RandStandard.INSTANCE.init());
        step("RandLua.init", () -> zombie.core.random.RandLua.INSTANCE.init());
        step("Bullet.init", () -> zombie.core.physics.Bullet.init());
        Core.soundDisabled = true;
        // InitDisplay with no options.ini (a fresh cache): loadOptions' initOptionsINI asks GLFW for the display modes
        // (no window here), then the language option comes from user.language
        step("InitDisplay language", () -> Core.getInstance().setOptionLanguageName(System.getProperty("user.language").toUpperCase()));
        step("asset managers", () -> {
            AssetManagers am = GameWindow.assetManagers;
            zombie.core.skinnedmodel.model.AiSceneAssetManager.instance.create(zombie.core.skinnedmodel.model.AiSceneAsset.ASSET_TYPE, am);
            zombie.core.textures.AnimatedTextureIDAssetManager.instance.create(zombie.core.textures.AnimatedTextureID.ASSET_TYPE, am);
            zombie.core.skinnedmodel.model.AnimationAssetManager.instance.create(zombie.core.skinnedmodel.model.AnimationAsset.ASSET_TYPE, am);
            zombie.core.skinnedmodel.advancedanimation.AnimNodeAssetManager.instance.create(zombie.core.skinnedmodel.advancedanimation.AnimNodeAsset.ASSET_TYPE, am);
            zombie.core.skinnedmodel.population.ClothingItemAssetManager.instance.create(zombie.core.skinnedmodel.population.ClothingItem.ASSET_TYPE, am);
            zombie.core.skinnedmodel.model.MeshAssetManager.instance.create(zombie.core.skinnedmodel.model.ModelMesh.ASSET_TYPE, am);
            zombie.core.skinnedmodel.model.ModelAssetManager.instance.create(zombie.core.skinnedmodel.model.Model.ASSET_TYPE, am);
            zombie.core.textures.NinePatchTextureAssetManager.instance.create(zombie.core.textures.NinePatchTexture.ASSET_TYPE, am);
            zombie.core.physics.PhysicsShapeAssetManager.instance.create(zombie.core.physics.PhysicsShape.ASSET_TYPE, am);
            zombie.core.textures.TextureIDAssetManager.instance.create(zombie.core.textures.TextureID.ASSET_TYPE, am);
            zombie.core.textures.TextureAssetManager.instance.create(zombie.core.textures.Texture.ASSET_TYPE, am);
            zombie.worldMap.WorldMapDataAssetManager.instance.create(zombie.worldMap.WorldMapData.ASSET_TYPE, am);
        });
        // GameWindow.mainThreadInit (FMODManager.init is skipped by -nosound)
        step("DebugOptions.init", () -> zombie.debug.DebugOptions.instance.init());
        step("GameProfiler.init", () -> zombie.GameProfiler.init());
        step("Dummy sound", () -> {
            zombie.SoundManager.instance = new zombie.DummySoundManager();
            zombie.AmbientStreamManager.instance = new zombie.DummyAmbientStreamManager();
            zombie.audio.BaseSoundBank.instance = new zombie.audio.DummySoundBank();
        });
        step("VoiceManager.loadConfig", () -> zombie.core.raknet.VoiceManager.instance.loadConfig());
        step("ZomboidFileSystem.init", () -> ZomboidFileSystem.instance.init());
        // GameWindow.init
        step("initFonts", () -> zombie.ui.TextManager.instance.Init());
        step("resetModFolders", () -> ZomboidFileSystem.instance.resetModFolders());
        step("resetDefaultModsForNewRelease", () -> ZomboidFileSystem.instance.resetDefaultModsForNewRelease("42_00"));
        step("loadMods", () -> ZomboidFileSystem.instance.loadMods("default"));
        step("loadModPackFiles", () -> ZomboidFileSystem.instance.loadModPackFiles());
        step("Languages.init", () -> zombie.core.Languages.instance.init());
        zombie.core.Translator.language = null;
        step("initFonts", () -> zombie.ui.TextManager.instance.Init());
        step("Translator.loadFiles", () -> zombie.core.Translator.loadFiles());
        step("LuaManager.init", () -> LuaLoadEnvProbe.luaInit());
        // GameWindow.initShared
        zombie.core.textures.TexturePackPage.ignoreWorldItemTextures = true;
        for (String p : new String[]{"UI", "UI2", "IconsMoveables", "RadioIcons", "ApComUI", "Mechanics", "WeatherFx"})
            step("LoadTexturePack " + p, () -> GameWindow.LoadTexturePack(p, 2));
        step("setTexturePackLookup", () -> GameWindow.setTexturePackLookup());
        step("IsoCursor.getInstance", () -> zombie.iso.sprite.IsoCursor.getInstance());
        step("IsoReticle.initInstances", () -> zombie.iso.sprite.IsoReticle.initInstances());
        step("MainScreenState.preloadBackgroundTextures", () -> zombie.gameStates.MainScreenState.preloadBackgroundTextures());
        step("PerkFactory.init", () -> zombie.characters.skills.PerkFactory.init());
        step("CustomPerks.init", () -> zombie.characters.skills.CustomPerks.instance.init());
        step("CustomPerks.initLua", () -> zombie.characters.skills.CustomPerks.instance.initLua());
        step("CustomSandboxOptions.init", () -> zombie.sandbox.CustomSandboxOptions.instance.init());
        step("CustomSandboxOptions.initInstance", () -> zombie.sandbox.CustomSandboxOptions.instance.initInstance(zombie.SandboxOptions.instance));
        step("ModRegistries.init", () -> zombie.scripting.objects.ModRegistries.init());
        step("ScriptManager.Load", () -> zombie.scripting.ScriptManager.instance.Load());
        step("SpriteModelManager.init", () -> zombie.spriteModel.SpriteModelManager.getInstance().init());
        step("ClothingDecals.init", () -> zombie.core.skinnedmodel.population.ClothingDecals.init());
        step("BeardStyles.init", () -> zombie.core.skinnedmodel.population.BeardStyles.init());
        step("HairStyles.init", () -> zombie.core.skinnedmodel.population.HairStyles.init());
        step("OutfitManager.init", () -> zombie.core.skinnedmodel.population.OutfitManager.init());
        step("VoiceStyles.init", () -> zombie.core.skinnedmodel.population.VoiceStyles.init());
        step("RandStandard.init", () -> zombie.core.random.RandStandard.INSTANCE.init());
        step("RandLua.init", () -> zombie.core.random.RandLua.INSTANCE.init());
        zombie.core.textures.TexturePackPage.ignoreWorldItemTextures = false;
        step("Mouse.initCustomCursor", () -> zombie.input.Mouse.initCustomCursor());
        step("TileGeometryManager.init", () -> zombie.tileDepth.TileGeometryManager.getInstance().init());
        step("TileDepthTextureAssignmentManager.init", () -> zombie.tileDepth.TileDepthTextureAssignmentManager.getInstance().init());
        step("SeamManager.init", () -> zombie.seams.SeamManager.getInstance().init());
        step("SeatingManager.init", () -> zombie.seating.SeatingManager.getInstance().init());
        step("GameInput.initControllers", () -> GameWindow.GameInput.initControllers());
        // GameWindow.init: LuaManager.LoadDirBase() is initChecksum (nothing for a client not connected) and these
        LuaManager.initChecksum();
        LuaLoadEnvProbe.loadDirs(a);
    }

    static String bits(float f) { return Integer.toHexString(Float.floatToRawIntBits(f)); }
    static String bits(double d) { return Long.toHexString(Double.doubleToRawLongBits(d)); }
    static String flag(boolean b) { return b ? "1" : "0"; }

    static void element(StringBuilder sb, String prefix, String path, UIElementInterface ei) throws Exception {
        sb.append(prefix);
        if (!(ei instanceof UIElement e)) {
            sb.append(' ').append(ei.getClass().getName()).append('\n');
            return;
        }
        sb.append(' ').append(bits(e.x)).append(' ').append(bits(e.y)).append(' ').append(bits(e.width)).append(' ').append(bits(e.height))
            .append(' ').append(flag(e.visible)).append(' ').append(flag(e.anchorTop)).append(flag(e.anchorLeft)).append(flag(e.anchorRight)).append(flag(e.anchorBottom))
            .append(' ').append(flag(e.alwaysOnTop)).append(' ').append(flag(e.capture)).append(' ').append(flag(e.scrollChildren)).append(' ').append(flag(e.scrollWithParent))
            .append(' ').append(bits((Double) LuaLoadEnvProbe.field(e, UIElement.class, "yScroll"))).append(' ').append(LuaLoadEnvProbe.field(e, UIElement.class, "scrollHeight"))
            .append(' ').append(flag((Boolean) LuaLoadEnvProbe.field(e, UIElement.class, "consumeMouseEvents")))
            .append(flag((Boolean) LuaLoadEnvProbe.field(e, UIElement.class, "wantKeyEvents")))
            .append(flag((Boolean) LuaLoadEnvProbe.field(e, UIElement.class, "wantExtraMouseEvents")))
            .append(flag((Boolean) LuaLoadEnvProbe.field(e, UIElement.class, "forceCursorVisible")))
            .append(' ').append(LuaLoadEnvProbe.value(e.table)).append('\n');
        // a table seen for the first time: its contents
        LuaLoadEnvProbe.dump("UITABLE " + path, e.table);
    }

    static void tree(StringBuilder sb, String path, UIElement e, int depth) throws Exception {
        for (int i = 0; i < e.controls.size(); i++) {
            String p = path + "." + i;
            element(sb, "UICHILD " + depth + " " + p, p, e.controls.get(i));
            tree(sb, p, e.controls.get(i), depth + 1);
        }
    }

    @SuppressWarnings("unchecked")
    static void ui(String list) throws Exception {
        StringBuilder sb = LuaLoadEnvProbe.sb;
        List<UIElementInterface> l = (List<UIElementInterface>) LuaLoadEnvProbe.field(null, zombie.ui.UIManager.class, list);
        for (int i = 0; i < l.size(); i++) {
            String p = list + "[" + i + "]";
            element(sb, "UI " + list + " " + i, p, l.get(i));
            if (l.get(i) instanceof UIElement e) tree(sb, p, e, 1);
        }
    }

    public static void main(String[] a) throws Exception {
        PrintStream out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, "UTF-8");
        System.setOut(System.err);
        init(a);
        zombie.gameStates.TermsOfServiceState tos = LuaBootProbe.events();
        LuaLoadEnvProbe.report();
        LuaBootProbe.report(tos);
        ui("toAdd");
        ui("toRemove");
        ui("UI");
        out.print(LuaLoadEnvProbe.sb);
        out.flush();
        Runtime.getRuntime().halt(0);
    }
}
