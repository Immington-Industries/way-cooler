//! Lua functionality

use rlua::Lua;
use std::sync::Mutex;

#[cfg(test)]
mod tests;

mod types;
mod thread;
mod rust_interop;
mod init_path;
mod utils;

pub struct LuaWrapper(pub Lua);

unsafe impl Send for LuaWrapper{}


lazy_static! {
    // XXX: The Mutex is not actually needed. The Lua state will be thread-local to the Lua thread.
    static ref LUA: Mutex<LuaWrapper> = Mutex::new(LuaWrapper(Lua::new()));
}

pub use self::types::{LuaQuery, LuaResponse};
pub use self::thread::{init, on_compositor_ready, running, send, update_registry_value,
                       run_with_lua, LuaSendError};
pub use self::utils::{mods_to_lua, mods_to_rust, mouse_events_to_lua};
