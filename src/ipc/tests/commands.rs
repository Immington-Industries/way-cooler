//! Tests individual commands via `reply`

use std::collections::{HashSet, BTreeMap};

use rustc_serialize::json::{Json, ToJson};

use super::super::channel;

use registry;

const PING: &'static str =     r#"{ "type":"ping" }"#;
const COMMANDS: &'static str = r#"{ "type":"commands" }"#;
const VERSION: &'static str =  r#"{ "type":"version" }"#;

const RUN_COMMAND: &'static str =     r#"{"type":"run", "key":"command" }"#;
const RUN_PANIC_COMMAND: &'static str=r#"{"type":"run","key":"panic_command"}"#;
const RUN_BAD_KEY: &'static str =     r#"{"type":"run", "key":"non_command" }"#;
const RUN_NO_KEY: &'static str =      r#"{"type":"run" }"#;

const GET_OBJECT: &'static str =     r#"{ "type":"get", "key":"point" }"#;
const GET_PROP: &'static str =       r#"{ "type":"get", "key":"get_prop" }"#;
const GET_READONLY: &'static str =   r#"{ "type":"get", "key":"readonly" }"#;
const GET_WRITEONLY: &'static str =  r#"{ "type":"get", "key":"writeonly" }"#;
const GET_PROP_WRITE: &'static str = r#"{ "type":"get", "key":"set_prop" }"#;
const GET_NO_KEY: &'static str =     r#"{ "type":"get" }"#;
const GET_BAD_KEY: &'static str =   r#"{ "type":"get", "key":"novalue" }"#;

const SET_OBJECT: &'static str
    = r#"{ "type":"set", "key":"ipc_test_bool", "value":false }"#;
const SET_PROP: &'static str
    = r#"{ "type":"set", "key":"prop", "value":null }"#;
const SET_READONLY: &'static str
    = r#"{ "type":"set", "key":"readonly", "value":12 }"#;
const SET_WRITEONLY: &'static str
    = r#"{ "type":"set", "key":"ipc_test_writeonly", "value":11 }"#;
const SET_PROP_READ: &'static str
    = r#"{ "type":"set", "key":"get_prop", "value":"asdf" }"#;
const SET_NEW_KEY: &'static str
    = r#"{ "type":"set", "key":"new_key", "value":"value" }"#;
const SET_NO_PERMS: &'static str
    = r#"{ "type":"set", "key":"noperms", "value":"something" }"#;
const SET_NO_KEY: &'static str
    = r#"{ "type":"set", "value":12 }"#;
const SET_NO_VALUE: &'static str
    = r#"{ "type":"set", "key":"foo" }"#;
const SET_BAD_KEY_TYPE: &'static str
    = r#"{ "type":"set", "key":12 }"#;

const EXISTS_OBJECT_RW: &'static str =  r#"{"type":"exists","key":"null" }"#;
const EXISTS_PROP: &'static str =       r#"{"type":"exists","key":"prop" }"#;
const EXISTS_PROP_READ: &'static str =  r#"{"type":"exists","key":"get_prop"}"#;
const EXISTS_PROP_WRITE: &'static str = r#"{"type":"exists","key":"set_prop"}"#;
const EXISTS_OBJECT_READ:&'static str = r#"{"type":"exists","key":"readonly"}"#;
const EXISTS_OBJECT_WRITE:&'static str= r#"{"type":"exists","key":"writeonly"}"#;
const EXISTS_BAD_KEY: &'static str =    r#"{"type":"exists","key": "nope"}"#;
const EXISTS_NO_KEY: &'static str =     r#"{"type":"exists"}"#;
const EXISTS_COMMAND: &'static str =    r#"{"type":"exists","key":"command"}"#;

const BAD_NO_REQUEST: &'static str = r#"{ "key":"foo", "value":"bar" }"#;
const BAD_INVALID_REQUEST: &'static str = r#"{ "type":"foo" }"#;
const BAD_REQUEST_IS_A_NUMBER: &'static str = r#"23"#;

macro_rules! reply {
    ($json:expr) => {
        super::super::command::reply(Json::from_str($json)
                                           .expect("Couldn't parse json"))
            .unwrap_or_else(|j| j)
    }
}

#[test]
fn ping() {
    let reply = reply!(PING);
    assert_eq!(reply, channel::success_json());
}

#[test]
fn commands() {
    let mut reply = reply!(COMMANDS);

    let mut obj = reply.as_object_mut()
        .expect("commands: reply not object");
    let mut value = obj.remove("value")
        .expect("commands: reply: no 'value'");
    let val_arr = value.as_array_mut()
        .expect("commands: reply: 'value' not array");
    let mut set = HashSet::new();
    for val in val_arr {
        set.insert(val.as_string()
                   .expect("commands: reply: 'value' not string array"));
    }
    assert_eq!(set.len(), 7);
    let desired = &["get", "set", "exists", "run", "version", "commands", "ping"];
    for com in desired.iter() {
        assert!(set.contains(com), "Missing command {}", com);
    }
}

#[test]
fn version() {
    let mut reply = reply!(VERSION);
    let version = reply.as_object_mut().expect("version: reply not object")
        .remove("value").expect("version: reply: no 'value'")
        .as_u64().expect("version: reply not u64");

    assert_eq!(version, super::super::VERSION);
}

#[test]
fn run_command() {
    let reply = reply!(RUN_COMMAND);
    assert_eq!(reply, channel::success_json());
}

// Note: If a command were to panic in way-cooler, the thread servicing that
// IPC connection would shut down. However, reply! is routing around that and
// everything is being run in each test thread. This is more of a general way
// of making sure commands are being run and we are aware of them.

#[test]
#[should_panic(reason = "panic_command panic")]
fn run_panic_command() {
    let _reply = reply!(RUN_PANIC_COMMAND);
}

#[test]
fn command_bad_args() {
    assert_eq!(reply!(RUN_BAD_KEY), Json::Object(json_object!{
        "type" => "error",
        "reason" => "command not found"
    }));
    assert_eq!(reply!(RUN_NO_KEY), Json::Object(json_object!{
        "type" => "error",
        "reason" => "missing message field",
        "missing" => "key",
        "expected" => "String"
    }));
}

#[test]
fn get_stuff() {
    assert_eq!(reply!(GET_OBJECT),
               channel::value_json(registry::tests::POINT.to_json()));
    assert_eq!(reply!(GET_PROP),
               channel::value_json(json!(registry::tests::PROP_GET_RESULT)));
    assert_eq!(reply!(GET_READONLY),
               channel::value_json(json!(registry::tests::READONLY)));
    assert_eq!(reply!(GET_WRITEONLY),
               channel::value_json(json!(registry::tests::WRITEONLY)));
    assert_eq!(reply!(GET_PROP_WRITE),
               channel::error_json("cannot get that key".to_string()));
    assert_eq!(reply!(GET_NO_KEY),
               channel::error_expecting_key("key", "String"));
    assert_eq!(reply!(GET_BAD_KEY),
               channel::error_json("key not found".to_string()));
}

#[test]
fn set_stuff() {
    use registry::AccessFlags;
    let error_set = channel::error_json("cannot set that key".to_string());
    registry::insert_json("ipc_test_bool".to_string(),
                          AccessFlags::all(),
                          true.to_json());
    registry::insert_json("ipc_test_writeonly".to_string(),
                          AccessFlags::WRITE(),
                          false.to_json());
    assert_eq!(reply!(SET_OBJECT), channel::success_json());
    assert_eq!(reply!(SET_PROP), channel::success_json());
    assert_eq!(reply!(SET_READONLY), error_set);
    assert_eq!(reply!(SET_WRITEONLY), channel::success_json());
    assert_eq!(reply!(SET_PROP_READ), error_set);
    assert_eq!(reply!(SET_NO_PERMS), error_set);
    assert_eq!(reply!(SET_NEW_KEY),
               channel::error_json("key not found, use insert".to_string()));
    assert_eq!(reply!(SET_NO_KEY),
               channel::error_expecting_key("key", "String"));
    assert_eq!(reply!(SET_BAD_KEY_TYPE),
               channel::error_expecting_key("key", "String"));
    assert_eq!(reply!(SET_NO_VALUE),
               channel::error_expecting_key("value", "any"));
}

#[test]
fn exists() {
    fn exists_json(mut others: BTreeMap<String, Json>) -> Json {
        others.insert("exists".to_string(), Json::Boolean(true));
        channel::success_json_with(others)
    }

    assert_eq!(reply!(EXISTS_OBJECT_RW),
               exists_json(json_object!{
                   "flags" => (json!([ "read".to_json(), "write".to_json()])),
                   "key_type" => "Object"
               }));
    assert_eq!(reply!(EXISTS_PROP),
               exists_json(json_object!{
                   "flags" => (json!([ "read".to_json(), "write".to_json()])),
                   "key_type" => "Property"
               }));
    assert_eq!(reply!(EXISTS_PROP_READ),
               exists_json(json_object!{
                   "flags" => (json!([ "read".to_json() ])),
                   "key_type" => "Property"
               }));
    assert_eq!(reply!(EXISTS_PROP_WRITE),
               exists_json(json_object!{
                   "flags" => (json!([ "write".to_json() ])),
                   "key_type" => "Property"
               }));
    assert_eq!(reply!(EXISTS_OBJECT_READ),
               exists_json(json_object!{
                   "flags" => (json!([ "read".to_json() ])),
                   "key_type" => "Object"
               }));
    assert_eq!(reply!(EXISTS_OBJECT_WRITE),
               exists_json(json_object!{
                   "flags" => (json!([ "write".to_json()])),
                   "key_type" => "Object"
               }));
    assert_eq!(reply!(EXISTS_BAD_KEY),
               channel::success_json_with(json_object!{
                   "exists" => false
               }));
    assert_eq!(reply!(EXISTS_NO_KEY),
               channel::error_expecting_key("key", "String"));
    assert_eq!(reply!(EXISTS_COMMAND),
               exists_json(json_object!{
                   "key_type" => "Command"
               }));
}

#[test]
fn bad_requests() {
    assert_eq!(reply!(BAD_NO_REQUEST),
               channel::error_expecting_key("type", "String"));
    assert_eq!(reply!(BAD_INVALID_REQUEST),
               channel::error_json("invalid request; see 'commands'".to_string()));
    assert_eq!(reply!(BAD_REQUEST_IS_A_NUMBER),
               channel::error_json("invalid format - object required".to_string()));
}
