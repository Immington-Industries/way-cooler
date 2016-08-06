use std::mem::transmute;
use std::os::unix::io::AsRawFd;
use std::io::Write;

use wayland_client::wayland::get_display;
use wayland_client::wayland::compositor::{WlCompositor, WlSurface};
use wayland_client::wayland::shell::{WlShellSurface, WlShell};
use wayland_client::wayland::shm::{WlBuffer, WlShm, WlShmFormat};
use wayland_client::wayland::seat::{WlSeat, WlPointerEvent};
use wayland_client::cursor::load_theme;
use wayland_client::{EventIterator, Proxy};

use rustwlc::WlcOutput;
use tempfile;

use byteorder::{NativeEndian, WriteBytesExt};

wayland_env!(WaylandEnv,
             compositor: WlCompositor,
             shell: WlShell,
             shm: WlShm,
             seat: WlSeat
);

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
/// Holds the bytes to represent a colored background.
/// To be written into a wayland surface.
pub struct Color(pub [u8; 4]);

impl Color {
    /// Generate a new color out of a u32.
    /// E.G: 0xFFFFFF
    pub fn from_u32(color: u32) -> Self {
        unsafe { Color(transmute(color)) }
    }

    pub fn as_u32(&self) -> u32 {
        unsafe { transmute(self.0)}
    }
}

pub fn generate_solid_background(color: Color, output: WlcOutput) {
    // Get shortcuts to the globals.
    let (display, iter) = get_display()
        .expect("Unable to connect to a wayland compositor");
    let (env, evt_iter) = WaylandEnv::init(display, iter);
    let compositor = env.compositor.as_ref().map(|o| &o.0).unwrap();
    let shell = env.shell.as_ref().map(|o| &o.0).unwrap();
    let shm = env.shm.as_ref().map(|o| &o.0).unwrap();
    let seat = env.seat.as_ref().map(|o| &o.0).unwrap();

    // Create the surface we are going to write into
    let surface = compositor.create_surface();
    let shell_surface = shell.get_shell_surface(&surface);
    let mut tmp = tempfile::tempfile().ok().expect("Unable to create a tempfile.");

    // Calculate how big the buffer needs to be from the output resolution
    let resolution = output.get_resolution().clone();
    let (width, height) = (resolution.w as i32, resolution.h as i32);
    let size = (resolution.w * resolution.h) as i32;

    // Write in the color coding to the surface
    for _ in 0..size {
        unsafe {
            tmp.write_u32::<NativeEndian>(transmute(color.0))
                .expect("Could not write to file")
        }
    }
    tmp.flush()
        .expect("Could not flush buffer");

    // Create the buffer that is mem-mapped to the temp file descriptor
    let pool = shm.create_pool(tmp.as_raw_fd(), size);
    let buffer = pool.create_buffer(0, width, height, width, WlShmFormat::Argb8888);
    // Tell Way Cooler not to set put this in the tree, treat as background
    shell_surface.set_class("Background".into());
    shell_surface.set_title(format!("0x{:x}", color.as_u32()));

    // Attach the buffer to the surface
    surface.attach(Some(&buffer), 0, 0);

    main_background_loop(compositor, shell, shm, seat, surface,
                         shell_surface, buffer, evt_iter);
}


/// Main loop for rendering backgrounds.
/// Need to keep the surface alive, and update it if the
/// user wants to change the background.
#[allow(unused_variables)]
fn main_background_loop(compositor: &WlCompositor, shell: &WlShell, shm: &WlShm,
                        seat: &WlSeat,
                        surface: WlSurface, shell_surface: WlShellSurface,
                        buffer: WlBuffer, mut event_iter: EventIterator) {
    use wayland_client::wayland::WaylandProtocolEvent;
    use wayland_client::Event;
    println!("Entering main loop");
    // For now just loop and do nothing
    // Eventually need to query the background state and update
    let cursor_surface = compositor.create_surface();
    let mut pointer = seat.get_pointer();
    let cursor_theme = load_theme(None, 16, shm);
    let maybe_cursor = cursor_theme.get_cursor("default");
    if maybe_cursor.is_none() {
        error!("Could not load cursor theme properly, cannot load background");
        error!("Please consult the developers about this issue with your distro version, this is a known issue");
        return;
    }
    let cursor = maybe_cursor.unwrap();
    let cursor_buffer = cursor.frame_buffer(0).expect("Couldn't get frame_buffer");
    cursor_surface.attach(Some(&*cursor_buffer), 0, 0);
    pointer.set_event_iterator(&event_iter);
    pointer.set_cursor(0, Some(&cursor_surface), 0, 0);
    surface.commit();
    event_iter.sync_roundtrip().unwrap();
    loop {
        for event in &mut event_iter {
            match event {
                Event::Wayland(wayland_event) => {
                    match wayland_event {
                        WaylandProtocolEvent::WlPointer(id, pointer_event) => {
                            match pointer_event {
                                WlPointerEvent::Enter(serial, surface, surface_x, surface_y) => {
                                    pointer.set_cursor(0, Some(&cursor_surface), 0, 0);
                                },
                                _ => {
                                }
                            }
                        },
                        _ => {/* unhandled events */}
                    }
                }
                _ => { /* unhandled events */ }
            }
        }
        event_iter.dispatch().expect("Connection with the compositor was lost.");
    }
}
