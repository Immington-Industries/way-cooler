//! way-cooler commands.

use std::collections::hash_map::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

mod defaults;

#[cfg(test)]
mod tests;

pub type CommandFn = Arc<Fn() + Send + Sync>;

pub type ComMap = HashMap<String, CommandFn>;

#[cfg(not(test))]
#[inline]
fn new_map() -> ComMap {
    HashMap::new()
}

#[cfg(test)]
#[inline]
fn new_map() -> ComMap {
    self::tests::command_map()
}

lazy_static! {
    /// Registry variable for the registry
    static ref COMMANDS: RwLock<ComMap> = RwLock::new(new_map());
}

/// Initialize commands API (register default commands)
pub fn init() {
    defaults::register_defaults();
}

/// Acquires a read lock on the commands map.
pub fn read_lock<'a>() -> RwLockReadGuard<'a, ComMap> {
    COMMANDS.read().expect("Unable to read from commands!")
}

/// Acquires a write lock on the commands map.
pub fn write_lock<'a>() -> RwLockWriteGuard<'a, ComMap> {
    COMMANDS.write().expect("Unable to write to commands!")
}

/// Gets a command from the API
pub fn get(name: &str) -> Option<CommandFn> {
    read_lock().get(name).map(|com| com.clone())
}

/// Gets a command in the API
#[allow(dead_code)] // Used in tests
pub fn set(name: String, val: CommandFn) -> Option<CommandFn> {
    write_lock().insert(name, val)
}
