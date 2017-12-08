//! Implementations of the callbacks exposed by wlc.
//! These functions are the main entry points into Way Cooler from user action.
use rustwlc::handle::{WlcOutput, WlcView};
use rustwlc::types::*;

use super::keys;
use super::layout::{lock_tree, try_lock_tree, TreeError};
use super::lua;

use ::modes::{write_current_mode, read_current_mode};


// NOTE That we don't hold the mode lock long, because wlc uses
// an immediate mode and it's possible to hold the lock twice
// if we don't copy the mode struct out.

pub extern fn output_created(output: WlcOutput) -> bool {
    trace!("output_created: {:?}: {}", output, output.get_name());
    let mut mode = read_current_mode().clone();
    mode.output_created(output)
}

pub extern fn output_destroyed(output: WlcOutput) {
    trace!("output_destroyed: {:?}", output);
    let mut mode = read_current_mode().clone();
    mode.output_destroyed(output)
}

pub extern fn output_focus(output: WlcOutput, focused: bool) {
    trace!("output_focus: {:?} focus={}", output, focused);
    let mut mode = read_current_mode().clone();
    mode.output_focused(output, focused)
}

pub extern fn output_resolution(output: WlcOutput,
                                old_size_ptr: &Size, new_size_ptr: &Size) {
    trace!("output_resolution: {:?} from  {:?} to {:?}",
           output, *old_size_ptr, *new_size_ptr);
    let mut mode = read_current_mode().clone();
    mode.output_resolution(output, *old_size_ptr, *new_size_ptr)
}

pub extern fn post_render(output: WlcOutput) {
    let mut mode = read_current_mode().clone();
    mode.output_render_post(output)
}

pub extern fn view_created(view: WlcView) -> bool {
    trace!("view_created: {:?}: \"{}\"", view, view.get_title());
    let mut mode;
    let res;
    {
        mode = read_current_mode().clone();
        res = mode.view_created(view);
    }
    *write_current_mode() = mode;
    res
}

pub extern fn view_destroyed(view: WlcView) {
    trace!("view_destroyed: {:?}", view);
    let mut mode;
    {
        mode = read_current_mode().clone();
        mode.view_destroyed(view);
    }
    *write_current_mode() = mode;
}

pub extern fn view_focus(current: WlcView, focused: bool) {
    trace!("view_focus: {:?} {}", current, focused);
    let mut mode = read_current_mode().clone();
    mode.view_focused(current, focused);
}

pub extern fn view_props_changed(view: WlcView, prop: ViewPropertyType) {
    trace!("view_props_changed for view {:?}: {:?}", view, prop);
    let mut mode = read_current_mode().clone();
    mode.view_props_changed(view, prop)
}

pub extern fn view_move_to_output(current: WlcView,
                                  o1: WlcOutput, o2: WlcOutput) {
    trace!("view_move_to_output: {:?}, {:?}, {:?}", current, o1, o2);
    let mut mode = read_current_mode().clone();
    mode.view_moved_to_output(current, o1, o2)
}

pub extern fn view_request_state(view: WlcView, state: ViewState, toggle: bool) {
    trace!("Setting {:?} to state {:?}", view, state);
    let mut mode = read_current_mode().clone();
    mode.view_request_state(view, state, toggle)
}

pub extern fn view_request_move(view: WlcView, dest: &Point) {
    trace!("View {:?} request to move to {:?}", view, dest);
    let mut mode = read_current_mode().clone();
    mode.view_request_move(view, *dest)
}

pub extern fn view_request_resize(view: WlcView, edge: ResizeEdge, point: &Point) {
    trace!("View {:?} request resize w/ edge {:?} to point {:?}",
           view, edge, point);
    let mut mode = read_current_mode().clone();
    mode.view_request_resize(view, edge, *point)
}

pub extern fn view_request_geometry(view: WlcView, geometry: &Geometry) {
    trace!("View {:?} requested geometry {:?}", view, geometry);
    let mut mode = read_current_mode().clone();
    mode.view_request_geometry(view, *geometry)
}

pub extern fn keyboard_key(view: WlcView, time: u32, mods: &KeyboardModifiers,
                           key: u32, state: KeyState) -> bool {
    let mut mode = read_current_mode().clone();
    mode.on_keyboard_key(view, time, *mods, key, state)
}

pub extern fn pointer_button(view: WlcView, time: u32,
                             mods: &KeyboardModifiers, button: u32,
                             state: ButtonState, point: &Point) -> bool {
    let mut mode = read_current_mode().clone();
    mode.on_pointer_button(view, time, *mods, button, state, *point)
}

pub extern fn pointer_scroll(view: WlcView, time: u32,
                         mods_ptr: &KeyboardModifiers, axis: ScrollAxis,
                         heights: [f64; 2]) -> bool {
    let mut mode = read_current_mode().clone();
    mode.on_pointer_scroll(view, time, *mods_ptr, axis, heights)
}

pub extern fn pointer_motion(view: WlcView, time: u32, x: f64, y: f64) -> bool {
    let mut mode = read_current_mode().clone();
    mode.on_pointer_motion(view, time, x, y)
}

pub extern fn compositor_ready() {
    info!("Preparing compositor!");
    lua::on_compositor_ready();
    keys::init();
}

pub extern fn compositor_terminating() {
    info!("Compositor terminating!");
    lua::send(lua::LuaQuery::Terminate).ok();
    if let Ok(mut tree) = try_lock_tree() {
        tree.destroy_tree().unwrap_or_else(|err|
            error!("Could not destroy tree: {:#?}", err)
        )
    }

}

pub extern fn view_pre_render(view: WlcView) {
    if let Ok(mut tree) = lock_tree() {
        tree.render_borders(view).unwrap_or_else(|err| {
            match err {
                // TODO Only filter if background or bar
                TreeError::ViewNotFound(_) => {},
                err => warn!("Error while rendering borders: {:?}", err)
            }
        })
    }
}


pub fn init() {
    use rustwlc::callback;

    callback::output_created(output_created);
    callback::output_destroyed(output_destroyed);
    callback::output_focus(output_focus);
    callback::output_resolution(output_resolution);
    callback::output_render_post(post_render);
    callback::view_created(view_created);
    callback::view_destroyed(view_destroyed);
    callback::view_focus(view_focus);
    callback::view_move_to_output(view_move_to_output);
    callback::view_request_geometry(view_request_geometry);
    callback::view_request_state(view_request_state);
    callback::view_request_move(view_request_move);
    callback::view_request_resize(view_request_resize);
    callback::view_properties_changed(view_props_changed);
    callback::keyboard_key(keyboard_key);
    callback::pointer_button(pointer_button);
    callback::pointer_scroll(pointer_scroll);
    callback::pointer_motion_v2(pointer_motion);
    callback::compositor_ready(compositor_ready);
    callback::compositor_terminate(compositor_terminating);
    callback::view_render_pre(view_pre_render);
    trace!("Registered wlc callbacks");
}
