//! zombie.ui.UIManager: the static lists of top-level UI elements the Lua UI adds and removes. `update` (which moves
//! `toAdd`/`toRemove` into `UI`) and rendering run once per frame; no frame runs headless.
use std::rc::Rc;

use super::ui_element::list_remove;
use crate::se::krka::kahlua::vm::object::Userdata;

/// UIManager's element lists (UIElementInterface objects, by identity).
#[derive(Default)]
pub struct UIManager {
    /// `UI`
    pub ui: Vec<Rc<dyn Userdata>>,
    /// `toAdd`
    pub to_add: Vec<Rc<dyn Userdata>>,
    /// `toRemove`
    pub to_remove: Vec<Rc<dyn Userdata>>,
    /// `toTop`
    pub to_top: Vec<Rc<dyn Userdata>>,
}

impl UIManager {
    /// zombie.ui.UIManager.AddUI: moved to the end of `toRemove` and of `toAdd` (update removes it from UI, then adds
    /// it last).
    pub fn add_ui(&mut self, el: &Rc<dyn Userdata>) {
        list_remove(&mut self.to_remove, el);
        self.to_remove.push(el.clone());
        list_remove(&mut self.to_add, el);
        self.to_add.push(el.clone());
    }

    /// zombie.ui.UIManager.RemoveElement
    pub fn remove_element(&mut self, el: &Rc<dyn Userdata>) {
        list_remove(&mut self.to_add, el);
        list_remove(&mut self.to_remove, el);
        self.to_remove.push(el.clone());
    }

    /// zombie.ui.UIManager.pushToTop
    pub fn push_to_top(&mut self, el: &Rc<dyn Userdata>) {
        self.to_top.push(el.clone());
    }

    /// zombie.ui.UIManager.clearArrays
    pub fn clear_arrays(&mut self) {
        self.to_add.clear();
        self.to_remove.clear();
        self.ui.clear();
    }
}
