//! A button that was pressed on a mouse by a user.
//!
//! This is mostly used to define bindings.

use std::default::Default;
use std::fmt::{self, Display, Formatter};

use rlua::{self, AnyUserData, Lua, Table, ToLua, UserData, UserDataMethods, Value};
use wlroots::events::key_events::Key;
use xcb::ffi::xproto::xcb_button_t;

use common::{
    class::{self, Class},
    object::{self, Object, Objectable},
    property::Property,
    signal,
};

#[derive(Clone, Debug)]
pub struct ButtonState {
    button: xcb_button_t,
    modifiers: Vec<Key>,
}

#[derive(Clone, Debug)]
pub struct Button<'lua>(Object<'lua>);

impl Display for ButtonState {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Button: {:p}", self)
    }
}

impl Default for ButtonState {
    fn default() -> Self {
        ButtonState {
            button: xcb_button_t::default(),
            modifiers: Vec::new(),
        }
    }
}

impl UserData for ButtonState {
    fn add_methods(methods: &mut UserDataMethods<Self>) {
        object::default_add_methods(methods);
    }
}

impl<'lua> Button<'lua> {
    fn new(lua: &'lua Lua, args: rlua::Table) -> rlua::Result<Object<'lua>> {
        let class = class::class_setup(lua, "button")?;
        Ok(Button::allocate(lua, class)?
            .handle_constructor_argument(args)?
            .build())
    }

    pub fn button(&self) -> rlua::Result<Value<'lua>> {
        let button = self.state()?;
        Ok(Value::Integer(button.button as _))
    }

    pub fn set_button(&mut self, new_val: xcb_button_t) -> rlua::Result<()> {
        let mut button = self.get_object_mut()?;
        button.button = new_val;
        Ok(())
    }

    pub fn modifiers(&self) -> rlua::Result<Vec<Key>> {
        let button = self.state()?;
        Ok(button.modifiers.clone())
    }

    pub fn set_modifiers(&mut self, mods: Table<'lua>) -> rlua::Result<()> {
        use lua::mods_to_rust;
        let mut button = self.get_object_mut()?;
        button.modifiers = mods_to_rust(mods)?;
        Ok(())
    }
}

impl<'lua> ToLua<'lua> for Button<'lua> {
    fn to_lua(self, lua: &'lua Lua) -> rlua::Result<Value<'lua>> {
        self.0.to_lua(lua)
    }
}

impl_objectable!(Button, ButtonState);

pub fn init(lua: &Lua) -> rlua::Result<Class> {
    Class::builder(lua, "button", None)?
        .method(
            "__call".into(),
            lua.create_function(|lua, args: rlua::Table| Button::new(lua, args))?,
        )?.property(Property::new(
            "button".into(),
            Some(lua.create_function(set_button)?),
            Some(lua.create_function(get_button)?),
            Some(lua.create_function(set_button)?),
        ))?.property(Property::new(
            "modifiers".into(),
            Some(lua.create_function(set_modifiers)?),
            Some(lua.create_function(get_modifiers)?),
            Some(lua.create_function(set_modifiers)?),
        ))?.save_class("button")?
        .build()
}

fn set_button<'lua>(
    lua: &'lua Lua,
    (obj, val): (AnyUserData<'lua>, Value<'lua>),
) -> rlua::Result<Value<'lua>> {
    use rlua::Value::*;
    let mut button = Button::cast(obj.clone().into())?;
    match val {
        Number(num) => button.set_button(num as _)?,
        Integer(num) => button.set_button(num as _)?,
        _ => button.set_button(xcb_button_t::default())?,
    }
    signal::emit_object_signal(lua, obj.into(), "property::button".into(), val)?;
    Ok(Value::Nil)
}

fn get_button<'lua>(_: &'lua Lua, obj: AnyUserData<'lua>) -> rlua::Result<Value<'lua>> {
    Button::cast(obj.into())?.button()
}

fn set_modifiers<'lua>(
    lua: &'lua Lua,
    (obj, modifiers): (AnyUserData<'lua>, Table<'lua>),
) -> rlua::Result<()> {
    let mut button = Button::cast(obj.clone().into())?;
    button.set_modifiers(modifiers.clone())?;
    signal::emit_object_signal(lua, obj.into(), "property::modifiers".into(), modifiers)?;
    Ok(())
}

fn get_modifiers<'lua>(lua: &'lua Lua, obj: AnyUserData<'lua>) -> rlua::Result<Value<'lua>> {
    use lua::mods_to_lua;
    mods_to_lua(lua, &Button::cast(obj.into())?.modifiers()?).map(Value::Table)
}
