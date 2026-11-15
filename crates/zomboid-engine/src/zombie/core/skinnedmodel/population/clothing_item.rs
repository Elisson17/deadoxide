//! zombie.core.skinnedmodel.population.ClothingItem: one wearable item's models, textures, masks and tint rules
//! (`media/clothing/clothingItems/<name>.xml`, found through the file guid table).
use crate::zombie::util::pz_xml_util::{jaxb_parse_boolean, jaxb_parse_int, XmlElement};
use crate::zombie::util::string_utils::is_null_or_whitespace;

/// zombie.core.skinnedmodel.population.ClothingItem.s_masksFolderDefault
pub const MASKS_FOLDER_DEFAULT: &str = "media/textures/Body/Masks";

/// zombie.core.skinnedmodel.population.ClothingItem: the fields its XML sets, plus `guid` and `mame` (the item
/// name, so spelled in the original) that `OutfitManager.getClothingItem` sets.
#[derive(Clone, Debug, PartialEq)]
pub struct ClothingItem {
    pub guid: Option<String>,
    pub male_model: Option<String>,
    pub female_model: Option<String>,
    pub alt_male_model: Option<String>,
    pub alt_female_model: Option<String>,
    pub is_static: bool,
    pub base_textures: Vec<String>,
    pub attach_bone: Option<String>,
    pub masks: Vec<i32>,
    pub masks_folder: Option<String>,
    pub underlay_masks_folder: Option<String>,
    pub texture_choices: Vec<String>,
    pub allow_random_hue: bool,
    pub allow_random_tint: bool,
    pub decal_group: Option<String>,
    pub shader: Option<String>,
    pub hat_category: Option<String>,
    pub spawn_with: Vec<String>,
    pub mame: Option<String>,
    /// The asset's state is FAILED (its file did not parse as a clothing item): the fields keep their defaults.
    pub failed: bool,
}

impl Default for ClothingItem {
    /// zombie.core.skinnedmodel.population.ClothingItem(AssetPath, AssetManager): the field initialisers.
    fn default() -> Self {
        ClothingItem {
            guid: None,
            male_model: None,
            female_model: None,
            alt_male_model: None,
            alt_female_model: None,
            is_static: false,
            base_textures: Vec::new(),
            attach_bone: None,
            masks: Vec::new(),
            masks_folder: Some(MASKS_FOLDER_DEFAULT.to_string()),
            underlay_masks_folder: Some(MASKS_FOLDER_DEFAULT.to_string()),
            texture_choices: Vec::new(),
            allow_random_hue: false,
            allow_random_tint: false,
            decal_group: None,
            shader: None,
            hat_category: None,
            spawn_with: Vec::new(),
            mame: None,
            failed: false,
        }
    }
}

/// ClothingItemAssetManager.fixPath: backslashes become slashes.
fn fix_path(path: Option<String>) -> Option<String> {
    path.map(|p| p.replace('\\', "/"))
}

fn fix_paths(paths: Vec<String>) -> Vec<String> {
    paths.into_iter().map(|p| p.replace('\\', "/")).collect()
}

impl ClothingItem {
    /// JAXB's unmarshalling of `ClothingItemXML` (root `clothingItem`; other elements ignored; the `m_GUID` it
    /// reads is not copied), then ClothingItemAssetManager.onFileTaskFinished: model and texture paths get forward
    /// slashes. Another root element is the original's `UnmarshalException` (the asset fails to load).
    pub fn from_xml(root: &XmlElement) -> Result<Self, String> {
        if root.name != "clothingItem" {
            return Err(format!(
                "javax.xml.bind.UnmarshalException: unexpected element {}",
                root.name
            ));
        }
        let mut male_model = None;
        let mut female_model = None;
        let mut alt_male_model = None;
        let mut alt_female_model = None;
        let mut is_static = false;
        let mut base_textures = Vec::new();
        let mut attach_bone = None;
        let mut masks = Vec::new();
        let mut masks_folder = Some(MASKS_FOLDER_DEFAULT.to_string());
        let mut underlay_masks_folder = Some(MASKS_FOLDER_DEFAULT.to_string());
        let mut texture_choices = Vec::new();
        let mut allow_random_hue = false;
        let mut allow_random_tint = false;
        let mut decal_group = None;
        let mut shader = None;
        let mut hat_category = None;
        let mut spawn_with = Vec::new();
        for c in root.elements() {
            let flag = |v: &mut bool| {
                if let Some(b) = jaxb_parse_boolean(&c.text()) {
                    *v = b;
                }
            };
            match c.name.as_str() {
                "m_MaleModel" => male_model = Some(c.text()),
                "m_FemaleModel" => female_model = Some(c.text()),
                "m_AltMaleModel" => alt_male_model = Some(c.text()),
                "m_AltFemaleModel" => alt_female_model = Some(c.text()),
                "m_Static" => flag(&mut is_static),
                "m_BaseTextures" => base_textures.push(c.text()),
                "m_AttachBone" => attach_bone = Some(c.text()),
                "m_Masks" => {
                    if let Some(v) = jaxb_parse_int(&c.text()) {
                        masks.push(v);
                    }
                }
                "m_MasksFolder" => masks_folder = Some(c.text()),
                "m_UnderlayMasksFolder" => underlay_masks_folder = Some(c.text()),
                "textureChoices" => texture_choices.push(c.text()),
                "m_AllowRandomHue" => flag(&mut allow_random_hue),
                "m_AllowRandomTint" => flag(&mut allow_random_tint),
                "m_DecalGroup" => decal_group = Some(c.text()),
                "m_Shader" => shader = Some(c.text()),
                "m_HatCategory" => hat_category = Some(c.text()),
                "m_SpawnWith" => spawn_with.push(c.text()),
                _ => {}
            }
        }
        Ok(ClothingItem {
            guid: None,
            male_model: fix_path(male_model),
            female_model: fix_path(female_model),
            alt_male_model: fix_path(alt_male_model),
            alt_female_model: fix_path(alt_female_model),
            is_static,
            base_textures: fix_paths(base_textures),
            attach_bone,
            masks,
            masks_folder: fix_path(masks_folder),
            underlay_masks_folder: fix_path(underlay_masks_folder),
            texture_choices: fix_paths(texture_choices),
            allow_random_hue,
            allow_random_tint,
            decal_group,
            shader,
            hat_category,
            spawn_with: fix_paths(spawn_with),
            mame: None,
            failed: false,
        })
    }

    /// zombie.core.skinnedmodel.population.ClothingItem.isHat
    pub fn is_hat(&self) -> bool {
        !is_null_or_whitespace(self.hat_category.as_deref())
            && self.hat_category.as_deref() != Some("nobeard")
    }

    /// zombie.core.skinnedmodel.population.ClothingItem.isMask
    pub fn is_mask(&self) -> bool {
        let Some(c) = self.hat_category.as_deref() else {
            return false;
        };
        !is_null_or_whitespace(Some(c)) && (c == "nohairnobeard" || !c.contains("hair"))
    }

    /// zombie.core.skinnedmodel.population.ClothingItem.hasModel
    pub fn has_model(&self) -> bool {
        !is_null_or_whitespace(self.male_model.as_deref())
            && !is_null_or_whitespace(self.female_model.as_deref())
    }
}
