//! TODO Fill in

use rustwlc::{Geometry, Point, Size, WlcOutput};
use std::fmt::{self, Display, Formatter};
use std::default::Default;
use std::rc::Rc;
use rlua::{self, Table, Lua, UserData, ToLua, Value};
use super::object::{Object, Objectable};
use super::property::Property;
use super::class::{self, Class, ClassBuilder};

pub const SCREENS_HANDLE: &'static str = "__screens";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    pub name: String,
    pub mm_width: u32,
    pub mm_height: u32,
    // TODO The XID array?
}

impl From<WlcOutput> for Output {
    fn from(output: WlcOutput) -> Output {
        let resolution = output.get_resolution().unwrap();
        Output {
            name: output.get_name(),
            mm_width: resolution.w,
            mm_height: resolution.h
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenState {
    // Is this screen still valid and may be used
    pub valid: bool,
    // Screen geometry
    pub geometry: Geometry,
    // Screen workarea
    pub workarea: Geometry,
    // The screen outputs information
    pub outputs: Vec<Output>,
    // Some XID indetifying this screen
    pub xid: u32
}

pub struct Screen<'lua>(Table<'lua>);

impl_objectable!(Screen, ScreenState);

impl Display for ScreenState {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Screen: {:p}", self)
    }
}

impl <'lua> ToLua<'lua> for Screen<'lua> {
    fn to_lua(self, lua: &'lua Lua) -> rlua::Result<Value<'lua>> {
        self.0.to_lua(lua)
    }
}

impl UserData for ScreenState {}

impl Default for ScreenState {
    fn default() -> Self {
        ScreenState {
            valid: true,
            geometry: Geometry::zero(),
            workarea: Geometry::zero(),
            outputs: vec![],
            xid: 0
        }
    }
}

impl <'lua> Screen<'lua> {
    fn new(lua: &Lua) -> rlua::Result<Object> {
        let class = class::class_setup(lua, "screen")?;
        Ok(Screen::allocate(lua, class)?.build())
    }

    fn init_screens(&mut self, outputs: Vec<Output>) -> rlua::Result<()> {
        let mut state = self.state()?;
        state.outputs = outputs;
        self.set_state(state)
    }

    fn get_geometry(&self, lua: &'lua Lua) -> rlua::Result<Table<'lua>> {
        let state = self.state()?;
        let Point { x, y } = state.geometry.origin;
        let Size { w, h } = state.geometry.size;
        // TODO I do this a lot, put it somewhere
        let table = lua.create_table();
        table.set("x", x)?;
        table.set("y", y)?;
        table.set("width", w)?;
        table.set("height", h)?;
        Ok(table)
    }

    fn get_workarea(&self, lua: &'lua Lua) -> rlua::Result<Table<'lua>> {
        let state = self.state()?;
        let Point { x, y } = state.workarea.origin;
        let Size { w, h } = state.workarea.size;
        // TODO I do this a lot, put it somewhere
        let table = lua.create_table();
        table.set("x", x)?;
        table.set("y", y)?;
        table.set("width", w)?;
        table.set("height", h)?;
        Ok(table)
    }
}

pub fn init(lua: &Lua) -> rlua::Result<Class> {
    let res = property_setup(lua, method_setup(lua, Class::builder(lua, "screen", Some(Rc::new(Screen::new)), None, None)?)?)?
        .save_class("screen")?
        .build()?;
    let mut screens: Vec<Screen> = vec![];
    for output in WlcOutput::list() {
        let mut screen = Screen::cast(Screen::new(lua)?)?;
        screen.init_screens(vec![output.into()])?;
        // TODO Move to Screen impl like the others
        screens.push(screen);
    }
    // TODO Uncomment
    // This breaks rc.lua because of layoutbox stuff.
    // Please fix that when you uncomment this.
    lua.globals().set(SCREENS_HANDLE, lua.create_table()/*screens.to_lua(lua)?*/)?;
    Ok(res)
}

fn method_setup<'lua>(lua: &'lua Lua, builder: ClassBuilder<'lua>) -> rlua::Result<ClassBuilder<'lua>> {
    // TODO Do properly
    use super::dummy;
    builder.method("connect_signal".into(), lua.create_function(dummy))?
           .method("__index".into(), lua.create_function(index))?
           .method("__call".into(), lua.create_function(iterate_over_screens))
}

fn property_setup<'lua>(lua: &'lua Lua, builder: ClassBuilder<'lua>) -> rlua::Result<ClassBuilder<'lua>> {
    builder
        .property(Property::new("geometry".into(),
                                None,
                                Some(lua.create_function(get_geometry)),
                                None))?
        .property(Property::new("workarea".into(),
                                None,
                                Some(lua.create_function(get_workarea)),
                                None))
}

fn get_geometry<'lua>(lua: &'lua Lua, table: Table<'lua>) -> rlua::Result<Table<'lua>> {
    let screen = Screen::cast(table.into())?;
    screen.get_geometry(lua)
}

fn get_workarea<'lua>(lua: &'lua Lua, table: Table<'lua>) -> rlua::Result<Table<'lua>> {
    let screen = Screen::cast(table.into())?;
    screen.get_workarea(lua)
}

/// Ok this requires some explanation...
/// Lua gives us the previous value in the loop, with the first one being nil
/// since there was nothing there before.
///
/// To ensure we loop over everything, we take the index of that value in our list,
/// increment it by 1 (starting at 0 if it's the start) and then once it falls outside
/// the bounds it will stop by returning nil.
fn iterate_over_screens<'lua>(lua: &'lua Lua,
                              (_, _, prev): (Value<'lua>, Value<'lua>, Value<'lua>))
                              -> rlua::Result<Value<'lua>> {
    let screens: Vec<Screen> = lua.globals().get::<_, Vec<Table>>(SCREENS_HANDLE)?
        .into_iter().map(|t| Screen::cast(t.into()).unwrap())
        .collect();
    let index = match prev {
        Value::Nil => 0,
        Value::Table(ref table) => {
            if let Ok(screen) = Screen::cast(table.clone().into()) {
                screens.iter().position(|t| t.state().unwrap() == screen.state().unwrap())
                    .unwrap_or(screens.len()) + 1
            } else {
                panic!("Unexpected non-screen table in loop");
            }
        }
        _ => panic!("Unexpected non-screen or nil value in screens loop")
    };
    if index < screens.len() {
        screens[index].get_table().to_lua(lua)
    } else {
        Ok(Value::Nil)
    }

}

fn index<'lua>(lua: &'lua Lua,
               (obj_table, index): (Table<'lua>, Value<'lua>))
               -> rlua::Result<Value<'lua>> {
    let screens: Vec<Screen> = lua.globals().get::<_, Vec<Table>>(SCREENS_HANDLE)?
        .into_iter().map(|t| Screen::cast(t.into()).unwrap())
        .collect();
    match index {
        Value::String(ref string) => {
            let string = string.to_str()?;
            if string == "primary" {
                // TODO Emit primary changed signal
                if screens.len() > 0 {
                    return screens[0].get_table().clone().to_lua(lua)
                }
            }
            for screen in screens.iter() {
                let screen_state = screen.state()?;
                for output in &screen_state.outputs {
                    if output.name.as_str() == string {
                        return screen.get_table().clone().to_lua(lua)
                    }
                }
            }
        },
        // TODO Might need to do Number instead
        Value::Integer(screen_index) => {
            if screen_index < 1 || screen_index as usize > screens.len() {
                return Err(rlua::Error::RuntimeError(
                    format!("invalid screen number: {} (of {} existing)",
                            screen_index, screens.len())))
            }
            return screens[screen_index as usize].get_table().clone().to_lua(lua).clone()
        },
        Value::Table(ref table) => {
            // If this is a screen, just return it
            if let Ok(screen) = Screen::cast(table.clone().into()) {
                return screen.to_lua(lua)
            }
        },
        // TODO This checke user data like in luaA_toudata in awesome
        _ => {}
    }
    // TODO checkudata
    let meta = obj_table.get_metatable().unwrap();
    meta.get(index.clone()).or_else(|_| super::object::default_index(lua, (obj_table, index)))
}
