//! zombie.ai.states.ZombieFaceTargetState
use crate::zombie::characters::iso_zombie::IsoZombie;
use crate::zombie::characters::iso_zombie_sp::SpRead;

impl IsoZombie {
    /// zombie.ai.states.ZombieFaceTargetState.execute: with a target, `faceThisObject(target)` (the player is an
    /// IsoGameCharacter: `faceThisObjectAlt`). `enter` and `exit` do nothing.
    pub(crate) fn face_target_execute(&mut self, rd: &SpRead) -> Result<(), String> {
        if self.sp.target {
            self.face_this_object_player(rd)?;
        }
        Ok(())
    }
}
