//! Utility methods and constructors for Lua classes

use std::default::Default;
use std::rc::Rc;
use rlua::{self, Lua, ToLua, Table, UserData, AnyUserData, Value, Function};
use super::object::Object;
use super::property::Property;

pub type Allocator = Rc<Fn(&Lua) -> rlua::Result<Object>>;
pub type Collector = Rc<Fn(Object)>;
pub type Checker = Rc<Fn(Object) -> bool>;

#[derive(Debug)]
pub struct Class<'lua> {
    table: Table<'lua>
}

#[derive(Clone)]
pub struct ClassState {
    // NOTE That this is missing fields from the C version.
    // They stored in the meta table instead, to not have unsafety.
    // They are fetchable using getters.
    name: String,
    allocator: Option<Allocator>,
    collector: Option<Collector>,
    checker: Option<Checker>,
    instances: u32
}

pub struct ClassBuilder<'lua>{
    lua: &'lua Lua,
    class: Class<'lua>
}

impl <'lua> ClassBuilder<'lua> {
    pub fn method(self, name: String, meth: rlua::Function)
                  -> rlua::Result<Self> {
        let meta = self.class.table.get_metatable()
            .expect("Class had no meta table!");
        meta.set(name, meth)?;
        Ok(self)
    }

    pub fn property(self, prop: Property<'lua>) -> rlua::Result<Self> {
        let properties = self.class.table.get::<_, Table>("properties")?;
        let length = properties.len().unwrap_or(0) + 1;
        properties.set(length, prop)?;
        Ok(self)
    }

    // TODO remove, do right
    pub fn dummy_property(self, key: String, val: rlua::Value<'lua>) -> rlua::Result<Self> {
        let meta = self.class.table.get_metatable()
            .expect("Class had no meta table!");
        meta.set(key, val)?;
        Ok(self)
    }

    pub fn save_class(mut self, name: &str)
                      -> rlua::Result<Self> {
        self.lua.globals().set(name, self.class.table)?;
        self.class.table = self.lua.globals().get(name)?;
        Ok(self)
    }

    pub fn build(self) -> rlua::Result<Class<'lua>> {
        Ok(self.class)
    }
}

impl <'lua> ToLua<'lua> for Class<'lua> {
    fn to_lua(self, lua: &'lua Lua) -> rlua::Result<Value<'lua>> {
        self.table.to_lua(lua)
    }
}

impl Default for ClassState {
    fn default() -> Self {
        ClassState {
            name: String::default(),
            allocator: Option::default(),
            collector: Option::default(),
            checker: Option::default(),
            instances: 0
        }
    }
}

impl UserData for ClassState {}

impl <'lua> Class<'lua> {
    pub fn builder(lua: &'lua Lua,
                   name: &str,
                   allocator: Option<Allocator>,
                   collector: Option<Collector>,
                   checker: Option<Checker>)
                   -> rlua::Result<ClassBuilder<'lua>> {
        let mut class = ClassState::default();
        class.allocator = allocator;
        class.collector = collector;
        class.checker = checker;
        let table = lua.create_table();
        // Store in not meta table so we can't index it
        table.set("data", class)?;
        table.set("name", name)?;
        table.set("properties", Vec::<Property>::new().to_lua(lua)?)?;
        let meta = lua.create_table();
        meta.set("signals", lua.create_table())?;
        meta.set("set_index_miss_handler",
                 lua.create_function(set_index_miss_handler).bind(table.clone())?)?;
        meta.set("set_newindex_miss_handler",
                 lua.create_function(set_newindex_miss_handler).bind(table.clone())?)?;
        meta.set("__index", meta.clone())?;
        table.set_metatable(Some(meta));
        Ok(ClassBuilder{
            lua: lua,
            class: Class { table }
        })
    }


    #[allow(dead_code)]
    pub fn properties(&self) -> rlua::Result<Table<'lua>> {
        self.table.get("properties")
    }

    #[allow(dead_code)]
    pub fn parent(&self) -> rlua::Result<Option<Class>> {
        use rlua::Value;
        match self.table.get::<_, Value>("parent")? {
            Value::Table(table) => {
                let data = table.get::<_, AnyUserData>("data")?;
                if !data.is::<ClassState>() {
                    Ok(None)
                } else {
                    Ok(Some(table.into()))
                }
            },
            _ => Ok(None)
        }
    }

    pub fn checker(&self) -> rlua::Result<Option<Checker>> {
        self.table.get::<_, ClassState>("data")
            .map(|state| state.checker)
    }
}

impl <'lua> From<Table<'lua>> for Class<'lua> {
    fn from(table: Table<'lua>) -> Self {
        Class { table }
    }
}

fn set_index_miss_handler<'lua>(_: &'lua Lua, (class, func): (Table, Function))
                                -> rlua::Result<()> {
    let meta = class.get_metatable()
        .expect("Object had no metatable");
    meta.set("__index_miss_handler", func)?;
    Ok(())
}
fn set_newindex_miss_handler<'lua>(_: &'lua Lua, (class, func): (Table, Function))
                                   -> rlua::Result<()> {
    let meta = class.get_metatable()
        .expect("Object had no metatable");
    meta.set("__newindex_miss_handler", func)?;
    Ok(())
}

pub fn class_setup<'lua>(lua: &'lua Lua, name: &str) -> rlua::Result<Class<'lua>> {
    let table = lua.globals().get::<_, Table>(name)
        .expect("Class was not set! Did you call init?");
    assert!(table.get::<_, AnyUserData>("data")?.is::<ClassState>(),
            "This table was not a class!");
    Ok(Class { table })
}
