//! The single-player zombie's movement: the animation's deferred (root) movement taken by `IsoGameCharacter.
//! doDeferredMovement` and `IsoZombie.moveUnmodded`, and `IsoMovingObject.postupdate`, which turns the requested next
//! position into the position (collision with the squares' walls, `getGlobalMovementMod`, stairs and slopes, the
//! current square).
//!
//! Measured assumptions of the headless world (oracle `zombieai`): no vehicle exists (`PolygonalMap2.resolveCollision`
//! builds its obstacle polygons from squares only when a vehicle polygon is already there, so it returns the position it
//! is given), no door or window of the scenarios' squares is in reach, the player stand-in does not move.
//!
//! NOT PORTED (UNKNOWN, each an `Err`): a collision that stops the zombie (the thump, hop and bend checks of
//! `checkBreakHoppable`/`checkHitHoppable`/`checkBreakBendableFence`, `Collided`'s Lua event), stairs and sloped
//! surfaces, `virtualizeZombie`, a zombie without a square.
use super::iso_zombie::{IsoZombie, ZombieState, ZombieWorld};
use super::iso_zombie_sp::{SpRead, Sq};
use crate::zombie::core::math::pz_math;
use crate::zombie::iso::iso_grid_square::IsoGridSquare;
use crate::zombie::iso::vector2::Vector2;

/// IsoChunkMap.chunkGridWidth (the static default; the dedicated server never calls `CalcChunkWidth`) and the chunk
/// map's centre in the harness (`cm.worldX = 1035`, `cm.worldY = 1458`).
const CHUNK_GRID_WIDTH: i32 = 13;
const CHUNK_MAP_WORLD_X: i32 = 1035;
const CHUNK_MAP_WORLD_Y: i32 = 1458;

type Pos = (i32, i32, i32);

fn pos_of(sq: &IsoGridSquare) -> Pos {
    (sq.x, sq.y, sq.z)
}

impl IsoZombie {
    /// IsoCell.getGridSquare(x, y, z) (single-player: the squares of the loaded chunks).
    fn move_square<'a>(rd: &'a SpRead, p: Pos) -> Option<&'a IsoGridSquare> {
        rd.squares.get_grid_square(p.0, p.1, p.2)
    }

    fn move_current<'a>(&self, rd: &'a SpRead) -> Option<&'a IsoGridSquare> {
        self.sp.current.and_then(|p| Self::move_square(rd, p))
    }

    /// IsoMovingObject.setCurrentSquareFromPosition(x1, y1, z1): the square at the position, else the first square
    /// below it down to level 0.
    fn set_current_square_from_position(&mut self, rd: &SpRead, x1: f32, y1: f32, z1: f32) {
        let xi = pz_math::fastfloor(x1);
        let yi = pz_math::fastfloor(y1);
        let mut cur = rd.squares.get_grid_square(xi, yi, pz_math::fastfloor(z1));
        if cur.is_none() {
            let mut n = pz_math::fastfloor(z1);
            while n >= 0 {
                cur = rd.squares.get_grid_square(xi, yi, n);
                if cur.is_some() {
                    break;
                }
                n -= 1;
            }
        }
        self.sp.current = cur.map(pos_of);
    }

    /// IsoMovingObject.findCurrentGridSquare: the square at the position, else the first one below it.
    fn find_current_grid_square(&self, rd: &SpRead) -> Option<Pos> {
        let xi = pz_math::fastfloor(self.character.moving.x);
        let yi = pz_math::fastfloor(self.character.moving.y);
        let mut zi = pz_math::fastfloor(self.character.moving.z);
        let z_sign = pz_math::sign(zi as f32);
        loop {
            let found = rd.squares.get_grid_square(xi, yi, zi);
            zi -= z_sign;
            if found.is_some() || zi == -z_sign {
                return found.map(pos_of);
            }
        }
    }

    /// IsoMovingObject.getFeelerTile(dist): the square `dist` ahead of the zombie along (next - last).
    fn get_feeler_tile(&self, rd: &SpRead, dist: f32) -> Option<Pos> {
        let mut vec = Vector2::new(
            self.sp.next_x - self.sp.last_x,
            self.sp.next_y - self.sp.last_y,
        );
        vec.set_length(dist);
        let m = &self.character.moving;
        rd.squares
            .get_grid_square(
                pz_math::fastfloor(m.x + vec.x),
                pz_math::fastfloor(m.y + vec.y),
                pz_math::fastfloor(m.z),
            )
            .map(pos_of)
    }

    /// IsoGridSquare.testCollideAdjacent(zombie, x, y, z) for a zombie (no `IsoPlayer`, not a server or a client): the
    /// special objects (doors, windows) of the squares, then the square's collide matrix. `Err` for a special object
    /// (`collideWith`, a Lua event, and `setCollidedObject`) and for a square without its neighbour in the chunk map's
    /// world.
    fn test_collide_adjacent(
        &self,
        rd: &SpRead,
        from: Pos,
        x: i32,
        y: i32,
        z: i32,
    ) -> Result<bool, String> {
        let Some(from_sq) = Self::move_square(rd, from) else {
            return Err("IsoGridSquare.testCollideAdjacent: the square is not loaded".into());
        };
        if !((-1..=1).contains(&x) && (-1..=1).contains(&y) && (-1..=1).contains(&z)) {
            return Ok(true);
        }
        // IsoMetaGrid.isValidChunk: inside the map (Muldraugh's cells are far from the edge of the world)
        let target = rd
            .squares
            .get_grid_square(from_sq.x + x, from_sq.y + y, from_sq.z + z);
        // shouldIgnoreCollisionWithSquare: `getSitOnFurnitureObject() == null`, `hasPath()` false
        if let Some(t) = target {
            let g = Sq(rd.squares);
            if from_sq
                .test_collide_special_objects(&rd.world.alias, Some(t), &g)
                .is_some()
            {
                return Err(
                    "IsoGridSquare.testCollideAdjacent: a special object (door, window) is in the way (not ported)"
                        .into(),
                );
            }
        }
        let g = Sq(rd.squares);
        let (collide, _, _) = from_sq.calculate_matrices(&rd.world.alias, &g);
        Ok(collide & (1 << ((x + 1) + (y + 1) * 3 + (z + 1) * 9)) != 0)
    }

    /// IsoZombie.shouldSlideHeadAwayFromWalls
    fn should_slide_head_away_from_walls(&self) -> bool {
        let idle_or_staggering = self.current_state == Some(ZombieState::Idle);
        if (!self.on_floor && !self.sp.knocked_down) && !idle_or_staggering {
            return false;
        }
        // !isCrawling() || path2 == null && !isMoving(): then not climbing a fence or a window (no such state yet)
        !self.crawling || !self.sp.is_moving
    }

    /// IsoMovingObject.slideAwayToCollisionPos(collNewPosX, collNewPosY, instant)
    fn slide_away_to_collision_pos(
        &mut self,
        coll_new_pos_x: f32,
        coll_new_pos_y: f32,
        instant: bool,
    ) {
        let x1 = self.character.moving.x;
        let y1 = self.character.moving.y;
        let original_x2 = self.sp.next_x;
        let original_y2 = self.sp.next_y;
        let original_vx = original_x2 - x1;
        let original_vy = original_y2 - y1;
        let original_vx_sign = pz_math::sign(original_vx);
        let original_vy_sign = pz_math::sign(original_vy);
        let mut new_pos_x = coll_new_pos_x;
        let mut new_pos_y = coll_new_pos_y;
        let collided_vx = coll_new_pos_x - x1;
        let collided_vy = coll_new_pos_y - y1;
        let collided_vx_sign = pz_math::sign(collided_vx);
        let collided_vy_sign = pz_math::sign(collided_vy);
        if original_vx_sign == collided_vx_sign {
            let original_vx_abs = original_vx * original_vx_sign as f32;
            let collided_vx_abs = collided_vx * collided_vx_sign as f32;
            new_pos_x =
                x1 + pz_math::max(original_vx_abs, collided_vx_abs) * original_vx_sign as f32;
        }
        if original_vy_sign == collided_vy_sign {
            let original_vy_abs = original_vy * original_vy_sign as f32;
            let collided_vy_abs = collided_vy * collided_vy_sign as f32;
            new_pos_y =
                y1 + pz_math::max(original_vy_abs, collided_vy_abs) * original_vy_sign as f32;
        }
        self.sp.next_x = new_pos_x;
        self.sp.next_y = new_pos_y;
        if instant {
            self.set_x(new_pos_x);
            self.set_y(new_pos_y);
        }
    }

    /// IsoMovingObject.setX: `x`, and `nx` with it.
    fn set_x(&mut self, x: f32) {
        self.character.moving.x = x;
        self.sp.next_x = x;
    }

    /// IsoMovingObject.setY
    fn set_y(&mut self, y: f32) {
        self.character.moving.y = y;
        self.sp.next_y = y;
    }

    /// IsoMovingObject.setZ: clamped to -32..31, `lz` follows.
    fn set_z(&mut self, z: f32) {
        let z = f32::min(31.0, f32::max(-32.0, z));
        self.character.moving.z = z;
        self.sp.last_z = z;
    }

    /// IsoGameCharacter.resolveCollisionWithNeighboringSquares(inRadius, newPos): whether the next position is past a
    /// wall of the square the zombie stands on, and where it is pushed to.
    fn resolve_collision_with_neighboring_squares(
        &self,
        rd: &SpRead,
        in_radius: f32,
    ) -> Result<Option<(f32, f32)>, String> {
        let m = &self.character.moving;
        let (current_x, current_y, current_z) = (m.x, m.y, m.z);
        let (next_x, next_y) = (self.sp.next_x, self.sp.next_y);
        let current_square_x = pz_math::fastfloor(current_x);
        let current_square_y = pz_math::fastfloor(current_y);
        let current_square_z = pz_math::fastfloor(current_z);
        let square_center_x = 0.5f32 + current_square_x as f32;
        let square_center_y = 0.5f32 + current_square_y as f32;
        let sqc_to_next_x = next_x - square_center_x;
        let sqc_to_next_y = next_y - square_center_y;
        let sqc_to_next_x_sign = pz_math::sign(sqc_to_next_x);
        let sqc_to_next_y_sign = pz_math::sign(sqc_to_next_y);
        let x2 = next_x + in_radius * sqc_to_next_x_sign as f32;
        let y2 = next_y + in_radius * sqc_to_next_y_sign as f32;
        let next_square_x = pz_math::fastfloor(x2);
        let next_square_y = pz_math::fastfloor(y2);
        if next_square_x == current_square_x && next_square_y == current_square_y {
            return Ok(None);
        }
        // getSquare() (current), else the square at the position
        let mut current_square = self.sp.current;
        if current_square.is_none_or(|c| c.0 != current_square_x || c.1 != current_square_y) {
            current_square = rd
                .squares
                .get_grid_square(current_square_x, current_square_y, current_square_z)
                .map(pos_of);
        }
        let Some(current_square) = current_square else {
            return Ok(None);
        };
        let mut collided = false;
        let (mut new_x, mut new_y) = (next_x, next_y);
        if sqc_to_next_x_sign != 0
            && self.test_collide_adjacent(rd, current_square, sqc_to_next_x_sign, 0, 0)?
        {
            collided = true;
            if sqc_to_next_x_sign == -1 {
                new_x = current_square_x as f32 + in_radius;
            } else if sqc_to_next_x_sign == 1 {
                new_x = current_square_x as f32 + 1.0f32 - in_radius;
            }
        }
        if sqc_to_next_y_sign != 0
            && self.test_collide_adjacent(rd, current_square, 0, sqc_to_next_y_sign, 0)?
        {
            collided = true;
            if sqc_to_next_y_sign == -1 {
                new_y = current_square_y as f32 + in_radius;
            } else if sqc_to_next_y_sign == 1 {
                new_y = current_square_y as f32 + 1.0f32 - in_radius;
            }
        }
        Ok(if collided { Some((new_x, new_y)) } else { None })
    }

    /// IsoZombie.slideHeadAwayFromWalls(instant): `slideAwayFromWalls(0.15F, instant, false)` for a zombie that is
    /// idle, staggering or on the floor (`DebugOptions.collideWithObstacles.debug.slideAwayFromWalls` defaults true).
    fn slide_head_away_from_walls(&mut self, rd: &SpRead, instant: bool) -> Result<(), String> {
        if self.sp.current.is_none() || !self.should_slide_head_away_from_walls() {
            return Ok(());
        }
        // includePolyCollisions false: only the neighbouring squares
        if let Some((nx, ny)) = self.resolve_collision_with_neighboring_squares(rd, 0.15)? {
            self.slide_away_to_collision_pos(nx, ny, instant);
        }
        Ok(())
    }

    /// IsoMovingObject.isInLoadedArea (single-player): the tiles of the player's chunk map.
    fn is_in_loaded_area(x: i32, y: i32) -> bool {
        let x_min = (CHUNK_MAP_WORLD_X - CHUNK_GRID_WIDTH / 2) * 8;
        let y_min = (CHUNK_MAP_WORLD_Y - CHUNK_GRID_WIDTH / 2) * 8;
        let width = CHUNK_GRID_WIDTH * 8;
        x >= x_min && x < x_min + width && y >= y_min && y < y_min + width
    }

    /// IsoMovingObject.DoCollide(favour): the zombie's next position against the squares' walls. Returns whether it
    /// collided (and sets the `collided*` flags and `current`). `Err` where the original would run a branch not ported.
    fn do_collide(&mut self, rd: &SpRead, favour: i32) -> Result<bool, String> {
        let (nx, ny, z) = (self.sp.next_x, self.sp.next_y, self.character.moving.z);
        self.set_current_square_from_position(rd, nx, ny, z);
        // chr.isRagdollSimulationActive(): no ragdoll controller exists
        if let Some(cur) = self.move_current(rd) {
            if !cur.treat_as_solid_floor(&rd.world.alias) {
                self.set_current_square_from_position(rd, nx, ny, z);
            }
            if self.sp.current.is_none() {
                return Ok(false);
            }
            self.set_current_square_from_position(rd, nx, ny, z);
        }
        if self.sp.current != self.sp.last && self.sp.last.is_some() && self.sp.current.is_some() {
            // `chr.getCurrentState().isIgnoreCollide(...)`: false for the ported states
            let (last, cur) = (
                self.sp.last.unwrap_or_default(),
                self.sp.current.unwrap_or_default(),
            );
            let (dx, dy, dz) = (cur.0 - last.0, cur.1 - last.1, cur.2 - last.2);
            let collide = self.test_collide_adjacent(rd, last, dx, dy, dz)?;
            if collide {
                return Err(
                    "IsoMovingObject.DoCollide: the zombie walks into a wall (the thump, hop and bend checks are not ported)"
                        .into(),
                );
            }
        } else if self.sp.next_x != self.sp.last_x || self.sp.next_y != self.sp.last_y {
            if self.sp.current.is_none() {
                return Err(
                    "IsoMovingObject.DoCollide: the zombie leaves the loaded squares (not ported)"
                        .into(),
                );
            }
            // chr.getPath2() == null (the headless path finder never finds a path), not sitting on furniture
            let feeler = self.get_feeler_tile(rd, 0.5);
            // chr.isClimbing(): false
            if let (Some(feeler), Some(cur)) = (feeler, self.sp.current) {
                if feeler != cur
                    && self.test_collide_adjacent(
                        rd,
                        cur,
                        feeler.0 - cur.0,
                        feeler.1 - cur.1,
                        feeler.2 - cur.2,
                    )?
                {
                    return Err(
                        "IsoMovingObject.DoCollide: the zombie walks into a wall (the thump, hop and bend checks are not ported)"
                            .into(),
                    );
                }
            }
        }
        let _ = favour;
        Ok(false)
    }

    /// IsoMovingObject.postupdate for the single-player zombie: slide away from walls, `ensureOnTile`, the next position
    /// (impulses are zero) limited to one tile, the collision passes, the movement modifier, stairs, slopes and the
    /// current square. `Err` where the original would leave the loaded area or take a branch not ported.
    pub(crate) fn moving_postupdate(
        &mut self,
        world: &ZombieWorld,
        rd: &SpRead,
    ) -> Result<(), String> {
        let _ = world;
        self.slide_head_away_from_walls(rd, true)?;
        // ensureOnTile: `current == null` and not an IsoPlayer: returns
        self.sp.time_since_zombie_attack += 1;
        // lastTargettedBy stays null
        self.sp.collided_this_frame = false;
        self.sp.collided_n = false;
        self.sp.collided_s = false;
        self.sp.collided_w = false;
        self.sp.collided_e = false;
        self.sp.last = self.sp.current;
        // nextX += impulsex (0): a float add of zero changes nothing but -0.0
        let m = self.character.moving.clone();
        let mut tempo = Vector2::new(self.sp.next_x - m.x, self.sp.next_y - m.y);
        if tempo.get_length() > 1.0 {
            tempo.normalize();
            self.sp.next_x = m.x + tempo.x;
            self.sp.next_y = m.y + tempo.y;
        }
        // `thisZombie == null || floor(z) != 0 || getCurrentBuilding() != null || isInLoadedArea(next) ||
        // !(PathFindState || WalkTowardState)`; else the zombie is virtualized
        let in_building = self.move_current(rd).is_some_and(|sq| sq.room.is_some());
        let path_or_walk = matches!(
            self.current_state,
            Some(ZombieState::PathFind) | Some(ZombieState::WalkToward)
        );
        if pz_math::fastfloor(m.z) == 0
            && !in_building
            && !Self::is_in_loaded_area(
                pz_math::fastfloor(self.sp.next_x),
                pz_math::fastfloor(self.sp.next_y),
            )
            && path_or_walk
        {
            return Err(
                "IsoMovingObject.postupdate: ZombiePopulationManager.virtualizeZombie (not ported)"
                    .into(),
            );
        }
        // collidedWithVehicle = false; PolygonalMap2.resolveCollision: no vehicle exists, the position is unchanged
        let onx = self.sp.next_x;
        let ony = self.sp.next_y;
        let mut len;
        let mut b_did_collide = false;
        // `collidable` is true
        let alt = self.sp.alt_collide;
        self.do_collide(rd, if alt { 2 } else { 1 })?;
        if self.sp.collided_n || self.sp.collided_s {
            self.sp.next_y = self.sp.last_y;
        }
        if self.sp.collided_w || self.sp.collided_e {
            self.sp.next_x = self.sp.last_x;
        }
        self.do_collide(rd, if alt { 1 } else { 2 })?;
        self.sp.alt_collide = !self.sp.alt_collide;
        if self.sp.collided_n || self.sp.collided_s {
            self.sp.next_y = self.sp.last_y;
            b_did_collide = true;
        }
        if self.sp.collided_w || self.sp.collided_e {
            self.sp.next_x = self.sp.last_x;
            b_did_collide = true;
        }
        len = (self.sp.next_x - self.sp.last_x).abs() + (self.sp.next_y - self.sp.last_y).abs();
        let _ = len;
        len = 0.0;
        let _ = len;
        let (lnx, lny) = (self.sp.next_x, self.sp.next_y);
        self.sp.next_x = onx;
        self.sp.next_y = ony;
        if b_did_collide {
            return Err(
                "IsoMovingObject.postupdate: the second collision pass after a collision is not ported".into(),
            );
        }
        let _ = (lnx, lny);
        if self.sp.collided_this_frame {
            self.sp.current = self.sp.last;
        }
        // checkHitWall: only an IsoPlayer reacts
        let mut dx = self.sp.next_x - self.character.moving.x;
        let mut dy = self.sp.next_y - self.character.moving.y;
        let movement_mod = if !(dx.abs() > 0.0) && !(dy.abs() > 0.0) {
            0.0
        } else {
            self.get_global_movement_mod(rd)
        };
        if dx.abs() > 0.01 || dy.abs() > 0.01 {
            dx *= movement_mod;
            dy *= movement_mod;
        }
        let new_x = self.character.moving.x + dx;
        self.set_x(new_x);
        let new_y = self.character.moving.y + dy;
        self.set_y(new_y);
        self.do_stairs(rd)?;
        self.handle_sloped_surface(rd)?;
        self.sp.current = self.find_current_grid_square(rd);
        self.snap_z_to_current_square(rd);
        // setMovingSquare(current): the moving-object lists matter to `separate`, not ported
        if self.sp.current.is_none() && self.sp.last.is_some() {
            self.sp.current = self.sp.last;
            if let Some(c) = self.sp.current {
                self.set_x(c.0 as f32 + 0.5);
                self.set_y(c.1 as f32 + 0.5);
                self.snap_z_to_current_square(rd);
            }
        }
        // ensureOnTile; `square = current`; `firstUpdate = false`
        Ok(())
    }

    /// IsoGameCharacter.getGlobalMovementMod -> IsoMovingObject.getGlobalMovementMod: 0.75 on stairs, else 1 (the
    /// vegetation's rustle effect and the tree noises change nothing a zombie reads).
    fn get_global_movement_mod(&self, rd: &SpRead) -> f32 {
        if self.move_current(rd).is_some_and(|c| c.has_stairs()) {
            0.75
        } else {
            1.0
        }
    }

    /// IsoMovingObject.doStairs: `Err` when the zombie is on or next to a staircase (not ported); else only `lz` follows
    /// `z`.
    fn do_stairs(&mut self, rd: &SpRead) -> Result<(), String> {
        let (Some(cur), Some(last)) = (
            self.move_current(rd),
            self.sp.last.and_then(|p| Self::move_square(rd, p)),
        ) else {
            return Ok(());
        };
        use crate::zombie::iso::sprite_details::iso_object_type::IsoObjectType as T;
        let stairs = |s: &IsoGridSquare| {
            s.has_type(T::stairsTN)
                || s.has_type(T::stairsTW)
                || s.has_type(T::stairsMN)
                || s.has_type(T::stairsMW)
                || s.has_type(T::stairsBN)
                || s.has_type(T::stairsBW)
        };
        if stairs(cur) || stairs(last) || cur.has_stairs() {
            return Err("IsoMovingObject.doStairs: a staircase is in reach (not ported)".into());
        }
        // `if (Math.abs(z - this.getZ()) < 0.95F) this.setZ(z)` with z == getZ()
        let z = self.character.moving.z;
        self.set_z(z);
        Ok(())
    }

    /// IsoMovingObject.handleSlopedSurface: `Err` for a sloped surface under or behind the zombie (not ported).
    fn handle_sloped_surface(&mut self, rd: &SpRead) -> Result<(), String> {
        let Some(cur) = self.move_current(rd) else {
            return Ok(());
        };
        let last = self.sp.last.and_then(|p| Self::move_square(rd, p));
        if cur.get_sloped_surface_direction(&rd.world.alias).is_some()
            || last.is_some_and(|l| l.get_sloped_surface_direction(&rd.world.alias).is_some())
        {
            return Err(
                "IsoMovingObject.handleSlopedSurface: a sloped surface (not ported)".into(),
            );
        }
        Ok(())
    }

    /// IsoMovingObject.snapZToCurrentSquare: `setZ(setLastZ(min(z, current.z + 0.99999F)))`.
    fn snap_z_to_current_square(&mut self, rd: &SpRead) {
        if let Some(cur) = self.move_current(rd) {
            let z = pz_math::min(self.character.moving.z, cur.z as f32 + 0.99999f32);
            self.sp.last_z = z;
            self.set_z(z);
        }
    }

    /// IsoZombie.moveUnmodded(dirX, dirY): a sprinter in a lunging, attacking or staggering state moves no further than
    /// to its target (`dist` less both widths plus 0.1), then `IsoMovingObject.moveUnmodded` adds the move to the next
    /// position.
    pub(crate) fn move_unmodded(&mut self, mut dir_x: f32, mut dir_y: f32, rd: &SpRead) {
        let lunging_or_attacking = matches!(
            self.current_state,
            Some(ZombieState::Lunge) | Some(ZombieState::Attack)
        );
        if self.speed_type == 1 && lunging_or_attacking && self.sp.target {
            let dx = rd.world.player.x - self.character.moving.x;
            let dy = rd.world.player.y - self.character.moving.y;
            let mut dist = f64::from(dx * dx + dy * dy).sqrt() as f32;
            dist -= self.character.moving.width + rd.world.player.width - 0.1f32;
            dist = pz_math::max(0.0, dist);
            let dir_length = f64::from(dir_x * dir_x + dir_y * dir_y).sqrt() as f32;
            if dir_length > dist {
                dir_x = dir_x * dist / dir_length;
                dir_y = dir_y * dist / dir_length;
            }
        }
        self.sp.next_x += dir_x;
        self.sp.next_y += dir_y;
    }

    /// IsoGameCharacter.doDeferredMovement for the single-player zombie on an animation frame: the movement the animation
    /// player accumulated, kept from running past a walk target, goes to `moveUnmodded`.
    pub(crate) fn do_deferred_movement(
        &mut self,
        animation_updating_this_frame: bool,
        rd: &SpRead,
    ) {
        if !animation_updating_this_frame {
            return;
        }
        let Some(player) = self.sp.anim_player.as_mut() else {
            return;
        };
        let mut d = player.get_deferred_movement(true);
        // `getPath2() != null` never holds headless (no path is found); not climbing
        if self.current_state == Some(ZombieState::WalkToward) {
            let mut target_vec = Vector2::new(
                self.path.target_x - self.character.moving.x,
                self.path.target_y - self.character.moving.y,
            );
            if target_vec.get_length_squared() < d.get_length_squared() {
                d.set_length(target_vec.get_length());
            }
            let _ = &mut target_vec;
        }
        if self.sp.deferred_movement_enabled {
            self.move_unmodded(d.x, d.y, rd);
        }
    }
}
