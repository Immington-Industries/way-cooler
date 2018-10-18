//! AwesomeWM Keygrabber interface

use rlua::{self, Function, Lua, Table, Value};
use wlroots::{
    events::key_events::{Key, KeyEvent},
    wlr_key_state,
    xkbcommon::xkb::keysym_get_name,
    KeyboardModifier, WLR_KEY_PRESSED,
};

use common::{
    object::{Object, Objectable},
    signal,
};
use objects::key;
use {lua, root::ROOT_KEYS_HANDLE, LUA};

pub const KEYGRABBER_TABLE: &str = "keygrabber";
const KEYGRABBER_CALLBACK: &str = "__callback";

/// Init the methods defined on this interface.
pub fn init(lua: &Lua) -> rlua::Result<()> {
    let keygrabber_table = lua.create_table()?;
    let meta = lua.create_table()?;
    meta.set("__index", lua.create_function(index)?)?;
    meta.set("__newindex", lua.create_function(new_index)?)?;
    keygrabber_table.set("run", lua.create_function(run)?)?;
    keygrabber_table.set("stop", lua.create_function(stop)?)?;
    keygrabber_table.set("isrunning", lua.create_function(isrunning)?)?;
    keygrabber_table.set_metatable(Some(meta));
    let globals = lua.globals();
    globals.set(KEYGRABBER_TABLE, keygrabber_table)
}

/// Given the current input, handle calling the Lua defined callback if it is
/// defined with the input.
pub fn keygrabber_handle(mods: Vec<Key>, sym: Key, state: wlr_key_state) -> rlua::Result<()> {
    LUA.with(|lua| {
        let lua = lua.borrow();
        let lua_state = if state == wlr_key_state::WLR_KEY_PRESSED {
            "press"
        } else {
            "release"
        }.into();
        let lua_sym = keysym_get_name(sym);
        let lua_mods = ::lua::mods_to_lua(&*lua, &mods)?;
        let res = call_keygrabber(&*lua, (lua_mods, lua_sym, lua_state));
        match res {
            Ok(_) | Err(rlua::Error::FromLuaConversionError { .. }) => Ok(()),
            err => err,
        }
    })
}

/// Check is the Lua callback function is set
pub fn is_keygrabber_set(lua: &Lua) -> bool {
    lua.named_registry_value::<Function>(KEYGRABBER_CALLBACK)
        .is_ok()
}

/// Call the Lua callback function for when a key is pressed.
pub fn call_keygrabber(lua: &Lua, (mods, key, event): (Table, String, String)) -> rlua::Result<()> {
    let lua_callback = lua.named_registry_value::<Function>(KEYGRABBER_CALLBACK)?;
    lua_callback.call((mods, key, event))
}

fn run(lua: &Lua, function: rlua::Function) -> rlua::Result<()> {
    match lua.named_registry_value::<Value>(KEYGRABBER_CALLBACK)? {
        Value::Function(_) => Err(rlua::Error::RuntimeError(
            "keygrabber callback already set!".into(),
        )),
        _ => lua.set_named_registry_value(KEYGRABBER_CALLBACK, function),
    }
}

fn stop(lua: &Lua, _: ()) -> rlua::Result<()> {
    lua.set_named_registry_value(KEYGRABBER_CALLBACK, Value::Nil)
}

fn isrunning(lua: &Lua, _: ()) -> rlua::Result<bool> {
    match lua.named_registry_value::<Value>(KEYGRABBER_CALLBACK)? {
        Value::Function(_) => Ok(true),
        _ => Ok(false),
    }
}

fn index(lua: &Lua, args: Value) -> rlua::Result<()> {
    signal::global_emit_signal(lua, ("debug::index::miss".into(), args))
}

fn new_index(lua: &Lua, args: Value) -> rlua::Result<()> {
    signal::global_emit_signal(lua, ("debug::newindex::miss".into(), args))
}

/// Emits the Awesome keybindinsg.
fn emit_awesome_keybindings(
    lua: &Lua,
    event: &KeyEvent,
    event_modifiers: KeyboardModifier,
) -> rlua::Result<()> {
    let state_string = if event.key_state() == WLR_KEY_PRESSED {
        "press"
    } else {
        "release"
    };
    // If keygrabber is set, grab key
    // TODO check behavior when event.pressed_keys() isn't a singleton
    if is_keygrabber_set(&*lua) {
        for event_keysym in event.pressed_keys() {
            let res = call_keygrabber(
                &*lua,
                (
                    lua::mods_to_lua(&*lua, &lua::num_to_mods(event_modifiers))?,
                    keysym_get_name(event_keysym),
                    state_string.into(),
                ),
            );
            if let Err(err) = res {
                warn!("Call to keygrabber failed for {}: {:?}", event_keysym, err);
            }
        }
    } else {
        // TODO Should also emit by current focused client so we can
        // do client based rules.
        let keybindings = lua.named_registry_value::<Vec<rlua::AnyUserData>>(ROOT_KEYS_HANDLE)?;
        for event_keysym in event.pressed_keys() {
            for binding in &keybindings {
                let obj: Object = binding.clone().into();
                let key = key::Key::cast(obj.clone()).unwrap();
                let keycode = key.keycode()?;
                let keysym = key.keysym()?;
                let modifiers = key.modifiers()?;
                let binding_match = ((keysym != 0 && keysym == event_keysym)
                    || (keycode != 0 && keycode == event.keycode()))
                    && (modifiers == 0 || modifiers == event_modifiers.bits());
                if binding_match {
                    if let Err(err) =
                        signal::emit_object_signal(&*lua, obj, state_string.into(), ())
                    {
                        warn!("Could not emit the signal for {}: {:?}", keysym, err);
                    }
                }
            }
        }
    }
    Ok(())
}
