//! Contains information for keybindings.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use rustwlc::xkb::{Keysym, NameFlags};
use rustwlc::types::*; // Need * for bitflags...
use std::hash::{Hash, Hasher};

lazy_static! {
    static ref BINDINGS: RwLock<HashMap<KeyPress, KeyEvent>> = {
        let mut map = HashMap::<KeyPress, KeyEvent>::new();

        let press_s = KeyPress::from_key_names(vec!["Mod4"], vec!["s"]).unwrap();
        map.insert(press_s, Arc::new(Box::new(key_s)));

        let press_f4 = KeyPress::from_key_names(vec!["Alt"],vec!["F4"]).unwrap();
        map.insert(press_f4, Arc::new(Box::new(key_f4)));

        let press_k = KeyPress::from_key_names(
            vec!["Ctrl"], vec!["k"]).unwrap();
        map.insert(press_k, Arc::new(Box::new(key_sleep)));

        let press_p = KeyPress::from_key_names(vec!["Ctrl"], vec!["p"]).unwrap();
        map.insert(press_p, Arc::new(Box::new(key_pointer_pos)));

        RwLock::new(map)
    };
}

fn key_sleep() {
    use std::thread;
    use std::time::Duration;

    use super::lua;
    use lua::LuaQuery;

    info!("keyhandler: Beginning thread::sleep keypress!");
    lua::try_send(LuaQuery::Execute("print('>entering sleep')\
                                 os.execute('sleep 5')\
                                 print('>leaving sleep')".to_string()))
                  .unwrap();
    info!("keyhandler: Finished thread::sleep keypress!");
}

fn key_pointer_pos() {
    use super::lua;
    use lua::LuaQuery;
    let code = "if wm == nil then print('wm table does not exist')\n\
                elseif wm.pointer == nil then print('wm.pointer table does not exist')\n\
                else\n\
                print('get_position is a func, preparing execution')
                local x, y = wm.pointer.get_position()\n\
                print('The cursor is at ' .. x .. ', ' .. y)\n\
                end".to_string();
    lua::try_send(LuaQuery::Execute(code)).unwrap();
}

fn key_s() {
    info!("[Key handler] S keypress!");
}

fn key_f4() {
    info!("[Key handler] F4 keypress!");
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct KeyPress {
    modifiers: KeyMod,
    keys: Vec<Keysym>
}

/// Parses a KeyMod from key names.
pub fn keymod_from_names(keys: Vec<&str>) -> Result<KeyMod, String> {
    let mut result = KeyMod::empty();
    for key in keys {
        match key.to_lowercase().as_str() {
            "shift"            => result = result | MOD_SHIFT,
            "control" | "ctrl" => result = result | MOD_CTRL,
            "alt"              => result = result | MOD_ALT,
            "mod2"             => result = result | MOD_MOD2,
            "mod3"             => result = result | MOD_MOD3,
            "mod4" | "super" | "logo" => result = result | MOD_MOD4,
            "mod5" | "5mod5me" => result = result | MOD_MOD5,
            err => return Err(format!("Invalid modifier: {}", err))
        }
    }
    return Ok(result);
}

impl KeyPress {
    /// Creates a new KeyPress struct from a list of modifier and key names
    pub fn from_key_names(mods: Vec<&str>, keys: Vec<&str>) -> Result<KeyPress, String> {
        match keymod_from_names(mods) {
            Err(message) => Err(message),
            Ok(mods) => {
                let mut syms: Vec<Keysym> = Vec::with_capacity(keys.len());
                for name in keys {
                    if let Some(sym) = Keysym::from_name(name.to_string(),
                                                         NameFlags::None) {
                            syms.push(sym);
                    }
                    // Else if could not parse
                    else if let Some(sym) = Keysym::from_name(name.to_string(),
                                                        NameFlags::CaseInsensitive) {
                        syms.push(sym);
                    }
                    else {
                        return Err(format!("Invalid key: {}", name));
                    }
                }
                syms.sort_by_key(|s| s.get_code());
                syms.dedup();
                return Ok(KeyPress { modifiers: mods, keys: syms });
            }
        }
    }

    /// Creates a KeyPress from keys that are pressed at the moment
    pub fn new(mods: KeyMod, mut keys: Vec<Keysym>) -> KeyPress {
        keys.sort_by_key(|k| k.get_code());
        keys.dedup();

        KeyPress { modifiers: mods, keys: keys }
    }
}

impl Hash for KeyPress {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        hasher.write_u32(self.modifiers.bits());
        for ref sym in &self.keys {
            hasher.write_u32(sym.get_code());
        }
    }
}

/// The type of function a key press handler is.
pub type KeyEvent = Arc<Box<Fn() + Send + Sync>>;

/// Get a key mapping from the list.
pub fn get(key: &KeyPress) -> Option<KeyEvent> {
    let bindings = BINDINGS.read().unwrap();
    match bindings.get(key) {
        None => None,
        Some(val) => Some(val.clone())
    }
}

/// Register a new key mapping
pub fn register(values: Vec<(KeyPress, KeyEvent)>) {
    let mut bindings = BINDINGS.write().unwrap();
    for value in values {
        bindings.insert(value.0, value.1);
    }
}
