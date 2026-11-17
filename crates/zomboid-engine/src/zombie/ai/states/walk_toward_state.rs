//! zombie.ai.states.WalkTowardState
use crate::zombie::characters::iso_zombie::{IsoZombie, ZombieWorld};
use crate::zombie::characters::iso_zombie_sp::SpRead;
use crate::zombie::core::math::pz_math;
use crate::zombie::iso::vector2::Vector2;

/// IsoUtils.DistanceTo(fromX, fromY, toX, toY): `(float) Math.sqrt(Math.pow(dx, 2.0) + Math.pow(dy, 2.0))`.
fn distance_to(from_x: f32, from_y: f32, to_x: f32, to_y: f32) -> f32 {
    let dx = f64::from(to_x - from_x);
    let dy = f64::from(to_y - from_y);
    (dx * dx + dy * dy).sqrt() as f32
}

impl IsoZombie {
    /// zombie.ai.states.WalkTowardState.enter: the state's params (`IGNORE_OFFSET` false the first time; one that is
    /// true compares against the wall clock, `Err`), `TICK_COUNT` 0, `walkingOnTheSpot.reset(x, y)`. (`isUseless()` is
    /// false for a zombie of this port; `getNetworkCharacterAI().extraUpdate()` is the network sync, UNKNOWN.)
    pub(crate) fn walk_toward_enter(&mut self) -> Result<(), String> {
        match self.sp.walk_toward_ignore_offset {
            None => self.sp.walk_toward_ignore_offset = Some(false),
            Some(true) => {
                return Err(
                    "WalkTowardState.enter: IGNORE_OFFSET is true and its age is the wall clock (not ported)"
                        .into(),
                )
            }
            Some(false) => {}
        }
        self.sp.walk_toward_tick_count = 0;
        let (x, y) = (self.character.moving.x, self.character.moving.y);
        self.path.walking_on_the_spot.reset(x, y);
        Ok(())
    }

    /// zombie.ai.states.WalkTowardState.execute for a zombie that has no target or a target whose location is not
    /// known (`pathToCharacter` and the vehicle branches are not ported: `Err`). The zombie never collides here: its
    /// movement is not ported (UNKNOWN), so `isCollidedThisFrame()` and `isCollidedWithVehicle()` are false.
    pub(crate) fn walk_toward_execute(
        &mut self,
        world: &ZombieWorld,
        rd: &SpRead,
    ) -> Result<(), String> {
        if !self.crawling {
            self.sp_set_bool("bOnFloor", false)?;
        }
        if self.sp.target {
            if self.is_target_location_known() {
                return Err("WalkTowardState.execute: pathToCharacter is not ported".into());
            } else if self.sp.last_target_seen_x != -1 {
                return Err(
                    "WalkTowardState.execute: pathing to the last seen location is not ported"
                        .into(),
                );
            }
        }
        let (mx, my, mz) = (
            self.character.moving.x,
            self.character.moving.y,
            self.character.moving.z,
        );
        // owner.getPathTargetX() == fastfloor(x) && owner.getPathTargetY() == fastfloor(y)
        if pz_math::fastfloor(self.path.target_x) == pz_math::fastfloor(mx)
            && pz_math::fastfloor(self.path.target_y) == pz_math::fastfloor(my)
        {
            if !self.sp.target {
                self.sp_set_bool("bPathfind", false)?;
                self.sp_set_bool("bMoving", false)?;
                return Ok(());
            }
            // the target's z is the player stand-in's
            if pz_math::fastfloor(rd.world.player.z) != pz_math::fastfloor(mz) {
                self.sp_set_bool("bPathfind", true)?;
                self.sp_set_bool("bMoving", false)?;
                return Ok(());
            }
        }
        let target_x = self.path.target_x;
        let target_y = self.path.target_y;
        let mut temp = Vector2::new(target_x - mx, target_y - my);
        let dist = temp.get_length();
        if dist < 0.25f32 {
            // setX/setY/setNextX/setNextY(target): the movement's next position is not modelled
            return Err(
                "WalkTowardState.execute: arriving within 0.25 of the target (setNextX/setNextY are not modelled)"
                    .into(),
            );
        }
        if dist < 0.025f32 {
            self.sp_set_bool("bPathfind", false)?;
            self.sp_set_bool("bMoving", false)?;
        } else {
            // !GameServer.server && !crawling && IGNORE_OFFSET == FALSE
            if !self.crawling && self.sp.walk_toward_ignore_offset == Some(false) {
                let dist_scale = (dist / 2.0f32).min(4.0f32);
                // (zomb.getID() + zomb.zombieId) % 20 / 10.0F - 1.0F
                let x =
                    ((rd.world.zombie_object_id + self.zombie_id) % 20) as f32 / 10.0f32 - 1.0f32;
                let y = x;
                let (px, py) = (self.character.moving.x, self.character.moving.y);
                if distance_to(px, py, target_x + x * dist_scale, target_y + y * dist_scale) < dist
                {
                    temp.x = target_x + x * dist_scale - px;
                    temp.y = target_y + y * dist_scale - py;
                }
            }
            self.running = false;
            temp.normalize();
            if self.crawling {
                return Err(
                    "WalkTowardState.execute: a crawler's turn direction is not ported".into(),
                );
            }
            // setDir(IsoDirections.fromAngle(temp)), then setForwardDirection(temp)
            self.character.set_forward_direction(temp.x, temp.y)?;
            let (px, py) = (self.character.moving.x, self.character.moving.y);
            if !self.sp.is_turning
                && self.path.walking_on_the_spot.check(
                    px,
                    py,
                    world.multiplier,
                    world.thirty_fps_multiplier(),
                )
            {
                self.sp_set_bool("bMoving", false)?;
            }
            // (tick 2: the zombie state audio parameter, no game state)
            self.sp.walk_toward_tick_count += 1;
        }
        Ok(())
    }

    /// zombie.ai.states.WalkTowardState.exit: `bMoving` false (`extraUpdate()` is the network sync, UNKNOWN).
    pub(crate) fn walk_toward_exit(&mut self) -> Result<(), String> {
        self.sp_set_bool("bMoving", false)
    }
}
