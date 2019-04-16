//! A button that was pressed on a mouse by a user.
//!
//! This is mostly used to define bindings.

use std::default::Default;

use rlua::{self, Lua, Table, UserData, UserDataMethods, Value};
use wlroots::input::keyboard::Key;
use xcb::ffi::xproto::xcb_button_t;

use crate::common::{
    class::{self, Class},
    object::{self, Object},
    property::Property,
    signal
};

#[derive(Clone, Debug)]
pub struct ButtonState {
    button: xcb_button_t,
    modifiers: Vec<Key>
}

pub type Button<'lua> = Object<'lua, ButtonState>;

impl Default for ButtonState {
    fn default() -> Self {
        ButtonState {
            button: xcb_button_t::default(),
            modifiers: Vec::new()
        }
    }
}

impl UserData for ButtonState {
    fn add_methods(methods: &mut UserDataMethods<Self>) {
        object::default_add_methods(methods);
    }
}

impl<'lua> Button<'lua> {
    fn new(lua: &'lua Lua, args: Table) -> rlua::Result<Button<'lua>> {
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
        let mut button = self.state_mut()?;
        button.button = new_val;
        Ok(())
    }

    pub fn modifiers(&self) -> rlua::Result<Vec<Key>> {
        let button = self.state()?;
        Ok(button.modifiers.clone())
    }

    pub fn set_modifiers(&mut self, mods: Table<'lua>) -> rlua::Result<()> {
        use crate::lua::mods_to_rust;
        let mut button = self.state_mut()?;
        button.modifiers = mods_to_rust(mods)?;
        Ok(())
    }
}

pub fn init(lua: &Lua) -> rlua::Result<Class<ButtonState>> {
    Class::<ButtonState>::builder(lua, "button", None)?
        .method(
            "__call".into(),
            lua.create_function(|lua, args: Table| Button::new(lua, args))?
        )?
        .property(Property::new(
            "button".into(),
            Some(lua.create_function(set_button)?),
            Some(lua.create_function(get_button)?),
            Some(lua.create_function(set_button)?)
        ))?
        .property(Property::new(
            "modifiers".into(),
            Some(lua.create_function(set_modifiers)?),
            Some(lua.create_function(get_modifiers)?),
            Some(lua.create_function(set_modifiers)?)
        ))?
        .save_class("button")?
        .build()
}

fn set_button<'lua>(
    lua: &'lua Lua,
    (mut button, val): (Button<'lua>, Value<'lua>)
) -> rlua::Result<Value<'lua>> {
    use rlua::Value::*;
    match val {
        Number(num) => button.set_button(num as _)?,
        Integer(num) => button.set_button(num as _)?,
        _ => button.set_button(xcb_button_t::default())?
    }
    signal::emit_object_signal(lua, button, "property::button".into(), val)?;
    Ok(Nil)
}

fn get_button<'lua>(_: &'lua Lua, button: Button<'lua>) -> rlua::Result<Value<'lua>> {
    button.button()
}

fn set_modifiers<'lua>(
    lua: &'lua Lua,
    (mut button, modifiers): (Button<'lua>, Table<'lua>)
) -> rlua::Result<()> {
    button.set_modifiers(modifiers.clone())?;
    signal::emit_object_signal(lua, button, "property::modifiers".into(), modifiers)?;
    Ok(())
}

fn get_modifiers<'lua>(lua: &'lua Lua, button: Button<'lua>) -> rlua::Result<Value<'lua>> {
    use crate::lua::mods_to_lua;
    mods_to_lua(lua, &button.modifiers()?).map(Value::Table)
}
