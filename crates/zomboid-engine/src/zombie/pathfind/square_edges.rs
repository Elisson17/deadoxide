//! The `IsoGridSquare` methods the path code reads (stairs and sloped-surface edges, the elements on a square's edges),
//! for the squares a map loader makes. Player-built objects (`IsoThumpable`) are not modelled by this port's squares:
//! `getThumpableWindow` and `getHoppableThumpable` find nothing here, as they find nothing on a loaded map.
use crate::zombie::core::tile_property_alias_map::TilePropertyAliasMap;
use crate::zombie::iso::iso_directions::IsoDirections;
use crate::zombie::iso::iso_grid_square::{GetSquare, IsoGridSquare};
use crate::zombie::iso::iso_object::ObjectKind;
use crate::zombie::iso::sprite_details::iso_object_type::IsoObjectType as T;

impl IsoGridSquare {
    /// zombie.iso.IsoGridSquare.HasStairsNorth
    pub fn has_stairs_north(&self) -> bool {
        self.has_type(T::stairsTN) || self.has_type(T::stairsMN) || self.has_type(T::stairsBN)
    }

    /// zombie.iso.IsoGridSquare.HasStairsWest
    pub fn has_stairs_west(&self) -> bool {
        self.has_type(T::stairsTW) || self.has_type(T::stairsMW) || self.has_type(T::stairsBW)
    }

    /// zombie.iso.IsoGridSquare.getStairsDirection
    pub fn get_stairs_direction(&self) -> Option<IsoDirections> {
        if self.has_stairs_north() {
            Some(IsoDirections::N)
        } else if self.has_stairs_west() {
            Some(IsoDirections::W)
        } else {
            None
        }
    }

    /// zombie.iso.IsoGridSquare.getStairsHeightMax
    pub fn get_stairs_height_max(&self) -> f32 {
        if self.has_type(T::stairsTN) || self.has_type(T::stairsTW) {
            1.0
        } else if self.has_type(T::stairsMN) || self.has_type(T::stairsMW) {
            0.66
        } else if !self.has_type(T::stairsMN) && !self.has_type(T::stairsMW) {
            0.0
        } else {
            0.33
        }
    }

    /// zombie.iso.IsoGridSquare.getStairsHeightMin
    pub fn get_stairs_height_min(&self) -> f32 {
        if self.has_type(T::stairsTN) || self.has_type(T::stairsTW) {
            0.66
        } else if !self.has_type(T::stairsMN) && !self.has_type(T::stairsMW) {
            0.0
        } else {
            0.33
        }
    }

    /// zombie.iso.IsoGridSquare.getStairsHeight (IsoDirections)
    pub fn get_stairs_height(&self, edge: IsoDirections) -> f32 {
        match self.get_stairs_direction() {
            None => 0.0,
            Some(d) if d == edge => self.get_stairs_height_max(),
            Some(d) if d.rot180() == edge => self.get_stairs_height_min(),
            Some(_) => -1.0,
        }
    }

    /// zombie.iso.IsoGridSquare.isStairsEdgeBlocked: the stairs' height on that edge differs from the neighbour's on
    /// its opposite edge, or there is no neighbour (`getAdjacentSquare`).
    pub fn is_stairs_edge_blocked(&self, g: &dyn GetSquare, edge: IsoDirections) -> bool {
        if self.get_stairs_direction().is_none() {
            return false;
        }
        match g.get_grid_square(self.x + edge.dx(), self.y + edge.dy(), self.z) {
            None => true,
            Some(sq2) => self.get_stairs_height(edge) != sq2.get_stairs_height(edge.rot180()),
        }
    }

    /// zombie.iso.IsoGridSquare.hasSlopedSurface
    pub fn has_sloped_surface(&self, alias: &TilePropertyAliasMap) -> bool {
        self.get_sloped_surface_direction(alias).is_some()
    }

    /// zombie.iso.IsoGridSquare.getSlopedSurfaceHeightMin
    pub fn get_sloped_surface_height_min(&self, alias: &TilePropertyAliasMap) -> f32 {
        self.properties.get_sloped_surface_height_min(alias) as f32 / 100.0
    }

    /// zombie.iso.IsoGridSquare.getSlopedSurfaceHeightMax
    pub fn get_sloped_surface_height_max(&self, alias: &TilePropertyAliasMap) -> f32 {
        self.properties.get_sloped_surface_height_max(alias) as f32 / 100.0
    }

    /// zombie.iso.IsoGridSquare.hasIdenticalSlopedSurface
    pub fn has_identical_sloped_surface(
        &self,
        other: &IsoGridSquare,
        alias: &TilePropertyAliasMap,
    ) -> bool {
        self.get_sloped_surface_direction(alias) == other.get_sloped_surface_direction(alias)
            && self.get_sloped_surface_height_min(alias)
                == other.get_sloped_surface_height_min(alias)
            && self.get_sloped_surface_height_max(alias)
                == other.get_sloped_surface_height_max(alias)
    }

    /// zombie.iso.IsoGridSquare.getWindowFrame (GridSquareEdgeFacingDirection): an `IsoWindowFrame` on that edge.
    fn has_window_frame(&self, north: bool) -> bool {
        self.objects
            .iter()
            .any(|o| matches!(&o.kind, ObjectKind::IsoWindowFrame { north: n } if *n == north))
    }

    /// zombie.iso.IsoGridSquare.getHoppableOrWindowFrame (GridSquareEdgeFacingDirection): an object whose hoppable or
    /// window-frame direction is that edge's (`hoppable_edges`).
    fn has_hoppable_or_window_frame(&self, north: bool) -> bool {
        self.hoppable_edges & if north { 1 } else { 2 } != 0
    }

    /// The four calls of `isAdjacentToEdgeElement`: the element on the north and west edge of this square, and on
    /// the north edge of the square to the south and the west edge of the square to the east (`getEdgeElement`).
    fn any_edge(&self, g: &dyn GetSquare, getter: &dyn Fn(&IsoGridSquare, bool) -> bool) -> bool {
        getter(self, true)
            || getter(self, false)
            || g.get_grid_square(self.x + 1, self.y, self.z)
                .is_some_and(|e| getter(e, false))
            || g.get_grid_square(self.x, self.y + 1, self.z)
                .is_some_and(|s| getter(s, true))
    }

    /// zombie.iso.IsoGridSquare.isAdjacentToWindow: a window, a thumpable window or a window frame on one of the four
    /// edges.
    pub fn is_adjacent_to_window(&self, g: &dyn GetSquare) -> bool {
        self.any_edge(g, &|s, north| {
            s.get_window(north).is_some() || s.has_window_frame(north)
        })
    }

    /// zombie.iso.IsoGridSquare.isAdjacentToHoppable: a hoppable object, a window frame or a hoppable thumpable on
    /// one of the four edges.
    pub fn is_adjacent_to_hoppable(&self, g: &dyn GetSquare) -> bool {
        self.any_edge(g, &|s, north| s.has_hoppable_or_window_frame(north))
    }
}
