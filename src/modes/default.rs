//! Implementations of the default callbacks exposed by wlc.
//! These functions are the main entry points into Way Cooler from user action.
//! This is the default mode that Way Cooler is in at initilization
use rustwlc::*;
use rustwlc::input::{pointer, keyboard};
use rustwlc::render::{read_pixels, wlc_pixel_format};
use uuid::Uuid;

use super::{EVENT_BLOCKED, EVENT_PASS_THROUGH, LEFT_CLICK, RIGHT_CLICK};
use ::keys::{self, KeyPress, KeyEvent};
use ::layout::{lock_tree, try_lock_tree, try_lock_action, Action, ContainerType,
                    MovementError, TreeError, FocusError};
use ::layout::commands::set_performing_action;
use ::layout::MIN_SIZE;
use ::lua::{self, LuaQuery};

use ::render::screen_scrape::{SCRAPED_PIXELS, read_screen_scrape_lock,
                              sync_scrape};
use ::awesome;

use registry::{self};
use super::Mode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Default;

#[allow(unused)]
impl Mode for Default {
    fn output_created(&mut self, output: WlcOutput) -> bool {
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

    fn output_destroyed(&mut self, output: WlcOutput) {
         // TODO Redistribute workspaces of that output.
    }

    fn output_focused(&mut self, output: WlcOutput, focused: bool) {
         // TODO Ensure focused on right workspace
    }

    fn output_resolution(&mut self, output: WlcOutput,
                                    old_size_ptr: Size, new_size_ptr: Size) {
        // Update the resolution of the output and its children
        let scale = 1;
        output.set_resolution(new_size_ptr, scale);
        if let Ok(mut tree) = try_lock_tree() {
            tree.layout_active_of(ContainerType::Output)
                .expect("Could not layout active output");
        }
    }

    fn output_render_post(&mut self, output: WlcOutput) {
        let need_to_fetch = read_screen_scrape_lock();
        if *need_to_fetch {
            if let Ok(mut scraped_pixels) = SCRAPED_PIXELS.try_lock() {
                if scraped_pixels.1 != Some(output) {
                    return
                }
                let resolution = output.get_resolution()
                    .expect("Output had no resolution");
                let geo = Geometry {
                    origin: Point { x: 0, y: 0 },
                    size: resolution
                };
                let result = read_pixels(wlc_pixel_format::WLC_RGBA8888, geo).1;
                scraped_pixels.0 = result;
                sync_scrape();
            }
        }
    }

    fn view_created(&mut self, view: WlcView) -> bool {
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
                    let output = WlcOutput::focused();
                    tree.add_bar(view, output).unwrap_or_else(|_| {
                        warn!("Could not add bar {:#?} to output {:#?}", view, output);
                    });
                    return true;
                }
            }
        }
        if let Ok(mut tree) = lock_tree() {
            for output in WlcOutput::list() {
                if tree.add_background(view, output).expect("Couldn't try to add background") {
                    view.set_output(output);
                    view.send_to_back();
                    view.set_mask(1);
                    let resolution = output.get_resolution()
                        .expect("Couldn't get output resolution");
                    let fullscreen = Geometry {
                        origin: Point { x: 0, y: 0 },
                        size: resolution
                    };
                    view.set_geometry(ResizeEdge::empty(), fullscreen);
                    return true
                }
            }
        }
        if let Ok(mut tree) = lock_tree() {
            let result = tree.add_view(view).and_then(|_| {
                view.set_state(VIEW_MAXIMIZED, true);
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

    fn view_destroyed(&mut self, view: WlcView) {
        match try_lock_tree() {
            Ok(mut tree) => {
                tree.remove_view(view).unwrap_or_else(|err| {
                    match err {
                        TreeError::ViewNotFound(_) => {},
                        _ => {
                            warn!("Error in view_destroyed: {:?}", err);
                        }
                    }});
            },
            Err(err) => warn!("Could not delete view {:?}, {:?}", view, err)
        }
    }

    fn view_focused(&mut self, current: WlcView, focused: bool) {
        current.set_state(VIEW_ACTIVATED, focused);
        if let Ok(mut tree) = try_lock_tree() {
            match tree.set_active_view(current) {
                Ok(_) => {},
                Err(err) => {
                    warn!("Could not set {:?} to be active view: {:?}", current, err);
                }
            }
        }
    }

    fn view_props_changed(&mut self, view: WlcView, prop: ViewPropertyType) {
        if prop.contains(PROPERTY_TITLE) {
            if let Ok(mut tree) = try_lock_tree() {
                match tree.update_title(view) {
                    Ok(_) => {},
                    Err(err) => {
                        warn!("Could not update title for view {:?} because {:#?}",
                            view, err);
                    }
                }
            }
        }
    }

    fn view_moved_to_output(&mut self, view: WlcView, o1: WlcOutput, o2: WlcOutput) {
        // TODO Ensure in correct workspace
    }

    fn view_request_state(&mut self, view: WlcView, state: ViewState, toggle: bool) {
        if state == VIEW_FULLSCREEN {
            if let Ok(mut tree) = try_lock_tree() {
                if let Ok(id) = tree.lookup_handle(view.into()) {
                    tree.set_fullscreen(id, toggle)
                        .expect("The ID was related to a non-view, somehow!");
                    match tree.container_in_active_workspace(id) {
                        Ok(true) => {
                            tree.layout_active_of(ContainerType::Workspace)
                                .unwrap_or_else(|err| {
                                    warn!("Could not layout active workspace \
                                           for view {:?}: {:?}",
                                            view, err)
                                });
                        },
                        Ok(false) => {},
                        Err(err) => warn!("Could not set {:?} fullscreen: {:?}",
                                           view, err)
                    }
                } else {
                    warn!("Could not find view {:?} in tree", view);
                }
            }
        }
    }

    fn view_request_move(&mut self, view: WlcView, _dest: Point) {
        if let Ok(mut tree) = try_lock_tree() {
            if let Err(err) = tree.set_active_view(view) {
                warn!("view_request_move error: {:?}", err);
            }
        }
    }

    fn view_request_resize(&mut self, view: WlcView, edge: ResizeEdge, point: Point) {
        if let Ok(mut tree) = try_lock_tree() {
            let in_action = try_lock_action()
                .map(|action| action.is_some())
                .unwrap_or(false);
            if in_action {
                if let Ok(id) = tree.lookup_handle(view.into()) {
                    if let Err(err) = tree.resize_container(id, edge, point) {
                        warn!("resize_container returned error: {:#?}",
                                err);
                    }
                }
            }
        }
    }

    fn on_keyboard_key(&mut self, _view: WlcView, _time: u32, mods: KeyboardModifiers,
                            key: u32, state: KeyState) -> bool {
        let empty_mods: KeyboardModifiers = KeyboardModifiers {
                mods: MOD_NONE,
                leds: KeyboardLed::empty()
        };
        let sym = keyboard::get_keysym_for_key(key, empty_mods);
        let press = KeyPress::new(mods.mods, sym.clone());
        awesome::keygrabber_handle(mods, sym, state).unwrap_or_else(|err| {
            warn!("handling keygrabber returned error: {:#?}", err);
        });
        if state == KeyState::Pressed {
            if let Some(key) = keys::get(&press) {
                info!("[key] Found an action for {}, blocking event", press);
                let action = key.event;
                let passthrough = key.passthrough;
                match action {
                    KeyEvent::Command(func) => {
                        func();
                    },
                    KeyEvent::Lua => {
                        lua::send(LuaQuery::HandleKey(press));
                    }
                }
                return !passthrough
            }
        }

        return EVENT_PASS_THROUGH
    }

    fn view_request_geometry(&mut self, view: WlcView, geometry: Geometry) {
        let unmanaged = view.get_type().intersects(VIEW_BIT_UNMANAGED);
        if geometry.size.w < MIN_SIZE.w && geometry.size.h < MIN_SIZE.h && !unmanaged {
            warn!("Ignoring requested geometry {:#?}, which is below min {:#?}",
                  geometry, MIN_SIZE);
            return;
        }
        if let Ok(mut tree) = try_lock_tree() {
            match tree.update_floating_geometry(view, geometry) {
                Ok(()) | Err(TreeError::ViewNotFound(_)) => {},
                err => warn!("Could not find view {:#?} \
                              in order to update geometry w/ {:#?} \
                              because of {:#?}",
                             view, geometry, err)
            }
        }
    }

    fn on_pointer_button(&mut self, view: WlcView, _time: u32,
                            mods: KeyboardModifiers, button: u32,
                                state: ButtonState, point: Point) -> bool {
        awesome::mousegrabber_handle(point.x, point.y, Some((button, state)))
            .unwrap_or_else(|err|
                            warn!("handling keygrabber returned error: {:#?}", err));
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
                            grab: point,
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
                        grab: point,
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
                                grab: point,
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

    fn on_pointer_scroll(&mut self, _view: WlcView, _time: u32,
                            _mods_ptr: KeyboardModifiers, _axis: ScrollAxis,
                            _heights: [f64; 2]) -> bool {
        EVENT_PASS_THROUGH
    }

    fn on_pointer_motion(&mut self, view: WlcView, _time: u32, x: f64, y: f64) -> bool {
        let mut result = EVENT_PASS_THROUGH;
        let point = Point::new(x as i32, y as i32);
        let mut maybe_action = None;
        {
            if let Ok(action_lock) = try_lock_action() {
                maybe_action = action_lock.clone();
            }
        }
        awesome::mousegrabber_handle(x as i32, y as i32, None)
            .unwrap_or_else(|err|
                            warn!("handling keygrabber returned error: {:#?}", err));
        match maybe_action {
            None => result = EVENT_PASS_THROUGH,
            Some(action) => {
                if action.edges.bits() != 0 {
                    if let Ok(mut tree) = try_lock_tree() {
                        if let Ok(active_id) = tree.lookup_handle(view.into()) {
                            match tree.resize_container(active_id, action.edges, point) {
                                // Return early here to not set the pointer
                                Ok(_) => return EVENT_BLOCKED,
                                Err(err) => warn!("Could not resize: {:#?}", err)
                            }
                        }
                    }
                } else {
                    if let Ok(mut tree) = try_lock_tree() {
                        match tree.try_drag_active(point) {
                            Ok(_) => result = EVENT_BLOCKED,
                            Err(TreeError::PerformingAction(_)) |
                            Err(TreeError::Movement(MovementError::NotFloating(_))) =>
                                result = EVENT_PASS_THROUGH,
                            Err(err) => {
                                warn!("Unexpected drarg error: {:#?}", err);
                                result = EVENT_PASS_THROUGH
                            }
                        }
                    }
                }
            }
        }
        pointer::set_position_v2(x, y);
        result
    }

    fn view_pre_render(&mut self, view: WlcView) {
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
}
