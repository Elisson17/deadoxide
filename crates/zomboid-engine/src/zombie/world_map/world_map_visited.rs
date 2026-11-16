//! zombie.worldMap.WorldMapVisited: which parts of the map a player has seen or knows (two bits per 32 x 32 square
//! unit, 8 x 8 units per cell) and its save file `map_visited.bin`.
//!
//! Ported: `setBounds`, `getInstance`'s load / known / save steps, `load` (the current format and the legacy row
//! format), the flag setters and clearers (`setVisitedInSquares` ... `clearKnownInCells`), `save` and `saveHeader`.
//! Not ported: the texture and shader state (client rendering), the change rectangles, the network and per-player
//! variants (`updatePlayer`, `processDataChunk`, `receiveRequestData`), `hasFlags` and `isVisited`.
use crate::zombie::core::math::pz_math;

/// The `IsoMetaGrid` bounds in cells that every size and clamp of this class reads (`minX`, `minY`, `maxX`, `maxY`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridBounds {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}

/// zombie.worldMap.WorldMapVisited
#[derive(Clone, Debug)]
pub struct WorldMapVisited {
    grid: GridBounds,
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
    /// `byte[] visited`: two bits per unit, `getSpan()` bytes per row.
    pub visited: Vec<u8>,
    pub main_menu: bool,
}

/// zombie.worldMap.WorldMapVisited.HEADER_BYTES
const HEADER_BYTES: usize = 28;
/// `SliceY.SliceBuffer`'s capacity.
const SLICE_BUFFER_CAPACITY: usize = 10_485_760;

/// A `BufferedInputStream` over the file bytes: `read(b, off, len)` fills `b` with as many bytes as the file has left
/// (up to `len`) and returns the count, or -1 at the end of the file.
struct Stream<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Stream<'_> {
    fn read(&mut self, buf: &mut [u8], off: usize, len: usize) -> i64 {
        if len == 0 {
            return 0;
        }
        if self.pos >= self.data.len() {
            return -1;
        }
        let n = len.min(self.data.len() - self.pos);
        buf[off..off + n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        n as i64
    }
}

fn be_int(buf: &[u8], at: usize) -> i32 {
    i32::from_be_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]])
}

impl WorldMapVisited {
    /// `getWidthInCells`
    fn width_in_cells(&self) -> i32 {
        self.grid
            .max_x
            .wrapping_sub(self.grid.min_x)
            .wrapping_add(1)
    }
    /// `getHeightInCells`
    fn height_in_cells(&self) -> i32 {
        self.grid
            .max_y
            .wrapping_sub(self.grid.min_y)
            .wrapping_add(1)
    }
    /// `getSpan`
    fn span(&self) -> i32 {
        self.width_in_cells().wrapping_mul(8) / 4
    }
    /// `getVisitedLength`
    pub fn visited_length(&self) -> i32 {
        self.span()
            .wrapping_mul(self.height_in_cells())
            .wrapping_mul(8)
    }

    /// `new WorldMapVisited()` then `setBounds(metaGrid.minX, minY, maxX, maxY)`: `getInstance` before it loads.
    pub fn new(grid: GridBounds) -> Self {
        let mut v = WorldMapVisited {
            grid,
            min_x: 0,
            min_y: 0,
            max_x: 0,
            max_y: 0,
            visited: Vec::new(),
            main_menu: false,
        };
        v.set_bounds(grid.min_x, grid.min_y, grid.max_x, grid.max_y);
        v
    }

    /// `setBounds`: bounds where min exceeds max become 0 and mark the main menu; the array is sized by the grid.
    fn set_bounds(&mut self, min_x: i32, min_y: i32, max_x: i32, max_y: i32) {
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (min_x, min_y, max_x, max_y);
        if min_x > max_x || min_y > max_y {
            max_y = 0;
            min_y = 0;
            max_x = 0;
            min_x = 0;
            self.main_menu = true;
        }
        self.min_x = min_x;
        self.min_y = min_y;
        self.max_x = max_x;
        self.max_y = max_y;
        self.visited = vec![0; self.visited_length().max(0) as usize];
    }

    /// `normalizeX`
    fn normalize_x(&self, x: i32) -> i32 {
        let x = x.wrapping_sub(self.grid.min_x.wrapping_mul(256));
        pz_math::clamp_i32(
            x,
            0,
            self.width_in_cells().wrapping_mul(256).wrapping_sub(1),
        )
    }
    /// `normalizeY`
    fn normalize_y(&self, y: i32) -> i32 {
        let y = y.wrapping_sub(self.grid.min_y.wrapping_mul(256));
        pz_math::clamp_i32(
            y,
            0,
            self.height_in_cells().wrapping_mul(256).wrapping_sub(1),
        )
    }

    /// `coordToUnit`
    fn coord_to_unit(coord: i32, is_max: bool) -> i32 {
        let mut c = coord / 32;
        if is_max && c % 32 == 0 {
            c -= 1;
        }
        c
    }

    /// `getFlags(x, y, visited)`: `b >> x % 4 * 2 & 3` on the signed byte.
    fn get_flags_unit(&self, x: i32, y: i32) -> i32 {
        let b = i32::from(self.visited[(x / 4 + y.wrapping_mul(self.span())) as usize] as i8);
        b >> (x % 4 * 2) & 3
    }

    /// `setFlags(x, y, flags, visited)` on a unit: the flags value is not masked, as in the original.
    fn set_flags_unit(&mut self, x: i32, y: i32, flags: i32) {
        let i = (x / 4 + y.wrapping_mul(self.span())) as usize;
        let b = i32::from(self.visited[i] as i8);
        let mask = 3 << (x % 4 * 2);
        let shift = x % 4 * 2;
        let b = b & !mask | flags.wrapping_shl(shift as u32);
        self.visited[i] = b as u8;
    }

    /// `setFlags(xMin, yMin, xMax, yMax, flags, visited)`: ORs the flags into every unit of the clamped region.
    pub fn set_flags(
        &mut self,
        x_min: i32,
        y_min: i32,
        x_max: i32,
        y_max: i32,
        flags: i32,
    ) -> bool {
        let x_min = self.normalize_x(x_min);
        let y_min = self.normalize_y(y_min);
        let x_max = self.normalize_x(x_max);
        let y_max = self.normalize_y(y_max);
        if x_min != x_max && y_min != y_max {
            let x_min_unit = Self::coord_to_unit(x_min, false);
            let x_max_unit = Self::coord_to_unit(x_max, true);
            let y_min_unit = Self::coord_to_unit(y_min, false);
            let y_max_unit = Self::coord_to_unit(y_max, true);
            let mut changed = false;
            for y in y_min_unit..=y_max_unit {
                for x in x_min_unit..=x_max_unit {
                    let it = self.get_flags_unit(x, y);
                    if it & flags != flags {
                        self.set_flags_unit(x, y, it | flags);
                        changed = true;
                    }
                }
            }
            changed
        } else {
            false
        }
    }

    /// `clearFlags(xMin, yMin, xMax, yMax, flags)`
    pub fn clear_flags(&mut self, x_min: i32, y_min: i32, x_max: i32, y_max: i32, flags: i32) {
        let x_min = self.normalize_x(x_min);
        let y_min = self.normalize_y(y_min);
        let x_max = self.normalize_x(x_max);
        let y_max = self.normalize_y(y_max);
        if x_min != x_max && y_min != y_max {
            let x_min_unit = Self::coord_to_unit(x_min, false);
            let x_max_unit = Self::coord_to_unit(x_max, true);
            let y_min_unit = Self::coord_to_unit(y_min, false);
            let y_max_unit = Self::coord_to_unit(y_max, true);
            for y in y_min_unit..=y_max_unit {
                for x in x_min_unit..=x_max_unit {
                    let it = self.get_flags_unit(x, y);
                    if it & flags != 0 {
                        self.set_flags_unit(x, y, it & !flags);
                    }
                }
            }
        }
    }

    /// `setKnownInCells`
    pub fn set_known_in_cells(&mut self, min_x: i32, min_y: i32, max_x: i32, max_y: i32) {
        self.set_flags(
            min_x.wrapping_mul(256),
            min_y.wrapping_mul(256),
            max_x.wrapping_add(1).wrapping_mul(256),
            max_y.wrapping_add(1).wrapping_mul(256),
            2,
        );
    }
    /// `clearKnownInCells`
    pub fn clear_known_in_cells(&mut self, min_x: i32, min_y: i32, max_x: i32, max_y: i32) {
        self.clear_flags(
            min_x.wrapping_mul(256),
            min_y.wrapping_mul(256),
            max_x.wrapping_add(1).wrapping_mul(256),
            max_y.wrapping_add(1).wrapping_mul(256),
            2,
        );
    }
    /// `setVisitedInCells`
    pub fn set_visited_in_cells(&mut self, min_x: i32, min_y: i32, max_x: i32, max_y: i32) {
        self.set_flags(
            min_x.wrapping_mul(256),
            min_y.wrapping_mul(256),
            max_x.wrapping_mul(256),
            max_y.wrapping_mul(256),
            1,
        );
    }
    /// `clearVisitedInCells`
    pub fn clear_visited_in_cells(&mut self, min_x: i32, min_y: i32, max_x: i32, max_y: i32) {
        self.clear_flags(
            min_x.wrapping_mul(256),
            min_y.wrapping_mul(256),
            max_x.wrapping_mul(256),
            max_y.wrapping_mul(256),
            1,
        );
    }
    /// `setKnownInSquares`
    pub fn set_known_in_squares(&mut self, min_x: i32, min_y: i32, max_x: i32, max_y: i32) {
        self.set_flags(min_x, min_y, max_x, max_y, 2);
    }
    /// `clearKnownInSquares`
    pub fn clear_known_in_squares(&mut self, min_x: i32, min_y: i32, max_x: i32, max_y: i32) {
        self.clear_flags(min_x, min_y, max_x, max_y, 2);
    }
    /// `setVisitedInSquares`
    pub fn set_visited_in_squares(&mut self, min_x: i32, min_y: i32, max_x: i32, max_y: i32) {
        self.set_flags(min_x, min_y, max_x, max_y, 1);
    }
    /// `clearVisitedInSquares`
    pub fn clear_visited_in_squares(&mut self, min_x: i32, min_y: i32, max_x: i32, max_y: i32) {
        self.clear_flags(min_x, min_y, max_x, max_y, 1);
    }

    /// `saveHeader` and the array: the bytes `save()` writes.
    pub fn save(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_BYTES + self.visited.len());
        for v in [249, 2, self.min_x, self.min_y, self.max_x, self.max_y, 8] {
            out.extend_from_slice(&v.to_be_bytes());
        }
        out.extend_from_slice(&self.visited);
        out
    }

    /// `load()` over the bytes of `map_visited.bin` (`None` is a missing file, which the original ignores). The
    /// shared slice buffer is taken as zero-filled when the load starts (see `ByteBuffer::wrap_slice_buffer`); the
    /// legacy row format reads each row into that same buffer, so a row cut short keeps the previous row's bytes
    /// after the new ones. An `Err` is the exception `getInstance` logs; what was set before it stays.
    pub fn load(&mut self, file: Option<&[u8]>) -> Result<(), String> {
        let Some(file) = file else {
            return Ok(());
        };
        let mut buf = vec![0u8; 28];
        let mut bis = Stream { data: file, pos: 0 };
        bis.read(&mut buf, 0, 24);
        let world_version = be_int(&buf, 0);
        let mut at = 4;
        let mut version = 1;
        if world_version >= 234 {
            version = be_int(&buf, at);
            at += 4;
            if !(1..=2).contains(&version) {
                return Err(format!(
                    "java.io.IOException: unknown file version {version}"
                ));
            }
            bis.read(&mut buf, 24, 4);
        }
        let min_x = be_int(&buf, at);
        let min_y = be_int(&buf, at + 4);
        let max_x = be_int(&buf, at + 8);
        let max_y = be_int(&buf, at + 12);
        let upc = be_int(&buf, at + 16);
        if version < 2
            || min_x != self.min_x
            || min_y != self.min_y
            || max_x != self.max_x
            || max_y != self.max_y
            || upc != 8
        {
            let row_len = max_x.wrapping_sub(min_x).wrapping_add(1).wrapping_mul(upc);
            if row_len < 0 {
                return Err(format!("java.lang.NegativeArraySizeException: {row_len}"));
            }
            let row_len = row_len as usize;
            if upc == 0 {
                return Err("java.lang.ArithmeticException: / by zero".to_string());
            }
            let spu = 256 / upc;
            // the row buffer is the slice buffer: row_len bytes are copied out of its start every row
            let mut sbuf = vec![0u8; row_len.max(buf.len())];
            sbuf[..buf.len()].copy_from_slice(&buf);
            if row_len > SLICE_BUFFER_CAPACITY {
                return Err("java.nio.BufferUnderflowException".to_string());
            }
            let mut y = min_y.wrapping_mul(upc);
            let y_end = max_y.wrapping_mul(upc);
            while y <= y_end {
                let num_bytes = bis.read(&mut sbuf, 0, row_len);
                let row = sbuf[..row_len].to_vec();
                if num_bytes < 0 {
                    return Err(format!(
                        "java.lang.IllegalArgumentException: newLimit < 0: ({num_bytes} < 0)"
                    ));
                }
                let mut x = min_x.wrapping_mul(upc);
                let x_end = max_x.wrapping_mul(upc);
                while x <= x_end {
                    let xx = x.wrapping_sub(min_x.wrapping_mul(upc));
                    let flags = i32::from(row[xx as usize] as i8);
                    self.set_flags(
                        x.wrapping_mul(spu),
                        y.wrapping_mul(spu),
                        x.wrapping_mul(spu).wrapping_add(spu).wrapping_sub(1),
                        y.wrapping_mul(spu).wrapping_add(spu).wrapping_sub(1),
                        flags,
                    );
                    x = x.wrapping_add(1);
                    if x == i32::MIN {
                        break;
                    }
                }
                y = y.wrapping_add(1);
                if y == i32::MIN {
                    break;
                }
            }
            return Ok(());
        }
        // the current format: the array is read in slice-buffer sized pieces
        let mut i = 0usize;
        while i < self.visited.len() {
            let size_read = bis.read_into_vec(SLICE_BUFFER_CAPACITY);
            match size_read {
                None => return Err(
                    "java.lang.ArrayIndexOutOfBoundsException: arraycopy: length -1 is negative"
                        .to_string(),
                ),
                Some(chunk) => {
                    if i + chunk.len() > self.visited.len() {
                        return Err(format!(
                            "java.lang.ArrayIndexOutOfBoundsException: arraycopy: last destination index {} out of bounds for byte[{}]",
                            i + chunk.len(),
                            self.visited.len()
                        ));
                    }
                    self.visited[i..i + chunk.len()].copy_from_slice(chunk);
                }
            }
            i += SLICE_BUFFER_CAPACITY;
        }
        Ok(())
    }
}

impl<'a> Stream<'a> {
    /// `read(in.array(), 0, capacity)` followed by the copy out of the buffer: the bytes read (at most `len`), or
    /// `None` at the end of the file.
    fn read_into_vec(&mut self, len: usize) -> Option<&'a [u8]> {
        if self.pos >= self.data.len() {
            return None;
        }
        let n = len.min(self.data.len() - self.pos);
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Some(s)
    }
}
