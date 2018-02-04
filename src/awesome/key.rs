//! TODO Fill in

use std::fmt::{self, Display, Formatter};
use std::default::Default;
use std::rc::Rc;
use rlua::{self, Table, Lua, UserData, ToLua, Value};
use super::object::{Object, Objectable};
use super::class::{self, Class, ClassBuilder};

#[derive(Clone, Debug)]
pub struct KeyState {
    // TODO Fill in
    dummy: i32
}

pub struct Key<'lua>(Table<'lua>);

impl Default for KeyState {
    fn default() -> Self {
        KeyState {
            dummy: 0
        }
    }
}

impl <'lua> Key<'lua> {
    fn new(lua: &Lua) -> rlua::Result<Object> {
        // TODO FIXME
        let class = class::class_setup(lua, "key")?;
        Ok(Key::allocate(lua, class)?.build())
    }
}

impl Display for KeyState {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Key: {:p}", self)
    }
}

impl <'lua> ToLua<'lua> for Key<'lua> {
    fn to_lua(self, lua: &'lua Lua) -> rlua::Result<Value<'lua>> {
        self.0.to_lua(lua)
    }
}

impl UserData for KeyState {}

pub fn init(lua: &Lua) -> rlua::Result<Class> {
    property_setup(lua, method_setup(lua, Class::builder(lua, "key", Some(Rc::new(Key::new)), None, None)?)?)?
        .save_class("key")?
        .build()
}

fn method_setup<'lua>(lua: &'lua Lua, builder: ClassBuilder<'lua>) -> rlua::Result<ClassBuilder<'lua>> {
    // TODO Do properly
    builder.method("__call".into(), lua.create_function(dummy_create))
}

fn property_setup<'lua>(lua: &'lua Lua, builder: ClassBuilder<'lua>) -> rlua::Result<ClassBuilder<'lua>> {
    // TODO Do properly
    builder.dummy_property("version".into(), "0".to_lua(lua)?)?
           .dummy_property("themes_path".into(), "/usr/share/awesome/themes".to_lua(lua)?)?
           .dummy_property("conffile".into(), "".to_lua(lua)?)
}

impl_objectable!(Key, KeyState);

fn dummy_create<'lua>(lua: &'lua Lua, _: rlua::Value) -> rlua::Result<Object<'lua>> {
    Key::new(lua)
}
