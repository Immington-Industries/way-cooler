//! AwesomeWM Mousegrabber interface

use ::lua::run_with_lua;
use rlua::{self, Lua, Table, Function, Value};
use rustwlc::ButtonState;

pub const MOUSEGRABBER_TABLE: &str = "mousegrabber";
const SECRET_CALLBACK: &str = "__callback";
const SECRET_CURSOR: &str = "__cursor";


/// Init the methods defined on this interface
pub fn init(lua: &Lua) -> rlua::Result<()> {
    let mousegrabber_table = lua.create_table();
    mousegrabber_table.set("run", lua.create_function(run))?;
    mousegrabber_table.set("stop", lua.create_function(stop))?;
    mousegrabber_table.set("isrunning", lua.create_function(isrunning))?;
    let globals = lua.globals();
    globals.set(MOUSEGRABBER_TABLE, mousegrabber_table)
}

pub fn mousegrabber_handle(x: i32, y: i32, button: Option<(u32, ButtonState)>)
                         -> rlua::Result<()> {
    run_with_lua(move |lua| {
        let button_events = button.map(|(button, button_state)|
                                       ::lua::mouse_events_to_lua(lua, button, button_state))
            .unwrap_or_else(|| Ok(vec![false, false, false, false, false]))?;
        call_mousegrabber(lua, (x, y, button_events))
    })
}

fn call_mousegrabber(lua: &Lua,
                     (x, y, button_events):
                     (i32, i32, Vec<bool>)) -> rlua::Result<()> {
    let globals = lua.globals();
    let lua_callback = match globals
        .get::<_, Table>(MOUSEGRABBER_TABLE)?
        .get::<_, Function>(SECRET_CALLBACK) {
        Ok(function) => function,
        _ => return Ok(())
    };
    let res_table = lua.create_table();
    res_table.set("x", x)?;
    res_table.set("y", y)?;
    res_table.set("buttons", button_events)?;
    match lua_callback.call(res_table)? {
        Value::Boolean(true) => {Ok(())},
        _ => stop(lua, ())
    }
}


fn run(lua: &Lua, (function, cursor): (Function, String)) -> rlua::Result<()> {
    let mousegrabber_table = lua.globals().get::<_, Table>(MOUSEGRABBER_TABLE)?;
    match mousegrabber_table.get::<_, Value>(SECRET_CALLBACK)? {
        Value::Function(_) =>
            Err(rlua::Error::RuntimeError("mousegrabber callback already set!"
                                          .into())),
        _ => {
            mousegrabber_table.set(SECRET_CALLBACK, function)?;
            mousegrabber_table.set(SECRET_CURSOR, cursor)
        }
    }
}

fn stop(lua: &Lua, _: ()) -> rlua::Result<()> {
    let mousegrabber_table = lua.globals().get::<_, Table>(MOUSEGRABBER_TABLE)?;
    mousegrabber_table.set(SECRET_CALLBACK, Value::Nil)
}

fn isrunning(lua: &Lua, _: ()) -> rlua::Result<bool> {
    let mousegrabber_table = lua.globals().get::<_, Table>(MOUSEGRABBER_TABLE)?;
    match mousegrabber_table.get::<_, Value>(SECRET_CALLBACK)? {
        Value::Function(_) => Ok(true),
        _ => Ok(false)
    }
}
