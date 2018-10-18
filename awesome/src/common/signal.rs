//! Signals are a list of methods associated with a name that will be called
//! when the signal is triggered.
//!
//! Signals are stored with the object in its metatable,
//! the methods defined here are just to make it easier to use.

use rlua::{self, Function, Lua, Table, ToLua, ToLuaMulti, Value};

use common::object::Object;
use GLOBAL_SIGNALS;

/// Connects functions to a signal. Creates a new entry in the table if it
/// doesn't exist.
pub fn connect_signal(
    lua: &Lua,
    obj: Object,
    name: String,
    funcs: &[Function],
) -> rlua::Result<()> {
    let signals = obj.signals()?;
    connect_signals(lua, signals, name, funcs)
}

fn connect_signals<'lua>(
    lua: &'lua Lua,
    signals: Table<'lua>,
    name: String,
    funcs: &[Function],
) -> rlua::Result<()> {
    if let Ok(Value::Table(table)) = signals.get::<_, Value>(name.as_str()) {
        let mut length = table.len()? + 1;
        for func in funcs {
            table.set(length, func.clone())?;
            length += 1;
        }
        Ok(())
    } else {
        let table = lua.create_table()?;
        for (index, func) in funcs.into_iter().enumerate() {
            table.set(index + 1, func.clone())?;
        }
        signals.set(name, table)
    }
}

pub fn disconnect_signal(lua: &Lua, obj: Object, name: String) -> rlua::Result<()> {
    let signals = obj.signals()?;
    disconnect_signals(lua, signals, name)
}

fn disconnect_signals(_: &Lua, signals: Table, name: String) -> rlua::Result<()> {
    signals.set(name, Value::Nil)
}

/// Evaluate the functions associated with a signal.
pub fn emit_object_signal<'lua, A>(
    lua: &'lua Lua,
    obj: Object<'lua>,
    name: String,
    args: A,
) -> rlua::Result<()>
where
    A: ToLuaMulti<'lua> + Clone,
{
    let signals = obj.signals()?;
    let mut args = args.to_lua_multi(lua)?;
    args.push_front(obj.to_lua(lua)?);
    emit_signals(lua, signals, name, args)
}

fn emit_signals<'lua, A>(
    _: &'lua Lua,
    signals: Table<'lua>,
    name: String,
    args: A,
) -> rlua::Result<()>
where
    A: ToLuaMulti<'lua> + Clone,
{
    if let Ok(Value::Table(table)) = signals.get::<_, Value>(name.clone()) {
        for entry in table.pairs::<Value, Function>() {
            if let Ok((_, func)) = entry {
                match func.call(args.clone()) {
                    Ok(()) => {}
                    Err(e) => {
                        error!("Error while emitting signal {}: {}", name, e);
                    }
                };
            }
        }
    }
    Ok(())
}

/// Connect the function to the named signal in the global signal list.
pub fn global_connect_signal<'lua>(
    lua: &'lua Lua,
    (name, func): (String, rlua::Function<'lua>),
) -> rlua::Result<()> {
    let global_signals = lua.named_registry_value::<Table>(GLOBAL_SIGNALS)?;
    connect_signals(lua, global_signals, name, &[func])
}

/// Disconnect the function from the named signal in the global signal list.
pub fn global_disconnect_signal<'lua>(lua: &'lua Lua, name: String) -> rlua::Result<()> {
    let global_signals = lua.named_registry_value::<Table>(GLOBAL_SIGNALS)?;
    disconnect_signals(lua, global_signals, name)
}

/// Emit the signal with the given name from the global signal list.
pub fn global_emit_signal<'lua>(lua: &'lua Lua, (name, args): (String, Value)) -> rlua::Result<()> {
    let global_signals = lua.named_registry_value::<Table>(GLOBAL_SIGNALS)?;
    emit_signals(lua, global_signals, name, args)
}
