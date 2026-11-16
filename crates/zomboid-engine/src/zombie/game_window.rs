//! zombie.GameWindow: the client's startup (MainScreenState.main, then mainThreadInit, init and initShared) as far
//! as the Lua state sees it, run without a window or sound: the client `-nosound` starts on a fresh cache folder.
//! Each step below is one the original runs headless (LuaGameProbe runs the original's); a step the scripts do not
//! see (the asset managers, DebugOptions, GameProfiler, VoiceManager.loadConfig, the texture packs' pixels, the
//! cursor, tile geometry, seams, seating, controllers) has nothing to port here. The list, with what cannot run
//! without a window, is in docs/COMPATIBILITY.md.
use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::characters::professions::character_profession_definition::CharacterProfessionDefinitions;
use super::characters::skills::perk_factory::PerkFactory;
use super::characters::traits::character_trait_definition::CharacterTraitDefinitions;
use super::core::core::get_option_font_size_real;
use super::core::random::rand_abstract::RandAbstract;
use super::core::textures::texture::Textures;
use super::core::translator::Translator;
use super::lua::exposed_methods::CORE_HEIGHT;
use super::scripting::objects::resource_location::ResourceLocation;
use super::scripting::script_manager::{BucketIndex, ScriptManager};
use super::ui::text_manager::TextManager;
use super::zomboid_file_system::ZomboidFileSystem;
use crate::org::uncommons::maths::random::cellular_automaton_rng::CellularAutomatonRng;

/// What the startup steps leave that the Lua state reads.
pub struct ClientStartup {
    /// The game folder.
    pub root: PathBuf,
    /// ZomboidFileSystem.instance after `init()`.
    pub fs: ZomboidFileSystem,
    /// Translator's statics after `loadFiles()`.
    pub translator: RefCell<Translator>,
    /// TextManager.instance after `Init()` (taken by the Lua state's TextManager object).
    pub text_manager: RefCell<Option<TextManager>>,
    /// Texture's and GameWindow's texture statics (the packs' index, the shared table): empty until initShared.
    pub textures: RefCell<Textures>,
    /// What initShared leaves (unset before it runs).
    pub shared: OnceCell<Shared>,
    /// Rand (RandStandard.INSTANCE) and RandLua.INSTANCE, seeded again by initShared.
    pub rand_standard: RefCell<RandAbstract<CellularAutomatonRng>>,
    pub rand_lua: RefCell<RandAbstract<CellularAutomatonRng>>,
}

/// The statics GameWindow.initShared fills (after LuaManager.init, before LoadDirBase).
pub struct Shared {
    /// PerkFactory.init's perks.
    pub perks: PerkFactory,
    /// ScriptManager.instance after `Load()` (the ported part).
    pub scripts: ScriptManager,
    /// The item collection's lookup data (`scripts.item_index()`).
    pub item_index: BucketIndex,
    /// The index in `scripts.items` of each item's full type.
    pub item_by_full_type: HashMap<String, usize>,
    /// CharacterTraitDefinition.characterTraitDefinitions (ScriptManager.Load's trait scripts).
    pub traits: CharacterTraitDefinitions,
    /// CharacterProfessionDefinition.characterProfessionDefinitions (its profession scripts).
    pub professions: CharacterProfessionDefinitions,
}

/// The registry lookups ScriptManager.Load's definition scripts use: whether `Registries.CHARACTER_TRAIT` and
/// `Registries.CHARACTER_PROFESSION` hold a location.
pub struct Registries<'a> {
    pub character_trait: &'a dyn Fn(&ResourceLocation) -> bool,
    pub character_profession: &'a dyn Fn(&ResourceLocation) -> bool,
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
    /// The startup in the original's order up to LuaManager.init, the steps the Lua state sees:
    /// InitDisplay with no options.ini: `Core.setOptionLanguageName(user.language.toUpperCase())`;
    /// mainThreadInit: `ZomboidFileSystem.instance.init()`;
    /// init: `initFonts()`, the mods (none: `getModIDs` stays empty), `Languages.instance.init()`,
    /// `Translator.language = null`, `initFonts()`, `Translator.loadFiles()`.
    /// The caller makes the Lua state (LuaManager.init), then runs `init_shared`.
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
        Ok(ClientStartup {
            root: root.to_path_buf(),
            fs,
            translator: RefCell::new(translator),
            text_manager: RefCell::new(Some(tm)),
            textures: RefCell::new(Textures::new(root)),
            shared: OnceCell::new(),
            // MainScreenState.main's init; drawn from only after initShared seeds them again
            rand_standard: RefCell::new(RandAbstract::new(CellularAutomatonRng::new(
                RAND_STANDARD_SEED,
            ))),
            rand_lua: RefCell::new(RandAbstract::new(CellularAutomatonRng::new(RAND_LUA_SEED))),
        })
    }

    /// The UI texture packs initShared loads (`LoadTexturePack(name, 2)`), in order.
    pub const UI_PACKS: [&'static str; 7] = [
        "UI",
        "UI2",
        "IconsMoveables",
        "RadioIcons",
        "ApComUI",
        "Mechanics",
        "WeatherFx",
    ];

    /// zombie.GameWindow.initShared, the steps the Lua state sees: the UI texture packs and their lookup,
    /// `PerkFactory.init()`, `ScriptManager.instance.Load()` (items, the trait and profession definition scripts),
    /// RandStandard/RandLua `init()` (seeded, see RAND_STANDARD_SEED).
    pub fn init_shared(&self, registries: &Registries) -> Result<(), String> {
        {
            let mut textures = self.textures.borrow_mut();
            for p in Self::UI_PACKS {
                textures.load_texture_pack(&self.fs, p);
            }
            textures.set_texture_pack_lookup();
        }
        let mut translator = self.translator.borrow_mut();
        let perks = PerkFactory::init(&mut translator)?;
        let scripts = ScriptManager::load(&self.root)?;
        let traits = CharacterTraitDefinitions::load(
            &scripts.bucket_scripts("character_trait_definition"),
            registries.character_trait,
            &mut translator,
        );
        let professions = CharacterProfessionDefinitions::load(
            &scripts.bucket_scripts("character_profession_definition"),
            registries.character_profession,
            registries.character_trait,
            &mut translator,
        );
        let item_index = scripts.item_index();
        let item_by_full_type = (scripts.items.iter().enumerate())
            .map(|(i, item)| (item.full_name(), i))
            .collect();
        *self.rand_standard.borrow_mut() =
            RandAbstract::new(CellularAutomatonRng::new(RAND_STANDARD_SEED));
        *self.rand_lua.borrow_mut() = RandAbstract::new(CellularAutomatonRng::new(RAND_LUA_SEED));
        let shared = Shared {
            perks,
            scripts,
            item_index,
            item_by_full_type,
            traits,
            professions,
        };
        self.shared
            .set(shared)
            .map_err(|_| "GameWindow.initShared ran twice".to_string())
    }

    /// The tile texture packs GameWindow.enter loads (texture2x: the 2x ones), in order.
    pub const TILE_PACKS: [&'static str; 8] = [
        "Tiles2x",
        "Overlays2x",
        "JumboTrees2x",
        "JumboTreesBigs2x",
        "Tiles2x.floor",
        "B42ChunkCaching2x",
        "B42ChunkCaching2x.floor",
        "Clock2x",
    ];

    /// zombie.GameWindow.enter before its OnGameBoot, the part the Lua state sees: the tile texture packs (Core's
    /// texture2x option is on by default: the 2x ones) and their lookup. The original then checks that
    /// `TileIndieStoneTentFrontLeft` exists (a RuntimeException otherwise; `Err` here).
    pub fn enter(&self) -> Result<(), String> {
        let mut t = self.textures.borrow_mut();
        for p in Self::TILE_PACKS {
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
    /// ScriptManager.FindItem(name): the index of the item in the scripts' `items` (none before initShared).
    pub fn find_item(&self, name: &str) -> Option<usize> {
        let shared = self.shared.get()?;
        let full = shared.scripts.find_item(&shared.item_index, name)?;
        shared.item_by_full_type.get(&full).copied()
    }
}
