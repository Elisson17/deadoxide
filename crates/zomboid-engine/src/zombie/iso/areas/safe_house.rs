//! zombie.iso.areas.SafeHouse: a claimed building, the saved fields.
use crate::java_nio::{read_string, ByteBuffer};

/// zombie.iso.areas.SafeHouse, the fields `load` reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SafeHouse {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub owner: String,
    pub hit_points: i32,
    /// `players`: the owner first, then each saved player not already in the list (`addPlayer`).
    pub players: Vec<String>,
    pub last_visited: i64,
    pub title: String,
    /// `datetimeCreated` as saved (world version 223 and later). The original replaces 0 with the wall clock.
    pub datetime_created: i64,
    /// `location` as saved (world version 223 and later). The original looks an empty one up in the spawn regions.
    pub location: String,
    pub players_respawn: Vec<String>,
}

impl SafeHouse {
    /// zombie.iso.areas.SafeHouse.load. Not modelled: the constructor's `id` ("x,y at <wall clock>") and
    /// `onlineId`, the wall-clock replacement of a zero creation time, the spawn-region lookup of an empty location,
    /// the server chat and `updateSafehousePlayersConnected`, and the default `title`/`hitPoints` of older saves.
    pub fn load(b: &mut ByteBuffer, world_version: i32) -> Result<Self, String> {
        let (x, y, w, h) = (b.get_int()?, b.get_int()?, b.get_int()?, b.get_int()?);
        let owner = read_string(b)?;
        let mut s = SafeHouse {
            x,
            y,
            w,
            h,
            players: vec![owner.clone()],
            owner,
            ..SafeHouse::default()
        };
        if world_version >= 216 {
            s.hit_points = b.get_int()?;
        }
        for _ in 0..b.get_int()? {
            let p = read_string(b)?;
            if !s.players.contains(&p) {
                s.players.push(p);
            }
        }
        s.last_visited = b.get_long()?;
        s.title = read_string(b)?;
        if world_version >= 223 {
            s.datetime_created = b.get_long()?;
            s.location = read_string(b)?;
        }
        for _ in 0..b.get_int()? {
            s.players_respawn.push(read_string(b)?);
        }
        Ok(s)
    }
}
