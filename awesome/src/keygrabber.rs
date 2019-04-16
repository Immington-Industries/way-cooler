//! AwesomeWM Keygrabber interface

use rlua::{self, Function, Lua, Table, Value};
use wlroots::{events::key_events::Key, wlr_key_state, xkbcommon::xkb::keysym_get_name, WLR_KEY_PRESSED};

use crate::common::signal;
use crate::LUA;

pub const KEYGRABBER_TABLE: &str = "keygrabber";
const KEYGRABBER_CALLBACK: &str = "__callback";

/// Init the methods defined on this interface.
pub fn init(lua: &Lua) -> rlua::Result<()> {
    let keygrabber_table = lua.create_table()?;
    let meta = lua.create_table()?;
    meta.set("__index", lua.create_function(index)?)?;
    meta.set("__newindex", lua.create_function(new_index)?)?;
    keygrabber_table.set("run", lua.create_function(run)?)?;
    keygrabber_table.set("stop", lua.create_function(stop)?)?;
    keygrabber_table.set("isrunning", lua.create_function(isrunning)?)?;
    keygrabber_table.set_metatable(Some(meta));
    let globals = lua.globals();
    globals.set(KEYGRABBER_TABLE, keygrabber_table)
}

/// Given the current input, handle calling the Lua defined callback if it is
/// defined with the input.
#[allow(dead_code)]
pub fn keygrabber_handle(mods: Vec<Key>, sym: Key, state: wlr_key_state) -> rlua::Result<()> {
    LUA.with(|lua| {
        let lua = lua.borrow();
        let lua_state = if state == WLR_KEY_PRESSED {
            "press"
        } else {
            "release"
        }
        .into();
        let lua_sym = keysym_get_name(sym.keycode());
        let mods : Vec<u32> = mods.iter().map(|k| k.keycode()).collect();
        let lua_mods = crate::lua::mods_to_lua(lua, &mods)?;
        let res = call_keygrabber(lua, (lua_mods, lua_sym, lua_state));
        match res {
            Ok(_) | Err(rlua::Error::FromLuaConversionError { .. }) => Ok(()),
            err => err
        }
    })
}

/// Check is the Lua callback function is set
#[allow(dead_code)]
pub fn is_keygrabber_set(lua: &Lua) -> bool {
    lua.named_registry_value::<Function>(KEYGRABBER_CALLBACK).is_ok()
}

/// Call the Lua callback function for when a key is pressed.
#[allow(dead_code)]
pub fn call_keygrabber(lua: &Lua, (mods, key, event): (Table, String, String)) -> rlua::Result<()> {
    let lua_callback = lua.named_registry_value::<Function>(KEYGRABBER_CALLBACK)?;
    lua_callback.call((mods, key, event))
}

fn run(lua: &Lua, function: Function) -> rlua::Result<()> {
    match lua.named_registry_value::<Value>(KEYGRABBER_CALLBACK)? {
        Value::Function(_) => Err(rlua::Error::RuntimeError(
            "keygrabber callback already set!".into()
        )),
        _ => lua.set_named_registry_value(KEYGRABBER_CALLBACK, function)
    }
}

fn stop(lua: &Lua, _: ()) -> rlua::Result<()> {
    lua.set_named_registry_value(KEYGRABBER_CALLBACK, Value::Nil)
}

fn isrunning(lua: &Lua, _: ()) -> rlua::Result<bool> {
    match lua.named_registry_value::<Value>(KEYGRABBER_CALLBACK)? {
        Value::Function(_) => Ok(true),
        _ => Ok(false)
    }
}

fn index(lua: &Lua, args: Value) -> rlua::Result<()> {
    signal::global_emit_signal(lua, ("debug::index::miss".into(), args))
}

fn new_index(lua: &Lua, args: Value) -> rlua::Result<()> {
    signal::global_emit_signal(lua, ("debug::newindex::miss".into(), args))
}
