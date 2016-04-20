//! Lua functions for the pointer.
//!
//! Should be registered under `wm.pointer` and `wm.keyboard`

use hlua::any::AnyLuaValue;
use hlua::any::AnyLuaValue::*;

use rustwlc::types::{Point};
use rustwlc::input::{pointer, keyboard};
use rustwlc::handle::WlcOutput;

/// Gets the position of the mouse
pub fn pointer_get_position() -> (f64, f64) {
    let location = pointer::get_position();
    (location.x as f64, location.y as f64)
}

/// Sets the position of the mouse
pub fn pointer_set_position(in_x: f64, in_y: f64) -> Result<(), String> {
    if in_x < 0f64 {
        return Err("Invalid negative x parameter!".to_string());
    }
    if in_y < 0f64 {
        return Err("Invalid negative y parameter!".to_string());
    }
    let x = in_x as i32;
    let y = in_y as i32;
    pointer::set_position(&Point { x: x, y: y });
    return Ok(());
}
