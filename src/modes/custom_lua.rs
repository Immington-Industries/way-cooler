//! A custom Lua callback mapping, where the default operation is done and
//! then a custom, user-defined Lua callback is ran.
use rustwlc::*;
use rustwlc::Size;
use ::lua::{self, LuaQuery};
use ::layout::{try_lock_tree, Handle};
use ::convert::json::{size_to_json, point_to_json, geometry_to_json};

use super::default::Default;
use super::Mode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustomLua;

impl Mode for CustomLua {
    /// Triggered when an output is connected.
    /// Sends the id of the created output to Lua.
    fn output_created(&mut self, output: WlcOutput) -> bool {
        let result = Default.output_created(output);
        if let Some(id) = lookup_handle(output.into()) {
            send_to_lua(format!("way_cooler.on_output_created(\"{}\")", id));
        }
        result
    }

    /// Triggered when an output is disconnected.
    /// Sends the id of the destroyed output to Lua.
    fn output_destroyed(&mut self, output: WlcOutput) {
        // Get the output ID before it's destroyed
        let id = lookup_handle(output.into());
        Default.output_destroyed(output);
        if let Some(id) = id {
            send_to_lua(format!("way_cooler.on_output_destroyed(\"{}\")", id))
        }
    }

    /// Triggered when an output loses or gains focus.
    /// Sends the id of the output, and whether it gained or lost focus.
    fn output_focused(&mut self, output: WlcOutput, focused: bool) {
        Default.output_focused(output, focused);
        if let Some(id) = lookup_handle(output.into()) {
            send_to_lua(format!("way_cooler.on_output_focused(\"{}\", {})",
                                id, focused));
        }
    }

    /// Triggered when an output's resolution changes.
    /// Sends the id of the output, and the old and new size of the output.
    fn output_resolution(&mut self, output: WlcOutput,
                         old_size: Size, new_size: Size) {
        Default.output_resolution(output, old_size, new_size);
        if let Some(id) = lookup_handle(output.into()) {
            send_to_lua(format!(
                "way_cooler.on_output_resolution_change(\"{}\", {}, {})",
                id, size_to_lua(old_size), size_to_lua(new_size)));
        }
    }

    /// Triggered after the output is rendered.
    /// Sends the id of the output after it's rendered.
    fn output_render_post(&mut self, output: WlcOutput) {
        Default.output_render_post(output);
        if let Some(id) = lookup_handle(output.into()) {
            send_to_lua(format!(
                "way_cooler.on_output_render_post(\"{}\")", id));
        }
    }

    /// Triggered when a view moves outputs.
    /// Sends the id of the view, the id of the old output, and the id of the
    /// the new output.
    fn view_moved_to_output(&mut self,
                            view: WlcView,
                            o1: WlcOutput,
                            o2: WlcOutput) {
        Default.view_moved_to_output(view, o1, o2);
        let (view_id, o1_id, o2_id) = (lookup_handle(view.into()),
                                       lookup_handle(o1.into()),
                                       lookup_handle(o2.into()));
        match (view_id, o1_id, o2_id) {
            (Some(view_id), Some(o1_id), Some(o2_id)) => {
                send_to_lua(format!(
                    "way_cooler.on_view_moved_to_output(\"{}\", \"{}\", \"{}\")",
                    view_id, o1_id, o2_id))
            },
            _ => {}
        }
    }

    /// Triggered when a view is created.
    /// Sends the id of the new view.
    fn view_created(&mut self, view: WlcView) -> bool {
        let result = Default.view_created(view);
        if let Some(id) = lookup_handle(view.into()) {
            send_to_lua(format!("way_cooler.on_view_created(\"{}\")", id))
        }
        result
    }

    /// Triggered when a view is destroyed.
    /// Sends the id of the view after its destroyed.
    fn view_destroyed(&mut self, view: WlcView) {
        // Get id before it's destroyed.
        let id = lookup_handle(view.into());
        Default.view_destroyed(view);
        if let Some(id) = id {
            send_to_lua(format!("way_cooler.on_view_destroyed(\"{}\")", id))
        }
    }

    /// Triggered when a view gains or loses focus.
    /// Sends the id of the view after its focus changes. The focus is true if
    /// it gained focus, or false if it lost focus.
    fn view_focused(&mut self, view: WlcView, focused: bool) {
        Default.view_focused(view, focused);
        if let Some(id) = lookup_handle(view.into()) {
            send_to_lua(format!("way_cooler.on_view_focused(\"{}\", {})",
                                id, focused))
        }
    }

    /// Triggered when the properties of a view changes.
    /// Sends the id of the view that had its properties changed, and the
    /// changed properties as a bit mask.
    fn view_props_changed(&mut self, view: WlcView, prop: ViewPropertyType) {
        Default.view_props_changed(view, prop);
        if let Some(id) = lookup_handle(view.into()) {
            send_to_lua(format!("way_cooler.on_view_props_changed(\"{}\", {})",
                                id, prop.bits()))
        }
    }

    /// Triggered when a view has its state change.
    /// Sends the id of the view, the bit field for the state field,
    /// and a bool representing how it was toggled.
    fn view_request_state(&mut self,
                          view: WlcView,
                          state: ViewState,
                          toggle: bool) {
        Default.view_request_state(view, state, toggle);
        if let Some(id) = lookup_handle(view.into()) {
            send_to_lua(format!(
                "way_cooler.on_view_request_state(\"{}\", {}, {})",
                id, state.bits(), toggle))
        }
    }

    /// Triggered when a view requests a different geometry.
    /// Sends the id of the view, and the new geometry.
    fn view_request_geometry(&mut self, view: WlcView, geometry: Geometry) {
        Default.view_request_geometry(view, geometry);
        if let Some(id) = lookup_handle(view.into()) {
            send_to_lua(format!(
                "way_cooler.on_view_request_geometry(\"{}\", {})",
                id, geometry_to_lua(geometry)))
        }
    }

    /// Triggered when a view requests to be moved, but not resized.
    /// Sends the id of the view, and the new destination point.
    fn view_request_move(&mut self, view: WlcView, dest: Point) {
        Default.view_request_move(view, dest);
        if let Some(id) = lookup_handle(view.into()) {
            send_to_lua(format!("way_cooler.on_view_request_move(\"{}\", {})",
                id, point_to_lua(dest)))
        }
    }

    /// Triggered when a view requests to be resized.
    /// Sends the id of the view, the bit field of the resize edge,
    /// and the point where the mouse resized.
    fn view_request_resize(&mut self,
                           view: WlcView,
                           edge: ResizeEdge,
                           point: Point) {
        Default.view_request_resize(view, edge, point);
        if let Some(id) = lookup_handle(view.into()) {
            send_to_lua(format!(
                "way_cooler.on_view_request_resize(\"{}\", {}, {})",
                id, edge.bits(), point_to_lua(point)))
        }
    }

    /// Triggered just before a view is rendered.
    /// Used to render the borders.
    /// Sends the id of the view that will be rendered.
    fn view_pre_render(&mut self, view: WlcView) {
        Default.view_pre_render(view);
        if let Some(id) = lookup_handle(view.into()) {
            send_to_lua(format!("way_cooler.on_view_pre_render(\"{}\")", id))
        }
    }

    /// Triggered when a keyboard key is clicked.
    /// Sends the id of the view that it was sent to,
    /// the key that was sent, any modifiers that were sent,
    /// and the state the key was in (up or down).
    fn on_keyboard_key(&mut self,
                       view: WlcView,
                       time: u32,
                       mods: KeyboardModifiers,
                       key: u32,
                       state: KeyState) -> bool {
        let result = Default.on_keyboard_key(view, time, mods, key, state);
        if let Some(id) = lookup_handle(view.into()) {
            send_to_lua(format!(
                "way_cooler.on_keyboard_key(\"{id}\", {key}, {mods}, {state})",
                id = id,
                key = key,
                mods = mods.mods.bits(),
                state = state as i32))
        }
        result
    }

    /// Triggered when a user presses a button on the mouse.
    /// Sends the UUID of the view clicked, any modifiers that were held
    /// down, where absolutely on the screen it was clicked, and what button
    /// was used to Lua.
    fn on_pointer_button(&mut self,
                         view: WlcView,
                         time: u32,
                         mods: KeyboardModifiers,
                         button: u32,
                         state: ButtonState,
                         point: Point) -> bool {
        let result = Default.on_pointer_button(view, time, mods, button, state, point);
        if let Some(id) = lookup_handle(view.into()) {
            send_to_lua(format!("way_cooler.on_pointer_button(\
                                 \"{view}\", {mods}, {pos}, {button})",
                                view = id,
                                mods = mods.mods.bits(),
                                pos = point_to_lua(point),
                                button = button))
        }
        result
    }

    /// Triggered when the user scrolls using a mouse.
    /// Sends the UUID of the view clicked, any modifiers that were held down,
    /// the axis of the scroll wheel, and both heights.
    fn on_pointer_scroll(&mut self,
                         view: WlcView,
                         time: u32,
                         mods: KeyboardModifiers,
                         axis: ScrollAxis,
                         heights: [f64; 2]) -> bool {
        let result = Default.on_pointer_scroll(view, time, mods, axis, heights);
        if let Some(id) = lookup_handle(view.into()) {
            send_to_lua(format!(
                "way_cooler.on_pointer_scroll(\"{}\", {}, {}, {}, {})",
                id, mods.mods.bits(), axis as i32, heights[0], heights[1]))
        }
        result
    }

    /// Triggerd when a user moves the mouse.
    /// Sends the UUID of the view where it was moved over,
    /// and the absolute point on the screen where it was.
    fn on_pointer_motion(&mut self,
                         view: WlcView,
                         time: u32,
                         x: f64,
                         y: f64) -> bool {
        let result = Default.on_pointer_motion(view, time, x, y);
        if let Some(id) = lookup_handle(view.into()) {
            send_to_lua(format!(
                "way_cooler.on_pointer_motion(\"{}\", {})",
                id, point_to_lua(Point::new(x as i32, y as i32))))
        }
        result
    }
}

/// Looks up the handle in the tree
fn lookup_handle(handle: Handle) -> Option<String> {
    if let Ok(tree) = try_lock_tree() {
        if let Ok(id) = tree.lookup_handle(handle.into()) {
            return Some(id.hyphenated().to_string())
        }
    }
    None
}

fn send_to_lua<Q: Into<String>>(msg: Q) {
    let msg = msg.into();
    lua::send(LuaQuery::Execute(msg.clone()));
}

fn size_to_lua(size: Size) -> String {
    format!("{}", size_to_json(size)).replace(":", "=").replace("\"", "")
}

fn point_to_lua(point: Point) -> String {
    format!("{}", point_to_json(point)).replace(":", "=").replace("\"", "")
}

fn geometry_to_lua(geometry: Geometry) -> String {
    format!("{}", geometry_to_json(geometry))
        .replace(":", "=").replace("\"", "")
}
