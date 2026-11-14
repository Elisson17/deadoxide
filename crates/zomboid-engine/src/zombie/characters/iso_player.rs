//! zombie.characters.IsoPlayer: the player character as a save stores it (`players.db`), and
//! zombie.savefile.PlayerDB's local-player reading.
use std::collections::{BTreeMap, BTreeSet};

use super::body_damage::body_part::BodySandbox;
use super::body_damage::fitness::Fitness;
use super::body_damage::nutrition::Nutrition;
use super::iso_game_character::IsoGameCharacter;
use super::player_craft_history::PlayerCraftHistory;
use crate::java_nio::{read_string, ByteBuffer};
use crate::zombie::inventory::inventory_item::ItemFactory;
use crate::zombie::scripting::objects::resource_location::ResourceLocation;
use crate::zombie::world::dictionary_data::DictionaryData;

/// `savedVehicleX`, `savedVehicleY`, `savedVehicleSeat`, `savedVehicleRunning`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SavedVehicle {
    pub x: f32,
    pub y: f32,
    pub seat: i8,
    pub running: bool,
}

/// zombie.characters.IsoPlayer, the saved fields.
#[derive(Clone, Debug, PartialEq)]
pub struct IsoPlayer {
    pub character: IsoGameCharacter,
    pub hours_survived: f64,
    pub zombie_kills: i32,
    /// (body location, index into `saved_items`) for each saved worn item whose index is in range. The original also
    /// drops locations its `BodyLocationGroup` does not know and lets `WornItems.setItem` replace conflicting items;
    /// neither is modelled.
    pub worn_items: Vec<(ResourceLocation, usize)>,
    pub survivor_kills: i32,
    pub nutrition: Nutrition,
    pub all_chat_muted: bool,
    pub tag_prefix: String,
    pub tag_color: [f32; 3],
    pub display_name: String,
    pub show_tag: bool,
    pub faction_pvp: bool,
    /// `None` before world version 239 (the field initialiser stays).
    pub auto_drink: Option<bool>,
    pub saved_vehicle: Option<SavedVehicle>,
    /// `mechanicsItem` (a HashMap in the original; ordered here).
    pub mechanics_item: BTreeMap<i64, i64>,
    pub fitness: Fitness,
    /// Full types of `alreadyReadBook`, in save order (registry ids the dictionary does not know are dropped).
    pub already_read_book: Vec<String>,
    pub known_media_lines: BTreeSet<String>,
    pub craft_history: PlayerCraftHistory,
}

impl IsoPlayer {
    /// zombie.characters.IsoPlayer.load: the serialise flag and object class id, IsoGameCharacter.load, then the
    /// player's fields. The random speech colour (three `Rand` draws) and the sprite/visual refresh are not modelled.
    pub fn load(
        b: &mut ByteBuffer,
        world_version: i32,
        factory: &dyn ItemFactory,
        dictionary: &DictionaryData,
        sb: &BodySandbox,
    ) -> Result<Self, String> {
        let _serialise = b.get()?;
        let _object_id = b.get()?;
        let mut character = IsoGameCharacter::load(b, world_version, factory, sb)?;
        let hours_survived = b.get_double()?;
        if character.descriptor.is_none() {
            return Err(
                "java.lang.NullPointerException: IsoPlayer.load without a descriptor".into(),
            );
        }
        let zombie_kills = b.get_int()?;
        let n_items = character.saved_items.len();
        let in_range = |i: i16| usize::try_from(i).ok().filter(|&i| i < n_items);
        let worn_count = b.get()?;
        let mut worn_items = Vec::new();
        for _ in 0..worn_count {
            let location = ResourceLocation::of(&read_string(b)?)?;
            let index = b.get_short()?;
            if let Some(i) = in_range(index) {
                worn_items.push((location, i));
            }
        }
        if let Some(i) = in_range(b.get_short()?) {
            character.left_hand = Some(i);
        }
        if let Some(i) = in_range(b.get_short()?) {
            character.right_hand = Some(i);
        }
        let survivor_kills = b.get_int()?;
        let nutrition = Nutrition::load(b)?;
        let all_chat_muted = b.get()? != 0;
        let tag_prefix = read_string(b)?;
        let tag_color = [b.get_float()?, b.get_float()?, b.get_float()?];
        let display_name = read_string(b)?;
        let show_tag = b.get()? != 0;
        let faction_pvp = b.get()? != 0;
        let auto_drink = if world_version >= 239 {
            Some(b.get()? != 0)
        } else {
            None
        };
        let _ = b.get()?;
        let saved_vehicle = if b.get()? != 0 {
            let v = SavedVehicle {
                x: b.get_float()?,
                y: b.get_float()?,
                seat: b.get()?,
                running: b.get()? != 0,
            };
            character.moving.z = 0.0;
            Some(v)
        } else {
            None
        };
        let mut mechanics_item = BTreeMap::new();
        for _ in 0..b.get_int()? {
            let k = b.get_long()?;
            mechanics_item.insert(k, b.get_long()?);
        }
        let fitness = Fitness::load(b)?;
        let mut already_read_book = Vec::new();
        for _ in 0..b.get_short()? {
            let id = b.get_short()?;
            if let Some(t) = dictionary.get_item_type_from_id(id) {
                already_read_book.push(t.to_string());
            }
        }
        // loadKnownMediaLines
        let mut known_media_lines = BTreeSet::new();
        for _ in 0..b.get_short()? {
            known_media_lines.insert(read_string(b)?);
        }
        if world_version >= 203 {
            let v = i32::from(b.get()?).clamp(0, 3);
            if let Some(d) = &mut character.descriptor {
                d.voice_type = v;
            }
        }
        let craft_history = if world_version >= 228 {
            PlayerCraftHistory::load(b)?
        } else {
            PlayerCraftHistory::default()
        };
        Ok(IsoPlayer {
            character,
            hours_survived,
            zombie_kills,
            worn_items,
            survivor_kills,
            nutrition,
            all_chat_muted,
            tag_prefix,
            tag_color,
            display_name,
            show_tag,
            faction_pvp,
            auto_drink,
            saved_vehicle,
            mechanics_item,
            fitness,
            already_read_book,
            known_media_lines,
            craft_history,
        })
    }
}

/// zombie.savefile.PlayerDB.PlayerData: one row of `localPlayers`.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerData {
    pub sql_id: i32,
    pub name: Option<String>,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub world_version: i32,
    pub is_dead: bool,
    pub data: Vec<u8>,
}

/// zombie.savefile.PlayerDB.SQLPlayerStore, the reading part.
#[cfg(not(target_arch = "wasm32"))]
pub mod player_db {
    use super::PlayerData;

    /// zombie.savefile.PlayerDB.SQLPlayerStore: the ids of `localPlayers` (`SELECT id FROM localPlayers`) and each row
    /// as `load(PlayerData)` reads it. Opened read-only.
    pub fn load_local_players(db: &std::path::Path) -> Result<Vec<PlayerData>, String> {
        let conn = rusqlite::Connection::open_with_flags(
            db,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| format!("{}: {e}", db.display()))?;
        let ids: Vec<i32> = {
            let mut st = conn
                .prepare("SELECT id FROM localPlayers")
                .map_err(|e| e.to_string())?;
            let rows = st.query_map([], |r| r.get(0)).map_err(|e| e.to_string())?;
            rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
        };
        let mut out = Vec::new();
        for id in ids {
            let mut st = conn
                .prepare("SELECT data,worldversion,x,y,z,isDead,name FROM localPlayers WHERE id=?")
                .map_err(|e| e.to_string())?;
            let row = st
                .query_row([id], |r| {
                    Ok(PlayerData {
                        sql_id: id,
                        data: r.get(0)?,
                        world_version: r.get(1)?,
                        x: r.get::<_, f64>(2)? as f32,
                        y: r.get::<_, f64>(3)? as f32,
                        z: r.get::<_, f64>(4)? as f32,
                        is_dead: r.get::<_, Option<i64>>(5)?.unwrap_or(0) != 0,
                        name: r.get(6)?,
                    })
                })
                .map_err(|e| e.to_string())?;
            out.push(row);
        }
        Ok(out)
    }
}
