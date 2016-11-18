#![allow(deprecated)] // keysyms

//! Callback methods for rustwlc
use rustwlc::handle::{WlcOutput, WlcView};
use rustwlc::types::*;
use rustwlc::input::{pointer, keyboard};

use rustc_serialize::json::Json;
use std::sync::Arc;
use std::thread;

use registry::{self, RegistryGetData};
use super::keys::{self, KeyPress, KeyEvent};
use super::layout::{Action, try_lock_tree, try_lock_action, ContainerType, MovementError, TreeError};
use super::layout::commands::set_performing_action;
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
    output.set_resolution(*new_size_ptr, 1);
    if let Ok(mut tree) = try_lock_tree() {
        tree.layout_active_of(ContainerType::Output)
            .expect("Could not layout active output");
    }
}

pub extern fn view_created(view: WlcView) -> bool {
    trace!("view_created: {:?}: \"{}\"", view, view.get_title());
    if let Ok(mut tree) = try_lock_tree() {
        tree.add_view(view.clone()).and_then(|_| {
            if view.get_class() == "Background" {
                return Ok(())
            }
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
            match err {
                TreeError::ViewNotFound(_) => {},
                _ => {
                    error!("Error in view_destroyed: {:?}", err);
                }
            }
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

pub extern fn view_request_state(view: WlcView, state: ViewState, handled: bool) {
    view.set_state(state, handled);
}

pub extern fn view_request_move(view: WlcView, _dest: &Point) {
    if let Ok(mut tree) = try_lock_tree() {
        if let Err(err) = tree.set_active_view(view) {
            error!("view_request_move error: {:?}", err);
        }
    }
}

pub extern fn view_request_resize(view: WlcView, edge: ResizeEdge, point: &Point) {
    if let Ok(mut tree) = try_lock_tree() {
        match try_lock_action() {
            Ok(guard) => {
                if guard.is_some() {
                    if let Some(id) = tree.lookup_view(view) {
                        if let Err(err) = tree.resize_container(id, edge, *point) {
                            error!("Problem: Command returned error: {:#?}", err);
                        }
                    }
                }
            },
            _ => {}
        }
    }
}

#[allow(non_snake_case)] // EMPTY_MODS will be a static once we have KEY_LED_NONE
pub extern fn keyboard_key(_view: WlcView, _time: u32, mods: &KeyboardModifiers,
                           key: u32, state: KeyState) -> bool {
    let EMPTY_MODS: KeyboardModifiers = KeyboardModifiers {
            mods: MOD_NONE,
            leds: KeyboardLed::empty()
    };
    let sym = keyboard::get_keysym_for_key(key, EMPTY_MODS);
    let press = KeyPress::new(mods.mods, sym);

    if state == KeyState::Pressed {
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

pub extern fn view_request_geometry(_view: WlcView, _geometry: &Geometry) {
}

pub extern fn pointer_button(view: WlcView, _time: u32,
                         mods: &KeyboardModifiers, button: u32,
                             state: ButtonState, point: &Point) -> bool {
    if state == ButtonState::Pressed {
        if button == 0x110 && !view.is_root() {
            if let Ok(mut tree) = try_lock_tree() {
                tree.set_active_view(view).ok();
                if mods.mods.contains(MOD_CTRL) {
                    let action = Action {
                        view: view,
                        grab: *point,
                        edges: ResizeEdge::empty()
                    };
                    set_performing_action(Some(action));
                }
            }
        } else if button == 0x111 && !view.is_root() {
            if let Ok(mut tree) = try_lock_tree() {
                tree.set_active_view(view).ok();
            }
            // TODO Make this set in the config file and read here.
            if mods.mods.contains(MOD_CTRL) {
                let action = Action {
                    view: view,
                    grab: *point,
                    edges: ResizeEdge::empty()
                };
                set_performing_action(Some(action));
                let geometry = view.get_geometry()
                    .expect("Could not get geometry of the view");
                let halfw = geometry.origin.x + geometry.size.w as i32 / 2;
                let halfh = geometry.origin.y + geometry.size.h as i32 / 2;

                {
                    let mut action: Action = try_lock_action().ok().and_then(|guard| *guard)
                        .unwrap_or(Action {
                            view: view,
                            grab: *point,
                            edges: ResizeEdge::empty()
                        });
                    let flag_x = if point.x < halfw {
                        RESIZE_LEFT
                    } else if point.x > halfw {
                        RESIZE_RIGHT
                    } else {
                        ResizeEdge::empty()
                    };

                    let flag_y = if point.y < halfh {
                        RESIZE_TOP
                    } else if point.y > halfh {
                        RESIZE_BOTTOM
                    } else {
                        ResizeEdge::empty()
                    };

                    action.edges = flag_x | flag_y;
                    set_performing_action(Some(action));
                }
                view.set_state(VIEW_RESIZING, true);
                return true;
            }
        }
    } else {
        if let Ok(lock) = try_lock_action() {
            match *lock {
                Some(action) => {
                    let view = action.view;
                    if view.get_state().contains(VIEW_RESIZING) {
                        view.set_state(VIEW_RESIZING, false);
                    }
                },
                _ => {}
            }
        }
        set_performing_action(None);
    }
    false
}

pub extern fn pointer_scroll(_view: WlcView, _time: u32,
                         _mods_ptr: &KeyboardModifiers, _axis: ScrollAxis,
                         _heights: [f64; 2]) -> bool {
    false
}

pub extern fn pointer_motion(view: WlcView, _time: u32, point: &Point) -> bool {
    let mut result = false;
    let mut maybe_action = None;
    {
        if let Ok(action_lock) = try_lock_action() {
            maybe_action = action_lock.clone();
        }
    }
    match maybe_action {
        None => result = false,
        Some(action) => {
            if action.edges.bits() != 0 {
                if let Ok(mut tree) = try_lock_tree() {
                    // TODO Change to id of _view
                    // Need to implement a map of view to uuid first though...
                    if let Some(active_id) = tree.lookup_view(view) {
                        match tree.resize_container(active_id, action.edges, *point) {
                            // Return early here to not set the pointer
                            Ok(_) => return true,
                            Err(err) => error!("Error: {:#?}", err)
                        }
                    }
                }
            } else {
                if let Ok(mut tree) = try_lock_tree() {
                    match tree.try_drag_active(*point) {
                        Ok(_) => result = true,
                        Err(TreeError::PerformingAction(_)) |
                        Err(TreeError::Movement(MovementError::NotFloating(_))) => result = false,
                        Err(err) => {
                            error!("Error: {:#?}", err);
                            result = false
                        }
                    }
                }
            }
        }
    }
    pointer::set_position(*point);
    result
}

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
