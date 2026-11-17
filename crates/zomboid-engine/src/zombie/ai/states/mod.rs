//! Package `zombie.ai.states`: the states a single-player zombie passes through with a target or a wander goal.
//! `ZombieIdleState` lives with the zombie (`characters::iso_zombie`, extended for the single-player game).
//! NOT PORTED (the headless oracle does not reach them, see `docs/COMPATIBILITY.md`): `WalkTowardState`,
//! `StaggerBackState`, `ZombieFallDownState`, `ZombieOnGroundState`, `ZombieGetUpState`, `ZombieEatBodyState`,
//! `ThumpState`, `ClimbOverFenceState` and the rest.
pub mod attack_state;
pub mod lunge_state;
pub mod path_find_state;
