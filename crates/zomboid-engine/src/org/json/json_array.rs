//! org.json.JSONArray (org.json 20250517): the tokener constructor.
use super::json_object::JsonValue;
use super::json_tokener::JsonTokener;
use super::JsonException;

/// org.json.JSONArray(JSONTokener, JSONParserConfiguration), not strict: `[`, values separated by `,` (an empty
/// slot is `JSONObject.NULL`); kept literal: a second `,` right after a `,` ends the array, as does `]` after `,`.
pub fn parse(x: &mut JsonTokener) -> Result<Vec<JsonValue>, JsonException> {
    let mut list = Vec::new();
    if x.next_clean() != u16::from(b'[') {
        return Err(x.syntax_error("A JSONArray text must start with '['"));
    }
    let next_char = x.next_clean();
    if next_char == 0 {
        return Err(x.syntax_error("Expected a ',' or ']'"));
    }
    if next_char == u16::from(b']') {
        return Ok(list);
    }
    x.back()?;
    loop {
        if x.next_clean() == u16::from(b',') {
            x.back()?;
            list.push(JsonValue::Null);
        } else {
            x.back()?;
            list.push(x.next_value()?);
        }
        match x.next_clean() {
            0 => return Err(x.syntax_error("Expected a ',' or ']'")),
            44 => {
                let next_char = x.next_clean();
                if next_char == 0 {
                    return Err(x.syntax_error("Expected a ',' or ']'"));
                }
                if next_char == u16::from(b']') || next_char == u16::from(b',') {
                    return Ok(list);
                }
                x.back()?;
            }
            93 => return Ok(list),
            _ => return Err(x.syntax_error("Expected a ',' or ']'")),
        }
    }
}
