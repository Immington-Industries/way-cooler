//! Callback methods for rustwlc
use rustwlc::handle::{WlcOutput, WlcView};
use rustwlc::types::*;
use rustwlc::input::{pointer, keyboard};

use registry::{self, RegistryGetData};

use rustc_serialize::json::Json;
use std::sync::Arc;

use std::thread;

use compositor;
use super::keys::{self, KeyPress, KeyEvent};
use super::layout::{try_lock_tree, ContainerType};
use super::lua::{self, LuaQuery};
use super::background;

/// If the event is handled by way-cooler
const EVENT_HANDLED: bool = true;

/// If the event should be passed through to clients
const EVENT_PASS_THROUGH: bool = false;

// wlc callbacks

pub extern fn output_created(output: WlcOutput) -> bool {
    trace!("output_created: {:?}: {}", output, output.get_name());
    if let Ok(mut tree) = try_lock_tree() {
        tree.add_output(output).and_then(|_|{
            tree.switch_to_workspace(&"1")
        }).is_ok()
    } else {
        false
    }
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
    // Update the resolution of the output and its children
    output.set_resolution(new_size_ptr.clone());
    if let Ok(mut tree) = try_lock_tree() {
        tree.layout_active_of(ContainerType::Output)
            .expect("Could not layout active output");
    }
}

pub extern fn view_created(view: WlcView) -> bool {
    trace!("view_created: {:?}: \"{}\"", view, view.get_title());
    if let Ok(mut tree) = try_lock_tree() {
        tree.add_view(view.clone()).and_then(|_| {
            tree.set_active_view(view)
        }).is_ok()
    } else {
        false
    }
}

pub extern fn view_destroyed(view: WlcView) {
    trace!("view_destroyed: {:?}", view);
    if let Ok(mut tree) = try_lock_tree() {
        tree.remove_view(view.clone()).and_then(|_| {
            tree.layout_active_of(ContainerType::Workspace)
        }).unwrap_or_else(|err| {
            error!("Error in view_destroyed: {}", err);
        });
    } else {
        error!("Could not delete view {:?}", view);
    }
}

pub extern fn view_focus(current: WlcView, focused: bool) {
    trace!("view_focus: {:?} {}", current, focused);
    current.set_state(VIEW_ACTIVATED, focused);
    // set the focus view in the tree
    // If tree is already grabbed,
    // it should have the active container all set
    if let Ok(mut tree) = try_lock_tree() {
        if tree.set_active_view(current.clone()).is_err() {
            error!("Could not layout {:?}", current);
        }
    }
}

pub extern fn view_move_to_output(current: WlcView,
                                  o1: WlcOutput, o2: WlcOutput) {
    trace!("view_move_to_output: {:?}, {:?}, {:?}", current, o1, o2);
}

pub extern fn view_request_geometry(view: WlcView, geometry: &Geometry) {
    trace!("view_request_geometry: {:?} wants {:?}", view, geometry);
    warn!("Denying view {} request for size", view.get_title());
}

pub extern fn view_request_state(view: WlcView, state: ViewState, handled: bool) {
    trace!("view_request_state: {}, {:?}, handled: {}",
    view.get_title(), state, handled);
    view.set_state(state, handled);
}

pub extern fn view_request_move(view: WlcView, dest: &Point) {
    // Called by views when they have a dang resize mouse thing, we should only
    // let it happen in view floating mode
    compositor::start_interactive_move(&view, dest);
    trace!("view_request_move: to {}", *dest);
}

pub extern fn view_request_resize(view: WlcView,
                              edge: ResizeEdge, location: &Point) {
    compositor::start_interactive_resize(&view, edge, location);
    trace!("view_request_resize: edge {:?}, to {}",
             edge, location);
}

pub extern fn keyboard_key(_view: WlcView, _time: u32, mods: &KeyboardModifiers,
                       key: u32, state: KeyState) -> bool {
    if state == KeyState::Pressed {
        // TODO this function will throw an error in Rustwlc right now
        // let mut keys = keyboard::get_current_keys().into_iter()
        //      .map(|&k| Keysym::from(k)).collect();
        let sym = keyboard::get_keysym_for_key(key, &KeyMod::empty());
        let press = KeyPress::new(mods.mods, sym);
        if let Some(action) = keys::get(&press) {
            debug!("[key] Found an action for {}", press);
            match action {
                KeyEvent::Command(func) => {
                    func();
                },
                KeyEvent::Lua => {
                    match lua::send(LuaQuery::HandleKey(press)) {
                        Ok(_) => {},
                        Err(err) => {
                            // We may want to wait for Lua's reply from
                            // keypresses; for example if the table is tampered
                            // with or Lua is restarted or Lua has an error.
                            // ATM Lua asynchronously logs this but in the future
                            // an error popup/etc is a good idea.
                            error!("Error sending keypress: {:?}", err);
                        }
                    }
                }
            }
            return EVENT_HANDLED
        }
    }

    return EVENT_PASS_THROUGH
}

pub extern fn pointer_button(view: WlcView, _time: u32,
                         mods: &KeyboardModifiers, button: u32,
                             state: ButtonState, point: &Point) -> bool {
    compositor::on_pointer_button(view, _time, mods, button, state, point)
}

pub extern fn pointer_scroll(_view: WlcView, _time: u32,
                         _mods_ptr: &KeyboardModifiers, axis: ScrollAxis,
                         heights: [f64; 2]) -> bool {
    trace!("pointer_scroll: {:?} {:?}", axis,
           heights.iter().map(|f| f.clone().round()).collect::<Vec<f64>>());
    false
}

pub extern fn pointer_motion(_view: WlcView, _time: u32, point: &Point) -> bool {
    pointer::set_position(point);
    compositor::on_pointer_motion(_view, _time, point)
}

/*
pub extern fn touch(view: WlcView, time: u32, mods_ptr: &KeyboardModifiers,
               touch: TouchType, key: i32, point_ptr: &Point) -> bool {
    false
}
*/

pub extern fn compositor_ready() {
    info!("Preparing compositor!");
    info!("Initializing Lua...");
    lua::init();
    info!("Loading background...");
    let maybe_color: Result<Arc<Json>, ()> = registry::get_data("background")
        .map(RegistryGetData::resolve).and_then(|(_, data)| {
            Ok(data)
        }).map_err(|_| ());
    if let Ok(color) = maybe_color {
        match *color {
            Json::F64(hex_color) => {
                for output in WlcOutput::list() {
                    let color = background::Color::from_u32(hex_color as u32);
                    // different thread for each output.
                    thread::spawn(move || {background::generate_solid_background(color, output.clone());});
                }
            }
            _ => {
                error!("Non-solid color backgrounds not yet supported, {:?}", color);
            }
        }
    } else {
        warn!("Couldn't read background value");
    }
}

pub extern fn compositor_terminating() {
    info!("Compositor terminating!");
    lua::send(lua::LuaQuery::Terminate).ok();
    if let Ok(mut tree) = try_lock_tree() {
        if tree.destroy_tree().is_err() {
            error!("Could not destroy tree");
        }
    }

}


pub fn init() {
    use rustwlc::callback;

    callback::output_created(output_created);
    callback::output_destroyed(output_destroyed);
    callback::output_focus(output_focus);
    callback::output_resolution(output_resolution);
    callback::view_created(view_created);
    callback::view_destroyed(view_destroyed);
    callback::view_focus(view_focus);
    callback::view_move_to_output(view_move_to_output);
    callback::view_request_geometry(view_request_geometry);
    callback::view_request_state(view_request_state);
    callback::view_request_move(view_request_move);
    callback::view_request_resize(view_request_resize);
    callback::keyboard_key(keyboard_key);
    callback::pointer_button(pointer_button);
    callback::pointer_scroll(pointer_scroll);
    callback::pointer_motion(pointer_motion);
    callback::compositor_ready(compositor_ready);
    callback::compositor_terminate(compositor_terminating);
    trace!("Registered wlc callbacks");
}
