//! Command part of IPC

use std::io::prelude::*;
use std::collections::BTreeMap;

use rustc_serialize::json::{encode, Json, ParserError};

use super::channel::{self, ReceiveError};

use registry;
use commands;

/// NOTE This should be removed when types are added to the registry, and this ugly check is unnecessary
macro_rules! expect_key {
    ($in_json:expr; $name:expr, $typ:ident) => {
        match $in_json.remove($name) {
            Some(key) => match key {
                Json::$typ(var) => var,
                _ => return Err(channel::error_expecting_key($name,
                          stringify!($typ)))
            },
            None => return Err(channel::error_expecting_key($name,
            stringify!($typ)))
        }
    }
}

/// Run a thread reading and replying to queries
pub fn listen_loop<S: Read + Write>(mut stream: &mut S) {
    loop {
        match channel::read_packet(&mut stream) {
            Ok(packet) => {
                trace!("Read packet: {}", encode(&packet)
                    .unwrap_or("<packet that was read already??>".to_string()));
                // Error half of result is discarded but isn't very relevant
                let reply = reply(packet).unwrap_or_else(|e| e);
                trace!("Writing reply: {}", encode(&reply)
                       .unwrap_or("<a reply which is not writable>".to_string()));
                channel::write_packet(&mut stream, &reply)
                    .expect("response: Unable to reply!");
            },
            Err(read_err) => match read_err {
                ReceiveError::IO(ioerr) => {
                    warn!("Unable to read reply! Closing connection.");
                    debug!("Got IOError: {:?}", ioerr);
                    return;
                },
                ReceiveError::InvalidJson(parse_err) => match parse_err {
                    ParserError::SyntaxError(code, start, end) => {
                        let reply = Json::Object(json_object!{
                            "type" => "error",
                            "reason" => "invalid json",
                            "code" => (format!("{:?}", code)),
                            "start" => (start as u64),
                            "end" => (end as u64)
                        });
                        channel::write_packet(&mut stream, &reply)
                            .expect("invalid syntax: Unaable to reply!");
                    }
                    ParserError::IoError(_) => unreachable!()
                }
            }
        }
    }
}

/// Generates the response needed to a given command
/// If the request is ill-formed it returns an Err.
/// If the request is valid but fails it returns an Err.
pub fn reply(json: Json) -> Result<Json, Json> {
    let mut object: BTreeMap<String, Json>;
    if let Json::Object(obj) = json {
        object = obj;
    }
    else {
        return Err(channel::error_json(
            "invalid format - object required".to_string()))
    }

    let request_type = expect_key!(object; "type", String);

    match request_type.as_str() {
        // Registry
        "get" => {
            use std::ops::Deref;
            use registry::RegistryError::*;

            let key = expect_key!(object; "key", String);

            match registry::get_data(&key) {
                Ok(data) => {
                    let (_flags, arc) = data.resolve();
                    let reply = channel::value_json(arc.deref().clone());
                    return Ok(reply);
                },
                Err(err) => match err {
                    KeyNotFound =>
                        return Err(channel::error_json("key not found".to_string())),
                    InvalidOperation =>
                        return Err(channel::error_json("cannot get that key".to_string())),
                }
            }
        },
        "set" => {
            use registry::RegistryError::*;

            let key = expect_key!(&mut object; "key", String);
            let value: Json;
            match object.remove("value") {
                Some(val) => value = val,
                None => return Err(channel::error_expecting_key("value", "any"))
            }

            let reg_set = try!(registry::set_json(key, value.clone())
                 .map_err(|e| match e {
                     InvalidOperation =>
                         channel::error_json("cannot set that key".to_string()),
                     KeyNotFound =>
                         channel::error_json("key not found, use insert".to_string())
                 }));
            reg_set.call(value);

            Ok(channel::success_json())
        },
        "exists" => {
            let key = expect_key!(&mut object; "key", String);


            if let Some((key_type, flags)) = registry::key_info(&key) {
                Ok(channel::success_json_with(json_object!{
                    "exists" => true,
                    "flags" => flags,
                    "key_type" => key_type
                }))
            }
            else if let Some(_) = commands::get(&key) {
                Ok(channel::success_json_with(json_object!{
                    "exists" => true,
                    "key_type" => "Command"
                }))
            }
            else {
                Ok(channel::success_json_with(json_object!{
                    "exists" => false
                }))
            }
        },

        // Commands
        "run" => {
            let key = expect_key!(&mut object; "key", String);

            let command = try!(commands::get(&key).ok_or(
                channel::error_json("command not found".to_string())));

            command();

            Ok(channel::success_json())
        },

        // Meta/API commands
        "version" => {
            Ok(channel::value_json(Json::U64(super::VERSION)))
        },

        "commands" => {
            Ok(channel::value_json(Json::Array([
                "get", "set", "exists", "run",
                "version", "commands", "ping",
                ].into_iter().map(|v | Json::String(v.to_string())).collect())))
        },

        "ping" => {
            Ok(channel::success_json())
        },

        _ => Err(channel::error_json("invalid request; see 'commands'".to_string()))
    }
}
