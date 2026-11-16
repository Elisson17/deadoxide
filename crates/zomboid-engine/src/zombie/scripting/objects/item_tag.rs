//! zombie.scripting.objects.ItemTag: the registry of item tags (`Registries.ITEM_TAG`), vanilla only.
use super::item_tag_table::BASE_IDS;
use super::resource_location::ResourceLocation;

/// zombie.scripting.objects.ItemTag.get(ResourceLocation): the registered tag, identified by its location (its
/// `toString()` is `namespace:path`), or `None` where the registry has none (Java null). The registry holds the ids
/// of `registerBase` (`RegistryReset.createLocation`: namespace "base", lower-cased path).
pub fn get(id: &ResourceLocation) -> Option<ResourceLocation> {
    (id.namespace == "base" && BASE_IDS.iter().any(|b| b.to_lowercase() == id.path))
        .then(|| id.clone())
}
