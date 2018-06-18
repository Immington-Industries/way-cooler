//! TODO Fill in

use rlua::{self, AnyUserData, Lua, MetaMethod, Table, ToLua, UserData, UserDataMethods, Value};
use wlroots;

use std::default::Default;
use std::fmt::{self, Display, Formatter};

use awesome::screen::{Screen, SCREENS_HANDLE};
use awesome::object::Objectable;

use compositor::Server;

const INDEX_MISS_FUNCTION: &'static str = "__index_miss_function";
const NEWINDEX_MISS_FUNCTION: &'static str = "__newindex_miss_function";

#[derive(Clone, Debug)]
pub struct MouseState {
    // TODO Fill in
    dummy: i32
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
    mouse_table.set("set_index_miss_handler",
                     lua.create_function(set_index_miss)?)?;
    mouse_table.set("set_newindex_miss_handler",
                     lua.create_function(set_newindex_miss)?)?;
    Ok(())
}

fn coords<'lua>(lua: &'lua Lua,
                (coords, _ignore_enter): (rlua::Value<'lua>, rlua::Value<'lua>))
                -> rlua::Result<Table<'lua>> {
    with_handles!([(compositor: {wlroots::compositor_handle().unwrap()})] => {
        let server: &mut Server = compositor.into();
        // TODO Update pointer as well?
        with_handles!([(cursor: {&mut server.cursor})] => {
            match coords {
                rlua::Value::Table(coords) => {
                    let (x, y): (i32, i32) = (coords.get("x")?, coords.get("y")?);
                    // TODO The ignore_enter is supposed to not send a send event to
                    // the client
                    cursor.warp(None, x as _, y as _);
                    Ok(coords)
                }
                _ => {
                    // get the coords
                    let (x, y) = cursor.coords();
                    let coords = lua.create_table()?;
                    coords.set("x", x as i32)?;
                    coords.set("y", y as i32)?;
                    // TODO It expects a table of what buttons were pressed.
                    coords.set("buttons", lua.create_table()?)?;
                    Ok(coords)
                }
            }
        }).expect("Cursor was not defined")
    }).expect("Could not lock compositor")
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

fn index<'lua>(lua: &'lua Lua,
               (mouse, index): (AnyUserData<'lua>, Value<'lua>))
               -> rlua::Result<Value<'lua>> {
    let obj_table = mouse.get_user_value::<Table>()?;
    match index {
        Value::String(ref string) => {
            let string = string.to_str()?;
            if string != "screen" {
                return obj_table.get(string)
            }

            let output = with_handles!([(compositor: {wlroots::compositor_handle().unwrap()})] => {
                let server: &mut Server = compositor.into();
                let Server { ref mut cursor,
                             ref mut layout,
                             .. } = *server;

                with_handles!([(cursor: {cursor}), (layout: {layout})] => {
                    let (lx, ly) = cursor.coords();
                    layout.output_at(lx, ly)
                }).unwrap()
            }).unwrap();

            let mut screens: Vec<Screen> = lua.named_registry_value::<Vec<AnyUserData>>(SCREENS_HANDLE)?
                .into_iter()
                .map(|obj| Screen::cast(obj.into()).unwrap())
                .collect();

            if let Some(output) = output {
                for screen in screens {
                    let state = screen.state()?;
                    if state.outputs.contains(&output) {
                        return screen.to_lua(lua);
                    }
                }
            }

            return Value::Nil.to_lua(lua);
        }
        _ => {}
    }
    return obj_table.get(index)
}
