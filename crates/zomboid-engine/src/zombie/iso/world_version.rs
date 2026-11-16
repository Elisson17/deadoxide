//! zombie.iso.IsoWorld.readWorldVersion and the `map_ver.bin` read of `IsoWorld.init`: the files that say which world
//! version and which map list a save was made with.
use crate::java_nio::{read_string_stream, ByteBuffer};

/// The failure of a read past the end of a file stream, as `DataInputStream` raises it (`java.io.EOFException`); the
/// shared buffer type reports it as an underflow.
fn stream_error(e: String) -> String {
    if e.contains("BufferUnderflow") {
        "java.io.EOFException".to_string()
    } else {
        e
    }
}

/// zombie.iso.IsoWorld.readWorldVersion: on a server `map_t.bin`'s "GMTM" magic then its int, otherwise `map_ver.bin`'s
/// int; -1 for a missing file, another magic or a file too short (the original logs the `IOException`).
pub fn read_world_version(server: bool, map_t: Option<&[u8]>, map_ver: Option<&[u8]>) -> i32 {
    let go = || -> Result<i32, String> {
        if server {
            let Some(d) = map_t else {
                return Ok(-1);
            };
            let mut b = ByteBuffer::wrap(d);
            let (b1, b2, b3, b4) = (b.get()?, b.get()?, b.get()?, b.get()?);
            if b1 == 71 && b2 == 77 && b3 == 84 && b4 == 77 {
                return b.get_int();
            }
            Ok(-1)
        } else {
            let Some(d) = map_ver else {
                return Ok(-1);
            };
            ByteBuffer::wrap(d).get_int()
        }
    };
    go().unwrap_or(-1)
}

/// What `IsoWorld.init` reads from `map_ver.bin` when it is not a server: the int world version, then
/// `GameWindow.ReadString(DataInputStream)` (the map list, `Core.gameMap` of the save). Errors are the exception
/// class and message; a short file is the stream's `EOFException`.
pub fn read_map_ver(data: &[u8]) -> Result<(i32, String), String> {
    let mut b = ByteBuffer::wrap(data);
    let version = b.get_int().map_err(stream_error)?;
    let map_dir = read_string_stream(&mut b).map_err(stream_error)?;
    Ok((version, map_dir))
}
