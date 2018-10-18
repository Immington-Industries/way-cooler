//! Lua interpreter setup and configuration reading.

mod config;
mod utils;

use self::config::load_config;
pub use self::utils::*;

use glib::MainLoop;
use rlua::{self, AnyUserData, Lua, Table, Value};

use std::cell::Cell;
use std::cell::RefCell;

use common::signal;

thread_local! {
    // NOTE The debug library does some powerful reflection that can do crazy things,
    // which is why it's unsafe to load.

    /// Global Lua state.
    pub static LUA: RefCell<Lua> = RefCell::new(unsafe { Lua::new_with_debug() });

    /// If set then we have restarted the Lua thread. We need to replace LUA when it's not borrowed.
    pub static NEXT_LUA: Cell<bool> = Cell::new(false);

    /// Main GLib loop
    static MAIN_LOOP: RefCell<MainLoop> = RefCell::new(MainLoop::new(None, false));
}

/// Sets up the Lua environment before running the compositor.
pub fn setup_lua() {
    LUA.with(|lua| {
        register_libraries(&*lua.borrow()).expect("Could not register lua libraries");
        info!("Initializing lua...");
        load_config(&mut *lua.borrow_mut());
    });
}

fn emit_refresh(lua: &Lua) {
    if let Err(err) = signal::global_emit_signal(lua, ("refresh".to_owned(), Value::Nil)) {
        error!("Internal error while emitting 'refresh' signal: {}", err);
    }
}

/// Main loop of the Lua thread:
///
/// * Initialise the Lua state
/// * Run a GMainLoop
pub fn enter_glib_loop() {
    MAIN_LOOP.with(|main_loop| main_loop.borrow().run());
}

pub fn terminate() {
    MAIN_LOOP.with(|main_loop| main_loop.borrow().quit())
}

/// Register all the Rust functions for the lua libraries
pub fn register_libraries(lua: &Lua) -> rlua::Result<()> {
    trace!("Setting up Lua libraries");
    // TODO Is this awesome init code necessary?
    let init_code = include_str!("../../../lib/lua/init.lua");
    lua.exec::<()>(init_code, Some("init.lua"))?;
    let globals = lua.globals();
    globals.set("type", lua.create_function(type_override)?)?;
    init_libs(&lua).expect("Could not initialize awesome compatibility modules");
    Ok(())
}

fn init_libs(lua: &Lua) -> rlua::Result<()> {
    use {objects::*, *};
    setup_awesome_path(lua)?;
    setup_global_signals(lua)?;
    setup_xcb_connection(lua)?;
    button::init(lua)?;
    awesome::init(lua)?;
    key::init(lua)?;
    client::init(lua)?;
    screen::init(lua)?;
    keygrabber::init(lua)?;
    root::init(lua)?;
    mouse::init(lua)?;
    tag::init(lua)?;
    drawin::init(lua)?;
    drawable::init(lua)?;
    mousegrabber::init(lua)?;
    Ok(())
}

/// This function behaves just like Lua's built-in type() function, but also
/// recognises classes and returns special names for them.
fn type_override(_lua: &Lua, arg: Value) -> rlua::Result<String> {
    // Lua's type() returns the result of lua_typename(), but rlua does not make
    // that available to us, so write our own.
    Ok(match arg {
        Value::Error(e) => return Err(e),
        Value::Nil => "nil",
        Value::Boolean(_) => "boolean",
        Value::LightUserData(_) => "userdata",
        Value::Integer(_) => "number",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Function(_) => "function",
        Value::Thread(_) => "thread",
        Value::Table(_) => "table",
        Value::UserData(o) => {
            // Handle our own objects specially: Get the object's class from its user
            // value's metatable's __class entry. Then get the class name
            // from the class's user value's metatable's name entry.
            return o
                .get_user_value::<Table>()
                .ok()
                .and_then(|table| table.get_metatable())
                .and_then(|meta| meta.raw_get::<_, AnyUserData>("__class").ok())
                .and_then(|class| class.get_user_value::<Table>().ok())
                .map(|table| table.raw_get("name"))
                .unwrap_or_else(|| Ok("userdata".into()));
        }
    }.into())
}
