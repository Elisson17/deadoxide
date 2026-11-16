//! org.lwjgl.util.vector.Matrix4f.

/// org.lwjgl.util.vector.Matrix4f: sixteen single-precision fields `mRC` (`new Matrix4f()` is the identity).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix4f {
    pub m00: f32,
    pub m01: f32,
    pub m02: f32,
    pub m03: f32,
    pub m10: f32,
    pub m11: f32,
    pub m12: f32,
    pub m13: f32,
    pub m20: f32,
    pub m21: f32,
    pub m22: f32,
    pub m23: f32,
    pub m30: f32,
    pub m31: f32,
    pub m32: f32,
    pub m33: f32,
}

impl Default for Matrix4f {
    fn default() -> Self {
        Matrix4f::new()
    }
}

impl Matrix4f {
    /// Matrix4f(): `setIdentity`.
    pub fn new() -> Self {
        Matrix4f {
            m00: 1.0,
            m01: 0.0,
            m02: 0.0,
            m03: 0.0,
            m10: 0.0,
            m11: 1.0,
            m12: 0.0,
            m13: 0.0,
            m20: 0.0,
            m21: 0.0,
            m22: 1.0,
            m23: 0.0,
            m30: 0.0,
            m31: 0.0,
            m32: 0.0,
            m33: 1.0,
        }
    }

    /// The sixteen fields in the order of `Matrix4f.store(FloatBuffer)`: m00, m01, m02, m03, m10, ...
    pub fn to_array(&self) -> [f32; 16] {
        [
            self.m00, self.m01, self.m02, self.m03, self.m10, self.m11, self.m12, self.m13,
            self.m20, self.m21, self.m22, self.m23, self.m30, self.m31, self.m32, self.m33,
        ]
    }

    /// Matrix4f.mul(left, right, dest): `left * right` (every field is computed before any is stored).
    pub fn mul(left: &Matrix4f, right: &Matrix4f) -> Matrix4f {
        Matrix4f {
            m00: left.m00 * right.m00
                + left.m10 * right.m01
                + left.m20 * right.m02
                + left.m30 * right.m03,
            m01: left.m01 * right.m00
                + left.m11 * right.m01
                + left.m21 * right.m02
                + left.m31 * right.m03,
            m02: left.m02 * right.m00
                + left.m12 * right.m01
                + left.m22 * right.m02
                + left.m32 * right.m03,
            m03: left.m03 * right.m00
                + left.m13 * right.m01
                + left.m23 * right.m02
                + left.m33 * right.m03,
            m10: left.m00 * right.m10
                + left.m10 * right.m11
                + left.m20 * right.m12
                + left.m30 * right.m13,
            m11: left.m01 * right.m10
                + left.m11 * right.m11
                + left.m21 * right.m12
                + left.m31 * right.m13,
            m12: left.m02 * right.m10
                + left.m12 * right.m11
                + left.m22 * right.m12
                + left.m32 * right.m13,
            m13: left.m03 * right.m10
                + left.m13 * right.m11
                + left.m23 * right.m12
                + left.m33 * right.m13,
            m20: left.m00 * right.m20
                + left.m10 * right.m21
                + left.m20 * right.m22
                + left.m30 * right.m23,
            m21: left.m01 * right.m20
                + left.m11 * right.m21
                + left.m21 * right.m22
                + left.m31 * right.m23,
            m22: left.m02 * right.m20
                + left.m12 * right.m21
                + left.m22 * right.m22
                + left.m32 * right.m23,
            m23: left.m03 * right.m20
                + left.m13 * right.m21
                + left.m23 * right.m22
                + left.m33 * right.m23,
            m30: left.m00 * right.m30
                + left.m10 * right.m31
                + left.m20 * right.m32
                + left.m30 * right.m33,
            m31: left.m01 * right.m30
                + left.m11 * right.m31
                + left.m21 * right.m32
                + left.m31 * right.m33,
            m32: left.m02 * right.m30
                + left.m12 * right.m31
                + left.m22 * right.m32
                + left.m32 * right.m33,
            m33: left.m03 * right.m30
                + left.m13 * right.m31
                + left.m23 * right.m32
                + left.m33 * right.m33,
        }
    }

    /// Matrix4f.transpose(src, dest)
    pub fn transpose(&self) -> Matrix4f {
        Matrix4f {
            m00: self.m00,
            m01: self.m10,
            m02: self.m20,
            m03: self.m30,
            m10: self.m01,
            m11: self.m11,
            m12: self.m21,
            m13: self.m31,
            m20: self.m02,
            m21: self.m12,
            m22: self.m22,
            m23: self.m32,
            m30: self.m03,
            m31: self.m13,
            m32: self.m23,
            m33: self.m33,
        }
    }

    /// Matrix4f.determinant
    pub fn determinant(&self) -> f32 {
        let mut f = self.m00
            * (self.m11 * self.m22 * self.m33
                + self.m12 * self.m23 * self.m31
                + self.m13 * self.m21 * self.m32
                - self.m13 * self.m22 * self.m31
                - self.m11 * self.m23 * self.m32
                - self.m12 * self.m21 * self.m33);
        f -= self.m01
            * (self.m10 * self.m22 * self.m33
                + self.m12 * self.m23 * self.m30
                + self.m13 * self.m20 * self.m32
                - self.m13 * self.m22 * self.m30
                - self.m10 * self.m23 * self.m32
                - self.m12 * self.m20 * self.m33);
        f += self.m02
            * (self.m10 * self.m21 * self.m33
                + self.m11 * self.m23 * self.m30
                + self.m13 * self.m20 * self.m31
                - self.m13 * self.m21 * self.m30
                - self.m10 * self.m23 * self.m31
                - self.m11 * self.m20 * self.m33);
        f - self.m03
            * (self.m10 * self.m21 * self.m32
                + self.m11 * self.m22 * self.m30
                + self.m12 * self.m20 * self.m31
                - self.m12 * self.m21 * self.m30
                - self.m10 * self.m22 * self.m31
                - self.m11 * self.m20 * self.m32)
    }

    #[allow(clippy::too_many_arguments)]
    fn determinant3x3(
        t00: f32,
        t01: f32,
        t02: f32,
        t10: f32,
        t11: f32,
        t12: f32,
        t20: f32,
        t21: f32,
        t22: f32,
    ) -> f32 {
        t00 * (t11 * t22 - t12 * t21)
            + t01 * (t12 * t20 - t10 * t22)
            + t02 * (t10 * t21 - t11 * t20)
    }

    /// Matrix4f.invert(src, dest): `None` is the Java `null` returned for a zero determinant.
    pub fn invert(src: &Matrix4f) -> Option<Matrix4f> {
        let determinant = src.determinant();
        if determinant == 0.0 {
            return None;
        }
        let d = Matrix4f::determinant3x3;
        let determinant_inv = 1.0f32 / determinant;
        let t00 = d(
            src.m11, src.m12, src.m13, src.m21, src.m22, src.m23, src.m31, src.m32, src.m33,
        );
        let t01 = -d(
            src.m10, src.m12, src.m13, src.m20, src.m22, src.m23, src.m30, src.m32, src.m33,
        );
        let t02 = d(
            src.m10, src.m11, src.m13, src.m20, src.m21, src.m23, src.m30, src.m31, src.m33,
        );
        let t03 = -d(
            src.m10, src.m11, src.m12, src.m20, src.m21, src.m22, src.m30, src.m31, src.m32,
        );
        let t10 = -d(
            src.m01, src.m02, src.m03, src.m21, src.m22, src.m23, src.m31, src.m32, src.m33,
        );
        let t11 = d(
            src.m00, src.m02, src.m03, src.m20, src.m22, src.m23, src.m30, src.m32, src.m33,
        );
        let t12 = -d(
            src.m00, src.m01, src.m03, src.m20, src.m21, src.m23, src.m30, src.m31, src.m33,
        );
        let t13 = d(
            src.m00, src.m01, src.m02, src.m20, src.m21, src.m22, src.m30, src.m31, src.m32,
        );
        let t20 = d(
            src.m01, src.m02, src.m03, src.m11, src.m12, src.m13, src.m31, src.m32, src.m33,
        );
        let t21 = -d(
            src.m00, src.m02, src.m03, src.m10, src.m12, src.m13, src.m30, src.m32, src.m33,
        );
        let t22 = d(
            src.m00, src.m01, src.m03, src.m10, src.m11, src.m13, src.m30, src.m31, src.m33,
        );
        let t23 = -d(
            src.m00, src.m01, src.m02, src.m10, src.m11, src.m12, src.m30, src.m31, src.m32,
        );
        let t30 = -d(
            src.m01, src.m02, src.m03, src.m11, src.m12, src.m13, src.m21, src.m22, src.m23,
        );
        let t31 = d(
            src.m00, src.m02, src.m03, src.m10, src.m12, src.m13, src.m20, src.m22, src.m23,
        );
        let t32 = -d(
            src.m00, src.m01, src.m03, src.m10, src.m11, src.m13, src.m20, src.m21, src.m23,
        );
        let t33 = d(
            src.m00, src.m01, src.m02, src.m10, src.m11, src.m12, src.m20, src.m21, src.m22,
        );
        // dest may alias src in Java, but every t.. above is computed first, so a fresh value is equivalent.
        let mut dest = *src;
        dest.m00 = t00 * determinant_inv;
        dest.m11 = t11 * determinant_inv;
        dest.m22 = t22 * determinant_inv;
        dest.m33 = t33 * determinant_inv;
        dest.m01 = t10 * determinant_inv;
        dest.m10 = t01 * determinant_inv;
        dest.m20 = t02 * determinant_inv;
        dest.m02 = t20 * determinant_inv;
        dest.m12 = t21 * determinant_inv;
        dest.m21 = t12 * determinant_inv;
        dest.m03 = t30 * determinant_inv;
        dest.m30 = t03 * determinant_inv;
        dest.m13 = t31 * determinant_inv;
        dest.m31 = t13 * determinant_inv;
        dest.m32 = t23 * determinant_inv;
        dest.m23 = t32 * determinant_inv;
        Some(dest)
    }
}
