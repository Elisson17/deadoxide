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

/// XML whitespace as JAXB's `WhiteSpaceProcessor` trims it: space, tab, line feed, carriage return.
fn xml_trim(s: &str) -> &str {
    s.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
}

/// javax.xml.bind.DatatypeConverter.parseBoolean as JAXB fills a `boolean` field: "true" or "1", "false" or "0",
/// around XML whitespace. Anything else is `None` and the field keeps its value (what the original does with other
/// text was not measured).
pub fn jaxb_parse_boolean(s: &str) -> Option<bool> {
    match xml_trim(s) {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

/// javax.xml.bind.DatatypeConverter.parseFloat as JAXB fills a `float` field: trimmed; "NaN", "INF", "-INF"; else
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
        assert_eq!(
            jaxb_parse_float(" 0.3 ").map(f32::to_bits),
            Some(0.3f32.to_bits())
        );
        assert_eq!(jaxb_parse_float("INF"), Some(f32::INFINITY));
        assert_eq!(jaxb_parse_float("1f"), None);
    }
}
