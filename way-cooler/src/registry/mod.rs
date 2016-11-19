//! way-cooler registry.

use std::ops::Deref;
use std::collections::hash_map::{HashMap, Entry};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use rustc_serialize::Decodable;
use rustc_serialize::json::{Json, ToJson, Decoder, DecoderError};

mod types;
pub use self::types::*; // Export constants too

#[cfg(test)]
pub mod tests;

pub type RegMap = HashMap<String, RegistryField>;

#[cfg(not(test))]
#[inline]
fn new_map() -> RegMap {
    HashMap::new()
}

#[cfg(test)]
#[inline]
fn new_map() -> RegMap {
    self::tests::registry_map()
}

lazy_static! {
    /// Static HashMap for the registry
    static ref REGISTRY: RwLock<RegMap> = RwLock::new(new_map());
}

/// Error types that can happen
#[derive(Debug, PartialEq)]
pub enum RegistryError {
    /// The registry key was not found
    KeyNotFound,
    /// Attempting to set a readonly value
    InvalidOperation,
}

/// Result type of gets/sets to the registry
pub type RegistryResult<T> = Result<T, RegistryError>;

/// Initialize the registry (register default API)
pub fn init() {
    use layout::tree_as_json as get_json;
    insert_property("tree_layout".to_string(), Some(Arc::new(get_json)), None);
}

/// Acquires a read lock on the registry.
pub fn read_lock<'a>() -> RwLockReadGuard<'a, RegMap> {
    REGISTRY.read().expect("Unable to read from registry!")
}

/// Acquires a write lock on the registry.
pub fn write_lock<'a>() -> RwLockWriteGuard<'a, RegMap> {
    REGISTRY.write().expect("Unable to write to registry!")
}

/// Gets a RegistryField enum with the specified name
pub fn get_field(name: &str) -> Option<RegistryField> {
    // clone() will either clone an Arc or an Arc+AccessFlags
    let result = read_lock().get(name).map(|v| v.clone());
    trace!("get: {} => {:?}", name, &result);
    return result;
}

/// Gets a data type from the registry, returning a reference to the property
/// method if the field is a property.
pub fn get_data(name: &str) -> RegistryResult<RegistryGetData> {
    get_field(name).ok_or(RegistryError::KeyNotFound)
        .and_then(|val| match val {
            RegistryField::Object { flags, data } =>
                Ok(RegistryGetData::Object(flags, data)),
            RegistryField::Property { get: maybe_get, set: maybe_set } =>
                match maybe_get {
                    Some(get) => {
                        let mut flags = AccessFlags::READ();
                        if maybe_set.is_some() {
                            flags.insert(AccessFlags::WRITE());
                        }
                        Ok(RegistryGetData::Property(flags, get))
                    }
                    None => Err(RegistryError::InvalidOperation)
                },
        })
}

/// Get a Rust structure from the registry
#[allow(dead_code)]
pub fn get_struct<T>(name: &str)
                     -> RegistryResult<(AccessFlags, Result<T, DecoderError>)>
where T: Decodable {
    get_data(name).map(|data| data.resolve())
        .map(|(flags, json)|
             (flags, T::decode(&mut Decoder::new(json.deref().clone()))))
}

/// Set a value to the given registry field.
pub fn insert_field(key: String, value: RegistryField) -> Option<RegistryField> {
    let mut reg = write_lock();
    reg.insert(key, value)
}

/// Add the json object to the registry
pub fn insert_json(key: String, flags: AccessFlags, value: Json)
                   -> Option<RegistryField> {
    write_lock().insert(key,
                        RegistryField::Object {
                            flags: flags,
                            data: Arc::new(value)
                        })
}

/// Set a value to the given JSON value.
pub fn set_json(key: String, json: Json) -> RegistryResult<RegistrySetData> {
    let mut reg = write_lock();
    match reg.entry(key) {
        Entry::Vacant(_vacancy) => {
            return Err(RegistryError::KeyNotFound)
        },
        Entry::Occupied(mut entry) => {
            let first_type = entry.get().get_type();
            let flags = entry.get().get_flags();
            if !flags.contains(AccessFlags::WRITE()) {
                return Err(RegistryError::InvalidOperation)
            }
            if first_type == FieldType::Object {
                return Ok(RegistrySetData::Displaced(
                    entry.insert(RegistryField::Object {
                        flags: flags, data: Arc::new(json)
                    }).as_object()
                        .expect("Just created object").1));
            }
            else if first_type == FieldType::Property {
                match entry.get().clone().as_property_set() {
                    Some(func) =>
                        return Ok(RegistrySetData::Property(flags, func)),
                    // None: should not happen
                    None => return Err(RegistryError::InvalidOperation)
                }
            }
            else {
                return Err(RegistryError::InvalidOperation);
            }
        }
    }
}

/// Set an object/property in the registry to a value using a ToJson.
#[allow(dead_code)]
pub fn insert_struct<T: ToJson>(key: String, flags: AccessFlags, value: T)
                             -> Option<RegistryField> {
    insert_field(key, RegistryField::Object {
        flags: flags,
        data: Arc::new(value.to_json()) })
}


/// Binds properties to a field of the registry
pub fn insert_property(key: String, get_fn: Option<GetFn>, set_fn: Option<SetFn>)
                          -> Option<RegistryField> {
    insert_field(key, RegistryField::Property { get: get_fn, set: set_fn })
}

/// Gets access flags and field type of the given key.
///
/// Returns `None` if the key does not exist.
pub fn key_info(key: &str) -> Option<(FieldType, AccessFlags)> {
    trace!("key_info: {}", key);
    let read_reg = read_lock();
    read_reg.get(key).map(|field| (field.get_type(), field.get_flags()))
}
