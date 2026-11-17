//! assimp `code/AssetLib/FBX/FBXTokenizer.cpp`, `FBXBinaryTokenizer.cpp`, `FBXParser.cpp`: the tokens of an FBX file
//! (text or binary), the scope/element tree built from them and the parsers of token values and data arrays.
//! `Err` is the error text of the `DeadlyImportError` the library throws.
use super::inflate::inflate_zlib;
use crate::native::assimp::fast_atof::{fast_atoreal_move, strtoul10_64};
use crate::native::assimp::scene::Vec3;

pub type R<T> = Result<T, String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokType {
    OpenBracket,
    CloseBracket,
    Data,
    Comma,
    Key,
}

/// FBX::Token: the bytes of the token (for a binary data token the type code and the payload), its type and (text
/// files) its line; for binary files `line` is the offset in the file.
#[derive(Clone, Debug)]
pub struct Token {
    pub ty: TokType,
    pub data: Vec<u8>,
    pub binary: bool,
    pub line: u32,
}

impl Token {
    /// Token::StringContents
    pub fn string(&self) -> String {
        self.data.iter().map(|&b| b as char).collect()
    }
}

fn tok_err<T>(msg: &str) -> R<T> {
    Err(format!("FBX-Tokenize: {msg}"))
}

// ---------------------------------------------------------------------------------------------------------------
// text tokenizer
// ---------------------------------------------------------------------------------------------------------------

fn is_space(c: u8) -> bool {
    c == b' ' || c == b'\t'
}
fn is_line_end(c: u8) -> bool {
    c == b'\r' || c == b'\n' || c == 0
}
fn is_space_or_newline(c: u8) -> bool {
    is_space(c) || is_line_end(c)
}

fn process_data_token(
    out: &mut Vec<Token>,
    input: &[u8],
    start: &mut Option<usize>,
    end: &mut Option<usize>,
    line: u32,
    ty: TokType,
    must_have_token: bool,
) -> R<()> {
    if let (Some(s), Some(e)) = (*start, *end) {
        let mut in_quotes = false;
        for &c in &input[s..=e] {
            if c == b'"' {
                in_quotes = !in_quotes;
            }
            if !in_quotes && is_space_or_newline(c) {
                return tok_err("unexpected whitespace in token");
            }
        }
        if in_quotes {
            return tok_err("non-terminated double quotes");
        }
        out.push(Token {
            ty,
            data: input[s..=e].to_vec(),
            binary: false,
            line,
        });
    } else if must_have_token {
        return tok_err("missing token");
    }
    *start = None;
    *end = None;
    Ok(())
}

/// `Tokenize(output_tokens, input)`: `input` is the file content (a NUL terminates the scan like the C string).
pub fn tokenize(input: &[u8]) -> R<Vec<Token>> {
    let mut out = Vec::new();
    let mut line: u32 = 1;
    let mut comment = false;
    let mut in_quotes = false;
    let mut pending = false;
    let mut tb: Option<usize> = None;
    let mut te: Option<usize> = None;
    let mut cursor = 0usize;
    let at = |i: usize| input.get(i).copied().unwrap_or(0);
    while at(cursor) != 0 {
        let c = at(cursor);
        'body: {
            if is_line_end(c) {
                comment = false;
                break 'body;
            }
            if comment {
                break 'body;
            }
            if in_quotes {
                if c == b'"' {
                    in_quotes = false;
                    te = Some(cursor);
                    process_data_token(
                        &mut out,
                        input,
                        &mut tb,
                        &mut te,
                        line,
                        TokType::Data,
                        false,
                    )?;
                    pending = false;
                }
                break 'body;
            }
            match c {
                b'"' => {
                    if tb.is_some() {
                        return tok_err("unexpected double-quote");
                    }
                    tb = Some(cursor);
                    in_quotes = true;
                    break 'body;
                }
                b';' => {
                    process_data_token(
                        &mut out,
                        input,
                        &mut tb,
                        &mut te,
                        line,
                        TokType::Data,
                        false,
                    )?;
                    comment = true;
                    break 'body;
                }
                b'{' => {
                    process_data_token(
                        &mut out,
                        input,
                        &mut tb,
                        &mut te,
                        line,
                        TokType::Data,
                        false,
                    )?;
                    out.push(Token {
                        ty: TokType::OpenBracket,
                        data: vec![c],
                        binary: false,
                        line,
                    });
                    break 'body;
                }
                b'}' => {
                    process_data_token(
                        &mut out,
                        input,
                        &mut tb,
                        &mut te,
                        line,
                        TokType::Data,
                        false,
                    )?;
                    out.push(Token {
                        ty: TokType::CloseBracket,
                        data: vec![c],
                        binary: false,
                        line,
                    });
                    break 'body;
                }
                b',' => {
                    if pending {
                        process_data_token(
                            &mut out,
                            input,
                            &mut tb,
                            &mut te,
                            line,
                            TokType::Data,
                            true,
                        )?;
                    }
                    out.push(Token {
                        ty: TokType::Comma,
                        data: vec![c],
                        binary: false,
                        line,
                    });
                    break 'body;
                }
                b':' => {
                    if pending {
                        process_data_token(
                            &mut out,
                            input,
                            &mut tb,
                            &mut te,
                            line,
                            TokType::Key,
                            true,
                        )?;
                    } else {
                        return tok_err("unexpected colon");
                    }
                    break 'body;
                }
                _ => {}
            }
            if is_space_or_newline(c) {
                if tb.is_some() {
                    let mut ty = TokType::Data;
                    let mut peek = cursor;
                    while at(peek) != 0 && is_space_or_newline(at(peek)) {
                        if at(peek) == b':' {
                            ty = TokType::Key;
                            cursor = peek;
                            break;
                        }
                        peek += 1;
                    }
                    process_data_token(&mut out, input, &mut tb, &mut te, line, ty, false)?;
                }
                pending = false;
            } else {
                te = Some(cursor);
                if tb.is_none() {
                    tb = Some(cursor);
                }
                pending = true;
            }
        }
        // increment(cursor, line, column)
        if at(cursor) == b'\n' {
            line += 1;
        }
        cursor += 1;
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------------------
// binary tokenizer
// ---------------------------------------------------------------------------------------------------------------

fn read_word(input: &[u8], cursor: &mut usize, end: usize) -> R<u32> {
    if end.saturating_sub(*cursor) < 4 {
        return tok_err("cannot ReadWord, out of bounds");
    }
    let v = u32::from_le_bytes([
        input[*cursor],
        input[*cursor + 1],
        input[*cursor + 2],
        input[*cursor + 3],
    ]);
    *cursor += 4;
    Ok(v)
}

fn read_double_word(input: &[u8], cursor: &mut usize, end: usize) -> R<u64> {
    if end.saturating_sub(*cursor) < 8 {
        return tok_err("cannot ReadDoubleWord, out of bounds");
    }
    let mut b = [0u8; 8];
    b.copy_from_slice(&input[*cursor..*cursor + 8]);
    *cursor += 8;
    Ok(u64::from_le_bytes(b))
}

fn read_byte(input: &[u8], cursor: &mut usize, end: usize) -> R<u8> {
    if end.saturating_sub(*cursor) < 1 {
        return tok_err("cannot ReadByte, out of bounds");
    }
    let b = input[*cursor];
    *cursor += 1;
    Ok(b)
}

fn read_data(input: &[u8], cursor: &mut usize, end: usize) -> R<Token> {
    if end.saturating_sub(*cursor) < 1 {
        return tok_err("cannot ReadData, out of bounds reading length");
    }
    let ty = input[*cursor];
    let begin = *cursor;
    *cursor += 1;
    match ty {
        b'Y' => *cursor += 2,
        b'C' => *cursor += 1,
        b'I' | b'F' => *cursor += 4,
        b'D' | b'L' => *cursor += 8,
        b'R' | b'S' => {
            let length = read_word(input, cursor, end)?;
            *cursor += length as usize;
        }
        b'b' => *cursor = end,
        b'f' | b'd' | b'l' | b'i' => {
            let length = read_word(input, cursor, end)?;
            let encoding = read_word(input, cursor, end)?;
            let comp_len = read_word(input, cursor, end)?;
            if encoding == 0 {
                let stride: u32 = if ty == b'f' || ty == b'i' { 4 } else { 8 };
                if length.wrapping_mul(stride) != comp_len {
                    return tok_err(
                        "cannot ReadData, calculated data stride differs from what the file claims",
                    );
                }
            } else if encoding != 1 {
                return tok_err("cannot ReadData, unknown encoding");
            }
            *cursor += comp_len as usize;
        }
        _ => return tok_err("cannot ReadData, unexpected type code"),
    }
    if *cursor > end {
        return tok_err("cannot ReadData, the remaining size is too small for the data type");
    }
    Ok(Token {
        ty: TokType::Data,
        data: input[begin..*cursor].to_vec(),
        binary: true,
        line: *cursor as u32,
    })
}

fn read_scope(
    out: &mut Vec<Token>,
    input: &[u8],
    cursor: &mut usize,
    end: usize,
    is64: bool,
) -> R<bool> {
    let word = |c: &mut usize| -> R<u64> {
        if is64 {
            read_double_word(input, c, end)
        } else {
            read_word(input, c, end).map(u64::from)
        }
    };
    let end_offset = word(cursor)?;
    if end_offset == 0 {
        return Ok(false);
    }
    if end_offset > end as u64 {
        return tok_err("block offset is out of range");
    } else if end_offset < *cursor as u64 {
        return tok_err("block offset is negative out of range");
    }
    let prop_count = word(cursor)?;
    let prop_length = word(cursor)?;
    // the name of the scope/key
    let name_len = read_byte(input, cursor, end)? as usize;
    if end - *cursor < name_len {
        return tok_err("cannot ReadString, out of bounds reading string");
    }
    let name = input[*cursor..*cursor + name_len].to_vec();
    *cursor += name_len;
    out.push(Token {
        ty: TokType::Key,
        data: name,
        binary: true,
        line: *cursor as u32,
    });
    let begin_cursor = *cursor;
    let prop_end = begin_cursor.saturating_add(prop_length as usize).min(end);
    for i in 0..prop_count {
        let t = read_data(input, cursor, prop_end)?;
        out.push(t);
        if i != prop_count - 1 {
            out.push(Token {
                ty: TokType::Comma,
                data: vec![0],
                binary: true,
                line: *cursor as u32,
            });
        }
    }
    if (*cursor - begin_cursor) as u64 != prop_length {
        return tok_err("property length not reached, something is wrong");
    }
    let sentinel: u64 = if is64 { 25 } else { 13 };
    if (*cursor as u64) < end_offset {
        if end_offset - (*cursor as u64) < sentinel {
            return tok_err("insufficient padding bytes at block end");
        }
        out.push(Token {
            ty: TokType::OpenBracket,
            data: vec![0],
            binary: true,
            line: *cursor as u32,
        });
        while (*cursor as u64) < end_offset - sentinel {
            read_scope(out, input, cursor, end, is64)?;
        }
        out.push(Token {
            ty: TokType::CloseBracket,
            data: vec![0],
            binary: true,
            line: *cursor as u32,
        });
        for i in 0..sentinel as usize {
            if input[*cursor + i] != 0 {
                return tok_err("failed to read nested block sentinel, expected all bytes to be 0");
            }
        }
        *cursor += sentinel as usize;
    }
    if *cursor as u64 != end_offset {
        return tok_err("scope length not reached, something is wrong");
    }
    Ok(true)
}

/// `TokenizeBinary(output_tokens, input, length)`: `input` is the file content plus the NUL the importer appends
/// (`length` counts it).
pub fn tokenize_binary(input: &[u8]) -> R<Vec<Token>> {
    let length = input.len();
    if length < 0x1b {
        return tok_err("file is too short");
    }
    if &input[..18] != b"Kaydara FBX Binary" {
        return tok_err("magic bytes not found");
    }
    let mut cursor = 18usize;
    for _ in 0..5 {
        read_byte(input, &mut cursor, length)?;
    }
    let version = read_word(input, &mut cursor, length)?;
    let is64 = version >= 7500;
    let mut out = Vec::new();
    while cursor < length {
        if !read_scope(&mut out, input, &mut cursor, length, is64)? {
            break;
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------------------
// parser
// ---------------------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct Element {
    pub key: Token,
    pub tokens: Vec<Token>,
    pub compound: Option<Scope>,
}

/// Scope: the elements in `std::multimap<std::string, Element*>` order (by key, equal keys in insertion order).
#[derive(Debug, Default)]
pub struct Scope {
    pub elements: Vec<(String, Element)>,
}

impl Scope {
    /// Scope::operator[]: the first element of that key.
    pub fn get(&self, name: &str) -> Option<&Element> {
        self.elements
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, e)| e)
    }
    /// Scope::GetCollection
    pub fn collection<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> {
        self.elements
            .iter()
            .filter(move |(k, _)| k == name)
            .map(|(_, e)| e)
    }
    pub fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }
}

struct Parser<'a> {
    tokens: &'a [Token],
    cursor: usize,
    current: Option<usize>,
}

impl Parser<'_> {
    fn advance(&mut self) -> Option<usize> {
        if self.cursor == self.tokens.len() {
            self.current = None;
        } else {
            self.current = Some(self.cursor);
            self.cursor += 1;
        }
        self.current
    }
}

fn parse_error<T>(msg: &str) -> R<T> {
    Err(format!("FBX-Parser {msg}"))
}

fn new_scope(p: &mut Parser, top_level: bool) -> R<Scope> {
    if !top_level {
        let t = p.current.ok_or("FBX-Parser unexpected end of file")?;
        if p.tokens[t].ty != TokType::OpenBracket {
            return parse_error("expected open bracket");
        }
    }
    let mut n = match p.advance() {
        Some(n) => n,
        None => return parse_error("unexpected end of file"),
    };
    let mut scope = Scope::default();
    // note: empty scopes are allowed
    while p.tokens[n].ty != TokType::CloseBracket {
        if p.tokens[n].ty != TokType::Key {
            return parse_error("unexpected token, expected TOK_KEY");
        }
        let key = p.tokens[n].string();
        if key.is_empty() {
            return parse_error("unexpected content: empty string.");
        }
        let element = new_element(p, n)?;
        scope.elements.push((key.clone(), element));
        match p.current {
            None => {
                if top_level {
                    scope
                        .elements
                        .sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
                    return Ok(scope);
                }
                return parse_error("unexpected end of file");
            }
            Some(c) => n = c,
        }
    }
    scope
        .elements
        .sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    Ok(scope)
}

fn new_element(p: &mut Parser, key_index: usize) -> R<Element> {
    let mut element = Element {
        key: p.tokens[key_index].clone(),
        tokens: Vec::new(),
        compound: None,
    };
    loop {
        let mut n = match p.advance() {
            Some(n) => n,
            None => return parse_error("unexpected end of file, expected closing bracket"),
        };
        if p.tokens[n].ty == TokType::Data {
            element.tokens.push(p.tokens[n].clone());
            let prev = n;
            n = match p.advance() {
                Some(n) => n,
                None => {
                    return parse_error("unexpected end of file, expected bracket, comma or key")
                }
            };
            let ty = p.tokens[n].ty;
            // some exporters are missing a comma on the next line
            if ty == TokType::Data
                && p.tokens[prev].ty == TokType::Data
                && p.tokens[n].line == p.tokens[prev].line.wrapping_add(1)
            {
                element.tokens.push(p.tokens[n].clone());
                continue;
            }
            if ty != TokType::OpenBracket
                && ty != TokType::CloseBracket
                && ty != TokType::Comma
                && ty != TokType::Key
            {
                return parse_error("unexpected token; expected bracket, comma or key");
            }
        }
        if p.tokens[n].ty == TokType::OpenBracket {
            element.compound = Some(new_scope(p, false)?);
            // current token should be a TOK_CLOSE_BRACKET
            match p.current {
                Some(c) if p.tokens[c].ty == TokType::CloseBracket => {}
                _ => return parse_error("expected closing bracket"),
            }
            p.advance();
            return Ok(element);
        }
        if p.tokens[n].ty == TokType::Key || p.tokens[n].ty == TokType::CloseBracket {
            return Ok(element);
        }
    }
}

/// `Parser::Parser(tokens, is_binary)` and `GetRootScope`.
pub fn parse(tokens: &[Token]) -> R<Scope> {
    let mut p = Parser {
        tokens,
        cursor: 0,
        current: None,
    };
    new_scope(&mut p, true)
}

// ---------------------------------------------------------------------------------------------------------------
// token values
// ---------------------------------------------------------------------------------------------------------------

fn expect_data(t: &Token) -> R<()> {
    if t.ty != TokType::Data {
        return Err("expected TOK_DATA token".to_string());
    }
    Ok(())
}

fn safe_bytes<const N: usize>(data: &[u8]) -> R<[u8; N]> {
    if data.len() < N {
        return Err(
            "Binary file corrupted: attempted to read beyond the end of the file".to_string(),
        );
    }
    let mut a = [0u8; N];
    a.copy_from_slice(&data[..N]);
    Ok(a)
}

pub fn parse_token_as_bool(t: &Token) -> R<bool> {
    expect_data(t)?;
    if t.binary {
        if t.data[0] != b'C' {
            return Err("failed to parse bool, unexpected data type".to_string());
        }
        return Ok(t.data.get(1).copied().unwrap_or(0) != 0);
    }
    if t.data.len() != 1 {
        return Err("failed to parse bool, unexpected token length".to_string());
    }
    let c = t.data[0];
    if c != b'Y' && c != b'T' {
        if c != b'N' && c != b'F' {
            return Err(
                "failed to parse bool, unexpected character, expected Y, N, T or F".to_string(),
            );
        }
        return Ok(false);
    }
    Ok(true)
}

/// strtol10: optional sign and the digits; `Err` when the token has anything else.
fn strtol_all(data: &[u8], what: &str) -> R<i64> {
    let mut i = 0;
    let inv = data.first() == Some(&b'-');
    if inv || data.first() == Some(&b'+') {
        i = 1;
    }
    let mut v: u64 = 0;
    while i < data.len() && data[i].is_ascii_digit() {
        v = v.wrapping_mul(10).wrapping_add(u64::from(data[i] - b'0'));
        i += 1;
    }
    if i != data.len() {
        return Err(what.to_string());
    }
    let v = v as i64;
    Ok(if inv { v.wrapping_neg() } else { v })
}

pub fn parse_token_as_int(t: &Token) -> R<i32> {
    expect_data(t)?;
    if t.binary {
        if t.data[0] != b'I' {
            return Err("failed to parse I(nt), unexpected data type (binary)".to_string());
        }
        return Ok(i32::from_le_bytes(safe_bytes::<4>(&t.data[1..])?));
    }
    Ok(strtol_all(&t.data, "failed to parse ProcessedInt")? as i32)
}

pub fn parse_token_as_int64(t: &Token) -> R<i64> {
    expect_data(t)?;
    if t.binary {
        if t.data[0] != b'L' {
            return Err("failed to parse Int64, unexpected data type".to_string());
        }
        return Ok(i64::from_le_bytes(safe_bytes::<8>(&t.data[1..])?));
    }
    strtol_all(&t.data, "failed to parse Int64")
}

pub fn parse_token_as_id(t: &Token) -> R<u64> {
    expect_data(t)?;
    if t.binary {
        if t.data[0] != b'L' {
            return Err(
                "failed to parse ID, unexpected data type, expected L(ong) (binary)".to_string(),
            );
        }
        return Ok(u64::from_le_bytes(safe_bytes::<8>(&t.data[1..])?));
    }
    let (v, _, _) = strtoul10_64(&t.data, 0, None)?;
    Ok(v)
}

pub fn parse_token_as_float(t: &Token) -> R<f32> {
    expect_data(t)?;
    if t.binary {
        return match t.data[0] {
            b'F' => Ok(f32::from_le_bytes(safe_bytes::<4>(&t.data[1..])?)),
            b'D' => Ok(f64::from_le_bytes(safe_bytes::<8>(&t.data[1..])?) as f32),
            _ => Err("failed to parse F(loat), unexpected data type (binary)".to_string()),
        };
    }
    if t.data.len() > 31 {
        return Ok(0.0);
    }
    let (v, _) = fast_atoreal_move(&t.data, 0)?;
    Ok(v)
}

pub fn parse_token_as_string(t: &Token) -> R<String> {
    expect_data(t)?;
    if t.binary {
        if t.data[0] != b'S' {
            return Err("failed to parse S(tring), unexpected data type (binary)".to_string());
        }
        let len = i32::from_le_bytes(safe_bytes::<4>(&t.data[1..])?);
        let body = t.data.get(5..).unwrap_or(&[]);
        if len < 0 || body.len() != len as usize {
            return Err("FBX string length mismatch".to_string());
        }
        return Ok(body.iter().map(|&b| b as char).collect());
    }
    if t.data.len() < 2 {
        return Err("token is too short to hold a string".to_string());
    }
    if t.data[0] != b'"' || t.data[t.data.len() - 1] != b'"' {
        return Err("expected double quoted string".to_string());
    }
    Ok(t.data[1..t.data.len() - 1]
        .iter()
        .map(|&b| b as char)
        .collect())
}

/// ParseTokenAsDim: `*<count>` of a text array.
pub fn parse_token_as_dim(t: &Token) -> R<usize> {
    expect_data(t)?;
    if t.binary {
        if t.data[0] != b'L' {
            return Err("failed to parse array dimension, unexpected data type".to_string());
        }
        return Ok(u64::from_le_bytes(safe_bytes::<8>(&t.data[1..])?) as usize);
    }
    if t.data[0] != b'*' {
        return Err("expected asterisk before array dimension".to_string());
    }
    if t.data.len() == 1 {
        return Err("expected number after asterisk".to_string());
    }
    let (v, _, _) = strtoul10_64(&t.data, 1, None)?;
    Ok(v as usize)
}

// ---------------------------------------------------------------------------------------------------------------
// element helpers
// ---------------------------------------------------------------------------------------------------------------

pub fn get_required_token(el: &Element, index: usize) -> R<&Token> {
    el.tokens
        .get(index)
        .ok_or_else(|| format!("missing token at index {index}"))
}

pub fn get_required_scope(el: &Element) -> R<&Scope> {
    el.compound
        .as_ref()
        .ok_or_else(|| "expected compound scope".to_string())
}

pub fn get_required_element<'a>(sc: &'a Scope, name: &str) -> R<&'a Element> {
    sc.get(name)
        .ok_or_else(|| format!("did not find required element \"{name}\""))
}

// ---------------------------------------------------------------------------------------------------------------
// data arrays
// ---------------------------------------------------------------------------------------------------------------

/// ReadBinaryDataArrayHead + ReadBinaryDataArray: the element type code, the count and the decoded bytes.
fn read_binary_array(t: &Token) -> R<(u8, u32, Vec<u8>)> {
    let data = &t.data;
    if data.len() < 5 {
        return Err("binary data array is too short, need five (5) bytes for type signature and element count".to_string());
    }
    let ty = data[0];
    let count = u32::from_le_bytes([data[1], data[2], data[3], data[4]]);
    if count == 0 {
        return Ok((ty, 0, Vec::new()));
    }
    let rest = &data[5..];
    if rest.len() < 8 {
        return Err("binary data array is too short".to_string());
    }
    let encmode = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]);
    let compressed = u32::from_le_bytes([rest[4], rest[5], rest[6], rest[7]]) as usize;
    let body = &rest[8..];
    if body.len() < compressed {
        return Err("binary data array is too short".to_string());
    }
    let body = &body[..compressed];
    let stride = match ty {
        b'f' | b'i' => 4usize,
        b'd' | b'l' => 8usize,
        _ => return Err("unexpected array type".to_string()),
    };
    let full = stride * count as usize;
    let buff = if encmode == 0 {
        let mut b = body.to_vec();
        b.resize(full, 0);
        b
    } else {
        inflate_zlib(body, full)
            .map_err(|_| "failure decompressing compressed data section".to_string())?
    };
    Ok((ty, count, buff))
}

fn f32s(buff: &[u8]) -> Vec<f32> {
    buff.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}
fn f64s(buff: &[u8]) -> Vec<f64> {
    buff.chunks_exact(8)
        .map(|c| f64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]))
        .collect()
}

/// The text array of an element: the tokens of its `a` child after checking the `*<dim>` token.
fn text_array(el: &Element) -> R<&[Token]> {
    let tok = &el.tokens;
    parse_token_as_dim(&tok[0])?;
    let scope = get_required_scope(el)?;
    let a = get_required_element(scope, "a")?;
    Ok(&a.tokens)
}

fn float_array(el: &Element) -> R<Vec<f32>> {
    // floats of a double or float array (binary) or text array, widened/narrowed as the callers' `ai_real` do
    let tok = el.tokens.first().ok_or("unexpected empty element")?;
    if tok.binary {
        let (ty, count, buff) = read_binary_array(tok)?;
        if count == 0 {
            return Ok(Vec::new());
        }
        return match ty {
            b'd' => Ok(f64s(&buff).into_iter().map(|d| d as f32).collect()),
            b'f' => Ok(f32s(&buff)),
            _ => Err("expected float or double array (binary)".to_string()),
        };
    }
    text_array(el)?.iter().map(parse_token_as_float).collect()
}

/// ParseVectorDataArray(std::vector<aiVector3D>&, const Element&)
pub fn parse_vec3_array(el: &Element) -> R<Vec<Vec3>> {
    let tok = el.tokens.first().ok_or("unexpected empty element")?;
    if tok.binary {
        let (ty, count, _) = read_binary_array(tok)?;
        if count == 0 {
            return Ok(Vec::new());
        }
        if ty != b'd' && ty != b'f' {
            return Err("expected float or double array (binary)".to_string());
        }
        if count % 3 != 0 {
            return Err("number of floats is not a multiple of three (3) (binary)".to_string());
        }
    } else {
        let a = text_array(el)?;
        if a.len() % 3 != 0 {
            return Err("number of floats is not a multiple of three (3)".to_string());
        }
    }
    let f = float_array(el)?;
    Ok(f.chunks_exact(3)
        .map(|c| Vec3::new(c[0], c[1], c[2]))
        .collect())
}

/// ParseVectorDataArray(std::vector<aiColor4D>&, const Element&)
pub fn parse_color_array(el: &Element) -> R<Vec<[f32; 4]>> {
    let tok = el.tokens.first().ok_or("unexpected empty element")?;
    if tok.binary {
        let (ty, count, _) = read_binary_array(tok)?;
        if count == 0 {
            return Ok(Vec::new());
        }
        if ty != b'd' && ty != b'f' {
            return Err("expected float or double array (binary)".to_string());
        }
        if count % 4 != 0 {
            return Err("number of floats is not a multiple of four (4) (binary)".to_string());
        }
    } else {
        let a = text_array(el)?;
        if a.len() % 4 != 0 {
            return Err("number of floats is not a multiple of four (4)".to_string());
        }
    }
    let f = float_array(el)?;
    Ok(f.chunks_exact(4)
        .map(|c| [c[0], c[1], c[2], c[3]])
        .collect())
}

/// ParseVectorDataArray(std::vector<aiVector2D>&, const Element&)
pub fn parse_vec2_array(el: &Element) -> R<Vec<[f32; 2]>> {
    let tok = el.tokens.first().ok_or("unexpected empty element")?;
    if tok.binary {
        let (ty, count, _) = read_binary_array(tok)?;
        if count == 0 {
            return Ok(Vec::new());
        }
        if ty != b'd' && ty != b'f' {
            return Err("expected float or double array (binary)".to_string());
        }
        if count % 2 != 0 {
            return Err("number of floats is not a multiple of two (2) (binary)".to_string());
        }
    } else {
        let a = text_array(el)?;
        if a.len() % 2 != 0 {
            return Err("number of floats is not a multiple of two (2)".to_string());
        }
    }
    let f = float_array(el)?;
    Ok(f.chunks_exact(2).map(|c| [c[0], c[1]]).collect())
}

/// ParseVectorDataArray(std::vector<float>&, const Element&)
pub fn parse_float_array(el: &Element) -> R<Vec<f32>> {
    float_array(el)
}

/// ParseVectorDataArray(std::vector<int>&, const Element&)
pub fn parse_int_array(el: &Element) -> R<Vec<i32>> {
    let tok = el.tokens.first().ok_or("unexpected empty element")?;
    if tok.binary {
        let (ty, count, buff) = read_binary_array(tok)?;
        if count == 0 {
            return Ok(Vec::new());
        }
        if ty != b'i' {
            return Err("expected int array (binary)".to_string());
        }
        return Ok(buff
            .chunks_exact(4)
            .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect());
    }
    text_array(el)?.iter().map(parse_token_as_int).collect()
}

/// ParseVectorDataArray(std::vector<int64_t>&, const Element&)
pub fn parse_int64_array(el: &Element) -> R<Vec<i64>> {
    let tok = el.tokens.first().ok_or("unexpected empty element")?;
    if tok.binary {
        let (ty, count, buff) = read_binary_array(tok)?;
        if count == 0 {
            return Ok(Vec::new());
        }
        if ty != b'l' {
            return Err("expected long array (binary)".to_string());
        }
        return Ok(buff
            .chunks_exact(8)
            .map(|c| i64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]))
            .collect());
    }
    text_array(el)?.iter().map(parse_token_as_int64).collect()
}
