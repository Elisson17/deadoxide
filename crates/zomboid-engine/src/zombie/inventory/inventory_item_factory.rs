//! zombie.inventory.InventoryItemFactory: creating an item from its WorldDictionary id and item script, as save
//! loading needs it (`CreateItem(short)`), with the recorded-media table items refer to.
use std::collections::{HashMap, HashSet};

use super::inventory_item::{ItemFactory, ItemTemplate, ItemType};
use crate::java;
use crate::zombie::radio::media::recorded_media::RecordedMedia;
use crate::zombie::scripting::item::Value;
use crate::zombie::scripting::script_manager::ScriptManager;
use crate::zombie::world::dictionary_data::DictionaryData;

/// What item creation reads from an item script.
#[derive(Clone, Debug, PartialEq)]
struct ScriptInfo {
    obsolete: bool,
    /// `None`: no base-game `ItemType` (Item.InstanceItem would create nothing).
    item_type: Option<ItemType>,
    condition_max: i32,
    hunger_change: f32,
}

/// zombie.inventory.InventoryItemFactory over a save's dictionary and the loaded item scripts.
pub struct InventoryItemFactory<'a> {
    dictionary: &'a DictionaryData,
    scripts: HashMap<String, ScriptInfo>,
    recorded_media: RecordedMedia,
    /// Ids the Lua `OnInitRecordedMedia` handler registers (`RecMedia["<id>"]` keys of recorded_media.lua).
    media_ids: HashSet<String>,
}

impl<'a> InventoryItemFactory<'a> {
    /// `recorded_media_lua` is media/lua/shared/RecordedMedia/recorded_media.lua; its `RecMedia["<id>"] = {` lines
    /// are the ids ISRecordedMedia.init registers (the Lua is not run).
    pub fn new(
        dictionary: &'a DictionaryData,
        scripts: &ScriptManager,
        recorded_media: RecordedMedia,
        recorded_media_lua: &str,
    ) -> Self {
        let scripts = scripts
            .items
            .iter()
            .map(|i| {
                let item_type = i
                    .unported
                    .iter()
                    .rev()
                    .find(|(k, _)| k == "itemtype")
                    .and_then(|(_, v)| ItemType::from_script(java::trim(v)));
                let condition_max = match i.get("conditionMax") {
                    Some(Value::I32(v)) => *v,
                    _ => 10,
                };
                let hunger_change = match i.get("hungerChange") {
                    Some(Value::F32(v)) => *v,
                    _ => 0.0,
                };
                (
                    i.full_name(),
                    ScriptInfo {
                        obsolete: matches!(i.get("obsolete"), Some(Value::Bool(true))),
                        item_type,
                        condition_max,
                        hunger_change,
                    },
                )
            })
            .collect();
        let media_ids = recorded_media_lua
            .lines()
            .filter_map(|l| {
                l.strip_prefix("RecMedia[\"")?
                    .split('"')
                    .next()
                    .map(str::to_string)
            })
            .collect();
        InventoryItemFactory {
            dictionary,
            scripts,
            recorded_media,
            media_ids,
        }
    }
}

impl ItemFactory for InventoryItemFactory<'_> {
    /// zombie.inventory.InventoryItemFactory.CreateItem (short): a valid dictionary entry whose script exists
    /// (`Moveables.*` types use the `Moveables.Moveable` script).
    fn create_item(&self, registry_id: i16) -> Option<ItemTemplate> {
        let info = self.dictionary.items.get(&registry_id)?;
        if info.obsolete || info.removed {
            return None;
        }
        let script = if info.full_type.starts_with("Moveables.") {
            "Moveables.Moveable"
        } else {
            info.full_type.as_str()
        };
        let si = self.scripts.get(script)?;
        let item_type = si.item_type?;
        Some(ItemTemplate {
            full_type: info.full_type.clone(),
            item_type,
            condition_max: si.condition_max,
            // Item.InstanceItem: food.setBaseHunger(this.hungerChange / 100.0F)
            base_hunger: if item_type == ItemType::Food {
                si.hunger_change / 100.0
            } else {
                0.0
            },
        })
    }

    fn is_obsolete(&self, full_type: &str) -> bool {
        self.scripts.get(full_type).is_some_and(|s| s.obsolete)
    }

    fn media_data_from_index(&self, index: i16) -> Option<String> {
        self.recorded_media
            .get_media_data_from_index(index, &|id| self.media_ids.contains(id))
            .map(str::to_string)
    }
}
