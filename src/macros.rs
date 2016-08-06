//! Warning: extreme macros

/// This macro creates structs that implement `ToTable` and `FromTable`.
///
/// # Examples:
///
/// ```rust
/// lua_convertible! {
///     #[derive(Debug)]
///     // #[attribtue]
///     struct Point {
///         x: i32,
///         y: i32
///     }
/// }
/// ```
#[macro_export]
macro_rules! lua_convertible {
    (  $(#[$attr:meta])*
       struct $name:ident { $($fname:ident : $ftype:ty),+  }  ) => {

        $(#[$attr])*
        pub struct $name {
            $($fname: $ftype),+
        }

        impl $crate::convert::ToTable for $name {
            fn to_table(self) -> hlua::any::AnyLuaValue {
                hlua::any::AnyLuaValue::LuaArray(vec![
                    $(  (hlua::any::AnyLuaValue::LuaString(stringify!($fname).to_string()),
                         self.$fname.to_table())  ),+
                ])
            }
        }

        impl $crate::convert::FromTable for $name {
            #[allow(unused_variables)]
            fn from_table(decoder: $crate::convert::LuaDecoder) ->
                $crate::convert::ConvertResult<$name> {
                $(  let (decoder, $fname) =
                    try!(decoder.read_field(stringify!($fname).to_string())); )+

                Ok($name {
                    $( $fname: $fname ),+
                })
            }
        }
    }
}
/// Creates a struct and implements `ToJson` and `Decodeable` from
/// rustc_serialize.
#[macro_export]
macro_rules! json_convertible {
    (  $(#[$attr:meta])*
       struct $name:ident { $($fname:ident : $ftype:ty),+  }  ) => {

        $(#[$attr])*
        pub struct $name {
            $($fname: $ftype),+
        }

        impl ::rustc_serialize::json::ToJson for $name {
            fn to_json(&self) -> ::rustc_serialize::json::Json {
                let mut tree = ::std::collections::BTreeMap::new();
                $( tree.insert(stringify!($fname).to_string(),
                                self.$fname.to_json()); )+
                ::rustc_serialize::json::Json::Object(tree)
            }
        }

        impl ::rustc_serialize::Decodable for $name {
            fn decode<D: ::rustc_serialize::Decoder>(d: &mut D) -> Result<$name, D::Error> {
                $( let $fname = try!(d.read_struct_field(
                    stringify!($fname), 0usize,
                    |f| ::rustc_serialize::Decodable::decode(f))); )+

                Ok($name {
                    $( $fname: $fname ),+
                })
            }
        }
    }
}
/// Create a keypress using fewer keystrokes. Provides a custom panic method.
#[macro_export]
macro_rules! keypress {
    ($modifier:expr, $key:expr) => {
        $crate::keys::KeyPress::from_key_names(&[$modifier],
                                 $key)
            .expect(concat!("Unable to create keypress from macro with ",
                            $modifier, " and ", $key))
    };
}

/// Return from a test method if DUMMY_RUSTWLC is defined.
#[cfg(test)]
macro_rules! require_rustwlc {
    () => {
        if cfg!(test) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::convert::{ToTable, FromTable, LuaDecoder};
    use hlua;
    use rustc_serialize::Decodable;
    use rustc_serialize::json::{Decoder, ToJson};

    lua_convertible! {
        #[derive(Debug, Clone, PartialEq)]
        struct Point {
            x: f32,
            y: f32
        }
    }

    json_convertible! {
        #[derive(Debug, Clone, PartialEq)]
        struct Rectangle {
            height: u32,
            width: u32
        }
    }

    #[test]
    fn require_rustwlc() {
        require_rustwlc!();
        // If we're here we can use rustwlc.
        // If we tried to get a view or something it'd fail though.
        let _ = keypress!("Ctrl", "p");
    }

    #[test]
    fn lua_convertible() {
        let point = Point { x: 0f32, y: 0f32 };
        let lua_point = point.clone().to_table();
        let maybe_point = Point::from_table(LuaDecoder::new(lua_point));
        let parsed_point = maybe_point.expect("Unable to parse point!");
        assert_eq!(parsed_point, point);
    }

    #[test]
    fn json_convertible() {
        let rect = Rectangle { height: 1u32, width: 2u32 };
        let json_rect = rect.to_json();
        let maybe_rect = Rectangle::decode(&mut Decoder::new(json_rect));
        let parsed_rect = maybe_rect.expect("Unable to parse rectangle!");
        assert_eq!(parsed_rect, rect);
    }

    #[test]
    fn keypress() {
        require_rustwlc!();
        use super::super::keys::KeyPress;
        use std::hash::{SipHasher, Hash};

        let press = KeyPress::from_key_names(&["Ctrl"], "p")
            .expect("Unable to construct regular keypress");
        let press_macro = keypress!("Ctrl", "p");
        let mut hasher = SipHasher::new();
        assert!(press.hash(&mut hasher) == press_macro.hash(&mut hasher),
                "Hashes do not match");
        assert_eq!(press, press_macro);
    }
}
