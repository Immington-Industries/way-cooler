//! Utilities to talk to Lua

use wlroots::{
    events::{
        key_events::Key,
        pointer_events::{wlr_button_state, BTN_EXTRA, BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, BTN_SIDE},
    },
    xkbcommon::xkb::keysyms::*,
    KeyboardModifier,
};

use rlua::{self, Error::RuntimeError, Lua, Table, Value};

/// Human readable versions of the standard modifier keys.
const MOD_NAMES: [&str; 8] = [
    "Shift", "Caps", "Control", "Alt", "Mod2", "Mod3", "Mod4", "Mod5",
];
/// Keycodes corresponding to various button events.
const MOUSE_EVENTS: [u32; 5] = [BTN_LEFT, BTN_RIGHT, BTN_MIDDLE, BTN_SIDE, BTN_EXTRA];

const MOD_TYPES: [(KeyboardModifier, Key); 7] = [
    (KeyboardModifier::WLR_MODIFIER_SHIFT, KEY_Shift_L),
    (KeyboardModifier::WLR_MODIFIER_CAPS, KEY_Caps_Lock),
    (KeyboardModifier::WLR_MODIFIER_CTRL, KEY_Control_L),
    (KeyboardModifier::WLR_MODIFIER_ALT, KEY_Alt_L),
    (KeyboardModifier::WLR_MODIFIER_MOD2, KEY_Meta_L),
    (KeyboardModifier::WLR_MODIFIER_LOGO, KEY_Super_L),
    (KeyboardModifier::WLR_MODIFIER_MOD5, KEY_Hyper_L),
];

/// Convert a modifier to the Lua interpretation
#[allow(non_upper_case_globals)]
pub fn mods_to_lua<'lua>(lua: &'lua Lua, mods: &[Key]) -> rlua::Result<Table<'lua>> {
    let mut mods_list: Vec<String> = Vec::with_capacity(MOD_NAMES.len());
    for modifier in mods {
        mods_list.push(
            match *modifier {
                KEY_Shift_L | KEY_Shift_R => "Shift",
                KEY_Control_L | KEY_Control_R => "Control",
                KEY_Caps_Lock => "Caps",
                KEY_Alt_L | KEY_Alt_R => "Alt",
                KEY_Meta_L | KEY_Meta_R => "Mod2",
                KEY_Super_L | KEY_Super_R => "Mod4",
                _ => continue,
            }.into(),
        );
    }
    lua.create_table_from(mods_list.into_iter().enumerate())
}

/// Convert a single number to a modifier list.
pub fn num_to_mods(modifiers: KeyboardModifier) -> Vec<Key> {
    let mut res = vec![];
    for (mod_km, mod_k) in MOD_TYPES.iter() {
        if (mod_km.clone() & modifiers) != KeyboardModifier::empty() {
            res.push(mod_k.clone());
        }
    }
    res
}

/// Convert a modifier list to a single number.
#[allow(non_upper_case_globals)]
pub fn mods_to_num(modifiers: Table) -> rlua::Result<KeyboardModifier> {
    let mut res = KeyboardModifier::empty();
    for modifier in mods_to_rust(modifiers)? {
        res.insert(match modifier {
            KEY_Shift_L => KeyboardModifier::WLR_MODIFIER_SHIFT,
            KEY_Caps_Lock => KeyboardModifier::WLR_MODIFIER_CAPS,
            KEY_Control_L => KeyboardModifier::WLR_MODIFIER_CTRL,
            KEY_Alt_L => KeyboardModifier::WLR_MODIFIER_ALT,
            KEY_Meta_L => KeyboardModifier::WLR_MODIFIER_MOD2,
            KEY_Super_L => KeyboardModifier::WLR_MODIFIER_LOGO,
            KEY_Hyper_L => KeyboardModifier::WLR_MODIFIER_MOD5,
            k => {
                error!("Unknown modifier {:?}", k);
                panic!("Unknown mod");
            }
        });
    }
    Ok(res)
}

/// Convert a modifier to the Rust interpretation, from the Lua interpretation
pub fn mods_to_rust(mods_table: Table) -> rlua::Result<Vec<Key>> {
    let mut mods = Vec::with_capacity(MOD_NAMES.len());
    for modifier in mods_table.pairs::<Value, String>() {
        mods.push(match &*modifier?.1 {
            "Shift" => KEY_Shift_L,
            "Caps" | "Lock" => KEY_Caps_Lock,
            "Control" | "Ctrl" => KEY_Control_L,
            "Alt" | "Mod1" => KEY_Alt_L,
            "Mod2" => KEY_Meta_L,
            "Mod3" => KEY_Alt_L,
            "Mod4" => KEY_Super_L,
            "Mod5" => KEY_Hyper_L,
            string => return Err(RuntimeError(format!("{} is an invalid modifier", string)))?,
        })
    }
    Ok(mods)
}

/// Convert a mouse event from Wayland to the representation Lua expcets
pub fn mouse_events_to_lua(
    _: &rlua::Lua,
    button: u32,
    button_state: wlr_button_state,
) -> rlua::Result<Vec<bool>> {
    let mut event_list = Vec::with_capacity(MOUSE_EVENTS.len());
    for mouse_event in &MOUSE_EVENTS[..5] {
        let state_pressed = button_state == wlr_button_state::WLR_BUTTON_PRESSED;
        let is_pressed = button == *mouse_event && state_pressed;
        event_list.push(is_pressed);
    }
    Ok(event_list)
}
