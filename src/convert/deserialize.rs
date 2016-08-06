//! Module for implementing convert's FromTable

use std::hash::Hash;
use std::cmp::Eq;
use std::collections::HashMap;

use hlua::any::AnyLuaValue;
use hlua::any::AnyLuaValue::*;


/// Represents types which can be serialized from a Lua table (AnyLuaValue).
///
/// For convenience, this method takes in a `LuaDecoder` (obtained from
/// `LuaDecoder::new(AnyLuaValue)`). See methods on `LuaDecoder`.
pub trait FromTable {
    /// Attempt to parse this value from the decoder.
    fn from_table(decoder: LuaDecoder) -> ConvertResult<Self>
        where Self: Sized;

    fn from_lua_table(table: AnyLuaValue) -> ConvertResult<Self>
        where Self: Sized {
        Self::from_table(LuaDecoder::new(table))
    }
}

/// Errors a converter can run into
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum ConverterError {
    /// The type of value present did not match the one expected
    UnexpectedType(String),
    /// The table index expected did not exist
    MissingTableIndex(String),
    /// The table index present was not valid
    InvalidTableIndex(String)
}

impl ConverterError {
    pub fn into_message(self) -> String {
        match self {
            ConverterError::UnexpectedType(m) => m,
            ConverterError::MissingTableIndex(m) => m,
            ConverterError::InvalidTableIndex(m) => m
        }
    }
}

/// Results for conversion operations
pub type ConvertResult<T> = Result<T, ConverterError>;

/// Can decode values with a FromTable
#[derive(Debug, PartialEq, Clone)]
pub struct LuaDecoder {
    val: AnyLuaValue
}

impl LuaDecoder {
    pub fn new(val: AnyLuaValue) -> LuaDecoder {
        LuaDecoder { val: val }
    }

    pub fn get_bool(self) -> ConvertResult<bool> {
        match self.val {
            LuaBoolean(val) => Ok(val),
            _ => Err(ConverterError::UnexpectedType(
                format!("Expected bool, got {:?}", self.val)))
        }
    }

    pub fn get_string(self) -> ConvertResult<String> {
        match self.val {
            LuaString(text) => Ok(text),
            _ => Err(ConverterError::UnexpectedType(
                format!("Expected String, got {:?}", self.val)))
        }
    }

    pub fn get_option<T: FromTable>(self) -> ConvertResult<Option<T>> {
        match self.val {
            LuaNil => Ok(None),
            val => T::from_table(LuaDecoder::new(val)).map(|val| Some(val))
        }
    }

    #[allow(dead_code)]
    pub fn read_field<T>(self, name: String) -> ConvertResult<(Self, T)>
        where T: FromTable {
        match self.val {
            LuaArray(mut arr) => {
                let maybe_pos = arr.iter().position(|ref val_pair| {
                    match val_pair.0 {
                        LuaString(ref key) => {
                            *key == name
                        },
                        _ => false
                    }
                });
                if let Some(pos) = maybe_pos {
                    let (_, val) = arr.remove(pos);
                    let val_parser = LuaDecoder::new(val);
                    T::from_table(val_parser).map(|parse|
                                    (LuaDecoder::new(LuaArray(arr)), parse))
                }
                else {
                    Err(ConverterError::MissingTableIndex(name))
                }
            }
            _ => Err(ConverterError::UnexpectedType(
                format!("Expected table, got {:?}", self.val)))
        }
    }

    pub fn get_unordered_array<T: FromTable>(self) -> ConvertResult<Vec<T>> {
        match self.val {
            LuaArray(arr) => {
                let mut turn = Vec::with_capacity(arr.len());
                // Completely ignore the keys, push values of type T
                for (_, lua_val) in arr.into_iter() {
                    match T::from_lua_table(lua_val) {
                        Ok(val) => turn.push(val),
                        Err(err) => {
                            return Err(ConverterError::InvalidTableIndex(
                                format!("Attempted to parse value in a Vec, \
                                got error: {}", err.into_message())));
                        }
                    }
                }
                return Ok(turn);
            }
            _ => Err(ConverterError::UnexpectedType(
                 format!("Expected table for Vec, got {:?}", self.val)))
        }
    }

    pub fn get_hash_map<K, V>(self) -> ConvertResult<HashMap<K, V>>
        where K: Eq + Hash + FromTable, V: FromTable {
        match self.val {
            LuaArray(arr) => {
                let mut map = HashMap::with_capacity(arr.len());
                for (lua_key, lua_value) in arr.into_iter() {
                    match K::from_lua_table(lua_key) {
                        Ok(key) => match V::from_lua_table(lua_value) {
                            Ok(value) => { map.insert(key, value); },
                            Err(e) =>
                                return Err(ConverterError::InvalidTableIndex(
                                    format!("Attempted to parse value in a Hash\
                                             Map, got error: {}",
                                            e.into_message())))
                        },
                        Err(e) => return Err(ConverterError::InvalidTableIndex(
                        format!("Attempted to parse key in a HashMap, got error\
                                 : {}", e.into_message())))
                    }
                }
                return Ok(map);
            },
            _ => Err(ConverterError::UnexpectedType(
            format!("Expected table for HashMap, got {:?}", self.val)))
        }
    }
}

macro_rules! primitive_decode {
    ( $($ptype:ty, $fun:ident;) +) => {
        $(impl LuaDecoder {
            pub fn $fun(self) -> ConvertResult<$ptype> {
                match self.val {
                    AnyLuaValue::LuaNumber(num) => Ok(num as $ptype),
                    other => Err(ConverterError::UnexpectedType(
                        format!("Expected {}, got {:?}",
                                stringify!($ptype), other)))
                }
            }
        }

        impl FromTable for $ptype {
            fn from_table(decoder: LuaDecoder) -> ConvertResult<$ptype> {
                LuaDecoder::$fun(decoder)
            }
        })+
    }
}

macro_rules! type_fn_decode {
    ( $( $ptype:ty => $fun:ident), + ) => {
        $(impl FromTable for $ptype {
            fn from_table(decoder: LuaDecoder) -> ConvertResult<$ptype> {
                LuaDecoder::$fun(decoder)
            }
        })+
    }
}

primitive_decode! {
    i8,  get_i8;
    i16, get_i16;
    i32, get_i32;

    u8,  get_u8;
    u16, get_u16;
    u32, get_u32;

    f32, get_f32;
    f64, get_f64;
}

type_fn_decode! {
    String => get_string,
    bool => get_bool
}

impl<T: FromTable> FromTable for Vec<T> {
    fn from_table(decoder: LuaDecoder) -> ConvertResult<Vec<T>> {
        decoder.get_unordered_array()
    }
}

impl<T: FromTable> FromTable for Option<T> {
    fn from_table(decoder: LuaDecoder) -> ConvertResult<Option<T>> {
        decoder.get_option()
    }
}

impl<K, V> FromTable for HashMap<K, V> where K: Hash + Eq + FromTable, V: FromTable {
    fn from_table(decoder: LuaDecoder) -> ConvertResult<HashMap<K,V>> {
        decoder.get_hash_map()
    }
}

use rustc_serialize::json::Json;

impl FromTable for Json {
    fn from_table(decoder: LuaDecoder) -> ConvertResult<Json> {
        super::json::lua_to_json(decoder.val)
            .map_err(|_| ConverterError::InvalidTableIndex(
                "Lua value had invalid table index!".to_string()))
    }
}
