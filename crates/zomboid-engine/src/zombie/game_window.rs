//! zombie.GameWindow: the client's startup (MainScreenState.main, then mainThreadInit, init and initShared) as far
//! as the Lua state sees it, run without a window or sound: the client `-nosound` starts on a fresh cache folder.
//! Each step below is one the original runs headless (LuaGameProbe runs the original's); a step the scripts do not
//! see (the asset managers, DebugOptions, GameProfiler, VoiceManager.loadConfig, the texture packs' pixels, the
//! cursor, tile geometry, seams, seating, controllers) has nothing to port here. The list, with what cannot run
//! without a window, is in docs/COMPATIBILITY.md.
use std::cell::RefCell;
use std::path::Path;

use super::core::core::get_option_font_size_real;
use super::core::translator::Translator;
use super::lua::exposed_methods::CORE_HEIGHT;
use super::scripting::script_manager::ScriptManager;
use super::ui::text_manager::TextManager;
use super::zomboid_file_system::ZomboidFileSystem;

/// What the startup steps leave that the Lua state reads.
pub struct ClientStartup {
    /// ZomboidFileSystem.instance after `init()`.
    pub fs: ZomboidFileSystem,
    /// Translator's statics after `loadFiles()`.
    pub translator: RefCell<Translator>,
    /// TextManager.instance after `Init()` (taken by the Lua state's TextManager object).
    pub text_manager: RefCell<Option<TextManager>>,
    /// ScriptManager.instance after `Load()` (the ported part).
    pub scripts: ScriptManager,
}

/// Core's options the startup reads, at their defaults (no options.ini on a fresh cache folder).
const OPTION_FONT_SIZE: i32 = 6;
const OPTION_ENABLE_DYSLEXIC_FONT: bool = false;

/// GameWindow.initFonts: `TextManager.instance.Init()` in the language Translator.getLanguage() picks (the original
/// then waits for the fonts' textures, which load on the render thread; not here).
fn init_fonts(
    fs: &ZomboidFileSystem,
    tm: &mut TextManager,
    translator: &mut Translator,
) -> Result<(), String> {
    let lang = translator.get_language().name.clone();
    tm.init(
        fs,
        &lang,
        OPTION_ENABLE_DYSLEXIC_FONT,
        get_option_font_size_real(OPTION_FONT_SIZE, CORE_HEIGHT),
    )
}

impl ClientStartup {
    /// The startup in the original's order, the steps the Lua state sees:
    /// InitDisplay with no options.ini: `Core.setOptionLanguageName(user.language.toUpperCase())`;
    /// mainThreadInit: `ZomboidFileSystem.instance.init()`;
    /// init: `initFonts()`, the mods (none: `getModIDs` stays empty), `Languages.instance.init()`,
    /// `Translator.language = null`, `initFonts()`, `Translator.loadFiles()`;
    /// initShared: `ScriptManager.instance.Load()`.
    /// LuaManager.init runs between init and initShared; nothing of initShared reaches the Lua state before
    /// LoadDirBase, so the caller makes the Lua state after this.
    pub fn init(root: &Path) -> Result<ClientStartup, String> {
        let mut translator = Translator::new(root);
        // InitDisplay: no options.ini, so the language option is the JVM's user.language upper-cased
        translator.option_language_name = crate::java::to_upper_case(&translator.user_language);
        let fs = ZomboidFileSystem::init(root)?;
        let mut tm = TextManager::new();
        init_fonts(&fs, &mut tm, &mut translator)?;
        translator.languages.init(root);
        translator.language = None;
        init_fonts(&fs, &mut tm, &mut translator)?;
        translator.load_files().map_err(|e| e.to_string())?;
        let scripts = ScriptManager::load(root)?;
        Ok(ClientStartup {
            fs,
            translator: RefCell::new(translator),
            text_manager: RefCell::new(Some(tm)),
            scripts,
        })
    }
}
