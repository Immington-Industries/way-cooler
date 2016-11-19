//! Contains code to handle an IPC channel which is issuing commands.

use std::io::Error as IOError;
use std::io::prelude::*;

use std::collections::BTreeMap;

use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};

use rustc_serialize::json::{Json, encode, ParserError, EncoderError};

/// Errors which arise from sending a message
#[derive(Debug)]
pub enum SendError {
    /// IO Error
    IO(IOError),
    /// JSON encoding error - less likely
    Encoding(EncoderError)
}

/// Reasons a client message might be erroneous
#[derive(Debug)]
pub enum ReceiveError {
    /// There were IO issues
    IO(IOError),
    /// Json was invalid
    InvalidJson(ParserError),
}

pub type SendResult = Result<(), SendError>;
pub type ReceiveResult = Result<Json, ReceiveError>;

/// Receives a packet from the given stream.
pub fn read_packet(stream: &mut Read) -> ReceiveResult {
    let len = try!(stream.read_u32::<NetworkEndian>()
                   .map_err(ReceiveError::IO));
    trace!("Listening for packet of length {}", len);
    return Json::from_reader(&mut stream.take(len as u64))
        .map_err(ReceiveError::InvalidJson);
}

/// Writes a packet to the given stream
pub fn write_packet(stream: &mut Write, packet: &Json) -> SendResult {
    let json_string = try!(encode(packet).map_err(SendError::Encoding));
    trace!("Writing packet of length {}: {}", json_string.len(), json_string);
    if json_string.len() > ::std::u32::MAX as usize {
        panic!("Attempted to send reply too big for the channel!");
    }
    try!(stream.write_u32::<NetworkEndian>(json_string.len() as u32)
         .map_err(SendError::IO));
    stream.write_all(json_string.as_bytes()).map_err(SendError::IO)
}

/// A Json message formatted with `"reason": reason`.
///
/// # Example
/// A call to `channel::error_json("foo".to_string())` yields
/// ```json
/// { "type": "error", "reason": "foo" }
/// ```
pub fn error_json(reason: String) -> Json {
    Json::Object(json_object!{
        "type" => "error",
        "reason" => reason
    })
}

/// Create a Json error message with additional fields.
///
/// # Example
/// Creating `foo = { "foo": "bar", "baz": 42 }` and calling
/// `channel::error_json("2foo2me".to_string(), foo)` yields
/// ```json
/// { "type": "error", "reason": "foo", "bar", "baz": 42 }
/// ```
pub fn error_json_with(reason: String,
                       others: BTreeMap<String, Json>) -> Json {
    if let Json::Object(mut json) = error_json(reason) {
        for (key, value) in others.into_iter() {
            json.insert(key, value);
        }
        return Json::Object(json)
    }
    unreachable!()
}

/// Create a Json error messgae for expecting a `key` of some `type`.
///
/// # Example
/// A call to `channel::error_expecting_key("foo", "string")` yields
/// ```json
/// { "type": "error", "reason": "message field not found",
///   "missing": "foo", "expected": "string" }
/// ```
pub fn error_expecting_key(key: &'static str, type_: &'static str) -> Json {
    error_json_with("missing message field".to_string(), json_object!{
        "missing" => key,
        "expected" => type_
    })
}

/// A Json representing a success packet.
///
/// # Example
/// `channel::success_json()` yields
/// ```json
/// { "type": "success" }
/// ```
pub fn success_json() -> Json {
    Json::Object(json_object!{ "type" => "success" })
}

/// Send an error message with additional fields
///
/// # Example
/// `channel::success_json_with(foo)` with foo such as `{ "foo": "bar" }` yields
/// ```json
/// { "type": "success", "foo": "bar" }
/// ```
pub fn success_json_with(others: BTreeMap<String, Json>) -> Json {
    if let Json::Object(mut json) = success_json() {
        for (key, value) in others.into_iter() {
            json.insert(key, value);
        }
        return Json::Object(json)
    }
    unreachable!()
}

/// A Json representing a value packet
///
/// # Example
/// `channel::value_json(foo)` with foo such as `{ "foo": 1, "bar": 2 }` yields
/// ```json
/// { "type": "success", "value": { "foo": 1, "bar": 2 } }
/// ```
pub fn value_json(value: Json) -> Json {
    if let Json::Object(mut responses) = success_json() {
        responses.insert("value".to_string(), value);
        return Json::Object(responses);
    }
    unreachable!()
}
