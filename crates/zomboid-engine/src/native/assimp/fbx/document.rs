//! assimp `FBXDocument.cpp`, `FBXModel.cpp`, `FBXMeshGeometry.cpp`, `FBXMaterial.cpp`, `FBXDeformer.cpp`,
//! `FBXNodeAttribute.cpp`, `FBXAnimation.cpp`: the FBX DOM (objects, connections, global settings) on top of the
//! parsed element tree. Objects are built on first use like `LazyObject::Get`; an object that fails to build is
//! absent (`Get` returns null) unless it was asked for with `dieOnError`.
use super::parser::*;
use super::properties::*;
use crate::native::assimp::scene::{Mat4, Vec3};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

pub const MAX_UV: usize = 8;
pub const MAX_COLOR: usize = 8;

/// FBX::Connection
#[derive(Debug)]
pub struct Connection {
    pub insertion_order: u64,
    pub src: u64,
    pub dest: u64,
    pub prop: String,
}

/// `ReadMatrix(element)`: sixteen values, transposed.
pub fn read_matrix(el: &Element) -> R<Mat4> {
    let v = parse_float_array(el)?;
    if v.len() != 16 {
        return Err("expected 16 matrix elements".to_string());
    }
    // aiMatrix4x4 a1..d4 = values row by row, then Transpose()
    Ok(Mat4 {
        a1: v[0],
        a2: v[4],
        a3: v[8],
        a4: v[12],
        b1: v[1],
        b2: v[5],
        b3: v[9],
        b4: v[13],
        c1: v[2],
        c2: v[6],
        c3: v[10],
        c4: v[14],
        d1: v[3],
        d2: v[7],
        d3: v[11],
        d4: v[15],
    })
}

#[derive(Debug)]
pub struct Model {
    pub shading: String,
    pub culling: String,
    pub props: Rc<PropertyTable>,
    pub materials: Vec<Rc<Object>>,
    pub geometry: Vec<Rc<Object>>,
    pub attributes: Vec<Rc<Object>>,
}

pub type MatIndexArray = Vec<i32>;

/// FBX::MeshGeometry
#[derive(Debug, Default)]
pub struct MeshGeometry {
    pub vertices: Vec<Vec3>,
    pub faces: Vec<u32>,
    pub materials: MatIndexArray,
    pub normals: Vec<Vec3>,
    pub tangents: Vec<Vec3>,
    pub binormals: Vec<Vec3>,
    pub uvs: [Vec<[f32; 2]>; MAX_UV],
    pub uv_names: [String; MAX_UV],
    pub colors: [Vec<[f32; 4]>; MAX_COLOR],
    pub skin: Option<Rc<Object>>,
    pub mapping_offsets: Vec<u32>,
    pub mapping_counts: Vec<u32>,
    pub mappings: Vec<u32>,
}

#[derive(Debug)]
pub struct Material {
    pub shading: String,
    pub multilayer: bool,
    pub props: Rc<PropertyTable>,
    /// `textures`: property name -> texture, in `std::map` order.
    pub textures: BTreeMap<String, Rc<Object>>,
}

#[derive(Debug)]
pub struct Texture {
    pub ty: String,
    pub file_name: String,
    pub relative_file_name: String,
    pub alpha_source: String,
    pub uv_trans: [f32; 2],
    pub uv_scaling: [f32; 2],
    pub crop: [i32; 4],
    pub props: Rc<PropertyTable>,
    pub media: Option<Rc<Object>>,
}

#[derive(Debug)]
pub struct Video {
    pub ty: String,
    pub file_name: String,
    pub relative_file_name: String,
    pub content: Option<Vec<u8>>,
    pub props: Rc<PropertyTable>,
}

#[derive(Debug)]
pub struct Skin {
    pub clusters: Vec<Rc<Object>>,
}

#[derive(Debug)]
pub struct Cluster {
    pub indices: Vec<i32>,
    pub weights: Vec<f32>,
    pub transform: Mat4,
    pub transform_link: Mat4,
    pub node: Rc<Object>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum AttrKind {
    Camera,
    CameraSwitcher,
    Light,
    Null,
    LimbNode,
}

#[derive(Debug)]
pub struct NodeAttr {
    pub kind: AttrKind,
    pub props: Rc<PropertyTable>,
}

#[derive(Debug)]
pub struct AnimStack {
    pub props: Rc<PropertyTable>,
    pub layers: Vec<Rc<Object>>,
}

#[derive(Debug)]
pub struct AnimLayer {
    pub props: Rc<PropertyTable>,
    pub id: u64,
}

#[derive(Debug)]
pub struct AnimCurveNode {
    pub id: u64,
    pub target: Option<u64>,
    pub prop: String,
    pub props: Rc<PropertyTable>,
}

#[derive(Debug)]
pub struct AnimCurve {
    pub keys: Vec<i64>,
    pub values: Vec<f32>,
}

#[derive(Debug)]
pub enum Kind {
    Model(Model),
    Mesh(MeshGeometry),
    Material(Material),
    Texture(Texture),
    LayeredTexture,
    Video(Video),
    Skin(Skin),
    Cluster(Cluster),
    NodeAttr(NodeAttr),
    AnimStack(AnimStack),
    AnimLayer(AnimLayer),
    AnimCurveNode(AnimCurveNode),
    AnimCurve(AnimCurve),
}

#[derive(Debug)]
pub struct Object {
    pub id: u64,
    pub name: String,
    pub kind: Kind,
}

enum Cache {
    Constructing,
    Failed,
    Done(Option<Rc<Object>>),
}

struct Lazy {
    key: String,
    /// index into the `Objects` scope's elements (`None` for the dummy root, id 0)
    elem: Option<usize>,
}

pub struct Document {
    pub binary: bool,
    root: Scope,
    objects_el: usize,
    pub fbx_version: i32,
    pub creator: String,
    templates: BTreeMap<String, Rc<PropertyTable>>,
    pub globals: Rc<PropertyTable>,
    lazy: BTreeMap<u64, Lazy>,
    pub animation_stacks: Vec<u64>,
    src_conns: BTreeMap<u64, Vec<Rc<Connection>>>,
    dest_conns: BTreeMap<u64, Vec<Rc<Connection>>>,
    cache: RefCell<HashMap<u64, Cache>>,
}

/// GetPropertyTable(doc, templateName, element, sc, no_warn)
fn get_property_table(
    doc_templates: &BTreeMap<String, Rc<PropertyTable>>,
    template_name: &str,
    sc: &Scope,
) -> R<Rc<PropertyTable>> {
    let template = if template_name.is_empty() {
        None
    } else {
        doc_templates.get(template_name).cloned()
    };
    match sc.get("Properties70") {
        Some(p70) if p70.compound.is_some() => Ok(Rc::new(PropertyTable::new(p70, template)?)),
        _ => Ok(match template {
            Some(t) => t,
            None => Rc::new(PropertyTable::empty()),
        }),
    }
}

impl Document {
    /// `Document::Document(parser, settings)`: header, property templates, global settings, objects, connections.
    pub fn new(root: Scope, binary: bool) -> R<Document> {
        let mut doc = Document {
            binary,
            root,
            objects_el: 0,
            fbx_version: 0,
            creator: String::new(),
            templates: BTreeMap::new(),
            globals: Rc::new(PropertyTable::empty()),
            lazy: BTreeMap::new(),
            animation_stacks: Vec::new(),
            src_conns: BTreeMap::new(),
            dest_conns: BTreeMap::new(),
            cache: RefCell::new(HashMap::new()),
        };
        doc.read_header()?;
        doc.read_property_templates()?;
        doc.read_global_settings()?;
        doc.read_objects()?;
        doc.read_connections()?;
        Ok(doc)
    }

    fn read_header(&mut self) -> R<()> {
        let ehead = self
            .root
            .get("FBXHeaderExtension")
            .filter(|e| e.compound.is_some())
            .ok_or("no FBXHeaderExtension dictionary found")?;
        let shead = ehead.compound.as_ref().unwrap();
        self.fbx_version = parse_token_as_int(get_required_token(
            get_required_element(shead, "FBXVersion")?,
            0,
        )?)?;
        if self.fbx_version < 7100 {
            return Err("unsupported, old format version, supported are only FBX 2011, FBX 2012 and FBX 2013".to_string());
        }
        if let Some(c) = shead.get("Creator") {
            self.creator = parse_token_as_string(get_required_token(c, 0)?)?;
        }
        if let Some(ts) = shead.get("CreationTimeStamp") {
            if let Some(sc) = &ts.compound {
                for k in [
                    "Year",
                    "Month",
                    "Day",
                    "Hour",
                    "Minute",
                    "Second",
                    "Millisecond",
                ] {
                    parse_token_as_int(get_required_token(get_required_element(sc, k)?, 0)?)?;
                }
            }
        }
        Ok(())
    }

    fn read_property_templates(&mut self) -> R<()> {
        let Some(defs) = self
            .root
            .get("Definitions")
            .and_then(|e| e.compound.as_ref())
        else {
            return Ok(());
        };
        let mut templates = BTreeMap::new();
        for el in defs.collection("ObjectType") {
            let Some(sc) = &el.compound else { continue };
            if el.tokens.is_empty() {
                continue;
            }
            let oname = parse_token_as_string(&el.tokens[0])?;
            for tel in sc.collection("PropertyTemplate") {
                let Some(tsc) = &tel.compound else { continue };
                if tel.tokens.is_empty() {
                    continue;
                }
                let pname = parse_token_as_string(&tel.tokens[0])?;
                if let Some(p70) = tsc.get("Properties70") {
                    templates.insert(
                        format!("{oname}.{pname}"),
                        Rc::new(PropertyTable::new(p70, None)?),
                    );
                }
            }
        }
        self.templates = templates;
        Ok(())
    }

    fn read_global_settings(&mut self) -> R<()> {
        match self.root.get("GlobalSettings") {
            Some(e) if e.compound.is_some() => {
                self.globals =
                    get_property_table(&self.templates, "", e.compound.as_ref().unwrap())?;
            }
            _ => self.globals = Rc::new(PropertyTable::empty()),
        }
        Ok(())
    }

    fn read_objects(&mut self) -> R<()> {
        let idx = self
            .root
            .elements
            .iter()
            .position(|(k, _)| k == "Objects")
            .ok_or("no Objects dictionary found")?;
        let eobjects = &self.root.elements[idx].1;
        let sobjects = eobjects
            .compound
            .as_ref()
            .ok_or("no Objects dictionary found")?;
        self.objects_el = idx;
        let mut lazy = BTreeMap::new();
        lazy.insert(
            0u64,
            Lazy {
                key: "Objects".to_string(),
                elem: None,
            },
        );
        let mut stacks = Vec::new();
        for (i, (k, el)) in sobjects.elements.iter().enumerate() {
            if el.tokens.is_empty() {
                return Err("expected ID after object key".to_string());
            }
            let id = parse_token_as_id(&el.tokens[0])?;
            if id == 0 {
                return Err("encountered object with implicitly defined id 0".to_string());
            }
            lazy.insert(
                id,
                Lazy {
                    key: k.clone(),
                    elem: Some(i),
                },
            );
            if k == "AnimationStack" {
                stacks.push(id);
            }
        }
        self.lazy = lazy;
        self.animation_stacks = stacks;
        Ok(())
    }

    fn read_connections(&mut self) -> R<()> {
        let econns = self
            .root
            .get("Connections")
            .filter(|e| e.compound.is_some())
            .ok_or("no Connections dictionary found")?;
        let sconns = econns.compound.as_ref().unwrap();
        let mut order = 0u64;
        let mut src_conns: BTreeMap<u64, Vec<Rc<Connection>>> = BTreeMap::new();
        let mut dest_conns: BTreeMap<u64, Vec<Rc<Connection>>> = BTreeMap::new();
        for el in sconns.collection("C") {
            let ty = parse_token_as_string(get_required_token(el, 0)?)?;
            if ty == "PP" {
                continue;
            }
            let src = parse_token_as_id(get_required_token(el, 1)?)?;
            let dest = parse_token_as_id(get_required_token(el, 2)?)?;
            let prop = if ty == "OP" {
                parse_token_as_string(get_required_token(el, 3)?)?
            } else {
                String::new()
            };
            if !self.lazy.contains_key(&src) || !self.lazy.contains_key(&dest) {
                continue;
            }
            let c = Rc::new(Connection {
                insertion_order: order,
                src,
                dest,
                prop,
            });
            order += 1;
            src_conns.entry(src).or_default().push(c.clone());
            dest_conns.entry(dest).or_default().push(c);
        }
        self.src_conns = src_conns;
        self.dest_conns = dest_conns;
        Ok(())
    }

    // -------------------------------------------------------------------------------------------------------
    // connections
    // -------------------------------------------------------------------------------------------------------

    fn sequenced(&self, id: u64, is_src: bool, classnames: Option<&[&str]>) -> Vec<Rc<Connection>> {
        let map = if is_src {
            &self.src_conns
        } else {
            &self.dest_conns
        };
        let mut out: Vec<Rc<Connection>> = Vec::new();
        if let Some(v) = map.get(&id) {
            for c in v {
                match classnames {
                    None => out.push(c.clone()),
                    Some(names) => {
                        let other = if is_src { c.dest } else { c.src };
                        let key = &self.lazy[&other].key;
                        if names.iter().any(|n| n == key) {
                            out.push(c.clone());
                        }
                    }
                }
            }
        }
        out.sort_by_key(|c| c.insertion_order);
        out
    }

    /// GetConnectionsBySourceSequenced(id[, classnames])
    pub fn conns_by_source(&self, id: u64, classnames: Option<&[&str]>) -> Vec<Rc<Connection>> {
        self.sequenced(id, true, classnames)
    }

    /// GetConnectionsByDestinationSequenced(id[, classnames])
    pub fn conns_by_dest(&self, id: u64, classnames: Option<&[&str]>) -> Vec<Rc<Connection>> {
        self.sequenced(id, false, classnames)
    }

    /// Connection::SourceObject (LazyObject::Get without dieOnError)
    pub fn source_object(&self, c: &Connection) -> Option<Rc<Object>> {
        self.object(c.src)
    }

    /// Connection::DestinationObject
    pub fn dest_object(&self, c: &Connection) -> Option<Rc<Object>> {
        self.object(c.dest)
    }

    // -------------------------------------------------------------------------------------------------------
    // objects
    // -------------------------------------------------------------------------------------------------------

    pub fn has_object(&self, id: u64) -> bool {
        self.lazy.contains_key(&id)
    }

    pub fn object_ids(&self) -> Vec<u64> {
        self.lazy.keys().copied().collect()
    }

    /// LazyObject::Get(false)
    pub fn object(&self, id: u64) -> Option<Rc<Object>> {
        {
            let cache = self.cache.borrow();
            match cache.get(&id) {
                Some(Cache::Constructing) | Some(Cache::Failed) => return None,
                Some(Cache::Done(o)) => return o.clone(),
                None => {}
            }
        }
        self.cache.borrow_mut().insert(id, Cache::Constructing);
        match self.construct(id) {
            Ok(o) => {
                let o = o.map(Rc::new);
                self.cache.borrow_mut().insert(id, Cache::Done(o.clone()));
                o
            }
            Err(_) => {
                self.cache.borrow_mut().insert(id, Cache::Failed);
                None
            }
        }
    }

    fn objects_scope(&self) -> &Scope {
        self.root.elements[self.objects_el]
            .1
            .compound
            .as_ref()
            .unwrap()
    }

    fn props(&self, template: &str, sc: &Scope) -> R<Rc<PropertyTable>> {
        get_property_table(&self.templates, template, sc)
    }

    fn construct(&self, id: u64) -> R<Option<Object>> {
        let lazy = &self.lazy[&id];
        let Some(ei) = lazy.elem else {
            // the dummy root object: `new LazyObject(0, *eobjects)` has too few tokens for Get
            return Err("expected at least 3 tokens: id, name and class tag".to_string());
        };
        let element = &self.objects_scope().elements[ei].1;
        let key = lazy.key.as_str();
        if element.tokens.len() < 3 {
            return Err("expected at least 3 tokens: id, name and class tag".to_string());
        }
        let mut name = parse_token_as_string(&element.tokens[1])?;
        let classtag = parse_token_as_string(&element.tokens[2])?;
        if self.binary {
            // "Name\0\1Class" -> "Class::Name"
            let b = name.as_bytes();
            let mut i = 0;
            while i + 1 < b.len() {
                if b[i] == 0 && b[i + 1] == 1 {
                    name = format!("{}::{}", &name[i + 2..], &name[..i]);
                    break;
                }
                i += 1;
            }
        }
        // strncmp(obtype, "<Class>", length) with length = the key's length
        let m = |class: &str| class.as_bytes().starts_with(key.as_bytes());
        let kind = if m("Geometry") {
            if classtag == "Mesh" {
                Some(Kind::Mesh(self.new_mesh(id, element)?))
            } else {
                None
            }
        } else if m("NodeAttribute") {
            let kind = match classtag.as_str() {
                "Camera" => Some(AttrKind::Camera),
                "CameraSwitcher" => Some(AttrKind::CameraSwitcher),
                "Light" => Some(AttrKind::Light),
                "Null" => Some(AttrKind::Null),
                "LimbNode" => Some(AttrKind::LimbNode),
                _ => None,
            };
            match kind {
                Some(kind) => Some(Kind::NodeAttr(
                    self.new_node_attr(kind, element, &classtag)?,
                )),
                None => None,
            }
        } else if m("Deformer") {
            match classtag.as_str() {
                "Cluster" => Some(Kind::Cluster(self.new_cluster(id, element)?)),
                "Skin" => Some(Kind::Skin(self.new_skin(id, element)?)),
                _ => None,
            }
        } else if m("Model") {
            if classtag != "IKEffector" && classtag != "FKEffector" {
                Some(Kind::Model(self.new_model(id, element)?))
            } else {
                None
            }
        } else if m("Material") {
            Some(Kind::Material(self.new_material(id, element)?))
        } else if m("Texture") {
            Some(Kind::Texture(self.new_texture(id, element)?))
        } else if m("LayeredTexture") {
            return Err("UNPORTED: LayeredTexture".to_string());
        } else if m("Video") {
            Some(Kind::Video(self.new_video(element)?))
        } else if m("AnimationStack") {
            Some(Kind::AnimStack(self.new_anim_stack(id, element)?))
        } else if m("AnimationLayer") {
            Some(Kind::AnimLayer(self.new_anim_layer(id, element)?))
        } else if m("AnimationCurve") {
            Some(Kind::AnimCurve(self.new_anim_curve(element)?))
        } else if m("AnimationCurveNode") {
            Some(Kind::AnimCurveNode(self.new_anim_curve_node(id, element)?))
        } else {
            None
        };
        Ok(kind.map(|kind| Object { id, name, kind }))
    }

    fn new_model(&self, id: u64, element: &Element) -> R<Model> {
        let sc = get_required_scope(element)?;
        let mut shading = "Y".to_string();
        let mut culling = String::new();
        if let Some(s) = sc.get("Shading") {
            shading = get_required_token(s, 0)?.string();
        }
        if let Some(c) = sc.get("Culling") {
            culling = parse_token_as_string(get_required_token(c, 0)?)?;
        }
        let props = self.props("Model.FbxNode", sc)?;
        let mut m = Model {
            shading,
            culling,
            props,
            materials: Vec::new(),
            geometry: Vec::new(),
            attributes: Vec::new(),
        };
        // ResolveLinks
        for con in self.conns_by_dest(id, Some(&["Geometry", "Material", "NodeAttribute"])) {
            if !con.prop.is_empty() {
                continue;
            }
            let Some(ob) = self.source_object(&con) else {
                continue;
            };
            match &ob.kind {
                Kind::Material(_) => m.materials.push(ob),
                Kind::Mesh(_) => m.geometry.push(ob),
                Kind::NodeAttr(_) => m.attributes.push(ob),
                _ => {}
            }
        }
        Ok(m)
    }

    fn new_node_attr(&self, kind: AttrKind, element: &Element, classname: &str) -> R<NodeAttr> {
        let sc = get_required_scope(element)?;
        let props = self.props(&format!("NodeAttribute.Fbx{classname}"), sc)?;
        Ok(NodeAttr { kind, props })
    }

    fn new_skin(&self, id: u64, element: &Element) -> R<Skin> {
        let sc = get_required_scope(element)?;
        let classname = parse_token_as_string(get_required_token(element, 2)?)?;
        let _props = self.props(&format!("Deformer.Fbx{classname}"), sc)?;
        if let Some(a) = sc.get("Link_DeformAcuracy") {
            parse_token_as_float(get_required_token(a, 0)?)?;
        }
        let mut clusters = Vec::new();
        for con in self.conns_by_dest(id, Some(&["Deformer"])) {
            if !con.prop.is_empty() {
                continue;
            }
            if let Some(ob) = self.source_object(&con) {
                if matches!(ob.kind, Kind::Cluster(_)) {
                    clusters.push(ob);
                }
            }
        }
        Ok(Skin { clusters })
    }

    fn new_cluster(&self, id: u64, element: &Element) -> R<Cluster> {
        let sc = get_required_scope(element)?;
        let classname = parse_token_as_string(get_required_token(element, 2)?)?;
        let _props = self.props(&format!("Deformer.Fbx{classname}"), sc)?;
        let indexes = sc.get("Indexes");
        let weights_el = sc.get("Weights");
        let transform = get_required_element(sc, "Transform")?;
        let transform_link = get_required_element(sc, "TransformLink")?;
        let transform = read_matrix(transform)?;
        let transform_link = read_matrix(transform_link)?;
        if indexes.is_some() != weights_el.is_some() {
            return Err("either Indexes or Weights are missing from Cluster".to_string());
        }
        let (mut indices, mut weights) = (Vec::new(), Vec::new());
        if let (Some(i), Some(w)) = (indexes, weights_el) {
            indices = parse_int_array(i)?;
            weights = parse_float_array(w)?;
        }
        if indices.len() != weights.len() {
            return Err("Number of Indices does not match Number of Weights".to_string());
        }
        let mut node = None;
        for con in self.conns_by_dest(id, Some(&["Model"])) {
            if !con.prop.is_empty() {
                continue;
            }
            if let Some(ob) = self.source_object(&con) {
                if matches!(ob.kind, Kind::Model(_)) {
                    node = Some(ob);
                    break;
                }
            }
        }
        let node = node.ok_or("failed to read target Node for Cluster")?;
        Ok(Cluster {
            indices,
            weights,
            transform,
            transform_link,
            node,
        })
    }

    fn new_material(&self, id: u64, element: &Element) -> R<Material> {
        let sc = get_required_scope(element)?;
        let mut multilayer = false;
        if let Some(m) = sc.get("MultiLayer") {
            multilayer = parse_token_as_int(get_required_token(m, 0)?)? != 0;
        }
        let shading = match sc.get("ShadingModel") {
            Some(s) => parse_token_as_string(get_required_token(s, 0)?)?,
            None => "phong".to_string(),
        };
        let template = match shading.as_str() {
            "phong" => "Material.FbxSurfacePhong",
            "lambert" => "Material.FbxSurfaceLambert",
            _ => "",
        };
        let props = self.props(template, sc)?;
        let mut textures = BTreeMap::new();
        for con in self.conns_by_dest(id, None) {
            if con.prop.is_empty() {
                continue;
            }
            let Some(ob) = self.source_object(&con) else {
                continue;
            };
            match &ob.kind {
                Kind::Texture(_) => {
                    textures.insert(con.prop.clone(), ob);
                }
                Kind::LayeredTexture => {
                    return Err("UNPORTED: LayeredTexture".to_string());
                }
                _ => {}
            }
        }
        Ok(Material {
            shading,
            multilayer,
            props,
            textures,
        })
    }

    fn new_texture(&self, id: u64, element: &Element) -> R<Texture> {
        let sc = get_required_scope(element)?;
        let s = |name: &str| -> R<String> {
            match sc.get(name) {
                Some(e) => parse_token_as_string(get_required_token(e, 0)?),
                None => Ok(String::new()),
            }
        };
        let ty = s("Type")?;
        let file_name = s("FileName")?;
        let relative_file_name = s("RelativeFilename")?;
        let mut uv_trans = [0.0f32, 0.0];
        let mut uv_scaling = [1.0f32, 1.0];
        if let Some(e) = sc.get("ModelUVTranslation") {
            uv_trans = [
                parse_token_as_float(get_required_token(e, 0)?)?,
                parse_token_as_float(get_required_token(e, 1)?)?,
            ];
        }
        if let Some(e) = sc.get("ModelUVScaling") {
            uv_scaling = [
                parse_token_as_float(get_required_token(e, 0)?)?,
                parse_token_as_float(get_required_token(e, 1)?)?,
            ];
        }
        let mut crop = [0i32; 4];
        if let Some(e) = sc.get("Cropping") {
            for (i, c) in crop.iter_mut().enumerate() {
                *c = parse_token_as_int(get_required_token(e, i)?)?;
            }
        }
        let alpha_source = s("Texture_Alpha_Source")?;
        let props = self.props("Texture.FbxFileTexture", sc)?;
        if let Some(sv) = props.get("Scaling")? {
            if let Prop::Vec3(v) = sv {
                uv_scaling = [v.x, v.y];
            }
        }
        if let Some(Prop::Vec3(v)) = props.get("Translation")? {
            uv_trans = [v.x, v.y];
        }
        let mut media = None;
        for con in self.conns_by_dest(id, None) {
            if let Some(ob) = self.source_object(&con) {
                if matches!(ob.kind, Kind::Video(_)) {
                    media = Some(ob);
                }
            }
        }
        Ok(Texture {
            ty,
            file_name,
            relative_file_name,
            alpha_source,
            uv_trans,
            uv_scaling,
            crop,
            props,
            media,
        })
    }

    fn new_video(&self, element: &Element) -> R<Video> {
        let sc = get_required_scope(element)?;
        let s = |name: &str| -> R<String> {
            match sc.get(name) {
                Some(e) => parse_token_as_string(get_required_token(e, 0)?),
                None => Ok(String::new()),
            }
        };
        let ty = s("Type")?;
        let file_name = s("FileName")?;
        let relative_file_name = s("RelativeFilename")?;
        let mut content = None;
        if let Some(c) = sc.get("Content") {
            if !c.tokens.is_empty() {
                let t = &c.tokens[0];
                if !t.binary {
                    return Err("UNPORTED: text embedded video content".to_string());
                }
                if t.data.len() < 5 {
                    return Err("binary data array is too short, need five (5) bytes for type signature and element count".to_string());
                }
                if t.data[0] == b'R' {
                    let len =
                        u32::from_le_bytes([t.data[1], t.data[2], t.data[3], t.data[4]]) as usize;
                    content = Some(t.data.get(5..5 + len).unwrap_or(&[]).to_vec());
                }
            }
        }
        let props = self.props("Video.FbxVideo", sc)?;
        Ok(Video {
            ty,
            file_name,
            relative_file_name,
            content,
            props,
        })
    }

    fn new_anim_stack(&self, id: u64, element: &Element) -> R<AnimStack> {
        let sc = get_required_scope(element)?;
        let props = self.props("AnimationStack.FbxAnimStack", sc)?;
        let mut layers = Vec::new();
        for con in self.conns_by_dest(id, Some(&["AnimationLayer"])) {
            if !con.prop.is_empty() {
                continue;
            }
            if let Some(ob) = self.source_object(&con) {
                if matches!(ob.kind, Kind::AnimLayer(_)) {
                    layers.push(ob);
                }
            }
        }
        Ok(AnimStack { props, layers })
    }

    fn new_anim_layer(&self, id: u64, element: &Element) -> R<AnimLayer> {
        let sc = get_required_scope(element)?;
        let props = self.props("AnimationLayer.FbxAnimLayer", sc)?;
        Ok(AnimLayer { props, id })
    }

    fn new_anim_curve(&self, element: &Element) -> R<AnimCurve> {
        let sc = get_required_scope(element)?;
        let key_time = get_required_element(sc, "KeyTime")?;
        let key_value = get_required_element(sc, "KeyValueFloat")?;
        let keys = parse_int64_array(key_time)?;
        let values = parse_float_array(key_value)?;
        if keys.len() != values.len() {
            return Err(
                "the number of key times does not match the number of keyframe values".to_string(),
            );
        }
        if !keys.windows(2).all(|w| w[0] <= w[1]) {
            return Err("the keyframes are not in ascending order".to_string());
        }
        if let Some(a) = sc.get("KeyAttrDataFloat") {
            parse_float_array(a)?;
        }
        if let Some(a) = sc.get("KeyAttrFlags") {
            parse_int_array(a)?;
        }
        Ok(AnimCurve { keys, values })
    }

    fn new_anim_curve_node(&self, id: u64, element: &Element) -> R<AnimCurveNode> {
        let sc = get_required_scope(element)?;
        let mut target = None;
        let mut prop = String::new();
        for con in self.conns_by_source(id, Some(&["Model", "NodeAttribute", "Deformer"])) {
            if con.prop.is_empty() {
                continue;
            }
            if self.dest_object(&con).is_none() {
                continue;
            }
            target = Some(con.dest);
            prop = con.prop.clone();
            break;
        }
        let props = self.props("AnimationCurveNode.FbxAnimCurveNode", sc)?;
        Ok(AnimCurveNode {
            id,
            target,
            prop,
            props,
        })
    }

    /// AnimationCurveNode::Curves(): `std::map<property name, curve>`.
    pub fn curve_node_curves(&self, node: &AnimCurveNode) -> BTreeMap<String, Rc<Object>> {
        let mut curves = BTreeMap::new();
        for con in self.conns_by_dest(node.id, Some(&["AnimationCurve"])) {
            if con.prop.is_empty() {
                continue;
            }
            let Some(ob) = self.source_object(&con) else {
                continue;
            };
            if matches!(ob.kind, Kind::AnimCurve(_)) {
                curves.entry(con.prop.clone()).or_insert(ob);
            }
        }
        curves
    }

    /// AnimationLayer::Nodes(whitelist)
    pub fn layer_nodes(&self, layer: &AnimLayer, whitelist: Option<&[&str]>) -> Vec<Rc<Object>> {
        let mut nodes = Vec::new();
        for con in self.conns_by_dest(layer.id, Some(&["AnimationCurveNode"])) {
            if !con.prop.is_empty() {
                continue;
            }
            let Some(ob) = self.source_object(&con) else {
                continue;
            };
            let Kind::AnimCurveNode(n) = &ob.kind else {
                continue;
            };
            if let Some(w) = whitelist {
                if !w.iter().any(|s| *s == n.prop) {
                    continue;
                }
            }
            nodes.push(ob);
        }
        nodes
    }

    // -------------------------------------------------------------------------------------------------------
    // mesh geometry
    // -------------------------------------------------------------------------------------------------------

    fn new_mesh(&self, id: u64, element: &Element) -> R<MeshGeometry> {
        let mut g = MeshGeometry::default();
        for con in self.conns_by_dest(id, Some(&["Deformer"])) {
            if !con.prop.is_empty() {
                continue;
            }
            if let Some(ob) = self.source_object(&con) {
                if matches!(ob.kind, Kind::Skin(_)) {
                    g.skin = Some(ob);
                }
            }
        }
        let sc = element
            .compound
            .as_ref()
            .ok_or("failed to read Geometry object (class: Mesh), no data scope found")?;
        let vertices_el = get_required_element(sc, "Vertices")?;
        let pvi_el = get_required_element(sc, "PolygonVertexIndex")?;
        let temp_verts = parse_vec3_array(vertices_el)?;
        if temp_verts.is_empty() {
            return Ok(g);
        }
        let temp_faces = parse_int_array(pvi_el)?;
        if temp_faces.is_empty() {
            return Ok(g);
        }
        let vertex_count = temp_verts.len();
        g.mapping_offsets = vec![0; vertex_count];
        g.mapping_counts = vec![0; vertex_count];
        g.mappings = vec![0; temp_faces.len()];
        let mut count = 0u32;
        for &index in &temp_faces {
            let absi = if index < 0 { -index - 1 } else { index };
            if absi as usize >= vertex_count {
                return Err("polygon vertex index out of range".to_string());
            }
            g.vertices.push(temp_verts[absi as usize]);
            count += 1;
            g.mapping_counts[absi as usize] += 1;
            if index < 0 {
                g.faces.push(count);
                count = 0;
            }
        }
        let mut cursor = 0u32;
        for i in 0..vertex_count {
            g.mapping_offsets[i] = cursor;
            cursor += g.mapping_counts[i];
            g.mapping_counts[i] = 0;
        }
        cursor = 0;
        for &index in &temp_faces {
            let absi = (if index < 0 { -index - 1 } else { index }) as usize;
            let pos = (g.mapping_offsets[absi] + g.mapping_counts[absi]) as usize;
            g.mappings[pos] = cursor;
            g.mapping_counts[absi] += 1;
            cursor += 1;
        }
        for layer_el in sc.collection("Layer") {
            let index = parse_token_as_int(get_required_token(layer_el, 0)?)?;
            let _ = index; // readAllLayers is true: every layer is read
            let layer = get_required_scope(layer_el)?;
            for le in layer.collection("LayerElement") {
                let elayer = get_required_scope(le)?;
                self.read_layer_element(&mut g, sc, elayer)?;
            }
        }
        Ok(g)
    }

    fn read_layer_element(
        &self,
        g: &mut MeshGeometry,
        top: &Scope,
        layer_element: &Scope,
    ) -> R<()> {
        let ty = parse_token_as_string(get_required_token(
            get_required_element(layer_element, "Type")?,
            0,
        )?)?;
        let typed_index = parse_token_as_int(get_required_token(
            get_required_element(layer_element, "TypedIndex")?,
            0,
        )?)?;
        for cand in top.collection(&ty) {
            let index = parse_token_as_int(get_required_token(cand, 0)?)?;
            if index == typed_index {
                return read_vertex_data(g, &ty, typed_index, get_required_scope(cand)?);
            }
        }
        Ok(()) // "failed to resolve vertex layer element" is logged
    }
}

fn read_vertex_data(g: &mut MeshGeometry, ty: &str, index: i32, source: &Scope) -> R<()> {
    let mapping = parse_token_as_string(get_required_token(
        get_required_element(source, "MappingInformationType")?,
        0,
    )?)?;
    let reference = parse_token_as_string(get_required_token(
        get_required_element(source, "ReferenceInformationType")?,
        0,
    )?)?;
    let vc = g.vertices.len();
    match ty {
        "LayerElementUV" => {
            if index as usize >= MAX_UV || index < 0 {
                return Ok(());
            }
            let i = index as usize;
            g.uv_names[i] = String::new();
            if let Some(n) = source.get("Name") {
                g.uv_names[i] = parse_token_as_string(get_required_token(n, 0)?)?;
            }
            let (offs, counts, maps) = (&g.mapping_offsets, &g.mapping_counts, &g.mappings);
            g.uvs[i] = resolve(
                source,
                &mapping,
                &reference,
                "UV",
                "UVIndex",
                vc,
                offs,
                counts,
                maps,
                parse_vec2_array,
            )?;
        }
        "LayerElementMaterial" => {
            if !g.materials.is_empty() {
                return Ok(());
            }
            let m = read_materials(g, source, &mapping, &reference)?;
            if m.iter().all(|&n| n < 0) {
                return Ok(());
            }
            g.materials = m;
        }
        "LayerElementNormal" => {
            if !g.normals.is_empty() {
                return Ok(());
            }
            let (offs, counts, maps) = (&g.mapping_offsets, &g.mapping_counts, &g.mappings);
            let v = resolve(
                source,
                &mapping,
                &reference,
                "Normals",
                "NormalsIndex",
                vc,
                offs,
                counts,
                maps,
                parse_vec3_array,
            )?;
            g.normals = v;
        }
        "LayerElementTangent" => {
            if !g.tangents.is_empty() {
                return Ok(());
            }
            let (offs, counts, maps) = (&g.mapping_offsets, &g.mapping_counts, &g.mappings);
            let v = resolve(
                source,
                &mapping,
                &reference,
                "Tangents",
                "TangentsIndex",
                vc,
                offs,
                counts,
                maps,
                parse_vec3_array,
            )?;
            g.tangents = v;
        }
        "LayerElementBinormal" => {
            if !g.binormals.is_empty() {
                return Ok(());
            }
            let (offs, counts, maps) = (&g.mapping_offsets, &g.mapping_counts, &g.mappings);
            let v = resolve(
                source,
                &mapping,
                &reference,
                "Binormals",
                "BinormalsIndex",
                vc,
                offs,
                counts,
                maps,
                parse_vec3_array,
            )?;
            g.binormals = v;
        }
        "LayerElementColor" => {
            if index as usize >= MAX_COLOR || index < 0 {
                return Ok(());
            }
            let (offs, counts, maps) = (&g.mapping_offsets, &g.mapping_counts, &g.mappings);
            let v = resolve(
                source,
                &mapping,
                &reference,
                "Colors",
                "ColorIndex",
                vc,
                offs,
                counts,
                maps,
                parse_color_array,
            )?;
            g.colors[index as usize] = v;
        }
        _ => {}
    }
    Ok(())
}

/// ReadVertexDataMaterials
fn read_materials(g: &MeshGeometry, source: &Scope, mapping: &str, reference: &str) -> R<Vec<i32>> {
    let face_count = g.faces.len();
    if face_count == 0 {
        return Ok(Vec::new());
    }
    let mut out = parse_int_array(get_required_element(source, "Materials")?)?;
    if mapping == "AllSame" {
        if out.is_empty() {
            return Ok(Vec::new());
        }
        if out.len() > 1 {
            out.clear();
        }
        // materials_out.assign(m_vertices.size(), materials_out.at(0)); `at(0)` of the cleared vector throws
        let Some(&first) = out.first() else {
            return Err("vector::at".to_string());
        };
        return Ok(vec![first; g.vertices.len()]);
    } else if mapping == "ByPolygon" && reference == "IndexToDirect" {
        out.resize(face_count, 0);
        return Ok(out);
    }
    // "ignoring material assignments, access type not implemented" is logged
    Ok(out)
}

fn has_element(sc: &Scope, name: &str) -> bool {
    sc.has(name)
}

#[allow(clippy::too_many_arguments)]
fn resolve<T: Clone + Default>(
    source: &Scope,
    mapping: &str,
    reference: &str,
    data_name: &str,
    index_name: &str,
    vertex_count: usize,
    offsets: &[u32],
    counts: &[u32],
    mappings: &[u32],
    parse: fn(&Element) -> R<Vec<T>>,
) -> R<Vec<T>> {
    let mut is_direct = reference == "Direct";
    let mut is_index_to_direct = reference == "IndexToDirect";
    if is_index_to_direct && !has_element(source, index_name) {
        is_direct = true;
        is_index_to_direct = false;
    }
    if is_index_to_direct == is_direct {
        return Ok(Vec::new()); // "ignoring vertex data channel, access type not implemented"
    }
    let mut out: Vec<T> = Vec::new();
    if mapping == "ByVertice" && is_direct {
        if !has_element(source, data_name) {
            return Ok(out);
        }
        let temp = parse(get_required_element(source, data_name)?)?;
        out.resize(vertex_count, T::default());
        for (i, t) in temp.iter().enumerate() {
            let istart = *offsets.get(i).ok_or("index out of range")? as usize;
            let iend = istart + counts[i] as usize;
            for j in istart..iend {
                out[mappings[j] as usize] = t.clone();
            }
        }
    } else if mapping == "ByVertice" && is_index_to_direct {
        let temp = parse(get_required_element(source, data_name)?)?;
        let idx = parse_int_array(get_required_element(source, index_name)?)?;
        out.resize(vertex_count, T::default());
        for (i, &ix) in idx.iter().enumerate() {
            let istart = *offsets.get(i).ok_or("index out of range")? as usize;
            let iend = istart + counts[i] as usize;
            for j in istart..iend {
                if ix < 0 || ix as usize >= temp.len() {
                    return Err("index out of range".to_string());
                }
                out[mappings[j] as usize] = temp[ix as usize].clone();
            }
        }
    } else if mapping == "ByPolygonVertex" && is_direct {
        let temp = parse(get_required_element(source, data_name)?)?;
        if temp.len() != vertex_count {
            return Ok(Vec::new()); // "length of input data unexpected for ByPolygon mapping"
        }
        out = temp;
    } else if mapping == "ByPolygonVertex" && is_index_to_direct {
        let temp = parse(get_required_element(source, data_name)?)?;
        let idx = parse_int_array(get_required_element(source, index_name)?)?;
        if idx.len() != vertex_count {
            return Ok(Vec::new());
        }
        out.resize(vertex_count, T::default());
        for (i, &ix) in idx.iter().enumerate() {
            if ix != -1 {
                if ix < 0 || ix as usize >= temp.len() {
                    return Err("index out of range".to_string());
                }
                out[i] = temp[ix as usize].clone();
            }
        }
    }
    Ok(out)
}
