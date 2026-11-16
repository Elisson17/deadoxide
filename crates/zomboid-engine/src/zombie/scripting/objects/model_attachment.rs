//! zombie.scripting.objects.ModelAttachment: a named attachment point of a model or vehicle script.
use crate::java;
use crate::org::joml::vector3f::Vector3f;
use crate::zombie::util::string_utils;

/// zombie.scripting.objects.ModelAttachment. The `owner` reference is kept as "has an owner": the only owner set
/// while scripts load is the vehicle script that holds the attachment.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelAttachment {
    pub owned: bool,
    pub id: String,
    pub offset: Vector3f,
    pub rotate: Vector3f,
    pub scale: f32,
    pub bone: Option<String>,
    pub can_attach: Option<Vec<String>>,
    pub zoffset: f32,
    pub update_constraint: bool,
}

impl ModelAttachment {
    /// zombie.scripting.objects.ModelAttachment.<init>(String) through `setId`: a null or whitespace id is the
    /// original's IllegalArgumentException.
    pub fn new(id: Option<&str>) -> Result<Self, String> {
        if string_utils::is_null_or_whitespace(id) {
            return Err(
                "java.lang.IllegalArgumentException: ModelAttachment id is null or empty".into(),
            );
        }
        Ok(ModelAttachment {
            owned: false,
            id: id.unwrap_or_default().to_string(),
            offset: Vector3f::default(),
            rotate: Vector3f::default(),
            scale: 1.0,
            bone: None,
            can_attach: None,
            zoffset: 0.0,
            update_constraint: true,
        })
    }

    /// zombie.scripting.objects.ModelAttachment.setBone: trimmed, empty is null.
    pub fn set_bone(&mut self, bone: &str) {
        let bone = java::trim(bone);
        self.bone = if bone.is_empty() {
            None
        } else {
            Some(bone.to_string())
        };
    }
}
