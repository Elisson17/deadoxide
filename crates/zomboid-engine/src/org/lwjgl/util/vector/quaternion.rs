//! org.lwjgl.util.vector.Quaternion.
use super::matrix4f::Matrix4f;
use super::vector4f::Vector4f;
use crate::java_fdlibm;

/// org.lwjgl.util.vector.Quaternion: `new Quaternion()` is the identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Default for Quaternion {
    fn default() -> Self {
        Quaternion::new()
    }
}

impl Quaternion {
    /// Quaternion(): `setIdentity`.
    pub fn new() -> Self {
        Quaternion {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        }
    }

    /// Quaternion(float, float, float, float)
    pub fn of(x: f32, y: f32, z: f32, w: f32) -> Self {
        Quaternion { x, y, z, w }
    }

    /// Quaternion.set(float, float, float, float)
    pub fn set(&mut self, x: f32, y: f32, z: f32, w: f32) {
        self.x = x;
        self.y = y;
        self.z = z;
        self.w = w;
    }

    /// Quaternion.setIdentity
    pub fn set_identity(&mut self) {
        self.x = 0.0;
        self.y = 0.0;
        self.z = 0.0;
        self.w = 1.0;
    }

    /// Quaternion.lengthSquared
    pub fn length_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w
    }

    /// Vector.length: `(float)Math.sqrt(lengthSquared())`.
    pub fn length(&self) -> f32 {
        f64::from(self.length_squared()).sqrt() as f32
    }

    /// Quaternion.normalise(Quaternion dest) on `src` into a new quaternion.
    pub fn normalise_to(src: &Quaternion) -> Quaternion {
        let inv_l = 1.0f32 / src.length();
        Quaternion::of(src.x * inv_l, src.y * inv_l, src.z * inv_l, src.w * inv_l)
    }

    /// Vector.normalise() on a quaternion: `Err` is the IllegalStateException("Zero length vector").
    pub fn normalise(&mut self) -> Result<(), String> {
        let len = self.length();
        if len != 0.0 {
            let l = 1.0f32 / len;
            self.x *= l;
            self.y *= l;
            self.z *= l;
            self.w *= l;
            Ok(())
        } else {
            Err("Zero length vector".to_string())
        }
    }

    /// Quaternion.dot
    pub fn dot(left: &Quaternion, right: &Quaternion) -> f32 {
        left.x * right.x + left.y * right.y + left.z * right.z + left.w * right.w
    }

    /// Quaternion.mul(left, right, dest)
    pub fn mul(left: &Quaternion, right: &Quaternion) -> Quaternion {
        Quaternion::of(
            left.x * right.w + left.w * right.x + left.y * right.z - left.z * right.y,
            left.y * right.w + left.w * right.y + left.z * right.x - left.x * right.z,
            left.z * right.w + left.w * right.z + left.x * right.y - left.y * right.x,
            left.w * right.w - left.x * right.x - left.y * right.y - left.z * right.z,
        )
    }

    /// Quaternion.mulInverse(left, right, dest): `left * right^-1`, the inverse taken as `conjugate / lengthSquared`
    /// (a zero length leaves the factor at zero).
    pub fn mul_inverse(left: &Quaternion, right: &Quaternion) -> Quaternion {
        let mut n = right.length_squared();
        n = if n == 0.0 { n } else { 1.0f32 / n };
        Quaternion::of(
            (left.x * right.w - left.w * right.x - left.y * right.z + left.z * right.y) * n,
            (left.y * right.w - left.w * right.y - left.z * right.x + left.x * right.z) * n,
            (left.z * right.w - left.w * right.z - left.x * right.y + left.y * right.x) * n,
            (left.w * right.w + left.x * right.x + left.y * right.y + left.z * right.z) * n,
        )
    }

    /// Quaternion.setFromAxisAngle(Vector4f)
    pub fn set_from_axis_angle(&mut self, a1: &Vector4f) {
        self.x = a1.x;
        self.y = a1.y;
        self.z = a1.z;
        let n = f64::from(self.x * self.x + self.y * self.y + self.z * self.z).sqrt() as f32;
        let s = (java_fdlibm::sin(0.5 * f64::from(a1.w)) / f64::from(n)) as f32;
        self.x *= s;
        self.y *= s;
        self.z *= s;
        self.w = java_fdlibm::cos(0.5 * f64::from(a1.w)) as f32;
    }

    /// Quaternion.setFromMatrix(Matrix4f)
    pub fn set_from_matrix(&mut self, m: &Matrix4f) {
        self.set_from_mat(
            m.m00, m.m01, m.m02, m.m10, m.m11, m.m12, m.m20, m.m21, m.m22,
        );
    }

    /// Quaternion.setFromMat (private).
    #[allow(clippy::too_many_arguments)]
    pub fn set_from_mat(
        &mut self,
        m00: f32,
        m01: f32,
        m02: f32,
        m10: f32,
        m11: f32,
        m12: f32,
        m20: f32,
        m21: f32,
        m22: f32,
    ) {
        let tr = m00 + m11 + m22;
        if f64::from(tr) >= 0.0 {
            let mut s = (f64::from(tr) + 1.0).sqrt() as f32;
            self.w = s * 0.5;
            s = 0.5 / s;
            self.x = (m21 - m12) * s;
            self.y = (m02 - m20) * s;
            self.z = (m10 - m01) * s;
        } else {
            let max = crate::java::math_max_f32(crate::java::math_max_f32(m00, m11), m22);
            if max == m00 {
                let mut s = (f64::from(m00 - (m11 + m22)) + 1.0).sqrt() as f32;
                self.x = s * 0.5;
                s = 0.5 / s;
                self.y = (m01 + m10) * s;
                self.z = (m20 + m02) * s;
                self.w = (m21 - m12) * s;
            } else if max == m11 {
                let mut s = (f64::from(m11 - (m22 + m00)) + 1.0).sqrt() as f32;
                self.y = s * 0.5;
                s = 0.5 / s;
                self.z = (m12 + m21) * s;
                self.x = (m01 + m10) * s;
                self.w = (m02 - m20) * s;
            } else {
                let mut s = (f64::from(m22 - (m00 + m11)) + 1.0).sqrt() as f32;
                self.z = s * 0.5;
                s = 0.5 / s;
                self.x = (m20 + m02) * s;
                self.y = (m12 + m21) * s;
                self.w = (m10 - m01) * s;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mul_inverse_of_a_rotation_undoes_it() {
        let mut r = Quaternion::new();
        r.set_from_axis_angle(&Vector4f::of(1.0, 0.0, 0.0, -1.5707964));
        let inv = Quaternion::mul_inverse(&Quaternion::new(), &r);
        let id = Quaternion::mul(&inv, &r);
        assert!(
            (id.w - 1.0).abs() < 1e-6
                && id.x.abs() < 1e-6
                && id.y.abs() < 1e-6
                && id.z.abs() < 1e-6
        );
    }

    #[test]
    fn mul_inverse_of_zero_is_zero() {
        let z = Quaternion::of(0.0, 0.0, 0.0, 0.0);
        let q = Quaternion::mul_inverse(&Quaternion::new(), &z);
        assert_eq!((q.x, q.y, q.z, q.w), (0.0, 0.0, 0.0, 0.0));
    }
}
