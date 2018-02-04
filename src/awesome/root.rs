//! TODO Fill in

use cairo_sys::cairo_pattern_t;
use std::fmt::{self, Display, Formatter};
use std::default::Default;
use std::rc::Rc;
use rlua::{self, Table, Lua, UserData, ToLua, Value, LightUserData};
use super::object::{Object, Objectable};
use super::class::{self, Class, ClassBuilder};

#[derive(Clone, Debug)]
pub struct RootState {
    // TODO Fill in
    dummy: i32
}

pub struct Root<'lua>(Table<'lua>);

impl Default for RootState {
    fn default() -> Self {
        RootState {
            dummy: 0
        }
    }
}

impl <'lua> Root<'lua> {
    fn new(lua: &Lua) -> rlua::Result<Object> {
        // TODO FIXME
        let class = class::class_setup(lua, "root")?;
        Ok(Root::allocate(lua, class)?.build())
    }
}

impl Display for RootState {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Root: {:p}", self)
    }
}

impl <'lua> ToLua<'lua> for Root<'lua> {
    fn to_lua(self, lua: &'lua Lua) -> rlua::Result<Value<'lua>> {
        self.0.to_lua(lua)
    }
}

impl UserData for RootState {}

pub fn init(lua: &Lua) -> rlua::Result<Class> {
    // FIXME: In awesome there is no root class
    method_setup(lua, Class::builder(lua, "FIXME", Some(Rc::new(Root::new)), None, None)?)?
        .save_class("root")?
        .build()
}

fn method_setup<'lua>(lua: &'lua Lua, builder: ClassBuilder<'lua>) -> rlua::Result<ClassBuilder<'lua>> {
    // TODO Do properly
    use super::dummy;
    builder.method("connect_signal".into(), lua.create_function(dummy))?
           .method("buttons".into(), lua.create_function(dummy))?
           .method("wallpaper".into(), lua.create_function(wallpaper))?
           .method("tags".into(), lua.create_function(tags))?
           .method("keys".into(), lua.create_function(dummy))?
           .method("size".into(), lua.create_function(dummy_double))?
           .method("size_mm".into(), lua.create_function(dummy_double))?
           .method("cursor".into(), lua.create_function(dummy))
}

impl_objectable!(Root, RootState);

fn dummy_double<'lua>(_: &'lua Lua, _: rlua::Value) -> rlua::Result<(i32, i32)> { Ok((0, 0)) }

/// Gets the wallpaper as a cairo surface or set it as a cairo pattern
fn wallpaper<'lua>(lua: &'lua Lua, pattern: Option<LightUserData>) -> rlua::Result<Value<'lua>> {
    // TODO FIXME Implement for realz
    if let Some(pattern) = pattern {
        // TODO Wrap before giving it to set_wallpaper
        let pattern = pattern.0 as *mut cairo_pattern_t;
        return set_wallpaper(lua, pattern)?.to_lua(lua)
    }
    // TODO Look it up in global conf (e.g probably super secret lua value)
    return Ok(Value::Nil)
}

fn set_wallpaper<'lua>(_: &'lua Lua, _pattern: *mut cairo_pattern_t) -> rlua::Result<bool> {
    warn!("Fake setting the wallpaper");
    Ok(true)
}

fn tags<'lua>(lua: &'lua Lua, _: ()) -> rlua::Result<Table<'lua>> {
    let table = lua.create_table();
    // TODO FIXME Get tags
    Ok(table)
}
