//! zombie.util.PZXmlUtil: the game's XML files read into a DOM (the JDK's non-validating `DocumentBuilder`), and the
//! javax.xml.bind conversions through which the `@XmlElement` classes get their values.
use std::path::Path;

/// A DOM child that JAXB looks at: an element or text (comments and processing instructions are left out).
#[derive(Clone, Debug, PartialEq)]
pub enum XmlNode {
    Element(XmlElement),
    Text(String),
}

/// org.w3c.dom.Element as far as the ported readers use it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct XmlElement {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<XmlNode>,
}

impl XmlElement {
    /// org.w3c.dom.Element.getAttribute: "" when the attribute is absent.
    pub fn attribute(&self, name: &str) -> &str {
        self.attributes
            .iter()
            .find(|(n, _)| n == name)
            .map_or("", |(_, v)| v.as_str())
    }

    /// The child elements in document order.
    pub fn elements(&self) -> impl Iterator<Item = &XmlElement> {
        self.children.iter().filter_map(|c| match c {
            XmlNode::Element(e) => Some(e),
            XmlNode::Text(_) => None,
        })
    }

    /// The text a JAXB simple-content property reads: the element's text children, joined, whitespace kept.
    pub fn text(&self) -> String {
        self.children
            .iter()
            .filter_map(|c| match c {
                XmlNode::Text(t) => Some(t.as_str()),
                XmlNode::Element(_) => None,
            })
            .collect()
    }
}

fn convert(node: roxmltree::Node) -> XmlElement {
    let mut e = XmlElement {
        name: node.tag_name().name().to_string(),
        attributes: node
            .attributes()
            .map(|a| (a.name().to_string(), a.value().to_string()))
            .collect(),
        children: Vec::new(),
    };
    for c in node.children() {
        if c.is_element() {
            e.children.push(XmlNode::Element(convert(c)));
        } else if c.is_text() {
            let t = c.text().unwrap_or("");
            // Element.normalize(): adjacent text nodes become one.
            if let Some(XmlNode::Text(prev)) = e.children.last_mut() {
                prev.push_str(t);
            } else {
                e.children.push(XmlNode::Text(t.to_string()));
            }
        }
    }
    e
}

/// zombie.util.PZXmlUtil.parseXmlInternal over the text of a file: the root element (a UTF-8 byte order mark is
/// skipped). Element names are local names: the original's parser is not namespace-aware and would keep a
/// `prefix:` (no game file seen so far uses one). `x_include` and `x_extends` (another file merged in by
/// `PZXmlUtil.parseXml`) are not ported and are an `Err` here rather than a silent difference.
pub fn parse_xml_str(text: &str) -> Result<XmlElement, String> {
    let opt = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };
    let doc = roxmltree::Document::parse_with_options(text, opt)
        .map_err(|e| format!("org.xml.sax.SAXParseException: {e}"))?;
    let root = convert(doc.root_element());
    for a in ["x_include", "x_extends"] {
        if !root.attribute(a).trim().is_empty() {
            return Err(format!("PZXmlUtil.parseXml: {a} is not ported"));
        }
    }
    Ok(root)
}

/// zombie.util.PZXmlUtil.parseXml(String) for a file path. The file must be UTF-8 (the original decodes the
/// encoding the file declares).
pub fn parse_xml(path: &Path) -> Result<XmlElement, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = String::from_utf8(bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    parse_xml_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// com.sun.xml.bind.WhiteSpaceProcessor.isWhiteSpace(char): space, tab, line feed, carriage return.
fn jaxb_is_white_space(ch: u16) -> bool {
    matches!(ch, 0x20 | 0x09 | 0x0a | 0x0d)
}

/// com.sun.xml.bind.WhiteSpaceProcessor.trim
fn xml_trim(s: &str) -> &str {
    s.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
}

/// com.sun.xml.bind.DatatypeConverterImpl._parseBoolean (the JAXB runtime the game bundles), as JAXB fills a
/// `boolean` field. Kept as written, quirks included: after skipping leading whitespace, '0' and '1' are false and
/// true; 't' must continue "rue" and 'f' "alse", else the result is false; then one more character is read past
/// trailing whitespace and the value stands only if that reaches the end (so "1x" is true and "1xy" is null).
/// `None` is the original's null and also its exception for text that ends inside "true"/"false" (what JAXB then
/// does with a primitive field was not measured; here the field keeps its value).
pub fn jaxb_parse_boolean(literal: &str) -> Option<bool> {
    let s: Vec<u16> = literal.encode_utf16().collect();
    let len = s.len();
    if len == 0 {
        return None;
    }
    let mut i = 0usize;
    let mut ch;
    loop {
        ch = s[i];
        i += 1;
        if !(jaxb_is_white_space(ch) && i < len) {
            break;
        }
    }
    let mut value = false;
    let rest = |word: &[u8], i: &mut usize| -> Option<bool> {
        let mut str_index = 0usize;
        loop {
            let c = *s.get(*i)?;
            *i += 1;
            let matched = u16::from(word[str_index]) == c;
            str_index += 1;
            if !(matched && *i < len && str_index < word.len()) {
                break;
            }
        }
        Some(str_index == word.len())
    };
    match ch {
        0x30 => value = false,
        0x31 => value = true,
        0x66 => {
            if !rest(b"alse", &mut i)? {
                return Some(false);
            }
            value = false;
        }
        0x74 => {
            if !rest(b"rue", &mut i)? {
                return Some(false);
            }
            value = true;
        }
        _ => {}
    }
    if i < len {
        loop {
            ch = s[i];
            i += 1;
            if !(jaxb_is_white_space(ch) && i < len) {
                break;
            }
        }
    }
    (i == len).then_some(value)
}

/// com.sun.xml.bind.DatatypeConverterImpl._parseInt: whitespace anywhere is skipped, digits accumulate (wrapping as
/// int), any '-' makes it negative, '+' is ignored; another character is the original's NumberFormatException
/// (`None`: the list element is not added).
pub fn jaxb_parse_int(s: &str) -> Option<i32> {
    let mut sign = 1i32;
    let mut r = 0i32;
    for ch in s.encode_utf16() {
        if jaxb_is_white_space(ch) {
            continue;
        }
        if (0x30..=0x39).contains(&ch) {
            r = r.wrapping_mul(10).wrapping_add(i32::from(ch - 0x30));
        } else if ch == 0x2d {
            sign = -1;
        } else if ch != 0x2b {
            return None;
        }
    }
    Some(r.wrapping_mul(sign))
}

/// com.sun.xml.bind.DatatypeConverterImpl._parseFloat as JAXB fills a `float` field: trimmed; "NaN", "INF", "-INF"; else
/// the text must start and end with a digit, '.', '+' or '-' and is `Float.parseFloat`. `None` (the field keeps its
/// value) where the original's conversion throws. Java's hexadecimal float literals are not accepted here.
pub fn jaxb_parse_float(s: &str) -> Option<f32> {
    let t = xml_trim(s);
    match t {
        "NaN" => return Some(f32::NAN),
        "INF" => return Some(f32::INFINITY),
        "-INF" => return Some(f32::NEG_INFINITY),
        _ => {}
    }
    let ok =
        |c: Option<char>| c.is_some_and(|c| c.is_ascii_digit() || matches!(c, '.' | '+' | '-'));
    if !ok(t.chars().next()) || !ok(t.chars().last()) {
        return None;
    }
    t.parse::<f32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A byte order mark is skipped, comments are not children, and text is kept as written.
    #[test]
    fn parses_elements_and_text() {
        let root = parse_xml_str(
            "\u{feff}<?xml version=\"1.0\"?><a k=\"v\"><!-- c --><b> x </b><b>y</b></a>",
        )
        .unwrap();
        assert_eq!(
            (root.name.as_str(), root.attribute("k"), root.attribute("z")),
            ("a", "v", "")
        );
        let texts: Vec<String> = root.elements().map(|e| e.text()).collect();
        assert_eq!(texts, [" x ", "y"]);
        assert!(parse_xml_str("<a x_include=\"b.xml\"/>").is_err());
    }

    #[test]
    fn jaxb_conversions() {
        assert_eq!(jaxb_parse_boolean(" true\n"), Some(true));
        assert_eq!(jaxb_parse_boolean("0"), Some(false));
        assert_eq!(jaxb_parse_boolean("yes"), None);
        // the original's quirks: one stray character after the value is read past, "trux" matches "tru"
        assert_eq!(jaxb_parse_boolean("1x"), Some(true));
        assert_eq!(jaxb_parse_boolean("1xy"), None);
        assert_eq!(jaxb_parse_boolean("trux"), Some(true));
        assert_eq!(jaxb_parse_boolean("tx"), Some(false));
        assert_eq!(jaxb_parse_boolean("t"), None);
        assert_eq!(jaxb_parse_int(" 1 2 "), Some(12));
        assert_eq!(jaxb_parse_int("-+7"), Some(-7));
        assert_eq!(jaxb_parse_int("7a"), None);
        assert_eq!(
            jaxb_parse_float(" 0.3 ").map(f32::to_bits),
            Some(0.3f32.to_bits())
        );
        assert_eq!(jaxb_parse_float("INF"), Some(f32::INFINITY));
        assert_eq!(jaxb_parse_float("1f"), None);
    }
}
