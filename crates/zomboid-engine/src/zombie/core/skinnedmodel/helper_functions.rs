//! zombie.core.skinnedmodel.HelperFunctions: the quaternion twist angles the deferred (root) movement reads.
use crate::org::lwjgl::util::vector::quaternion::Quaternion;

/// HelperFunctions.getRotationY(Quaternion): the rotation about Y of the quaternion's X and Z axes (`rotation` is
/// normalised in place first; a zero length is the IllegalStateException). The decompiled body reuses its locals, here
/// each keeps one meaning: `r_x*` is the quaternion applied to the X axis, `r_z*` to the Z axis.
pub fn get_rotation_y(rotation: &mut Quaternion) -> Result<f32, String> {
    rotation
        .normalise()
        .map_err(|_| "java.lang.IllegalStateException".to_string())?;
    let qs = rotation.w;
    let v_x = rotation.x;
    let v_y = rotation.y;
    let v_z = rotation.z;
    let s2 = qs * qs;
    let v2 = v_x * v_x + v_y * v_y + v_z * v_z;
    // p = (1, 0, 0)
    let v_cross_px = v_y * 0.0f32 - v_z * 0.0f32;
    let v_cross_pz = v_x * 0.0f32 - v_y * 1.0f32;
    let v_dot_p = 1.0f32 * v_x + 0.0f32 * v_y + 0.0f32 * v_z;
    let r_x_x = (s2 - v2) * 1.0f32 + 2.0f32 * qs * v_cross_px + 2.0f32 * v_x * v_dot_p;
    let r_x_z = (s2 - v2) * 0.0f32 + 2.0f32 * qs * v_cross_pz + 2.0f32 * v_z * v_dot_p;
    // p = (0, 0, 1)
    let v_cross_px2 = v_y * 1.0f32 - v_z * 0.0f32;
    let v_cross_pz2 = v_x * 0.0f32 - v_y * 0.0f32;
    let v_dot_p2 = 0.0f32 * v_x + 0.0f32 * v_y + 1.0f32 * v_z;
    let r_z_x = (s2 - v2) * 0.0f32 + 2.0f32 * qs * v_cross_px2 + 2.0f32 * v_x * v_dot_p2;
    let r_z_z = (s2 - v2) * 1.0f32 + 2.0f32 * qs * v_cross_pz2 + 2.0f32 * v_z * v_dot_p2;
    let c = (r_x_x + r_z_z) / 2.0f32;
    let s = (r_x_z - r_z_x) / 2.0f32;
    // PZMath.sqrt = org.joml.Math.sqrt = (float) Math.sqrt
    let norm = f64::from(c * c + s * s).sqrt() as f32;
    let norm_c = c / norm;
    let norm_s = s / norm;
    Ok(crate::java_fdlibm::atan2(f64::from(-norm_s), f64::from(norm_c)) as f32)
}

/// HelperFunctions.getRotationZ(Quaternion): the rotation about Z (the quaternion applied to the X axis; no
/// normalisation).
pub fn get_rotation_z(rotation: &Quaternion) -> f32 {
    let s = rotation.w;
    let v_x = rotation.x;
    let v_y = rotation.y;
    let v_z = rotation.z;
    let s2 = s * s;
    let v2 = v_x * v_x + v_y * v_y + v_z * v_z;
    let v_cross_py = v_z * 1.0f32;
    let v_dot_p = 1.0f32 * v_x;
    let r_x = (s2 - v2) * 1.0f32 + 2.0f32 * v_x * v_dot_p;
    let r_y = 2.0f32 * s * v_cross_py + 2.0f32 * v_y * v_dot_p;
    crate::java_fdlibm::atan2(f64::from(r_y), f64::from(r_x)) as f32
}

/// PZMath.slerp(result, from, to, alpha): the double-precision spherical lerp (`org.joml.Math.acos/sin` are the
/// `java.lang.Math` ones), each part rounded to float once.
pub fn slerp(from: &Quaternion, to: &Quaternion, alpha: f32) -> Quaternion {
    // `double dot = from.x * to.x + ...`: the four products and their sum are float arithmetic, widened at the end
    let dot = f64::from(from.x * to.x + from.y * to.y + from.z * to.z + from.w * to.w);
    let abs_dot = if dot < 0.0 { -dot } else { dot };
    let mut scale0 = f64::from(1.0f32 - alpha);
    let mut scale1 = f64::from(alpha);
    if 1.0 - abs_dot > 0.1 {
        let angle = crate::java_fdlibm::acos(abs_dot);
        let sin_angle = crate::java_fdlibm::sin(angle);
        let inv_sin_angle = 1.0 / sin_angle;
        scale0 = crate::java_fdlibm::sin(angle * (1.0 - f64::from(alpha))) * inv_sin_angle;
        scale1 = crate::java_fdlibm::sin(angle * f64::from(alpha)) * inv_sin_angle;
    }
    if dot < 0.0 {
        scale1 = -scale1;
    }
    Quaternion::of(
        (scale0 * f64::from(from.x) + scale1 * f64::from(to.x)) as f32,
        (scale0 * f64::from(from.y) + scale1 * f64::from(to.y)) as f32,
        (scale0 * f64::from(from.z) + scale1 * f64::from(to.z)) as f32,
        (scale0 * f64::from(from.w) + scale1 * f64::from(to.w)) as f32,
    )
}

/// PZMath.lerp(Vector3f out, a, b, t): `a + (b - a) * t` per part, in float.
pub fn lerp3(a: (f32, f32, f32), b: (f32, f32, f32), t: f32) -> (f32, f32, f32) {
    (
        a.0 + (b.0 - a.0) * t,
        a.1 + (b.1 - a.1) * t,
        a.2 + (b.2 - a.2) * t,
    )
}
