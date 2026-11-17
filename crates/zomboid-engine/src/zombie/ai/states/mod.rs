//! Package `zombie.ai.states`: the states a single-player zombie passes through with a target or a wander goal.
//! `ZombieIdleState` lives with the zombie (`characters::iso_zombie`, extended for the single-player game).
//! Ported and proven by `sh tools/oracle.sh zombieai`: `PathFindState`, `LungeState`, `AttackState`,
//! `ZombieFaceTargetState`, `WalkTowardState`.
//! NOT PORTED (the headless oracle reaches `StaggerBackState` only through a hit that needs `IsoZombie.Hit`; the rest
//! no scenario reached, see `docs/COMPATIBILITY.md`): `StaggerBackState`, `ZombieFallDownState`, `ZombieOnGroundState`,
//! `ZombieGetUpState`, `ZombieEatBodyState`, `ThumpState`, `ClimbOverFenceState` and the rest.
pub mod attack_state;
pub mod lunge_state;
pub mod path_find_state;
pub mod walk_toward_state;
pub mod zombie_face_target_state;
