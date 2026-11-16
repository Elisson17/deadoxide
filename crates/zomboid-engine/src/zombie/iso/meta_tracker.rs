//! zombie.iso.MetaTracker (the read side of `metadata.bin`), with the two classes it loads: zombie.iso.Helicopter
//! (the part a save keeps) and the alarm list of zombie.AmbientStreamManager (zombie.iso.Alarm).
//!
//! Not ported: `save`, the helicopter's and the alarms' update and sound code, and the single-player branch of
//! `Helicopter.load` (`active && !GameServer.server && !GameClient.client` takes `IsoPlayer.players[0]` as the target
//! and sets the move vector; it needs the player). What is proved is the dedicated-server run of the original.
use crate::java_nio::ByteBuffer;

/// zombie.iso.Helicopter.State
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelicopterState {
    Arriving,
    Hovering,
    Searching,
    Leaving,
}

impl HelicopterState {
    /// `State.values()[ordinal]`: `ArrayIndexOutOfBoundsException` outside 0..4.
    pub fn from_ordinal(i: i32) -> Result<Self, String> {
        Ok(match i {
            0 => HelicopterState::Arriving,
            1 => HelicopterState::Hovering,
            2 => HelicopterState::Searching,
            3 => HelicopterState::Leaving,
            _ => {
                return Err(format!(
                    "java.lang.ArrayIndexOutOfBoundsException: Index {i} out of bounds for length 4"
                ))
            }
        })
    }

    /// `name()`
    pub fn name(self) -> &'static str {
        match self {
            HelicopterState::Arriving => "Arriving",
            HelicopterState::Hovering => "Hovering",
            HelicopterState::Searching => "Searching",
            HelicopterState::Leaving => "Leaving",
        }
    }
}

/// zombie.iso.Helicopter, the saved fields.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Helicopter {
    pub state: Option<HelicopterState>,
    pub active: bool,
    pub x: f32,
    pub y: f32,
}

impl Helicopter {
    /// zombie.iso.Helicopter.load on a dedicated server: the active flag, the state ordinal, the position. A failure
    /// part-way keeps the fields read before it.
    pub fn load(&mut self, bb: &mut ByteBuffer, _world_version: i32) -> Result<(), String> {
        self.active = bb.get()? != 0;
        self.state = Some(HelicopterState::from_ordinal(bb.get_int()?)?);
        self.x = bb.get_float()?;
        self.y = bb.get_float()?;
        Ok(())
    }
}

/// zombie.iso.Alarm, the saved fields (the constructor's end time is replaced by `load`).
#[derive(Clone, Debug, PartialEq)]
pub struct Alarm {
    pub x: i32,
    pub y: i32,
    pub end_game_time: f32,
}

impl Alarm {
    /// zombie.iso.Alarm.load
    pub fn load(bb: &mut ByteBuffer, _world_version: i32) -> Result<Self, String> {
        let x = bb.get_int()?;
        let y = bb.get_int()?;
        let end_game_time = bb.get_float()?;
        Ok(Alarm {
            x,
            y,
            end_game_time,
        })
    }
}

/// zombie.AmbientStreamManager.load: a short count, then that many alarms appended to `alarm_list` as each is read.
pub fn ambient_stream_manager_load(
    alarm_list: &mut Vec<Alarm>,
    bb: &mut ByteBuffer,
    world_version: i32,
) -> Result<(), String> {
    let size = bb.get_short()?;
    for _ in 0..size.max(0) {
        alarm_list.push(Alarm::load(bb, world_version)?);
    }
    Ok(())
}

/// zombie.iso.MetaTracker.load over the bytes of `metadata.bin` (`None` is a missing file): the world version, then
/// the helicopter, then the ambient stream manager's alarms. The original logs a failure and keeps what was loaded;
/// this returns the error text.
pub fn load(
    data: Option<&[u8]>,
    helicopter: &mut Helicopter,
    alarm_list: &mut Vec<Alarm>,
) -> Result<(), String> {
    let Some(data) = data else {
        return Ok(());
    };
    let mut bb = ByteBuffer::wrap(data);
    let world_version = bb.get_int()?;
    helicopter.load(&mut bb, world_version)?;
    ambient_stream_manager_load(alarm_list, &mut bb, world_version)
}
