//! zombie.entity.components.crafting.recipe.OverlayMapper: the overlay style a recipe's result shows, by default or by
//! the item one input line used. Items are held by their full names, the input script by its `ioLines` index.

/// zombie.entity.components.crafting.recipe.OverlayMapper
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OverlayMapper {
    pub default_styles: Vec<String>,
    /// `styleMap` (HashMap<Item, String>): item -> style, in insertion order.
    pub style_map: Vec<(String, String)>,
    pub input_script: Option<usize>,
}

impl OverlayMapper {
    /// zombie.entity.components.crafting.recipe.OverlayMapper.setDefaultOverlayStyle: every non-empty piece of
    /// `style.split(";")` is appended.
    pub fn set_default_overlay_style(&mut self, style: &str) {
        for val in crate::java::split_char(style, ';') {
            if !val.is_empty() {
                self.default_styles.push(val.to_string());
            }
        }
    }

    /// zombie.entity.components.crafting.recipe.OverlayMapper.addOverlayStyleEntry: each item through
    /// `ScriptManager.getItem` (`get_item`, the item's full name); an unknown item or one already mapped throws.
    pub fn add_overlay_style_entry(
        &mut self,
        style: &str,
        items: &[&str],
        get_item: &dyn Fn(&str) -> Option<String>,
    ) -> Result<(), String> {
        for item_name in items {
            let Some(item) = get_item(item_name) else {
                return Err(format!(
                    "java.lang.Exception: OverlayMapper item not found: {item_name}"
                ));
            };
            if let Some((_, old)) = self.style_map.iter().find(|(k, _)| *k == item) {
                return Err(format!("java.lang.Exception: OverlayMapper item {item_name} already has style mapping: {old}. Cannot assign style: {style}"));
            }
            self.style_map.push((item, style.to_string()));
        }
        Ok(())
    }

    /// zombie.entity.components.crafting.recipe.OverlayMapper.registerInputScript
    pub fn register_input_script(&mut self, input_script: usize) {
        self.input_script = Some(input_script);
    }

    /// zombie.entity.components.crafting.recipe.OverlayMapper.isEmpty
    pub fn is_empty(&self) -> bool {
        self.default_styles.is_empty() && self.style_map.is_empty()
    }
}
