//! assimp `code/AssetLib/X/XFileParser.cpp` and `XFileHelper.h`: the reader of DirectX `.x` files in the text format
//! (`xof 0303txt 0032`; the binary and compressed variants are not ported: no file of the game uses them).
use super::fast_atof::fast_atoreal_move;
use super::scene::{Mat4, Vec3};

/// XFile::Material (`mTextures`: name and whether it is a normal map)
#[derive(Clone, Debug, Default)]
pub struct XMaterial {
    pub name: String,
    pub is_reference: bool,
    pub diffuse: [f32; 4],
    pub specular_exponent: f32,
    pub specular: [f32; 3],
    pub emissive: [f32; 3],
    pub textures: Vec<(String, bool)>,
    pub scene_index: u32,
}

/// XFile::Bone
#[derive(Clone, Debug)]
pub struct XBone {
    pub name: String,
    pub weights: Vec<(u32, f32)>,
    pub offset_matrix: Mat4,
}

/// XFile::Mesh
#[derive(Clone, Debug, Default)]
pub struct XMesh {
    pub name: String,
    pub positions: Vec<Vec3>,
    pub pos_faces: Vec<Vec<u32>>,
    pub normals: Vec<Vec3>,
    pub norm_faces: Vec<Vec<u32>>,
    pub num_textures: usize,
    pub tex_coords: [Vec<[f32; 2]>; 8],
    pub num_color_sets: usize,
    pub colors: [Vec<[f32; 4]>; 8],
    pub face_materials: Vec<u32>,
    pub materials: Vec<XMaterial>,
    pub bones: Vec<XBone>,
}

/// XFile::Node
#[derive(Clone, Debug)]
pub struct XNode {
    pub name: String,
    pub trafo_matrix: Mat4,
    pub children: Vec<XNode>,
    pub meshes: Vec<XMesh>,
}

impl XNode {
    fn new() -> Self {
        XNode {
            name: String::new(),
            trafo_matrix: Mat4::default(),
            children: Vec::new(),
            meshes: Vec::new(),
        }
    }
}

/// XFile::AnimBone
#[derive(Clone, Debug, Default)]
pub struct XAnimBone {
    pub bone_name: String,
    pub pos_keys: Vec<(f64, Vec3)>,
    /// `aiQuatKey` values as read (`w x y z`)
    pub rot_keys: Vec<(f64, [f32; 4])>,
    pub scale_keys: Vec<(f64, Vec3)>,
    pub trafo_keys: Vec<(f64, Mat4)>,
}

/// XFile::Animation
#[derive(Clone, Debug, Default)]
pub struct XAnimation {
    pub name: String,
    pub anims: Vec<XAnimBone>,
}

/// XFile::Scene
#[derive(Clone, Debug)]
pub struct XScene {
    pub root_node: Option<XNode>,
    pub global_meshes: Vec<XMesh>,
    pub global_materials: Vec<XMaterial>,
    pub anims: Vec<XAnimation>,
    pub anim_ticks_per_second: u32,
}

struct Parser<'a> {
    buf: &'a [u8],
    p: usize,
    end: usize,
    line_number: u32,
}

type R<T> = Result<T, String>;

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

impl<'a> Parser<'a> {
    fn at(&self, i: usize) -> u8 {
        self.buf.get(i).copied().unwrap_or(0)
    }

    /// XFileParser::ThrowException (text format): `DeadlyImportError("Line ", mLineNumber, ": ", text)`.
    fn throw<T>(&self, text: &str) -> R<T> {
        Err(format!("Line {}: {}", self.line_number, text))
    }

    /// XFileParser::FindNextNoneWhiteSpace
    fn find_next_none_white_space(&mut self) {
        loop {
            while self.p < self.end && is_space(self.at(self.p)) {
                if self.at(self.p) == b'\n' {
                    self.line_number += 1;
                }
                self.p += 1;
            }
            if self.p >= self.end {
                return;
            }
            if (self.at(self.p) == b'/' && self.at(self.p + 1) == b'/') || self.at(self.p) == b'#' {
                self.read_until_end_of_line();
            } else {
                break;
            }
        }
    }

    /// XFileParser::ReadUntilEndOfLine
    fn read_until_end_of_line(&mut self) {
        if self.p >= self.end {
            return;
        }
        while self.p < self.end {
            if self.at(self.p) == b'\n' || self.at(self.p) == b'\r' {
                self.p += 1;
                self.line_number += 1;
                return;
            }
            self.p += 1;
        }
    }

    /// XFileParser::GetNextToken (text format)
    fn get_next_token(&mut self) -> String {
        let mut s: Vec<u8> = Vec::new();
        self.find_next_none_white_space();
        if self.p >= self.end {
            return String::new();
        }
        while self.p < self.end && !is_space(self.at(self.p)) {
            let c = self.at(self.p);
            if c == b';' || c == b'}' || c == b'{' || c == b',' {
                if s.is_empty() {
                    s.push(c);
                    self.p += 1;
                }
                break;
            }
            s.push(c);
            self.p += 1;
        }
        String::from_utf8_lossy(&s).into_owned()
    }

    /// XFileParser::GetNextTokenAsString (text format)
    fn get_next_token_as_string(&mut self) -> R<String> {
        self.find_next_none_white_space();
        if self.p >= self.end {
            return self.throw("Unexpected end of file while parsing string");
        }
        if self.at(self.p) != b'"' {
            return self.throw("Expected quotation mark.");
        }
        self.p += 1;
        let mut bytes: Vec<u8> = Vec::new();
        while self.p < self.end && self.at(self.p) != b'"' {
            bytes.push(self.at(self.p));
            self.p += 1;
        }
        let s = String::from_utf8_lossy(&bytes).into_owned();
        if self.p + 1 >= self.end {
            return self.throw("Unexpected end of file while parsing string");
        }
        if self.at(self.p + 1) != b';' || self.at(self.p) != b'"' {
            return self.throw("Expected quotation mark and semicolon at the end of a string.");
        }
        self.p += 2;
        Ok(s)
    }

    /// XFileParser::readHeadOfDataObject
    fn read_head_of_data_object(&mut self) -> R<String> {
        let mut name = String::new();
        let name_or_brace = self.get_next_token();
        if name_or_brace != "{" {
            name = name_or_brace;
            if self.get_next_token() != "{" {
                return self.throw("Opening brace expected.");
            }
        }
        Ok(name)
    }

    /// XFileParser::CheckForClosingBrace
    fn check_for_closing_brace(&mut self) -> R<()> {
        if self.get_next_token() != "}" {
            return self.throw("Closing brace expected.");
        }
        Ok(())
    }

    /// XFileParser::CheckForSemicolon
    fn check_for_semicolon(&mut self) -> R<()> {
        if self.get_next_token() != ";" {
            return self.throw("Semicolon expected.");
        }
        Ok(())
    }

    /// XFileParser::CheckForSeparator
    fn check_for_separator(&mut self) -> R<()> {
        let token = self.get_next_token();
        if token != "," && token != ";" {
            return self.throw("Separator character (';' or ',') expected.");
        }
        Ok(())
    }

    /// XFileParser::TestForSeparator
    fn test_for_separator(&mut self) {
        self.find_next_none_white_space();
        if self.p >= self.end {
            return;
        }
        if self.at(self.p) == b';' || self.at(self.p) == b',' {
            self.p += 1;
        }
    }

    /// XFileParser::ReadInt (text format)
    fn read_int(&mut self) -> R<u32> {
        self.find_next_none_white_space();
        let mut is_negative = false;
        if self.at(self.p) == b'-' {
            is_negative = true;
            self.p += 1;
        }
        if !self.at(self.p).is_ascii_digit() {
            return self.throw("Number expected.");
        }
        let mut number: u32 = 0;
        while self.p < self.end {
            if !self.at(self.p).is_ascii_digit() {
                break;
            }
            number = number
                .wrapping_mul(10)
                .wrapping_add(u32::from(self.at(self.p) - 48));
            self.p += 1;
        }
        self.check_for_separator()?;
        Ok(if is_negative {
            (number as i32).wrapping_neg() as u32
        } else {
            number
        })
    }

    /// XFileParser::ReadFloat (text format)
    fn read_float(&mut self) -> R<f32> {
        self.find_next_none_white_space();
        let rest = self.buf.get(self.p..).unwrap_or(&[]);
        if rest.starts_with(b"-1.#IND00") || rest.starts_with(b"1.#IND00") {
            self.p += 9;
            self.check_for_separator()?;
            return Ok(0.0);
        } else if rest.starts_with(b"1.#QNAN0") {
            self.p += 8;
            self.check_for_separator()?;
            return Ok(0.0);
        }
        match fast_atoreal_move(self.buf, self.p) {
            Ok((v, next)) => {
                self.p = next;
                self.check_for_separator()?;
                Ok(v)
            }
            Err(e) => Err(e),
        }
    }

    fn read_vector2(&mut self) -> R<[f32; 2]> {
        let x = self.read_float()?;
        let y = self.read_float()?;
        self.test_for_separator();
        Ok([x, y])
    }

    fn read_vector3(&mut self) -> R<Vec3> {
        let x = self.read_float()?;
        let y = self.read_float()?;
        let z = self.read_float()?;
        self.test_for_separator();
        Ok(Vec3::new(x, y, z))
    }

    fn read_rgb(&mut self) -> R<[f32; 3]> {
        let r = self.read_float()?;
        let g = self.read_float()?;
        let b = self.read_float()?;
        self.test_for_separator();
        Ok([r, g, b])
    }

    fn read_rgba(&mut self) -> R<[f32; 4]> {
        let r = self.read_float()?;
        let g = self.read_float()?;
        let b = self.read_float()?;
        let a = self.read_float()?;
        self.test_for_separator();
        Ok([r, g, b, a])
    }

    /// The sixteen floats of a matrix in file order: `a1 b1 c1 d1 a2 b2 ...`.
    fn read_matrix(&mut self) -> R<Mat4> {
        Ok(Mat4 {
            a1: self.read_float()?,
            b1: self.read_float()?,
            c1: self.read_float()?,
            d1: self.read_float()?,
            a2: self.read_float()?,
            b2: self.read_float()?,
            c2: self.read_float()?,
            d2: self.read_float()?,
            a3: self.read_float()?,
            b3: self.read_float()?,
            c3: self.read_float()?,
            d3: self.read_float()?,
            a4: self.read_float()?,
            b4: self.read_float()?,
            c4: self.read_float()?,
            d4: self.read_float()?,
        })
    }

    /// XFileParser::ParseFile
    fn parse_file(&mut self, scene: &mut XScene) -> R<()> {
        loop {
            let object_name = self.get_next_token();
            if object_name.is_empty() {
                break;
            }
            match object_name.as_str() {
                "template" => self.parse_data_object_template()?,
                "Frame" => self.parse_data_object_frame(scene, None)?,
                "Mesh" => {
                    let mut mesh = XMesh::default();
                    self.parse_data_object_mesh(&mut mesh)?;
                    scene.global_meshes.push(mesh);
                }
                "AnimTicksPerSecond" => self.parse_data_object_anim_ticks_per_second(scene)?,
                "AnimationSet" => self.parse_data_object_animation_set(scene)?,
                "Material" => {
                    let mut material = XMaterial::default();
                    self.parse_data_object_material(&mut material)?;
                    scene.global_materials.push(material);
                }
                "}" => {}
                _ => self.parse_unknown_data_object()?,
            }
        }
        Ok(())
    }

    /// XFileParser::ParseDataObjectTemplate
    fn parse_data_object_template(&mut self) -> R<()> {
        self.read_head_of_data_object()?;
        let _guid = self.get_next_token();
        loop {
            let s = self.get_next_token();
            if s == "}" {
                break;
            }
            if s.is_empty() {
                return self
                    .throw("Unexpected end of file reached while parsing template definition");
            }
        }
        Ok(())
    }

    /// XFileParser::ParseDataObjectFrame: the node is parsed on its own and attached by the caller.
    fn parse_data_object_frame(&mut self, scene: &mut XScene, parent: Option<&mut XNode>) -> R<()> {
        let name = self.read_head_of_data_object()?;
        let mut node = XNode::new();
        node.name = name.clone();
        loop {
            let object_name = self.get_next_token();
            if object_name.is_empty() {
                return self.throw("Unexpected end of file reached while parsing frame");
            }
            match object_name.as_str() {
                "}" => break,
                "Frame" => self.parse_data_object_frame(scene, Some(&mut node))?,
                "FrameTransformMatrix" => {
                    self.read_head_of_data_object()?;
                    node.trafo_matrix = self.read_matrix()?;
                    self.check_for_semicolon()?;
                    self.check_for_closing_brace()?;
                }
                "Mesh" => {
                    let mut mesh = XMesh {
                        name: name.clone(),
                        ..XMesh::default()
                    };
                    self.parse_data_object_mesh(&mut mesh)?;
                    node.meshes.push(mesh);
                }
                _ => self.parse_unknown_data_object()?,
            }
        }
        match parent {
            Some(p) => p.children.push(node),
            None => match scene.root_node.take() {
                Some(root) => {
                    let mut root = if root.name != "$dummy_root" {
                        let mut dummy = XNode::new();
                        dummy.name = "$dummy_root".to_string();
                        dummy.children.push(root);
                        dummy
                    } else {
                        root
                    };
                    root.children.push(node);
                    scene.root_node = Some(root);
                }
                None => scene.root_node = Some(node),
            },
        }
        Ok(())
    }

    /// XFileParser::ParseDataObjectMesh
    fn parse_data_object_mesh(&mut self, mesh: &mut XMesh) -> R<()> {
        self.read_head_of_data_object()?;
        let num_vertices = self.read_int()?;
        mesh.positions = Vec::with_capacity((num_vertices as usize).min(1 << 20));
        for _ in 0..num_vertices {
            let v = self.read_vector3()?;
            mesh.positions.push(v);
        }
        let num_pos_faces = self.read_int()?;
        mesh.pos_faces = Vec::with_capacity((num_pos_faces as usize).min(1 << 20));
        for a in 0..num_pos_faces {
            let num_indices = self.read_int()?;
            if num_indices < 3 {
                return self.throw(&format!(
                    "Invalid index count {} for face {}.",
                    num_indices, a
                ));
            }
            let mut face = Vec::new();
            for _ in 0..num_indices {
                face.push(self.read_int()?);
            }
            mesh.pos_faces.push(face);
            self.test_for_separator();
        }
        loop {
            let object_name = self.get_next_token();
            if object_name.is_empty() {
                return self.throw("Unexpected end of file while parsing mesh structure");
            }
            match object_name.as_str() {
                "}" => break,
                "MeshNormals" => self.parse_data_object_mesh_normals(mesh)?,
                "MeshTextureCoords" => self.parse_data_object_mesh_texture_coords(mesh)?,
                "MeshVertexColors" => self.parse_data_object_mesh_vertex_colors(mesh)?,
                "MeshMaterialList" => self.parse_data_object_mesh_material_list(mesh)?,
                "VertexDuplicationIndices" => self.parse_unknown_data_object()?,
                "DeclData" => self.parse_data_object_decl_data(mesh)?,
                "XSkinMeshHeader" => {
                    self.read_head_of_data_object()?;
                    self.read_int()?;
                    self.read_int()?;
                    self.read_int()?;
                    self.check_for_closing_brace()?;
                }
                "SkinWeights" => self.parse_data_object_skin_weights(mesh)?,
                _ => self.parse_unknown_data_object()?,
            }
        }
        Ok(())
    }

    /// The `DeclData` object (D3DX vertex declaration with the vertex data as DWORDs): the texture coordinate elements
    /// (usage 5) become texture coordinate sets; the normals and tangents it carries are not used.
    fn parse_data_object_decl_data(&mut self, mesh: &mut XMesh) -> R<()> {
        self.read_head_of_data_object()?;
        let num_elements = self.read_int()?;
        let mut elements: Vec<(u32, u32, u32, u32)> = Vec::new();
        for _ in 0..num_elements {
            let ty = self.read_int()?;
            let method = self.read_int()?;
            let usage = self.read_int()?;
            let usage_index = self.read_int()?;
            self.test_for_separator();
            elements.push((ty, method, usage, usage_index));
        }
        let num_dwords = self.read_int()?;
        let mut data: Vec<u32> = Vec::new();
        for _ in 0..num_dwords {
            data.push(self.read_int()?);
        }
        self.check_for_closing_brace()?;
        // D3DDECLTYPE sizes in DWORDs
        let size = |ty: u32| -> Option<usize> {
            Some(match ty {
                0 | 4..=6 | 8..=11 | 13..=15 => 1,
                1 | 7 | 12 | 16 => 2,
                2 => 3,
                3 => 4,
                _ => return None,
            })
        };
        let mut stride = 0usize;
        for e in &elements {
            stride += size(e.0).ok_or_else(|| "Unknown DeclData element type".to_string())?;
        }
        if stride == 0 {
            return Ok(());
        }
        let num_vertices = data.len() / stride;
        let mut offset = 0usize;
        for e in &elements {
            if e.2 == 5 && e.0 == 1 {
                if mesh.num_textures + 1 > 8 {
                    return self.throw("Too many sets of texture coordinates");
                }
                let mut coords = Vec::with_capacity(num_vertices);
                for v in 0..num_vertices {
                    let base = v * stride + offset;
                    coords.push([f32::from_bits(data[base]), f32::from_bits(data[base + 1])]);
                }
                mesh.tex_coords[mesh.num_textures] = coords;
                mesh.num_textures += 1;
            }
            offset += size(e.0).unwrap_or(0);
        }
        Ok(())
    }

    /// XFileParser::ParseDataObjectSkinWeights
    fn parse_data_object_skin_weights(&mut self, mesh: &mut XMesh) -> R<()> {
        self.read_head_of_data_object()?;
        let name = self.get_next_token_as_string()?;
        let num_weights = self.read_int()?;
        let mut weights: Vec<(u32, f32)> = Vec::new();
        for _ in 0..num_weights {
            let v = self.read_int()?;
            weights.push((v, 0.0));
        }
        if num_weights == 0 {
            // an empty array is just its terminating ';'
            self.test_for_separator();
        }
        for w in weights.iter_mut() {
            w.1 = self.read_float()?;
        }
        if num_weights == 0 {
            self.test_for_separator();
        }
        let offset_matrix = self.read_matrix()?;
        self.check_for_semicolon()?;
        self.check_for_closing_brace()?;
        mesh.bones.push(XBone {
            name,
            weights,
            offset_matrix,
        });
        Ok(())
    }

    /// XFileParser::ParseDataObjectMeshNormals
    fn parse_data_object_mesh_normals(&mut self, mesh: &mut XMesh) -> R<()> {
        self.read_head_of_data_object()?;
        let num_normals = self.read_int()?;
        mesh.normals = Vec::new();
        for _ in 0..num_normals {
            let v = self.read_vector3()?;
            mesh.normals.push(v);
        }
        let num_faces = self.read_int()?;
        if num_faces as usize != mesh.pos_faces.len() {
            return self.throw("Normal face count does not match vertex face count.");
        }
        if num_faces > 0 {
            mesh.norm_faces = Vec::new();
            for _ in 0..num_faces {
                let num_indices = self.read_int()?;
                let mut face = Vec::new();
                for _ in 0..num_indices {
                    face.push(self.read_int()?);
                }
                mesh.norm_faces.push(face);
                self.test_for_separator();
            }
        }
        self.check_for_closing_brace()
    }

    /// XFileParser::ParseDataObjectMeshTextureCoords
    fn parse_data_object_mesh_texture_coords(&mut self, mesh: &mut XMesh) -> R<()> {
        self.read_head_of_data_object()?;
        if mesh.num_textures + 1 > 8 {
            return self.throw("Too many sets of texture coordinates");
        }
        let set = mesh.num_textures;
        mesh.num_textures += 1;
        let num_coords = self.read_int()?;
        if num_coords as usize != mesh.positions.len() {
            return self.throw("Texture coord count does not match vertex count");
        }
        let mut coords = Vec::new();
        for _ in 0..num_coords {
            coords.push(self.read_vector2()?);
        }
        mesh.tex_coords[set] = coords;
        self.check_for_closing_brace()
    }

    /// XFileParser::ParseDataObjectMeshVertexColors
    fn parse_data_object_mesh_vertex_colors(&mut self, mesh: &mut XMesh) -> R<()> {
        self.read_head_of_data_object()?;
        if mesh.num_color_sets + 1 > 8 {
            return self.throw("Too many colorsets");
        }
        let set = mesh.num_color_sets;
        mesh.num_color_sets += 1;
        let num_colors = self.read_int()?;
        if num_colors as usize != mesh.positions.len() {
            return self.throw("Vertex color count does not match vertex count");
        }
        let mut colors = vec![[0.0f32, 0.0, 0.0, 1.0]; num_colors as usize];
        for _ in 0..num_colors {
            let index = self.read_int()?;
            if index as usize >= mesh.positions.len() {
                return self.throw("Vertex color index out of bounds");
            }
            colors[index as usize] = self.read_rgba()?;
            // HACK: (thom) Maxon Cinema XPort plugin puts a third separator here, kwxPort puts a comma.
            // Ignore gracefully.
            if self.at(self.p) == b';' || self.at(self.p) == b',' {
                self.p += 1;
            }
        }
        mesh.colors[set] = colors;
        self.check_for_closing_brace()
    }

    /// XFileParser::ParseDataObjectMeshMaterialList
    fn parse_data_object_mesh_material_list(&mut self, mesh: &mut XMesh) -> R<()> {
        self.read_head_of_data_object()?;
        self.read_int()?;
        let num_mat_indices = self.read_int()?;
        if num_mat_indices as usize != mesh.pos_faces.len() && num_mat_indices != 1 {
            return self.throw("Per-Face material index count does not match face count.");
        }
        for _ in 0..num_mat_indices {
            let v = self.read_int()?;
            mesh.face_materials.push(v);
        }
        if self.p < self.end && self.at(self.p) == b';' {
            self.p += 1;
        }
        while mesh.face_materials.len() < mesh.pos_faces.len() {
            let first = *mesh
                .face_materials
                .first()
                .ok_or_else(|| "Line: face material list is empty".to_string())?;
            mesh.face_materials.push(first);
        }
        loop {
            let object_name = self.get_next_token();
            if object_name.is_empty() {
                return self.throw("Unexpected end of file while parsing mesh material list.");
            }
            match object_name.as_str() {
                "}" => break,
                "{" => {
                    let mat_name = self.get_next_token();
                    mesh.materials.push(XMaterial {
                        name: mat_name,
                        is_reference: true,
                        ..XMaterial::default()
                    });
                    self.check_for_closing_brace()?;
                }
                "Material" => {
                    let mut m = XMaterial::default();
                    self.parse_data_object_material(&mut m)?;
                    mesh.materials.push(m);
                }
                ";" => {}
                _ => self.parse_unknown_data_object()?,
            }
        }
        Ok(())
    }

    /// XFileParser::ParseDataObjectMaterial
    fn parse_data_object_material(&mut self, material: &mut XMaterial) -> R<()> {
        let mut mat_name = self.read_head_of_data_object()?;
        if mat_name.is_empty() {
            mat_name = format!("material{}", self.line_number);
        }
        material.name = mat_name;
        material.is_reference = false;
        material.diffuse = self.read_rgba()?;
        material.specular_exponent = self.read_float()?;
        material.specular = self.read_rgb()?;
        material.emissive = self.read_rgb()?;
        loop {
            let object_name = self.get_next_token();
            if object_name.is_empty() {
                return self.throw("Unexpected end of file while parsing mesh material");
            }
            match object_name.as_str() {
                "}" => break,
                "TextureFilename" | "TextureFileName" => {
                    let t = self.parse_data_object_texture_filename()?;
                    material.textures.push((t, false));
                }
                "NormalmapFilename" | "NormalmapFileName" => {
                    let t = self.parse_data_object_texture_filename()?;
                    material.textures.push((t, true));
                }
                _ => self.parse_unknown_data_object()?,
            }
        }
        Ok(())
    }

    /// XFileParser::ParseDataObjectTextureFilename
    fn parse_data_object_texture_filename(&mut self) -> R<String> {
        self.read_head_of_data_object()?;
        let mut name = self.get_next_token_as_string()?;
        self.check_for_closing_brace()?;
        while let Some(pos) = name.find("\\\\") {
            name.replace_range(pos..pos + 2, "\\");
        }
        Ok(name)
    }

    /// XFileParser::ParseDataObjectAnimTicksPerSecond
    fn parse_data_object_anim_ticks_per_second(&mut self, scene: &mut XScene) -> R<()> {
        self.read_head_of_data_object()?;
        scene.anim_ticks_per_second = self.read_int()?;
        self.check_for_closing_brace()
    }

    /// XFileParser::ParseDataObjectAnimationSet
    fn parse_data_object_animation_set(&mut self, scene: &mut XScene) -> R<()> {
        let anim_name = self.read_head_of_data_object()?;
        let mut anim = XAnimation {
            name: anim_name,
            anims: Vec::new(),
        };
        loop {
            let object_name = self.get_next_token();
            if object_name.is_empty() {
                return self.throw("Unexpected end of file while parsing animation set.");
            }
            match object_name.as_str() {
                "}" => break,
                "Animation" => self.parse_data_object_animation(&mut anim)?,
                _ => self.parse_unknown_data_object()?,
            }
        }
        scene.anims.push(anim);
        Ok(())
    }

    /// XFileParser::ParseDataObjectAnimation
    fn parse_data_object_animation(&mut self, anim: &mut XAnimation) -> R<()> {
        self.read_head_of_data_object()?;
        let mut banim = XAnimBone::default();
        loop {
            let object_name = self.get_next_token();
            if object_name.is_empty() {
                return self.throw("Unexpected end of file while parsing animation.");
            }
            match object_name.as_str() {
                "}" => break,
                "AnimationKey" => self.parse_data_object_animation_key(&mut banim)?,
                "AnimationOptions" => self.parse_unknown_data_object()?,
                "{" => {
                    banim.bone_name = self.get_next_token();
                    self.check_for_closing_brace()?;
                }
                _ => self.parse_unknown_data_object()?,
            }
        }
        anim.anims.push(banim);
        Ok(())
    }

    /// XFileParser::ParseDataObjectAnimationKey
    fn parse_data_object_animation_key(&mut self, bone: &mut XAnimBone) -> R<()> {
        self.read_head_of_data_object()?;
        let key_type = self.read_int()?;
        let num_keys = self.read_int()?;
        for _ in 0..num_keys {
            let time = f64::from(self.read_int()?);
            match key_type {
                0 => {
                    if self.read_int()? != 4 {
                        return self
                            .throw("Invalid number of arguments for quaternion key in animation");
                    }
                    let w = self.read_float()?;
                    let x = self.read_float()?;
                    let y = self.read_float()?;
                    let z = self.read_float()?;
                    bone.rot_keys.push((time, [w, x, y, z]));
                    self.check_for_semicolon()?;
                }
                1 | 2 => {
                    if self.read_int()? != 3 {
                        return self
                            .throw("Invalid number of arguments for vector key in animation");
                    }
                    let v = self.read_vector3()?;
                    if key_type == 2 {
                        bone.pos_keys.push((time, v));
                    } else {
                        bone.scale_keys.push((time, v));
                    }
                }
                3 | 4 => {
                    if self.read_int()? != 16 {
                        return self
                            .throw("Invalid number of arguments for matrix key in animation");
                    }
                    let m = self.read_matrix()?;
                    bone.trafo_keys.push((time, m));
                    self.check_for_semicolon()?;
                }
                _ => {
                    return self.throw(&format!("Unknown key type {} in animation.", key_type));
                }
            }
            self.check_for_separator()?;
        }
        self.check_for_closing_brace()
    }

    /// XFileParser::ParseUnknownDataObject
    fn parse_unknown_data_object(&mut self) -> R<()> {
        loop {
            let t = self.get_next_token();
            if t.is_empty() {
                return self.throw("Unexpected end of file while parsing unknown segment.");
            }
            if t == "{" {
                break;
            }
        }
        let mut counter = 1u32;
        while counter > 0 {
            let t = self.get_next_token();
            if t.is_empty() {
                return self.throw("Unexpected end of file while parsing unknown segment.");
            }
            if t == "{" {
                counter += 1;
            } else if t == "}" {
                counter -= 1;
            }
        }
        Ok(())
    }
}

/// XFileParser::FilterHierarchy: a node whose only child is an unnamed node holding meshes takes them (and its matrix).
fn filter_hierarchy(node: &mut XNode) {
    if node.children.len() == 1 && node.meshes.is_empty() {
        let child_name_empty = node.children[0].name.is_empty();
        let child_has_meshes = !node.children[0].meshes.is_empty();
        if child_name_empty && child_has_meshes {
            let child = node.children.remove(0);
            node.meshes.extend(child.meshes);
            node.trafo_matrix = node.trafo_matrix.mul(&child.trafo_matrix);
        }
    }
    for c in node.children.iter_mut() {
        filter_hierarchy(c);
    }
}

/// XFileParser::XFileParser(buffer) and GetImportedData: `Err` is the DeadlyImportError text.
pub fn parse(buffer: &[u8]) -> Result<XScene, String> {
    if buffer.len() < 16 {
        return Err("XFile is too small.".to_string());
    }
    let mut buf = buffer.to_vec();
    buf.push(0);
    if !buf.starts_with(b"xof ") {
        return Err("Header mismatch, file is not an XFile.".to_string());
    }
    if &buf[8..12] != b"txt " {
        return Err("Unsupported X file format (only the text format is ported).".to_string());
    }
    let float_size = u32::from(buf[12] - 48) * 1000
        + u32::from(buf[13] - 48) * 100
        + u32::from(buf[14] - 48) * 10
        + u32::from(buf[15] - 48);
    if float_size != 32 && float_size != 64 {
        return Err("Unknown float size, expected 32 or 64.".to_string());
    }
    let end = buf.len() - 1;
    let mut scene = XScene {
        root_node: None,
        global_meshes: Vec::new(),
        global_materials: Vec::new(),
        anims: Vec::new(),
        anim_ticks_per_second: 0,
    };
    let mut p = Parser {
        buf: &buf,
        p: 16,
        end,
        line_number: 0,
    };
    p.parse_file(&mut scene)?;
    if let Some(root) = scene.root_node.as_mut() {
        filter_hierarchy(root);
    }
    Ok(scene)
}
