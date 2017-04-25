//! Implementations of the callbacks exposed by wlc.
//! These functions are the main entry points into Way Cooler from user action.
#![allow(deprecated)] // keysyms

use rustwlc::handle::{WlcOutput, WlcView};
use rustwlc::types::{ButtonState, KeyboardModifiers, KeyState, KeyboardLed, ScrollAxis, Size,
                     Point, Geometry, ResizeEdge, ViewState, ViewPropertyType, PROPERTY_TITLE,
                     VIEW_MAXIMIZED, VIEW_ACTIVATED, VIEW_RESIZING, VIEW_FULLSCREEN,
                     MOD_NONE, RESIZE_LEFT, RESIZE_RIGHT, RESIZE_TOP, RESIZE_BOTTOM};
use rustwlc::input::{pointer, keyboard};
use rustwlc::render::{read_pixels, wlc_pixel_format};
use uuid::Uuid;

use super::keys::{self, KeyPress, KeyEvent};
use super::layout::{lock_tree, try_lock_tree, try_lock_action, Action, ContainerType,
                    MovementError, TreeError, FocusError};
use super::layout::commands::set_performing_action;
use super::lua::{self, LuaQuery};

use ::render::screen_scrape::{read_screen_scrape_lock, scraped_pixels_lock,
                              sync_scrape};

use registry::{self};

/// If the event is handled by way-cooler
const EVENT_BLOCKED: bool = true;

/// If the event should be passed through to clients
const EVENT_PASS_THROUGH: bool = false;

const LEFT_CLICK: u32 = 0x110;
const RIGHT_CLICK: u32 = 0x111;

pub extern fn output_created(output: WlcOutput) -> bool {
    trace!("output_created: {:?}: {}", output, output.get_name());
    if let Ok(mut tree) = try_lock_tree() {
        let result = tree.add_output(output).and_then(|_|{
            tree.switch_to_workspace(&"1")
                .map(|_| tree.layout_active_of(ContainerType::Output))
        });
        match result {
            // If the output exists, we just couldn't add it to the tree because
            // it's already there. That's OK
            Ok(_) | Err(TreeError::OutputExists(_)) => true,
            _ => false
        }
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
    let scale = 1;
    output.set_resolution(*new_size_ptr, scale);
    if let Ok(mut tree) = try_lock_tree() {
        tree.layout_active_of(ContainerType::Output)
            .expect("Could not layout active output");
    }
}

pub extern fn post_render(output: WlcOutput) {
    let need_to_fetch = read_screen_scrape_lock();
    if *need_to_fetch {
        if let Ok(mut scraped_pixels) = scraped_pixels_lock() {
            let resolution = output.get_resolution()
                .expect("Output had no resolution");
            let geo = Geometry {
                origin: Point { x: 0, y: 0 },
                size: resolution
            };
            let result = read_pixels(wlc_pixel_format::WLC_RGBA8888, geo).1;
            *scraped_pixels = result;
            sync_scrape();
        }
    }
}

pub extern fn view_created(view: WlcView) -> bool {
    debug!("view_created: {:?}: \"{}\"", view, view.get_title());
    let lock = registry::clients_read();
    let client = lock.client(Uuid::nil()).unwrap();
    let handle = registry::ReadHandle::new(&client);
    let bar = handle.read("programs".into())
        .expect("programs category didn't exist")
        .get("x11_bar".into())
        .and_then(|data| data.as_string().map(str::to_string));
    // TODO Move this hack, probably could live somewhere else
    if let Some(bar_name) = bar {
        if view.get_title().as_str() == bar_name {
            view.set_mask(1);
            view.bring_to_front();
            if let Ok(mut tree) = try_lock_tree() {
                for output in WlcOutput::list() {
                    tree.add_bar(view, output).unwrap_or_else(|_| {
                        warn!("Could not add bar {:#?} to output {:#?}", view, output);
                    });
                }
                return true;
            }
        }
    }
    // TODO Remove this hack
    if view.get_class().as_str() == "Background" {
        debug!("Setting background: {}", view.get_title());
        view.send_to_back();
        view.set_mask(1);
        let output = view.get_output();
        let resolution = output.get_resolution()
            .expect("Couldn't get output resolution");
        let fullscreen = Geometry {
            origin: Point { x: 0, y: 0 },
            size: resolution
        };
        view.set_geometry(ResizeEdge::empty(), fullscreen);
        if let Ok(mut tree) = lock_tree() {
            let outputs = tree.outputs();
            return tree.add_background(view, outputs.as_slice()).map(|_| true)
                .unwrap_or_else(|err| {
                    error!("Could not add background due to {:?}", err);
                    true
                })
        } else {
            error!("Could not lock tree");
        }
        return false
    }
    if let Ok(mut tree) = lock_tree() {
        let result = tree.add_view(view).and_then(|_| {
            // view.set_state(VIEW_MAXIMIZED, true);
            match tree.set_active_view(view) {
                // If blocked by fullscreen, we don't focus on purpose
                Err(TreeError::Focus(FocusError::BlockedByFullscreen(_, _))) => Ok(()),
                result => result
            }
        });
        if result.is_err() {
            warn!("Could not add {:?}. Reason: {:?}", view, result);
        }
        result.is_ok()
    } else {
        false
    }
}

pub extern fn view_destroyed(view: WlcView) {
    trace!("view_destroyed: {:?}", view);
    match try_lock_tree() {
        Ok(mut tree) => {
            tree.remove_view(view).unwrap_or_else(|err| {
                match err {
                    TreeError::ViewNotFound(_) => {},
                    _ => {
                        error!("Error in view_destroyed: {:?}", err);
                    }
                }});
        },
        Err(err) => error!("Could not delete view {:?}, {:?}", view, err)
    }
}

pub extern fn view_focus(current: WlcView, focused: bool) {
    trace!("view_focus: {:?} {}", current, focused);
    current.set_state(VIEW_ACTIVATED, focused);
    if let Ok(mut tree) = try_lock_tree() {
        match tree.set_active_view(current) {
            Ok(_) => {},
            Err(err) => {
                error!("Could not set {:?} to be active view: {:?}", current, err);
            }
        }
    }
}

pub extern fn view_props_changed(view: WlcView, prop: ViewPropertyType) {
    if prop.contains(PROPERTY_TITLE) {
        if let Ok(mut tree) = try_lock_tree() {
            match tree.update_title(view) {
                Ok(_) => {},
                Err(err) => {
                    error!("Could not update title for view {:?} because {:#?}",
                           view, err);
                }
            }
        }
    }
}

pub extern fn view_move_to_output(current: WlcView,
                                  o1: WlcOutput, o2: WlcOutput) {
    trace!("view_move_to_output: {:?}, {:?}, {:?}", current, o1, o2);
}

pub extern fn view_request_state(view: WlcView, state: ViewState, toggle: bool) {
    trace!("Setting {:?} to state {:?}", view, state);
    if state == VIEW_FULLSCREEN {
        if let Ok(mut tree) = try_lock_tree() {
            if let Ok(id) = tree.lookup_view(view) {
                tree.set_fullscreen(id, toggle)
                    .expect("The ID was related to a non-view, somehow!");
                match tree.container_in_active_workspace(id) {
                    Ok(true) => {
                        tree.layout_active_of(ContainerType::Workspace)
                            .unwrap_or_else(|err| {
                                error!("Could not layout active workspace for view {:?}: {:?}",
                                        view, err)
                            });
                    },
                    Ok(false) => {},
                    Err(err) => error!("Could not set {:?} fullscreen: {:?}", view, err)
                }
            } else {
                warn!("Could not find view {:?} in tree", view);
            }
        }
    }
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
                    if let Ok(id) = tree.lookup_view(view) {
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

pub extern fn keyboard_key(_view: WlcView, _time: u32, mods: &KeyboardModifiers,
                           key: u32, state: KeyState) -> bool {
    let empty_mods: KeyboardModifiers = KeyboardModifiers {
            mods: MOD_NONE,
            leds: KeyboardLed::empty()
    };
    let sym = keyboard::get_keysym_for_key(key, empty_mods);
    let press = KeyPress::new(mods.mods, sym);

    if state == KeyState::Pressed {
        if let Some(action) = keys::get(&press) {
            info!("[key] Found an action for {}, blocking event", press);
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
            return EVENT_BLOCKED
        }
    }

    return EVENT_PASS_THROUGH
}

pub extern fn view_request_geometry(view: WlcView, geometry: &Geometry) {
    if let Ok(mut tree) = try_lock_tree() {
        tree.update_floating_geometry(view, *geometry).unwrap_or_else(|_| {
            warn!("Could not find view {:#?} \
                   in order to update geometry w/ {:#?}",
                  view, *geometry);
        });
    }
}

pub extern fn pointer_button(view: WlcView, _time: u32,
                         mods: &KeyboardModifiers, button: u32,
                             state: ButtonState, point: &Point) -> bool {
    if state == ButtonState::Pressed {
        let mouse_mod = keys::mouse_modifier();
        if button == LEFT_CLICK && !view.is_root() {
            if let Ok(mut tree) = try_lock_tree() {
                tree.set_active_view(view).unwrap_or_else(|_| {
                    // still focus on view, even if not in tree.
                    view.focus();
                });
                if mods.mods.contains(mouse_mod) {
                    let action = Action {
                        view: view,
                        grab: *point,
                        edges: ResizeEdge::empty()
                    };
                    set_performing_action(Some(action));
                }
            }
        } else if button == RIGHT_CLICK && !view.is_root() {
            info!("User right clicked w/ mods \"{:?}\" on {:?}", mods, view);
            if let Ok(mut tree) = try_lock_tree() {
                tree.set_active_view(view).ok();
            }
            if mods.mods.contains(mouse_mod) {
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
                return EVENT_BLOCKED
            }
        }
    } else {
        if let Ok(lock) = try_lock_action() {
            let unknown = format!("unknown ({})", button);
            info!("User released {:?} mouse button",
                  match button {
                      RIGHT_CLICK => "right",
                      LEFT_CLICK => "left",
                      _ => unknown.as_str()
                  });
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
    EVENT_PASS_THROUGH
}

pub extern fn pointer_scroll(_view: WlcView, _time: u32,
                         _mods_ptr: &KeyboardModifiers, _axis: ScrollAxis,
                         _heights: [f64; 2]) -> bool {
    EVENT_PASS_THROUGH
}

pub extern fn pointer_motion(view: WlcView, _time: u32, point: &Point) -> bool {
    let mut result = EVENT_PASS_THROUGH;
    let mut maybe_action = None;
    {
        if let Ok(action_lock) = try_lock_action() {
            maybe_action = action_lock.clone();
        }
    }
    match maybe_action {
        None => result = EVENT_PASS_THROUGH,
        Some(action) => {
            if action.edges.bits() != 0 {
                if let Ok(mut tree) = try_lock_tree() {
                    if let Ok(active_id) = tree.lookup_view(view) {
                        match tree.resize_container(active_id, action.edges, *point) {
                            // Return early here to not set the pointer
                            Ok(_) => return EVENT_BLOCKED,
                            Err(err) => warn!("Could not resize: {:#?}", err)
                        }
                    }
                }
            } else {
                if let Ok(mut tree) = try_lock_tree() {
                    match tree.try_drag_active(*point) {
                        Ok(_) => result = EVENT_BLOCKED,
                        Err(TreeError::PerformingAction(_)) |
                        Err(TreeError::Movement(MovementError::NotFloating(_))) =>
                            result = EVENT_PASS_THROUGH,
                        Err(err) => {
                            error!("Error: {:#?}", err);
                            result = EVENT_PASS_THROUGH
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
    keys::init();
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
    callback::pointer_motion(pointer_motion);
    callback::compositor_ready(compositor_ready);
    callback::compositor_terminate(compositor_terminating);
    callback::view_render_pre(view_pre_render);
    trace!("Registered wlc callbacks");
}
