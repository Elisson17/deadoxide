//! zombie.util.PZXmlUtil: the game's XML files read into a DOM (the JDK's non-validating `DocumentBuilder`), and the
//! javax.xml.bind conversions through which the `@XmlElement` classes get their values.
use std::path::Path;

use crate::java;
use crate::zombie::util::string_utils;
use crate::zombie::zomboid_file_system::ZomboidFileSystem;

/// A DOM child: an element, text (CDATA included), or a comment or processing instruction (`Other`: JAXB and
/// `getTextContent` skip it, but it separates text nodes and makes an element not text-only for `PZXmlUtil.resolve`).
#[derive(Clone, Debug, PartialEq)]
pub enum XmlNode {
    Element(XmlElement),
    Text(String),
    Other,
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
            _ => None,
        })
    }

    /// The text a JAXB simple-content property reads: the element's text children, joined, whitespace kept.
    pub fn text(&self) -> String {
        self.children
            .iter()
            .filter_map(|c| match c {
                XmlNode::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect()
    }

    /// org.w3c.dom.Node.getTextContent of an element: the text of every descendant text node, in document order
    /// (comments and processing instructions left out).
    pub fn text_content(&self) -> String {
        let mut s = String::new();
        self.append_text_content(&mut s);
        s
    }

    fn append_text_content(&self, s: &mut String) {
        for c in &self.children {
            match c {
                XmlNode::Text(t) => s.push_str(t),
                XmlNode::Element(e) => e.append_text_content(s),
                XmlNode::Other => {}
            }
        }
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
        } else if c.is_comment() || c.is_pi() {
            e.children.push(XmlNode::Other);
        }
    }
    e
}

/// zombie.util.PZXmlUtil.parseXmlInternal over the text of a file: the root element (a UTF-8 byte order mark is
/// skipped). Element names are local names: the original's parser is not namespace-aware and would keep a
/// `prefix:` (no game file seen so far uses one). `x_include` and `x_extends` (another file merged in by
/// `PZXmlUtil.parseXml`) are not ported and are an `Err` here rather than a silent difference.
pub fn parse_xml_str(text: &str) -> Result<XmlElement, String> {
    let root = parse_document(text)?;
    for a in ["x_include", "x_extends"] {
        if !root.attribute(a).trim().is_empty() {
            return Err(format!("PZXmlUtil.parseXml: {a} is not ported"));
        }
    }
    Ok(root)
}

/// The document element of an XML text, normalized (`DocumentBuilder.parse`, then `Element.normalize()`).
fn parse_document(text: &str) -> Result<XmlElement, String> {
    let opt = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };
    let doc = roxmltree::Document::parse_with_options(text, opt)
        .map_err(|e| format!("org.xml.sax.SAXParseException: {e}"))?;
    Ok(convert(doc.root_element()))
}

/// zombie.util.PZXmlUtil.parseXmlInternal: the file `file_name` (relative to the game folder, the original's working
/// directory) parsed and normalized. `Err` is the original's IOException or SAXException.
pub fn parse_xml_internal(fs: &ZomboidFileSystem, file_name: &str) -> Result<XmlElement, String> {
    let path = fs.base().join(file_name);
    let bytes = std::fs::read(&path).map_err(|e| format!("java.io.IOException: {file_name}: {e}"))?;
    let text = String::from_utf8(bytes).map_err(|e| format!("{file_name}: {e}"))?;
    parse_document(text.strip_prefix('\u{feff}').unwrap_or(&text))
}

/// zombie.util.PZXmlUtil.parseXml(String): the file a path or guid names (`resolveFileOrGUID`), with its `x_include`
/// merged in, then, when its root has `x_extends`, resolved over the file that names (a guid, else a path relative to
/// this file; parsed the same way, recursively). `Err` is PZXmlParserException.
pub fn parse_xml_resolved(fs: &ZomboidFileSystem, source: &str) -> Result<XmlElement, String> {
    let file_name = fs.resolve_file_or_guid(source);
    let root = parse_xml_internal(fs, &file_name).map_err(|e| {
        format!("Exception thrown while parsing XML file \"{file_name}\": {e}")
    })?;
    let root = include_another_file(fs, root, &file_name)?;
    let extends_source = root.attribute("x_extends");
    if java::trim(extends_source).is_empty() {
        return Ok(root);
    }
    let extends_source = if fs.is_valid_file_path_guid(extends_source) {
        extends_source.to_string()
    } else {
        fs.resolve_relative_path(&file_name, extends_source)
    };
    let extended_root = parse_xml_resolved(fs, &extends_source)?;
    Ok(resolve(&root, &extended_root))
}

/// zombie.util.PZXmlUtil.includeAnotherFile: when the root has `x_include`, the elements of the root of the file it
/// names (same tag name required, else the root is returned as is) are inserted before the root's first child, in
/// order; then `normalize()`.
fn include_another_file(
    fs: &ZomboidFileSystem,
    root: XmlElement,
    file_name: &str,
) -> Result<XmlElement, String> {
    let include_source = root.attribute("x_include");
    if java::trim(include_source).is_empty() {
        return Ok(root);
    }
    let include_source = if fs.is_valid_file_path_guid(include_source) {
        include_source.to_string()
    } else {
        fs.resolve_relative_path(file_name, include_source)
    };
    let include_root = parse_xml_resolved(fs, &include_source)?;
    if include_root.name != root.name {
        return Ok(root);
    }
    let mut copy = root;
    let inserted: Vec<XmlNode> = include_root
        .children
        .into_iter()
        .filter(|c| matches!(c, XmlNode::Element(_)))
        .collect();
    copy.children.splice(0..0, inserted);
    normalize(&mut copy);
    Ok(copy)
}

/// org.w3c.dom.Node.normalize: adjacent text nodes merged and empty ones removed, at every depth.
fn normalize(e: &mut XmlElement) {
    let mut out: Vec<XmlNode> = Vec::with_capacity(e.children.len());
    for c in std::mem::take(&mut e.children) {
        match c {
            XmlNode::Text(t) => {
                if t.is_empty() {
                    continue;
                }
                if let Some(XmlNode::Text(prev)) = out.last_mut() {
                    prev.push_str(&t);
                } else {
                    out.push(XmlNode::Text(t));
                }
            }
            XmlNode::Element(mut c) => {
                normalize(&mut c);
                out.push(XmlNode::Element(c));
            }
            XmlNode::Other => out.push(XmlNode::Other),
        }
    }
    e.children = out;
}

/// zombie.util.PZXmlUtil.isTextOnly: every child is a text node that is not all whitespace (and there is one).
fn is_text_only(root: &XmlElement) -> bool {
    let mut result = false;
    for c in &root.children {
        let is_text = match c {
            XmlNode::Text(t) => !string_utils::is_null_or_whitespace(Some(t)),
            _ => false,
        };
        if !is_text {
            result = false;
            break;
        }
        result = true;
    }
    result
}

/// zombie.util.PZXmlUtil.resolve(Element, Element, Document): a text-only `root` replaces `parent`; else a new
/// element named after `root` with the parent's attributes overridden or extended by the root's, and the child
/// elements of both merged by `TagTable` (the parent's in its order, each resolved with the root's element of the
/// same tag and `x_name` (or, unnamed, the same index among that tag's entries), then the root's unmatched ones). Text
/// of a merged element is dropped.
pub fn resolve(root: &XmlElement, parent: &XmlElement) -> XmlElement {
    if is_text_only(root) {
        return root.clone();
    }
    let mut attributes = parent.attributes.clone();
    for (name, value) in &root.attributes {
        match attributes.iter_mut().find(|(n, _)| n == name) {
            Some(a) => a.1 = value.clone(),
            None => attributes.push((name.clone(), value.clone())),
        }
    }
    let mut parent_table = TagTable::create(parent);
    let child_table = TagTable::create(root);
    parent_table.resolve_with(child_table);
    XmlElement {
        name: root.name.clone(),
        attributes,
        children: parent_table
            .resolved_elements
            .into_iter()
            .map(|e| XmlNode::Element(e.element))
            .collect(),
    }
}

/// zombie.util.PZXmlUtil.NamedTagEntry
#[derive(Clone)]
struct NamedTagEntry {
    tag: String,
    name: String,
    element: XmlElement,
    index: i32,
}

impl NamedTagEntry {
    /// zombie.util.PZXmlUtil.NamedTagEntry.getUniqueKey: the `x_name`, or "node_<index>" when blank.
    fn unique_key(&self) -> String {
        if string_utils::is_null_or_whitespace(Some(&self.name)) {
            format!("node_{}", self.index)
        } else {
            self.name.clone()
        }
    }
}

/// zombie.util.PZXmlUtil.TagTable: the entries in order (`resolvedElements`), and per tag the `NamedTags` HashMap from
/// unique key to entry (an index into `resolved_elements`), iterated in Java's HashMap order where it matters.
struct TagTable {
    tags: std::collections::HashMap<String, java::JavaHashMap<usize>>,
    resolved_elements: Vec<NamedTagEntry>,
}

impl TagTable {
    /// zombie.util.PZXmlUtil.TagTable.createTagTable: an entry for each child element, in order.
    fn create(root: &XmlElement) -> TagTable {
        let mut t = TagTable {
            tags: std::collections::HashMap::new(),
            resolved_elements: Vec::new(),
        };
        for e in root.elements() {
            t.add_element(e.clone());
        }
        t
    }

    /// zombie.util.PZXmlUtil.TagTable.getEntry: by `x_name` when the entry has one, else the first entry of the tag
    /// (in HashMap order) whose index is the same.
    fn get_entry(&self, child: &NamedTagEntry) -> Option<usize> {
        let named = self.tags.get(&child.tag)?;
        if string_utils::is_null_or_whitespace(Some(&child.name)) {
            named
                .values()
                .find(|&&i| self.resolved_elements[i].index == child.index)
                .copied()
        } else {
            named.get(&child.name).copied()
        }
    }

    /// zombie.util.PZXmlUtil.TagTable.addEntry(Element): indexed by the number of keys its tag has so far.
    fn add_element(&mut self, element: XmlElement) {
        let tag = element.name.clone();
        let index = self.tags.get(&tag).map_or(0, |m| m.len() as i32);
        let name = element.attribute("x_name").to_string();
        self.add_entry(NamedTagEntry {
            tag,
            name,
            element,
            index,
        });
    }

    /// zombie.util.PZXmlUtil.TagTable.addEntry(NamedTagEntry)
    fn add_entry(&mut self, entry: NamedTagEntry) {
        let key = entry.unique_key();
        let tag = entry.tag.clone();
        self.resolved_elements.push(entry);
        let i = self.resolved_elements.len() - 1;
        self.tags.entry(tag).or_default().put(key, i);
    }

    /// zombie.util.PZXmlUtil.TagTable.resolveWith: each child entry in order resolves over its parent entry, or is
    /// added.
    fn resolve_with(&mut self, child_table: TagTable) {
        for child in child_table.resolved_elements {
            match self.get_entry(&child) {
                None => self.add_entry(child),
                Some(p) => {
                    let merged = resolve(&child.element, &self.resolved_elements[p].element);
                    self.resolved_elements[p].element = merged;
                }
            }
        }
    }
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
