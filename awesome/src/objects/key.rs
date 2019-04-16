//! A button that was pressed on a keyboard by a user.
//!
//! This is mostly used to define keybindings.

use rlua::{self, Lua, Table, ToLua, UserData, UserDataMethods, Value};
use wlroots::input::keyboard;
use wlroots::{self, xkbcommon::xkb};

use crate::common::{
    class::{self, Class, ClassBuilder},
    object::{self, Object},
    property::Property
};
use crate::lua::mods_to_num;

#[derive(Clone, Debug, Default)]
pub struct KeyState {
    modifiers: u32,
    keysym: keyboard::Key,
    keycode: xkb::Keycode
}

pub type Key<'lua> = Object<'lua, KeyState>;

impl<'lua> Key<'lua> {
    fn new(lua: &'lua Lua, args: Table) -> rlua::Result<Key<'lua>> {
        // TODO FIXME
        let class = class::class_setup(lua, "key")?;
        Ok(Key::allocate(lua, class)?
            .handle_constructor_argument(args)?
            .build())
    }

    pub fn set_modifiers(&mut self, modifiers: u32) -> rlua::Result<()> {
        let mut state = self.state_mut()?;
        state.modifiers = modifiers;
        Ok(())
    }

    pub fn modifiers(&self) -> rlua::Result<u32> {
        let state = self.state()?;
        Ok(state.modifiers)
    }

    pub fn set_keysym(&mut self, keysym: wlroots::Key) -> rlua::Result<()> {
        let mut state = self.state_mut()?;
        state.keysym = keysym;
        Ok(())
    }

    pub fn keysym(&self) -> rlua::Result<wlroots::Key> {
        let state = self.state()?;
        Ok(state.keysym)
    }

    pub fn set_keycode(&mut self, keycode: xkb::Keycode) -> rlua::Result<()> {
        let mut state = self.state_mut()?;
        state.keycode = keycode;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn keycode(&self) -> rlua::Result<xkb::Keycode> {
        let state = self.state()?;
        Ok(state.keycode)
    }
}

impl UserData for KeyState {
    fn add_methods(methods: &mut UserDataMethods<Self>) {
        object::default_add_methods(methods);
    }
}

pub fn init(lua: &Lua) -> rlua::Result<Class<KeyState>> {
    property_setup(lua, method_setup(lua, Class::builder(lua, "key", None)?)?)?
        .save_class("key")?
        .build()
}

fn method_setup<'lua>(
    lua: &'lua Lua,
    builder: ClassBuilder<'lua, KeyState>
) -> rlua::Result<ClassBuilder<'lua, KeyState>> {
    // TODO Do properly
    builder.method(
        "__call".into(),
        lua.create_function(|lua, args: Table| Key::new(lua, args))?
    )
}

fn property_setup<'lua>(
    lua: &'lua Lua,
    builder: ClassBuilder<'lua, KeyState>
) -> rlua::Result<ClassBuilder<'lua, KeyState>> {
    // TODO Do properly
    builder
        .property(Property::new(
            "key".into(),
            Some(lua.create_function(set_key)?),
            Some(lua.create_function(get_key)?),
            Some(lua.create_function(set_key)?)
        ))?
        .property(Property::new(
            "keysym".into(),
            None,
            Some(lua.create_function(get_keysym)?),
            None
        ))?
        .property(Property::new(
            "modifiers".into(),
            Some(lua.create_function(set_modifiers)?),
            Some(lua.create_function(get_modifiers)?),
            Some(lua.create_function(set_modifiers)?)
        ))
}

fn get_modifiers<'lua>(_: &'lua Lua, key: Key<'lua>) -> rlua::Result<u32> {
    key.modifiers()
}

fn set_modifiers<'lua>(_: &'lua Lua, (mut key, mods): (Key<'lua>, Table<'lua>)) -> rlua::Result<()> {
    key.set_modifiers(mods_to_num(mods)?.bits())
}

fn get_keysym<'lua>(lua: &'lua Lua, key: Key<'lua>) -> rlua::Result<Value<'lua>> {
    // TODO Shouldn't this be able to fail?
    xkb::keysym_get_name(key.keysym()?).to_lua(lua)
}

fn get_key<'lua>(lua: &'lua Lua, key: Key<'lua>) -> rlua::Result<Value<'lua>> {
    key.keysym()?.to_lua(lua)
}

fn set_key<'lua>(_: &'lua Lua, (mut key, key_name): (Key<'lua>, String)) -> rlua::Result<Value<'lua>> {
    if key_name.starts_with('#') && key_name.len() >= 2 {
        let number = key_name[1..]
            .parse::<xkb::Keycode>()
            .map_err(|err| rlua::Error::RuntimeError(format!("Parse error: {:?}", err)))?;
        // the - 8 is because of xcb conventions, where "#10" is the keysim for 1,
        // and the keycode of 1 is 0x02 (obviously)
        key.set_keycode(number - 8)?;
    } else {
        let keysym = xkb::keysym_from_name(key_name.as_str(), 0);
        key.set_keysym(keysym)?;
    }
    Ok(rlua::Value::Nil)
}
