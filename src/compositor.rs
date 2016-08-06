//! Compositor: wm state including the layout, helm, and interactions
//! state machines.

use std::sync::RwLock;

use rustwlc;
use rustwlc::*;
use layout::try_lock_tree;

lazy_static! {
    static ref COMPOSITOR: RwLock<Compositor> = RwLock::new(Compositor::new());
}

const ERR_LOCK: &'static str = "Unable to lock compositor!";

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
        if let Ok(mut tree) = try_lock_tree() {
            if let Err(err) = tree.set_active_view(view.clone()) {
                error!("start_interactive_move error: {}", err);
                return false
            } else {
                return true
            }
        }
    }
    return false
}

/// Performs an operation on a pointer button, to be used in the callback
pub fn on_pointer_button(view: WlcView, _time: u32, _mods: &KeyboardModifiers,
                         _button: u32, state: ButtonState, _point: &Point)
                         -> bool {
    if state == ButtonState::Pressed {
        if !view.is_root() {
            if let Ok(mut tree) = try_lock_tree() {
                tree.set_active_view(view.clone())
                    .unwrap_or_else(|err| {
                        error!("Could not set active container {:?}", err);
                    });
            }
        }
    }
    else {
        stop_interactive_action();
    }

    let comp = COMPOSITOR.read().expect(ERR_LOCK);
    return comp.view.is_some()
}

/// Performs an operation on a pointer motion, to be used in the callback
pub fn on_pointer_motion(_view: WlcView, _time: u32, point: &Point) -> bool {
    rustwlc::input::pointer::set_position(point);
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
        if let Ok(mut tree) = try_lock_tree() {
            return Ok(try!(tree.set_active_view(view.clone()).map_err(|_| {
                "start_interactive_action: Could not start move"
            })));
        }
    }
    Ok(())
}

