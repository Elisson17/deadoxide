//! zombie.characters.WornItems.BodyLocations, BodyLocationGroup and BodyLocation: the clothing slots the game's
//! `NPCs/BodyLocations.lua` declares and their exclusion/hide/alt-model rules. Location ids are ItemBodyLocation
//! registry objects, compared by identity as the original's (no `equals` override).
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::se::krka::kahlua::vm::object::{JavaClass, Object, Throwable, Userdata, R};
use crate::zombie::lua::exposer::class_named;

/// zombie.characters.WornItems.BodyLocations: the groups, in creation order (a Java static here owned by the Lua
/// state's JavaStatics).
#[derive(Default)]
pub struct BodyLocations {
    groups: Vec<Rc<BodyLocationGroup>>,
}

impl BodyLocations {
    /// zombie.characters.WornItems.BodyLocations.getGroup: the group with that id, made on first use.
    pub fn get_group(&mut self, id: &str) -> R<Rc<BodyLocationGroup>> {
        if let Some(g) = self.groups.iter().find(|g| g.id == id) {
            return Ok(g.clone());
        }
        let g = Rc::new(BodyLocationGroup::new(id)?);
        self.groups.push(g.clone());
        Ok(g)
    }

    /// zombie.characters.WornItems.BodyLocations.getAllGroups
    pub fn get_all_groups(&self) -> &[Rc<BodyLocationGroup>] {
        &self.groups
    }
}

/// zombie.characters.WornItems.BodyLocationGroup
pub struct BodyLocationGroup {
    pub id: String,
    pub locations: RefCell<Vec<Rc<BodyLocation>>>,
}

impl BodyLocationGroup {
    /// zombie.characters.WornItems.BodyLocationGroup.<init>
    pub fn new(id: &str) -> R<BodyLocationGroup> {
        if id.is_empty() {
            return Err(Throwable::Java {
                class: "java.lang.IllegalArgumentException",
                message: Some("id is empty".into()),
            });
        }
        Ok(BodyLocationGroup {
            id: id.to_string(),
            locations: RefCell::new(Vec::new()),
        })
    }

    /// zombie.characters.WornItems.BodyLocationGroup.getLocation
    pub fn get_location(&self, id: &Object) -> Option<Rc<BodyLocation>> {
        self.locations
            .borrow()
            .iter()
            .find(|l| l.is_id(id))
            .cloned()
    }

    /// zombie.characters.WornItems.BodyLocationGroup.getOrCreateLocation
    pub fn get_or_create_location(&self, id: &Object) -> Rc<BodyLocation> {
        if let Some(l) = self.get_location(id) {
            return l;
        }
        let l = Rc::new(BodyLocation {
            group_id: self.id.clone(),
            id: id.clone(),
            exclusive: RefCell::new(Vec::new()),
            hide_model: RefCell::new(Vec::new()),
            alt_model: RefCell::new(Vec::new()),
            multi_item: Cell::new(false),
        });
        self.locations.borrow_mut().push(l.clone());
        l
    }

    /// zombie.characters.WornItems.BodyLocationGroup.getLocationByIndex
    pub fn get_location_by_index(&self, index: i32) -> Option<Rc<BodyLocation>> {
        let l = self.locations.borrow();
        usize::try_from(index).ok().and_then(|i| l.get(i).cloned())
    }

    /// zombie.characters.WornItems.BodyLocationGroup.moveLocationToIndex
    pub fn move_location_to_index(&self, id: &Object, index: i32) {
        let mut l = self.locations.borrow_mut();
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        if index >= l.len() {
            return;
        }
        let mut i = 0;
        while i < l.len() {
            if l[i].is_id(id) {
                let x = l.remove(i);
                l.insert(index, x);
            }
            i += 1;
        }
    }

    /// zombie.characters.WornItems.BodyLocationGroup.size
    pub fn size(&self) -> i32 {
        self.locations.borrow().len() as i32
    }

    fn location_or_npe(&self, id: &Object) -> R<Rc<BodyLocation>> {
        self.get_location(id).ok_or_else(|| Throwable::Java {
            class: "java.lang.NullPointerException",
            message: None,
        })
    }

    /// zombie.characters.WornItems.BodyLocationGroup.setExclusive: both ways; an unknown location is a
    /// NullPointerException, as in the original.
    pub fn set_exclusive(&self, first: &Object, second: &Object) -> R<()> {
        let a = self.location_or_npe(first)?;
        let b = self.location_or_npe(second)?;
        a.set_exclusive(second);
        b.set_exclusive(first);
        Ok(())
    }

    /// zombie.characters.WornItems.BodyLocationGroup.isExclusive
    pub fn is_exclusive(&self, first: &Object, second: &Object) -> R<bool> {
        Ok(self.location_or_npe(first)?.is_exclusive(second))
    }

    /// zombie.characters.WornItems.BodyLocationGroup.setHideModel
    pub fn set_hide_model(&self, first: &Object, second: &Object) -> R<()> {
        self.location_or_npe(first)?.set_hide_model(second);
        Ok(())
    }

    /// zombie.characters.WornItems.BodyLocationGroup.isHideModel
    pub fn is_hide_model(&self, first: &Object, second: &Object) -> R<bool> {
        Ok(self.location_or_npe(first)?.is_hide_model(second))
    }

    /// zombie.characters.WornItems.BodyLocationGroup.setAltModel
    pub fn set_alt_model(&self, first: &Object, second: &Object) -> R<()> {
        self.location_or_npe(first)?.set_alt_model(second);
        Ok(())
    }

    /// zombie.characters.WornItems.BodyLocationGroup.isAltModel
    pub fn is_alt_model(&self, first: &Object, second: &Object) -> R<bool> {
        Ok(self.location_or_npe(first)?.is_alt_model(second))
    }

    /// zombie.characters.WornItems.BodyLocationGroup.indexOf
    pub fn index_of(&self, id: &Object) -> i32 {
        self.locations
            .borrow()
            .iter()
            .position(|l| l.is_id(id))
            .map_or(-1, |i| i as i32)
    }

    /// zombie.characters.WornItems.BodyLocationGroup.setMultiItem
    pub fn set_multi_item(&self, id: &Object, multi_item: bool) -> R<()> {
        self.location_or_npe(id)?.multi_item.set(multi_item);
        Ok(())
    }

    /// zombie.characters.WornItems.BodyLocationGroup.isMultiItem
    pub fn is_multi_item(&self, id: &Object) -> R<bool> {
        Ok(self.location_or_npe(id)?.multi_item.get())
    }
}

impl Userdata for BodyLocationGroup {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.characters.WornItems.BodyLocationGroup")
    }
    fn java_to_string(&self) -> String {
        format!(
            "zombie.characters.WornItems.BodyLocationGroup@{:x}",
            self as *const Self as usize >> 4 & 0x7fff_ffff
        )
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// zombie.characters.WornItems.BodyLocation
pub struct BodyLocation {
    /// The owning group's id (the original keeps a reference to the group).
    pub group_id: String,
    pub id: Object,
    pub exclusive: RefCell<Vec<Object>>,
    pub hide_model: RefCell<Vec<Object>>,
    pub alt_model: RefCell<Vec<Object>>,
    pub multi_item: Cell<bool>,
}

fn add_once(list: &RefCell<Vec<Object>>, id: &Object) {
    if !list.borrow().iter().any(|x| x.same(id)) {
        list.borrow_mut().push(id.clone());
    }
}

impl BodyLocation {
    /// zombie.characters.WornItems.BodyLocation.setExclusive
    pub fn set_exclusive(&self, id: &Object) {
        add_once(&self.exclusive, id);
    }
    /// zombie.characters.WornItems.BodyLocation.setHideModel
    pub fn set_hide_model(&self, id: &Object) {
        add_once(&self.hide_model, id);
    }
    /// zombie.characters.WornItems.BodyLocation.setAltModel
    pub fn set_alt_model(&self, id: &Object) {
        add_once(&self.alt_model, id);
    }
    /// zombie.characters.WornItems.BodyLocation.isExclusive
    pub fn is_exclusive(&self, id: &Object) -> bool {
        self.exclusive.borrow().iter().any(|x| x.same(id))
    }
    /// zombie.characters.WornItems.BodyLocation.isHideModel
    pub fn is_hide_model(&self, id: &Object) -> bool {
        self.hide_model.borrow().iter().any(|x| x.same(id))
    }
    /// zombie.characters.WornItems.BodyLocation.isAltModel
    pub fn is_alt_model(&self, id: &Object) -> bool {
        self.alt_model.borrow().iter().any(|x| x.same(id))
    }
    /// zombie.characters.WornItems.BodyLocation.isId (`id.equals(other)`: identity)
    pub fn is_id(&self, id: &Object) -> bool {
        self.id.same(id)
    }
}

impl Userdata for BodyLocation {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.characters.WornItems.BodyLocation")
    }
    fn java_to_string(&self) -> String {
        format!(
            "zombie.characters.WornItems.BodyLocation@{:x}",
            self as *const Self as usize >> 4 & 0x7fff_ffff
        )
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
