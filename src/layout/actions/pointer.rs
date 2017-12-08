use rustwlc::{input, Point, ResizeEdge, Geometry,
              RESIZE_TOPLEFT, RESIZE_TOPRIGHT, RESIZE_BOTTOMLEFT, RESIZE_BOTTOMRIGHT,};

use super::super::{LayoutTree, TreeError};
use super::super::commands::{CommandResult};
use uuid::Uuid;

impl LayoutTree {
    /// Sets the absolute position of the cursor on the screen.
    pub fn set_pointer_pos(&mut self, point: Point) -> CommandResult {
        input::pointer::set_position_v2(point.x as f64, point.y as f64);
        Ok(())
    }

    /// Places the cursor at the corner of the window behind the UUID.
    pub fn grab_at_corner(&mut self, id: Uuid, edge: ResizeEdge)
                          -> Result<Point, TreeError> {
        let container = try!(self.lookup(id));
        let Geometry { mut origin, size } = container.get_actual_geometry()
            .expect("Container had no geometry");
        drop(container);
        if edge.contains(RESIZE_TOPLEFT) {
            input::pointer::set_position_v2(origin.x as f64, origin.y as f64);
        } else if edge.contains(RESIZE_TOPRIGHT) {
            origin.x += size.w as i32;
            input::pointer::set_position_v2(origin.x as f64, origin.y as f64);
        } else if edge.contains(RESIZE_BOTTOMLEFT) {
            origin.y += size.h as i32;
            input::pointer::set_position_v2(origin.x as f64, origin.y as f64);
        } else if edge.contains(RESIZE_BOTTOMRIGHT) {
            origin.x += size.w as i32;
            origin.y += size.h as i32;
            input::pointer::set_position_v2(origin.x as f64, origin.y as f64);
        }
        Ok(origin)
    }
}
