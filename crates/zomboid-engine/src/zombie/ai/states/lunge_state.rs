//! zombie.ai.states.LungeState
use crate::zombie::characters::iso_zombie::{IsoZombie, ZombieWorld};
use crate::zombie::characters::iso_zombie_sp::SpRead;
use crate::zombie::iso::vector2::Vector2;

impl IsoZombie {
    /// zombie.ai.states.LungeState.enter: the lunge sound (wall-clock gated, no state change) and `lungeTimer = 180`.
    pub(crate) fn lunge_enter(&mut self) {
        self.sp.lunge_timer = 180.0;
        self.sp.lunge_tick_count = 0;
    }

    /// zombie.ai.states.LungeState.execute: on its feet and shootable, the lunger's walk variant, the lunge timer down by
    /// `getThirtyFPSMultiplier()`, the zombie turned to its target. `Err` for the branch that paths to the last seen
    /// position (not ported).
    pub(crate) fn lunge_execute(&mut self, world: &ZombieWorld, rd: &SpRead) -> Result<(), String> {
        self.sp_set_bool("bOnFloor", false)?;
        // setShootable(true): not read by anything modelled
        if self.lunger {
            self.sp.walk_variant_use = Some("ZombieWalk3".to_string());
        }
        self.sp.lunge_timer -= world.thirty_fps_multiplier();
        // (the player is not in ghost mode)
        if self.sp.lunge_timer < 0.0 {
            self.sp.lunge_timer = 0.0;
        }
        if self.sp.lunge_timer <= 0.0 {
            self.allow_repath_delay = 0.0;
        }
        let mut temp = Vector2::new(self.sp.vector_to_target.x, self.sp.vector_to_target.y);
        // getZombieLungeSpeed
        self.running = world.lore.speed == 1 && !self.inactive || self.speed_type == 1;
        temp.normalize();
        self.character.set_forward_direction(temp.x, temp.y)?;
        if !self.is_target_location_known() && self.sp.last_target_seen_x != -1 {
            return Err(
                "LungeState.execute: pathing to the last seen position is not ported".into(),
            );
        }
        let _ = rd;
        self.sp.lunge_tick_count += 1;
        Ok(())
    }
}
