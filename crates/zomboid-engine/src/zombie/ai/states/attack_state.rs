//! zombie.ai.states.AttackState
use crate::zombie::characters::action::action_context::ActionContext;
use crate::zombie::characters::iso_zombie::IsoZombie;
use crate::zombie::characters::iso_zombie_sp::SpRead;
use crate::zombie::core::skinnedmodel::advancedanimation::anim_event::AnimEvent;
use crate::zombie::iso::vector2::Vector2;

impl IsoZombie {
    /// zombie.ai.states.AttackState.enter: `AttackOutcome` "start", `AttackDidDamage` and `ZombieBiteDone` cleared.
    pub(crate) fn attack_enter(&mut self) -> Result<(), String> {
        self.sp.attack_outcome = "start".to_string();
        self.sp_clear_variable("AttackDidDamage")?;
        self.sp_clear_variable("ZombieBiteDone")
    }

    /// zombie.ai.states.AttackState.execute for a player target on its feet, alive and with no hit reaction variables
    /// (the stand-in has none). `Err` for the branches that need the player's defence (`PlayerHitReaction` set) or a
    /// sprinter's slow factor on the player.
    pub(crate) fn attack_execute(&mut self, rd: &SpRead) -> Result<(), String> {
        // `targetChar.getVariableString("ZombieHitReaction")` is "" for the stand-in, never "Chainsaw"
        let (b_attack, did_damage, hit_reaction, player_hit_reaction) = {
            let ctx = self.anim.as_ref().map(|a| &a.action_context);
            (
                self.sp_variable_boolean(ctx, "bAttack", rd),
                self.sp_variable_boolean(ctx, "AttackDidDamage", rd),
                self.sp_variable_string(ctx, "targethitreaction", rd),
                self.sp_variable_string(ctx, "PlayerHitReaction", rd),
            )
        };
        let mut outcome = self.sp.attack_outcome.clone();
        if outcome == "success"
            && b_attack
            && hit_reaction
                .as_deref()
                .is_some_and(|h| crate::java::equals_ignore_case(h, "EndDeath"))
        {
            outcome = "enddeath".to_string();
            self.sp.attack_outcome = outcome.clone();
        }
        // `getVariableString("ZombieBiteDone") == null`: never null (the original returns "" without a slot)
        let _ = did_damage;
        if !self.sp.target {
            self.sp.target_seen_time = 10.0;
        }
        if self.sp.target
            && outcome != "started"
            && player_hit_reaction.as_deref().is_some_and(|s| !s.is_empty())
        {
            return Err("AttackState.execute: the target's defence test is not ported".into());
        }
        // setShootable(true)
        if self.sp.target && !self.crawling {
            if outcome != "fail" && outcome != "interrupted" {
                self.face_this_object_player(rd)?;
            }
            self.sp_set_bool("bOnFloor", false)?;
        }
        if self.sp.target && self.speed_type == 1 && (outcome == "start" || outcome == "success") {
            return Err("AttackState.execute: a sprinter's slow factor on the target is not ported".into());
        }
        // target.setTimeSinceZombieAttack(0), setLastTargettedBy(this): the stand-in's fields, not read
        if !self.crawling {
            self.sp_set_string("AttackType", "bite")?;
        }
        Ok(())
    }

    /// IsoGameCharacter.faceThisObject(player) = faceThisObjectAlt: the direction to the player's position when it is
    /// not on the zombie, then the animation player's `updateForwardDirection` (the player is made if there is none).
    pub(crate) fn face_this_object_player(&mut self, rd: &SpRead) -> Result<(), String> {
        let (mx, my) = (self.character.moving.x, self.character.moving.y);
        let tempo = Vector2::new(rd.world.player.x - mx, rd.world.player.y - my);
        if tempo.get_length() > 0.0 {
            self.character.set_forward_direction(tempo.x, tempo.y)?;
        }
        self.sp_ensure_anim_player();
        self.sp_update_player_forward();
        Ok(())
    }

    /// zombie.ai.states.AttackState.exit: `AttackOutcome`, `AttackType`, `PlayerHitReaction` cleared, the state machine
    /// unlocked, `allowRepathDelay` 0 (a target on the floor would become the body to eat: not reached).
    pub(crate) fn attack_exit(&mut self) -> Result<(), String> {
        self.sp_clear_variable("AttackOutcome")?;
        self.sp_clear_variable("AttackType")?;
        self.sp_clear_variable("PlayerHitReaction")?;
        self.allow_repath_delay = 0.0;
        Ok(())
    }

    /// zombie.ai.states.AttackState.animEvent: `SetAttackOutcome` sets `success` or `fail` from `bAttack`; the events
    /// that bite or eat (`AttackCollisionCheck` with a target, `EatBody`) are not ported (`Err`).
    pub(crate) fn attack_anim_event(
        &mut self,
        ctx: &ActionContext,
        rd: &SpRead,
        event: &AnimEvent,
    ) -> Result<(), String> {
        let Some(name) = event.event_name.as_deref() else {
            return Ok(());
        };
        if crate::java::equals_ignore_case(name, "SetAttackOutcome") {
            self.sp.attack_outcome = if self.sp_variable_boolean(Some(ctx), "bAttack", rd) {
                "success"
            } else {
                "fail"
            }
            .to_string();
        }
        if crate::java::equals_ignore_case(name, "AttackCollisionCheck") && self.sp.target {
            return Err("AttackState.animEvent: AttackCollisionCheck (the bite) is not ported".into());
        }
        if crate::java::equals_ignore_case(name, "EatBody") {
            return Err("AttackState.animEvent: EatBody is not ported".into());
        }
        // "SetState": the audio parameter ParameterZombieState.Attack, not modelled
        Ok(())
    }
}
