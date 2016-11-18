use std::fmt::{Debug, Formatter};
use std::fmt::Result as FmtResult;

use commands::CommandFn;

// Actions which can be taken on a keypress
#[derive(Clone)]
pub enum KeyEvent {
    /// A way-cooler command is run
    Command(CommandFn),
    /// A Lua function is invoked.
    /// The String field is used as a unique identifier to the Lua thread.
    Lua
}

impl Debug for KeyEvent {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match *self {
            KeyEvent::Command(_) => write!(f, "KeyEvent::Command"),
            KeyEvent::Lua => write!(f, "KeyEvent::Lua")
        }
    }
}
