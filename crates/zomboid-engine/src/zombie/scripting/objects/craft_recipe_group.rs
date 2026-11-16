//! zombie.scripting.objects.CraftRecipeGroup

/// zombie.scripting.objects.CraftRecipeGroup: the ids of its constants (OPEN_BOX), `toString()`.
pub const IDS: [&str; 1] = ["OpenBox"];

/// zombie.scripting.objects.CraftRecipeGroup.fromString: the constant whose id equals `id`, else null.
pub fn from_string(id: &str) -> Option<&'static str> {
    IDS.iter().copied().find(|e| *e == id)
}
