//! zombie.globalObjects.GlobalObjectLookup: objects indexed by cell, then by chunk of the cell, shared by every system
//! of one side (`sharedServer`). A square outside the map has no cell: its object is in its system's list but not
//! found by the lookup.

/// What `GlobalObjectLookup.init(IsoMetaGrid)` keeps of the grid: `minX`, `minY`, `getWidth()`, `getHeight()` in cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MetaGridBounds {
    pub min_x: i32,
    pub min_y: i32,
    pub width: i32,
    pub height: i32,
}

/// An object as the chunk lists hold it: its system and its index in that system's object list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectRef {
    pub system: usize,
    pub index: usize,
}

/// zombie.globalObjects.GlobalObjectLookup.Cell: 32 x 32 chunks, each created on first use.
struct Cell {
    chunks: Vec<Option<Vec<ObjectRef>>>,
}

/// zombie.globalObjects.GlobalObjectLookup (with its `Shared` cell array).
pub struct GlobalObjectLookup {
    bounds: MetaGridBounds,
    cells: Vec<Option<Cell>>,
}

impl GlobalObjectLookup {
    /// `GlobalObjectLookup.init`: a cell slot for each cell of the grid.
    pub fn new(bounds: MetaGridBounds) -> Self {
        let n = i64::from(bounds.width.max(0)) * i64::from(bounds.height.max(0));
        let mut cells = Vec::new();
        cells.resize_with(n as usize, || None);
        GlobalObjectLookup { bounds, cells }
    }

    /// `getCellAt`: the slot index of the cell holding a square, `None` outside the grid (the original logs an error).
    /// Java int arithmetic wraps.
    fn cell_index(&self, square_x: i32, square_y: i32) -> Option<usize> {
        let b = self.bounds;
        let x = square_x.wrapping_sub(b.min_x.wrapping_mul(256));
        let y = square_y.wrapping_sub(b.min_y.wrapping_mul(256));
        if x >= 0 && y >= 0 && x < b.width.wrapping_mul(256) && y < b.height.wrapping_mul(256) {
            let cell_x = x / 256;
            let cell_y = y / 256;
            Some(cell_x.wrapping_add(cell_y.wrapping_mul(b.width)) as usize)
        } else {
            None
        }
    }

    /// `Cell.getChunkAt`'s index: the chunk of a square inside the cell at slot `index`.
    fn chunk_index(&self, index: usize, square_x: i32, square_y: i32) -> usize {
        let b = self.bounds;
        let cell_x = b.min_x + (index as i32 % b.width);
        let cell_y = b.min_y + (index as i32 / b.width);
        let x = square_x.wrapping_sub(cell_x.wrapping_mul(256)) / 8;
        let y = square_y.wrapping_sub(cell_y.wrapping_mul(256)) / 8;
        (x + y * 32) as usize
    }

    /// `getChunkForChunkPos(wx, wy, false)`: the objects listed in a chunk, if it has a list.
    fn chunk_objects(&self, wx: i32, wy: i32) -> Option<&Vec<ObjectRef>> {
        let (sx, sy) = (wx.wrapping_mul(8), wy.wrapping_mul(8));
        let index = self.cell_index(sx, sy)?;
        let chunk = self.chunk_index(index, sx, sy);
        self.cells[index].as_ref()?.chunks[chunk].as_ref()
    }

    /// `addObject`: lists the object in its chunk, creating the cell and the chunk; an object outside the grid is not
    /// listed. `Cell.addObject` throws "duplicate object" for an object already in the chunk list.
    pub fn add_object(&mut self, object: ObjectRef, x: i32, y: i32) -> Result<(), String> {
        let Some(index) = self.cell_index(x, y) else {
            return Ok(());
        };
        let chunk = self.chunk_index(index, x, y);
        let cell = self.cells[index].get_or_insert_with(|| {
            let mut chunks = Vec::new();
            chunks.resize_with(1024, || None);
            Cell { chunks }
        });
        let list = cell.chunks[chunk].get_or_insert_with(Vec::new);
        if list.contains(&object) {
            return Err("java.lang.IllegalStateException: duplicate object".to_string());
        }
        list.push(object);
        Ok(())
    }

    /// `getObjectAt`: the object of `system` at the square, scanning the chunk list in order. `locate` gives the
    /// position of an object (the lookup keeps references only).
    pub fn get_object_at(
        &self,
        system: usize,
        x: i32,
        y: i32,
        z: i32,
        locate: &dyn Fn(ObjectRef) -> (i32, i32, i32),
    ) -> Option<ObjectRef> {
        let index = self.cell_index(x, y)?;
        let chunk = self.chunk_index(index, x, y);
        let list = self.cells[index].as_ref()?.chunks[chunk].as_ref()?;
        list.iter()
            .copied()
            .find(|o| o.system == system && locate(*o) == (x, y, z))
    }

    /// `hasObjectsInChunk`
    pub fn has_objects_in_chunk(&self, system: usize, wx: i32, wy: i32) -> bool {
        self.chunk_objects(wx, wy)
            .is_some_and(|l| l.iter().any(|o| o.system == system))
    }

    /// `getObjectsInChunk`: the system's objects of the chunk in list order.
    pub fn get_objects_in_chunk(&self, system: usize, wx: i32, wy: i32) -> Vec<ObjectRef> {
        self.chunk_objects(wx, wy)
            .map(|l| l.iter().copied().filter(|o| o.system == system).collect())
            .unwrap_or_default()
    }
}
