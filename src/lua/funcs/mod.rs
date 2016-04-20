//! Rust functions for lua libraries

use hlua;
use hlua::{Lua, LuaTable, Function};

// Prevent functions declared here from not being registered
#[forbid(dead_code)]
#[forbid(unused_variables)]

mod input;
mod lua_registry;

/// Register all the Rust functions for the lua libraries
pub fn register_libraries(lua: &mut Lua) {
    // Yeah, need to access individual tables

    {
        let mut wm: LuaTable<_> = lua.get("wm").unwrap();
        let mut pointer: LuaTable<_> = wm.get("pointer").unwrap();

        pointer.set("get_position", hlua::function0(input::pointer_get_position));
        pointer.set("set_position", hlua::function2(input::pointer_set_position));
    }

    // Initialize registry at the end?
    init_registry(lua);
}

#[inline] // It's called once
fn init_registry(lua: &mut Lua) {
    let mut reg_table: LuaTable<_> = lua.get("registry").unwrap();
    let mut meta_reg = reg_table.get_or_create_metatable();
    meta_reg.set("__metatable", "Turtles all the way down");
    meta_reg.set("__index", hlua::function2(lua_registry::index));
    meta_reg.set("__newindex", hlua::function3(lua_registry::new_index));
}
