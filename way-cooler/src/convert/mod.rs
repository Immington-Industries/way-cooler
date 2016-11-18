//! Conversions between AnyLuaValue and structures.
//! Also see macros.

pub mod serialize;
pub mod deserialize;
pub mod json;

pub use self::serialize::ToTable;
pub use self::deserialize::
{FromTable, LuaDecoder, ConverterError, ConvertResult};

// Tests for serialize <-> deserialize compatability

#[cfg(test)]
mod tests {
    use hlua::any::AnyLuaValue;
    use hlua::any::AnyLuaValue::*;
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct Point {
        x: i32,
        y: u32
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Coord {
        name: String,
        point: Point
    }

    impl ToTable for Point {
        fn to_table(self) -> AnyLuaValue {
            LuaArray(vec![
                (LuaString("x".to_string()), LuaNumber(self.x as f64)),
                (LuaString("y".to_string()), LuaNumber(self.y as f64))
            ])
        }
    }
    impl ToTable for Coord {
        fn to_table(self) -> AnyLuaValue {
            LuaArray(vec![
                (LuaString("name".to_string()), LuaString(self.name)),
                (LuaString("point".to_string()), self.point.to_table())
            ])
        }
    }
    impl FromTable for Point {
        fn from_table(decoder: LuaDecoder) -> ConvertResult<Point> {
            let (decoder, x) = try!(decoder.read_field("x".to_string()));
            let (_, y) = try!(decoder.read_field("y".to_string()));
            Ok(Point { x: x, y: y })
        }
    }
    impl FromTable for Coord {
        fn from_table(decoder: LuaDecoder) -> ConvertResult<Coord> {
            let (decoder, name) = try!(decoder.read_field("name".to_string()));
            let (_, point) = try!(decoder.read_field("point".to_string()));
            Ok(Coord { name: name, point: point })
        }
    }

    #[test]
    fn test_point_and_coord() {
        let origin = Point { x: 0 , y: 0 };
        let origin_table = origin.clone().to_table();
        let origin_from_table = Point::from_lua_table(origin_table);
        assert_eq!(origin, origin_from_table.unwrap());
    }
}
