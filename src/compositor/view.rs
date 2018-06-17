use compositor::Shell;
use std::sync::Mutex;
use wlroots::XdgV6ShellState::*;
use wlroots::{Area, Origin, Size, SurfaceHandle};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PendingMoveResize {
    pub update_x: bool,
    pub update_y: bool,
    pub serial: u32,
    pub area: Area
}

#[derive(Debug, Default)]
pub struct View {
    pub shell: Shell,
    pub origin: Mutex<Origin>,
    pub pending_move_resize: Mutex<Option<PendingMoveResize>>
}

unsafe impl Sync for View {}

unsafe impl Send for View {}

impl PartialEq for View {
    fn eq(&self, other: &View) -> bool {
        self.shell == other.shell
    }
}

impl Eq for View {}

impl View {
    pub fn new(shell: Shell) -> View {
        View { shell: shell,
               origin: Mutex::new(Origin::default()),
               pending_move_resize: Mutex::new(None),
        }
    }

    pub fn surface(&self) -> SurfaceHandle {
        match self.shell {
            Shell::XdgV6(ref xdg_surface) => {
                with_handles!([(xdg_surface: {xdg_surface})] => {
                    xdg_surface.surface()
                }).unwrap()
            }
        }
    }

    pub fn activate(&self, activate: bool) {
        match self.shell {
            Shell::XdgV6(ref xdg_surface) => {
                with_handles!([(xdg_surface: {xdg_surface})] => {
                    match xdg_surface.state() {
                        Some(&mut TopLevel(ref mut toplevel)) => {
                            toplevel.set_activated(activate);
                        },
                        _ => unimplemented!()
                    }
                }).unwrap();
            }
        }
    }

    pub fn get_size(&self) -> Size {
        match self.shell {
            Shell::XdgV6(ref xdg_surface) => {
                with_handles!([(xdg_surface: {xdg_surface})] => {
                    let Area { origin: _, size } = xdg_surface.geometry();
                    size
                }).unwrap()
            }
        }
    }

    pub fn move_resize(&self, area: Area) {
        let Area { origin: Origin { x, y },
                   size: Size { width, height } } = area;
        let width = width as u32;
        let height = height as u32;

        let Origin { x: view_x,
                     y: view_y } = *self.origin.lock().unwrap();

        let update_x = x != view_x;
        let update_y = y != view_y;
        let mut serial = 0;

        match self.shell {
            Shell::XdgV6(ref xdg_surface) => {
                with_handles!([(xdg_surface: {xdg_surface})] => {
                    match xdg_surface.state() {
                        Some(&mut TopLevel(ref mut toplevel)) => {
                            // TODO apply size constraints
                            serial = toplevel.set_size(width, height);
                        },
                        _ => unimplemented!()
                    }
                }).unwrap();
            }
        }

        if serial == 0 {
            // size didn't change
            *self.origin.lock().unwrap() = Origin { x, y };
        } else {
            *self.pending_move_resize.lock().unwrap() =
                Some(PendingMoveResize {
                    update_x,
                    update_y,
                    area,
                    serial
                });
        }
    }

    pub fn for_each_surface(&self, f: &mut FnMut(SurfaceHandle, i32, i32)) {
        match self.shell {
            Shell::XdgV6(ref xdg_surface) => {
                with_handles!([(xdg_surface: {xdg_surface})] => {
                    xdg_surface.for_each_surface(f);
                }).unwrap();
            }
        }
    }

    pub fn title(&self) -> String {
        match self.shell {
            Shell::XdgV6(ref xdg_surface) => {
                with_handles!([(xdg_surface: {xdg_surface})] => {
                    match xdg_surface.state() {
                        Some(&mut TopLevel(ref mut toplevel)) => {
                            toplevel.title()
                        },
                        _ => unimplemented!()
                    }
                }).unwrap()
            }
        }
    }

    pub fn geometry(&self) -> Area {
        match self.shell {
            Shell::XdgV6(ref xdg_surface) => {
                with_handles!([(xdg_surface: {xdg_surface})] => {
                    xdg_surface.geometry()
                }).unwrap()
            }
        }
    }
}
