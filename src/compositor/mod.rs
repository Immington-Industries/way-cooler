mod output;
mod input;
mod seat;
mod cursor;
mod shells;
mod view;
mod xwayland;

pub use self::cursor::*;
pub use self::input::*;
pub use self::output::*;
pub use self::seat::*;
pub use self::shells::*;
pub use self::view::*;
pub use self::xwayland::*;

use wlroots::{self, Compositor, CompositorBuilder, Cursor, CursorHandle, KeyboardHandle,
              OutputHandle, OutputLayout, OutputLayoutHandle, PointerHandle, XCursorManager};

#[derive(Debug)]
pub struct Server {
    pub xcursor_manager: XCursorManager,
    pub layout: OutputLayoutHandle,
    pub seat: Seat,
    pub cursor: CursorHandle,
    pub keyboards: Vec<KeyboardHandle>,
    pub pointers: Vec<PointerHandle>,
    pub outputs: Vec<OutputHandle>,
    pub views: Vec<View>
}

impl Default for Server {
    fn default() -> Server {
        let xcursor_manager =
            XCursorManager::create("default".to_string(), 24).expect("Could not create xcursor \
                                                                      manager");
        xcursor_manager.load(1.0);
        Server { xcursor_manager,
                 layout: OutputLayoutHandle::default(),
                 seat: Seat::default(),
                 cursor: CursorHandle::default(),
                 keyboards: Vec::default(),
                 pointers: Vec::default(),
                 outputs: Vec::default(),
                 views: Vec::default() }
    }
}

impl Server {
    pub fn new(layout: OutputLayoutHandle, mut cursor: CursorHandle) -> Self {
        let mut xcursor_manager =
            XCursorManager::create("default".to_string(), 24).expect("Could not create xcursor \
                                                                      manager");
        xcursor_manager.load(1.0);
        cursor.run(|c| xcursor_manager.set_cursor_image("left_ptr".to_string(), c))
              .unwrap();

        Server { xcursor_manager,
                 layout,
                 cursor,
                 ..Server::default() }
    }
}

compositor_data!(Server);

pub fn init() -> Compositor {
    let layout = OutputLayout::create(Box::new(OutputLayoutManager::new()));
    let cursor = Cursor::create(Box::new(CursorManager::new()));
    let mut compositor = CompositorBuilder::new().gles2(true)
                                                 .data_device(true)
                                                 .output_manager(Box::new(OutputManager::new()))
                                                 .input_manager(Box::new(InputManager::new()))
                                                 .xwayland(Box::new(XWaylandManager::new()))
                                                 .xdg_shell_v6_manager(Box::new(XdgV6ShellManager))
                                                 .build_auto(Server::new(layout, cursor));
    // NOTE We need to create this afterwards because it needs the compositor
    // running to announce the seat.
    let seat = wlroots::Seat::create(&mut compositor,
                                     "seat0".into(),
                                     Box::new(SeatManager::new()));
    {
        let server: &mut Server = (&mut compositor).into();
        server.seat = Seat::new(seat);
    }
    compositor
}
