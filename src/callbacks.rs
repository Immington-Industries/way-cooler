//! Callback methods for rustwlc
use rustwlc::handle::{WlcOutput, WlcView};
use rustwlc::types::*;
use rustwlc::input::{pointer, keyboard};


use compositor;
use super::keys;
use super::lua;
use super::keys::KeyPress;
use super::layout::tree;

/// If the event is handled by way-cooler
const EVENT_HANDLED: bool = true;

/// If the event should be passed through to clients
const EVENT_PASS_THROUGH: bool = false;

// wlc callbacks

pub extern fn output_created(output: WlcOutput) -> bool {
    trace!("output_created: {:?}: {}", output, output.get_name());
    {
        if let Ok(mut tree) = tree::try_lock_tree() {
            tree.add_output(output.clone());
            tree.switch_to_workspace(&"1");
            true
        } else {
            false
        }
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
    if let Ok(mut tree) = tree::try_lock_tree() {
        tree.add_view(view.clone());
        view.set_mask(output.get_mask());
        view.bring_to_front();
        view.focus();
        true
    } else {
        false
    }
}

pub extern fn view_destroyed(view: WlcView) {
    trace!("view_destroyed: {:?}", view);
    if let Ok(mut tree) = tree::try_lock_tree() {
        tree.remove_view(&view);
    } else {
        warn!("Could not delete view {:?}", view);
    }
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
        let keys = vec![sym];

        let press = KeyPress::new(mods.mods, keys);
        if let Some(action) = keys::get(&press) {
            info!("[key] Found an action for {:?}", press);
            action();
            return EVENT_HANDLED;
        }
    }

    return EVENT_PASS_THROUGH;
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
}

pub extern fn compositor_terminating() {
    info!("Compositor terminating!");
    lua::send(lua::LuaQuery::Terminate).ok();
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
