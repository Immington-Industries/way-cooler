//! Tests

use std::thread;
use std::time::Duration;
use std::collections::BTreeMap;

use rustc_serialize::json::{Json, ParserError, ErrorCode};

use ipc::unique_ish_id;
use super::super::channel::*;

const COMPLEX_JSON: &'static str =
    r#"{ "foo": "bar", "baz": 1, "bar": [ "a", "b", "c", 22], "obj":
{ "foo": "bar", "baz": 22.4 } }"#;

const BAD_JSON_DELIMITER: &'static str = r#"{ "foo": "bar" "#;

const SUCCESS_JSON: &'static str = r#"{ "type": "success" }"#;
const VALUE_JSON: &'static str = r#"{ "type": "success", "value": "foo" }"#;
const SUCCESS_WITH_FOO: &'static str =
    r#"{ "type": "success", "foo": "bar" }"#;
const ERR_JSON_FOO: &'static str = r#"{ "type": "error", "reason": "foo" }"#;
const ERR_JSON_WITH_FOO: &'static str =
    r#"{ "type": "error", "reason": "foo", "foo": "bar" }"#;
const ERR_EXPECTING_KEY: &'static str =
    r#"{ "type": "error", "reason": "missing message field", "missing": "foo",
         "expected": "string" }"#;


#[test]
fn u32_coversion() {
    let nums = [0u32, !2, 255, 256, 1024, 2048, 12, 4096, 1, 0xfffffffe,
                0b11111111, 0xffefff, 0x11111111, 0x0101011, 0xffffffff];
    for num in &nums {
        let be_num = num.to_be();
        let le_num = num.to_le();
        assert_eq!(u32_to_bytes(be_num), u32_to_bytes(be_num));
        assert_eq!(u32_from_bytes(u32_to_bytes(le_num)), le_num);
    }
}

#[test]
fn uniquesh_id_is_uniqueish() {
    let mut ids = Vec::with_capacity(15);
    for _ in 0..15 {
        println!("New id: {}", unique_ish_id());
        ids.push(unique_ish_id());
        thread::sleep(Duration::from_millis(50));
    }
    ids.dedup();
    assert!(ids.len() == 15, "unique_ish_id conflict: {:?}", ids);
}

fn packet_stream_from_str(text: &str) -> Vec<u8> {
    let mut turn = Vec::with_capacity(text.len()+4);
    let mut bytes = text.as_bytes();
    assert!(bytes.len() < 255,
            "Tests only work with < 255 char packets!");
    turn.push(0);
    turn.push(0);
    turn.push(0);
    turn.push(bytes.len() as u8);
    turn.extend_from_slice(&mut bytes);
    return turn;
}

#[test]
fn read_packet_bad_json() {
    let packet = packet_stream_from_str(BAD_JSON_DELIMITER);
    match read_packet(&mut &*packet) {
        Ok(json) => panic!("Got some Json somehow: {:?}", json),
        Err(response) => match response {
            ReceiveError::InvalidJson(err) => match err {
                ParserError::SyntaxError(code, start, end) => {
                    assert_eq!(code, ErrorCode::EOFWhileParsingObject);
                    assert_eq!(start, 1);
                    assert_eq!(end, 16);
                },
                other_json @ _ =>
                    panic!("Unexpected json errr: {:?}", other_json)
            },
            other @ _ => panic!("Unexpected err: {:?}", other)
        }
    }
}

#[test]
fn read_packet_json() {
    let packet = packet_stream_from_str(r#"{ "foo": "bar" }"#);
    match read_packet(&mut &*packet) {
        Ok(json) => {
            let desired = Json::Object(json_object! {
                "foo" => "bar"
            });
            assert_eq!(json, desired);
        }
        Err(err) => panic!("Unable to parse Json: {:?}", err)
    }
}

#[test]
fn read_packet_too_short() {
    let mut packet = Vec::new();
    packet.push(0); packet.push(0); packet.push(0);
    packet.push(5); // Len == 5
    packet.extend_from_slice(br#"{ "foo": 1 }"#);
    // Gets to just before close quote of foo

    match read_packet(&mut &*packet) {
        Ok(json) => panic!("Got a json {:?}, read too much?", json),
        Err(response) => match response {
            ReceiveError::InvalidJson(json_err) => match json_err {
                ParserError::SyntaxError(code, start, end) => {
                    assert_eq!(code, ErrorCode::EOFWhileParsingString);
                    assert_eq!(start, 1); assert_eq!(end, 6);
                },
                other_json @ _ => panic!("Wrong json err: {:?}", other_json)
            },
            other @ _ => panic!("Wrong error: {:?}", other)
        }
    }
}

// There is no packet_too_long test because it would result in the server
// waiting on a call to read() and the client probably not sending more data.

#[test]
fn write_packet_integrity() {
    let packet_json = Json::from_str(COMPLEX_JSON).expect("complex_json");
    let mut packet = Vec::new();
    write_packet(&mut packet, &packet_json).expect("Unable to write pakcet");
    // Remove length
    for _ in 0 .. 4 { packet.remove(0); }
    let text = String::from_utf8(packet).expect("Packet had invalid utf8");
    assert_eq!(packet_json, Json::from_str(&text).expect("Couldn't parse text"));
}

#[test]
fn read_write_packet() {
    let packet_json = Json::from_str(COMPLEX_JSON).expect("complex_json");
    let mut packet = Vec::new();
    write_packet(&mut packet, &packet_json).expect("Could not write");
    let read_json = read_packet(&mut &*packet).expect("Could not read");
    assert_eq!(read_json, packet_json);
}

#[test]
fn success_json_is_successful() {
    assert_eq!(success_json(),
               Json::from_str(SUCCESS_JSON).expect("Can't get success Json"));
    assert_eq!(value_json(Json::String("foo".to_string())),
               Json::from_str(VALUE_JSON).expect("Can't get value Json"));
    let json = json_object!{ "foo" => "bar" };
    assert_eq!(success_json_with(json),
               Json::from_str(SUCCESS_WITH_FOO).expect("Can't get foo Json"));
}

#[test]
fn error_json_is_erroneous() {
    assert_eq!(error_json("foo".to_string()),
               Json::from_str(ERR_JSON_FOO).expect("Can't get err Json"));
    assert_eq!(error_expecting_key("foo", "string"),
               Json::from_str(ERR_EXPECTING_KEY).expect("Cant't get err key Json"));
    let mut json = BTreeMap::new();
    json.insert("foo".to_string(), Json::String("bar".to_string()));
    assert_eq!(error_json_with("foo".to_string(), json),
               Json::from_str(ERR_JSON_WITH_FOO).expect("Can't get foo err Json"));
}


