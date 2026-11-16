//! zombie.core.skinnedmodel.advancedanimation.Anim2DBlendTriangle
use super::anim_node::JaxbContext;
use crate::zombie::util::pz_xml_util::XmlElement;

/// zombie.core.skinnedmodel.advancedanimation.Anim2DBlendTriangle: three blends of the node (`@XmlIDREF`), here as
/// indices into the node's `blends2d`; `None` is null (an id no blend has).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Anim2DBlendTriangle {
    pub node1: Option<usize>,
    pub node2: Option<usize>,
    pub node3: Option<usize>,
}

impl Anim2DBlendTriangle {
    /// JAXB's unmarshalling of `node1`, `node2`, `node3`: each an IDREF (`TransducedAccessor.IDREFTransducedAccessorImpl`)
    /// resolved now when the id is bound, else after the whole document (the triangle is `triangle` in the node's list).
    pub fn from_xml(e: &XmlElement, triangle: usize, ctx: &mut JaxbContext) -> Self {
        let mut t = Anim2DBlendTriangle::default();
        for c in e.elements() {
            let slot = match c.name.as_str() {
                "node1" => 0,
                "node2" => 1,
                "node3" => 2,
                _ => continue,
            };
            let value = ctx.idref(&c.text(), triangle, slot);
            if value.is_some() {
                t.set(slot, value);
            }
        }
        t
    }

    /// Field `slot` (0 = node1, 1 = node2, 2 = node3).
    pub fn set(&mut self, slot: usize, value: Option<usize>) {
        match slot {
            0 => self.node1 = value,
            1 => self.node2 = value,
            _ => self.node3 = value,
        }
    }
}
