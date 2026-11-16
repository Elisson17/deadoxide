//! zombie.config.ConfigFile.read, with the `StringConfigOption` it makes of every `name=value` line (`write` and the
//! other option types belong to the saving side and are not ported).
use std::path::Path;

use crate::java;

/// zombie.config.StringConfigOption as ConfigFile.read creates it: `new StringConfigOption(name, value, -1)`.
#[derive(Clone, Debug, PartialEq)]
pub struct StringConfigOption {
    pub name: String,
    pub value: String,
}

impl StringConfigOption {
    /// zombie.config.StringConfigOption(String, String, int): ConfigOption's constructor throws an
    /// IllegalArgumentException for an empty name (or one with '='); `Err` is its `toString()`.
    pub fn new(name: &str, default_value: &str) -> Result<StringConfigOption, String> {
        if name.is_empty() || name.contains('=') {
            return Err("java.lang.IllegalArgumentException".to_string());
        }
        Ok(StringConfigOption {
            name: name.to_string(),
            value: default_value.to_string(),
        })
    }

    /// zombie.config.ConfigOption.getName
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// zombie.config.StringConfigOption.getValueAsString
    pub fn get_value_as_string(&self) -> &str {
        &self.value
    }
}

/// zombie.config.ConfigFile (the reading side)
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigFile {
    pub options: Vec<StringConfigOption>,
    pub version: i32,
    pub version_string: String,
}

impl Default for ConfigFile {
    /// zombie.config.ConfigFile(): no options, version 0, version key "Version".
    fn default() -> Self {
        ConfigFile {
            options: Vec::new(),
            version: 0,
            version_string: "Version".to_string(),
        }
    }
}

impl ConfigFile {
    /// zombie.config.ConfigFile.getVersion
    pub fn get_version(&self) -> i32 {
        self.version
    }

    /// zombie.config.ConfigFile.getOptions
    pub fn get_options(&self) -> &[StringConfigOption] {
        &self.options
    }

    /// zombie.config.ConfigFile.read: false when the file does not exist or a line raises an exception (the options
    /// read before it stay in `options`). Lines are the text `BufferedReader.readLine` returns (the file read as
    /// UTF-8), trimmed; blank lines and `#` comments are skipped; a line without '=' is only logged; the others are
    /// `line.split("=")` (trailing empty pieces dropped): the `versionString` key sets `version`
    /// (`Integer.parseInt(ss[1])`, a NumberFormatException only logged), any other key becomes an option with value
    /// `ss[1]` or "". `ss[0]` or `ss[1]` past the end of the array (a line of only '=', or "Version=") and an empty
    /// key (IllegalArgumentException) end the read with false.
    pub fn read(&mut self, file_name: &str) -> bool {
        self.options = Vec::new();
        self.version = 0;
        let path = Path::new(file_name);
        if !crate::vfs::exists(path) {
            return false;
        }
        let Ok(bytes) = crate::vfs::read(path) else {
            return false;
        };
        let text = String::from_utf16_lossy(&java::utf8_decode_jdk(&bytes));
        for line in java::buffered_reader_lines(&text) {
            let line = java::trim(line);
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if !line.contains('=') {
                // fileError: DebugLog only
                continue;
            }
            let ss = java::split_char(line, '=');
            let Some(key) = ss.first() else {
                return false;
            };
            if self.version_string == *key {
                let Some(value) = ss.get(1) else {
                    return false;
                };
                if let Some(v) = java::parse_int(value) {
                    self.version = v;
                }
            } else {
                let Ok(option) = StringConfigOption::new(key, ss.get(1).copied().unwrap_or(""))
                else {
                    return false;
                };
                self.options.push(option);
            }
        }
        true
    }
}
