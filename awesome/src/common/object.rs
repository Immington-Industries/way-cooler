//! Utility methods and constructors for Lua objects

use super::class::Class;
use super::property::Property;
use super::signal;
use rlua::{
    self, AnyUserData, FromLua, Function, Lua, MetaMethod, Table, ToLua, UserData, UserDataMethods,
    Value,
};
use std::convert::From;
use std::fmt::Display;

/// All Lua objects can be cast to this.
#[derive(Clone, Debug)]
pub struct Object<'lua> {
    pub object: AnyUserData<'lua>,
}

impl<'lua> From<AnyUserData<'lua>> for Object<'lua> {
    fn from(object: AnyUserData<'lua>) -> Self {
        Object { object }
    }
}

/// Construct a new object, used when using the default Objectable::new.
pub struct ObjectBuilder<'lua> {
    lua: &'lua Lua,
    object: Object<'lua>,
}

impl<'lua> ObjectBuilder<'lua> {
    pub fn add_to_meta(self, new_meta: Table<'lua>) -> rlua::Result<Self> {
        let meta = self
            .object
            .table()?
            .get_metatable()
            .expect("Object had no meta table");
        for entry in new_meta.pairs::<rlua::Value, rlua::Value>() {
            let (key, value) = entry?;
            meta.set(key, value)?;
        }
        self.object.table()?.set_metatable(Some(meta));
        Ok(self)
    }

    #[allow(dead_code)]
    pub fn add_to_signals(self, name: String, func: rlua::Function) -> rlua::Result<Self> {
        signal::connect_signal(self.lua, self.object.clone(), name, &[func])?;
        Ok(self)
    }

    pub fn handle_constructor_argument(self, args: Table) -> rlua::Result<Self> {
        let meta = self
            .object
            .table()?
            .get_metatable()
            .expect("Object had no meta table");
        let class = meta.get::<_, AnyUserData>("__class")?;
        let class_table = class.get_user_value::<Table>()?;
        let props = class_table.get::<_, Vec<Property>>("properties")?;

        // Handle all table entries that correspond to known properties,
        // silently ignore all other keys
        for pair in args.pairs() {
            let (key, value): (Value, Value) = pair?;
            if let rlua::Value::String(key) = key {
                if let Ok(key) = key.to_str() {
                    // Find the property
                    for prop in props.iter() {
                        if prop.name == key {
                            // Property exists and has a cb_new callback
                            if let Some(ref new) = prop.cb_new {
                                let _: () = new.bind(self.object.clone())?.call(value)?;
                                break;
                            }
                        }
                    }
                }
            }
        }
        Ok(self)
    }

    pub fn build(self) -> Object<'lua> {
        self.object
    }
}

/// Trait implemented by all objects that represent OO lua objects.
///
/// This trait allows casting an object gotten back from the Lua runtime
/// into a concrete object so that Rust can do things with it.
///
/// You can't do anything to the object until it has been converted into a
/// canonical form using this trait.
pub trait Objectable<'lua, T, S: UserData + Default + Display + Send> {
    fn cast(obj: Object<'lua>) -> rlua::Result<T> {
        if obj.object.is::<S>()? {
            Ok(Self::_wrap(obj))
        } else {
            use rlua::Error::RuntimeError;
            Err(RuntimeError(
                "Could not cast object to concrete type".into(),
            ))
        }
    }

    /// Given the internal object table, constructs a concrete object out
    /// of it.
    ///
    /// This is only used internally, which is why it's prefixed with a "_"
    /// Please do not use it outside of object.rs.
    fn _wrap(table: Object<'lua>) -> T;

    fn state(&self) -> rlua::Result<::std::cell::Ref<S>>;

    /// Gets a mutable reference to the internal state for the concrete object.
    fn get_object_mut(&mut self) -> rlua::Result<::std::cell::RefMut<S>>;

    /// Lua objects in Way Cooler are just how they are in Awesome:
    /// * We expose the user data directly.
    /// * State for the object is stored using set_user_value in a "wrapper"
    /// table. * The wrapper table has a data field which hosts the data.
    /// * Class methods/attributes are on the meta table of the wrapper table.
    fn allocate(lua: &'lua Lua, class: Class) -> rlua::Result<ObjectBuilder<'lua>> {
        let object = lua.create_userdata(S::default())?;
        // TODO Increment the instance count
        let wrapper_table = lua.create_table()?;
        let data_table = lua.create_table()?;
        wrapper_table.set("data", data_table)?;
        let meta = lua.create_table()?;
        meta.set("__class", class)?;
        meta.set("properties", Vec::<Property>::new().to_lua(lua)?)?;
        meta.set("signals", lua.create_table()?)?;
        meta.set("connect_signal", lua.create_function(connect_signal)?)?;
        meta.set("disconnect_signal", lua.create_function(disconnect_signal)?)?;
        meta.set("emit_signal", lua.create_function(emit_signal)?)?;
        meta.set("__index", meta.clone())?;
        meta.set(
            "__tostring",
            lua.create_function(|_, data: AnyUserData| Ok(format!("{}", data.borrow::<S>()?)))?,
        )?;
        wrapper_table.set_metatable(Some(meta));
        object.set_user_value(wrapper_table)?;
        // TODO Emit new signal event
        let object = Object { object };
        Ok(ObjectBuilder { object, lua })
    }
}

impl<'lua> ToLua<'lua> for Object<'lua> {
    fn to_lua(self, _: &'lua Lua) -> rlua::Result<Value<'lua>> {
        Ok(Value::UserData(self.object))
    }
}

impl<'lua> Object<'lua> {
    pub fn signals(&self) -> rlua::Result<rlua::Table<'lua>> {
        self.table()?.get::<_, Table>("signals")
    }

    pub fn table(&self) -> rlua::Result<Table<'lua>> {
        self.object.get_user_value::<Table<'lua>>()
    }
}

/// Can be used for implementing UserData for Lua objects. This provides some
/// default metafunctions.
pub fn default_add_methods<S>(methods: &mut UserDataMethods<S>)
where
    S: UserData,
{
    methods.add_meta_function(MetaMethod::Index, default_index);
    methods.add_meta_function(MetaMethod::NewIndex, default_newindex);
    methods.add_meta_function(MetaMethod::ToString, default_tostring);
}

/// Default indexing of an Awesome object.
///
/// Automatically looks up contents in meta table, so instead of overriding this
/// it's easier to just add the required data in the meta table.
pub fn default_index<'lua>(
    lua: &'lua Lua,
    (obj, index): (AnyUserData<'lua>, Value<'lua>),
) -> rlua::Result<Value<'lua>> {
    // Look up in metatable first
    let obj: Object<'lua> = obj.into();
    let obj_table = obj.table()?;
    let meta = obj_table.get_metatable().expect("Object had no metatable");
    if meta.get::<_, AnyUserData>("__class").is_ok() {
        if let Ok(val) = meta.raw_get::<_, Value>(index.clone()) {
            match val {
                Value::Nil => {}
                val => return Ok(val),
            }
        }
    }
    let index = match String::from_lua(index, lua) {
        Ok(s) => s,
        Err(_) => return Ok(Value::Nil),
    };
    match index.as_str() {
        "valid" => Ok(Value::Boolean(
            if let Ok(class) = meta.get::<_, AnyUserData>("__class") {
                let class: Class = class.into();
                class.checker()?.map(|checker| checker(obj)).unwrap_or(true)
            } else {
                false
            },
        )),
        "data" => obj_table.to_lua(lua),
        index => {
            // Try see if there is a property of the class with the name
            if let Ok(class) = meta.get::<_, AnyUserData>("__class") {
                let class_table = class.get_user_value::<Table>()?;
                let props = class_table.get::<_, Vec<Property>>("properties")?;
                for prop in props {
                    if prop.name.as_str() == index {
                        // Property exists and has an index callback
                        if let Some(index) = prop.cb_index {
                            return index.call(obj);
                        }
                    }
                }
                if let Some(meta) = class_table.get_metatable() {
                    match meta.get::<_, Function>("__index_miss_handler") {
                        Ok(function) => return function.bind(obj)?.call(index),
                        Err(_) => {}
                    }
                }
            }
            // TODO property miss handler if index doesn't exst
            Ok(rlua::Value::Nil)
        }
    }
}

/// Default new indexing (assignment) of an Awesome object.
///
/// Automatically looks up contents in meta table, so instead of overriding this
/// it's easier to just add the required data in the meta table.
pub fn default_newindex<'lua>(
    _: &'lua Lua,
    (obj, index, val): (AnyUserData<'lua>, String, Value<'lua>),
) -> rlua::Result<Value<'lua>> {
    // Look up in metatable first
    let obj: Object<'lua> = obj.into();
    let obj_table = obj.table()?;
    if let Some(meta) = obj_table.get_metatable() {
        if let Ok(val) = meta.raw_get::<_, Value>(index.clone()) {
            match val {
                Value::Nil => {}
                val => return Ok(val),
            }
        }
        let class = meta.get::<_, AnyUserData>("__class")?;
        let class_table = class.get_user_value::<Table>()?;
        let props = class_table.get::<_, Vec<Property>>("properties")?;
        for prop in props {
            if prop.name.as_str() == index {
                // Property exists and has a newindex callback
                if let Some(newindex) = prop.cb_newindex {
                    return newindex.bind(obj.clone())?.call(val);
                }
            }
        }
        if let Some(meta) = class_table.get_metatable() {
            match meta.get::<_, Function>("__newindex_miss_handler") {
                Ok(function) => return function.bind(obj)?.call((index, val)),
                Err(_) => {}
            }
        }
        // TODO property miss handler if index doesn't exst
    }
    Ok(Value::Nil)
}

pub fn default_tostring<'lua>(_: &'lua Lua, obj: AnyUserData<'lua>) -> rlua::Result<String> {
    let obj: Object<'lua> = obj.into();
    let obj_table = obj.table()?;
    if let Some(meta) = obj_table.get_metatable() {
        let class = meta.get::<_, AnyUserData>("__class")?;
        let class_table = class.get_user_value::<Table>()?;
        let name = class_table.get::<_, String>("name")?;
        return Ok(format!("{}: {:p}", name, &obj.object as *const _));
    }
    Err(rlua::Error::UserDataTypeMismatch)
}

fn connect_signal(
    lua: &Lua,
    (obj, signal, func): (AnyUserData, String, Function),
) -> rlua::Result<()> {
    signal::connect_signal(lua, obj.into(), signal, &[func])
}

fn disconnect_signal(lua: &Lua, (obj, signal): (AnyUserData, String)) -> rlua::Result<()> {
    signal::disconnect_signal(lua, obj.into(), signal)
}

fn emit_signal(lua: &Lua, (obj, signal, args): (AnyUserData, String, Value)) -> rlua::Result<()> {
    signal::emit_object_signal(lua, obj.into(), signal, args)
}
