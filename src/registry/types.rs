//! Types used in the registry.

use std::sync::Arc;

use rustc_serialize::json::{Json, ToJson};

/// How much access things have
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RegistryAccess {
    Public,
    Lua,
    Private
}

/// Values stored in the registry
#[derive(Debug)]
pub struct RegistryValue {
    access: RegistryAccess,
    object: Arc<Json>
}

impl RegistryValue {
    /// Creates a new RegistryValue
    pub fn new<T>(access: RegistryAccess, data: T) -> RegistryValue
        where T: ToJson  {
        RegistryValue {
            access: access,
            object: Arc::new(data.to_json())
        }
    }

    /// What access the module has to it
    pub fn access(&self) -> RegistryAccess {
        self.access
    }

    /// Gets the json of a registry value
    pub fn get_json(&self) -> Arc<Json> {
        self.object.clone()
    }
}
