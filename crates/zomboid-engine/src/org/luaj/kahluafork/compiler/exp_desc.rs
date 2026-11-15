//! org.luaj.kahluafork.compiler.ExpDesc: the description of an expression while it is being compiled.

/// org.luaj.kahluafork.compiler.ExpDesc. A new one is all zeros (jump lists 0, not NO_JUMP); `init` leaves `aux` and
/// the numeric value alone, as the original does.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ExpDesc {
    pub k: i32,
    pub info: i32,
    pub aux: i32,
    nval: f64,
    has_nval: bool,
    pub t: i32,
    pub f: i32,
}

impl ExpDesc {
    /// org.luaj.kahluafork.compiler.ExpDesc.setNval
    pub fn set_nval(&mut self, r: f64) {
        self.nval = r;
        self.has_nval = true;
    }

    /// org.luaj.kahluafork.compiler.ExpDesc.nval: the number set, else `info` as a double.
    pub fn nval(&self) -> f64 {
        if self.has_nval {
            self.nval
        } else {
            f64::from(self.info)
        }
    }

    /// org.luaj.kahluafork.compiler.ExpDesc.init
    pub fn init(&mut self, k: i32, i: i32) {
        self.f = -1;
        self.t = -1;
        self.k = k;
        self.info = i;
    }

    /// org.luaj.kahluafork.compiler.ExpDesc.hasjumps
    pub fn hasjumps(&self) -> bool {
        self.t != self.f
    }

    /// org.luaj.kahluafork.compiler.ExpDesc.isnumeral
    pub fn isnumeral(&self) -> bool {
        self.k == 5 && self.t == -1 && self.f == -1
    }

    /// org.luaj.kahluafork.compiler.ExpDesc.setvalue: every field copied.
    pub fn setvalue(&mut self, other: &ExpDesc) {
        *self = *other;
    }
}
