//! Types defined by Lua thread

use std::fmt::{Debug, Formatter};
use std::fmt::Result as FmtResult;
use std::cmp::{PartialEq, Eq};

use hlua;
use hlua::Lua;
use hlua::any::AnyLuaValue;

use keys::KeyPress;

/// Methods that the Lua thread can execute.
pub type LuaFunc = fn(&mut Lua) -> AnyLuaValue;

/// Messages sent to the lua thread
#[allow(dead_code)]
pub enum LuaQuery {
    /// Pings the lua thread
    Ping,
    /// Halt the lua thread
    Terminate,
    // Restart the lua thread
    Restart,

    /// Execute a string
    Execute(String),
    /// Execute a file
    ExecFile(String),
    /// Execute some Rust using the Lua context.
    ExecRust(LuaFunc),

    /// Handle the key press for the given key.
    HandleKey(KeyPress),
}

impl Debug for LuaQuery {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match *self {
            LuaQuery::Ping => write!(f, "LuaQuery::Ping"),
            LuaQuery::Terminate => write!(f, "LuaQuery::Terminate"),
            LuaQuery::Restart => write!(f, "LuaQuery::Restart"),
            LuaQuery::Execute(ref val) =>
                write!(f, "LuaQuery::Execute({:?})", val),
            LuaQuery::ExecFile(ref val) =>
                write!(f, "LuaQuery::ExecFile({:?})", val),
            // This is why there's no #[derive(Debug)],
            // and why we have lua/types.rs
            LuaQuery::ExecRust(_) =>
                write!(f, "LuaQuery::ExecRust()"),
            LuaQuery::HandleKey(ref press) =>
                write!(f, "LuaQuery::HandleKey({:?})", press),
        }
    }
}

unsafe impl Send for LuaQuery { }
unsafe impl Sync for LuaQuery { }

impl PartialEq for LuaQuery {
    fn eq(&self, other: &LuaQuery) -> bool {
        match (self, other) {
            (&LuaQuery::Ping, &LuaQuery::Ping) => true,
            (&LuaQuery::Terminate, &LuaQuery::Terminate) => true,
            (&LuaQuery::Restart, &LuaQuery::Restart) => true,

            (&LuaQuery::Execute(ref s1), &LuaQuery::Execute(ref s2)) =>
                s1 == s2,
            (&LuaQuery::ExecFile(ref s1), &LuaQuery::ExecFile(ref s2)) =>
                s1 == s2,
            (&LuaQuery::ExecRust(_), &LuaQuery::ExecRust(_)) => true,
            (&LuaQuery::HandleKey(ref p1), &LuaQuery::HandleKey(ref p2)) =>
                p1 == p2,
            _ => false
        }
    }
}

impl Eq for LuaQuery { }

/// Messages received from lua thread
#[allow(dead_code)]
pub enum LuaResponse {
    /// If the identifier had length 0
    InvalidName,
    /// Lua variable obtained
    Variable(Option<AnyLuaValue>),
    /// Lua error
    Error(hlua::LuaError),
    /// A function is returned
    Function(hlua::functions_read::LuaFunction<String>),
    /// Pong response from lua ping
    Pong,
}

#[allow(dead_code)]
impl LuaResponse {
    /// Whether this response is an InvalidName or Error
    pub fn is_err(&self) -> bool {
        match self {
            &LuaResponse::InvalidName | &LuaResponse::Error(_) => true,
            _ => false
        }
    }

    /// If this response is a Variable, Function, or Pong
    #[inline]
    pub fn is_ok(&self) -> bool {
        !self.is_err()
    }
}

impl PartialEq for LuaResponse {
    fn eq(&self, other: &LuaResponse) -> bool {
        match (self, other) {
            (&LuaResponse::InvalidName, &LuaResponse::InvalidName) => true,
            (&LuaResponse::Pong, &LuaResponse::Pong) => true,

            (&LuaResponse::Variable(ref v1), &LuaResponse::Variable(ref v2)) =>
                v1 == v2,
            (&LuaResponse::Error(ref e1), &LuaResponse::Error(ref e2)) =>
                format!("{:?}", e1) == format!("{:?}", e2),
            (&LuaResponse::Function(_), &LuaResponse::Function(_)) => true,

            _ => false
        }
    }
}

unsafe impl Send for LuaResponse { }
unsafe impl Sync for LuaResponse { }

impl Debug for LuaResponse {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match *self {
            LuaResponse::InvalidName =>
                write!(f, "LuaReponse::InvalidName"),
            LuaResponse::Variable(ref var) =>
                write!(f, "LuaResponse::Variable({:?})", var),
            LuaResponse::Error(ref err) =>
                write!(f, "LuaResponse::Error({:?})", err),
            LuaResponse::Function(_) =>
                write!(f, "LuaResponse::Function"),
            LuaResponse::Pong =>
                write!(f, "LuaResponse::Pong")
        }
    }
}
