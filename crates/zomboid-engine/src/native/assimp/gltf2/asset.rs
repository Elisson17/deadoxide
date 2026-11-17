//! assimp's `glTF2Asset.h` / `glTF2Asset.inl`: the binary container (`Asset::ReadBinaryHeader`), the dictionaries of the
//! JSON document (`Asset::Load`, `LazyDict<T>::Retrieve`, the `Read` of each object) and the accessors
//! (`Accessor::ExtractData`, `Accessor::Indexer`).
//!
//! Only what the importer needs is kept. A feature the library handles but the game's files never use is refused with an
//! `UNPORTED` error instead of being guessed (sparse accessors, extensions, morph targets, cameras, lights, external
//! buffers, textures other than the base colour).
use super::json::{self, Json};

pub type R<T> = Result<T, String>;

fn unported<T>(what: &str) -> R<T> {
    Err(format!("UNPORTED glTF2: {what}"))
}

/// `glTF2::AttribType::Value` / `AttribTypeTraits`: the component count of an accessor type.
pub fn num_components(ty: &str) -> Option<usize> {
    match ty {
        "SCALAR" => Some(1),
        "VEC2" => Some(2),
        "VEC3" => Some(3),
        "VEC4" | "MAT2" => Some(4),
        "MAT3" => Some(9),
        "MAT4" => Some(16),
        _ => None,
    }
}

/// `glTF2::ComponentTypeSize`; `None` for a type the library rejects ("GLTF: Unsupported Component Type").
pub fn component_size(component_type: u32) -> Option<usize> {
    match component_type {
        5120 | 5121 => Some(1),
        5122 | 5123 => Some(2),
        5125 | 5126 => Some(4),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct BufferView {
    pub buffer: usize,
    pub byte_offset: usize,
    pub byte_length: usize,
    pub byte_stride: usize,
}

#[derive(Clone, Debug)]
pub struct Accessor {
    pub id: String,
    pub name: String,
    pub buffer_view: Option<usize>,
    pub byte_offset: usize,
    pub component_type: u32,
    pub count: usize,
    pub ty: String,
}

#[derive(Clone, Debug)]
pub struct Image {
    pub uri: Option<String>,
    pub buffer_view: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Sampler {
    pub id: String,
    pub name: String,
    pub mag_filter: Option<u32>,
    pub min_filter: Option<u32>,
    pub wrap_s: Option<u32>,
    pub wrap_t: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct Texture {
    pub sampler: Option<usize>,
    pub source: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct TextureInfo {
    pub index: usize,
    pub tex_coord: u32,
}

#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    pub base_color_factor: [f32; 4],
    pub base_color_texture: Option<TextureInfo>,
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub emissive_factor: [f32; 3],
    pub double_sided: bool,
    pub alpha_mode: String,
    pub alpha_cutoff: f32,
}

impl Default for Material {
    /// `glTF2::Material::SetDefaults`
    fn default() -> Self {
        Material {
            name: String::new(),
            base_color_factor: [1.0, 1.0, 1.0, 1.0],
            base_color_texture: None,
            metallic_factor: 1.0,
            roughness_factor: 1.0,
            emissive_factor: [0.0, 0.0, 0.0],
            double_sided: false,
            alpha_mode: "OPAQUE".to_string(),
            alpha_cutoff: 0.5,
        }
    }
}

/// `Mesh::Primitive::Attributes`: the accessor of each attribute set (a gap in the numbering stays `None`).
#[derive(Clone, Debug, Default)]
pub struct Attributes {
    pub position: Vec<Option<usize>>,
    pub normal: Vec<Option<usize>>,
    pub tangent: Vec<Option<usize>>,
    pub texcoord: Vec<Option<usize>>,
    pub color: Vec<Option<usize>>,
    pub joint: Vec<Option<usize>>,
    pub weight: Vec<Option<usize>>,
}

#[derive(Clone, Debug)]
pub struct Primitive {
    pub mode: u32,
    pub attributes: Attributes,
    pub indices: Option<usize>,
    pub material: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Mesh {
    pub id: String,
    pub name: String,
    pub primitives: Vec<Primitive>,
}

#[derive(Clone, Debug)]
pub struct Skin {
    pub inverse_bind_matrices: Option<usize>,
    pub joints: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub children: Vec<usize>,
    pub meshes: Vec<usize>,
    pub skin: Option<usize>,
    pub matrix: Option<[f32; 16]>,
    pub translation: Option<[f32; 3]>,
    pub rotation: Option<[f32; 4]>,
    pub scale: Option<[f32; 3]>,
}

/// `AnimationSampler::interpolation`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interpolation {
    Linear,
    Step,
    CubicSpline,
}

#[derive(Clone, Debug)]
pub struct AnimSampler {
    pub input: Option<usize>,
    pub output: Option<usize>,
    pub interpolation: Interpolation,
}

/// `AnimationTarget::path`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Path {
    Translation,
    Rotation,
    Scale,
    Weights,
}

#[derive(Clone, Debug)]
pub struct Channel {
    pub sampler: i64,
    pub node: usize,
    pub path: Path,
}

#[derive(Clone, Debug)]
pub struct Animation {
    pub name: String,
    pub samplers: Vec<AnimSampler>,
    pub channels: Vec<Channel>,
}

/// The loaded asset (`glTF2::Asset` after `Load`).
#[derive(Debug, Default)]
pub struct Asset {
    /// the data of every buffer (the binary chunk for the buffer without a URI)
    pub buffers: Vec<Vec<u8>>,
    pub buffer_views: Vec<BufferView>,
    pub accessors: Vec<Accessor>,
    pub images: Vec<Image>,
    pub samplers: Vec<Sampler>,
    pub textures: Vec<Texture>,
    pub materials: Vec<Material>,
    pub meshes: Vec<Mesh>,
    pub skins: Vec<Skin>,
    pub nodes: Vec<Node>,
    pub animations: Vec<Animation>,
    /// the nodes of the default scene (`Asset::scene`), `None` when the document has no scene
    pub scene_nodes: Option<Vec<usize>>,
}

/// `Asset::ReadBinaryHeader`: the JSON text and the binary chunk of a `.glb` (`GLB_Header`, `GLB_Chunk`).
pub fn read_glb(bytes: &[u8]) -> R<(&[u8], &[u8])> {
    let u32_at = |o: usize| -> R<u32> {
        bytes
            .get(o..o + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| "GLTF: Unable to read the glTF header".to_string())
    };
    if bytes.get(0..4) != Some(b"glTF") {
        return Err("GLTF: Bad binary header".to_string());
    }
    let version = u32_at(4)?;
    if version != 2 {
        return unported(&format!(
            "binary glTF version {version} (the glTF1 importer is not ported)"
        ));
    }
    let length = u32_at(8)? as usize;
    if length > bytes.len() {
        return Err("GLTF: Header length mismatch".to_string());
    }
    let bytes = &bytes[..length];
    let json_len = u32_at(12)? as usize;
    if u32_at(16)? != 0x4E4F_534A {
        return Err("GLTF: JSON chunk missing".to_string());
    }
    let json_end = 20usize
        .checked_add(json_len)
        .filter(|e| *e <= bytes.len())
        .ok_or_else(|| "GLTF: JSON chunk length".to_string())?;
    let text = &bytes[20..json_end];
    // mBodyOffset = (header + chunk header + scene length + 3) & ~3: the next chunk header
    let body = (json_end + 3) & !3;
    let mut bin: &[u8] = &[];
    if body + 8 <= bytes.len() {
        let bin_len = u32_at(body)? as usize;
        let bin_type = u32_at(body + 4)?;
        if bin_type == 0x004E_4942 {
            let end = (body + 8)
                .checked_add(bin_len)
                .filter(|e| *e <= bytes.len())
                .ok_or_else(|| "GLTF: BIN chunk length".to_string())?;
            bin = &bytes[body + 8..end];
        }
    }
    Ok((text, bin))
}

fn uint_member(obj: &Json, name: &str) -> Option<u32> {
    obj.get(name).and_then(Json::as_uint)
}

fn string_member(obj: &Json, name: &str) -> String {
    obj.get(name)
        .and_then(Json::as_str)
        .unwrap_or("")
        .to_string()
}

/// `ReadValue(Value&, vec<N>&)`: an array of exactly N numbers.
fn float_array<const N: usize>(obj: &Json, name: &str) -> Option<[f32; N]> {
    let arr = obj.get(name)?.as_array()?;
    if arr.len() != N {
        return None;
    }
    let mut out = [0.0f32; N];
    for (o, v) in out.iter_mut().zip(arr) {
        if let Some(f) = v.as_f32() {
            *o = f;
        }
    }
    Some(out)
}

fn dict<'a>(doc: &'a Json, name: &str) -> &'a [Json] {
    doc.get(name).and_then(Json::as_array).unwrap_or(&[])
}

fn index_in(len: usize, what: &str, i: u32) -> R<usize> {
    let i = i as usize;
    if i < len {
        Ok(i)
    } else {
        Err(format!(
            "GLTF: Object at index \"{i}\" is not found in {what}"
        ))
    }
}

impl Asset {
    /// `Asset::Load` for a binary container: the objects of the document, loaded lazily in the order the library
    /// retrieves them (see [`Loader`]).
    pub fn load(doc: &Json, body: &[u8]) -> R<Asset> {
        Loader::new(doc).run(body)
    }

    /// `Accessor::GetStride`: the buffer view's `byteStride`, else the size of one element.
    pub fn stride(&self, acc: &Accessor) -> R<usize> {
        let elem = self.element_size(acc)?;
        Ok(match acc.buffer_view {
            Some(bv) if self.buffer_views[bv].byte_stride != 0 => self.buffer_views[bv].byte_stride,
            _ => elem,
        })
    }

    /// `Accessor::GetElementSize`: components times the component size.
    pub fn element_size(&self, acc: &Accessor) -> R<usize> {
        let comp = component_size(acc.component_type)
            .ok_or_else(|| format!("GLTF: Unsupported Component Type {}", acc.component_type))?;
        Ok(num_components(&acc.ty).unwrap_or(1) * comp)
    }

    /// `Accessor::GetMaxByteSize`: the byte length of the buffer view.
    fn max_byte_size(&self, acc: &Accessor) -> usize {
        acc.buffer_view
            .map_or(0, |bv| self.buffer_views[bv].byte_length)
    }

    /// `Accessor::GetPointer`: the bytes the accessor starts at, up to the end of its buffer (`None`: no data).
    fn data(&self, acc: &Accessor) -> Option<&[u8]> {
        let bv = &self.buffer_views[acc.buffer_view?];
        let buffer = self.buffers.get(bv.buffer)?;
        let start = bv.byte_offset.checked_add(acc.byte_offset)?;
        buffer.get(start..)
    }

    /// `Accessor::ExtractData<T>(out, remappingIndices)`: the elements as packed bytes (`element_size` each), either
    /// all `count` of them or the ones the remapping table lists, in its order. `target` is `sizeof(T)`.
    pub fn extract(&self, acc_index: usize, remap: Option<&[u32]>, target: usize) -> R<Vec<u8>> {
        let acc = &self.accessors[acc_index];
        let ctx = context(&acc.id, &acc.name);
        let data = self
            .data(acc)
            .ok_or_else(|| format!("GLTF2: data is null when extracting data from {ctx}"))?;
        let elem = self.element_size(acc)?;
        let stride = self.stride(acc)?;
        if elem > target {
            return Err(format!(
                "GLTF: elemSize {elem} > targetElemSize {target} in {ctx}"
            ));
        }
        let max_size = self.max_byte_size(acc);
        let used = remap.map_or(acc.count, <[u32]>::len);
        let mut out = Vec::with_capacity(used * elem);
        let beyond = || format!("GLTF: the data of {ctx} lies outside its buffer");
        match remap {
            None => {
                if stride.saturating_mul(acc.count) > max_size {
                    return Err(format!(
                        "GLTF: count*stride {} > maxSize {max_size} in {ctx}",
                        stride.saturating_mul(acc.count)
                    ));
                }
                for i in 0..acc.count {
                    let o = i * stride;
                    out.extend_from_slice(data.get(o..o + elem).ok_or_else(beyond)?);
                }
            }
            Some(list) => {
                let max_index = (max_size / stride.max(1)) as i64 - 1;
                for &src in list {
                    if max_index < i64::from(src) && max_index >= 0 {
                        return Err(format!(
                            "GLTF: index*stride {} > maxSize {max_size} in {ctx}",
                            stride * src as usize
                        ));
                    }
                    let o = src as usize * stride;
                    out.extend_from_slice(data.get(o..o + elem).ok_or_else(beyond)?);
                }
            }
        }
        Ok(out)
    }

    /// `Accessor::Indexer` + `GetUInt(i)` over the whole accessor: every index, read as the first `min(size, 4)` bytes
    /// of the element.
    pub fn indices(&self, acc_index: usize, mesh_context: &str) -> R<Vec<u32>> {
        let acc = &self.accessors[acc_index];
        let data = self
            .data(acc)
            .ok_or_else(|| format!("GLTF: Invalid accessor without data in mesh {mesh_context}"))?;
        let elem = self.element_size(acc)?;
        let stride = self.stride(acc)?;
        let max_size = self.max_byte_size(acc);
        let mut out = Vec::with_capacity(acc.count);
        for i in 0..acc.count {
            if max_size <= i * stride {
                return Err(format!(
                    "GLTF: Invalid index {i}, count out of range for buffer with stride {stride} and size {max_size}."
                ));
            }
            let mut b = [0u8; 4];
            let n = elem.min(4);
            let src = data.get(i * stride..i * stride + n).ok_or_else(|| {
                format!("GLTF: the indices of {} lie outside their buffer", acc.id)
            })?;
            b[..n].copy_from_slice(src);
            out.push(u32::from_le_bytes(b));
        }
        Ok(out)
    }
}

/// `glTF2::getContextForErrorMessages(id, name)`
pub fn context(id: &str, name: &str) -> String {
    if name.is_empty() {
        id.to_string()
    } else {
        format!("{id} (\"{name}\")")
    }
}

impl Attributes {
    /// `GetAttribVector`: the list an attribute name belongs to (by prefix) and the length of that prefix.
    fn list_for(&mut self, name: &str) -> Option<(&mut Vec<Option<usize>>, usize)> {
        let table: [(&str, usize); 7] = [
            ("POSITION", 0),
            ("NORMAL", 1),
            ("TANGENT", 2),
            ("TEXCOORD", 3),
            ("COLOR", 4),
            ("JOINTS", 5),
            ("WEIGHTS", 6),
        ];
        for (prefix, which) in table {
            if name.starts_with(prefix) {
                let list = match which {
                    0 => &mut self.position,
                    1 => &mut self.normal,
                    2 => &mut self.tangent,
                    3 => &mut self.texcoord,
                    4 => &mut self.color,
                    5 => &mut self.joint,
                    _ => &mut self.weight,
                };
                return Some((list, prefix.len()));
            }
        }
        None
    }
}

/// Parse the JSON chunk of a binary glTF.
pub fn parse_document(text: &[u8]) -> R<Json> {
    json::parse(text)
}

/// The part of `glTF2::LazyDict<T>` the importer's output depends on. A dictionary object is created when something
/// first refers to it (`LazyDict::Retrieve`), and it is appended to the dictionary after its own `Read` returned, so
/// a node's children and everything they refer to come before the node. `Asset::Load` starts from the default scene,
/// then retrieves every skin and every animation. The index of an object in its dictionary (`Ref::index`) is its position
/// in that order, not its position in the document: the aiMesh order, the material indices, the numbering of embedded
/// textures and the keys of the `unordered_map` of `GatherSamplers` all follow it. Objects nothing refers to are never
/// loaded.
struct Loader<'a> {
    doc: &'a Json,
    asset: Asset,
    nodes: Slots,
    meshes: Slots,
    materials: Slots,
    skins: Slots,
    textures: Slots,
    images: Slots,
}

/// `mObjsByOIndex` of one dictionary: the document index -> the position in the loaded order (`None` while the object is
/// being read, which only a cycle of children can observe).
#[derive(Default)]
struct Slots(std::collections::HashMap<usize, Option<usize>>);

impl<'a> Loader<'a> {
    fn new(doc: &'a Json) -> Self {
        Loader {
            doc,
            asset: Asset::default(),
            nodes: Slots::default(),
            meshes: Slots::default(),
            materials: Slots::default(),
            skins: Slots::default(),
            textures: Slots::default(),
            images: Slots::default(),
        }
    }

    /// `Asset::Load`
    fn run(mut self, body: &[u8]) -> R<Asset> {
        let doc = self.doc;
        let version = doc
            .get("asset")
            .and_then(|x| x.get("version"))
            .and_then(Json::as_str);
        match version {
            Some(v) if v.starts_with('2') => {}
            Some(_) => return Err("GLTF: Unsupported glTF version".to_string()),
            None => return Err("GLTF: Missing asset.version".to_string()),
        }
        for key in ["extensionsRequired", "extensionsUsed"] {
            if doc
                .get(key)
                .and_then(Json::as_array)
                .is_some_and(|a| !a.is_empty())
            {
                return unported(key);
            }
        }
        self.read_buffers(body)?;
        self.read_buffer_views()?;
        self.read_accessors()?;
        self.read_samplers();
        // the default scene: its nodes, depth first
        let scenes = dict(doc, "scenes");
        // without a "scene" member the first scene is the default one
        let default_scene = match doc.get("scene").and_then(Json::as_uint) {
            Some(si) => Some(si as usize),
            None if !scenes.is_empty() => Some(0),
            None => None,
        };
        if let Some(si) = default_scene {
            let scene = scenes
                .get(si)
                .ok_or_else(|| format!("GLTF: Object at index \"{si}\" is not found in scenes"))?;
            let mut roots = Vec::new();
            for n in scene.get("nodes").and_then(Json::as_array).unwrap_or(&[]) {
                let Some(n) = n.as_uint() else { continue };
                roots.push(self.node(n as usize)?);
            }
            self.asset.scene_nodes = Some(roots);
        }
        for i in 0..dict(doc, "skins").len() {
            self.skin(i)?;
        }
        for i in 0..dict(doc, "animations").len() {
            let anim = self.read_animation(&dict(doc, "animations")[i])?;
            self.asset.animations.push(anim);
        }
        Ok(self.asset)
    }

    /// `Buffer::Read`: the buffer without a URI is the binary chunk (`byteLength` bytes of it).
    fn read_buffers(&mut self, body: &[u8]) -> R<()> {
        for (i, b) in dict(self.doc, "buffers").iter().enumerate() {
            let byte_length = uint_member(b, "byteLength")
                .ok_or_else(|| "GLTF: Buffer without byteLength".to_string())?
                as usize;
            if b.get("uri").is_some() {
                return unported("a buffer with a URI");
            }
            if i != 0 {
                return unported("a second buffer without a URI");
            }
            if byte_length > body.len() {
                return Err(format!(
                    "GLTF: Buffer \"buffers[{i}]\" length {byte_length} exceeds the binary chunk of {} bytes",
                    body.len()
                ));
            }
            self.asset.buffers.push(body[..byte_length].to_vec());
        }
        Ok(())
    }

    /// `BufferView::Read`
    fn read_buffer_views(&mut self) -> R<()> {
        for v in dict(self.doc, "bufferViews") {
            let buffer = index_in(
                self.asset.buffers.len(),
                "buffers",
                uint_member(v, "buffer")
                    .ok_or_else(|| "GLTF: bufferView without buffer".to_string())?,
            )?;
            self.asset.buffer_views.push(BufferView {
                buffer,
                byte_offset: uint_member(v, "byteOffset").unwrap_or(0) as usize,
                byte_length: uint_member(v, "byteLength").unwrap_or(0) as usize,
                byte_stride: uint_member(v, "byteStride").unwrap_or(0) as usize,
            });
        }
        Ok(())
    }

    /// `Accessor::Read` (without the sparse part)
    fn read_accessors(&mut self) -> R<()> {
        for (i, v) in dict(self.doc, "accessors").iter().enumerate() {
            if v.get("sparse").is_some() {
                return unported("a sparse accessor");
            }
            let buffer_view = match uint_member(v, "bufferView") {
                Some(bv) => Some(index_in(self.asset.buffer_views.len(), "bufferViews", bv)?),
                None => None,
            };
            let ty = string_member(v, "type");
            if num_components(&ty).is_none() {
                return Err(format!("GLTF: Unknown accessor type \"{ty}\""));
            }
            self.asset.accessors.push(Accessor {
                id: format!("accessors[{i}]"),
                name: string_member(v, "name"),
                buffer_view,
                byte_offset: uint_member(v, "byteOffset").unwrap_or(0) as usize,
                component_type: uint_member(v, "componentType")
                    .ok_or_else(|| "GLTF: accessor without componentType".to_string())?,
                count: uint_member(v, "count")
                    .ok_or_else(|| "GLTF: accessor without count".to_string())?
                    as usize,
                ty,
            });
        }
        Ok(())
    }

    /// `Sampler::Read`
    fn read_samplers(&mut self) {
        for (i, v) in dict(self.doc, "samplers").iter().enumerate() {
            self.asset.samplers.push(Sampler {
                id: format!("samplers[{i}]"),
                name: string_member(v, "name"),
                mag_filter: uint_member(v, "magFilter"),
                min_filter: uint_member(v, "minFilter"),
                wrap_s: uint_member(v, "wrapS"),
                wrap_t: uint_member(v, "wrapT"),
            });
        }
    }

    fn missing(what: &str, i: usize) -> String {
        format!("GLTF: Object at index \"{i}\" is not found in {what}")
    }

    /// `Ref<Image> = images.Retrieve(i)`
    fn image(&mut self, o: usize) -> R<usize> {
        if let Some(r) = self.images.0.get(&o) {
            return r.ok_or_else(|| "GLTF: cyclic reference".to_string());
        }
        let v = dict(self.doc, "images")
            .get(o)
            .ok_or_else(|| Self::missing("images", o))?;
        let uri = v.get("uri").and_then(Json::as_str).map(str::to_string);
        if uri.as_deref().is_some_and(|u| u.starts_with("data:")) {
            return unported("an image with a data URI");
        }
        let buffer_view = match uint_member(v, "bufferView") {
            Some(bv) => Some(index_in(self.asset.buffer_views.len(), "bufferViews", bv)?),
            None => None,
        };
        let mime = v.get("mimeType").and_then(Json::as_str).unwrap_or("");
        if uri.is_none() && (buffer_view.is_none() || mime.is_empty()) {
            return Err(format!(
                "GLTF2: images[{o}] does not have a URI, so it must have a valid bufferView and mimetype"
            ));
        }
        self.asset.images.push(Image { uri, buffer_view });
        let r = self.asset.images.len() - 1;
        self.images.0.insert(o, Some(r));
        Ok(r)
    }

    /// `Ref<Texture> = textures.Retrieve(i)`: `Texture::Read` retrieves the sampler and then the source image.
    fn texture(&mut self, o: usize) -> R<usize> {
        if let Some(r) = self.textures.0.get(&o) {
            return r.ok_or_else(|| "GLTF: cyclic reference".to_string());
        }
        let v = dict(self.doc, "textures")
            .get(o)
            .ok_or_else(|| Self::missing("textures", o))?;
        let sampler = match uint_member(v, "sampler") {
            Some(s) => Some(index_in(self.asset.samplers.len(), "samplers", s)?),
            None => None,
        };
        let source = match uint_member(v, "source") {
            Some(s) => Some(self.image(s as usize)?),
            None => None,
        };
        self.asset.textures.push(Texture { sampler, source });
        let r = self.asset.textures.len() - 1;
        self.textures.0.insert(o, Some(r));
        Ok(r)
    }

    fn texture_info(&mut self, info: &Json) -> R<TextureInfo> {
        for (k, _) in info.as_object().unwrap_or(&[]) {
            if k != "index" && k != "texCoord" {
                return unported(&format!("texture info member \"{k}\""));
            }
        }
        let index = self.texture(
            uint_member(info, "index")
                .ok_or_else(|| "GLTF: texture info without index".to_string())?
                as usize,
        )?;
        Ok(TextureInfo {
            index,
            tex_coord: uint_member(info, "texCoord").unwrap_or(0),
        })
    }

    /// `Ref<Material> = materials.Retrieve(i)`: `Material::Read` (`pbrMetallicRoughness`, `emissiveFactor`,
    /// `doubleSided`, `alphaMode`, `alphaCutoff`)
    fn material(&mut self, o: usize) -> R<usize> {
        if let Some(r) = self.materials.0.get(&o) {
            return r.ok_or_else(|| "GLTF: cyclic reference".to_string());
        }
        let v = dict(self.doc, "materials")
            .get(o)
            .ok_or_else(|| Self::missing("materials", o))?;
        let mut m = Material {
            name: string_member(v, "name"),
            ..Material::default()
        };
        for (k, _) in v.as_object().unwrap_or(&[]) {
            match k.as_str() {
                "name"
                | "pbrMetallicRoughness"
                | "emissiveFactor"
                | "doubleSided"
                | "alphaMode"
                | "alphaCutoff" => {}
                other => return unported(&format!("material member \"{other}\"")),
            }
        }
        if let Some(f) = float_array::<3>(v, "emissiveFactor") {
            m.emissive_factor = f;
        }
        if let Some(b) = v.get("doubleSided").and_then(Json::as_bool) {
            m.double_sided = b;
        }
        if let Some(s) = v.get("alphaMode").and_then(Json::as_str) {
            m.alpha_mode = s.to_string();
        }
        if let Some(f) = v.get("alphaCutoff").and_then(Json::as_f32) {
            m.alpha_cutoff = f;
        }
        if let Some(pbr) = v.get("pbrMetallicRoughness") {
            for (k, _) in pbr.as_object().unwrap_or(&[]) {
                match k.as_str() {
                    "baseColorFactor" | "baseColorTexture" | "metallicFactor"
                    | "roughnessFactor" => {}
                    other => return unported(&format!("pbrMetallicRoughness member \"{other}\"")),
                }
            }
            if let Some(f) = float_array::<4>(pbr, "baseColorFactor") {
                m.base_color_factor = f;
            }
            if let Some(f) = pbr.get("metallicFactor").and_then(Json::as_f32) {
                m.metallic_factor = f;
            }
            if let Some(f) = pbr.get("roughnessFactor").and_then(Json::as_f32) {
                m.roughness_factor = f;
            }
            if let Some(t) = pbr.get("baseColorTexture") {
                m.base_color_texture = Some(self.texture_info(t)?);
            }
        }
        self.asset.materials.push(m);
        let r = self.asset.materials.len() - 1;
        self.materials.0.insert(o, Some(r));
        Ok(r)
    }

    /// `Ref<Mesh> = meshes.Retrieve(i)`: `Mesh::Read`, the primitives with their attribute lists (`GetAttribVector`).
    fn mesh(&mut self, o: usize) -> R<usize> {
        if let Some(r) = self.meshes.0.get(&o) {
            return r.ok_or_else(|| "GLTF: cyclic reference".to_string());
        }
        let v = dict(self.doc, "meshes")
            .get(o)
            .ok_or_else(|| Self::missing("meshes", o))?;
        let mut mesh = Mesh {
            id: format!("meshes[{o}]"),
            name: string_member(v, "name"),
            primitives: Vec::new(),
        };
        for k in ["weights", "extensions"] {
            if v.get(k).is_some() {
                return unported(&format!("mesh member \"{k}\""));
            }
        }
        for p in v.get("primitives").and_then(Json::as_array).unwrap_or(&[]) {
            for k in ["targets", "extensions"] {
                if p.get(k).is_some() {
                    return unported(&format!("primitive member \"{k}\" (morph targets)"));
                }
            }
            let mut prim = Primitive {
                mode: uint_member(p, "mode").unwrap_or(4),
                attributes: Attributes::default(),
                indices: None,
                material: None,
            };
            if let Some(attrs) = p.get("attributes").and_then(Json::as_object) {
                for (name, value) in attrs {
                    let Some(acc) = value.as_uint() else { continue };
                    let Some((list, prefix)) = prim.attributes.list_for(name) else {
                        continue;
                    };
                    let idx = if name.as_bytes().get(prefix) == Some(&b'_') {
                        name[prefix + 1..]
                            .bytes()
                            .take_while(u8::is_ascii_digit)
                            .fold(0usize, |a, b| a * 10 + usize::from(b - b'0'))
                    } else {
                        0
                    };
                    let acc = index_in(self.asset.accessors.len(), "accessors", acc)?;
                    if list.len() <= idx {
                        list.resize(idx + 1, None);
                    }
                    list[idx] = Some(acc);
                }
            }
            if let Some(i) = uint_member(p, "indices") {
                prim.indices = Some(index_in(self.asset.accessors.len(), "accessors", i)?);
            }
            if let Some(m) = uint_member(p, "material") {
                prim.material = Some(self.material(m as usize)?);
            }
            mesh.primitives.push(prim);
        }
        self.asset.meshes.push(mesh);
        let r = self.asset.meshes.len() - 1;
        self.meshes.0.insert(o, Some(r));
        Ok(r)
    }

    /// `Ref<Skin> = skins.Retrieve(i)`: `Skin::Read`, the inverse bind matrices and the joint nodes in order.
    fn skin(&mut self, o: usize) -> R<usize> {
        if let Some(r) = self.skins.0.get(&o) {
            return r.ok_or_else(|| "GLTF: cyclic reference".to_string());
        }
        let v = dict(self.doc, "skins")
            .get(o)
            .ok_or_else(|| Self::missing("skins", o))?;
        for k in ["skeleton", "extensions"] {
            if v.get(k).is_some() {
                return unported(&format!("skin member \"{k}\""));
            }
        }
        self.skins.0.insert(o, None);
        let inverse_bind_matrices = match uint_member(v, "inverseBindMatrices") {
            Some(a) => Some(index_in(self.asset.accessors.len(), "accessors", a)?),
            None => None,
        };
        let mut joints = Vec::new();
        for j in v.get("joints").and_then(Json::as_array).unwrap_or(&[]) {
            let Some(j) = j.as_uint() else { continue };
            joints.push(self.node(j as usize)?);
        }
        self.asset.skins.push(Skin {
            inverse_bind_matrices,
            joints,
        });
        let r = self.asset.skins.len() - 1;
        self.skins.0.insert(o, Some(r));
        Ok(r)
    }

    /// `Ref<Node> = nodes.Retrieve(i)`: `Node::Read`. The children are retrieved first, then the mesh, then the skin; an
    /// unnamed node is named after its id.
    fn node(&mut self, o: usize) -> R<usize> {
        if let Some(r) = self.nodes.0.get(&o) {
            return r.ok_or_else(|| "GLTF: a node is its own ancestor".to_string());
        }
        let v = dict(self.doc, "nodes")
            .get(o)
            .ok_or_else(|| Self::missing("nodes", o))?;
        for k in ["camera", "extensions", "weights"] {
            if v.get(k).is_some() {
                return unported(&format!("node member \"{k}\""));
            }
        }
        self.nodes.0.insert(o, None);
        let id = format!("nodes[{o}]");
        let mut name = string_member(v, "name");
        if name.is_empty() {
            name = id.clone();
        }
        let mut children = Vec::new();
        for c in v.get("children").and_then(Json::as_array).unwrap_or(&[]) {
            let Some(c) = c.as_uint() else { continue };
            children.push(self.node(c as usize)?);
        }
        let mut meshes = Vec::new();
        if let Some(m) = uint_member(v, "mesh") {
            meshes.push(self.mesh(m as usize)?);
        }
        let skin = match uint_member(v, "skin") {
            Some(s) => Some(self.skin(s as usize)?),
            None => None,
        };
        self.asset.nodes.push(Node {
            id,
            name,
            children,
            meshes,
            skin,
            matrix: float_array::<16>(v, "matrix"),
            translation: float_array::<3>(v, "translation"),
            rotation: float_array::<4>(v, "rotation"),
            scale: float_array::<3>(v, "scale"),
        });
        let r = self.asset.nodes.len() - 1;
        self.nodes.0.insert(o, Some(r));
        Ok(r)
    }

    /// `Animation::Read`: the samplers' accessors and the target nodes of the channels (a node outside the scene is
    /// loaded here).
    fn read_animation(&mut self, v: &Json) -> R<Animation> {
        let mut anim = Animation {
            name: string_member(v, "name"),
            samplers: Vec::new(),
            channels: Vec::new(),
        };
        for s in v.get("samplers").and_then(Json::as_array).unwrap_or(&[]) {
            let access = |this: &Self, key: &str| -> R<Option<usize>> {
                match uint_member(s, key) {
                    Some(a) => Ok(Some(index_in(this.asset.accessors.len(), "accessors", a)?)),
                    None => Ok(None),
                }
            };
            let interpolation = match s.get("interpolation").and_then(Json::as_str) {
                None | Some("LINEAR") => Interpolation::Linear,
                Some("STEP") => Interpolation::Step,
                Some("CUBICSPLINE") => Interpolation::CubicSpline,
                Some(other) => return Err(format!("GLTF: unknown interpolation \"{other}\"")),
            };
            anim.samplers.push(AnimSampler {
                input: access(self, "input")?,
                output: access(self, "output")?,
                interpolation,
            });
        }
        for c in v.get("channels").and_then(Json::as_array).unwrap_or(&[]) {
            let target = c
                .get("target")
                .ok_or_else(|| "GLTF: animation channel without target".to_string())?;
            let Some(node) = uint_member(target, "node") else {
                return unported("an animation channel target without a node");
            };
            let path = match target.get("path").and_then(Json::as_str) {
                Some("translation") => Path::Translation,
                Some("rotation") => Path::Rotation,
                Some("scale") => Path::Scale,
                Some("weights") => Path::Weights,
                _ => return Err("GLTF: unknown animation channel path".to_string()),
            };
            anim.channels.push(Channel {
                sampler: c
                    .get("sampler")
                    .and_then(Json::as_int)
                    .map_or(-1, i64::from),
                node: self.node(node as usize)?,
                path,
            });
        }
        Ok(anim)
    }
}
