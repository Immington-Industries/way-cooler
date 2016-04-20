//! Metamethods for accesssing the registry values!

use super::super::super::registry;
use registry::RegistryAccess;

use hlua::{LuaTable};
use hlua::any::AnyLuaValue;

use rustc_serialize::json::{Json, ToJson};
use std::ops::Deref;
pub fn index(table: AnyLuaValue, lua_key: AnyLuaValue)
             -> Result<AnyLuaValue, &'static str> {
    if let AnyLuaValue::LuaString(key) = lua_key {
        let maybe_json = registry::get_json(&key);
        if let Some(json_pair) = maybe_json {
            let (access, json) = json_pair;
            if access == RegistryAccess::Private {
                // TODO In lua_async: return nil
                Err("No value found for that key!")
            }
            else {
                Ok(convert_json(json.deref().to_json()))
            }
        }
        else {
            Err("No value found for that key!")
        }
    }
    else {
        Err("The registry can only be indexed with keys!")
    }
}

// Prevent lua from changing the registry?
pub fn new_index(table: AnyLuaValue, key: AnyLuaValue, val: AnyLuaValue)
                 -> Result<(), &'static str> {
    Err("You can't set the registry!")
}

fn convert_json(json: Json) -> AnyLuaValue {
    match json {
        Json::String(val) => AnyLuaValue::LuaString(val),
        Json::Boolean(val) => AnyLuaValue::LuaBoolean(val),
        Json::F64(val) => AnyLuaValue::LuaNumber(val),
        Json::I64(val) => AnyLuaValue::LuaNumber((val as i32) as f64),
        Json::U64(val) => AnyLuaValue::LuaNumber((val as u32) as f64),
        Json::Null => AnyLuaValue::LuaString("none".to_string()),
        Json::Array(vals) => {
            let mut count = 0f64;
            // Gotta love that 1-based indexing. Start at zero but increment for
            // the first one. It works here at least.
            AnyLuaValue::LuaArray(vals.into_iter().map(|v| {
                count += 1.0;
                (AnyLuaValue::LuaNumber(count), convert_json(v))
            }).collect())
        },
        Json::Object(vals) => {
            AnyLuaValue::LuaArray(vals.into_iter().map(|(key, val)| {
                (AnyLuaValue::LuaString(key), convert_json(val))
            }).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::convert_json;
    use hlua::any::AnyLuaValue::*;
    use rustc_serialize::json::{Json, ToJson, Object};
    use std::collections::BTreeMap;

    #[test]
    fn convert_string() {
        let input = "Hello World".to_string();
        let json_input = Json::String(input.clone());
        assert_eq!(convert_json(json_input), LuaString(input));
    }
    #[test]
    fn convert_numbers() {
        let lua_number = LuaNumber(42f64);
        assert_eq!(convert_json(Json::F64(42f64)), lua_number);
        assert_eq!(convert_json(Json::I64(42i64)), lua_number);
        assert_eq!(convert_json(Json::U64(42u64)), lua_number);
    }

    #[test]
    fn convert_bool() {
        assert_eq!(convert_json(Json::Boolean(true)), LuaBoolean(true));
        assert_eq!(convert_json(Json::Boolean(false)), LuaBoolean(false));
    }

    #[test]
    fn convert_array() {
        let json = Json::Array(
            vec![Json::Boolean(true), Json::F64(11f64),
                 Json::String("Foo".to_string())]
        );
        let lua = LuaArray(vec![
                (LuaNumber(1f64), LuaBoolean(true)),
                (LuaNumber(2f64), LuaNumber(11f64)),
                (LuaNumber(3f64), LuaString("Foo".to_string()))
            ]);
        assert_eq!(convert_json(json), lua);
    }

    #[test]
    fn convert_object() {
        // { name="foo", point: {x=0, y=1}, valid=false }
        // The fields are emitted in alphabetical order thanks to Json's
        // internal BTreeMap representation.

        let mut json_tree: Object = BTreeMap::new();
        let mut point_tree: Object = BTreeMap::new();
        point_tree.insert("x".to_string(), Json::U64(0u64));
        point_tree.insert("y".to_string(), Json::U64(1u64));
        json_tree.insert("point".to_string(), Json::Object(point_tree));
        json_tree.insert("name".to_string(), Json::String("foo".to_string()));
        json_tree.insert("valid".to_string(), Json::Boolean(false));

        let lua = LuaArray(vec![
            (LuaString("name".to_string()), LuaString("foo".to_string())),
            (LuaString("point".to_string()), LuaArray(vec![
                (LuaString("x".to_string()), LuaNumber(0f64)),
                (LuaString("y".to_string()), LuaNumber(1f64))
            ])),
            (LuaString("valid".to_string()), LuaBoolean(false)),
        ]);

        assert_eq!(convert_json(Json::Object(json_tree)), lua);
    }
}
