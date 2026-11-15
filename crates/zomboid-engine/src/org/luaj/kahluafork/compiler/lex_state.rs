//! org.luaj.kahluafork.compiler.LexState: Kahlua's Lua lexer and parser (luaj's port of Lua 5.1's llex.c and
//! lparser.c), which drives `FuncState` to emit the bytecode of a chunk.
use super::exp_desc::ExpDesc;
use super::func_state::{
    assert_, getarg_a, set_opcode, setarg_b, setarg_c, ConsControl, FuncState, LUAI_MAXVARS,
    OP_CALL, OP_CLOSURE, OP_FORLOOP, OP_FORPREP, OP_GETUPVAL, OP_LOADK, OP_MOVE, OP_NEWTABLE,
    OP_SETTABLE, OP_TAILCALL, OP_TFORLOOP, OP_VARARG,
};
use crate::java::utf8_decode_jdk;
use crate::se::krka::kahlua::vm::prototype::Prototype;

/// A compile error: the `KahluaException` message.
pub type R<T> = Result<T, String>;

const MAX_INT: i32 = 2147483645;
const TK_AND: i32 = 257;
const TK_BREAK: i32 = 258;
const TK_DO: i32 = 259;
const TK_ELSE: i32 = 260;
const TK_ELSEIF: i32 = 261;
const TK_END: i32 = 262;
const TK_FALSE: i32 = 263;
const TK_FOR: i32 = 264;
const TK_FUNCTION: i32 = 265;
const TK_IF: i32 = 266;
const TK_IN: i32 = 267;
const TK_LOCAL: i32 = 268;
const TK_NIL: i32 = 269;
const TK_NOT: i32 = 270;
const TK_OR: i32 = 271;
const TK_REPEAT: i32 = 272;
const TK_RETURN: i32 = 273;
const TK_THEN: i32 = 274;
const TK_TRUE: i32 = 275;
const TK_UNTIL: i32 = 276;
const TK_WHILE: i32 = 277;
const TK_CONCAT: i32 = 278;
const TK_DOTS: i32 = 279;
const TK_EQ: i32 = 280;
const TK_GE: i32 = 281;
const TK_LE: i32 = 282;
const TK_NE: i32 = 283;
const TK_NUMBER: i32 = 284;
const TK_NAME: i32 = 285;
const TK_STRING: i32 = 286;
const TK_EOS: i32 = 287;

/// org.luaj.kahluafork.compiler.LexState.luaX_tokens
const LUAX_TOKENS: [&str; 31] = [
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "if", "in", "local",
    "nil", "not", "or", "repeat", "return", "then", "true", "until", "while", "..", "...", "==",
    ">=", "<=", "~=", "<number>", "<name>", "<string>", "<eof>",
];
const PRIORITY_LEFT: [i32; 15] = [6, 6, 7, 7, 7, 10, 5, 3, 3, 3, 3, 3, 3, 2, 1];
const PRIORITY_RIGHT: [i32; 15] = [6, 6, 7, 7, 7, 9, 4, 3, 3, 3, 3, 3, 3, 2, 1];
const UNARY_PRIORITY: i32 = 8;

fn lua_qs(s: &str) -> String {
    format!("'{s}'")
}

/// org.luaj.kahluafork.compiler.Token: `r` and `ts` keep their last values across tokens that do not set them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Token {
    pub token: i32,
    pub r: f64,
    pub ts: Option<String>,
}

#[derive(Clone, Copy)]
enum Which {
    T,
    Lookahead,
}

/// The lexer half of `LexState` (what `FuncState` reads: the last line for each instruction, and the token for
/// error messages).
#[derive(Clone, Debug)]
pub struct Lexer {
    z: Vec<u16>,
    pos: usize,
    current: i32,
    linenumber: i32,
    pub lastline: i32,
    t: Token,
    lookahead: Token,
    buff: Vec<u8>,
    source: String,
    n_ccalls: i32,
    /// `FuncState.currentFile`
    pub current_file: Option<String>,
    /// `FuncState.currentfullFile`
    pub currentfull_file: Option<String>,
}

fn isalnum(c: i32) -> bool {
    (48..=57).contains(&c) || (97..=122).contains(&c) || (65..=90).contains(&c) || c == 95
}

fn isalpha(c: i32) -> bool {
    (97..=122).contains(&c) || (65..=90).contains(&c)
}

fn isdigit(c: i32) -> bool {
    (48..=57).contains(&c)
}

fn isspace(c: i32) -> bool {
    c <= 32
}

/// UTF-16 units to a Rust string, a lone surrogate as '?' (what the original's UTF-8 output writes for one).
fn utf16_string(units: &[u16]) -> String {
    char::decode_utf16(units.iter().copied())
        .map(|r| r.unwrap_or('?'))
        .collect()
}

impl Lexer {
    fn tok(&mut self, w: Which) -> &mut Token {
        match w {
            Which::T => &mut self.t,
            Which::Lookahead => &mut self.lookahead,
        }
    }

    /// org.luaj.kahluafork.compiler.LexState.nextChar
    fn next_char(&mut self) {
        self.current = match self.z.get(self.pos) {
            Some(&c) => {
                self.pos += 1;
                i32::from(c)
            }
            None => -1,
        };
    }

    fn curr_is_newline(&self) -> bool {
        self.current == 10 || self.current == 13
    }

    fn save_and_next(&mut self) {
        self.save(self.current);
        self.next_char();
    }

    /// org.luaj.kahluafork.compiler.LexState.save: the character is stored as `(byte)c`, its low 8 bits.
    fn save(&mut self, c: i32) {
        self.buff.push(c as u8);
    }

    /// org.luaj.kahluafork.compiler.LexState.token2str. A single character at or above 257 (U+FEFF, a byte order
    /// mark, is one) indexes past `luaX_tokens`: `Err` is the original's ArrayIndexOutOfBoundsException message.
    fn token2str(token: i32) -> Result<String, String> {
        if token < 257 {
            if token < 32 {
                Ok(format!("char({token})"))
            } else {
                Ok(utf16_string(&[token as u16]))
            }
        } else {
            let i = token - 257;
            LUAX_TOKENS.get(i as usize).map(|s| s.to_string()).ok_or_else(|| {
                format!(
                    "java.lang.ArrayIndexOutOfBoundsException: Index {i} out of bounds for length {}",
                    LUAX_TOKENS.len()
                )
            })
        }
    }

    /// org.luaj.kahluafork.compiler.LexState.txtToken
    fn txt_token(&self, token: i32) -> Result<String, String> {
        match token {
            TK_NUMBER | TK_NAME | TK_STRING => Ok(utf16_string(&utf8_decode_jdk(&self.buff))),
            _ => Self::token2str(token),
        }
    }

    /// org.luaj.kahluafork.compiler.LexState.lexerror: the message of the `KahluaException` it throws (or of the
    /// exception that building it throws).
    pub fn lexerror(&self, msg: &str, token: i32) -> String {
        if token != 0 {
            match self.txt_token(token) {
                Ok(t) => format!("{}:{}: {msg} near `{t}`", self.source, self.linenumber),
                Err(e) => e,
            }
        } else {
            format!("{}:{}: {msg}", self.source, self.linenumber)
        }
    }

    /// org.luaj.kahluafork.compiler.LexState.syntaxerror
    pub fn syntaxerror(&self, msg: &str) -> String {
        self.lexerror(msg, self.t.token)
    }

    /// org.luaj.kahluafork.compiler.LexState.newstring: the bytes decoded as UTF-8 by the JDK's rules.
    fn newstring(&self, offset: usize, len: usize) -> String {
        utf16_string(&utf8_decode_jdk(&self.buff[offset..offset + len]))
    }

    /// org.luaj.kahluafork.compiler.LexState.inclinenumber
    fn inclinenumber(&mut self) -> R<()> {
        let old = self.current;
        assert_(self.curr_is_newline())?;
        self.next_char();
        if self.curr_is_newline() && self.current != old {
            self.next_char();
        }
        self.linenumber += 1;
        if self.linenumber >= MAX_INT {
            return Err(self.syntaxerror("chunk has too many lines"));
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.skipShebang
    fn skip_shebang(&mut self) {
        if self.current == 35 {
            while !self.curr_is_newline() && self.current != -1 {
                self.next_char();
            }
        }
    }

    /// org.luaj.kahluafork.compiler.LexState.check_next
    fn check_next(&mut self, set: &str) -> bool {
        if self.current < 0 || !set.chars().any(|c| c as i32 == self.current) {
            return false;
        }
        self.save_and_next();
        true
    }

    /// org.luaj.kahluafork.compiler.LexState.str2d: "0x" is `Long.parseLong` in base 16, anything else
    /// `Double.parseDouble` (so "1.5f" and "2d" are numbers).
    fn str2d(&self, s: &str) -> R<f64> {
        let d = match s.strip_prefix("0x") {
            Some(hex) => crate::java::parse_long(hex, 16).map(|v| v as f64),
            None => crate::java::parse_double(s),
        };
        d.ok_or_else(|| self.lexerror("malformed number", TK_NUMBER))
    }

    /// org.luaj.kahluafork.compiler.LexState.read_numeral
    fn read_numeral(&mut self, w: Which) -> R<()> {
        assert_(isdigit(self.current))?;
        loop {
            self.save_and_next();
            if !(isdigit(self.current) || self.current == 46) {
                break;
            }
        }
        if self.check_next("Ee") {
            self.check_next("+-");
        }
        while isalnum(self.current) || self.current == 95 {
            self.save_and_next();
        }
        let s = utf16_string(&utf8_decode_jdk(&self.buff));
        let r = self.str2d(&s)?;
        self.tok(w).r = r;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.skip_sep
    fn skip_sep(&mut self) -> R<i32> {
        let mut count = 0;
        let s = self.current;
        assert_(s == 91 || s == 93)?;
        self.save_and_next();
        while self.current == 61 {
            self.save_and_next();
            count += 1;
        }
        Ok(if self.current == s { count } else { -count - 1 })
    }

    /// org.luaj.kahluafork.compiler.LexState.read_long_string: `w` is the token to fill, `None` for a comment.
    fn read_long_string(&mut self, w: Option<Which>, sep: i32) -> R<()> {
        self.save_and_next();
        if self.curr_is_newline() {
            self.inclinenumber()?;
        }
        loop {
            match self.current {
                -1 => {
                    let msg = if w.is_some() {
                        "unfinished long string"
                    } else {
                        "unfinished long comment"
                    };
                    return Err(self.lexerror(msg, TK_EOS));
                }
                10 | 13 => {
                    self.save(10);
                    self.inclinenumber()?;
                    if w.is_none() {
                        self.buff.clear();
                    }
                }
                91 => {
                    if self.skip_sep()? == sep {
                        self.save_and_next();
                    }
                }
                93 => {
                    if self.skip_sep()? == sep {
                        self.save_and_next();
                        break;
                    }
                }
                _ => {
                    if w.is_some() {
                        self.save_and_next();
                    } else {
                        self.next_char();
                    }
                }
            }
        }
        if let Some(w) = w {
            let skip = (2 + sep) as usize;
            let ts = self.newstring(skip, self.buff.len() - 2 * skip);
            self.tok(w).ts = Some(ts);
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.read_string
    fn read_string(&mut self, del: i32, w: Which) -> R<()> {
        self.save_and_next();
        loop {
            if self.current == del {
                self.save_and_next();
                let ts = self.newstring(1, self.buff.len() - 2);
                self.tok(w).ts = Some(ts);
                return Ok(());
            }
            match self.current {
                -1 => return Err(self.lexerror("unfinished string", TK_EOS)),
                10 | 13 => return Err(self.lexerror("unfinished string", TK_STRING)),
                92 => {
                    self.next_char();
                    let c = match self.current {
                        -1 => continue,
                        10 | 13 => {
                            self.save(10);
                            self.inclinenumber()?;
                            continue;
                        }
                        97 => 7,
                        98 => 8,
                        102 => 12,
                        110 => 10,
                        114 => 13,
                        116 => 9,
                        118 => 11,
                        _ => {
                            if !isdigit(self.current) {
                                self.save_and_next();
                                continue;
                            }
                            let mut i = 0;
                            let mut c = 0;
                            loop {
                                c = 10 * c + self.current - 48;
                                self.next_char();
                                i += 1;
                                if !(i < 3 && isdigit(self.current)) {
                                    break;
                                }
                            }
                            if c > 255 {
                                return Err(self.lexerror("escape sequence too large", TK_STRING));
                            }
                            self.save(c);
                            continue;
                        }
                    };
                    self.save(c);
                    self.next_char();
                }
                _ => self.save_and_next(),
            }
        }
    }

    /// org.luaj.kahluafork.compiler.LexState.llex
    fn llex(&mut self, w: Which) -> R<i32> {
        self.buff.clear();
        loop {
            match self.current {
                -1 => return Ok(TK_EOS),
                10 | 13 => self.inclinenumber()?,
                34 | 39 => {
                    self.read_string(self.current, w)?;
                    return Ok(TK_STRING);
                }
                45 => {
                    self.next_char();
                    if self.current != 45 {
                        return Ok(45);
                    }
                    self.next_char();
                    let mut long_comment = false;
                    if self.current == 91 {
                        let sep = self.skip_sep()?;
                        self.buff.clear();
                        if sep >= 0 {
                            self.read_long_string(None, sep)?;
                            self.buff.clear();
                            long_comment = true;
                        }
                    }
                    if !long_comment {
                        while !self.curr_is_newline() && self.current != -1 {
                            self.next_char();
                        }
                    }
                }
                46 => {
                    self.save_and_next();
                    if self.check_next(".") {
                        if self.check_next(".") {
                            return Ok(TK_DOTS);
                        }
                        return Ok(TK_CONCAT);
                    }
                    if !isdigit(self.current) {
                        return Ok(46);
                    }
                    self.read_numeral(w)?;
                    return Ok(TK_NUMBER);
                }
                60 => {
                    self.next_char();
                    if self.current != 61 {
                        return Ok(60);
                    }
                    self.next_char();
                    return Ok(TK_LE);
                }
                62 => {
                    self.next_char();
                    if self.current != 61 {
                        return Ok(62);
                    }
                    self.next_char();
                    return Ok(TK_GE);
                }
                91 => {
                    let sep = self.skip_sep()?;
                    if sep >= 0 {
                        self.read_long_string(Some(w), sep)?;
                        return Ok(TK_STRING);
                    }
                    if sep == -1 {
                        return Ok(91);
                    }
                    return Err(self.lexerror("invalid long string delimiter", TK_STRING));
                }
                61 => {
                    self.next_char();
                    if self.current != 61 {
                        return Ok(61);
                    }
                    self.next_char();
                    return Ok(TK_EQ);
                }
                126 => {
                    self.next_char();
                    if self.current != 61 {
                        return Ok(126);
                    }
                    self.next_char();
                    return Ok(TK_NE);
                }
                _ => {
                    if !isspace(self.current) {
                        if isdigit(self.current) {
                            self.read_numeral(w)?;
                            return Ok(TK_NUMBER);
                        }
                        if !isalpha(self.current) && self.current != 95 {
                            let c = self.current;
                            self.next_char();
                            return Ok(c);
                        }
                        loop {
                            self.save_and_next();
                            if !(isalnum(self.current) || self.current == 95) {
                                break;
                            }
                        }
                        let ts = self.newstring(0, self.buff.len());
                        if let Some(i) = LUAX_TOKENS[..21].iter().position(|&r| r == ts) {
                            return Ok(257 + i as i32);
                        }
                        self.tok(w).ts = Some(ts);
                        return Ok(TK_NAME);
                    }
                    assert_(!self.curr_is_newline())?;
                    self.next_char();
                }
            }
        }
    }

    /// org.luaj.kahluafork.compiler.LexState.next
    fn next(&mut self) -> R<()> {
        self.lastline = self.linenumber;
        if self.lookahead.token != TK_EOS {
            self.t = self.lookahead.clone();
            self.lookahead.token = TK_EOS;
        } else {
            self.t.token = self.llex(Which::T)?;
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.lookahead
    fn lookahead(&mut self) -> R<()> {
        assert_(self.lookahead.token == TK_EOS)?;
        self.lookahead.token = self.llex(Which::Lookahead)?;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.error_expected
    fn error_expected(&self, token: i32) -> String {
        match Self::token2str(token) {
            Ok(t) => self.syntaxerror(&format!("{} expected", lua_qs(&t))),
            Err(e) => e,
        }
    }

    /// org.luaj.kahluafork.compiler.LexState.testnext
    fn testnext(&mut self, c: i32) -> R<bool> {
        if self.t.token == c {
            self.next()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// org.luaj.kahluafork.compiler.LexState.check
    fn check(&self, c: i32) -> R<()> {
        if self.t.token != c {
            return Err(self.error_expected(c));
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.checknext
    fn checknext(&mut self, c: i32) -> R<()> {
        self.check(c)?;
        self.next()
    }

    /// org.luaj.kahluafork.compiler.LexState.check_condition
    fn check_condition(&self, c: bool, msg: &str) -> R<()> {
        if !c {
            return Err(self.syntaxerror(msg));
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.check_match
    fn check_match(&mut self, what: i32, who: i32, where_: i32) -> R<()> {
        if !self.testnext(what)? {
            if where_ == self.linenumber {
                return Err(self.error_expected(what));
            }
            let what = Self::token2str(what)?;
            let who = Self::token2str(who)?;
            return Err(self.syntaxerror(&format!(
                "{} expected (to close {} at line {where_})",
                lua_qs(&what),
                lua_qs(&who)
            )));
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.str_checkname
    fn str_checkname(&mut self) -> R<Option<String>> {
        self.check(TK_NAME)?;
        let ts = self.t.ts.clone();
        self.next()?;
        Ok(ts)
    }
}

/// org.luaj.kahluafork.compiler.LexState: the lexer and the stack of functions being compiled (`fs` and its `prev`
/// chain, innermost last).
pub struct LexState {
    lx: Lexer,
    fs: Vec<FuncState>,
}

/// org.luaj.kahluafork.compiler.LexState.luaO_int2fb
fn luao_int2fb(mut x: i32) -> i32 {
    let mut e = 0;
    while x >= 16 {
        x = (x + 1) >> 1;
        e += 1;
    }
    if x < 8 {
        x
    } else {
        ((e + 1) << 3) | (x - 8)
    }
}

fn getunopr(op: i32) -> i32 {
    match op {
        35 => 2,
        45 => 0,
        TK_NOT => 1,
        _ => 3,
    }
}

fn getbinopr(op: i32) -> i32 {
    match op {
        37 => 4,
        42 => 2,
        43 => 0,
        45 => 1,
        47 => 3,
        60 => 9,
        62 => 11,
        94 => 5,
        TK_AND => 13,
        TK_OR => 14,
        TK_CONCAT => 6,
        TK_EQ => 8,
        TK_GE => 12,
        TK_LE => 10,
        TK_NE => 7,
        _ => 15,
    }
}

fn block_follow(token: i32) -> bool {
    matches!(token, TK_ELSE | TK_ELSEIF | TK_END | TK_UNTIL | TK_EOS)
}

fn hasmultret(k: i32) -> bool {
    k == 13 || k == 14
}

/// org.luaj.kahluafork.compiler.LexState.compile, as `LuaCompiler.loadis(Reader, name, source, env)` calls it: `chars`
/// are the UTF-16 units the reader yields, `current_file` and `currentfull_file` the `FuncState` statics that
/// `LuaManager.RunLuaInternal` sets. `Err` is the `KahluaException` message.
pub fn compile(
    chars: Vec<u16>,
    name: Option<&str>,
    source: Option<&str>,
    current_file: Option<&str>,
    currentfull_file: Option<&str>,
) -> R<Prototype> {
    let (name, source) = match name {
        Some(n) => (n.to_string(), n.to_string()),
        None => {
            let s = source.ok_or("java.lang.NullPointerException: no source")?;
            let units: Vec<u16> = s.encode_utf16().collect();
            let trimmed = if units.len() > 80 {
                format!("{}...", utf16_string(&units[..77]))
            } else {
                s.to_string()
            };
            ("stdin".to_string(), format!("[string \"{trimmed}\"]"))
        }
    };
    let mut lx = Lexer {
        z: chars,
        pos: 0,
        current: -1,
        linenumber: 1,
        lastline: 1,
        t: Token::default(),
        lookahead: Token {
            token: TK_EOS,
            ..Token::default()
        },
        buff: Vec::new(),
        source,
        n_ccalls: 0,
        current_file: current_file.map(str::to_string),
        currentfull_file: currentfull_file.map(str::to_string),
    };
    lx.next_char();
    lx.skip_shebang();
    let mut ls = LexState { lx, fs: Vec::new() };
    let mut main = FuncState::new(None);
    main.is_vararg = 2;
    main.f.name = Some(name);
    ls.fs.push(main);
    ls.lx.next()?;
    ls.chunk()?;
    ls.lx.check(TK_EOS)?;
    let main = ls.close_func()?;
    assert_(ls.fs.is_empty())?;
    assert_(main.f.num_upvalues == 0)?;
    Ok(main.f)
}

impl LexState {
    /// The innermost function and the lexer, borrowed apart.
    fn cur(&mut self) -> (&Lexer, &mut FuncState) {
        let n = self.fs.len() - 1;
        (&self.lx, &mut self.fs[n])
    }

    fn cur_ref(&self) -> &FuncState {
        &self.fs[self.fs.len() - 1]
    }

    /// org.luaj.kahluafork.compiler.LexState.codestring
    fn codestring(&mut self, e: &mut ExpDesc, s: &str) {
        let (_, fs) = self.cur();
        let k = fs.string_k(s);
        e.init(4, k);
    }

    /// org.luaj.kahluafork.compiler.LexState.checkname
    fn checkname(&mut self, e: &mut ExpDesc) -> R<()> {
        let s = self.lx.str_checkname()?;
        self.codestring(e, s.as_deref().unwrap_or("null"));
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.registerlocalvar
    fn registerlocalvar(&mut self, varname: Option<String>) -> i32 {
        let (_, fs) = self.cur();
        fs.locvars
            .push(varname.unwrap_or_else(|| "null".to_string()));
        fs.locvars.len() as i32 - 1
    }

    /// org.luaj.kahluafork.compiler.LexState.new_localvar (`Core.debug`'s variable lines are not ported)
    fn new_localvar(&mut self, name: Option<String>, n: i32) -> R<()> {
        let (lx, fs) = self.cur();
        fs.checklimit(lx, fs.nactvar + n + 1, LUAI_MAXVARS, "local variables")?;
        let idx = self.registerlocalvar(name);
        let (_, fs) = self.cur();
        let at = (fs.nactvar + n) as usize;
        fs.actvar[at] = idx as i16;
        Ok(())
    }

    fn new_localvarliteral(&mut self, v: &str, n: i32) -> R<()> {
        self.new_localvar(Some(v.to_string()), n)
    }

    /// org.luaj.kahluafork.compiler.LexState.adjustlocalvars
    fn adjustlocalvars(&mut self, nvars: i32) {
        let (_, fs) = self.cur();
        fs.nactvar += nvars;
    }

    /// org.luaj.kahluafork.compiler.FuncState.singlevaraux over the function stack (`level` is the function).
    fn singlevaraux(&mut self, level: usize, n: &str, var: &mut ExpDesc, base: i32) -> R<i32> {
        let v = self.fs[level].searchvar(n);
        if v >= 0 {
            var.init(6, v);
            if base == 0 {
                self.fs[level].markupval(v);
            }
            return Ok(6);
        }
        if level == 0 {
            var.init(8, 255);
            return Ok(8);
        }
        if self.singlevaraux(level - 1, n, var, 0)? == 8 {
            return Ok(8);
        }
        let lx = &self.lx;
        var.info = self.fs[level].indexupvalue(lx, n, var)?;
        var.k = 7;
        Ok(7)
    }

    /// org.luaj.kahluafork.compiler.LexState.singlevar
    fn singlevar(&mut self, var: &mut ExpDesc) -> R<()> {
        let varname = self
            .lx
            .str_checkname()?
            .unwrap_or_else(|| "null".to_string());
        let level = self.fs.len() - 1;
        if self.singlevaraux(level, &varname, var, 1)? == 8 {
            let (_, fs) = self.cur();
            var.info = fs.string_k(&varname);
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.adjust_assign
    fn adjust_assign(&mut self, nvars: i32, nexps: i32, e: &mut ExpDesc) -> R<()> {
        let (lx, fs) = self.cur();
        let mut extra = nvars - nexps;
        if hasmultret(e.k) {
            extra += 1;
            if extra < 0 {
                extra = 0;
            }
            fs.setreturns(lx, e, extra)?;
            if extra > 1 {
                fs.reserveregs(lx, extra - 1)?;
            }
        } else {
            if e.k != 0 {
                fs.exp2nextreg(lx, e)?;
            }
            if extra > 0 {
                let reg = fs.freereg;
                fs.reserveregs(lx, extra)?;
                fs.nil(lx, reg, extra)?;
            }
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.enterlevel
    fn enterlevel(&mut self) -> R<()> {
        self.lx.n_ccalls += 1;
        if self.lx.n_ccalls > 200 {
            return Err(self.lx.lexerror("chunk has too many syntax levels", 0));
        }
        Ok(())
    }

    fn leavelevel(&mut self) {
        self.lx.n_ccalls -= 1;
    }

    /// org.luaj.kahluafork.compiler.LexState.pushclosure
    fn pushclosure(&mut self, func: FuncState, v: &mut ExpDesc) -> R<()> {
        let (lx, fs) = self.cur();
        let nup = func.f.num_upvalues;
        fs.f.prototypes.push(func.f);
        fs.np += 1;
        let np = fs.np;
        v.init(11, fs.code_abx(lx, OP_CLOSURE, 0, np - 1)?);
        for i in 0..nup as usize {
            let o = if func.upvalues_k[i] == 6 {
                OP_MOVE
            } else {
                OP_GETUPVAL
            };
            fs.code_abc(lx, o, 0, func.upvalues_info[i], 0)?;
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.close_func: pops the function and returns it.
    fn close_func(&mut self) -> R<FuncState> {
        let (lx, fs) = self.cur();
        fs.f.is_vararg = fs.is_vararg != 0;
        fs.nactvar = 0;
        fs.ret(lx, 0, 0)?;
        fs.finish();
        assert_(fs.bl.is_empty())?;
        self.fs
            .pop()
            .ok_or_else(|| "compiler assert failed".to_string())
    }

    /// org.luaj.kahluafork.compiler.LexState.field
    fn field(&mut self, v: &mut ExpDesc) -> R<()> {
        let mut key = ExpDesc::default();
        {
            let (lx, fs) = self.cur();
            fs.exp2anyreg(lx, v)?;
        }
        self.lx.next()?;
        self.checkname(&mut key)?;
        let (lx, fs) = self.cur();
        fs.indexed(lx, v, &mut key)
    }

    /// org.luaj.kahluafork.compiler.LexState.yindex
    fn yindex(&mut self, v: &mut ExpDesc) -> R<()> {
        self.lx.next()?;
        self.expr(v)?;
        {
            let (lx, fs) = self.cur();
            fs.exp2val(lx, v)?;
        }
        self.lx.checknext(93)
    }

    /// org.luaj.kahluafork.compiler.LexState.recfield
    fn recfield(&mut self, cc: &mut ConsControl, t_info: i32) -> R<()> {
        let reg = self.cur_ref().freereg;
        let mut key = ExpDesc::default();
        let mut val = ExpDesc::default();
        if self.lx.t.token == TK_NAME {
            let (lx, fs) = self.cur();
            fs.checklimit(lx, cc.nh, MAX_INT, "items in a constructor")?;
            self.checkname(&mut key)?;
        } else {
            self.yindex(&mut key)?;
        }
        cc.nh += 1;
        self.lx.checknext(61)?;
        let rkkey = {
            let (lx, fs) = self.cur();
            fs.exp2rk(lx, &mut key)?
        };
        self.expr(&mut val)?;
        let (lx, fs) = self.cur();
        let rkval = fs.exp2rk(lx, &mut val)?;
        fs.code_abc(lx, OP_SETTABLE, t_info, rkkey, rkval)?;
        fs.freereg = reg;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.listfield
    fn listfield(&mut self, cc: &mut ConsControl) -> R<()> {
        self.expr(&mut cc.v)?;
        let (lx, fs) = self.cur();
        fs.checklimit(lx, cc.na, MAX_INT, "items in a constructor")?;
        cc.na += 1;
        cc.tostore += 1;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.constructor
    fn constructor(&mut self, t: &mut ExpDesc) -> R<()> {
        let line = self.lx.linenumber;
        let pc = {
            let (lx, fs) = self.cur();
            fs.code_abc(lx, OP_NEWTABLE, 0, 0, 0)?
        };
        let mut cc = ConsControl::default();
        t.init(11, pc);
        cc.v.init(0, 0);
        {
            let (lx, fs) = self.cur();
            fs.exp2nextreg(lx, t)?;
        }
        self.lx.checknext(123)?;
        loop {
            assert_(cc.v.k == 0 || cc.tostore > 0)?;
            if self.lx.t.token == 125 {
                break;
            }
            {
                let (lx, fs) = self.cur();
                fs.closelistfield(lx, &mut cc, t.info)?;
            }
            match self.lx.t.token {
                91 => self.recfield(&mut cc, t.info)?,
                TK_NAME => {
                    self.lx.lookahead()?;
                    if self.lx.lookahead.token != 61 {
                        self.listfield(&mut cc)?;
                    } else {
                        self.recfield(&mut cc, t.info)?;
                    }
                }
                _ => self.listfield(&mut cc)?,
            }
            if !(self.lx.testnext(44)? || self.lx.testnext(59)?) {
                break;
            }
        }
        self.lx.check_match(125, 123, line)?;
        let (lx, fs) = self.cur();
        fs.lastlistfield(lx, &mut cc, t.info)?;
        let i = setarg_b(fs.code_at(pc), luao_int2fb(cc.na));
        fs.set_code_at(pc, setarg_c(i, luao_int2fb(cc.nh)));
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.parlist
    fn parlist(&mut self) -> R<()> {
        let mut nparams = 0;
        self.cur().1.is_vararg = 0;
        if self.lx.t.token != 41 {
            loop {
                match self.lx.t.token {
                    TK_DOTS => {
                        self.lx.next()?;
                        self.cur().1.is_vararg |= 2;
                    }
                    TK_NAME => {
                        let name = self.lx.str_checkname()?;
                        self.new_localvar(name, nparams)?;
                        nparams += 1;
                    }
                    _ => return Err(self.lx.syntaxerror("<name> or '...' expected")),
                }
                if !(self.cur_ref().is_vararg == 0 && self.lx.testnext(44)?) {
                    break;
                }
            }
        }
        self.adjustlocalvars(nparams);
        let (lx, fs) = self.cur();
        fs.f.num_params = fs.nactvar - (fs.is_vararg & 1);
        let n = fs.nactvar;
        fs.reserveregs(lx, n)
    }

    /// org.luaj.kahluafork.compiler.LexState.body: the new function is named after the current token's string
    /// (`t.ts`, whatever name or string was lexed into it last).
    fn body(&mut self, e: &mut ExpDesc, needself: bool, line: i32) -> R<()> {
        let mut new_fs = FuncState::new(self.lx.t.ts.clone());
        new_fs.linedefined = line;
        self.fs.push(new_fs);
        self.lx.checknext(40)?;
        if needself {
            self.new_localvarliteral("self", 0)?;
            self.adjustlocalvars(1);
        }
        self.parlist()?;
        self.lx.checknext(41)?;
        self.chunk()?;
        self.cur().1.lastlinedefined = self.lx.linenumber;
        self.lx.check_match(TK_END, TK_FUNCTION, line)?;
        let func = self.close_func()?;
        self.pushclosure(func, e)
    }

    /// org.luaj.kahluafork.compiler.LexState.explist1
    fn explist1(&mut self, v: &mut ExpDesc) -> R<i32> {
        let mut n = 1;
        self.expr(v)?;
        while self.lx.testnext(44)? {
            {
                let (lx, fs) = self.cur();
                fs.exp2nextreg(lx, v)?;
            }
            self.expr(v)?;
            n += 1;
        }
        Ok(n)
    }

    /// org.luaj.kahluafork.compiler.LexState.funcargs
    fn funcargs(&mut self, f: &mut ExpDesc) -> R<()> {
        let mut args = ExpDesc::default();
        let line = self.lx.linenumber;
        match self.lx.t.token {
            40 => {
                if line != self.lx.lastline {
                    return Err(self
                        .lx
                        .syntaxerror("ambiguous syntax (function call x new statement)"));
                }
                self.lx.next()?;
                if self.lx.t.token == 41 {
                    args.k = 0;
                } else {
                    self.explist1(&mut args)?;
                    let (lx, fs) = self.cur();
                    fs.setmultret(lx, &mut args)?;
                }
                self.lx.check_match(41, 40, line)?;
            }
            123 => self.constructor(&mut args)?,
            TK_STRING => {
                let ts = self.lx.t.ts.clone().unwrap_or_else(|| "null".to_string());
                self.codestring(&mut args, &ts);
                self.lx.next()?;
            }
            _ => return Err(self.lx.syntaxerror("function arguments expected")),
        }
        let (lx, fs) = self.cur();
        assert_(f.k == 12)?;
        let base = f.info;
        let nparams = if hasmultret(args.k) {
            -1
        } else {
            if args.k != 0 {
                fs.exp2nextreg(lx, &mut args)?;
            }
            fs.freereg - (base + 1)
        };
        f.init(13, fs.code_abc(lx, OP_CALL, base, nparams + 1, 2)?);
        fs.fixline(line);
        fs.freereg = base + 1;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.prefixexp
    fn prefixexp(&mut self, v: &mut ExpDesc) -> R<()> {
        match self.lx.t.token {
            40 => {
                let line = self.lx.linenumber;
                self.lx.next()?;
                self.expr(v)?;
                self.lx.check_match(41, 40, line)?;
                let (lx, fs) = self.cur();
                fs.dischargevars(lx, v)
            }
            TK_NAME => self.singlevar(v),
            _ => Err(self.lx.syntaxerror("unexpected symbol")),
        }
    }

    /// org.luaj.kahluafork.compiler.LexState.primaryexp
    fn primaryexp(&mut self, v: &mut ExpDesc) -> R<()> {
        self.prefixexp(v)?;
        loop {
            match self.lx.t.token {
                40 | 123 | TK_STRING => {
                    {
                        let (lx, fs) = self.cur();
                        fs.exp2nextreg(lx, v)?;
                    }
                    self.funcargs(v)?;
                }
                46 => self.field(v)?,
                58 => {
                    let mut key = ExpDesc::default();
                    self.lx.next()?;
                    self.checkname(&mut key)?;
                    {
                        let (lx, fs) = self.cur();
                        fs.self_(lx, v, &mut key)?;
                    }
                    self.funcargs(v)?;
                }
                91 => {
                    let mut key = ExpDesc::default();
                    {
                        let (lx, fs) = self.cur();
                        fs.exp2anyreg(lx, v)?;
                    }
                    self.yindex(&mut key)?;
                    let (lx, fs) = self.cur();
                    fs.indexed(lx, v, &mut key)?;
                }
                _ => return Ok(()),
            }
        }
    }

    /// org.luaj.kahluafork.compiler.LexState.simpleexp
    fn simpleexp(&mut self, v: &mut ExpDesc) -> R<()> {
        match self.lx.t.token {
            123 => return self.constructor(v),
            TK_FALSE => v.init(3, 0),
            TK_FUNCTION => {
                self.lx.next()?;
                let line = self.lx.linenumber;
                return self.body(v, false, line);
            }
            TK_NIL => v.init(1, 0),
            TK_TRUE => v.init(2, 0),
            TK_DOTS => {
                let isv = self.cur_ref().is_vararg;
                self.lx
                    .check_condition(isv != 0, "cannot use '...' outside a vararg function")?;
                let (lx, fs) = self.cur();
                fs.is_vararg &= -5;
                v.init(14, fs.code_abc(lx, OP_VARARG, 0, 1, 0)?);
            }
            TK_NUMBER => {
                v.init(5, 0);
                v.set_nval(self.lx.t.r);
            }
            TK_STRING => {
                let ts = self.lx.t.ts.clone().unwrap_or_else(|| "null".to_string());
                self.codestring(v, &ts);
            }
            _ => return self.primaryexp(v),
        }
        self.lx.next()
    }

    /// org.luaj.kahluafork.compiler.LexState.subexpr
    fn subexpr(&mut self, v: &mut ExpDesc, limit: i32) -> R<i32> {
        self.enterlevel()?;
        let uop = getunopr(self.lx.t.token);
        if uop != 3 {
            self.lx.next()?;
            self.subexpr(v, UNARY_PRIORITY)?;
            let (lx, fs) = self.cur();
            fs.prefix(lx, uop, v)?;
        } else {
            self.simpleexp(v)?;
        }
        let mut op = getbinopr(self.lx.t.token);
        while op != 15 && PRIORITY_LEFT[op as usize] > limit {
            let mut v2 = ExpDesc::default();
            self.lx.next()?;
            {
                let (lx, fs) = self.cur();
                fs.infix(lx, op, v)?;
            }
            let nextop = self.subexpr(&mut v2, PRIORITY_RIGHT[op as usize])?;
            let (lx, fs) = self.cur();
            fs.posfix(lx, op, v, &mut v2)?;
            op = nextop;
        }
        self.leavelevel();
        Ok(op)
    }

    /// org.luaj.kahluafork.compiler.LexState.expr
    fn expr(&mut self, v: &mut ExpDesc) -> R<()> {
        self.subexpr(v, 0)?;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.block
    fn block(&mut self) -> R<()> {
        let idx = self.cur().1.enterblock(false)?;
        self.chunk()?;
        assert_(self.cur_ref().bl[idx].breaklist == -1)?;
        let (lx, fs) = self.cur();
        fs.leaveblock(lx)
    }

    /// org.luaj.kahluafork.compiler.LexState.check_conflict over the assignment's earlier targets.
    fn check_conflict(&mut self, lh: &mut [ExpDesc], v: &ExpDesc) -> R<()> {
        let (lx, fs) = self.cur();
        let extra = fs.freereg;
        let mut conflict = false;
        for target in lh.iter_mut().rev() {
            if target.k == 9 {
                if target.info == v.info {
                    conflict = true;
                    target.info = extra;
                }
                if target.aux == v.info {
                    conflict = true;
                    target.aux = extra;
                }
            }
        }
        if conflict {
            let r = fs.freereg;
            fs.code_abc(lx, OP_MOVE, r, v.info, 0)?;
            fs.reserveregs(lx, 1)?;
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.assignment: `lh` holds the targets so far, this level's last.
    fn assignment(&mut self, lh: &mut Vec<ExpDesc>, nvars: i32) -> R<()> {
        let cur = lh.len() - 1;
        let mut e = ExpDesc::default();
        let k = lh[cur].k;
        self.lx
            .check_condition((6..=9).contains(&k), "syntax error")?;
        if self.lx.testnext(44)? {
            let mut nv = ExpDesc::default();
            self.primaryexp(&mut nv)?;
            if nv.k == 6 {
                self.check_conflict(&mut lh[..], &nv)?;
            }
            lh.push(nv);
            self.assignment(lh, nvars + 1)?;
            lh.pop();
        } else {
            self.lx.checknext(61)?;
            let nexps = self.explist1(&mut e)?;
            if nexps == nvars {
                let (lx, fs) = self.cur();
                fs.setoneret(&mut e);
                return fs.storevar(lx, &lh[cur], &mut e);
            }
            self.adjust_assign(nvars, nexps, &mut e)?;
            if nexps > nvars {
                self.cur().1.freereg -= nexps - nvars;
            }
        }
        let (lx, fs) = self.cur();
        e.init(12, fs.freereg - 1);
        fs.storevar(lx, &lh[cur], &mut e)
    }

    /// org.luaj.kahluafork.compiler.LexState.cond
    fn cond(&mut self) -> R<i32> {
        let mut v = ExpDesc::default();
        self.expr(&mut v)?;
        if v.k == 1 {
            v.k = 3;
        }
        let (lx, fs) = self.cur();
        fs.goiftrue(lx, &mut v)?;
        Ok(v.f)
    }

    /// org.luaj.kahluafork.compiler.LexState.breakstat
    fn breakstat(&mut self) -> R<()> {
        let (lx, fs) = self.cur();
        let mut upval = false;
        let mut found = None;
        for i in (0..fs.bl.len()).rev() {
            if fs.bl[i].isbreakable {
                found = Some(i);
                break;
            }
            upval |= fs.bl[i].upval;
        }
        let Some(i) = found else {
            return Err(lx.syntaxerror("no loop to break"));
        };
        if upval {
            let n = fs.bl[i].nactvar;
            fs.code_abc(lx, 35, n, 0, 0)?;
        }
        let old = fs.bl[i].breaklist;
        let j = fs.jump(lx)?;
        fs.bl[i].breaklist = fs.concat(lx, old, j)?;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.whilestat
    fn whilestat(&mut self, line: i32) -> R<()> {
        self.lx.next()?;
        let whileinit = self.cur().1.getlabel();
        let condexit = self.cond()?;
        self.cur().1.enterblock(true)?;
        self.lx.checknext(TK_DO)?;
        self.block()?;
        {
            let (lx, fs) = self.cur();
            let j = fs.jump(lx)?;
            fs.patchlist(lx, j, whileinit)?;
        }
        self.lx.check_match(TK_END, TK_WHILE, line)?;
        let (lx, fs) = self.cur();
        fs.leaveblock(lx)?;
        fs.patchtohere(lx, condexit)
    }

    /// org.luaj.kahluafork.compiler.LexState.repeatstat
    fn repeatstat(&mut self, line: i32) -> R<()> {
        let repeat_init = self.cur().1.getlabel();
        self.cur().1.enterblock(true)?;
        let bl2 = self.cur().1.enterblock(false)?;
        self.lx.next()?;
        self.chunk()?;
        self.lx.check_match(TK_UNTIL, TK_REPEAT, line)?;
        let condexit = self.cond()?;
        if !self.cur_ref().bl[bl2].upval {
            let (lx, fs) = self.cur();
            fs.leaveblock(lx)?;
            fs.patchlist(lx, condexit, repeat_init)?;
        } else {
            self.breakstat()?;
            let (lx, fs) = self.cur();
            fs.patchtohere(lx, condexit)?;
            fs.leaveblock(lx)?;
            let j = fs.jump(lx)?;
            fs.patchlist(lx, j, repeat_init)?;
        }
        let (lx, fs) = self.cur();
        fs.leaveblock(lx)
    }

    /// org.luaj.kahluafork.compiler.LexState.exp1
    fn exp1(&mut self) -> R<i32> {
        let mut e = ExpDesc::default();
        self.expr(&mut e)?;
        let k = e.k;
        let (lx, fs) = self.cur();
        fs.exp2nextreg(lx, &mut e)?;
        Ok(k)
    }

    /// org.luaj.kahluafork.compiler.LexState.forbody
    fn forbody(&mut self, base: i32, line: i32, nvars: i32, isnum: bool) -> R<()> {
        self.adjustlocalvars(3);
        self.lx.checknext(TK_DO)?;
        let prep = {
            let (lx, fs) = self.cur();
            if isnum {
                fs.code_asbx(lx, OP_FORPREP, base, -1)?
            } else {
                fs.jump(lx)?
            }
        };
        self.cur().1.enterblock(false)?;
        self.adjustlocalvars(nvars);
        {
            let (lx, fs) = self.cur();
            fs.reserveregs(lx, nvars)?;
        }
        self.block()?;
        let (lx, fs) = self.cur();
        fs.leaveblock(lx)?;
        fs.patchtohere(lx, prep)?;
        let endfor = if isnum {
            fs.code_asbx(lx, OP_FORLOOP, base, -1)?
        } else {
            fs.code_abc(lx, OP_TFORLOOP, base, 0, nvars)?
        };
        fs.fixline(line);
        let list = if isnum { endfor } else { fs.jump(lx)? };
        fs.patchlist(lx, list, prep + 1)
    }

    /// org.luaj.kahluafork.compiler.LexState.fornum
    fn fornum(&mut self, varname: Option<String>, line: i32) -> R<()> {
        let base = self.cur_ref().freereg;
        self.new_localvarliteral("(for index)", 0)?;
        self.new_localvarliteral("(for limit)", 1)?;
        self.new_localvarliteral("(for step)", 2)?;
        self.new_localvar(varname, 3)?;
        self.lx.checknext(61)?;
        self.exp1()?;
        self.lx.checknext(44)?;
        self.exp1()?;
        if self.lx.testnext(44)? {
            self.exp1()?;
        } else {
            let (lx, fs) = self.cur();
            let r = fs.freereg;
            let k = fs.number_k(1.0);
            fs.code_abx(lx, OP_LOADK, r, k)?;
            fs.reserveregs(lx, 1)?;
        }
        self.forbody(base, line, 1, true)
    }

    /// org.luaj.kahluafork.compiler.LexState.forlist
    fn forlist(&mut self, indexname: Option<String>) -> R<()> {
        let mut e = ExpDesc::default();
        let mut nvars = 0;
        let base = self.cur_ref().freereg;
        self.new_localvarliteral("(for generator)", nvars)?;
        nvars += 1;
        self.new_localvarliteral("(for state)", nvars)?;
        nvars += 1;
        self.new_localvarliteral("(for control)", nvars)?;
        nvars += 1;
        self.new_localvar(indexname, nvars)?;
        nvars += 1;
        while self.lx.testnext(44)? {
            let name = self.lx.str_checkname()?;
            self.new_localvar(name, nvars)?;
            nvars += 1;
        }
        self.lx.checknext(TK_IN)?;
        let line = self.lx.linenumber;
        let n = self.explist1(&mut e)?;
        self.adjust_assign(3, n, &mut e)?;
        {
            let (lx, fs) = self.cur();
            fs.checkstack(lx, 3)?;
        }
        self.forbody(base, line, nvars - 3, false)
    }

    /// org.luaj.kahluafork.compiler.LexState.forstat
    fn forstat(&mut self, line: i32) -> R<()> {
        self.cur().1.enterblock(true)?;
        self.lx.next()?;
        let varname = self.lx.str_checkname()?;
        match self.lx.t.token {
            44 | TK_IN => self.forlist(varname)?,
            61 => self.fornum(varname, line)?,
            _ => return Err(self.lx.syntaxerror("'=' or 'in' expected")),
        }
        self.lx.check_match(TK_END, TK_FOR, line)?;
        let (lx, fs) = self.cur();
        fs.leaveblock(lx)
    }

    /// org.luaj.kahluafork.compiler.LexState.test_then_block
    fn test_then_block(&mut self) -> R<i32> {
        self.lx.next()?;
        let condexit = self.cond()?;
        self.lx.checknext(TK_THEN)?;
        self.block()?;
        Ok(condexit)
    }

    /// org.luaj.kahluafork.compiler.LexState.ifstat
    fn ifstat(&mut self, line: i32) -> R<()> {
        let mut escapelist = -1;
        let mut flist = self.test_then_block()?;
        while self.lx.t.token == TK_ELSEIF {
            {
                let (lx, fs) = self.cur();
                let j = fs.jump(lx)?;
                escapelist = fs.concat(lx, escapelist, j)?;
                fs.patchtohere(lx, flist)?;
            }
            flist = self.test_then_block()?;
        }
        if self.lx.t.token == TK_ELSE {
            {
                let (lx, fs) = self.cur();
                let j = fs.jump(lx)?;
                escapelist = fs.concat(lx, escapelist, j)?;
                fs.patchtohere(lx, flist)?;
            }
            self.lx.next()?;
            self.block()?;
        } else {
            let (lx, fs) = self.cur();
            escapelist = fs.concat(lx, escapelist, flist)?;
        }
        {
            let (lx, fs) = self.cur();
            fs.patchtohere(lx, escapelist)?;
        }
        self.lx.check_match(TK_END, TK_IF, line)
    }

    /// org.luaj.kahluafork.compiler.LexState.localfunc
    fn localfunc(&mut self) -> R<()> {
        let mut v = ExpDesc::default();
        let mut b = ExpDesc::default();
        let name = self.lx.str_checkname()?;
        self.new_localvar(name, 0)?;
        {
            let (lx, fs) = self.cur();
            v.init(6, fs.freereg);
            fs.reserveregs(lx, 1)?;
        }
        self.adjustlocalvars(1);
        let line = self.lx.linenumber;
        self.body(&mut b, false, line)?;
        let (lx, fs) = self.cur();
        fs.storevar(lx, &v, &mut b)
    }

    /// org.luaj.kahluafork.compiler.LexState.localstat
    fn localstat(&mut self) -> R<()> {
        let mut nvars = 0;
        let mut e = ExpDesc::default();
        loop {
            let name = self.lx.str_checkname()?;
            self.new_localvar(name, nvars)?;
            nvars += 1;
            if !self.lx.testnext(44)? {
                break;
            }
        }
        let nexps = if self.lx.testnext(61)? {
            self.explist1(&mut e)?
        } else {
            e.k = 0;
            0
        };
        self.adjust_assign(nvars, nexps, &mut e)?;
        self.adjustlocalvars(nvars);
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.funcname
    fn funcname(&mut self, v: &mut ExpDesc) -> R<bool> {
        let mut needself = false;
        self.singlevar(v)?;
        while self.lx.t.token == 46 {
            self.field(v)?;
        }
        if self.lx.t.token == 58 {
            needself = true;
            self.field(v)?;
        }
        Ok(needself)
    }

    /// org.luaj.kahluafork.compiler.LexState.funcstat
    fn funcstat(&mut self, line: i32) -> R<()> {
        let mut v = ExpDesc::default();
        let mut b = ExpDesc::default();
        self.lx.next()?;
        let needself = self.funcname(&mut v)?;
        self.body(&mut b, needself, line)?;
        let (lx, fs) = self.cur();
        fs.storevar(lx, &v, &mut b)?;
        fs.fixline(line);
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.LexState.exprstat
    fn exprstat(&mut self) -> R<()> {
        let mut v = ExpDesc::default();
        self.primaryexp(&mut v)?;
        if v.k == 13 {
            let (_, fs) = self.cur();
            let i = fs.getcode(&v);
            fs.set_code(&v, setarg_c(i, 1));
            Ok(())
        } else {
            let mut lh = vec![v];
            self.assignment(&mut lh, 1)
        }
    }

    /// org.luaj.kahluafork.compiler.LexState.retstat
    fn retstat(&mut self) -> R<()> {
        let mut e = ExpDesc::default();
        self.lx.next()?;
        let (first, nret);
        if !block_follow(self.lx.t.token) && self.lx.t.token != 59 {
            let n = self.explist1(&mut e)?;
            let (lx, fs) = self.cur();
            if hasmultret(e.k) {
                fs.setmultret(lx, &mut e)?;
                if e.k == 13 && n == 1 {
                    let i = fs.getcode(&e);
                    fs.set_code(&e, set_opcode(i, OP_TAILCALL));
                    assert_(getarg_a(fs.getcode(&e)) == fs.nactvar)?;
                }
                first = fs.nactvar;
                nret = -1;
            } else if n == 1 {
                first = fs.exp2anyreg(lx, &mut e)?;
                nret = n;
            } else {
                fs.exp2nextreg(lx, &mut e)?;
                first = fs.nactvar;
                assert_(n == fs.freereg - first)?;
                nret = n;
            }
        } else {
            nret = 0;
            first = 0;
        }
        let (lx, fs) = self.cur();
        fs.ret(lx, first, nret)
    }

    /// org.luaj.kahluafork.compiler.LexState.statement: true for a statement that must be last (`return`,
    /// `break`).
    fn statement(&mut self) -> R<bool> {
        let line = self.lx.linenumber;
        match self.lx.t.token {
            TK_BREAK => {
                self.lx.next()?;
                self.breakstat()?;
                Ok(true)
            }
            TK_DO => {
                self.lx.next()?;
                self.block()?;
                self.lx.check_match(TK_END, TK_DO, line)?;
                Ok(false)
            }
            TK_FOR => {
                self.forstat(line)?;
                Ok(false)
            }
            TK_FUNCTION => {
                self.funcstat(line)?;
                Ok(false)
            }
            TK_IF => {
                self.ifstat(line)?;
                Ok(false)
            }
            TK_LOCAL => {
                self.lx.next()?;
                if self.lx.testnext(TK_FUNCTION)? {
                    self.localfunc()?;
                } else {
                    self.localstat()?;
                }
                Ok(false)
            }
            TK_REPEAT => {
                self.repeatstat(line)?;
                Ok(false)
            }
            TK_RETURN => {
                self.retstat()?;
                Ok(true)
            }
            TK_WHILE => {
                self.whilestat(line)?;
                Ok(false)
            }
            _ => {
                self.exprstat()?;
                Ok(false)
            }
        }
    }

    /// org.luaj.kahluafork.compiler.LexState.chunk
    fn chunk(&mut self) -> R<()> {
        let mut islast = false;
        self.enterlevel()?;
        while !islast && !block_follow(self.lx.t.token) {
            islast = self.statement()?;
            self.lx.testnext(59)?;
            let fs = self.cur_ref();
            assert_(fs.f.max_stacksize >= fs.freereg && fs.freereg >= fs.nactvar)?;
            let (_, fs) = self.cur();
            fs.freereg = fs.nactvar;
        }
        self.leavelevel();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compile_str(src: &str) -> R<Prototype> {
        compile(
            src.encode_utf16().collect(),
            Some("t.lua"),
            None,
            Some("t.lua"),
            Some("t.lua"),
        )
    }

    /// `local a = 1 + 2` folds to one LOADK of 3 and the return; `x = y` is GETGLOBAL then SETGLOBAL.
    #[test]
    fn folds_and_globals() {
        let p = compile_str("local a = 1 + 2\nx = y").unwrap();
        assert_eq!(p.constants.len(), 3);
        assert_eq!(super::super::func_state::get_opcode(p.code[0]), OP_LOADK);
        assert_eq!(p.code.len(), 4);
        assert_eq!(p.lines, [1, 2, 2, 2]);
        assert_eq!(p.locvars, ["a"]);
    }

    /// Error messages carry the chunk name, line and token.
    #[test]
    fn syntax_error_message() {
        assert_eq!(
            compile_str("x = = 1").unwrap_err(),
            "t.lua:1: unexpected symbol near `=`"
        );
        assert_eq!(
            compile_str("x = 'a").unwrap_err(),
            "t.lua:1: unfinished string near `<eof>`"
        );
    }

    /// An anonymous function takes the name of the last name lexed into the current token.
    #[test]
    fn function_names() {
        let p = compile_str("function a.b:c() end\nlocal f = function() end").unwrap();
        assert_eq!(p.prototypes[0].name.as_deref(), Some("c"));
        assert_eq!(p.prototypes[1].name.as_deref(), Some("f"));
    }
}
