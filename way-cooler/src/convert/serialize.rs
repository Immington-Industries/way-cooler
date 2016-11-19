//! Module for implementing convert's ToTable.

use hlua::any::AnyLuaValue;
use hlua::any::AnyLuaValue::*;

/// Represents types which can be serialized into a Lua table (AnyLuaValue).
///
/// If you wish to continue using the object after serialization, consider
/// using `impl<a> ToTable for &'a T`, which will take an `&T` as its `self`.
pub trait ToTable {
    /// Write this value into an AnyLuaValue
    fn to_table(self) -> AnyLuaValue;
}

// Implementation for standard numeric types
macro_rules! numeric_impl {
    ($($typ:ty), +) => {
        $(impl ToTable for $typ {
            fn to_table(self) -> AnyLuaValue {
                LuaNumber(self as f64)
            }
        })+
    }
}

numeric_impl!(usize, isize);
numeric_impl!(i8, i16, i32);
numeric_impl!(u8, u16, u32);
numeric_impl!(f32, f64);

impl ToTable for bool {
    fn to_table(self) -> AnyLuaValue {
        LuaBoolean(self)
    }
}

impl ToTable for String {
    fn to_table(self) -> AnyLuaValue {
        LuaString(self)
    }
}

impl<T: ToTable> ToTable for Option<T> {
    fn to_table(self) -> AnyLuaValue {
        match self {
            Some(val) => val.to_table(),
            None => LuaNil
        }
    }
}

impl<T: ToTable> ToTable for Vec<T> {
    fn to_table(self) -> AnyLuaValue {
        LuaArray(self.into_iter().enumerate()
            .map(|(ix, val)| {
                (LuaNumber(ix as f64), val.to_table())
            }).collect())
    }
}

use std::collections::HashMap;
use std::hash::Hash;
impl<K, V> ToTable for HashMap<K, V>
where K: Eq + Hash + ToTable, V: ToTable {
    fn to_table(self) -> AnyLuaValue {
        let mut table = Vec::with_capacity(self.capacity());
        for (key, value) in self.into_iter() {
            table.push((key.to_table(), value.to_table()));
        }
        LuaArray(table)
    }
}

impl ToTable for () {
    fn to_table(self) -> AnyLuaValue {
        LuaNil
    }
}

use rustc_serialize::json::Json;

impl ToTable for Json {
    fn to_table(self) -> AnyLuaValue {
        super::json::json_to_lua(self)
    }
}
