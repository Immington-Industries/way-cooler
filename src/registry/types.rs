//! Types used in the registry.

use std::sync::Arc;
use std::fmt::{Debug, Formatter};
use std::fmt::Result as FmtResult;

use rustc_serialize::json::{Json, ToJson};

bitflags! {
    /// Access permissions for items in the registry
    pub flags AccessFlags: u8 {
        /// Clients can read/get the data
        const READ    = 1 << 0,
        /// Clients can write/set the data
        const WRITE   = 1 << 1,
    }
}

impl AccessFlags {
    /// Read permissions
    #[inline]
    #[allow(non_snake_case)]
    pub fn READ() -> AccessFlags { READ }

    /// Write permissions
    #[inline]
    #[allow(non_snake_case)]
    pub fn WRITE() -> AccessFlags { WRITE }
}

impl ToJson for AccessFlags {
    fn to_json(&self) -> Json {
        let mut arr = Vec::with_capacity(2);
        if self.contains(READ) {
            arr.push(Json::String("read".to_string()));
        }
        if self.contains(WRITE) {
            arr.push(Json::String("write".to_string()))
        }
        Json::Array(arr)
    }
}

/// Function which will yield an object
pub type GetFn = Arc<Fn() -> Json + Send + Sync>;

/// Function which will set an object
pub type SetFn = Arc<Fn(Json) + Send + Sync>;

/// Enum of types of registry fields
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum FieldType { Object, Property }

/// Data which can be stored in the registry
#[derive(Clone)]
pub enum RegistryField {
    /// An object with permission flags
    Object {
        /// Permission flags for Lua get/setting the value
        flags: AccessFlags,
        /// Data associated with this value
        data: Arc<Json>
    },
    /// A registry value whose get and set maps to other Rust code
    Property {
        /// Method called to set a property
        get: Option<GetFn>,
        /// Method called to set a property
        set: Option<SetFn>
    },
}

/// Result of what can be accessed from a registry value.
#[derive(Clone)]
pub enum RegistryGetData {
    /// An object in the registry
    Object(AccessFlags, Arc<Json>),
    /// Get field of a property in the registry
    Property(AccessFlags, GetFn)
}

/// Result of what can be set to a registry value.
#[derive(Clone)]
pub enum RegistrySetData {
    /// Some data was displaced, here it is
    Displaced(Arc<Json>),
    /// A property was retrieved, you should run it
    Property(AccessFlags, SetFn)
}

impl Debug for RegistryField {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            &RegistryField::Object { ref flags, ref data } =>
                f.debug_struct("RegistryField::Object")
                .field("flags", flags as &Debug)
                .field("data", data as &Debug).finish(),
            &RegistryField::Property { ref get, ref set } => {
                let new_get = match get { &Some(_) => Some(true), &None => None };
                let new_set = match set { &Some(_) => Some(true), &None => None };
                f.debug_struct("RegistryField::Property")
                    .field("get", &new_get)
                    .field("set", &new_set)
                    .finish()
            }
        }
    }
}

impl Debug for RegistryGetData {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            &RegistryGetData::Object(ref flags, ref data) =>
                f.debug_struct("RegistryGetData::Object")
                .field("flags", flags as &Debug)
                .field("data", data as &Debug).finish(),
            &RegistryGetData::Property(ref flags, _) =>
                write!(f, "RegistryGetData::Property({:?})", flags)
        }
    }
}

impl Debug for RegistrySetData {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            &RegistrySetData::Displaced(ref data) =>
                f.debug_struct("RegistrySetData::Displaced")
                .field("data", data as &Debug).finish(),
            &RegistrySetData::Property(_, _) =>
                write!(f, "RegistrySetData::Property(...)")
        }
    }
}

impl ToJson for FieldType {
    fn to_json(&self) -> Json {
        match *self {
            FieldType::Property => Json::String("Property".to_string()),
            FieldType::Object => Json::String("Object".to_string())
        }
    }
}

impl RegistryField {
    /// Converts this RegistryField to maybe an object. Does not call property methods.
    pub fn as_object(self) -> Option<(AccessFlags, Arc<Json>)> {
        match self {
            RegistryField::Object { flags, data } => Some((flags, data)),
            _ => None
        }
    }

    /// Gets this field as maybe a property with maybe a getter and setter.
    pub fn as_property(self) -> Option<(Option<GetFn>, Option<SetFn>)> {
        match self {
            RegistryField::Property { get, set } => Some((get, set)),
            _ => None
        }
    }

    /// Returns the getter, if this field is a property with a getter.
    #[allow(dead_code)]
    pub fn as_property_get(self) -> Option<GetFn> {
        self.as_property().and_then(|(maybe_get, _)| maybe_get)
    }

    /// Returns a setter if this field is a property with a setter.
    pub fn as_property_set(self) -> Option<SetFn> {
        self.as_property().and_then(|(_, maybe_set)| maybe_set)
    }

    /// Gets the type of this registry field
    pub fn get_type(&self) -> FieldType {
        match self {
            &RegistryField::Object { .. }   => FieldType::Object,
            &RegistryField::Property { .. } => FieldType::Property,
        }
    }

    /// Gets the set of AccessFlags needed for a registry field with said
    /// options
    pub fn get_flags(&self) -> AccessFlags {
        match *self {
            RegistryField::Object { ref flags, .. } => flags.clone(),
            RegistryField::Property { ref get, ref set } => {
                let mut flags = AccessFlags::empty();
                if get.is_some() { flags.insert(AccessFlags::READ()) }
                if set.is_some() { flags.insert(AccessFlags::WRITE()) }
                flags
            }
        }
    }
}

impl RegistryGetData {
    /// Collapses the waveform.
    ///
    /// If this is a Json, returns the Json data. If this is a property, runs the
    /// method and returns the output.
    pub fn resolve(self) -> (AccessFlags, Arc<Json>) {
        match self {
            RegistryGetData::Object(flags, data) => (flags, data),
            RegistryGetData::Property(flags, get) =>
                (flags, Arc::new(get()))
        }
    }

    /// Gets the FieldType of this GetData (property or object)
    #[allow(dead_code)]
    pub fn get_type(&self) -> FieldType {
        match self {
            &RegistryGetData::Property(_, _) => FieldType::Property,
            &RegistryGetData::Object(_, _) => FieldType::Object
        }
    }
}

impl RegistrySetData {
    /// If this set data is a property, calls the property
    pub fn call(self, json: Json) {
        match self {
            RegistrySetData::Displaced(_) => (),
            RegistrySetData::Property(_flags, set) => set(json)
        }
    }

    /// Gets the FieldType of this SetData (property or object)
    #[allow(dead_code)]
    pub fn get_type(&self) -> FieldType {
        match *self {
            RegistrySetData::Displaced(_) => FieldType::Object,
            RegistrySetData::Property(_, _) => FieldType::Property
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use rustc_serialize::json::Json;
    use super::*;

    #[test]
    fn registry_field_debug() {
        let field_obj = RegistryField::Object {
            flags: AccessFlags::READ(),
            data: Arc::new(Json::String("foo".to_string()))
        };
        let field_prop = RegistryField::Property { get: None, set: None };

        assert_eq!(format!("{:?}", field_obj),
            "RegistryField::Object { flags: READ, data: String(\"foo\") }");
        assert_eq!(format!("{:?}", field_prop),
                   "RegistryField::Property { get: None, set: None }");
    }

    #[test]
    fn registry_get_data_debug() {
        let get_obj = RegistryGetData::Object(
            AccessFlags::READ(), Arc::new(Json::String("foo".to_string())));
        assert_eq!(format!("{:?}", get_obj),
              "RegistryGetData::Object { flags: READ, data: String(\"foo\") }");
    }

    #[test]
    fn registry_field() {
        let prop = RegistryField::Property {
            get: None, set: None
        };
        assert_eq!(prop.get_type(), FieldType::Property);

        let null_field = RegistryField::Object {
            flags: AccessFlags::READ(),
            data: Arc::new(Json::Null)
        };

        /* Set Data */
        assert_eq!(null_field.get_type(), FieldType::Object);
        let null_data = null_field.clone().as_object().unwrap().1;
        assert_eq!(*null_data, Json::Null);
        let prop = RegistrySetData::Displaced(null_data);
        assert_eq!(prop.get_type(), FieldType::Object);

        // send function
        fn send(_json: Json) {}

        let set_prop = RegistrySetData::Property(AccessFlags::WRITE(),
                                                 Arc::new(send));
        assert_eq!(set_prop.get_type(), FieldType::Property);

        /* Get Data */
        assert_eq!(null_field.get_type(), FieldType::Object);
        let null_data = null_field.as_object().unwrap().1;
        assert_eq!(*null_data, Json::Null);
        let prop = RegistryGetData::Object(AccessFlags::READ(),
                                           null_data.clone());
        assert_eq!(prop.get_type(), FieldType::Object);

        // send function
        fn _get() -> Json { panic!()}

        let set_prop = RegistryGetData::Property(AccessFlags::WRITE(),
                                                 Arc::new(_get));
        assert_eq!(set_prop.get_type(), FieldType::Property);
    }
}
