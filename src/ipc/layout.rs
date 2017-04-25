#![allow(unused_variables, dead_code)] // macros

/// Dbus macro for Layout code

use super::utils::{parse_edge, parse_uuid, parse_direction, parse_axis, lock_tree_dbus};

use dbus::tree::MethodErr;

use super::super::layout::{Layout, commands as layout_cmd};
use rustwlc::{ResizeEdge, Point};

dbus_interface! {
    path: "/org/way_cooler/Layout";
    name: "org.way_cooler.Layout";

    fn ActiveContainerId() -> container_id: DBusResult<String> {
        let tree = try!(lock_tree_dbus());
        match tree.active_id() {
            Some(id) => Ok(id.to_string()),
            None => Ok("".to_string())
        }
    }

    fn ToggleFloat(container_id: String) -> success: DBusResult<bool> {
        let maybe_uuid = try!(parse_uuid("container_id", &container_id));
        match maybe_uuid {
            Some(uuid) => {
                let mut tree = try!(lock_tree_dbus());
                tree.toggle_float()
                    .and(Ok(true))
                    .map_err(|err| {
                        MethodErr::failed(&format!("{:?}", err))
                    })
            },
            None => {
                layout_cmd::toggle_float();
                Ok(true)
            }
        }
    }

    fn MoveContainer(container_id: String, direction: String) -> success: DBusResult<bool> {
        let target_uuid = try!(parse_uuid("container_id", &container_id));
        let direction = try!(parse_direction("direction", direction.as_str()));
        let mut tree = try!(lock_tree_dbus());
        tree.move_active(target_uuid, direction)
            .and(Ok(true))
            .map_err(|err| MethodErr::failed(&format!("{:?}", err)))
    }

    fn SplitContainer(container_id: String, split_axis: String) -> success: DBusResult<bool> {
        let uuid = try!(parse_uuid("container_id", &container_id));
        let axis = try!(parse_axis("split_direction", split_axis.as_str()));
        // TODO Tree commands need to have these defined on the Tree,
        // for now this is _ok_, but we are swallowing an potential Tree lock error here.
        match axis {
            Layout::Scroll => layout_cmd::split_horizontal(),
            Layout::Horizontal => layout_cmd::split_horizontal(),
            Layout::Vertical => layout_cmd::split_vertical()
        }
        Ok(true)
    }

    fn ToggleCardinalTiling(container_id: String) -> success: DBusResult<bool> {
        let mut tree = try!(lock_tree_dbus());
        let uuid = try!(try!(parse_uuid("container_id", &container_id))
                        .or_else(|| tree.active_id())
                        .ok_or(MethodErr::failed(&"No active container")));
        tree.toggle_cardinal_tiling(uuid)
            .and(Ok(true))
            .map_err(|err| MethodErr::failed(&format!("{:?}", err)))
    }

    fn SwitchWorkspace(w_name: String) -> success: DBusResult<bool> {
        let mut tree = try!(lock_tree_dbus());
        tree.switch_to_workspace(w_name.as_str())
            .and(Ok(true))
            .map_err(|err| MethodErr::failed(&format!("{:?}", err)))
    }

    fn CloseView(view_id: String) -> success: DBusResult<bool> {
        let mut tree = try!(lock_tree_dbus());
        let uuid = try!(try!(parse_uuid("view_id", &view_id))
            .or_else(|| tree.active_id())
            .ok_or(MethodErr::failed(&"No active container")));
        tree.remove_view_by_id(uuid)
            .and(Ok(true))
            .map_err(|err| MethodErr::failed(&format!("{:?}", err)))
    }

    fn Focus(container_id: String) -> success: DBusResult<bool> {
        let mut tree = try!(lock_tree_dbus());
        let uuid = try!(try!(parse_uuid("container_id", &container_id))
                        .or_else(|| tree.active_id())
                        .ok_or(MethodErr::failed(&"No active container")));

        tree.focus(uuid)
            .and(Ok(true))
            .map_err(|err| MethodErr::failed(&format!("{:?}", err)))
    }

    fn FocusDir(direction: String) -> success: DBusResult<bool> {
        let direction = try!(parse_direction("direction", direction.as_str()));
        let mut tree = try!(lock_tree_dbus());
        tree.move_focus(direction)
            .and(Ok(true))
            .map_err(|err| MethodErr::failed(&format!("{:?}", err)))
    }

    fn SendToWorkspace(container_id: String, w_name: String) -> success: DBusResult<bool> {
        let mut tree = try!(lock_tree_dbus());
        let uuid = try!(try!(parse_uuid("container_id", &container_id))
                        .or_else(|| tree.active_id())
                        .ok_or(MethodErr::failed(&"No active container")));
        tree.send_to_workspace(uuid, w_name.as_str())
            .and(Ok(true))
            .map_err(|err| MethodErr::failed(&format!("{:?}", err)))
    }

    fn Debug() -> success: DBusResult<String> {
        Ok(format!("{}", layout_cmd::tree_as_json()))
    }

    fn ContainerInActiveWorkspace(container_id: String) -> success: DBusResult<bool> {
        let tree = try!(lock_tree_dbus());
        let uuid = try!(try!(parse_uuid("container_id", &container_id))
                        .or_else(|| tree.active_id())
                        .ok_or(MethodErr::failed(&"No active container")));
        tree.container_in_active_workspace(uuid)
            .map_err(|err| MethodErr::failed(&format!("{:?}", err)))
    }

    fn FullScreen(container_id: String, toggle: bool) -> success: DBusResult<bool> {
        let mut tree = try!(lock_tree_dbus());
        let uuid = try!(try!(parse_uuid("container_id", &container_id))
                        .or_else(|| tree.active_id())
                        .ok_or(MethodErr::failed(&"No active container")));
        tree.set_fullscreen(uuid, toggle)
            .and(Ok(true))
            .map_err(|err| MethodErr::failed(&format!("{:?}", err)))
    }

    fn SetPointerPos(x: i32, y: i32) -> success: DBusResult<bool> {
        let mut tree = try!(lock_tree_dbus());
        let point = Point { x: x, y: y};
        tree.set_pointer_pos(point)
            .and(Ok(true))
            .map_err(|err| MethodErr::failed(&format!("{:?}", err)))
    }

    fn GrabAtCorner(container_id: String, dir: String) -> success: DBusResult<bool> {
        let mut tree = try!(lock_tree_dbus());
        let uuid = try!(try!(parse_uuid("container_id", &container_id))
                        .or_else(|| tree.active_id())
                        .ok_or(MethodErr::failed(&"No active container")));
        let mut edge = ResizeEdge::empty();
        for word in dir.split(',') {
            edge |= try!(parse_edge(word))
        }
        tree.grab_at_corner(uuid, edge)
            .and(Ok(true))
            .map_err(|err| MethodErr::failed(&format!("{:?}", err)))
    }

    fn ActiveWorkspace() -> name: DBusResult<String> {
        let tree = try!(lock_tree_dbus());
        tree.active_workspace()
            .map(|container| container.name())
            .map_err(|err| MethodErr::failed(&format!("{:?}", err)))
    }
}
