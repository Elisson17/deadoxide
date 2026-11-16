//! zombie.core.skinnedmodel.advancedanimation.AnimationVariableReference
use crate::zombie::util::string_utils;

/// zombie.core.skinnedmodel.advancedanimation.AnimationVariableReference: a variable name, optionally of a sub
/// variable source (`$<source>.<name>`). The handle is allocated lazily by the original and is not kept here.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationVariableReference {
    pub sub_variable_source_name: Option<String>,
    pub name: String,
}

impl AnimationVariableReference {
    /// zombie.core.skinnedmodel.advancedanimation.AnimationVariableReference.fromRawVariableName (with `parse`): a name
    /// starting with '$' whose first '.' is past index 1 splits into source and name, unless the name part is blank
    /// or the source has characters other than letters, digits and '_' (the original warns and keeps the raw name).
    pub fn from_raw_variable_name(raw_variable_name: &str) -> Self {
        let mut r = AnimationVariableReference {
            sub_variable_source_name: None,
            name: raw_variable_name.to_string(),
        };
        let u: Vec<u16> = raw_variable_name.encode_utf16().collect();
        if u.first() == Some(&u16::from(b'$')) {
            if let Some(index_of_dot) = u.iter().position(|&c| c == u16::from(b'.')) {
                if index_of_dot > 1 {
                    let sub_source = String::from_utf16_lossy(&u[1..index_of_dot]);
                    let sub_name = String::from_utf16_lossy(&u[index_of_dot + 1..]);
                    if !string_utils::is_null_or_whitespace(Some(&sub_name))
                        && string_utils::is_valid_variable_name(&sub_source)
                    {
                        r.sub_variable_source_name = Some(sub_source);
                        r.name = sub_name;
                    }
                }
            }
        }
        r
    }
}
