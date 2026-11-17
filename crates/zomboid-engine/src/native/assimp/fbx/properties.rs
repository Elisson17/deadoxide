//! assimp `FBXProperties.cpp`: the property tables (`Properties70`) of FBX objects, with their templates.
use super::parser::*;
use crate::native::assimp::scene::Vec3;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// FBX::Property (TypedProperty<T> for the T that `ReadTypedProperty` creates).
#[derive(Clone, Debug, PartialEq)]
pub enum Prop {
    Str(String),
    Bool(bool),
    Int(i32),
    ULong(u64),
    Int64(i64),
    Vec3(Vec3),
    /// `TypedProperty<aiColor4D>` (`ColorAndAlpha`)
    Vec4([f32; 4]),
    Float(f32),
}

/// `ReadTypedProperty(element)`: `Ok(None)` for a property of a type the library does not interpret.
fn read_typed_property(el: &Element) -> R<Option<Prop>> {
    let tok = &el.tokens;
    // fewer than two tokens: no type to look at
    if tok.len() < 2 {
        return Ok(None);
    }
    // `checkTokenCount(tokens, n)`
    let count = |n: usize| -> R<()> {
        if tok.len() < n {
            Err(format!("not enough tokens, expected at least {n}"))
        } else {
            Ok(())
        }
    };
    let s = parse_token_as_string(&tok[1])?;
    Ok(match s.as_str() {
        "KString" => {
            count(5)?;
            Some(Prop::Str(parse_token_as_string(&tok[4])?))
        }
        "bool" | "Bool" => {
            count(5)?;
            Some(Prop::Bool(parse_token_as_int(&tok[4])? != 0))
        }
        "int" | "Int" | "enum" | "Enum" | "Integer" => {
            count(5)?;
            Some(Prop::Int(parse_token_as_int(&tok[4])?))
        }
        "ULongLong" => {
            count(5)?;
            Some(Prop::ULong(parse_token_as_id(&tok[4])?))
        }
        "KTime" => {
            count(5)?;
            Some(Prop::Int64(parse_token_as_int64(&tok[4])?))
        }
        "Vector3D" | "ColorRGB" | "Vector" | "Color" | "Lcl Translation" | "Lcl Rotation"
        | "Lcl Scaling" => {
            count(7)?;
            let x = parse_token_as_float(&tok[4])?;
            let y = parse_token_as_float(&tok[5])?;
            let z = parse_token_as_float(&tok[6])?;
            Some(Prop::Vec3(Vec3::new(x, y, z)))
        }
        "double" | "Number" | "float" | "Float" | "FieldOfView" | "UnitScaleFactor" => {
            count(5)?;
            Some(Prop::Float(parse_token_as_float(&tok[4])?))
        }
        "ColorAndAlpha" => {
            count(8)?;
            Some(Prop::Vec4([
                parse_token_as_float(&tok[4])?,
                parse_token_as_float(&tok[5])?,
                parse_token_as_float(&tok[6])?,
                parse_token_as_float(&tok[7])?,
            ]))
        }
        _ => None,
    })
}

/// FBX::PropertyTable. The properties are read lazily in the library (`Get` parses and caches); the parse result is
/// kept here and `accessed` records which names `Get` has been asked for, which `GetUnparsedProperties` depends on.
#[derive(Default, Debug)]
pub struct PropertyTable {
    lazy: BTreeMap<String, R<Option<Prop>>>,
    template: Option<Rc<PropertyTable>>,
    accessed: RefCell<BTreeSet<String>>,
}

fn peek_property_name(el: &Element) -> R<String> {
    if el.tokens.len() < 4 {
        return Ok(String::new());
    }
    parse_token_as_string(&el.tokens[0])
}

impl PropertyTable {
    /// PropertyTable(element, templateProps)
    pub fn new(element: &Element, template: Option<Rc<PropertyTable>>) -> R<PropertyTable> {
        let scope = get_required_scope(element)?;
        let mut lazy = BTreeMap::new();
        for (k, v) in &scope.elements {
            if k != "P" {
                continue;
            }
            let name = peek_property_name(v)?;
            if name.is_empty() {
                continue;
            }
            if lazy.contains_key(&name) {
                continue;
            }
            lazy.insert(name, read_typed_property(v));
        }
        Ok(PropertyTable {
            lazy,
            template,
            accessed: RefCell::new(BTreeSet::new()),
        })
    }

    pub fn empty() -> PropertyTable {
        PropertyTable::default()
    }

    /// PropertyTable::Get: `Err` is an exception thrown while parsing the property.
    pub fn get(&self, name: &str) -> R<Option<Prop>> {
        match self.lazy.get(name) {
            Some(p) => {
                self.accessed.borrow_mut().insert(name.to_string());
                p.clone()
            }
            None => match &self.template {
                Some(t) => t.get(name),
                None => Ok(None),
            },
        }
    }

    /// `PropertyGet<aiVector3D>(props, name, ok, useTemplate)`
    pub fn opt_vec3(&self, name: &str, use_template: bool) -> R<Option<Vec3>> {
        Ok(match self.get_with_template(name, use_template)? {
            Some(Prop::Vec3(v)) => Some(v),
            _ => None,
        })
    }
    /// `PropertyGet<float>(props, name, ok, useTemplate)`
    pub fn opt_float(&self, name: &str, use_template: bool) -> R<Option<f32>> {
        Ok(match self.get_with_template(name, use_template)? {
            Some(Prop::Float(v)) => Some(v),
            _ => None,
        })
    }
    /// `PropertyGet<std::string>(props, name, ok, useTemplate)`
    pub fn opt_string(&self, name: &str, use_template: bool) -> R<Option<String>> {
        Ok(match self.get_with_template(name, use_template)? {
            Some(Prop::Str(v)) => Some(v),
            _ => None,
        })
    }
    /// `PropertyGet<int64_t>(props, name, ok, useTemplate)`
    pub fn opt_int64(&self, name: &str, use_template: bool) -> R<Option<i64>> {
        Ok(match self.get_with_template(name, use_template)? {
            Some(Prop::Int64(v)) => Some(v),
            _ => None,
        })
    }

    /// `PropertyGet<T>(props, name, defaultValue)` for each T.
    pub fn float(&self, name: &str, default: f32) -> R<f32> {
        Ok(match self.get(name)? {
            Some(Prop::Float(f)) => f,
            _ => default,
        })
    }
    pub fn int(&self, name: &str, default: i32) -> R<i32> {
        Ok(match self.get(name)? {
            Some(Prop::Int(f)) => f,
            _ => default,
        })
    }
    pub fn boolean(&self, name: &str, default: bool) -> R<bool> {
        Ok(match self.get(name)? {
            Some(Prop::Bool(f)) => f,
            _ => default,
        })
    }
    pub fn vec3(&self, name: &str, default: Vec3) -> R<Vec3> {
        Ok(match self.get(name)? {
            Some(Prop::Vec3(f)) => f,
            _ => default,
        })
    }
    pub fn string(&self, name: &str) -> R<Option<String>> {
        Ok(match self.get(name)? {
            Some(Prop::Str(f)) => Some(f),
            _ => None,
        })
    }
    pub fn int64(&self, name: &str, default: i64) -> R<i64> {
        Ok(match self.get(name)? {
            Some(Prop::Int64(f)) => f,
            _ => default,
        })
    }
    pub fn ulong(&self, name: &str, default: u64) -> R<u64> {
        Ok(match self.get(name)? {
            Some(Prop::ULong(f)) => f,
            _ => default,
        })
    }

    /// `PropertyGet<T>(props, name, result, useTemplate)`: the property of this table, or with `use_template` the
    /// template's when this table has none (a property of another type counts as missing).
    pub fn get_with_template(&self, name: &str, use_template: bool) -> R<Option<Prop>> {
        let mut p = self.get(name)?;
        if p.is_none() && use_template {
            if let Some(t) = &self.template {
                p = t.get(name)?;
            }
        }
        Ok(p)
    }

    /// GetUnparsedProperties: the properties `Get` has not been asked for yet, by name, that the library can interpret.
    pub fn unparsed(&self) -> R<Vec<(String, Prop)>> {
        let accessed = self.accessed.borrow();
        let mut out = Vec::new();
        for (k, p) in &self.lazy {
            if accessed.contains(k) {
                continue;
            }
            if let Some(p) = p.clone()? {
                out.push((k.clone(), p));
            }
        }
        Ok(out)
    }
}
