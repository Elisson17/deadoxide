//! zombie.worldMap.symbols.MapSymbolDefinitions: the stamps the world map offers (`Definitions/MapSymbolDefinitions.lua`
//! adds them with `addTexture`).
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::se::krka::kahlua::vm::object::{JavaClass, Userdata};
use crate::zombie::lua::exposer::class_named;

/// zombie.worldMap.symbols.MapSymbolDefinitions.MapSymbolDefinition
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapSymbolDefinition {
    pub id: String,
    pub texture_path: String,
    pub width: i32,
    pub height: i32,
    pub tab: Option<String>,
}

impl Userdata for MapSymbolDefinition {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.worldMap.symbols.MapSymbolDefinitions$MapSymbolDefinition")
    }
    fn java_to_string(&self) -> String {
        format!(
            "zombie.worldMap.symbols.MapSymbolDefinitions$MapSymbolDefinition@{:x}",
            self as *const Self as usize >> 4 & 0x7fff_ffff
        )
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// zombie.worldMap.symbols.MapSymbolDefinitions
#[derive(Default)]
pub struct MapSymbolDefinitions {
    pub symbol_list: RefCell<Vec<Rc<MapSymbolDefinition>>>,
    pub symbol_by_id: RefCell<HashMap<String, Rc<MapSymbolDefinition>>>,
}

impl MapSymbolDefinitions {
    /// zombie.worldMap.symbols.MapSymbolDefinitions.addTexture(String, String, int, int, String)
    pub fn add_texture(&self, id: &str, path: &str, width: i32, height: i32, tab: Option<&str>) {
        let def = Rc::new(MapSymbolDefinition {
            id: id.to_string(),
            texture_path: path.to_string(),
            width,
            height,
            tab: tab.map(str::to_string),
        });
        self.symbol_list.borrow_mut().push(def.clone());
        self.symbol_by_id.borrow_mut().insert(id.to_string(), def);
    }

    /// zombie.worldMap.symbols.MapSymbolDefinitions.addTexture(String, String[, String]): 20x20 when the texture
    /// loads, 18x18 when `Texture.getSharedTexture` gives null (`texture_loaded`).
    pub fn add_texture_sized_by(
        &self,
        id: &str,
        path: &str,
        tab: Option<&str>,
        texture_loaded: bool,
    ) {
        let n = if texture_loaded { 20 } else { 18 };
        self.add_texture(id, path, n, n, tab);
    }

    /// zombie.worldMap.symbols.MapSymbolDefinitions.getSymbolCount
    pub fn get_symbol_count(&self) -> i32 {
        self.symbol_list.borrow().len() as i32
    }
}

impl Userdata for MapSymbolDefinitions {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.worldMap.symbols.MapSymbolDefinitions")
    }
    fn java_to_string(&self) -> String {
        format!(
            "zombie.worldMap.symbols.MapSymbolDefinitions@{:x}",
            self as *const Self as usize >> 4 & 0x7fff_ffff
        )
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
