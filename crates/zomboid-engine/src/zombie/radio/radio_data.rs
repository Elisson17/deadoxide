//! zombie.radio.RadioData: the radio's channels, scripts, broadcasts and lines, read from `media/radio/*.xml`
//! (`RadioData.xml`), with the text of every line from the `RD_<id>` translations.
//!
//! Ported: `fetchAllRadioData` for the vanilla game folder, `ReadFile`, `loadRootInfo`, `loadRadioScripts`'s adverts and
//! channels, `loadScripts`, `loadBroadcast` (the random pre and post segments), `loadExitOptions`, `checkForCustomAirTimer`.
//! Not ported: mods (`fetchRadioData(true)`'s mod folders; none are active on the server the oracle boots), the
//! community translation files (`RadioTranslationData` is ported; `loadRadioScripts`' use of it only fills
//! `currentTranslation`, which nothing reads: `checkForTranslation` has no caller), `getTranslatorNames`, and the
//! debug output.
use std::collections::HashMap;
use std::path::Path;

use super::channel_category::ChannelCategory;
use super::scripting::radio_broad_cast::{BroadcastId, Broadcasts, RadioBroadCast};
use super::scripting::radio_channel::{RadioChannel, RadioDataInfo};
use super::scripting::radio_line::RadioLine;
use super::scripting::radio_script::RadioScript;
use super::{parse_float_java, parse_int_java};
use crate::java;
use crate::java::JavaRandom;
use crate::zombie::core::random::rand_abstract::RandAbstract;
use crate::zombie::core::translator::Translator;
use crate::zombie::util::pz_xml_util::{parse_document, XmlElement};

/// zombie.radio.RadioData
#[derive(Clone, Debug)]
pub struct RadioData {
    pub is_vanilla: bool,
    pub guid: Option<String>,
    pub version: i32,
    pub xml_file_path: String,
    pub radio_channels: Vec<RadioChannel>,
    root_node: Option<XmlElement>,
    /// `advertQue`: the advert scripts by their GUID (the original keys them by `GetGUID()`)
    advert_que: HashMap<String, RadioScript>,
}

/// What `loadRadioScripts` reads the translations through: `Translator.getText(key)`.
pub trait RadioText {
    fn text(&mut self, key: &str) -> Result<String, String>;
}

impl RadioText for Translator {
    fn text(&mut self, key: &str) -> Result<String, String> {
        self.get_text(key, &[])
    }
}

const NPE_ATTR: &str = "java.lang.NullPointerException: Cannot invoke \"org.w3c.dom.Node.getTextContent()\" because the return value of \"org.w3c.dom.NamedNodeMap.getNamedItem(String)\" is null";

/// `getAttrib(node, name)`: `node.getAttributes().getNamedItem(name).getTextContent().trim()`; a missing attribute is
/// the original's `NullPointerException`.
fn get_attrib(node: &XmlElement, name: &str) -> Result<String, String> {
    node.attributes
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| java::trim(v).to_string())
        .ok_or_else(|| NPE_ATTR.to_string())
}

/// The first element named `name` in document order, the element itself included
/// (`Document.getElementsByTagName(name).item(0)`).
fn first_by_tag<'a>(e: &'a XmlElement, name: &str) -> Option<&'a XmlElement> {
    if e.name == name {
        return Some(e);
    }
    e.elements().find_map(|c| first_by_tag(c, name))
}

impl RadioData {
    /// `RadioData(xmlFile)`
    pub fn new(xml_file: &str) -> Self {
        RadioData {
            is_vanilla: false,
            guid: None,
            version: 0,
            xml_file_path: xml_file.to_string(),
            radio_channels: Vec::new(),
            root_node: None,
            advert_que: HashMap::new(),
        }
    }

    /// `ReadFile(filePath)` over the bytes of the file: the XML parser of the JDK reads them in the encoding of the byte
    /// order mark or of the `encoding` of the XML declaration (`decode_xml`); bytes that are not valid in it end the parse.
    pub fn read_file_bytes(file_path: &str, bytes: &[u8]) -> Option<RadioData> {
        RadioData::read_file(file_path, &decode_xml(bytes)?)
    }

    /// `ReadFile(filePath)` over the text of the file: the data when a `RadioData` element holds a `RootInfo` with a
    /// `FileGUID` and a `Version` that is not negative; `None` for anything else (the original logs the exception that
    /// ended a parse).
    pub fn read_file(file_path: &str, text: &str) -> Option<RadioData> {
        let mut d = RadioData::new(file_path);
        let doc = parse_document(text).ok()?;
        let node = first_by_tag(&doc, "RadioData")?;
        d.root_node = Some(node.clone());
        d.load_root_info().ok()?;
        (d.guid.is_some() && d.version >= 0).then_some(d)
    }

    /// `loadRootInfo` (private): the `Version` and `FileGUID` of every `RootInfo` child of the root; a `Version` that
    /// does not parse is the original's `NumberFormatException`.
    fn load_root_info(&mut self) -> Result<(), String> {
        let root = self.root_node.clone().unwrap_or_default();
        for n in root.elements() {
            if n.name == "RootInfo" {
                for n2 in n.elements() {
                    let name = java::trim(&n2.name);
                    let val = n2.text_content();
                    if name == "Version" {
                        self.version = parse_int_java(&val)?;
                    } else if name == "FileGUID" {
                        self.guid = Some(val);
                    }
                }
            }
        }
        Ok(())
    }

    /// `loadRadioScripts` (private): the adverts, then the channels. True when there is a `Channels` element. An
    /// exception (a missing attribute, a number that does not parse, an unknown category) ends the load; what was read
    /// before it stays (`radio_channels`).
    pub fn load_radio_scripts<R: JavaRandom, T: RadioText>(
        &mut self,
        broadcasts: &mut Broadcasts,
        tr: &mut T,
        rand: &mut RandAbstract<R>,
    ) -> Result<bool, String> {
        let mut success = false;
        self.advert_que.clear();
        let root = self.root_node.clone().unwrap_or_default();
        for n in root.elements() {
            if n.name == "Adverts" {
                self.load_adverts(n, broadcasts, tr, rand)?;
            }
        }
        for n in root.elements() {
            if n.name == "Channels" {
                self.load_channels(n, broadcasts, tr, rand)?;
                success = true;
            }
        }
        self.root_node = None;
        Ok(success)
    }

    /// `loadAdverts`: the advert scripts, by GUID, unless one is already queued under the script's name.
    fn load_adverts<R: JavaRandom, T: RadioText>(
        &mut self,
        parent: &XmlElement,
        broadcasts: &mut Broadcasts,
        tr: &mut T,
        rand: &mut RandAbstract<R>,
    ) -> Result<(), String> {
        let scripts = self.load_scripts(parent, true, broadcasts, tr, rand)?;
        for script in scripts {
            if !self.advert_que.contains_key(&script.name) {
                self.advert_que.insert(script.guid.clone(), script);
            }
        }
        Ok(())
    }

    /// `loadChannels`
    fn load_channels<R: JavaRandom, T: RadioText>(
        &mut self,
        parent: &XmlElement,
        broadcasts: &mut Broadcasts,
        tr: &mut T,
        rand: &mut RandAbstract<R>,
    ) -> Result<(), String> {
        for n in parent.elements() {
            if n.name == "ChannelEntry" {
                let id = get_attrib(n, "ID")?;
                let name = get_attrib(n, "name")?;
                let cat = get_attrib(n, "cat")?;
                let freq = get_attrib(n, "freq")?;
                let startscript = get_attrib(n, "startscript")?;
                let freq = parse_int_java(&freq)?;
                let cat = ChannelCategory::value_of(&cat)?;
                let mut chan = RadioChannel::new(&name, freq, cat, &id);
                let scripts = self.load_scripts(n, false, broadcasts, tr, rand)?;
                for script in scripts {
                    chan.add_radio_script(script);
                }
                chan.set_active_script(Some(&startscript), 0, rand);
                chan.radio_data = Some(RadioDataInfo {
                    is_vanilla: self.is_vanilla,
                    guid: self.guid.clone(),
                });
                self.radio_channels.push(chan);
            }
        }
        Ok(())
    }

    /// `loadScripts(parent, scripts, advertScript)`
    fn load_scripts<R: JavaRandom, T: RadioText>(
        &mut self,
        parent: &XmlElement,
        advert_script: bool,
        broadcasts: &mut Broadcasts,
        tr: &mut T,
        rand: &mut RandAbstract<R>,
    ) -> Result<Vec<RadioScript>, String> {
        let mut scripts = Vec::new();
        for n in parent.elements() {
            if n.name == "ScriptEntry" {
                let id = get_attrib(n, "ID")?;
                let name = get_attrib(n, "name")?;
                let loopmin = get_attrib(n, "loopmin")?;
                let loopmax = get_attrib(n, "loopmax")?;
                let mut script = RadioScript::new(
                    &name,
                    parse_int_java(&loopmin)?,
                    parse_int_java(&loopmax)?,
                    &id,
                );
                for nn in n.elements() {
                    if nn.name == "BroadcastEntry" {
                        self.load_broadcast(nn, Some(&mut script), broadcasts, tr, rand)?;
                    } else if !advert_script && nn.name == "ExitOptions" {
                        Self::load_exit_options(nn, &mut script)?;
                    }
                }
                scripts.push(script);
            }
        }
        Ok(scripts)
    }

    /// `loadBroadcast(broadcast, script)`: a broadcast that is not a segment may get one random advert as its pre or
    /// post segment (draws `Rand.Next(101) < 75`, `Rand.Next(101) < 50` and `Rand.Next(size)`).
    fn load_broadcast<R: JavaRandom, T: RadioText>(
        &mut self,
        broadcast: &XmlElement,
        script: Option<&mut RadioScript>,
        broadcasts: &mut Broadcasts,
        tr: &mut T,
        rand: &mut RandAbstract<R>,
    ) -> Result<BroadcastId, String> {
        let id = get_attrib(broadcast, "ID")?;
        let startstamp = get_attrib(broadcast, "timestamp")?;
        let endstamp = get_attrib(broadcast, "endstamp")?;
        let start_stamp_int = parse_int_java(&startstamp)?;
        let end_stamp_int = parse_int_java(&endstamp)?;
        let segment = get_attrib(broadcast, "isSegment")?;
        let is_segment = java::to_lower_case(&segment) == "true";
        let advert_cat = get_attrib(broadcast, "advertCat")?;
        let mut bc = RadioBroadCast::new(&id, start_stamp_int, end_stamp_int);
        if !is_segment
            && java::to_lower_case(&advert_cat) != "none"
            && self.advert_que.contains_key(&advert_cat)
            && rand.next(101) < 75
        {
            let list = self.advert_que[&advert_cat].broadcasts.clone();
            if !list.is_empty() {
                if rand.next(101) < 50 {
                    bc.pre_segment = Some(list[rand.next(list.len() as i32) as usize]);
                } else {
                    bc.post_segment = Some(list[rand.next(list.len() as i32) as usize]);
                }
            }
        }
        for l in broadcast.elements() {
            if l.name == "LineEntry" {
                let line_id = get_attrib(l, "ID")?;
                let r = get_attrib(l, "r")?;
                let g = get_attrib(l, "g")?;
                let b = get_attrib(l, "b")?;
                let codes = if l.attributes.iter().any(|(n, _)| n == "codes") {
                    Some(get_attrib(l, "codes")?)
                } else {
                    None
                };
                let text = tr.text(&format!("RD_{line_id}"))?;
                let mut radio_line = RadioLine::new(
                    Some(&text),
                    parse_float_java(&r)? / 255.0,
                    parse_float_java(&g)? / 255.0,
                    parse_float_java(&b)? / 255.0,
                    codes.as_deref(),
                );
                let text = java::trim(&text).to_string();
                if java::to_lower_case(&text).starts_with("${t:") {
                    let t = Self::check_for_custom_air_timer(&text, &mut radio_line);
                    radio_line.text = t;
                }
                bc.add_radio_line(radio_line);
            }
        }
        let bid = broadcasts.add(bc);
        if let Some(script) = script {
            script.add_broadcast(bid, is_segment, broadcasts);
        }
        Ok(bid)
    }

    /// `loadExitOptions`
    fn load_exit_options(
        exit_options: &XmlElement,
        script: &mut RadioScript,
    ) -> Result<(), String> {
        for exit_option in exit_options.elements() {
            if exit_option.name == "ExitOption" {
                let script_name = get_attrib(exit_option, "script")?;
                let chance = get_attrib(exit_option, "chance")?;
                let delay = get_attrib(exit_option, "delay")?;
                let chance_int = parse_int_java(&chance)?;
                let delay_int = parse_int_java(&delay)?;
                script.add_exit_option(&script_name, chance_int, delay_int);
            }
        }
        Ok(())
    }

    /// `checkForCustomAirTimer(line, radioLine)`: the first `${t:<number>}` of the text gives the line's air time and is
    /// cut out; the text then starts with `[cdt=<number>]` when the number is not negative. A number that does not parse
    /// leaves the air time as it was (the original logs it); the pattern is case sensitive.
    fn check_for_custom_air_timer(line: &str, radio_line: &mut RadioLine) -> String {
        let mut result = line.to_string();
        let mut val = -1.0f32;
        // Pattern.compile("\\$\\{t:([^}]+)\\}").matcher(line).find()
        let mut from = 0;
        while let Some(rel) = line[from..].find("${t:") {
            let start = from + rel;
            let body = start + 4;
            if let Some(close_rel) = line[body..].find('}') {
                if close_rel > 0 {
                    let group = &line[body..body + close_rel];
                    let g = java::to_lower_case(group);
                    if let Ok(v) = parse_float_java(java::trim(&g)) {
                        val = v;
                        radio_line.set_air_time(v);
                    }
                    result = format!("{}{}", &line[..start], &line[body + close_rel + 1..]);
                    break;
                }
            }
            from = start + 1;
        }
        if val >= 0.0 {
            format!(
                "[cdt={}]{}",
                java::float_to_string(val),
                java::trim(&result)
            )
        } else {
            java::trim(&result).to_string()
        }
    }
}

/// `searchForFiles(path, extension, files)` (private static): every file under `path`, depth first, whose absolute path
/// (lower-cased) contains `extension`. Directory entries are visited in name order (the original uses the file system's).
fn search_for_files(path: &Path, extension: &str, files: &mut Vec<String>) {
    if path.is_dir() {
        let mut names: Vec<String> = std::fs::read_dir(path)
            .map(|d| {
                d.filter_map(Result::ok)
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        for n in names {
            search_for_files(&path.join(n), extension, files);
        }
    } else {
        let abs = path.to_string_lossy().into_owned();
        if java::to_lower_case(&abs).contains(extension) {
            files.push(abs);
        }
    }
}

/// `RadioData.fetchAllRadioData()` for the vanilla files of the game folder `root` (`media/radio/*.xml`): every file that
/// reads as radio data, loaded with its scripts (`loadRadioScripts`); one whose scripts fail to load is dropped, and an
/// exception that escapes `loadRadioScripts` is returned (the original lets it through to `ZomboidRadio.Init`'s catch).
/// The broadcasts of all files are in the returned arena.
pub fn fetch_all_radio_data<R: JavaRandom, T: RadioText>(
    root: &Path,
    tr: &mut T,
    rand: &mut RandAbstract<R>,
) -> Result<(Vec<RadioData>, Broadcasts), String> {
    let mut files = Vec::new();
    search_for_files(&root.join("media").join("radio"), "xml", &mut files);
    let vanilla = files.clone();
    let mut list: Vec<RadioData> = Vec::new();
    for file in &files {
        let rd = std::fs::read(file)
            .ok()
            .and_then(|b| RadioData::read_file_bytes(file, &b));
        if let Some(mut rd) = rd {
            for v in &vanilla {
                if v == file {
                    rd.is_vanilla = true;
                }
            }
            list.push(rd);
        }
    }
    let mut broadcasts = Broadcasts::default();
    let mut i = list.len();
    while i > 0 {
        i -= 1;
        if !list[i].load_radio_scripts(&mut broadcasts, tr, rand)? {
            list.remove(i);
        }
    }
    Ok((list, broadcasts))
}

/// The text of an XML file as the JDK's parser (Xerces) reads it: a byte order mark (UTF-8, UTF-16) or the first bytes
/// of a UTF-16 declaration choose the encoding; otherwise the `encoding` of the declaration does (UTF-8 when there is
/// none), and a UTF-8 byte order mark before a declaration of another 8-bit encoding is skipped. `None` is a fatal
/// error of the parser: bytes invalid in the encoding, an unknown or unsupported encoding name (only UTF-8, UTF-16,
/// US-ASCII, ISO-8859-1, ISO-8859-15 and windows-1252 are ported; other JDK charsets are UNKNOWN), a UTF-16 stream whose
/// declaration names another encoding, an XML version other than 1.0 and 1.1.
fn decode_xml(bytes: &[u8]) -> Option<String> {
    #[derive(PartialEq)]
    enum Enc {
        Utf8,
        Utf16Be,
        Utf16Le,
        Latin1,
        Latin9,
        Cp1252,
        Ascii,
    }
    let (body, wide) = if let Some(r) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        (r, None)
    } else if let Some(r) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        (r, Some(Enc::Utf16Be))
    } else if let Some(r) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        (r, Some(Enc::Utf16Le))
    } else if bytes.starts_with(&[0x00, 0x3C, 0x00, 0x3F]) {
        (bytes, Some(Enc::Utf16Be))
    } else if bytes.starts_with(&[0x3C, 0x00, 0x3F, 0x00]) {
        (bytes, Some(Enc::Utf16Le))
    } else {
        (bytes, None)
    };
    let text = if let Some(enc) = wide {
        if body.len() % 2 != 0 {
            return None;
        }
        let units: Vec<u16> = body
            .chunks_exact(2)
            .map(|c| {
                if enc == Enc::Utf16Be {
                    u16::from_be_bytes([c[0], c[1]])
                } else {
                    u16::from_le_bytes([c[0], c[1]])
                }
            })
            .collect();
        let text: String = char::decode_utf16(units.iter().copied())
            .collect::<Result<String, _>>()
            .ok()?;
        if let Some(name) = declared_encoding(&text) {
            let n = name.to_ascii_lowercase();
            if !(n.starts_with("utf-16") || n.starts_with("utf16")) {
                return None;
            }
        }
        text
    } else {
        let head: String = body.iter().take(200).map(|&b| char::from(b)).collect();
        let enc = match declared_encoding(&head) {
            None => Enc::Utf8,
            Some(name) => match name.to_ascii_lowercase().as_str() {
                "utf-8" | "utf8" => Enc::Utf8,
                "iso-8859-1" | "iso8859-1" | "iso_8859-1" | "iso8859_1" | "iso_8859_1"
                | "latin1" | "l1" | "iso-ir-100" | "cp819" | "ibm819" | "csisolatin1"
                | "8859_1" => Enc::Latin1,
                "iso-8859-15" | "iso8859-15" | "iso_8859-15" | "iso8859_15" | "iso_8859_15"
                | "latin9" | "latin-9" | "l9" | "8859_15" => Enc::Latin9,
                "windows-1252" | "cp1252" => Enc::Cp1252,
                "us-ascii" | "ascii" | "iso646-us" | "ansi_x3.4-1968" | "cp367" | "ibm367"
                | "us" | "646" | "csascii" | "iso-ir-6" | "ansi_x3.4-1986" | "iso_646.irv:1991" => {
                    Enc::Ascii
                }
                _ => return None,
            },
        };
        match enc {
            Enc::Utf8 => String::from_utf8(body.to_vec()).ok()?,
            Enc::Ascii => {
                if body.iter().any(|&b| b >= 0x80) {
                    return None;
                }
                body.iter().map(|&b| char::from(b)).collect()
            }
            Enc::Latin1 => body.iter().map(|&b| char::from(b)).collect(),
            Enc::Latin9 => body
                .iter()
                .map(|&b| match b {
                    0xA4 => '\u{20AC}',
                    0xA6 => '\u{0160}',
                    0xA8 => '\u{0161}',
                    0xB4 => '\u{017D}',
                    0xB8 => '\u{017E}',
                    0xBC => '\u{0152}',
                    0xBD => '\u{0153}',
                    0xBE => '\u{0178}',
                    _ => char::from(b),
                })
                .collect(),
            Enc::Cp1252 => {
                const HIGH: [u32; 32] = [
                    0x20AC, 0xFFFD, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030,
                    0x0160, 0x2039, 0x0152, 0xFFFD, 0x017D, 0xFFFD, 0xFFFD, 0x2018, 0x2019, 0x201C,
                    0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0xFFFD,
                    0x017E, 0x0178,
                ];
                body.iter()
                    .map(|&b| {
                        if (0x80..0xA0).contains(&b) {
                            char::from_u32(HIGH[usize::from(b - 0x80)]).unwrap_or('\u{FFFD}')
                        } else {
                            char::from(b)
                        }
                    })
                    .collect()
            }
            Enc::Utf16Be | Enc::Utf16Le => return None,
        }
    };
    if let Some(v) = declared_attribute(&text, "version") {
        if v != "1.0" && v != "1.1" {
            return None;
        }
    }
    Some(text)
}

/// The value of `attribute` in an XML declaration at the very start of `text`, if there is one.
fn declared_attribute(text: &str, attribute: &str) -> Option<String> {
    let rest = text.strip_prefix("<?xml")?;
    let decl = &rest[..rest.find("?>")?];
    if !decl.starts_with([' ', '\t', '\r', '\n']) {
        return None;
    }
    let mut from = 0;
    while let Some(i) = decl[from..].find(attribute) {
        let at = from + i;
        let before_ok = at == 0 || decl[..at].ends_with([' ', '\t', '\r', '\n']);
        let after = decl[at + attribute.len()..].trim_start_matches([' ', '\t', '\r', '\n']);
        if before_ok {
            if let Some(v) = after.strip_prefix('=') {
                let v = v.trim_start_matches([' ', '\t', '\r', '\n']);
                let q = v.chars().next().filter(|c| *c == '"' || *c == '\'')?;
                let v = &v[1..];
                return Some(v[..v.find(q)?].to_string());
            }
        }
        from = at + attribute.len();
    }
    None
}

fn declared_encoding(text: &str) -> Option<String> {
    declared_attribute(text, "encoding")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the JDK parser does with the encoding of a file, measured on the original (oracle `radio`, x_*.xml).
    #[test]
    fn xml_encodings_follow_the_jdk_parser() {
        let latin = b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><a>Caf\xe9</a>";
        assert_eq!(
            decode_xml(latin)
                .as_deref()
                .map(|t| t.ends_with("Caf\u{e9}</a>")),
            Some(true)
        );
        // a UTF-8 byte order mark before another 8-bit encoding is skipped
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend_from_slice(latin);
        assert!(decode_xml(&bom).is_some_and(|t| t.starts_with("<?xml")));
        // invalid UTF-8, bytes above 0x7F in US-ASCII, an unknown name, XML 2.0: fatal errors
        assert_eq!(decode_xml(b"<a>\xe9</a>"), None);
        assert_eq!(
            decode_xml(b"<?xml version=\"1.0\" encoding=\"US-ASCII\"?><a>\xe9</a>"),
            None
        );
        assert_eq!(
            decode_xml(b"<?xml version=\"1.0\" encoding=\"Klingon\"?><a/>"),
            None
        );
        assert_eq!(decode_xml(b"<?xml version=\"2.0\"?><a/>"), None);
        assert!(decode_xml(b"<?xml version=\"1.1\"?><a/>").is_some());
        // UTF-16 with a byte order mark, and without one (the first bytes of "<?")
        let mut wide = vec![0xFF, 0xFE];
        wide.extend("<a>\u{4e2d}</a>".encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(decode_xml(&wide).as_deref(), Some("<a>\u{4e2d}</a>"));
        let bare: Vec<u8> = "<?xml version=\"1.0\"?><a/>"
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect();
        assert!(decode_xml(&bare).is_some());
        // a UTF-16 stream that declares UTF-8
        let mut lie = vec![0xFF, 0xFE];
        lie.extend(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><a/>"
                .encode_utf16()
                .flat_map(u16::to_le_bytes),
        );
        assert_eq!(decode_xml(&lie), None);
    }
}
