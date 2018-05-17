//! This handles the XWayland server and any XWayland clients that connect to
//! Way Cooler.

use awesome;
use std::panic;
use wlroots::{CompositorHandle, XWaylandManagerHandler};

pub struct XWaylandManager;

impl XWaylandManager {
    pub fn new() -> Self {
        XWaylandManager
    }
}

impl XWaylandManagerHandler for XWaylandManager {
    fn on_ready(&mut self, _: CompositorHandle) {
        // TODO Do this properly with Results!
        match panic::catch_unwind(|| ::awesome::lua::setup_lua()) {
            Ok(_) => {}
            Err(err) => {
                awesome::lua::terminate();
                panic::resume_unwind(err)
            }
        }
    }
}
