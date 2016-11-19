//! Contains information for keybindings.

use std::collections::HashMap;
use std::sync::RwLock;

use rustwlc::types::*; // Need * for bitflags...

mod keypress;
pub use self::keypress::KeyPress;

mod event;
pub use self::event::KeyEvent;

lazy_static! {
    static ref BINDINGS: RwLock<HashMap<KeyPress, KeyEvent>> =
        RwLock::new(HashMap::new());

    static ref NAME_MAPPING: HashMap<&'static str, &'static str> = {
        let mut map = HashMap::new();
        map.insert("enter", "return");
        map.insert("\n", "return");
        map.insert("\t", "tab");
        map
    };
}

/// Parses a KeyMod from key names.
pub fn keymod_from_names(keys: &[&str]) -> Result<KeyMod, String> {
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
    return Ok(result)
}

#[allow(deprecated)] // keysyms
pub fn init() {
    use rustwlc::xkb::keysyms;
    use commands;
    register(KeyPress::new(MOD_ALT | MOD_SHIFT, keysyms::KEY_Escape),
             KeyEvent::Command(commands::get("quit")
                               .expect("Error reading commands::quit")));
}

/// Get a key mapping from the list.
pub fn get(key: &KeyPress) -> Option<KeyEvent> {
    let bindings = BINDINGS.read()
        .expect("Keybindings/get: unable to lock keybindings");
    bindings.get(key).map(KeyEvent::clone)
}

/// Register a new set of key mappings
pub fn register(key: KeyPress, event: KeyEvent) -> Option<KeyEvent> {
    let mut bindings = BINDINGS.write()
        .expect("Keybindings/register: unable to lock keybindings");
    bindings.insert(key, event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn test_cmd() {
        assert!(true);
    }

    fn keypress() -> KeyPress {
        keypress!("Ctrl", "t")
    }

    #[test]
    fn add_key() {
        require_rustwlc!();
        register(keypress(), KeyEvent::Command(Arc::new(test_cmd)));
        assert!(get(&keypress()).is_some(), "Key not registered");
    }
}
