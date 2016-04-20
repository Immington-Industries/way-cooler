//! way-cooler registry.

use std::ops::Deref;
use std::cmp::Eq;
use std::fmt::Display;
use std::hash::Hash;
use std::borrow::Borrow;

use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use rustc_serialize::{Decodable, json};
use rustc_serialize::json::{Json, ToJson};

mod types;
pub use self::types::{RegistryAccess, RegistryValue};

#[cfg(test)]
mod tests;

pub type RegMap = HashMap<String, RegistryValue>;

lazy_static! {
    /// Registry variable for the registry
    static ref REGISTRY: RwLock<RegMap> =
        RwLock::new(HashMap::new());
}

/// Error types that can happen
#[derive(Debug, PartialEq, Eq)]
pub enum RegistryError {
    /// The value in the registry could not be parsed
    InvalidJson,
    /// The registry key was not found
    KeyNotFound
}

/// Acquires a read lock on the registry.
pub fn read_lock<'a>() -> RwLockReadGuard<'a, RegMap> {
    REGISTRY.read().unwrap()
}

/// Acquires a write lock on the registry.
pub fn write_lock<'a>() -> RwLockWriteGuard<'a, RegMap> {
    REGISTRY.write().unwrap()
}

/// Gets a Json object from a registry key
pub fn get_json<K>(name: &K) -> Option<(RegistryAccess, Arc<Json>)>
where String: Borrow<K>, K: Hash + Eq + Display {
    trace!("get_json: {}", *name);
    let ref reg = *read_lock();
    if let Some(val) = reg.get(name) {
        Some((val.access(), val.get_json()))
    }
    else {
        None
    }
}

/// Gets an object from the regsitry, decoding its internal json
/// representation.
pub fn get<K, T>(name: &K) -> Result<(RegistryAccess, T), RegistryError>
where T: Decodable, String: Borrow<K>, K: Hash + Eq + Display {
    let maybe_json = get_json(name);
    if let Some(json_pair) = maybe_json {
        let (access, json_arc) = json_pair;
        let mut decoder = json::Decoder::new(json_arc.deref().to_json());
        match T::decode(&mut decoder) {
            Ok(val) => Ok((access, val)),
            Err(e) => Err(RegistryError::InvalidJson)
        }
    }
    else {
        Err(RegistryError::KeyNotFound)
    }
}

/// Set a key in the registry to a particular value
pub fn set<T: ToJson>(key: String, val: T) {
    trace!("set: {}", key);
    let ref mut write_reg = *write_lock();
    let regvalue = RegistryValue::new(RegistryAccess::Public, val);
    write_reg.insert(key, regvalue);
}

/// Whether this map contains a key
pub fn contains_key<K>(key: &K) -> bool
where String: Borrow<K>, K: Hash + Eq + Display {
    trace!("contains_key: {}", *key);
    let ref read_reg = *read_lock();
    read_reg.contains_key(key)
}
