//! Represents a mouse that the user controls.
//!
//! We can request the compositor to do anything with the muose, including
//! changing the cursor and selecting where it should be on the screen.

use std::default::Default;
use std::fmt::{self, Display, Formatter};

use rlua::{self, AnyUserData, Lua, MetaMethod, Table, ToLua, UserData, UserDataMethods, Value};

use common::object::Objectable;
use objects::screen::{Screen, SCREENS_HANDLE};

const INDEX_MISS_FUNCTION: &'static str = "__index_miss_function";
const NEWINDEX_MISS_FUNCTION: &'static str = "__newindex_miss_function";

#[derive(Clone, Debug)]
pub struct MouseState {
    // TODO Fill in
    dummy: i32,
}

impl Default for MouseState {
    fn default() -> Self {
        MouseState { dummy: 0 }
    }
}

impl Display for MouseState {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Mouse: {:p}", self)
    }
}

impl UserData for MouseState {
    fn add_methods(methods: &mut UserDataMethods<Self>) {
        methods.add_meta_function(MetaMethod::Index, index);
    }
}

pub fn init(lua: &Lua) -> rlua::Result<()> {
    let mouse_table = lua.create_table()?;
    let meta_table = lua.create_table()?;
    let mouse = lua.create_userdata(MouseState::default())?;
    method_setup(lua, &mouse_table)?;
    let globals = lua.globals();
    mouse_table.set_metatable(Some(meta_table));
    mouse.set_user_value(mouse_table)?;
    globals.set("mouse", mouse)
}

fn method_setup(lua: &Lua, mouse_table: &Table) -> rlua::Result<()> {
    mouse_table.set("coords", lua.create_function(coords)?)?;
    mouse_table.set(
        "set_index_miss_handler",
        lua.create_function(set_index_miss)?,
    )?;
    mouse_table.set(
        "set_newindex_miss_handler",
        lua.create_function(set_newindex_miss)?,
    )?;
    Ok(())
}

fn coords<'lua>(
    lua: &'lua Lua,
    (coords, _ignore_enter): (rlua::Value<'lua>, rlua::Value<'lua>),
) -> rlua::Result<Table<'lua>> {
    // TODO Get Cords
    unimplemented!()
}

fn set_index_miss(lua: &Lua, func: rlua::Function) -> rlua::Result<()> {
    let button = lua.globals().get::<_, AnyUserData>("button")?;
    let table = button.get_user_value::<Table>()?;
    table.set(INDEX_MISS_FUNCTION, func)
}

fn set_newindex_miss(lua: &Lua, func: rlua::Function) -> rlua::Result<()> {
    let button = lua.globals().get::<_, AnyUserData>("button")?;
    let table = button.get_user_value::<Table>()?;
    table.set(NEWINDEX_MISS_FUNCTION, func)
}

fn index<'lua>(
    lua: &'lua Lua,
    (mouse, index): (AnyUserData<'lua>, Value<'lua>),
) -> rlua::Result<Value<'lua>> {
    let obj_table = mouse.get_user_value::<Table>()?;
    match index {
        Value::String(ref string) => {
            let string = string.to_str()?;
            if string != "screen" {
                return obj_table.get(string);
            }

            // TODO Get output
            let output = unimplemented!();

            let mut screens: Vec<Screen> = lua
                .named_registry_value::<Vec<AnyUserData>>(SCREENS_HANDLE)?
                .into_iter()
                .map(|obj| Screen::cast(obj.into()).unwrap())
                .collect();

            if let Some(output) = output {
                for screen in &screens {
                    let state = screen.state()?;
                    if state.outputs.contains(&output) {
                        return screen.clone().to_lua(lua);
                    }
                }
            }
            if screens.len() > 0 {
                return screens[0].clone().to_lua(lua);
            }

            return Ok(Value::Nil);
        }
        _ => {}
    }
    return obj_table.get(index);
}
