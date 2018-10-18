use rlua::{self, FromLua, Lua, ToLua, Value};

pub type PropF<'lua> = rlua::Function<'lua>;

/// Type that represents a Lua table with these fields.
///
/// NOTE Not actually UserData, because it has lua functions in it.
/// It's just the deserialized version of a table.
#[derive(Debug)]
pub struct Property<'lua> {
    pub name: String,
    pub cb_new: Option<PropF<'lua>>,
    pub cb_index: Option<PropF<'lua>>,
    pub cb_newindex: Option<PropF<'lua>>,
}

impl<'lua> ToLua<'lua> for Property<'lua> {
    fn to_lua(self, lua: &'lua Lua) -> rlua::Result<Value<'lua>> {
        let table = lua.create_table()?;
        table.set("name", self.name)?;
        let metatable = lua.create_table()?;
        metatable.set(
            "__call",
            self.cb_new.map(Value::Function).unwrap_or(Value::Nil),
        )?;
        metatable.set(
            "__index",
            self.cb_index.map(Value::Function).unwrap_or(Value::Nil),
        )?;
        metatable.set(
            "__newindex",
            self.cb_newindex.map(Value::Function).unwrap_or(Value::Nil),
        )?;
        table.set_metatable(Some(metatable));
        Ok(Value::Table(table))
    }
}

impl<'lua> FromLua<'lua> for Property<'lua> {
    fn from_lua(val: Value<'lua>, _: &'lua Lua) -> rlua::Result<Self> {
        if let Value::Table(table) = val {
            let name = table.get("name")?;
            let meta = table
                .get_metatable()
                .expect("Property table had no metatable");
            let cb_new = meta.get("__call").ok();
            let cb_index = meta.get("__index").ok();
            let cb_newindex = meta.get("__newindex").ok();
            Ok(Property {
                name,
                cb_new,
                cb_index,
                cb_newindex,
            })
        } else {
            use rlua::Error::FromLuaConversionError;
            Err(FromLuaConversionError {
                from: "something else",
                to: "Property",
                message: None,
            })
        }
    }
}

impl<'lua> Property<'lua> {
    pub fn new(
        name: String,
        cb_new: Option<PropF<'lua>>,
        cb_index: Option<PropF<'lua>>,
        cb_newindex: Option<PropF<'lua>>,
    ) -> Self {
        Property {
            name,
            cb_new,
            cb_index,
            cb_newindex,
        }
    }
}
