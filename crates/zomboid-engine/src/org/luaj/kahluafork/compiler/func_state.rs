//! org.luaj.kahluafork.compiler.FuncState: the code generator for one function being compiled (luaj's port of
//! Lua 5.1's lcode.c), with the instruction encoding and the opcode table.
use std::collections::HashMap;

use super::exp_desc::ExpDesc;
use super::lex_state::{Lexer, R};
use crate::se::krka::kahlua::vm::prototype::{Constant, Prototype};

pub const MAXSTACK: i32 = 250;
const LUAI_MAXUPVALUES: i32 = 60;
pub const LUAI_MAXVARS: i32 = 200;
pub const NO_REG: i32 = 255;
pub const MAXINDEXRK: i32 = 255;
pub const MAXARG_SBX: i32 = 131071;
pub const MAXARG_C: i32 = 511;
pub const LFIELDS_PER_FLUSH: i32 = 50;

pub const OP_MOVE: i32 = 0;
pub const OP_LOADK: i32 = 1;
pub const OP_LOADBOOL: i32 = 2;
pub const OP_LOADNIL: i32 = 3;
pub const OP_GETUPVAL: i32 = 4;
pub const OP_GETGLOBAL: i32 = 5;
pub const OP_GETTABLE: i32 = 6;
pub const OP_SETGLOBAL: i32 = 7;
pub const OP_SETUPVAL: i32 = 8;
pub const OP_SETTABLE: i32 = 9;
pub const OP_NEWTABLE: i32 = 10;
pub const OP_SELF: i32 = 11;
pub const OP_ADD: i32 = 12;
pub const OP_SUB: i32 = 13;
pub const OP_MUL: i32 = 14;
pub const OP_DIV: i32 = 15;
pub const OP_MOD: i32 = 16;
pub const OP_POW: i32 = 17;
pub const OP_UNM: i32 = 18;
pub const OP_NOT: i32 = 19;
pub const OP_LEN: i32 = 20;
pub const OP_CONCAT: i32 = 21;
pub const OP_JMP: i32 = 22;
pub const OP_EQ: i32 = 23;
pub const OP_LT: i32 = 24;
pub const OP_LE: i32 = 25;
pub const OP_TEST: i32 = 26;
pub const OP_TESTSET: i32 = 27;
pub const OP_CALL: i32 = 28;
pub const OP_TAILCALL: i32 = 29;
pub const OP_RETURN: i32 = 30;
pub const OP_FORLOOP: i32 = 31;
pub const OP_FORPREP: i32 = 32;
pub const OP_TFORLOOP: i32 = 33;
pub const OP_SETLIST: i32 = 34;
pub const OP_CLOSE: i32 = 35;
pub const OP_CLOSURE: i32 = 36;
pub const OP_VARARG: i32 = 37;

/// org.luaj.kahluafork.compiler.FuncState.luaP_opmodes
const LUAP_OPMODES: [i32; 38] = [
    96, 113, 84, 96, 80, 113, 108, 49, 16, 60, 84, 108, 124, 124, 124, 124, 124, 124, 96, 96, 96,
    104, 34, 188, 188, 188, 228, 228, 84, 84, 16, 98, 98, 132, 20, 0, 81, 80,
];

/// org.luaj.kahluafork.compiler.FuncState.GET_OPCODE
pub fn get_opcode(i: i32) -> i32 {
    i & 63
}

/// org.luaj.kahluafork.compiler.FuncState.GETARG_A
pub fn getarg_a(i: i32) -> i32 {
    (i >> 6) & 0xFF
}

/// org.luaj.kahluafork.compiler.FuncState.GETARG_B
pub fn getarg_b(i: i32) -> i32 {
    (i >> 23) & 511
}

/// org.luaj.kahluafork.compiler.FuncState.GETARG_C
pub fn getarg_c(i: i32) -> i32 {
    (i >> 14) & 511
}

/// org.luaj.kahluafork.compiler.FuncState.GETARG_sBx
pub fn getarg_sbx(i: i32) -> i32 {
    ((i >> 14) & 262143) - 131071
}

/// org.luaj.kahluafork.compiler.FuncState.SET_OPCODE
pub fn set_opcode(i: i32, o: i32) -> i32 {
    (i & -64) | (o & 63)
}

/// org.luaj.kahluafork.compiler.FuncState.SETARG_A
pub fn setarg_a(i: i32, u: i32) -> i32 {
    (i & -16321) | ((u << 6) & 16320)
}

/// org.luaj.kahluafork.compiler.FuncState.SETARG_B
pub fn setarg_b(i: i32, u: i32) -> i32 {
    (i & 8388607) | (u.wrapping_shl(23) & -8388608)
}

/// org.luaj.kahluafork.compiler.FuncState.SETARG_C
pub fn setarg_c(i: i32, u: i32) -> i32 {
    (i & -8372225) | ((u << 14) & 8372224)
}

/// org.luaj.kahluafork.compiler.FuncState.SETARG_Bx
fn setarg_bx(i: i32, u: i32) -> i32 {
    (i & 16383) | (u.wrapping_shl(14) & -16384)
}

/// org.luaj.kahluafork.compiler.FuncState.CREATE_ABC
pub fn create_abc(o: i32, a: i32, b: i32, c: i32) -> i32 {
    (o & 63) | ((a << 6) & 16320) | (b.wrapping_shl(23) & -8388608) | ((c << 14) & 8372224)
}

/// org.luaj.kahluafork.compiler.FuncState.CREATE_ABx
fn create_abx(o: i32, a: i32, bc: i32) -> i32 {
    (o & 63) | ((a << 6) & 16320) | (bc.wrapping_shl(14) & -16384)
}

/// org.luaj.kahluafork.compiler.FuncState.ISK
fn isk(x: i32) -> bool {
    (x & 256) != 0
}

/// org.luaj.kahluafork.compiler.FuncState.RKASK
fn rkask(x: i32) -> i32 {
    x | 256
}

fn get_op_mode(m: i32) -> i32 {
    LUAP_OPMODES[m as usize] & 3
}

fn get_b_mode(m: i32) -> i32 {
    (LUAP_OPMODES[m as usize] >> 4) & 3
}

fn get_c_mode(m: i32) -> i32 {
    (LUAP_OPMODES[m as usize] >> 2) & 3
}

fn test_t_mode(m: i32) -> bool {
    (LUAP_OPMODES[m as usize] & 128) != 0
}

/// org.luaj.kahluafork.compiler.FuncState._assert: "compiler assert failed".
pub fn assert_(b: bool) -> R<()> {
    if b {
        Ok(())
    } else {
        Err("compiler assert failed".to_string())
    }
}

/// org.luaj.kahluafork.compiler.BlockCnt (`previous` is the block below it on the function's block stack).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockCnt {
    pub breaklist: i32,
    pub nactvar: i32,
    pub upval: bool,
    pub isbreakable: bool,
}

/// org.luaj.kahluafork.compiler.ConsControl (`t`, the table being built, is passed beside it).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ConsControl {
    pub v: ExpDesc,
    pub nh: i32,
    pub na: i32,
    pub tostore: i32,
}

/// A key of the constant table (`Hashtable<Object, Integer>`): `Double.equals` compares `doubleToLongBits`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ConstKey {
    Nil,
    Boolean(bool),
    Number(u64),
    String(String),
}

/// org.luaj.kahluafork.compiler.FuncState
#[derive(Clone, Debug)]
pub struct FuncState {
    pub locvars: Vec<String>,
    pub upvalues: Vec<String>,
    pub linedefined: i32,
    pub lastlinedefined: i32,
    pub is_vararg: i32,
    pub f: Prototype,
    htable: HashMap<ConstKey, i32>,
    /// `bl` and its `previous` chain, innermost last.
    pub bl: Vec<BlockCnt>,
    pub pc: i32,
    lasttarget: i32,
    jpc: i32,
    pub freereg: i32,
    nk: i32,
    pub np: i32,
    pub nactvar: i32,
    pub upvalues_k: [i32; 60],
    pub upvalues_info: [i32; 60],
    pub actvar: [i16; 200],
}

impl FuncState {
    /// org.luaj.kahluafork.compiler.FuncState(LexState, String): a new function named `name` (the main chunk's
    /// constructor, which copies the enclosing function's name, is the same with that name).
    pub fn new(name: Option<String>) -> Self {
        FuncState {
            locvars: Vec::new(),
            upvalues: Vec::new(),
            linedefined: 0,
            lastlinedefined: 0,
            is_vararg: 0,
            f: Prototype {
                name,
                max_stacksize: 2,
                ..Prototype::default()
            },
            htable: HashMap::new(),
            bl: Vec::new(),
            pc: 0,
            lasttarget: -1,
            jpc: -1,
            freereg: 0,
            nk: 0,
            np: 0,
            nactvar: 0,
            upvalues_k: [0; 60],
            upvalues_info: [0; 60],
            actvar: [0; 200],
        }
    }

    /// The instruction at `pc` (`InstructionPtr.get`).
    pub fn code_at(&self, pc: i32) -> i32 {
        self.f.code[pc as usize]
    }

    /// `InstructionPtr.set` at `pc`.
    pub fn set_code_at(&mut self, pc: i32, v: i32) {
        self.f.code[pc as usize] = v;
    }

    /// org.luaj.kahluafork.compiler.FuncState.getcode
    pub fn getcode(&self, e: &ExpDesc) -> i32 {
        self.code_at(e.info)
    }

    /// `getcodePtr(e).set(v)`
    pub fn set_code(&mut self, e: &ExpDesc, v: i32) {
        self.set_code_at(e.info, v);
    }

    /// org.luaj.kahluafork.compiler.FuncState.codeAsBx
    pub fn code_asbx(&mut self, lx: &Lexer, o: i32, a: i32, sbx: i32) -> R<i32> {
        self.code_abx(lx, o, a, sbx + 131071)
    }

    /// org.luaj.kahluafork.compiler.FuncState.setmultret
    pub fn setmultret(&mut self, lx: &Lexer, e: &mut ExpDesc) -> R<()> {
        self.setreturns(lx, e, -1)
    }

    /// org.luaj.kahluafork.compiler.FuncState.getlocvar
    fn getlocvar(&self, i: i32) -> &str {
        &self.locvars[self.actvar[i as usize] as usize]
    }

    /// org.luaj.kahluafork.compiler.FuncState.checklimit
    pub fn checklimit(&self, lx: &Lexer, v: i32, l: i32, msg: &str) -> R<()> {
        if v > l {
            return Err(self.errorlimit(lx, l, msg));
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.errorlimit
    fn errorlimit(&self, lx: &Lexer, limit: i32, what: &str) -> String {
        let msg = if self.linedefined == 0 {
            format!("main function has more than {limit} {what}")
        } else {
            format!(
                "function at line {} has more than {limit} {what}",
                self.linedefined
            )
        };
        lx.lexerror(&msg, 0)
    }

    /// org.luaj.kahluafork.compiler.FuncState.indexupvalue
    pub fn indexupvalue(&mut self, lx: &Lexer, name: &str, v: &ExpDesc) -> R<i32> {
        for i in 0..self.f.num_upvalues {
            if self.upvalues_k[i as usize] == v.k && self.upvalues_info[i as usize] == v.info {
                assert_(self.upvalues[i as usize] == name)?;
                return Ok(i);
            }
        }
        self.checklimit(lx, self.f.num_upvalues + 1, LUAI_MAXUPVALUES, "upvalues")?;
        assert_(v.k == 6 || v.k == 7)?;
        let n = self.f.num_upvalues;
        self.f.num_upvalues += 1;
        self.upvalues.truncate(n as usize);
        self.upvalues.push(name.to_string());
        self.upvalues_k[n as usize] = v.k;
        self.upvalues_info[n as usize] = v.info;
        Ok(n)
    }

    /// org.luaj.kahluafork.compiler.FuncState.searchvar
    pub fn searchvar(&self, n: &str) -> i32 {
        let mut i = self.nactvar - 1;
        while i >= 0 {
            if n == self.getlocvar(i) {
                return i;
            }
            i -= 1;
        }
        -1
    }

    /// org.luaj.kahluafork.compiler.FuncState.markupval
    pub fn markupval(&mut self, level: i32) {
        if let Some(bl) = self.bl.iter_mut().rev().find(|bl| bl.nactvar <= level) {
            bl.upval = true;
        }
    }

    /// org.luaj.kahluafork.compiler.FuncState.enterblock: pushes the block; returns its index on the stack.
    pub fn enterblock(&mut self, isbreakable: bool) -> R<usize> {
        self.bl.push(BlockCnt {
            breaklist: -1,
            isbreakable,
            nactvar: self.nactvar,
            upval: false,
        });
        assert_(self.freereg == self.nactvar)?;
        Ok(self.bl.len() - 1)
    }

    /// org.luaj.kahluafork.compiler.FuncState.leaveblock (`ls.removevars` sets this function's `nactvar`).
    pub fn leaveblock(&mut self, lx: &Lexer) -> R<()> {
        let bl = self.bl.pop().ok_or("compiler assert failed")?;
        self.nactvar = bl.nactvar;
        if bl.upval {
            self.code_abc(lx, OP_CLOSE, bl.nactvar, 0, 0)?;
        }
        assert_(!bl.isbreakable || !bl.upval)?;
        assert_(bl.nactvar == self.nactvar)?;
        self.freereg = self.nactvar;
        self.patchtohere(lx, bl.breaklist)
    }

    /// org.luaj.kahluafork.compiler.FuncState.closelistfield
    pub fn closelistfield(&mut self, lx: &Lexer, cc: &mut ConsControl, t_info: i32) -> R<()> {
        if cc.v.k != 0 {
            self.exp2nextreg(lx, &mut cc.v)?;
            cc.v.k = 0;
            if cc.tostore == LFIELDS_PER_FLUSH {
                self.setlist(lx, t_info, cc.na, cc.tostore)?;
                cc.tostore = 0;
            }
        }
        Ok(())
    }

    fn hasmultret(k: i32) -> bool {
        k == 13 || k == 14
    }

    /// org.luaj.kahluafork.compiler.FuncState.lastlistfield
    pub fn lastlistfield(&mut self, lx: &Lexer, cc: &mut ConsControl, t_info: i32) -> R<()> {
        if cc.tostore != 0 {
            if Self::hasmultret(cc.v.k) {
                self.setmultret(lx, &mut cc.v)?;
                self.setlist(lx, t_info, cc.na, -1)?;
                cc.na -= 1;
            } else {
                if cc.v.k != 0 {
                    self.exp2nextreg(lx, &mut cc.v)?;
                }
                self.setlist(lx, t_info, cc.na, cc.tostore)?;
            }
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.nil
    pub fn nil(&mut self, lx: &Lexer, from: i32, n: i32) -> R<()> {
        if self.pc > self.lasttarget {
            if self.pc == 0 {
                if from >= self.nactvar {
                    return Ok(());
                }
            } else {
                let previous = self.code_at(self.pc - 1);
                if get_opcode(previous) == OP_LOADNIL {
                    let pfrom = getarg_a(previous);
                    let pto = getarg_b(previous);
                    if pfrom <= from && from <= pto + 1 {
                        if from + n - 1 > pto {
                            self.set_code_at(self.pc - 1, setarg_b(previous, from + n - 1));
                        }
                        return Ok(());
                    }
                }
            }
        }
        self.code_abc(lx, OP_LOADNIL, from, from + n - 1, 0)?;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.jump
    pub fn jump(&mut self, lx: &Lexer) -> R<i32> {
        let jpc = self.jpc;
        self.jpc = -1;
        let j = self.code_asbx(lx, OP_JMP, 0, -1)?;
        self.concat(lx, j, jpc)
    }

    /// org.luaj.kahluafork.compiler.FuncState.ret
    pub fn ret(&mut self, lx: &Lexer, first: i32, nret: i32) -> R<()> {
        self.code_abc(lx, OP_RETURN, first, nret + 1, 0)?;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.condjump
    fn condjump(&mut self, lx: &Lexer, op: i32, a: i32, b: i32, c: i32) -> R<i32> {
        self.code_abc(lx, op, a, b, c)?;
        self.jump(lx)
    }

    /// org.luaj.kahluafork.compiler.FuncState.fixjump
    fn fixjump(&mut self, lx: &Lexer, pc: i32, dest: i32) -> R<()> {
        let offset = dest - (pc + 1);
        assert_(dest != -1)?;
        if offset.abs() > MAXARG_SBX {
            return Err(lx.syntaxerror("control structure too long"));
        }
        let i = self.code_at(pc);
        self.set_code_at(pc, setarg_bx(i, offset + MAXARG_SBX));
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.getlabel
    pub fn getlabel(&mut self) -> i32 {
        self.lasttarget = self.pc;
        self.pc
    }

    /// org.luaj.kahluafork.compiler.FuncState.getjump
    fn getjump(&self, pc: i32) -> i32 {
        let offset = getarg_sbx(self.code_at(pc));
        if offset == -1 {
            -1
        } else {
            pc + 1 + offset
        }
    }

    /// org.luaj.kahluafork.compiler.FuncState.getjumpcontrol: the index of the instruction that controls the jump.
    fn getjumpcontrol(&self, pc: i32) -> i32 {
        if pc >= 1 && test_t_mode(get_opcode(self.code_at(pc - 1))) {
            pc - 1
        } else {
            pc
        }
    }

    /// org.luaj.kahluafork.compiler.FuncState.need_value
    fn need_value(&self, mut list: i32) -> bool {
        while list != -1 {
            let i = self.code_at(self.getjumpcontrol(list));
            if get_opcode(i) != OP_TESTSET {
                return true;
            }
            list = self.getjump(list);
        }
        false
    }

    /// org.luaj.kahluafork.compiler.FuncState.patchtestreg
    fn patchtestreg(&mut self, node: i32, reg: i32) -> bool {
        let at = self.getjumpcontrol(node);
        let i = self.code_at(at);
        if get_opcode(i) != OP_TESTSET {
            return false;
        }
        if reg != NO_REG && reg != getarg_b(i) {
            self.set_code_at(at, setarg_a(i, reg));
        } else {
            self.set_code_at(at, create_abc(OP_TEST, getarg_b(i), 0, getarg_c(i)));
        }
        true
    }

    /// org.luaj.kahluafork.compiler.FuncState.removevalues
    fn removevalues(&mut self, mut list: i32) {
        while list != -1 {
            self.patchtestreg(list, NO_REG);
            list = self.getjump(list);
        }
    }

    /// org.luaj.kahluafork.compiler.FuncState.patchlistaux
    fn patchlistaux(
        &mut self,
        lx: &Lexer,
        mut list: i32,
        vtarget: i32,
        reg: i32,
        dtarget: i32,
    ) -> R<()> {
        while list != -1 {
            let next = self.getjump(list);
            if self.patchtestreg(list, reg) {
                self.fixjump(lx, list, vtarget)?;
            } else {
                self.fixjump(lx, list, dtarget)?;
            }
            list = next;
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.dischargejpc
    fn dischargejpc(&mut self, lx: &Lexer) -> R<()> {
        self.patchlistaux(lx, self.jpc, self.pc, NO_REG, self.pc)?;
        self.jpc = -1;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.patchlist
    pub fn patchlist(&mut self, lx: &Lexer, list: i32, target: i32) -> R<()> {
        if target == self.pc {
            self.patchtohere(lx, list)
        } else {
            assert_(target < self.pc)?;
            self.patchlistaux(lx, list, target, NO_REG, target)
        }
    }

    /// org.luaj.kahluafork.compiler.FuncState.patchtohere
    pub fn patchtohere(&mut self, lx: &Lexer, list: i32) -> R<()> {
        self.getlabel();
        self.jpc = self.concat(lx, self.jpc, list)?;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.concat
    pub fn concat(&mut self, lx: &Lexer, l1: i32, l2: i32) -> R<i32> {
        if l2 == -1 {
            return Ok(l1);
        }
        if l1 == -1 {
            return Ok(l2);
        }
        let mut list = l1;
        loop {
            let next = self.getjump(list);
            if next == -1 {
                break;
            }
            list = next;
        }
        self.fixjump(lx, list, l2)?;
        Ok(l1)
    }

    /// org.luaj.kahluafork.compiler.FuncState.checkstack
    pub fn checkstack(&mut self, lx: &Lexer, n: i32) -> R<()> {
        let newstack = self.freereg + n;
        if newstack > self.f.max_stacksize {
            if newstack >= MAXSTACK {
                return Err(lx.syntaxerror("function or expression too complex"));
            }
            self.f.max_stacksize = newstack;
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.reserveregs
    pub fn reserveregs(&mut self, lx: &Lexer, n: i32) -> R<()> {
        self.checkstack(lx, n)?;
        self.freereg += n;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.freereg(int)
    fn free_reg(&mut self, reg: i32) -> R<()> {
        if !isk(reg) && reg >= self.nactvar {
            self.freereg -= 1;
            assert_(reg == self.freereg)?;
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.freeexp
    fn freeexp(&mut self, e: &ExpDesc) -> R<()> {
        if e.k == 12 {
            self.free_reg(e.info)?;
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.addk
    fn addk(&mut self, v: Constant) -> i32 {
        let key = match &v {
            Constant::Nil => ConstKey::Nil,
            Constant::Boolean(b) => ConstKey::Boolean(*b),
            Constant::Number(d) => ConstKey::Number(if d.is_nan() {
                0x7ff8_0000_0000_0000
            } else {
                d.to_bits()
            }),
            Constant::String(s) => ConstKey::String(s.clone()),
        };
        if let Some(&idx) = self.htable.get(&key) {
            return idx;
        }
        let idx = self.nk;
        self.htable.insert(key, idx);
        self.f.constants.push(v);
        self.nk += 1;
        idx
    }

    /// org.luaj.kahluafork.compiler.FuncState.stringK
    pub fn string_k(&mut self, s: &str) -> i32 {
        self.addk(Constant::String(s.to_string()))
    }

    /// org.luaj.kahluafork.compiler.FuncState.numberK
    pub fn number_k(&mut self, r: f64) -> i32 {
        self.addk(Constant::Number(r))
    }

    fn bool_k(&mut self, b: bool) -> i32 {
        self.addk(Constant::Boolean(b))
    }

    fn nil_k(&mut self) -> i32 {
        self.addk(Constant::Nil)
    }

    /// org.luaj.kahluafork.compiler.FuncState.setreturns
    pub fn setreturns(&mut self, lx: &Lexer, e: &mut ExpDesc, nresults: i32) -> R<()> {
        if e.k == 13 {
            let i = self.getcode(e);
            self.set_code(e, setarg_c(i, nresults + 1));
        } else if e.k == 14 {
            let i = self.getcode(e);
            self.set_code(e, setarg_b(i, nresults + 1));
            let i = self.getcode(e);
            self.set_code(e, setarg_a(i, self.freereg));
            self.reserveregs(lx, 1)?;
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.setoneret
    pub fn setoneret(&mut self, e: &mut ExpDesc) {
        if e.k == 13 {
            e.k = 12;
            e.info = getarg_a(self.getcode(e));
        } else if e.k == 14 {
            let i = self.getcode(e);
            self.set_code(e, setarg_b(i, 2));
            e.k = 11;
        }
    }

    /// org.luaj.kahluafork.compiler.FuncState.dischargevars
    pub fn dischargevars(&mut self, lx: &Lexer, e: &mut ExpDesc) -> R<()> {
        match e.k {
            6 => e.k = 12,
            7 => {
                e.info = self.code_abc(lx, OP_GETUPVAL, 0, e.info, 0)?;
                e.k = 11;
            }
            8 => {
                e.info = self.code_abx(lx, OP_GETGLOBAL, 0, e.info)?;
                e.k = 11;
            }
            9 => {
                self.free_reg(e.aux)?;
                self.free_reg(e.info)?;
                e.info = self.code_abc(lx, OP_GETTABLE, 0, e.info, e.aux)?;
                e.k = 11;
            }
            13 | 14 => self.setoneret(e),
            _ => {}
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.code_label
    fn code_label(&mut self, lx: &Lexer, a: i32, b: i32, jump: i32) -> R<i32> {
        self.getlabel();
        self.code_abc(lx, OP_LOADBOOL, a, b, jump)
    }

    /// org.luaj.kahluafork.compiler.FuncState.discharge2reg
    fn discharge2reg(&mut self, lx: &Lexer, e: &mut ExpDesc, reg: i32) -> R<()> {
        self.dischargevars(lx, e)?;
        match e.k {
            1 => self.nil(lx, reg, 1)?,
            2 | 3 => {
                self.code_abc(lx, OP_LOADBOOL, reg, i32::from(e.k == 2), 0)?;
            }
            4 => {
                self.code_abx(lx, OP_LOADK, reg, e.info)?;
            }
            5 => {
                let k = self.number_k(e.nval());
                self.code_abx(lx, OP_LOADK, reg, k)?;
            }
            11 => {
                let i = self.getcode(e);
                self.set_code(e, setarg_a(i, reg));
            }
            12 => {
                if reg != e.info {
                    self.code_abc(lx, OP_MOVE, reg, e.info, 0)?;
                }
            }
            _ => {
                assert_(e.k == 0 || e.k == 10)?;
                return Ok(());
            }
        }
        e.info = reg;
        e.k = 12;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.discharge2anyreg
    fn discharge2anyreg(&mut self, lx: &Lexer, e: &mut ExpDesc) -> R<()> {
        if e.k != 12 {
            self.reserveregs(lx, 1)?;
            self.discharge2reg(lx, e, self.freereg - 1)?;
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.exp2reg
    fn exp2reg(&mut self, lx: &Lexer, e: &mut ExpDesc, reg: i32) -> R<()> {
        self.discharge2reg(lx, e, reg)?;
        if e.k == 10 {
            e.t = self.concat(lx, e.t, e.info)?;
        }
        if e.hasjumps() {
            let mut p_f = -1;
            let mut p_t = -1;
            if self.need_value(e.t) || self.need_value(e.f) {
                let fj = if e.k == 10 { -1 } else { self.jump(lx)? };
                p_f = self.code_label(lx, reg, 0, 1)?;
                p_t = self.code_label(lx, reg, 1, 0)?;
                self.patchtohere(lx, fj)?;
            }
            let final_ = self.getlabel();
            self.patchlistaux(lx, e.f, final_, reg, p_f)?;
            self.patchlistaux(lx, e.t, final_, reg, p_t)?;
        }
        e.t = -1;
        e.f = -1;
        e.info = reg;
        e.k = 12;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.exp2nextreg
    pub fn exp2nextreg(&mut self, lx: &Lexer, e: &mut ExpDesc) -> R<()> {
        self.dischargevars(lx, e)?;
        self.freeexp(e)?;
        self.reserveregs(lx, 1)?;
        self.exp2reg(lx, e, self.freereg - 1)
    }

    /// org.luaj.kahluafork.compiler.FuncState.exp2anyreg
    pub fn exp2anyreg(&mut self, lx: &Lexer, e: &mut ExpDesc) -> R<i32> {
        self.dischargevars(lx, e)?;
        if e.k == 12 {
            if !e.hasjumps() {
                return Ok(e.info);
            }
            if e.info >= self.nactvar {
                self.exp2reg(lx, e, e.info)?;
                return Ok(e.info);
            }
        }
        self.exp2nextreg(lx, e)?;
        Ok(e.info)
    }

    /// org.luaj.kahluafork.compiler.FuncState.exp2val
    pub fn exp2val(&mut self, lx: &Lexer, e: &mut ExpDesc) -> R<()> {
        if e.hasjumps() {
            self.exp2anyreg(lx, e)?;
            Ok(())
        } else {
            self.dischargevars(lx, e)
        }
    }

    /// org.luaj.kahluafork.compiler.FuncState.exp2RK
    pub fn exp2rk(&mut self, lx: &Lexer, e: &mut ExpDesc) -> R<i32> {
        self.exp2val(lx, e)?;
        match e.k {
            1 | 2 | 3 | 5 if self.nk <= MAXINDEXRK => {
                e.info = if e.k == 1 {
                    self.nil_k()
                } else if e.k == 5 {
                    self.number_k(e.nval())
                } else {
                    self.bool_k(e.k == 2)
                };
                e.k = 4;
                return Ok(rkask(e.info));
            }
            4 if e.info <= MAXINDEXRK => return Ok(rkask(e.info)),
            _ => {}
        }
        self.exp2anyreg(lx, e)
    }

    /// org.luaj.kahluafork.compiler.FuncState.storevar
    pub fn storevar(&mut self, lx: &Lexer, var: &ExpDesc, ex: &mut ExpDesc) -> R<()> {
        match var.k {
            6 => {
                self.freeexp(ex)?;
                return self.exp2reg(lx, ex, var.info);
            }
            7 => {
                let e = self.exp2anyreg(lx, ex)?;
                self.code_abc(lx, OP_SETUPVAL, e, var.info, 0)?;
            }
            8 => {
                let e = self.exp2anyreg(lx, ex)?;
                self.code_abx(lx, OP_SETGLOBAL, e, var.info)?;
            }
            9 => {
                let e = self.exp2rk(lx, ex)?;
                self.code_abc(lx, OP_SETTABLE, var.info, var.aux, e)?;
            }
            _ => assert_(false)?,
        }
        self.freeexp(ex)
    }

    /// org.luaj.kahluafork.compiler.FuncState.self
    pub fn self_(&mut self, lx: &Lexer, e: &mut ExpDesc, key: &mut ExpDesc) -> R<()> {
        self.exp2anyreg(lx, e)?;
        self.freeexp(e)?;
        let func = self.freereg;
        self.reserveregs(lx, 2)?;
        let e_info = e.info;
        let rk = self.exp2rk(lx, key)?;
        self.code_abc(lx, OP_SELF, func, e_info, rk)?;
        self.freeexp(key)?;
        e.info = func;
        e.k = 12;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.invertjump
    fn invertjump(&mut self, e: &ExpDesc) -> R<()> {
        let at = self.getjumpcontrol(e.info);
        let i = self.code_at(at);
        let op = get_opcode(i);
        assert_(test_t_mode(op) && op != OP_TESTSET && op != OP_TEST)?;
        let nota = if getarg_a(i) != 0 { 0 } else { 1 };
        self.set_code_at(at, setarg_a(i, nota));
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.jumponcond
    fn jumponcond(&mut self, lx: &Lexer, e: &mut ExpDesc, cond: i32) -> R<i32> {
        if e.k == 11 {
            let ie = self.getcode(e);
            if get_opcode(ie) == OP_NOT {
                self.pc -= 1;
                return self.condjump(lx, OP_TEST, getarg_b(ie), 0, if cond != 0 { 0 } else { 1 });
            }
        }
        self.discharge2anyreg(lx, e)?;
        self.freeexp(e)?;
        self.condjump(lx, OP_TESTSET, NO_REG, e.info, cond)
    }

    /// org.luaj.kahluafork.compiler.FuncState.goiftrue
    pub fn goiftrue(&mut self, lx: &Lexer, e: &mut ExpDesc) -> R<()> {
        self.dischargevars(lx, e)?;
        let old_f = e.f;
        let pc = match e.k {
            2 | 4 | 5 => -1,
            3 => self.jump(lx)?,
            10 => {
                self.invertjump(e)?;
                e.info
            }
            _ => self.jumponcond(lx, e, 0)?,
        };
        e.f = self.concat(lx, old_f, pc)?;
        self.patchtohere(lx, e.t)?;
        e.t = -1;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.goiffalse
    fn goiffalse(&mut self, lx: &Lexer, e: &mut ExpDesc) -> R<()> {
        self.dischargevars(lx, e)?;
        let old_t = e.t;
        let pc = match e.k {
            1 | 3 => -1,
            2 => self.jump(lx)?,
            10 => e.info,
            _ => self.jumponcond(lx, e, 1)?,
        };
        e.t = self.concat(lx, old_t, pc)?;
        self.patchtohere(lx, e.f)?;
        e.f = -1;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.codenot
    fn codenot(&mut self, lx: &Lexer, e: &mut ExpDesc) -> R<()> {
        self.dischargevars(lx, e)?;
        match e.k {
            1 | 3 => e.k = 2,
            2 | 4 | 5 => e.k = 3,
            10 => self.invertjump(e)?,
            11 | 12 => {
                self.discharge2anyreg(lx, e)?;
                self.freeexp(e)?;
                e.info = self.code_abc(lx, OP_NOT, 0, e.info, 0)?;
                e.k = 11;
            }
            _ => assert_(false)?,
        }
        std::mem::swap(&mut e.f, &mut e.t);
        self.removevalues(e.f);
        self.removevalues(e.t);
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.indexed
    pub fn indexed(&mut self, lx: &Lexer, t: &mut ExpDesc, k: &mut ExpDesc) -> R<()> {
        t.aux = self.exp2rk(lx, k)?;
        t.k = 9;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.constfolding: Java double arithmetic (`%` is the truncating remainder,
    /// not Lua's); a NaN or infinite result is not folded.
    fn constfolding(op: i32, e1: &mut ExpDesc, e2: &ExpDesc) -> R<bool> {
        if !(e1.isnumeral() && e2.isnumeral()) {
            return Ok(false);
        }
        let v1 = e1.nval();
        let v2 = e2.nval();
        let r = match op {
            OP_ADD => v1 + v2,
            OP_SUB => v1 - v2,
            OP_MUL => v1 * v2,
            OP_DIV => v1 / v2,
            OP_MOD => v1 % v2,
            OP_POW | OP_LEN => return Ok(false),
            OP_UNM => -v1,
            _ => {
                assert_(false)?;
                return Ok(false);
            }
        };
        if r.is_finite() {
            e1.set_nval(r);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// org.luaj.kahluafork.compiler.FuncState.codearith
    fn codearith(&mut self, lx: &Lexer, op: i32, e1: &mut ExpDesc, e2: &mut ExpDesc) -> R<()> {
        if Self::constfolding(op, e1, e2)? {
            return Ok(());
        }
        let o2 = if op != OP_UNM && op != OP_LEN {
            self.exp2rk(lx, e2)?
        } else {
            0
        };
        let o1 = self.exp2rk(lx, e1)?;
        if o1 > o2 {
            self.freeexp(e1)?;
            self.freeexp(e2)?;
        } else {
            self.freeexp(e2)?;
            self.freeexp(e1)?;
        }
        e1.info = self.code_abc(lx, op, 0, o1, o2)?;
        e1.k = 11;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.codecomp
    fn codecomp(
        &mut self,
        lx: &Lexer,
        op: i32,
        mut cond: i32,
        e1: &mut ExpDesc,
        e2: &mut ExpDesc,
    ) -> R<()> {
        let mut o1 = self.exp2rk(lx, e1)?;
        let mut o2 = self.exp2rk(lx, e2)?;
        self.freeexp(e2)?;
        self.freeexp(e1)?;
        if cond == 0 && op != OP_EQ {
            std::mem::swap(&mut o1, &mut o2);
            cond = 1;
        }
        e1.info = self.condjump(lx, op, cond, o1, o2)?;
        e1.k = 10;
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.prefix
    pub fn prefix(&mut self, lx: &Lexer, op: i32, e: &mut ExpDesc) -> R<()> {
        let mut e2 = ExpDesc::default();
        e2.init(5, 0);
        match op {
            0 => {
                if e.k == 4 {
                    self.exp2anyreg(lx, e)?;
                }
                self.codearith(lx, OP_UNM, e, &mut e2)
            }
            1 => self.codenot(lx, e),
            2 => {
                self.exp2anyreg(lx, e)?;
                self.codearith(lx, OP_LEN, e, &mut e2)
            }
            _ => assert_(false),
        }
    }

    /// org.luaj.kahluafork.compiler.FuncState.infix
    pub fn infix(&mut self, lx: &Lexer, op: i32, v: &mut ExpDesc) -> R<()> {
        match op {
            0..=5 => {
                if !v.isnumeral() {
                    self.exp2rk(lx, v)?;
                }
            }
            6 => self.exp2nextreg(lx, v)?,
            13 => self.goiftrue(lx, v)?,
            14 => self.goiffalse(lx, v)?,
            _ => {
                self.exp2rk(lx, v)?;
            }
        }
        Ok(())
    }

    /// org.luaj.kahluafork.compiler.FuncState.posfix
    pub fn posfix(&mut self, lx: &Lexer, op: i32, e1: &mut ExpDesc, e2: &mut ExpDesc) -> R<()> {
        match op {
            0 => self.codearith(lx, OP_ADD, e1, e2),
            1 => self.codearith(lx, OP_SUB, e1, e2),
            2 => self.codearith(lx, OP_MUL, e1, e2),
            3 => self.codearith(lx, OP_DIV, e1, e2),
            4 => self.codearith(lx, OP_MOD, e1, e2),
            5 => self.codearith(lx, OP_POW, e1, e2),
            6 => {
                self.exp2val(lx, e2)?;
                if e2.k == 11 && get_opcode(self.getcode(e2)) == OP_CONCAT {
                    assert_(e1.info == getarg_b(self.getcode(e2)) - 1)?;
                    self.freeexp(e1)?;
                    let i = self.getcode(e2);
                    self.set_code(e2, setarg_b(i, e1.info));
                    e1.k = 11;
                    e1.info = e2.info;
                    Ok(())
                } else {
                    self.exp2nextreg(lx, e2)?;
                    self.codearith(lx, OP_CONCAT, e1, e2)
                }
            }
            7 => self.codecomp(lx, OP_EQ, 0, e1, e2),
            8 => self.codecomp(lx, OP_EQ, 1, e1, e2),
            9 => self.codecomp(lx, OP_LT, 1, e1, e2),
            10 => self.codecomp(lx, OP_LE, 1, e1, e2),
            11 => self.codecomp(lx, OP_LT, 0, e1, e2),
            12 => self.codecomp(lx, OP_LE, 0, e1, e2),
            13 => {
                assert_(e1.t == -1)?;
                self.dischargevars(lx, e2)?;
                e2.f = self.concat(lx, e2.f, e1.f)?;
                e1.setvalue(e2);
                Ok(())
            }
            14 => {
                assert_(e1.f == -1)?;
                self.dischargevars(lx, e2)?;
                e2.t = self.concat(lx, e2.t, e1.t)?;
                e1.setvalue(e2);
                Ok(())
            }
            _ => assert_(false),
        }
    }

    /// org.luaj.kahluafork.compiler.FuncState.fixline
    pub fn fixline(&mut self, line: i32) {
        self.f.lines[(self.pc - 1) as usize] = line;
    }

    /// org.luaj.kahluafork.compiler.FuncState.code: the jumps to here are patched first; every call stamps the
    /// prototype with the file statics (`FuncState.currentFile` / `currentfullFile`).
    pub fn code(&mut self, lx: &Lexer, instruction: i32, line: i32) -> R<i32> {
        self.dischargejpc(lx)?;
        let pc = self.pc as usize;
        if pc < self.f.code.len() {
            self.f.code[pc] = instruction;
        } else {
            self.f.code.push(instruction);
        }
        if pc < self.f.lines.len() {
            self.f.lines[pc] = line;
        } else {
            self.f.lines.push(line);
        }
        self.f.file = lx.current_file.clone();
        self.f.filename = lx.currentfull_file.clone();
        self.pc += 1;
        Ok(self.pc - 1)
    }

    /// org.luaj.kahluafork.compiler.FuncState.codeABC
    pub fn code_abc(&mut self, lx: &Lexer, o: i32, a: i32, b: i32, c: i32) -> R<i32> {
        assert_(get_op_mode(o) == 0)?;
        assert_(get_b_mode(o) != 0 || b == 0)?;
        assert_(get_c_mode(o) != 0 || c == 0)?;
        self.code(lx, create_abc(o, a, b, c), lx.lastline)
    }

    /// org.luaj.kahluafork.compiler.FuncState.codeABx
    pub fn code_abx(&mut self, lx: &Lexer, o: i32, a: i32, bc: i32) -> R<i32> {
        assert_(get_op_mode(o) == 1 || get_op_mode(o) == 2)?;
        assert_(get_c_mode(o) == 0)?;
        self.code(lx, create_abx(o, a, bc), lx.lastline)
    }

    /// org.luaj.kahluafork.compiler.FuncState.setlist
    pub fn setlist(&mut self, lx: &Lexer, base: i32, nelems: i32, tostore: i32) -> R<()> {
        let c = (nelems - 1) / LFIELDS_PER_FLUSH + 1;
        let b = if tostore == -1 { 0 } else { tostore };
        assert_(tostore != 0)?;
        if c <= MAXARG_C {
            self.code_abc(lx, OP_SETLIST, base, b, c)?;
        } else {
            self.code_abc(lx, OP_SETLIST, base, b, 0)?;
            self.code(lx, c, lx.lastline)?;
        }
        self.freereg = base + 1;
        Ok(())
    }

    /// The prototype as `LexState.close_func` leaves it: arrays cut to their used length.
    pub fn finish(&mut self) {
        let pc = self.pc as usize;
        self.f.code.truncate(pc);
        self.f.lines.truncate(pc);
        self.f.locvars = self.locvars.clone();
    }
}
