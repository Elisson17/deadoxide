//! zombie.scripting.objects.ResourceLocation: a `namespace:path` id, lower-cased, "base" by default.
use std::fmt;

/// zombie.scripting.objects.ResourceLocation
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResourceLocation {
    pub namespace: String,
    pub path: String,
}

impl ResourceLocation {
    /// zombie.scripting.objects.ResourceLocation(String, String): both parts lower-cased (`Locale.ENGLISH`).
    pub fn new(namespace: &str, path: &str) -> Result<Self, String> {
        if namespace.is_empty() {
            return Err(
                "java.lang.IllegalArgumentException: Namespace cannot be null or empty".into(),
            );
        }
        if path.is_empty() {
            return Err("java.lang.IllegalArgumentException: Path cannot be null or empty".into());
        }
        Ok(ResourceLocation {
            namespace: namespace.to_lowercase(),
            path: path.to_lowercase(),
        })
    }

    /// zombie.scripting.objects.ResourceLocation.of: `id.split(":", 2)`; without a colon the namespace is "base".
    pub fn of(id: &str) -> Result<Self, String> {
        if id.is_empty() {
            return Err(
                "java.lang.IllegalArgumentException: Identifier cannot be null or empty".into(),
            );
        }
        match id.split_once(':') {
            None => ResourceLocation::new("base", id),
            Some((ns, path)) => ResourceLocation::new(ns, path),
        }
    }
}

impl fmt::Display for ResourceLocation {
    /// `toString()`: the id `namespace:path`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.path)
    }
}
