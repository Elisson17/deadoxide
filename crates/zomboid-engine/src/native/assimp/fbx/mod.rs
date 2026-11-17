//! assimp's FBX importer (`code/AssetLib/FBX`): `FBXTokenizer`, `FBXParser`, `FBXDocument` and the DOM classes,
//! `FBXConverter` and `FBXImporter`.
pub mod converter;
pub mod document;
pub mod inflate;
pub mod parser;
pub mod properties;

/// `FBXImporter::InternReadFile`: the scene of an FBX file (before the post-processing steps).
pub fn import(bytes: &[u8]) -> Result<crate::native::assimp::scene::Scene, String> {
    let doc = read_document(bytes)?;
    converter::convert(&doc)
}

/// The first half of `FBXImporter::InternReadFile`: tokenize (text or binary, by the header), parse and build the
/// document of the file.
pub fn read_document(bytes: &[u8]) -> Result<document::Document, String> {
    let mut contents = bytes.to_vec();
    contents.push(0);
    let binary = contents.starts_with(b"Kaydara FBX Binary");
    let tokens = if binary {
        parser::tokenize_binary(&contents)?
    } else {
        parser::tokenize(&contents)?
    };
    let root = parser::parse(&tokens)?;
    document::Document::new(root, binary)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASCII: &str = "; FBX 7.4.0 project file\n\
FBXHeaderExtension:  {\n\tFBXHeaderVersion: 1003\n\tFBXVersion: 7400\n\tCreator: \"test\"\n}\n\
Definitions:  {\n\tVersion: 100\n}\n\
Objects:  {\n\tModel: 100, \"Model::Cube\", \"Mesh\" {\n\t\tVersion: 232\n\t\tProperties70:  {\n\
\t\t\tP: \"Lcl Translation\", \"Lcl Translation\", \"\", \"A\",1.5,2,-3\n\t\t}\n\t}\n}\n\
Connections:  {\n\tC: \"OO\",100,0\n}\n";

    #[test]
    fn ascii_document() {
        let doc = read_document(ASCII.as_bytes()).unwrap();
        assert!(!doc.binary);
        assert_eq!(doc.fbx_version, 7400);
        assert_eq!(doc.object_ids(), vec![0, 100]);
        let ob = doc.object(100).unwrap();
        assert_eq!(ob.name, "Model::Cube");
        let document::Kind::Model(m) = &ob.kind else {
            panic!("not a model")
        };
        let t = m
            .props
            .vec3(
                "Lcl Translation",
                crate::native::assimp::scene::Vec3::new(0.0, 0.0, 0.0),
            )
            .unwrap();
        assert_eq!((t.x, t.y, t.z), (1.5, 2.0, -3.0));
        assert_eq!(doc.conns_by_source(100, None).len(), 1);
    }

    #[test]
    fn rejects_old_versions() {
        let text = ASCII.replace("7400", "6100");
        assert!(read_document(text.as_bytes()).is_err());
    }
}
