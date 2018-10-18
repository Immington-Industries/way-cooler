//! API for root resources, such as wallpapers and keybindings.

use std::default::Default;
use std::fmt::{self, Display, Formatter};

use cairo_sys::cairo_pattern_t;
use rlua::{self, LightUserData, Lua, Table, ToLua, UserData, UserDataMethods, Value};

use common::{
    class::{Class, ClassBuilder},
    object::{self, Object, Objectable},
};
use objects::tag;

/// Handle to the list of global key bindings
pub const ROOT_KEYS_HANDLE: &'static str = "__ROOT_KEYS";

#[derive(Clone, Debug)]
pub struct RootState {
    // TODO Fill in
    dummy: i32,
}

pub struct Root<'lua>(Object<'lua>);

impl Default for RootState {
    fn default() -> Self {
        RootState { dummy: 0 }
    }
}

impl Display for RootState {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Root: {:p}", self)
    }
}

impl<'lua> ToLua<'lua> for Root<'lua> {
    fn to_lua(self, lua: &'lua Lua) -> rlua::Result<Value<'lua>> {
        self.0.to_lua(lua)
    }
}

impl UserData for RootState {
    fn add_methods(methods: &mut UserDataMethods<Self>) {
        object::default_add_methods(methods);
    }
}

pub fn init(lua: &Lua) -> rlua::Result<Class> {
    // FIXME: In awesome there is no root class
    method_setup(lua, Class::builder(lua, "FIXME", None)?)?
        .save_class("root")?
        .build()
}

fn method_setup<'lua>(
    lua: &'lua Lua,
    builder: ClassBuilder<'lua>,
) -> rlua::Result<ClassBuilder<'lua>> {
    // TODO Do properly
    use objects::dummy;
    builder
        .method("connect_signal".into(), lua.create_function(dummy)?)?
        .method("buttons".into(), lua.create_function(dummy)?)?
        .method("wallpaper".into(), lua.create_function(wallpaper)?)?
        .method("tags".into(), lua.create_function(tags)?)?
        .method("keys".into(), lua.create_function(root_keys)?)?
        .method("size".into(), lua.create_function(dummy_double)?)?
        .method("size_mm".into(), lua.create_function(dummy_double)?)?
        .method("cursor".into(), lua.create_function(dummy)?)
}

impl_objectable!(Root, RootState);

fn dummy_double<'lua>(_: &'lua Lua, _: rlua::Value) -> rlua::Result<(i32, i32)> {
    Ok((0, 0))
}

/// Gets the wallpaper as a cairo surface or set it as a cairo pattern
fn wallpaper<'lua>(lua: &'lua Lua, pattern: Option<LightUserData>) -> rlua::Result<Value<'lua>> {
    // TODO FIXME Implement for realz
    if let Some(pattern) = pattern {
        // TODO Wrap before giving it to set_wallpaper
        let pattern = pattern.0 as *mut cairo_pattern_t;
        return set_wallpaper(lua, pattern)?.to_lua(lua);
    }
    // TODO Look it up in global conf (e.g probably super secret lua value)
    return Ok(Value::Nil);
}

fn set_wallpaper<'lua>(_: &'lua Lua, _pattern: *mut cairo_pattern_t) -> rlua::Result<bool> {
    warn!("Fake setting the wallpaper");
    Ok(true)
}

fn tags<'lua>(lua: &'lua Lua, _: ()) -> rlua::Result<Table<'lua>> {
    let table = lua.create_table()?;
    let activated_tags = lua.named_registry_value::<Table>(tag::TAG_LIST)?;
    for pair in activated_tags.pairs::<Value, Value>() {
        let (key, value) = pair?;
        table.set(key, value)?;
    }
    Ok(table)
}

/// Get or set global key bindings.
///
/// These bindings will be available when you press keys on the root window.
fn root_keys<'lua>(
    lua: &'lua Lua,
    key_array: rlua::Value<'lua>,
) -> rlua::Result<rlua::Value<'lua>> {
    match key_array {
        // Set the global keys
        Value::Table(key_array) => {
            let copy = lua.create_table()?;
            // NOTE We make a deep clone so they can't modify references.
            for entry in key_array.clone().pairs() {
                let (key, value) = entry?;
                copy.set::<Value, Value>(key, value)?;
            }
            lua.set_named_registry_value(ROOT_KEYS_HANDLE, copy)?;
            Ok(Value::Table(key_array))
        }
        // Get the global keys
        Value::Nil => {
            let res = lua.create_table()?;
            for entry in lua
                .named_registry_value::<Table>(ROOT_KEYS_HANDLE)
                .or(lua.create_table())?
                .pairs()
            {
                let (key, value) = entry?;
                res.set::<Value, Value>(key, value)?;
            }
            Ok(Value::Table(res))
        }
        v => Err(rlua::Error::RuntimeError(format!(
            "Expected nil or array \
             of keys, got {:?}",
            v
        ))),
    }
}

#[cfg(test)]
mod test {
    use super::super::key;
    use super::super::root;
    use super::super::tag;
    use rlua::Lua;

    #[test]
    fn tags_none() {
        let lua = Lua::new();
        tag::init(&lua).unwrap();
        root::init(&lua).unwrap();
        lua.eval(
            r#"
local t = root.tags()
assert(type(t) == "table")
assert(type(next(t)) == "nil")
"#,
            None,
        ).unwrap()
    }

    #[test]
    fn tags_does_not_copy() {
        let lua = Lua::new();
        tag::init(&lua).unwrap();
        root::init(&lua).unwrap();
        lua.eval(
            r#"
local t = tag{ activated = true }
local t2 = root.tags()[1]
assert(t == t2)
t2.name = "Foo"
assert(t.name == "Foo")
"#,
            None,
        ).unwrap()
    }

    #[test]
    fn tags_some() {
        let lua = Lua::new();
        tag::init(&lua).unwrap();
        root::init(&lua).unwrap();
        lua.eval(
            r#"
local first = tag{ activated = true }
local second = tag{ activated = true }
local t = root.tags()
assert(t[1] == first)
assert(t[2] == second)
"#,
            None,
        ).unwrap()
    }

    #[test]
    fn tags_removal() {
        let lua = Lua::new();
        tag::init(&lua).unwrap();
        root::init(&lua).unwrap();
        lua.eval(
            r#"
local first = tag{ activated = true }
local second = tag{ activated = true }
first.activated = false
local t = root.tags()
assert(t[1] == second)
assert(type(t[2]) == "nil")
"#,
            None,
        ).unwrap()
    }

    #[test]
    fn keys() {
        let lua = Lua::new();
        key::init(&lua).unwrap();
        root::init(&lua).unwrap();
        lua.eval(
            r#"
assert(next(root.keys()) == nil)

local first = key{}
local second = key{}
local keys = { first, second }

local res = root.keys(keys)
assert(res[1] == first)
assert(res[2] == second)
assert(res[3] == nil)

keys[3] = key{}
local res = root.keys()
assert(res[1] == first)
assert(res[2] == second)
assert(res[3] == nil)
"#,
            None,
        ).unwrap()
    }
}
