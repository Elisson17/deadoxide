//! zombie.characters.AttachedItems.AttachedLocations, AttachedLocationGroup and AttachedLocation: the places a
//! character carries items on its body (holsters, belt, back), declared by `NPCs/AttachedLocations.lua`.
use std::cell::RefCell;
use std::rc::Rc;

use crate::se::krka::kahlua::vm::object::{JavaClass, Throwable, Userdata, R};
use crate::zombie::lua::exposer::class_named;

fn illegal_argument(message: &str) -> Throwable {
    Throwable::Java {
        class: "java.lang.IllegalArgumentException",
        message: Some(message.into()),
    }
}

/// zombie.characters.AttachedItems.AttachedLocations: the groups in creation order.
#[derive(Default)]
pub struct AttachedLocations {
    groups: Vec<Rc<AttachedLocationGroup>>,
}

impl AttachedLocations {
    /// zombie.characters.AttachedItems.AttachedLocations.getGroup: the group with that id, made on first use.
    pub fn get_group(&mut self, id: &str) -> R<Rc<AttachedLocationGroup>> {
        if let Some(g) = self.groups.iter().find(|g| g.id == id) {
            return Ok(g.clone());
        }
        if id.is_empty() {
            return Err(illegal_argument("id is empty"));
        }
        let g = Rc::new(AttachedLocationGroup {
            id: id.to_string(),
            locations: RefCell::new(Vec::new()),
        });
        self.groups.push(g.clone());
        Ok(g)
    }
}

/// zombie.characters.AttachedItems.AttachedLocationGroup
pub struct AttachedLocationGroup {
    pub id: String,
    pub locations: RefCell<Vec<Rc<AttachedLocation>>>,
}

impl AttachedLocationGroup {
    /// zombie.characters.AttachedItems.AttachedLocationGroup.getLocation
    pub fn get_location(&self, id: &str) -> Option<Rc<AttachedLocation>> {
        self.locations.borrow().iter().find(|l| l.id == id).cloned()
    }

    /// zombie.characters.AttachedItems.AttachedLocationGroup.getOrCreateLocation
    pub fn get_or_create_location(&self, id: &str) -> R<Rc<AttachedLocation>> {
        if let Some(l) = self.get_location(id) {
            return Ok(l);
        }
        if id.is_empty() {
            return Err(illegal_argument("id is empty"));
        }
        let l = Rc::new(AttachedLocation {
            group_id: self.id.clone(),
            id: id.to_string(),
            attachment_name: RefCell::new(None),
        });
        self.locations.borrow_mut().push(l.clone());
        Ok(l)
    }

    /// zombie.characters.AttachedItems.AttachedLocationGroup.getLocationByIndex
    pub fn get_location_by_index(&self, index: i32) -> Option<Rc<AttachedLocation>> {
        let l = self.locations.borrow();
        usize::try_from(index).ok().and_then(|i| l.get(i).cloned())
    }

    /// zombie.characters.AttachedItems.AttachedLocationGroup.size
    pub fn size(&self) -> i32 {
        self.locations.borrow().len() as i32
    }

    /// zombie.characters.AttachedItems.AttachedLocationGroup.indexOf
    pub fn index_of(&self, id: &str) -> i32 {
        self.locations
            .borrow()
            .iter()
            .position(|l| l.id == id)
            .map_or(-1, |i| i as i32)
    }

    /// zombie.characters.AttachedItems.AttachedLocationGroup.checkValid
    pub fn check_valid(&self, id: &str) -> R<()> {
        if id.is_empty() {
            Err(illegal_argument("locationId is empty"))
        } else if self.index_of(id) == -1 {
            Err(Throwable::runtime(format!("no such location \"{id}\"")))
        } else {
            Ok(())
        }
    }
}

impl Userdata for AttachedLocationGroup {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.characters.AttachedItems.AttachedLocationGroup")
    }
    fn java_to_string(&self) -> String {
        format!(
            "zombie.characters.AttachedItems.AttachedLocationGroup@{:x}",
            self as *const Self as usize >> 4 & 0x7fff_ffff
        )
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// zombie.characters.AttachedItems.AttachedLocation
pub struct AttachedLocation {
    /// The owning group's id (the original keeps a reference to the group).
    pub group_id: String,
    pub id: String,
    pub attachment_name: RefCell<Option<String>>,
}

impl AttachedLocation {
    /// zombie.characters.AttachedItems.AttachedLocation.setAttachmentName (its checks test the location id, as
    /// the original's do).
    pub fn set_attachment_name(&self, name: Option<String>) -> R<()> {
        if self.id.is_empty() {
            return Err(illegal_argument("attachmentName is empty"));
        }
        *self.attachment_name.borrow_mut() = name;
        Ok(())
    }
}

impl Userdata for AttachedLocation {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.characters.AttachedItems.AttachedLocation")
    }
    fn java_to_string(&self) -> String {
        format!(
            "zombie.characters.AttachedItems.AttachedLocation@{:x}",
            self as *const Self as usize >> 4 & 0x7fff_ffff
        )
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
