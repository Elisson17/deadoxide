//! zombie.GameWindow: the client's startup (MainScreenState.main, then mainThreadInit, init and initShared) as far
//! as the Lua state sees it, run without a window or sound: the client `-nosound` starts on a fresh cache folder.
//! Each step below is one the original runs headless (LuaGameProbe runs the original's); a step the scripts do not
//! see (the asset managers, DebugOptions, GameProfiler, VoiceManager.loadConfig, the texture packs' pixels, the
//! cursor, tile geometry, seams, seating, controllers) has nothing to port here. The list, with what cannot run
//! without a window, is in docs/COMPATIBILITY.md.
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use super::core::core::get_option_font_size_real;
use super::core::random::rand_abstract::RandAbstract;
use super::core::textures::texture::Textures;
use super::core::translator::Translator;
use super::lua::exposed_methods::CORE_HEIGHT;
use super::scripting::script_manager::{BucketIndex, ScriptManager};
use super::ui::text_manager::TextManager;
use super::zomboid_file_system::ZomboidFileSystem;
use crate::org::uncommons::maths::random::cellular_automaton_rng::CellularAutomatonRng;

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
    /// The item collection's lookup data (`scripts.item_index()`).
    pub item_index: BucketIndex,
    /// The index in `scripts.items` of each item's full type.
    pub item_by_full_type: HashMap<String, usize>,
    /// Texture's and GameWindow's texture statics (the packs' index, the shared table).
    pub textures: RefCell<Textures>,
    /// Rand (RandStandard.INSTANCE) and RandLua.INSTANCE after initShared's `init()`, with the fixed seeds.
    pub rand_standard: RefCell<RandAbstract<CellularAutomatonRng>>,
    pub rand_lua: RefCell<RandAbstract<CellularAutomatonRng>>,
}

/// The seeds of RandStandard and RandLua: the original's come from SecureRandom (PZSeedGenerator); LuaGameProbe
/// gives both generators these fixed seeds after initShared's `init()`, and so does the port.
pub const RAND_STANDARD_SEED: [u8; 4] = [0, 0, 0, 1];
pub const RAND_LUA_SEED: [u8; 4] = [0, 0, 0, 2];

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
    /// initShared: the UI texture packs, `ScriptManager.instance.Load()`, RandStandard/RandLua `init()` (seeded, see
    /// RAND_STANDARD_SEED).
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
        // initShared
        let mut textures = Textures::new(root);
        for p in [
            "UI",
            "UI2",
            "IconsMoveables",
            "RadioIcons",
            "ApComUI",
            "Mechanics",
            "WeatherFx",
        ] {
            textures.load_texture_pack(&fs, p);
        }
        textures.set_texture_pack_lookup();
        let scripts = ScriptManager::load(root)?;
        let item_index = scripts.item_index();
        let item_by_full_type = (scripts.items.iter().enumerate())
            .map(|(i, item)| (item.full_name(), i))
            .collect();
        Ok(ClientStartup {
            fs,
            translator: RefCell::new(translator),
            text_manager: RefCell::new(Some(tm)),
            scripts,
            item_index,
            item_by_full_type,
            textures: RefCell::new(textures),
            rand_standard: RefCell::new(RandAbstract::new(CellularAutomatonRng::new(
                RAND_STANDARD_SEED,
            ))),
            rand_lua: RefCell::new(RandAbstract::new(CellularAutomatonRng::new(RAND_LUA_SEED))),
        })
    }

    /// zombie.GameWindow.enter before its OnGameBoot, the part the Lua state sees: the tile texture packs (Core's
    /// texture2x option is on by default: the 2x ones) and their lookup. The original then checks that
    /// `TileIndieStoneTentFrontLeft` exists (a RuntimeException otherwise; `Err` here).
    pub fn enter(&self) -> Result<(), String> {
        let mut t = self.textures.borrow_mut();
        for p in [
            "Tiles2x",
            "Overlays2x",
            "JumboTrees2x",
            "JumboTreesBigs2x",
            "Tiles2x.floor",
            "B42ChunkCaching2x",
            "B42ChunkCaching2x.floor",
            "Clock2x",
        ] {
            t.load_texture_pack(&self.fs, p);
        }
        t.set_texture_pack_lookup();
        for i in 0..4 {
            t.get_shared_texture(&self.fs, Some(&format!("animated_clock_01_{i}")));
        }
        if t.get_shared_texture(&self.fs, Some("TileIndieStoneTentFrontLeft"))
            .is_none()
        {
            return Err("Rebuild Tiles.pack with \"1 Include This in .pack\" as individual images not tilesheets".into());
        }
        Ok(())
    }
}

impl ClientStartup {
    /// ScriptManager.FindItem(name): the index of the item in `scripts.items`.
    pub fn find_item(&self, name: &str) -> Option<usize> {
        let full = self.scripts.find_item(&self.item_index, name)?;
        self.item_by_full_type.get(&full).copied()
    }
}
