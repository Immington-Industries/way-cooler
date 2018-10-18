mod xdg_v6;

pub use self::xdg_v6::*;

use wlroots::{Area, HandleResult, SurfaceHandle, XdgV6ShellSurfaceHandle};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Shell {
    XdgV6(XdgV6ShellSurfaceHandle), /* TODO WlShell
                                     * TODO Xdg */
}

impl Shell {
    /// Get a wlr surface from the shell.
    pub fn surface(&mut self) -> SurfaceHandle {
        match *self {
            Shell::XdgV6(ref mut shell) => shell
                .run(|shell| shell.surface())
                .expect("An xdg v6 client did not provide us a surface"),
        }
    }

    /// Get the geometry of a shell.
    pub fn geometry(&mut self) -> HandleResult<Area> {
        match *self {
            Shell::XdgV6(ref mut shell) => shell.run(|shell| shell.geometry()),
        }
    }
}

impl Into<Shell> for XdgV6ShellSurfaceHandle {
    fn into(self) -> Shell {
        Shell::XdgV6(self)
    }
}
