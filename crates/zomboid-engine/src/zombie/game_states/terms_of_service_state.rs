//! zombie.gameStates.TermsOfServiceState: the start-up state that lets the Lua UI (ISTermsOfServiceUI) ask for the
//! terms of service; Lua tells it through `fromLua0` whether it made its window and when to leave.
use crate::se::krka::kahlua::vm::object::Throwable;

/// zombie.gameStates.TermsOfServiceState's fields.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TermsOfServiceState {
    pub exit: bool,
    pub created: bool,
}

impl TermsOfServiceState {
    /// zombie.gameStates.TermsOfServiceState.fromLua0: "created" and "exit" set their flag; any other name is an
    /// IllegalArgumentException (a null name, the string switch's NullPointerException).
    pub fn from_lua0(&mut self, func: Option<&str>) -> Result<(), Throwable> {
        match func {
            Some("created") => self.created = true,
            Some("exit") => self.exit = true,
            Some(f) => {
                return Err(Throwable::Java {
                    class: "java.lang.IllegalArgumentException",
                    message: Some(format!("unhandled \"{f}\"")),
                })
            }
            None => {
                return Err(Throwable::Java {
                    class: "java.lang.NullPointerException",
                    message: None,
                })
            }
        }
        Ok(())
    }

    /// zombie.gameStates.TermsOfServiceState.enter, after its `triggerEvent("OnGameStateEnter", this)`: without a
    /// window made by Lua the state leaves at once.
    pub fn after_enter_event(&mut self) {
        if !self.created {
            self.exit = true;
        }
    }
}
