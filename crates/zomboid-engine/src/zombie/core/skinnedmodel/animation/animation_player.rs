//! zombie.core.skinnedmodel.animation.AnimationPlayer, the body-angle part: `setTargetDirection`, `DoAngles`
//! (`interpolateBodyAngle`, `interpolateBodyTwist`, `interpolateShoulderTwist`) and the fields they keep. The bone
//! transforms, the twist bones, the deferred (root) movement and the ragdoll controller are not here (UNKNOWN).
use crate::zombie::core::math::pz_math;
use crate::zombie::iso::vector2::Vector2;

const PI: f32 = std::f64::consts::PI as f32;

/// zombie.iso.Vector2.getDirection(x, y): `PZMath.wrap((float) Math.atan2(y, x), -PI, PI)`.
pub fn vector2_get_direction(x: f32, y: f32) -> f32 {
    let angle = crate::java_fdlibm::atan2(f64::from(y), f64::from(x)) as f32;
    pz_math::wrap(angle, -PI, PI)
}

/// The angle state of an AnimationPlayer (the field initialisers: `maxTwistAngle = degToRad(70)`,
/// `shoulderTwistWeight = 1`, `angleStepDelta = angleTwistDelta = 1`).
#[derive(Clone, Debug)]
pub struct AnimationPlayer {
    pub angle: f32,
    pub target_angle: f32,
    pub twist_angle: f32,
    pub shoulder_twist_angle: f32,
    pub shoulder_twist_weight: f32,
    pub target_twist_angle: f32,
    pub max_twist_angle: f32,
    pub excess_twist: f32,
    pub angle_step_delta: f32,
    pub angle_twist_delta: f32,
    target_dir: Vector2,
    /// `deferredRotationWeight` (the deferred rotation of the tracks is not computed: always 0, UNKNOWN)
    deferred_rotation_weight: f32,
    deferred_angle_delta: f32,
    pub character_allows_twist: bool,
}

impl Default for AnimationPlayer {
    fn default() -> Self {
        AnimationPlayer {
            angle: 0.0,
            target_angle: 0.0,
            twist_angle: 0.0,
            shoulder_twist_angle: 0.0,
            shoulder_twist_weight: 1.0,
            target_twist_angle: 0.0,
            max_twist_angle: pz_math::deg_to_rad(70.0),
            excess_twist: 0.0,
            angle_step_delta: 1.0,
            angle_twist_delta: 1.0,
            target_dir: Vector2::default(),
            deferred_rotation_weight: 0.0,
            deferred_angle_delta: 0.0,
            character_allows_twist: false,
        }
    }
}

impl AnimationPlayer {
    /// AnimationPlayer.updateForwardDirection(character): `setTargetDirection(forward)`, `characterAllowsTwist` and
    /// `shoulderTwistWeight` of the character.
    pub fn update_forward_direction(
        &mut self,
        forward: Vector2,
        allows_twist: bool,
        shoulder_twist_weight: f32,
    ) {
        self.set_target_direction(forward.x, forward.y);
        self.character_allows_twist = allows_twist;
        self.shoulder_twist_weight = shoulder_twist_weight;
    }

    /// AnimationPlayer.setTargetDirection
    pub fn set_target_direction(&mut self, dir_x: f32, dir_y: f32) {
        if self.target_dir.x != dir_x || self.target_dir.y != dir_y {
            self.target_angle = vector2_get_direction(dir_x, dir_y);
            self.target_twist_angle = pz_math::get_closest_angle(self.angle, self.target_angle);
            let target_twist_clamped = pz_math::clamp(
                self.target_twist_angle,
                -self.max_twist_angle,
                self.max_twist_angle,
            );
            self.excess_twist =
                pz_math::get_closest_angle(target_twist_clamped, self.target_twist_angle);
            self.target_dir = Vector2::new(dir_x, dir_y);
        }
    }

    /// AnimationPlayer.setAngleToTarget
    fn set_angle_to_target(&mut self) {
        self.angle = self.target_angle;
    }

    /// AnimationPlayer.DoAngles(deltaT) for a player that is not ragdolling: `GameTime.getMultiplierFromTimeDelta` is
    /// `timeDelta * 0.8F * multiplierBias * 60.0F` (bias 1).
    pub fn do_angles(&mut self, delta_t: f32) {
        let mut angle_scaled_step_base = 0.15f32 * (delta_t * 0.8f32 * 1.0f32 * 60.0f32);
        angle_scaled_step_base = pz_math::min(angle_scaled_step_base, PI);
        self.interpolate_body_angle(angle_scaled_step_base);
        self.interpolate_body_twist(angle_scaled_step_base);
        self.interpolate_shoulder_twist(angle_scaled_step_base);
    }

    fn interpolate_body_angle(&mut self, angle_scaled_step_base: f32) {
        let target_angle = self.target_angle;
        let diff = pz_math::get_closest_angle(self.angle, target_angle);
        if pz_math::equal(diff, 0.0, 0.001) {
            self.set_angle_to_target();
            self.target_twist_angle = 0.0;
        } else {
            let diff_sign = pz_math::sign(diff);
            let angle_step_undeferred =
                angle_scaled_step_base * diff_sign as f32 * self.angle_step_delta;
            let angle_step = if self.deferred_rotation_weight > 0.0 {
                self.deferred_angle_delta * self.deferred_rotation_weight
                    + angle_step_undeferred * (1.0 - self.deferred_rotation_weight)
            } else {
                angle_step_undeferred
            };
            let step_sign = pz_math::sign(angle_step);
            let prev_angle = self.angle;
            let next_angle_unclamped = prev_angle + angle_step;
            let new_diff_unclamped = pz_math::get_closest_angle(next_angle_unclamped, target_angle);
            let new_diff_unclamped_sign = pz_math::sign(new_diff_unclamped);
            if new_diff_unclamped_sign != diff_sign && step_sign == diff_sign {
                self.set_angle_to_target();
                self.target_twist_angle = 0.0;
            } else {
                self.angle = next_angle_unclamped;
                self.target_twist_angle = new_diff_unclamped;
            }
        }
    }

    fn interpolate_body_twist(&mut self, angle_scaled_step_base: f32) {
        let target_twist_unclamped = pz_math::wrap(self.target_twist_angle, -PI, PI);
        let target_twist = pz_math::clamp(
            target_twist_unclamped,
            -self.max_twist_angle,
            self.max_twist_angle,
        );
        self.excess_twist = pz_math::get_closest_angle(target_twist, target_twist_unclamped);
        let twist_diff = pz_math::get_closest_angle(self.twist_angle, target_twist);
        if pz_math::equal(twist_diff, 0.0, 0.001) {
            self.twist_angle = target_twist;
        } else {
            let twist_diff_sign = pz_math::sign(twist_diff);
            let twist_angle_step =
                angle_scaled_step_base * twist_diff_sign as f32 * pz_abs(self.angle_twist_delta);
            let prev_twist = self.twist_angle;
            let next_twist_unclamped = prev_twist + twist_angle_step;
            let new_diff_unclamped = pz_math::get_closest_angle(next_twist_unclamped, target_twist);
            let new_diff_unclamped_sign = pz_math::sign(new_diff_unclamped);
            if new_diff_unclamped_sign == twist_diff_sign {
                self.twist_angle = next_twist_unclamped;
            } else {
                self.twist_angle = target_twist;
            }
        }
    }

    fn interpolate_shoulder_twist(&mut self, angle_scaled_step_base: f32) {
        let target_twist = pz_math::wrap(self.twist_angle, -PI, PI);
        let twist_diff = pz_math::get_closest_angle(self.shoulder_twist_angle, target_twist);
        if pz_math::equal(twist_diff, 0.0, 0.001) {
            self.shoulder_twist_angle = target_twist;
        } else {
            let twist_diff_sign = pz_math::sign(twist_diff);
            let twist_angle_step = angle_scaled_step_base
                * twist_diff_sign as f32
                * pz_abs(self.angle_twist_delta)
                * 0.55f32;
            let prev_twist = self.shoulder_twist_angle;
            let next_twist_unclamped = prev_twist + twist_angle_step;
            let new_diff_unclamped = pz_math::get_closest_angle(next_twist_unclamped, target_twist);
            let new_diff_unclamped_sign = pz_math::sign(new_diff_unclamped);
            if new_diff_unclamped_sign == twist_diff_sign {
                self.shoulder_twist_angle = next_twist_unclamped;
            } else {
                self.shoulder_twist_angle = target_twist;
            }
        }
    }

    /// AnimationPlayer.getLookAngle's body part: `getAngle() + getTwistAngle()`.
    pub fn look_angle(&self) -> f32 {
        self.angle + self.twist_angle
    }
}

/// zombie.core.math.PZMath.abs (float): `val * sign(val)`.
fn pz_abs(val: f32) -> f32 {
    val * pz_math::sign(val) as f32
}
