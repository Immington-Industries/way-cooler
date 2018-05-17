use compositor::{Server, Shell, View};
use wlroots::{CompositorHandle, XdgV6ShellHandler, XdgV6ShellManagerHandler,
              XdgV6ShellSurfaceHandle};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct XdgV6 {
    shell_surface: XdgV6ShellSurfaceHandle
}

impl XdgV6 {
    pub fn new() -> Self {
        XdgV6 { ..XdgV6::default() }
    }
}

impl XdgV6ShellHandler for XdgV6 {}

pub struct XdgV6ShellManager;

impl XdgV6ShellManagerHandler for XdgV6ShellManager {
    fn new_surface(&mut self,
                   compositor: CompositorHandle,
                   surface: XdgV6ShellSurfaceHandle)
                   -> Option<Box<XdgV6ShellHandler>> {
        with_handles!([(compositor: {compositor}), (surface: {surface})] => {
            let server: &mut Server = compositor.data.downcast_mut().unwrap();
            server.views
                .push(View::new(Shell::XdgV6(surface.weak_reference().into())));
        }).unwrap();
        Some(Box::new(XdgV6::new()))
    }

    fn surface_destroyed(&mut self,
                         compositor: CompositorHandle,
                         surface: XdgV6ShellSurfaceHandle) {
        with_handles!([(compositor: {compositor}), (surface: {surface})] => {
            let server: &mut Server = compositor.into();
            let destroyed_shell = surface.weak_reference().into();
            if let Some(pos) = server.views
                .iter()
                    .position(|view| view.shell == destroyed_shell)
                    {
                        server.views.remove(pos);
                    }
        }).unwrap();
    }
}
