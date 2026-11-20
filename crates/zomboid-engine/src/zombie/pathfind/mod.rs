//! Package `zombie.pathfind`: the geometry helpers the world's zones use, the part of `PathFindBehavior2` a zombie's
//! `pathToLocation` writes and the squares-only part of `LineClearCollideMain`.
pub mod a_star;
pub mod highlevel;
pub mod liang_barsky;
pub mod line_clear_collide;
pub mod line_clear_collide_main;
pub mod path;
pub mod path_find_behavior2;
pub mod path_map;
pub mod polygonal_map2;
pub mod square_edges;
pub mod vga_star;
