//! Callback methods for rustwlc
use rustwlc::handle::{WlcOutput, WlcView};
use rustwlc::types::*;
use rustwlc::input::{pointer, keyboard};
use rustwlc::xkb::Keysym;

use super::keys;
use super::lua;
use super::keys::{KeyEvent, KeyPress};

/// If the event is handled by way-cooler
const EVENT_HANDLED: bool = true;

/// If the event should be passed through to clients
const EVENT_PASS_THROUGH: bool = false;

// wlc callbacks

pub extern fn output_created(output: WlcOutput) -> bool {
    trace!("output_created: {:?}: {}", output, output.get_name());
    return true;
}

pub extern fn output_destroyed(output: WlcOutput) {
    trace!("output_destroyed: {:?}", output);
}

pub extern fn output_focus(output: WlcOutput, focused: bool) {
    trace!("output_focus: {:?} focus={}", output, focused);
}

pub extern fn output_resolution(output: WlcOutput,
                            old_size_ptr: &Size, new_size_ptr: &Size) {
    trace!("output_resolution: {:?} from  {:?} to {:?}",
             output, *old_size_ptr, *new_size_ptr);
}
/*
pub extern fn output_render_pre(output: WlcOutput) {
    //println!("output_render_pre");
}

pub extern fn output_render_post(output: WlcOutput) {
    //println!("output_render_post");
}
*/
pub extern fn view_created(view: WlcView) -> bool {
    trace!("view_created: {:?}: \"{}\"", view, view.get_title());
    let output = view.get_output();
    view.set_mask(output.get_mask());
    view.bring_to_front();
    view.focus();
    return true;
}

pub extern fn view_destroyed(view: WlcView) {
    trace!("view_destroyed: {:?}", view);
}

pub extern fn view_focus(current: WlcView, focused: bool) {
    trace!("view_focus: {:?} {}", current, focused);
    current.set_state(VIEW_ACTIVATED, focused);
}

pub extern fn view_move_to_output(current: WlcView,
                                  o1: WlcOutput, o2: WlcOutput) {
    trace!("view_move_to_output: {:?}, {:?}, {:?}", current, o1, o2);
}

pub extern fn view_request_geometry(view: WlcView, geometry: &Geometry) {
    trace!("view_request_geometry: {:?} wants {:?}", view, geometry);
    view.set_geometry(EDGE_NONE, geometry);
}

pub extern fn view_request_state(view: WlcView, state: ViewState, handled: bool) {
    trace!("view_request_state: {}, {:?}, handled: {}",
    view.get_title(), state, handled);
    view.set_state(state, handled);
}

pub extern fn view_request_move(view: WlcView, dest: &Point) {
    // Called by views when they have a dang resize mouse thing, we should only
    // let it happen in view floating mode
    trace!("view_request_move: to {}, start interactive mode.", *dest);
}

pub extern fn view_request_resize(view: WlcView,
                              edge: ResizeEdge, location: &Point) {
    trace!("view_request_resize: edge {:?}, to {}, start interactive mode.",
             edge, location);
}

pub extern fn keyboard_key(_view: WlcView, _time: u32, mods: &KeyboardModifiers,
                       key: u32, state: KeyState) -> bool {
    if state == KeyState::Pressed {
        // TODO this function will throw an error in Rustwlc right now
        // let mut keys = keyboard::get_current_keys().into_iter()
        //      .map(|&k| Keysym::from(k)).collect();
        let sym = keyboard::get_keysym_for_key(key, &KeyMod::empty());
        let mut keys = vec![sym];

        let press = KeyPress::new(mods.mods, keys);
        trace!("keypress: {:?}", press);

        if let Some(action) = keys::get(&press) {
            info!("[key] Found a key!");
            action();
            return EVENT_HANDLED;
        }
        else {
            trace!("keypress: No callback");
        }
    }

    return EVENT_PASS_THROUGH;
}

pub extern fn pointer_button(view: WlcView, _time: u32,
                         mods: &KeyboardModifiers, button: u32,
                         state: ButtonState, point: &Point) -> bool {
    if state == ButtonState::Pressed {
        trace!("button: 0x{:x}, point {}", &button, point);

        if !view.is_root() {
            view.focus();
        }

        if button == 0xfee9 {
            trace!("Found left click?");
        }

        //let sym = keyboard::get_keysym_for_key(key, &KeyMod::empty());
        let sym = Keysym::from(button);
        let press = KeyPress::new(mods.mods, vec![sym.clone()]);
        trace!("sym: {} {:?}", &sym.get_name().unwrap(), press);
    }
    false
}

pub extern fn pointer_scroll(_view: WlcView, button: u32,
                         _mods_ptr: &KeyboardModifiers, axis: ScrollAxis,
                         heights: [f64; 2]) -> bool {
    trace!("pointer_scroll: press {}, {:?} to {:?}", button, axis, heights);
    false
}

pub extern fn pointer_motion(_view: WlcView, _time: u32, point: &Point) -> bool {
    pointer::set_position(point);
    false
}

/*
pub extern fn touch(view: WlcView, time: u32, mods_ptr: &KeyboardModifiers,
               touch: TouchType, key: i32, point_ptr: &Point) -> bool {
    false
}
*/

pub extern fn compositor_ready() {
    info!("Preparing compositor!");
    info!("Initializing lua...");
    lua::init();
}
