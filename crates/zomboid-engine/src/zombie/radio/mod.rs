pub mod channel_category;
pub mod devices;
pub mod media;
pub mod radio_api;
pub mod radio_data;
pub mod radio_translation_data;
pub mod script;
pub mod scripting;
pub mod zomboid_radio;

/// `Integer.parseInt` as the original's `Result` text: `java.lang.NumberFormatException: For input string: "x"`.
pub(crate) fn parse_int_java(s: &str) -> Result<i32, String> {
    crate::java::parse_int(s).ok_or_else(|| {
        format!(
            "java.lang.NumberFormatException: {}",
            crate::java::number_format_message(s)
        )
    })
}

/// `Float.parseFloat` as the original's `Result` text: the trimmed input in the message, "empty String" for an empty one.
pub(crate) fn parse_float_java(s: &str) -> Result<f32, String> {
    crate::java::parse_float(s).ok_or_else(|| {
        let t = crate::java::trim(s);
        format!(
            "java.lang.NumberFormatException: {}",
            if t.is_empty() {
                "empty String".to_string()
            } else {
                crate::java::number_format_message(t)
            }
        )
    })
}
