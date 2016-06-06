//! Compositor: wm state including the layout, helm, and interactions
//! state machines.

use std::sync::RwLock;

use rustwlc;
use rustwlc::*;
use layout::tree;

lazy_static! {
    static ref COMPOSITOR: RwLock<Compositor> = RwLock::new(Compositor::new());
}

const ERR_LOCK: &'static str = "Unable to lock compositor!";
const ERR_TREE: &'static str = "Unable to lock tree!";
const ERR_GEO: &'static str = "Unable to access view geometry!";

#[derive(Debug, PartialEq)]
pub struct Compositor {
    pub view: Option<WlcView>,
    pub grab: Point,
    pub edges: ResizeEdge,
    //pub actions: ClientState
}

impl Compositor {
    fn new() -> Compositor {
        Compositor {
            view: None,
            grab: Point {x: 0, y: 0},
            edges: ResizeEdge::empty()
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ClientState {
    view_action: ViewAction,
    next_action: ClickAction
}

impl Default for ClientState {
    fn default() -> ClientState {
        ClientState {
            view_action: ViewAction::None,
            next_action: ClickAction::None
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[allow(dead_code)]
pub enum ViewAction {
    None,
    Resize,
    Move
}

impl ViewAction {
    /// Is this ViewAction set
    #[allow(dead_code)]
    pub fn is_some(&self) -> bool {
        *self != ViewAction::None
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
/// Actions that the user may set before clicking on a window
#[allow(dead_code)]
enum ClickAction {
    None,
    CloseWindow,
    BeginMoving,
    BeginResizing,
    SelectWindow,

}

/// Maximizes the view to the size of the output it sits in
pub fn set_focused_window_maximized(wlc_view: &WlcView) {
    let maybe_geometry = wlc_view.get_geometry();
    if maybe_geometry.is_none() {
        return;
    }
    let geometry = maybe_geometry.expect(ERR_GEO);
    if start_interactive_action(wlc_view, &geometry.origin).is_err() {
        return;
    };
    {
        let mut comp = COMPOSITOR.write().expect(ERR_LOCK);
        if let Some(ref mut view) = comp.view {
            let output = view.get_output();
            trace!("Output size of the view: {:?}", output.get_resolution());
            let output_size = output.get_resolution();
            let geometry = Geometry { origin: Point { x: 0, y: 0},
                                        size: output_size.clone() };
            view.set_geometry(EDGE_NONE, &geometry);
        }
    }
    stop_interactive_action();
}

/// Makes the compositor no longer track the node to be used in some interaction
pub fn stop_interactive_action() {
    if let Ok(mut comp) = COMPOSITOR.write() {
        match comp.view {
            None => return,
            Some(ref view) => view.set_state(VIEW_RESIZING, false)
        }

        comp.view = None;
        comp.edges = ResizeEdge::empty();
    }
}

/// Automatically adds the view as the object of interest if there is no other
/// action currently being performed on some view
pub fn start_interactive_resize(view: &WlcView, edges: ResizeEdge, origin: &Point) {
    let geometry = match view.get_geometry() {
        None => { return; }
        Some(g) => g,
    };

    if start_interactive_action(view, origin).is_err() {
        return;
    }
    let halfw = geometry.origin.x + geometry.size.w as i32 / 2;
    let halfh = geometry.origin.y + geometry.size.h as i32 / 2;

    if let Ok(mut comp) = COMPOSITOR.write() {
        comp.edges = edges.clone();
        if comp.edges.bits() == 0 {
            let flag_x = if origin.x < halfw {
                RESIZE_LEFT
            } else if origin.x > halfw {
                RESIZE_RIGHT
            } else {
                ResizeEdge::empty()
            };

            let flag_y = if origin.y < halfh {
                RESIZE_TOP
            } else if origin.y > halfh {
                RESIZE_BOTTOM
            } else {
                ResizeEdge::empty()
            };

            comp.edges = flag_x | flag_y;
        }
    }
    view.set_state(VIEW_RESIZING, true);
}

/// Begin using the given view as the object of interest in an interactive
/// move. If another action is currently being performed,
/// this function returns false
pub fn start_interactive_move(view: &WlcView, origin: &Point) -> bool {
    if let Ok(mut comp) = COMPOSITOR.write() {
        if comp.view != None {
            return false;
        }
        comp.grab = origin.clone();
        comp.view = Some(view.clone());
        {
            let mut tree = tree::try_lock_tree().expect(ERR_TREE);
            tree.set_active_container(view.clone());
        }
        true
    } else {
        false
    }

}

/// Performs an operation on a pointer button, to be used in the callback
pub fn on_pointer_button(view: WlcView, _time: u32, mods: &KeyboardModifiers, button: u32,
                         state: ButtonState, point: &Point) -> bool {
    if state == ButtonState::Pressed {
        if !view.is_root() {
            view.focus();
            view.bring_to_front();
            if mods.mods.contains(MOD_CTRL) {
                // Button left, we need to include linux/input.h somehow
                if button == 0x110 {
                    start_interactive_move(&view, point);
                }
                if button == 0x111 {
                    start_interactive_resize(&view, ResizeEdge::empty(), point);
                }
                if mods.mods.contains(MOD_SHIFT) {
                    set_focused_window_maximized(&view);
                }
            }
        }
    }
    else {
        stop_interactive_action();
    }

    {
        let comp = COMPOSITOR.read().expect(ERR_LOCK);
        return comp.view.is_some();
    }
}

/// Performs an operation on a pointer motion, to be used in the callback
pub fn on_pointer_motion(_view: WlcView, _time: u32, point: &Point) -> bool {
    rustwlc::input::pointer::set_position(point);
    if let Ok(comp) = COMPOSITOR.read() {
        if let Some(ref view) = comp.view {
            let dx = point.x - comp.grab.x;
            let dy = point.y - comp.grab.y;
            let mut geo = view.get_geometry().expect(ERR_GEO).clone();
            if comp.edges.bits() != 0 {
                let min = Size { w: 80u32, h: 40u32};
                let mut new_geo = geo.clone();

                if comp.edges.contains(RESIZE_LEFT) {
                    if dx < 0 {
                        new_geo.size.w += dx.abs() as u32;
                    } else {
                        new_geo.size.w -= dx.abs() as u32;
                    }
                    new_geo.origin.x += dx;
                }
                else if comp.edges.contains(RESIZE_RIGHT) {
                    if dx < 0 {
                        new_geo.size.w -= dx.abs() as u32;
                    } else {
                        new_geo.size.w += dx.abs() as u32;
                    }
                }

                if comp.edges.contains(RESIZE_TOP) {
                    if dy < 0 {
                        new_geo.size.h += dy.abs() as u32;
                    } else {
                        new_geo.size.h -= dy.abs() as u32;
                    }
                    new_geo.origin.y += dy;
                }
                else if comp.edges.contains(RESIZE_BOTTOM) {
                    if dy < 0 {
                        new_geo.size.h -= dy.abs() as u32;
                    } else {
                        new_geo.size.h += dy.abs() as u32;
                    }
                }

                if new_geo.size.w >= min.w {
                    geo.origin.x = new_geo.origin.x;
                    geo.size.w = new_geo.size.w;
                }

                if new_geo.size.h >= min.h {
                    geo.origin.y = new_geo.origin.y;
                    geo.size.h = new_geo.size.h;
                }

                view.set_geometry(comp.edges, &geo);
            }
            else {
                geo.origin.x += dx;
                geo.origin.y += dy;
                view.set_geometry(ResizeEdge::empty(), &geo);
            }
        }
    }
    if let Ok(mut comp) = COMPOSITOR.write() {
        comp.grab = point.clone();
        comp.view.is_some()
    } else {
        false
    }
}

/// Sets the given view to be the object of interest in some interactive action.
/// If another view is currently being used as the object of interest, an Err
/// is returned.
fn start_interactive_action(view: &WlcView, origin: &Point) -> Result<(), &'static str> {
    if let Ok(mut comp) = COMPOSITOR.write() {
        if comp.view != None {
            return Err("Compositor already interacting with another view");
        }
        comp.grab = origin.clone();
        comp.view = Some(view.clone());
        {
            let mut tree = tree::try_lock_tree().expect(ERR_TREE);
            tree.set_active_container(view.clone());
        }
    }

    view.bring_to_front();
    Ok(())
}

