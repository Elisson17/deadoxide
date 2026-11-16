//! zombie.core.skinnedmodel.advancedanimation.Anim2DBlend
use super::anim_node::JaxbContext;
use crate::zombie::util::pz_xml_util::{jaxb_parse_float, xml_trim, XmlElement};

/// zombie.core.skinnedmodel.advancedanimation.Anim2DBlend: one animation of a 2D blend space and its position.
#[derive(Clone, Debug, PartialEq)]
pub struct Anim2DBlend {
    pub anim_name: String,
    pub pos_x: f32,
    pub pos_y: f32,
    pub speed_scale: f32,
    /// `@XmlAttribute(name = "referenceID") @XmlID`: the id `node1`..`node3` of the triangles refer to.
    pub reference_id: Option<String>,
}

impl Default for Anim2DBlend {
    /// zombie.core.skinnedmodel.advancedanimation.Anim2DBlend(): the field initialisers.
    fn default() -> Self {
        Anim2DBlend {
            anim_name: String::new(),
            pos_x: 0.0,
            pos_y: 0.0,
            speed_scale: 1.0,
            reference_id: None,
        }
    }
}

impl Anim2DBlend {
    /// JAXB's unmarshalling of `m_AnimName`, `m_XPos`, `m_YPos`, `m_SpeedScale` and the `referenceID` attribute. The
    /// id is trimmed (`IDTransducerImpl`) and bound in the document's id table to this blend, `index` in the node's
    /// list (`UnmarshallingContext.addToIdTable`; a later blend with the same id replaces it).
    pub fn from_xml(e: &XmlElement, index: usize, ctx: &mut JaxbContext) -> Self {
        let mut b = Anim2DBlend::default();
        if let Some((_, v)) = e.attributes.iter().find(|(n, _)| n == "referenceID") {
            let id = xml_trim(v).to_string();
            ctx.bind_id(&id, index);
            b.reference_id = Some(id);
        }
        for c in e.elements() {
            let float = |f: &mut f32| {
                if let Some(v) = jaxb_parse_float(&c.text()) {
                    *f = v;
                }
            };
            match c.name.as_str() {
                "m_AnimName" => b.anim_name = c.text(),
                "m_XPos" => float(&mut b.pos_x),
                "m_YPos" => float(&mut b.pos_y),
                "m_SpeedScale" => float(&mut b.speed_scale),
                _ => {}
            }
        }
        b
    }
}
