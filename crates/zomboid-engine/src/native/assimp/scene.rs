//! assimp `aiScene` and the types it holds (`scene.h`, `mesh.h`, `anim.h`, `material.h`, `types.h`), with the
//! members the game's jassimp wrapper exposes.

/// aiVector3D
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Vec3 { x, y, z }
    }

    /// aiVector3t::Length: `std::sqrt(x*x + y*y + z*z)`.
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

/// aiQuaternion (`w, x, y, z`)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub w: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Default for Quat {
    fn default() -> Self {
        Quat {
            w: 1.0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

/// aiMatrix3x3 (row-major `a1 a2 a3 / b1 b2 b3 / c1 c2 c3`)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3 {
    pub a1: f32,
    pub a2: f32,
    pub a3: f32,
    pub b1: f32,
    pub b2: f32,
    pub b3: f32,
    pub c1: f32,
    pub c2: f32,
    pub c3: f32,
}

impl Quat {
    /// aiQuaternion::GetMatrix
    pub fn get_matrix(&self) -> Mat3 {
        let (w, x, y, z) = (self.w, self.x, self.y, self.z);
        Mat3 {
            a1: 1.0 - 2.0 * (y * y + z * z),
            a2: 2.0 * (x * y - z * w),
            a3: 2.0 * (x * z + y * w),
            b1: 2.0 * (x * y + z * w),
            b2: 1.0 - 2.0 * (x * x + z * z),
            b3: 2.0 * (y * z - x * w),
            c1: 2.0 * (x * z - y * w),
            c2: 2.0 * (y * z + x * w),
            c3: 1.0 - 2.0 * (x * x + y * y),
        }
    }

    /// aiQuaternion(const aiMatrix3x3&)
    pub fn from_matrix(m: &Mat3) -> Quat {
        let t = m.a1 + m.b2 + m.c3;
        if t > 0.0 {
            let s = (1.0f32 + t).sqrt() * 2.0;
            Quat {
                x: (m.c2 - m.b3) / s,
                y: (m.a3 - m.c1) / s,
                z: (m.b1 - m.a2) / s,
                w: 0.25 * s,
            }
        } else if m.a1 > m.b2 && m.a1 > m.c3 {
            let s = (1.0f32 + m.a1 - m.b2 - m.c3).sqrt() * 2.0;
            Quat {
                x: 0.25 * s,
                y: (m.b1 + m.a2) / s,
                z: (m.a3 + m.c1) / s,
                w: (m.c2 - m.b3) / s,
            }
        } else if m.b2 > m.c3 {
            let s = (1.0f32 + m.b2 - m.a1 - m.c3).sqrt() * 2.0;
            Quat {
                x: (m.b1 + m.a2) / s,
                y: 0.25 * s,
                z: (m.c2 + m.b3) / s,
                w: (m.a3 - m.c1) / s,
            }
        } else {
            let s = (1.0f32 + m.c3 - m.a1 - m.b2).sqrt() * 2.0;
            Quat {
                x: (m.a3 + m.c1) / s,
                y: (m.c2 + m.b3) / s,
                z: 0.25 * s,
                w: (m.b1 - m.a2) / s,
            }
        }
    }
}

/// `std::sin(float)`: the correctly rounded result (computed in double precision).
pub fn sinf(a: f32) -> f32 {
    f64::from(a).sin() as f32
}

/// `std::cos(float)`: the correctly rounded result (computed in double precision).
pub fn cosf(a: f32) -> f32 {
    f64::from(a).cos() as f32
}

/// aiMatrix4x4 (row-major `a1 a2 a3 a4 / b1 ... / d1 d2 d3 d4`)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    pub a1: f32,
    pub a2: f32,
    pub a3: f32,
    pub a4: f32,
    pub b1: f32,
    pub b2: f32,
    pub b3: f32,
    pub b4: f32,
    pub c1: f32,
    pub c2: f32,
    pub c3: f32,
    pub c4: f32,
    pub d1: f32,
    pub d2: f32,
    pub d3: f32,
    pub d4: f32,
}

impl Default for Mat4 {
    /// aiMatrix4x4(): identity
    fn default() -> Self {
        Mat4 {
            a1: 1.0,
            a2: 0.0,
            a3: 0.0,
            a4: 0.0,
            b1: 0.0,
            b2: 1.0,
            b3: 0.0,
            b4: 0.0,
            c1: 0.0,
            c2: 0.0,
            c3: 1.0,
            c4: 0.0,
            d1: 0.0,
            d2: 0.0,
            d3: 0.0,
            d4: 1.0,
        }
    }
}

impl Mat4 {
    /// The sixteen values row by row (`get(r, c)` of jassimp's `AiMatrix4f`).
    pub fn to_array(&self) -> [f32; 16] {
        [
            self.a1, self.a2, self.a3, self.a4, self.b1, self.b2, self.b3, self.b4, self.c1,
            self.c2, self.c3, self.c4, self.d1, self.d2, self.d3, self.d4,
        ]
    }

    /// aiMatrix4x4::operator*
    pub fn mul(&self, m: &Mat4) -> Mat4 {
        let a = self;
        Mat4 {
            a1: m.a1 * a.a1 + m.b1 * a.a2 + m.c1 * a.a3 + m.d1 * a.a4,
            a2: m.a2 * a.a1 + m.b2 * a.a2 + m.c2 * a.a3 + m.d2 * a.a4,
            a3: m.a3 * a.a1 + m.b3 * a.a2 + m.c3 * a.a3 + m.d3 * a.a4,
            a4: m.a4 * a.a1 + m.b4 * a.a2 + m.c4 * a.a3 + m.d4 * a.a4,
            b1: m.a1 * a.b1 + m.b1 * a.b2 + m.c1 * a.b3 + m.d1 * a.b4,
            b2: m.a2 * a.b1 + m.b2 * a.b2 + m.c2 * a.b3 + m.d2 * a.b4,
            b3: m.a3 * a.b1 + m.b3 * a.b2 + m.c3 * a.b3 + m.d3 * a.b4,
            b4: m.a4 * a.b1 + m.b4 * a.b2 + m.c4 * a.b3 + m.d4 * a.b4,
            c1: m.a1 * a.c1 + m.b1 * a.c2 + m.c1 * a.c3 + m.d1 * a.c4,
            c2: m.a2 * a.c1 + m.b2 * a.c2 + m.c2 * a.c3 + m.d2 * a.c4,
            c3: m.a3 * a.c1 + m.b3 * a.c2 + m.c3 * a.c3 + m.d3 * a.c4,
            c4: m.a4 * a.c1 + m.b4 * a.c2 + m.c4 * a.c3 + m.d4 * a.c4,
            d1: m.a1 * a.d1 + m.b1 * a.d2 + m.c1 * a.d3 + m.d1 * a.d4,
            d2: m.a2 * a.d1 + m.b2 * a.d2 + m.c2 * a.d3 + m.d2 * a.d4,
            d3: m.a3 * a.d1 + m.b3 * a.d2 + m.c3 * a.d3 + m.d3 * a.d4,
            d4: m.a4 * a.d1 + m.b4 * a.d2 + m.c4 * a.d3 + m.d4 * a.d4,
        }
    }

    /// aiMatrix4x4::Translation
    pub fn translation(v: Vec3) -> Mat4 {
        Mat4 {
            a4: v.x,
            b4: v.y,
            c4: v.z,
            ..Mat4::default()
        }
    }

    /// aiMatrix4x4::Scaling
    pub fn scaling(v: Vec3) -> Mat4 {
        Mat4 {
            a1: v.x,
            b2: v.y,
            c3: v.z,
            ..Mat4::default()
        }
    }

    /// aiMatrix4x4::RotationX (`std::cos(float)` is computed in double precision and rounded)
    pub fn rotation_x(a: f32) -> Mat4 {
        let (s, c) = (sinf(a), cosf(a));
        Mat4 {
            b2: c,
            c3: c,
            b3: -s,
            c2: s,
            ..Mat4::default()
        }
    }

    /// aiMatrix4x4::RotationY
    pub fn rotation_y(a: f32) -> Mat4 {
        let (s, c) = (sinf(a), cosf(a));
        Mat4 {
            a1: c,
            c3: c,
            c1: -s,
            a3: s,
            ..Mat4::default()
        }
    }

    /// aiMatrix4x4::RotationZ
    pub fn rotation_z(a: f32) -> Mat4 {
        let (s, c) = (sinf(a), cosf(a));
        Mat4 {
            a1: c,
            b2: c,
            a2: -s,
            b1: s,
            ..Mat4::default()
        }
    }

    /// aiMatrix4x4::IsIdentity (epsilon 10e-3)
    pub fn is_identity(&self) -> bool {
        let e = 10e-3_f64 as f32;
        let small = |v: f32| v <= e && v >= -e;
        let one = |v: f32| f64::from(v) <= 1.0 + f64::from(e) && f64::from(v) >= 1.0 - f64::from(e);
        small(self.a2)
            && small(self.a3)
            && small(self.a4)
            && small(self.b1)
            && small(self.b3)
            && small(self.b4)
            && small(self.c1)
            && small(self.c2)
            && small(self.c4)
            && small(self.d1)
            && small(self.d2)
            && small(self.d3)
            && one(self.a1)
            && one(self.b2)
            && one(self.c3)
            && one(self.d4)
    }

    /// aiMatrix4x4::Inverse
    pub fn inverse(&self) -> Mat4 {
        let det = self.determinant();
        if det == 0.0 {
            let n = f32::NAN;
            return Mat4 {
                a1: n,
                a2: n,
                a3: n,
                a4: n,
                b1: n,
                b2: n,
                b3: n,
                b4: n,
                c1: n,
                c2: n,
                c3: n,
                c4: n,
                d1: n,
                d2: n,
                d3: n,
                d4: n,
            };
        }
        let invdet = 1.0f32 / det;
        let Mat4 {
            a1,
            a2,
            a3,
            a4,
            b1,
            b2,
            b3,
            b4,
            c1,
            c2,
            c3,
            c4,
            d1,
            d2,
            d3,
            d4,
        } = *self;
        Mat4 {
            a1: invdet
                * (b2 * (c3 * d4 - c4 * d3) + b3 * (c4 * d2 - c2 * d4) + b4 * (c2 * d3 - c3 * d2)),
            a2: -invdet
                * (a2 * (c3 * d4 - c4 * d3) + a3 * (c4 * d2 - c2 * d4) + a4 * (c2 * d3 - c3 * d2)),
            a3: invdet
                * (a2 * (b3 * d4 - b4 * d3) + a3 * (b4 * d2 - b2 * d4) + a4 * (b2 * d3 - b3 * d2)),
            a4: -invdet
                * (a2 * (b3 * c4 - b4 * c3) + a3 * (b4 * c2 - b2 * c4) + a4 * (b2 * c3 - b3 * c2)),
            b1: -invdet
                * (b1 * (c3 * d4 - c4 * d3) + b3 * (c4 * d1 - c1 * d4) + b4 * (c1 * d3 - c3 * d1)),
            b2: invdet
                * (a1 * (c3 * d4 - c4 * d3) + a3 * (c4 * d1 - c1 * d4) + a4 * (c1 * d3 - c3 * d1)),
            b3: -invdet
                * (a1 * (b3 * d4 - b4 * d3) + a3 * (b4 * d1 - b1 * d4) + a4 * (b1 * d3 - b3 * d1)),
            b4: invdet
                * (a1 * (b3 * c4 - b4 * c3) + a3 * (b4 * c1 - b1 * c4) + a4 * (b1 * c3 - b3 * c1)),
            c1: invdet
                * (b1 * (c2 * d4 - c4 * d2) + b2 * (c4 * d1 - c1 * d4) + b4 * (c1 * d2 - c2 * d1)),
            c2: -invdet
                * (a1 * (c2 * d4 - c4 * d2) + a2 * (c4 * d1 - c1 * d4) + a4 * (c1 * d2 - c2 * d1)),
            c3: invdet
                * (a1 * (b2 * d4 - b4 * d2) + a2 * (b4 * d1 - b1 * d4) + a4 * (b1 * d2 - b2 * d1)),
            c4: -invdet
                * (a1 * (b2 * c4 - b4 * c2) + a2 * (b4 * c1 - b1 * c4) + a4 * (b1 * c2 - b2 * c1)),
            d1: -invdet
                * (b1 * (c2 * d3 - c3 * d2) + b2 * (c3 * d1 - c1 * d3) + b3 * (c1 * d2 - c2 * d1)),
            d2: invdet
                * (a1 * (c2 * d3 - c3 * d2) + a2 * (c3 * d1 - c1 * d3) + a3 * (c1 * d2 - c2 * d1)),
            d3: -invdet
                * (a1 * (b2 * d3 - b3 * d2) + a2 * (b3 * d1 - b1 * d3) + a3 * (b1 * d2 - b2 * d1)),
            d4: invdet
                * (a1 * (b2 * c3 - b3 * c2) + a2 * (b3 * c1 - b1 * c3) + a3 * (b1 * c2 - b2 * c1)),
        }
    }

    /// aiMatrix4x4::Determinant
    pub fn determinant(&self) -> f32 {
        self.a1 * self.b2 * self.c3 * self.d4 - self.a1 * self.b2 * self.c4 * self.d3
            + self.a1 * self.b3 * self.c4 * self.d2
            - self.a1 * self.b3 * self.c2 * self.d4
            + self.a1 * self.b4 * self.c2 * self.d3
            - self.a1 * self.b4 * self.c3 * self.d2
            - self.a2 * self.b3 * self.c4 * self.d1
            + self.a2 * self.b3 * self.c1 * self.d4
            - self.a2 * self.b4 * self.c1 * self.d3
            + self.a2 * self.b4 * self.c3 * self.d1
            - self.a2 * self.b1 * self.c3 * self.d4
            + self.a2 * self.b1 * self.c4 * self.d3
            + self.a3 * self.b4 * self.c1 * self.d2
            - self.a3 * self.b4 * self.c2 * self.d1
            + self.a3 * self.b1 * self.c2 * self.d4
            - self.a3 * self.b1 * self.c4 * self.d2
            + self.a3 * self.b2 * self.c4 * self.d1
            - self.a3 * self.b2 * self.c1 * self.d4
            - self.a4 * self.b1 * self.c2 * self.d3
            + self.a4 * self.b1 * self.c3 * self.d2
            - self.a4 * self.b2 * self.c3 * self.d1
            + self.a4 * self.b2 * self.c1 * self.d3
            - self.a4 * self.b3 * self.c1 * self.d2
            + self.a4 * self.b3 * self.c2 * self.d1
    }

    /// aiMatrix4x4::Decompose(scaling, rotation, position)
    pub fn decompose(&self) -> (Vec3, Quat, Vec3) {
        let pos = Vec3::new(self.a4, self.b4, self.c4);
        let mut cols = [
            Vec3::new(self.a1, self.b1, self.c1),
            Vec3::new(self.a2, self.b2, self.c2),
            Vec3::new(self.a3, self.b3, self.c3),
        ];
        let mut scaling = Vec3::new(cols[0].length(), cols[1].length(), cols[2].length());
        if self.determinant() < 0.0 {
            scaling = Vec3::new(-scaling.x, -scaling.y, -scaling.z);
        }
        if scaling.x != 0.0 {
            cols[0] = Vec3::new(
                cols[0].x / scaling.x,
                cols[0].y / scaling.x,
                cols[0].z / scaling.x,
            );
        }
        if scaling.y != 0.0 {
            cols[1] = Vec3::new(
                cols[1].x / scaling.y,
                cols[1].y / scaling.y,
                cols[1].z / scaling.y,
            );
        }
        if scaling.z != 0.0 {
            cols[2] = Vec3::new(
                cols[2].x / scaling.z,
                cols[2].y / scaling.z,
                cols[2].z / scaling.z,
            );
        }
        let m = Mat3 {
            a1: cols[0].x,
            a2: cols[1].x,
            a3: cols[2].x,
            b1: cols[0].y,
            b2: cols[1].y,
            b3: cols[2].y,
            c1: cols[0].z,
            c2: cols[1].z,
            c3: cols[2].z,
        };
        (scaling, Quat::from_matrix(&m), pos)
    }
}

/// aiVertexWeight
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexWeight {
    pub vertex_id: u32,
    pub weight: f32,
}

/// aiBone
#[derive(Clone, Debug, PartialEq)]
pub struct Bone {
    pub name: String,
    pub weights: Vec<VertexWeight>,
    pub offset_matrix: Mat4,
}

/// aiPrimitiveType flags
pub const PRIMITIVE_POINT: u32 = 1;
pub const PRIMITIVE_LINE: u32 = 2;
pub const PRIMITIVE_TRIANGLE: u32 = 4;
pub const PRIMITIVE_POLYGON: u32 = 8;

/// aiMesh (`AI_MAX_NUMBER_OF_TEXTURECOORDS` and `AI_MAX_NUMBER_OF_COLOR_SETS` are 8)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub name: String,
    pub primitive_types: u32,
    pub vertices: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub tangents: Vec<Vec3>,
    pub bitangents: Vec<Vec3>,
    pub colors: [Vec<[f32; 4]>; 8],
    pub texture_coords: [Vec<Vec3>; 8],
    pub num_uv_components: [u32; 8],
    pub faces: Vec<Vec<u32>>,
    pub bones: Vec<Bone>,
    pub material_index: u32,
}

/// aiMaterialProperty (the data bytes as stored)
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialProperty {
    pub key: String,
    pub semantic: u32,
    pub index: u32,
    pub data: Vec<u8>,
    /// aiPropertyTypeInfo (1 float, 2 double, 3 string, 4 integer, 5 buffer)
    pub ptype: u32,
}

/// aiMaterial
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Material {
    pub properties: Vec<MaterialProperty>,
}

/// aiNode
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    pub transformation: Mat4,
    pub meshes: Vec<u32>,
    pub children: Vec<Node>,
}

/// aiVectorKey
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VectorKey {
    pub time: f64,
    pub value: Vec3,
}

/// aiQuatKey
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuatKey {
    pub time: f64,
    pub value: Quat,
}

/// aiNodeAnim (`pre_state` and `post_state` are the aiAnimBehaviour values)
#[derive(Clone, Debug, PartialEq)]
pub struct NodeAnim {
    pub node_name: String,
    pub position_keys: Vec<VectorKey>,
    pub rotation_keys: Vec<QuatKey>,
    pub scaling_keys: Vec<VectorKey>,
    pub pre_state: i32,
    pub post_state: i32,
}

impl NodeAnim {
    /// whether any of the three key arrays is non-empty (the library drops a channel with none)
    pub fn has_keys(&self) -> bool {
        !self.position_keys.is_empty()
            || !self.rotation_keys.is_empty()
            || !self.scaling_keys.is_empty()
    }
}

/// aiAnimation
#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    pub name: String,
    pub duration: f64,
    pub ticks_per_second: f64,
    pub channels: Vec<NodeAnim>,
}

/// aiScene (textures, lights and cameras are not read by the game's loaders)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    pub animations: Vec<Animation>,
    pub root_node: Option<Node>,
    pub num_cameras: u32,
    pub num_lights: u32,
}
