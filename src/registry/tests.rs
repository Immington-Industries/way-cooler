//! Tests for the registry

use std::sync::Arc;
use std::collections::HashMap;

use rustc_serialize::json::{Json, ToJson};

use registry;
use registry::{RegMap, RegistryField, GetFn, SetFn,
               AccessFlags, FieldType};

/// Gets the initial HashMap used by the registry
pub fn registry_map() -> RegMap {
    let mut map = HashMap::new();

    let values: Vec<(&str, AccessFlags, Json)> = vec![
        ("bool",  AccessFlags::all(), BOOL.to_json()),
        ("u64",   AccessFlags::all(), U64.to_json()),
        ("i64",   AccessFlags::all(), I64.to_json()),
        ("f64",   AccessFlags::all(), F64.to_json()),
        ("text",  AccessFlags::all(), TEXT.to_json()),
        ("point", AccessFlags::all(), POINT.to_json()),
        ("null",  AccessFlags::all(), Json::Null),
        ("u64s",  AccessFlags::all(), Json::Array(u64s())),

        ("readonly",  AccessFlags::READ(),  READONLY.to_json()),
        ("writeonly", AccessFlags::WRITE(), WRITEONLY.to_json()),
        ("noperms",   AccessFlags::empty(), NO_PERMS.to_json())
    ];

    for (name, flags, json) in values.into_iter() {
        assert!(map.insert(name.to_string(),
                           RegistryField::Object {
                               flags: flags,
                               data: Arc::new(json)
                           }).is_none(), "Duplicate element inserted!");
    }

    let props: Vec<(&str, Option<GetFn>, Option<SetFn>)> = vec![
        ("prop", Some(Arc::new(get_prop)),
         Some(Arc::new(set_prop))),
        ("get_prop", Some(Arc::new(get_prop)), None),
        ("set_prop", None, Some(Arc::new(set_prop))),

        ("get_panic_prop", Some(Arc::new(get_panic_prop)), None),
        ("set_panic_prop", None, Some(Arc::new(set_panic_prop))),
    ];

    for (name, get, set) in props.into_iter() {
        assert!(map.insert(name.to_string(),
                           RegistryField::Property {
                               get: get, set: set
                           }).is_none(), "Duplicate element inserted!");
    }
    map
}

// Constants for use with registry-accessing tests

pub const BOOL: bool = true;
pub const U64: u64 = 42;
pub const I64: i64 = -1;
pub const F64: f64 = 21.5;

pub const TEXT: &'static str = "Hello way-cooler";
pub const POINT: Point = Point { x: 12, y: 12 };

pub const READONLY: &'static str = "read only text";
pub const WRITEONLY: &'static str = "write only text";
pub const NO_PERMS: &'static str = "<look ma no perms>";

pub const PROP_GET_RESULT: &'static str = "get property";

pub fn get_prop() -> Json {
    PROP_GET_RESULT.to_json()
}

pub fn get_panic_prop() -> Json {
    panic!("get_panic_prop panic")
}

pub fn set_prop(_json: Json) {
    println!("set_prop being called!");
}

pub fn set_panic_prop(_json: Json) {
    panic!("set_panic_prop panic")
}


/// [0, 1, 2]
pub fn u64s() -> Vec<Json> {
    vec![Json::U64(0), Json::U64(1), Json::U64(2)]
}

json_convertible! {
    /// Has an x and y field
    #[derive(Debug, Clone, Eq, PartialEq)]
    struct Point {
        x: i32,
        y: i32
    }
}

unsafe impl Sync for Point {}

#[test]
fn add_keys() {
}

#[test]
fn contains_keys() {
    let keys = [
        "bool", "u64", "i64", "f64", "null", "text", "point",
        "u64s", "readonly", "writeonly", "prop", "noperms",
        "get_prop", "set_prop", "get_panic_prop", "set_panic_prop",
    ];
    for key in keys.into_iter() {
        assert!(registry::key_info(key).is_some(),
                "Could not find key {}", key);
    }
}

#[test]
fn key_info() {
    let keys = [
        ("bool", FieldType::Object, AccessFlags::all()),
        ("u64",  FieldType::Object, AccessFlags::all()),
        ("readonly", FieldType::Object, AccessFlags::READ()),
        ("writeonly", FieldType::Object, AccessFlags::WRITE()),
        ("prop", FieldType::Property, AccessFlags::all()),
        ("noperms", FieldType::Object, AccessFlags::empty()),
        ("get_prop", FieldType::Property, AccessFlags::READ()),
        ("set_prop", FieldType::Property, AccessFlags::WRITE()),
    ];
    for &(key, type_, flags) in keys.into_iter() {
        assert!(registry::key_info(key) == Some((type_, flags)),
                "Invalid flags for {}", key);
    }
}

#[test]
fn objects_and_keys_equal() {
    let values = vec![
        ("bool",  AccessFlags::all(), BOOL.to_json()),
        ("u64",   AccessFlags::all(), U64.to_json()),
        ("i64",   AccessFlags::all(), I64.to_json()),
        ("f64",   AccessFlags::all(), F64.to_json()),
        ("text",  AccessFlags::all(), TEXT.to_json()),
        ("point", AccessFlags::all(), POINT.to_json()),
        ("null",  AccessFlags::all(), Json::Null),
        ("u64s",  AccessFlags::all(), Json::Array(u64s())),

        ("readonly",  AccessFlags::READ(),  READONLY.to_json()),
        ("writeonly", AccessFlags::WRITE(), WRITEONLY.to_json()),
        ("noperms",   AccessFlags::empty(), NO_PERMS.to_json())
    ];

    for (name, flags, json) in values.into_iter() {
        let (found_flags, found) = registry::get_data(name)
            .expect(&format!("Unable to get key {}", name))
            .resolve();
        assert_eq!(found_flags, flags);
        assert_eq!(*found, json);
    }
}

#[test]
fn key_perms() {
    let perms = vec![
        ("bool",      AccessFlags::all()),
        ("readonly",  AccessFlags::READ()),
        ("writeonly", AccessFlags::WRITE()),
        ("prop",      AccessFlags::all()),
        ("get_prop", AccessFlags::READ()),
        //("set_prop", AccessFlags::WRITE())
    ];

    for (name, flags) in perms.into_iter() {
        let found_data = registry::get_data(name)
            .expect(&format!("Could not get data for {}", name));

        let (found_flags, _) = found_data.resolve();
        println!("Testing flags for {}", name);
        assert_eq!(found_flags, flags);
    }
}

#[test]
fn property_get() {
    let prop_read = registry::get_data("get_prop")
        .expect("Couldn't get prop_read");

    assert_eq!(*prop_read.resolve().1, PROP_GET_RESULT.to_json());
}

#[test]
#[should_panic(expected="get_panic_prop panic")]
fn panicking_property_get() {
    let prop_read = registry::get_data("get_panic_prop")
        .expect("Couldn't get prop_read");

    assert_eq!(*prop_read.resolve().1, PROP_GET_RESULT.to_json());
}

#[test]
fn property_set() {
    registry::set_json("set_prop".to_string(), Json::Null)
            .expect("Unable to set data").call(Json::Null);
}

#[test]
#[should_panic(expected="set_panic_prop panic")]
fn panicking_property_set() {
    registry::set_json("set_panic_prop".to_string(), Json::Null)
            .expect("Unable to set data").call(Json::Null);
}
