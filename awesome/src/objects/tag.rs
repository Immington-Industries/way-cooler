//! A tag is similar to a workspace, except a client can be attached
//! to multiple tags at once.

use std::default::Default;
use std::fmt::{self, Display, Formatter};

use rlua::{self, AnyUserData, Integer, Lua, Table, ToLua, UserData, UserDataMethods, Value};

use common::{
    class::{self, Class, ClassBuilder},
    object::{self, Object, Objectable},
    property::Property,
    signal,
};

pub const TAG_LIST: &'static str = "__tag_list";

#[derive(Clone, Debug)]
pub struct TagState {
    name: Option<String>,
    selected: bool,
    activated: bool,
}

pub struct Tag<'lua>(Object<'lua>);

impl Default for TagState {
    fn default() -> Self {
        TagState {
            name: None,
            selected: false,
            activated: false,
        }
    }
}

impl<'lua> Tag<'lua> {
    fn new(lua: &'lua Lua, args: Table) -> rlua::Result<Object<'lua>> {
        let class = class::class_setup(lua, "tag")?;
        Ok(Tag::allocate(lua, class)?
            .handle_constructor_argument(args)?
            .build())
    }
}

impl Display for TagState {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Tag: {:p}", self)
    }
}

impl<'lua> ToLua<'lua> for Tag<'lua> {
    fn to_lua(self, lua: &'lua Lua) -> rlua::Result<Value<'lua>> {
        self.0.to_lua(lua)
    }
}

impl UserData for TagState {
    fn add_methods(methods: &mut UserDataMethods<Self>) {
        object::default_add_methods(methods);
    }
}

pub fn init(lua: &Lua) -> rlua::Result<Class> {
    lua.set_named_registry_value(TAG_LIST, lua.create_table()?)?;
    method_setup(lua, Class::builder(lua, "tag", None)?)?
        .save_class("tag")?
        .build()
}

fn method_setup<'lua>(
    lua: &'lua Lua,
    builder: ClassBuilder<'lua>,
) -> rlua::Result<ClassBuilder<'lua>> {
    // TODO Do properly
    use super::dummy;
    builder
        .method("connect_signal".into(), lua.create_function(dummy)?)?
        .method(
            "__call".into(),
            lua.create_function(|lua, args: Table| Tag::new(lua, args))?,
        )?.property(Property::new(
            "name".into(),
            Some(lua.create_function(set_name)?),
            Some(lua.create_function(get_name)?),
            Some(lua.create_function(set_name)?),
        ))?.property(Property::new(
            "selected".into(),
            Some(lua.create_function(set_selected)?),
            Some(lua.create_function(get_selected)?),
            Some(lua.create_function(set_selected)?),
        ))?.property(Property::new(
            "activated".into(),
            Some(lua.create_function(set_activated)?),
            Some(lua.create_function(get_activated)?),
            Some(lua.create_function(set_activated)?),
        ))?.property(Property::new(
            "clients".into(),
            None,
            Some(lua.create_function(get_clients)?),
            None,
        ))
}

impl_objectable!(Tag, TagState);

fn set_name<'lua>(
    lua: &'lua Lua,
    (obj, val): (AnyUserData<'lua>, String),
) -> rlua::Result<Value<'lua>> {
    let mut tag = Tag::cast(obj.clone().into())?;
    tag.get_object_mut()?.name = Some(val.clone());
    signal::emit_object_signal(lua, obj.into(), "property::name".into(), ())?;
    Ok(Value::Nil)
}

fn get_name<'lua>(lua: &'lua Lua, obj: AnyUserData<'lua>) -> rlua::Result<Value<'lua>> {
    match obj.borrow::<TagState>()?.name {
        None => Ok(Value::Nil),
        Some(ref name) => Ok(Value::String(lua.create_string(&name)?)),
    }
}

fn set_selected<'lua>(
    lua: &'lua Lua,
    (obj, val): (AnyUserData<'lua>, bool),
) -> rlua::Result<Value<'lua>> {
    let mut tag = Tag::cast(obj.clone().into())?;
    {
        let mut tag = tag.get_object_mut()?;
        if tag.selected == val {
            return Ok(Value::Nil);
        }
        tag.selected = val;
    }
    signal::emit_object_signal(lua, obj.into(), "property::selected".into(), ())?;
    Ok(Value::Nil)
}

fn get_selected<'lua>(_: &'lua Lua, obj: AnyUserData<'lua>) -> rlua::Result<Value<'lua>> {
    Ok(Value::Boolean(obj.borrow::<TagState>()?.selected))
}

fn set_activated<'lua>(
    lua: &'lua Lua,
    (obj, val): (AnyUserData<'lua>, bool),
) -> rlua::Result<Value<'lua>> {
    let mut tag = Tag::cast(obj.clone().into())?;
    {
        let mut tag = tag.get_object_mut()?;
        if tag.activated == val {
            return Ok(Value::Nil);
        }
        tag.activated = val;
    }
    let activated_tags = lua.named_registry_value::<Table>(TAG_LIST)?;
    let activated_tags_count = activated_tags.len()?;
    if val {
        let index = activated_tags_count + 1;
        activated_tags.set(index, obj.clone())?;
    } else {
        // Find and remove the tag in/from the list of tags
        {
            let tag_ref = &*obj.borrow::<TagState>()? as *const _;
            let mut found = false;
            for pair in activated_tags.clone().pairs::<Integer, AnyUserData>() {
                let (key, value) = pair?;
                if tag_ref == &*value.borrow::<TagState>()? as *const _ {
                    found = true;
                    // Now remove this by shifting everything down...
                    for index in key..activated_tags_count {
                        activated_tags.set(index, activated_tags.get::<_, Value>(index + 1)?)?;
                    }
                    activated_tags.set(activated_tags_count, Value::Nil)?;
                    break;
                }
            }
            assert!(found);
        }
        set_selected(lua, (obj.clone(), false))?;
    }
    signal::emit_object_signal(lua, obj.into(), "property::activated".into(), ())?;
    Ok(Value::Nil)
}

fn get_activated<'lua>(_: &'lua Lua, obj: AnyUserData<'lua>) -> rlua::Result<Value<'lua>> {
    Ok(Value::Boolean(obj.borrow::<TagState>()?.activated))
}

fn get_clients<'lua>(lua: &'lua Lua, _obj: AnyUserData<'lua>) -> rlua::Result<Value<'lua>> {
    // TODO / FIXME: Do this properly.
    // - Actually return clients.
    // - Right now this is a property that returns a function. Can we get rid of
    // some of this   indirection?
    Ok(Value::Function(
        lua.create_function(|lua, _: ()| lua.create_table())?,
    ))
}

#[cfg(test)]
mod test {
    use super::super::tag;
    use rlua::Lua;

    #[test]
    fn tag_name_empty() {
        let lua = Lua::new();
        tag::init(&lua).unwrap();
        lua.eval(
            r#"
assert(type(tag{}.name) == "nil")
"#,
            None,
        ).unwrap()
    }

    #[test]
    fn tag_name_change() {
        let lua = Lua::new();
        tag::init(&lua).unwrap();
        lua.eval(
            r#"
local t = tag{ name = "a very cool tag" }
assert(t.name == "a very cool tag")
"#,
            None,
        ).unwrap()
    }

    #[test]
    fn tag_name_signal() {
        let lua = Lua::new();
        tag::init(&lua).unwrap();
        lua.eval(
            r#"
local t = tag{}

local called = 0
t:connect_signal("property::name", function(t)
    assert(t.name == "bye")
    called = called + 1
end)

t.name = "bye"
assert(t.name == "bye")
assert(called == 1)
"#,
            None,
        ).unwrap()
    }

    #[test]
    fn tag_selected() {
        let lua = Lua::new();
        tag::init(&lua).unwrap();
        lua.eval(
            r#"
local t = tag{}
assert(t.selected == false)

local called = 0
t:connect_signal("property::selected", function(t)
    called = called + 1
end)

t.selected = false
assert(t.selected == false)
assert(called == 0)

t.selected = true
assert(t.selected == true)
assert(called == 1)

t.selected = true
assert(t.selected == true)
assert(called == 1)

t.selected = false
assert(t.selected == false)
assert(called == 2)
"#,
            None,
        ).unwrap()
    }

    #[test]
    fn tag_activated() {
        let lua = Lua::new();
        tag::init(&lua).unwrap();
        lua.eval(
            r#"
local t = tag{}
assert(t.activated == false)

local called = 0
t:connect_signal("property::activated", function(t)
    called = called + 1
end)

t.activated = false
assert(t.activated == false)
assert(called == 0)

t.activated = true
assert(t.activated == true)
assert(called == 1)

t.activated = true
assert(t.activated == true)
assert(called == 1)

t.activated = false
assert(t.activated == false)
assert(called == 2)
"#,
            None,
        ).unwrap()
    }

    #[test]
    fn tag_activated_selected() {
        let lua = Lua::new();
        tag::init(&lua).unwrap();
        lua.eval(
            r#"
local t = tag{selected = true, activated = true}

local called_selected, called_activated = 0, 0
t:connect_signal("property::activated", function(t)
    called_activated = called_activated + 1
end)
t:connect_signal("property::selected", function(t)
    called_selected = called_selected + 1
end)

t.activated = false
assert(t.activated == false)
assert(t.selected == false)
assert(called_activated == 1)
assert(called_selected == 1)
"#,
            None,
        ).unwrap()
    }
}
