//! The AI states a zombie's action states map to: `IsoZombie.initializeStates` (the non-crawler branch) registers a
//! `State` for each lower-cased action state name; a name that is not registered gets the machine's default state,
//! `ZombieIdleState` (`StateMachineComponent.actionStateChanged`: `tryGetAIState(name)`, else `defaultState`).
use super::iso_zombie::ZombieState;

/// `registerAIState(name, State.instance())` of `IsoZombie.initializeStates` for a zombie that does not crawl:
/// (lower-case action state name, class name of the state).
pub const REGISTERED_AI_STATES: [(&str, &str); 88] = [
    ("idle", "ZombieIdleState"),
    ("attack-network", "AttackNetworkState"),
    ("attackvehicle-network", "GenericDefaultState"),
    ("fakedead-attack-network", "GenericDefaultState"),
    ("lunge-network", "LungeNetworkState"),
    ("walktoward-network", "WalkTowardNetworkState"),
    (
        "corpsethrownoutwindow-onback",
        "GrappledThrownOutWindowState",
    ),
    (
        "corpsethrownoutwindow-onfront",
        "GrappledThrownOutWindowState",
    ),
    (
        "corpsethrownoverfence-onback",
        "GrappledThrownOverFenceState",
    ),
    (
        "corpsethrownoverfence-onfront",
        "GrappledThrownOverFenceState",
    ),
    (
        "corpsethrownintocontainer-onback",
        "GrappledThrownIntoContainerState",
    ),
    (
        "corpsethrownintocontainer-onfront",
        "GrappledThrownIntoContainerState",
    ),
    ("attack", "AttackState"),
    ("attackvehicle", "AttackVehicleState"),
    ("bumped", "BumpedState"),
    ("climbfence", "ClimbOverFenceState"),
    ("climbwindow", "ClimbThroughWindowState"),
    ("eatbody", "ZombieEatBodyState"),
    ("falldown", "ZombieFallDownState"),
    ("vehiclecollision-falldown", "ZombieFallDownState"),
    ("vehiclecollision-ragdoll", "ZombieFallDownState"),
    ("falldown-eatingbody", "ZombieFallDownState"),
    ("falldown-knifedeath", "ZombieFallDownState"),
    ("falldown-knifedeath-ragdoll", "ZombieFallDownState"),
    ("falldown-onknees", "ZombieFallDownState"),
    ("falldown-whilesitting", "ZombieFallDownState"),
    ("falldown-headleft", "ZombieFallDownState"),
    ("falldown-headright", "ZombieFallDownState"),
    ("falldown-headtop", "ZombieFallDownState"),
    ("falldown-speardeath", "ZombieFallDownState"),
    ("falldown-speardeath-ragdoll", "ZombieFallDownState"),
    ("falldown-speardeath1", "ZombieFallDownState"),
    ("falldown-speardeath2", "ZombieFallDownState"),
    ("falldown-uppercut", "ZombieFallDownState"),
    ("falldown-ragdoll", "ZombieFallDownState"),
    ("falling", "ZombieFallingState"),
    ("face-target", "ZombieFaceTargetState"),
    ("fakedead", "FakeDeadZombieState"),
    ("fakedead-attack", "FakeDeadAttackState"),
    ("getdown", "ZombieGetDownState"),
    ("getup", "ZombieGetUpState"),
    ("getup-fromonback", "ZombieGetUpState"),
    ("getup-fromonfront", "ZombieGetUpState"),
    ("getup-fromsitting", "ZombieGetUpState"),
    ("hitreaction", "ZombieHitReactionState"),
    ("hitreaction-fencewindow", "ZombieHitReactionState"),
    ("hitreaction-knockeddown", "ZombieHitReactionState"),
    ("hitreaction-ragdoll", "ZombieHitReactionState"),
    ("hitreaction-shothead-bwd", "ZombieFallDownState"),
    ("hitreaction-shothead-bwd-ragdoll", "ZombieFallDownState"),
    ("hitreaction-shothead-fwd", "ZombieFallDownState"),
    ("hitreaction-shothead-fwd-ragdoll", "ZombieFallDownState"),
    ("hitreaction-shothead-fwd02", "ZombieFallDownState"),
    ("hitreaction-shothead-fwd02-ragdoll", "ZombieFallDownState"),
    ("hitreaction-onfloor", "ZombieHitReactionState"),
    ("hitreaction-gettingup", "ZombieHitReactionState"),
    ("hitreaction-whilesitting", "ZombieHitReactionState"),
    ("hitreaction-hit", "ZombieHitReactionState"),
    ("idle", "ZombieIdleState"),
    ("lunge", "LungeState"),
    ("onground", "ZombieOnGroundState"),
    ("falldown-ragdoll", "ZombieFallDownState"),
    ("onground-ragdoll", "ZombieRagdollOnGroundState"),
    ("pathfind", "PathFindState"),
    ("sitting", "ZombieSittingState"),
    ("staggerback", "StaggerBackState"),
    ("staggerback-knockeddown", "StaggerBackState"),
    ("staggerback-ragdoll", "StaggerBackState"),
    ("staggerback-knockeddown-ragdoll", "StaggerBackState"),
    ("knockeddown-headleft", "ZombieFallDownState"),
    ("knockeddown-headright", "ZombieFallDownState"),
    ("knockeddown-headtop", "ZombieFallDownState"),
    ("knockeddown-shotchestl", "ZombieFallDownState"),
    ("knockeddown-shotchestr", "ZombieFallDownState"),
    ("knockeddown-shotlegl", "ZombieFallDownState"),
    ("knockeddown-shotlegr", "ZombieFallDownState"),
    ("knockeddown-shotshoulderl", "ZombieFallDownState"),
    ("knockeddown-shotshoulderr", "ZombieFallDownState"),
    ("knockeddown-uppercut", "ZombieFallDownState"),
    ("vehiclecollision", "VehicleCollisionState"),
    ("vehiclecollision-staggerback", "StaggerBackState"),
    (
        "vehiclecollision-bumped",
        "VehicleCollisionMinorStaggerState",
    ),
    ("vehiclecollision-onground", "VehicleCollisionOnGroundState"),
    (
        "vehiclecollision-onground-dead",
        "VehicleCollisionOnGroundState",
    ),
    ("thump", "ThumpState"),
    ("turnalerted", "ZombieTurnAlerted"),
    ("walktoward", "WalkTowardState"),
    ("grappled", "GenericDefaultState"),
];

/// The AI state the action state `root` selects. `Err` for a registered state whose class is not ported.
pub fn ai_state_for(root: &str) -> Result<ZombieState, String> {
    let lower = root.to_lowercase();
    let Some((_, class)) = REGISTERED_AI_STATES.iter().find(|(n, _)| *n == lower) else {
        // StateMachineComponent.actionStateChanged: `aiState == null` -> `defaultState` (`setDefaultState(ZombieIdleState)`)
        return Ok(ZombieState::Idle);
    };
    match *class {
        "ZombieIdleState" => Ok(ZombieState::Idle),
        "PathFindState" => Ok(ZombieState::PathFind),
        "LungeState" => Ok(ZombieState::Lunge),
        "AttackState" => Ok(ZombieState::Attack),
        "ZombieFaceTargetState" => Ok(ZombieState::FaceTarget),
        "WalkTowardState" => Ok(ZombieState::WalkToward),
        "StaggerBackState" => Ok(ZombieState::StaggerBack),
        "ZombieFallDownState" => Ok(ZombieState::FallDown),
        "ZombieOnGroundState" => Ok(ZombieState::OnGround),
        "ZombieGetUpState" => Ok(ZombieState::GetUp),
        other => Err(format!(
            "IsoZombie.actionStateChanged: the AI state {other} of action state '{root}' is not ported"
        )),
    }
}
