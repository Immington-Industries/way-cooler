use rustwlc::{input, Point, ResizeEdge, Geometry,
              RESIZE_LEFT, RESIZE_RIGHT, RESIZE_TOP, RESIZE_BOTTOM,
              RESIZE_TOPLEFT, RESIZE_TOPRIGHT, RESIZE_BOTTOMLEFT, RESIZE_BOTTOMRIGHT,
};

use super::super::{Action, Direction, LayoutTree, TreeError};
use super::super::commands::{CommandResult};
use super::super::core::container::{ContainerType, MIN_SIZE};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum ResizeErr {
    /// Expected the node associated with the UUID to be floating.
    ExpectedFloating(Uuid),
    /// Expected the node associated with the UUID to not be floating
    ExpectedNotFloating(Uuid)
}

impl LayoutTree {
    /// Updates pointer position for the node behind the id,
    /// in the same direction as the edge.
    pub fn update_pointer_pos(&self, id: Uuid, edge: ResizeEdge) -> CommandResult {
        let container = try!(self.lookup(id));
        let Geometry { mut origin, size } = container.get_geometry()
            .expect("Container had no geometry");
        drop(container);
        if edge.contains(RESIZE_TOPLEFT) {
            input::pointer::set_position(origin);
        } else if edge.contains(RESIZE_TOPRIGHT) {
            origin.x += size.w as i32;
            input::pointer::set_position(origin);
        } else if edge.contains(RESIZE_BOTTOMLEFT) {
            origin.y += size.h as i32;
            input::pointer::set_position(origin);
        } else if edge.contains(RESIZE_BOTTOMRIGHT) {
            origin.x += size.w as i32;
            origin.y += size.h as i32;
            input::pointer::set_position(origin);
        }
        Ok(())
    }

    /// Resizes a floating container. If the container was not floating, an Err is returned.
    pub fn resize_floating(&mut self, id: Uuid, edge: ResizeEdge, pointer: Point,
                           action: &mut Action) -> CommandResult {
        {
            let container = try!(self.lookup_mut(id));
            if !container.floating() {
                return Err(TreeError::Resize(ResizeErr::ExpectedFloating(container.get_id())))
            }
            match container.get_type() {
                ContainerType::View | ContainerType::Container => {},
                _ => return Err(TreeError::UuidWrongType(container.get_id(),
                                                        vec!(ContainerType::View)))
            }
            let geo = container.get_geometry()
                .expect("Could not get geometry of the container");

            let new_geo = calculate_resize(geo, edge, pointer, action.grab);
            action.grab = pointer;
            container.set_geometry(edge, new_geo);
        }
        self.update_pointer_pos(id, edge)
    }

    pub fn resize_tiled(&mut self, id: Uuid, edge: ResizeEdge, pointer: Point,
                        action: &mut Action) -> CommandResult {
        // This is the vector of operations we will perform, we do all geometry sets atomically.
        let mut resizing_ops: Vec<(Uuid, (ResizeEdge, Geometry))> = Vec::with_capacity(4);
        let dirs_moving_in = Direction::from_edge(edge);
        let next_containers: Vec<(Uuid, Uuid)> = dirs_moving_in.iter()
            .map(|dir| self.container_in_dir(id, *dir))
            .flat_map(|result| result.into_iter())
            .collect();
        for ancestor_id in next_containers.iter().map(|uuids| uuids.0) {
            let container = try!(self.lookup(ancestor_id));
            if container.floating() {
                return Err(TreeError::Resize(ResizeErr::ExpectedNotFloating(ancestor_id)))
            }
            match container.get_type() {
                ContainerType::View | ContainerType::Container => {},
                _ => return Err(TreeError::UuidWrongType(ancestor_id,
                                                        vec!(ContainerType::View)))
            }
            let geo = container.get_geometry()
                .expect("Could not get geometry of the container");
            if geometry_resize_too_small(geo, edge, pointer, action.grab) {
                return Ok(())
            }
            let new_geo = calculate_resize(geo, edge, pointer, action.grab);
            resizing_ops.push((ancestor_id, (edge, new_geo)));
        }
        let siblings: Vec<Uuid> = next_containers.into_iter()
            .map(|uuids| uuids.1).collect();
        // and now we mutate the siblings
        let reversed_dir: Vec<Direction> = dirs_moving_in.iter()
            .map(|dir| dir.reverse()).collect();
        let reversed_edge = Direction::to_edge(reversed_dir.as_slice());
        for sibling in siblings {
            let container = try!(self.lookup_mut(sibling));
            if container.floating() {
                return Err(TreeError::Resize(ResizeErr::ExpectedNotFloating(container.get_id())))
            }
            match container.get_type() {
                ContainerType::View | ContainerType::Container => {},
                _ => return Err(TreeError::UuidWrongType(container.get_id(),
                                                         vec!(ContainerType::View)))
            }
            let geo = container.get_geometry()
                .expect("Could not get geometry of the container");
            if geometry_resize_too_small(geo, reversed_edge, pointer, action.grab) {
                return Ok(())
            }
            let new_geo = calculate_resize(geo, reversed_edge, pointer, action.grab);
            if new_geo.size.w <= MIN_SIZE.w || new_geo.size.h <= MIN_SIZE.h {
                return Ok(())
            }
            resizing_ops.push((sibling, (reversed_edge, new_geo)));
        }
        action.grab = pointer;
        for (id, (edge, geo)) in resizing_ops {
            let container = self.lookup_mut(id)
                .expect("Id no longer points to node!");
            container.set_geometry(edge, geo);
        }
        action.grab = pointer;
        let node_ix = self.tree.lookup_id(id).unwrap();
        let workspace_ix = self.tree.ancestor_of_type(node_ix, ContainerType::Workspace).unwrap();
        self.layout(workspace_ix);
        self.update_pointer_pos(id, edge)
    }
}

/// Calculates what the new geometry is of a window.
/// Needs the geometry of the window, the edge direction the pointer is moving in,
/// the current position of the pointer, and the previous place the pointer was at.
fn calculate_resize(geo: Geometry, edge: ResizeEdge,
                    cur_pointer: Point, prev_pointer: Point) -> Geometry {
    let mut new_geo = geo.clone();
    let dx = cur_pointer.x - prev_pointer.x;
    let dy = cur_pointer.y - prev_pointer.y;
    if edge.contains(RESIZE_LEFT) {
        if dx < 0 {
            new_geo.size.w = geo.size.w.saturating_add(dx.abs() as u32);
        } else {
            new_geo.size.w = geo.size.w.saturating_sub(dx.abs() as u32);
        }
        new_geo.origin.x += dx;
    }
    else if edge.contains(RESIZE_RIGHT) {
        if dx < 0 {
            new_geo.size.w = geo.size.w.saturating_sub(dx.abs() as u32);
        } else {
            new_geo.size.w = geo.size.w.saturating_add(dx.abs() as u32);
        }
    }

    if edge.contains(RESIZE_TOP) {
        if dy < 0 {
            new_geo.size.h = geo.size.h.saturating_add(dy.abs() as u32);
        } else {
            new_geo.size.h = geo.size.h.saturating_sub(dy.abs() as u32);
        }
        new_geo.origin.y += dy;
    }
    else if edge.contains(RESIZE_BOTTOM) {
        if dy < 0 {
            new_geo.size.h = geo.size.h.saturating_sub(dy.abs() as u32);
        } else {
            new_geo.size.h = geo.size.h.saturating_add(dy.abs() as u32);
        }
    }

    if new_geo.size.w < MIN_SIZE.w {
        new_geo.origin.x = geo.origin.x;
        new_geo.size.w = MIN_SIZE.w;
    }

    if new_geo.size.h < MIN_SIZE.h {
        new_geo.origin.y = geo.origin.y;
        new_geo.size.h = MIN_SIZE.h;
    }
    new_geo
}

/// If the geometry is at the minimum size (in either the x or y plane)
/// and the pointer is trying to make it even smaller in that direction,
/// the it returns true (to indicate you should abandon all operations).
///
/// Otherwise returns false
fn geometry_resize_too_small(geo: Geometry, edge: ResizeEdge, cur_point: Point,
                       prev_point: Point) -> bool {
    if geo.size.w > MIN_SIZE.w && geo.size.h > MIN_SIZE.h {
        return false
    }
    if edge.contains(RESIZE_RIGHT) && cur_point.x - prev_point.x < 0 {
        true
    } else if edge.contains(RESIZE_LEFT) && cur_point.x - prev_point.x > 0 {
        true
    } else if edge.contains(RESIZE_TOP) && cur_point.y - prev_point.y > 0 {
        true
    } else if edge.contains(RESIZE_BOTTOM) && cur_point.y - prev_point.y < 0 {
        true
    } else {
        false
    }
}
