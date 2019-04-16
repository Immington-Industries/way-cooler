//! A screen as reported by the compositor.
//!
//! Note that there isn't a one-to-one mapping between number of outputs,
//! screens, and the outputs as reported by Way Cooler.

use std::default::Default;

use rlua::{self, AnyUserData, Lua, MetaMethod, Table, ToLua, UserData, UserDataMethods, Value};
use wlroots::area::{Area, Origin, Size};

use crate::common::{
    class::{self, Class, ClassBuilder},
    object::{self, Object},
    property::Property,
    signal::emit_object_signal
};
use crate::wayland_obj::Output;

pub const SCREENS_HANDLE: &'static str = "__screens";

pub type Screen<'lua> = Object<'lua, ScreenState>;

#[derive(Clone)]
pub struct ScreenState {
    // Is this screen still valid and may be used
    pub valid: bool,
    // Screen geometry
    pub geometry: Area,
    // Screen workarea
    pub workarea: Area,
    // The screen outputs information
    pub outputs: Vec<Output>,
    // Some XID identifying this screen
    pub xid: u32
}

unsafe impl Send for ScreenState {}

impl PartialEq for ScreenState {
    fn eq(&self, other: &ScreenState) -> bool {
        self.valid == other.valid &&
            self.geometry == other.geometry &&
            self.workarea == other.workarea &&
            self.xid == other.xid &&
            self.outputs == other.outputs
    }
}

impl Eq for ScreenState {}

impl Default for ScreenState {
    fn default() -> Self {
        ScreenState {
            valid: true,
            geometry: Area::default(),
            workarea: Area::default(),
            outputs: vec![],
            xid: 0
        }
    }
}

impl UserData for ScreenState {
    fn add_methods(methods: &mut UserDataMethods<Self>) {
        object::default_add_methods(methods);
        methods.add_meta_function(MetaMethod::Index, index);
    }
}

impl<'lua> Screen<'lua> {
    pub fn new(lua: &Lua) -> rlua::Result<Screen> {
        let class = class::class_setup(lua, "screen")?;
        Ok(Screen::allocate(lua, class)?.build())
    }

    pub fn init_screens(&mut self, output: Output, outputs: Vec<Output>) -> rlua::Result<()> {
        let mut state = self.state_mut()?;
        let (width, height) = output.resolution();
        let resolution = Size { width, height };
        state.outputs = outputs;
        state.geometry = state.geometry.with_size(resolution);
        state.workarea = state.workarea.with_size(resolution);
        Ok(())
    }

    pub fn set_geometry(&mut self, lua: &Lua, geometry: Area) -> rlua::Result<()> {
        if self.state()?.geometry != geometry {
            let geometry = self.state()?.geometry;
            let old_area = lua.create_table()?;
            old_area.set("x", geometry.origin.x)?;
            old_area.set("y", geometry.origin.y)?;
            old_area.set("width", geometry.size.width)?;
            old_area.set("height", geometry.size.height)?;
            self.state_mut()?.geometry = geometry;
            emit_object_signal(lua, self.clone().into(), "property::geometry".into(), old_area)?;
        }
        Ok(())
    }

    pub fn set_workarea(&mut self, lua: &Lua, geometry: Area) -> rlua::Result<()> {
        if self.state()?.workarea != geometry {
            let geometry = self.state()?.geometry;
            let old_area = lua.create_table()?;
            old_area.set("x", geometry.origin.x)?;
            old_area.set("y", geometry.origin.y)?;
            old_area.set("width", geometry.size.width)?;
            old_area.set("height", geometry.size.height)?;
            self.state_mut()?.workarea = geometry;
            emit_object_signal(lua, self.clone().into(), "property::workarea".into(), old_area)?;
        }
        Ok(())
    }

    pub fn get_geometry(&self, lua: &'lua Lua) -> rlua::Result<Table<'lua>> {
        let state = self.state()?;
        let Origin { x, y } = state.geometry.origin;
        let Size { width, height } = state.geometry.size;
        let table = lua.create_table()?;
        table.set("x", x)?;
        table.set("y", y)?;
        table.set("width", width)?;
        table.set("height", height)?;
        Ok(table)
    }

    pub fn get_workarea(&self, lua: &'lua Lua) -> rlua::Result<Table<'lua>> {
        let state = self.state()?;
        let Origin { x, y } = state.workarea.origin;
        let Size { width, height } = state.workarea.size;
        let table = lua.create_table()?;
        table.set("x", x)?;
        table.set("y", y)?;
        table.set("width", width)?;
        table.set("height", height)?;
        Ok(table)
    }
}

/// Adds a screen to the list of screens.
pub fn add_screen(lua: &Lua, screen: Screen) -> rlua::Result<()> {
    let mut screens = lua.named_registry_value::<Vec<AnyUserData>>(SCREENS_HANDLE)?;
    screens.push(screen.into());
    lua.set_named_registry_value(SCREENS_HANDLE, screens.to_lua(lua)?)?;
    Ok(())
}

/// Find a screen based on the output.
pub fn get_screen(lua: &Lua, output: Output) -> rlua::Result<Screen> {
    lua.named_registry_value::<Vec<AnyUserData>>(SCREENS_HANDLE)?
        .into_iter()
        .map(|obj| Screen::cast(obj.into()).unwrap())
        // TODO Can there be multiple screens the output is associated with?
        // Will need to handle that case
        .find(|screen| {
            let state = screen.state().expect("Could not get screen state");
            state.outputs.iter().any(|cur_output| *cur_output == output)
        })
        .ok_or(rlua::Error::RuntimeError(format!("No screen with output {:?}", output)))
}

pub fn init<'lua>(lua: &'lua Lua) -> rlua::Result<Class<ScreenState>> {
    let builder = Class::builder(lua, "screen", None)?;
    let res = property_setup(lua, method_setup(lua, builder)?)?
        .save_class("screen")?
        .build()?;
    let screens: &mut Vec<Screen> = &mut vec![];
    // TODO Get the list of outputs from Way Cooler
    // for output in server.outputs.iter() {
    //    let mut screen = Screen::new(lua)?;
    //    screen.init_screens(output.clone(), vec![output.clone()])?;
    //    // TODO Move to Screen impl like the others
    //    screens.push(screen);
    //}

    // If no screens exist, fake one.
    if screens.is_empty() {
        let mut screen = Screen::new(lua)?;
        {
            let mut obj = screen.state_mut()?;
            obj.geometry = Size::new(1024, 768).into();
            obj.workarea = obj.geometry;
        }
        screens.push(screen);
    }

    lua.set_named_registry_value(SCREENS_HANDLE, screens.clone().to_lua(lua)?)?;
    Ok(res)
}

fn method_setup<'lua>(
    lua: &'lua Lua,
    builder: ClassBuilder<'lua, ScreenState>
) -> rlua::Result<ClassBuilder<'lua, ScreenState>> {
    // TODO Do properly
    use super::dummy;
    builder
        .method("connect_signal".into(), lua.create_function(dummy)?)?
        .method("count".into(), lua.create_function(count)?)?
        .method("__call".into(), lua.create_function(iterate_over_screens)?)?
        .method("__index".into(), lua.create_function(index)?)
}

fn property_setup<'lua>(
    lua: &'lua Lua,
    builder: ClassBuilder<'lua, ScreenState>
) -> rlua::Result<ClassBuilder<'lua, ScreenState>> {
    builder
        .property(Property::new(
            "geometry".into(),
            None,
            Some(lua.create_function(get_geometry)?),
            None
        ))?
        .property(Property::new(
            "workarea".into(),
            None,
            Some(lua.create_function(get_workarea)?),
            None
        ))
}

fn get_geometry<'lua>(lua: &'lua Lua, screen: Screen<'lua>) -> rlua::Result<Table<'lua>> {
    screen.get_geometry(lua)
}

fn get_workarea<'lua>(lua: &'lua Lua, screen: Screen<'lua>) -> rlua::Result<Table<'lua>> {
    screen.get_workarea(lua)
}

fn count<'lua>(lua: &'lua Lua, _: ()) -> rlua::Result<Value<'lua>> {
    let screens = lua.named_registry_value::<Vec<Screen>>(SCREENS_HANDLE)?;
    Ok(Value::Integer(screens.len() as _))
}

/// Ok this requires some explanation...
/// Lua gives us the previous value in the loop, with the first one being nil
/// since there was nothing there before.
///
/// To ensure we loop over everything, we take the index of that value in our
/// list, increment it by 1 (starting at 0 if it's the start) and then once it
/// falls outside the bounds it will stop by returning nil.
fn iterate_over_screens<'lua>(
    lua: &'lua Lua,
    (_, prev): (Value<'lua>, Value<'lua>)
) -> rlua::Result<Value<'lua>> {
    let mut screens = lua.named_registry_value::<Vec<Screen>>(SCREENS_HANDLE)?;

    let index = match prev {
        Value::Nil => 0,
        Value::UserData(ref object) => {
            if let Ok(screen) = Screen::cast(object.clone().into()) {
                screens
                    .iter()
                    .position(|t| *t.state().unwrap() == *screen.state().unwrap())
                    .unwrap_or(screens.len()) +
                    1
            } else {
                panic!("Unexpected non-screen table in loop");
            }
        },
        _ => panic!("Unexpected non-screen or nil value in screens loop")
    };
    if index < screens.len() {
        screens.remove(index).to_lua(lua)
    } else {
        Ok(Value::Nil)
    }
}

fn index<'lua>(lua: &'lua Lua, (obj, index): (AnyUserData<'lua>, Value<'lua>)) -> rlua::Result<Value<'lua>> {
    let screens: Vec<Screen> = lua.named_registry_value(SCREENS_HANDLE)?;
    match index {
        Value::String(ref string) => {
            let string = string.to_str()?;
            if string == "primary" {
                // TODO Emit primary changed signal
                if screens.len() > 0 {
                    return screens[0].clone().to_lua(lua);
                }
            }
            for screen in screens.iter() {
                let screen_state = screen.state()?;
                for output in &screen_state.outputs {
                    if output.name() == string {
                        return screen.clone().to_lua(lua);
                    }
                }
            }
        },
        // TODO Might need to do Number instead
        Value::Integer(screen_index) => {
            if screen_index < 1 || screen_index as usize > screens.len() {
                return Err(rlua::Error::RuntimeError(format!(
                    "invalid screen \
                     number: {} (of {} \
                     existing)",
                    screen_index,
                    screens.len()
                )));
            }
            return screens[(screen_index - 1) as usize].clone().to_lua(lua);
        },
        Value::UserData(ref obj) => {
            // If this is a screen, just return it
            if let Ok(screen) = Screen::cast(obj.clone().into()) {
                return screen.to_lua(lua);
            }
        },
        // TODO This checke user data like in luaA_toudata in awesome
        _ => {}
    }
    // TODO checkudata
    let table: Table = obj.get_user_value()?;
    let meta = table.get_metatable().expect("screen had no metatable");
    match meta.get(index.clone()) {
        Err(_) | Ok(Value::Nil) => object::default_index(lua, (Screen::cast(obj)?, index)),
        Ok(value) => Ok(value)
    }
}
