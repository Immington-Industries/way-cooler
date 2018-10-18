use xcb::ffi::xproto::xcb_atom_t;

use std::sync::Mutex;

lazy_static! {
    pub static ref PROPERTIES: Mutex<Vec<XProperty>> = Mutex::new(vec![]);
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum XPropertyType {
    /// UTF8 string
    String,
    /// Cardinal
    Number,
    /// Cardinal with values 0 and 1 (or "0 and != 0")
    Boolean,
}

impl XPropertyType {
    pub fn from_string(type_: String) -> Option<Self> {
        match type_.as_str() {
            "string" => Some(XPropertyType::String),
            "number" => Some(XPropertyType::Number),
            "boolean" => Some(XPropertyType::Boolean),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct XProperty {
    pub atom: xcb_atom_t,
    pub name: String,
    pub type_: XPropertyType,
}

impl XProperty {
    pub fn new(name: String, type_: XPropertyType, atom: xcb_atom_t) -> Self {
        XProperty { atom, name, type_ }
    }
}
