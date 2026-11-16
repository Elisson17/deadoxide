//! zombie.core.input.Input.getKeyName and org.lwjglx.input.Keyboard.getKeyName.
use crate::zombie::core::translator::Translator;

/// org.lwjglx.input.Keyboard.keyName's length (`new String[256]`).
const KEY_NAME_LENGTH: i32 = 256;

/// org.lwjglx.input.Keyboard.getKeyName: `keyName[code]`. The table is filled by `Keyboard.initKeyNames`, which the
/// game runs when it creates its window (GameWindow.InitDisplay's `Keyboard.create`); a headless state has not run it,
/// so every entry is null. An index outside the array is an ArrayIndexOutOfBoundsException.
pub fn keyboard_get_key_name(code: i32) -> Result<Option<String>, String> {
    if !(0..KEY_NAME_LENGTH).contains(&code) {
        return Err(format!(
            "java.lang.ArrayIndexOutOfBoundsException: Index {code} out of bounds for length {KEY_NAME_LENGTH}"
        ));
    }
    Ok(None)
}

/// zombie.core.input.Input.getKeyName: a mouse button (10000 and up) is `IGUI_mouse_btn_<n>`; a key is its LWJGL name
/// translated by `IGUI_Key_<name>` when the translation exists (`"IGUI_Key_" + null` is "IGUI_Key_null").
pub fn get_key_name(translator: &mut Translator, code: i32) -> Result<Option<String>, String> {
    if code >= 10000 {
        return translator
            .get_text(&format!("IGUI_mouse_btn_{}", code - 10000), &[])
            .map(Some);
    }
    let key_name = keyboard_get_key_name(code)?;
    let translated = translator.get_text_or_null(
        &format!("IGUI_Key_{}", key_name.as_deref().unwrap_or("null")),
        &[],
    )?;
    Ok(translated.or(key_name))
}
