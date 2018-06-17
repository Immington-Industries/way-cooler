use compositor::{Action, Server, Shell, View};
use wlroots::{CompositorHandle, Origin, SurfaceHandle, SurfaceHandler, XdgV6ShellHandler,
              XdgV6ShellManagerHandler, XdgV6ShellState::*, XdgV6ShellSurfaceHandle};

use std::sync::Arc;
use awesome::{notify_client_add, notify_client_remove};
use awesome::lua::LUA;
use wlroots::xdg_shell_v6_events::{MoveEvent, ResizeEvent};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct XdgV6 {
    shell_surface: XdgV6ShellSurfaceHandle
}

impl XdgV6 {
    pub fn new() -> Self {
        XdgV6 { ..XdgV6::default() }
    }
}

impl XdgV6ShellHandler for XdgV6 {
    fn resize_request(&mut self,
                      compositor: CompositorHandle,
                      _: SurfaceHandle,
                      shell_surface: XdgV6ShellSurfaceHandle,
                      event: &ResizeEvent) {
        with_handles!([(compositor: {compositor})] => {
            let server: &mut Server = compositor.into();
            let Server { ref mut seat,
                         ref mut views,
                         ref mut cursor,
                         .. } = *server;
            let resizing_shell = shell_surface.into();

            if let Some(view) = views.iter().find(|view| view.shell == resizing_shell).cloned() {
                seat.begin_resize(cursor, view.clone(), views, event.edges())
            }
        }).unwrap();
    }

    fn move_request(&mut self,
                    compositor: CompositorHandle,
                    _: SurfaceHandle,
                    shell_surface: XdgV6ShellSurfaceHandle,
                    _: &MoveEvent) {
        with_handles!([(compositor: {compositor})] => {
            let server: &mut Server = compositor.into();
            let ref mut seat = server.seat;
            let ref mut cursor = server.cursor;

            if let Some(ref mut view) = seat.focused {
                let shell: Shell = shell_surface.into();
                let action = &mut seat.action;
                if view.shell == shell {
                    with_handles!([(cursor: {cursor})] => {
                        let (lx, ly) = cursor.coords();
                        let Origin { x: shell_x, y: shell_y } = *view.origin.lock().unwrap();
                        let (view_sx, view_sy) = (lx - shell_x as f64, ly - shell_y as f64);
                        let start = Origin::new(view_sx as _, view_sy as _);
                        *action = Some(Action::Moving { start: start });
                    }).unwrap();
                }
            }
        }).unwrap();
    }

    fn on_commit(&mut self,
                 compositor: CompositorHandle,
                 _: SurfaceHandle,
                 shell_surface: XdgV6ShellSurfaceHandle) {
        let configure_serial = {
            with_handles!([(shell_surface: {shell_surface.clone()})] => {
                shell_surface.configure_serial()
            }).unwrap()
        };

        let surface = shell_surface.into();
        with_handles!([(compositor: {compositor})] => {
            let server: &mut Server = compositor.into();
            let Server { ref mut seat,
                         ref mut views,
                         ref mut cursor,
                         .. } = *server;

            if let Some(view) = views.iter().find(|view| view.shell == surface).cloned() {
                let move_resize = *view.pending_move_resize.lock().unwrap();
                if let Some(move_resize) = move_resize {
                    if move_resize.serial >= configure_serial {
                        let mut origin = *view.origin.lock().unwrap();
                        let Origin { mut x, mut y } = origin;
                        if move_resize.update_x {
                            x  = move_resize.area.origin.x + move_resize.area.size.width -
                                 view.get_size().width;
                        }
                        if move_resize.update_y {
                            y  = move_resize.area.origin.y + move_resize.area.size.height -
                                 view.get_size().height;
                        }

                        *view.origin.lock().unwrap() = Origin { x, y };

                        if move_resize.serial == configure_serial {
                            *view.pending_move_resize.lock().unwrap() = None;
                        }
                    }
                }
            }
        }).unwrap();
    }

    fn map_request(&mut self,
                   compositor: CompositorHandle,
                   _: SurfaceHandle,
                   mut shell_surface: XdgV6ShellSurfaceHandle) {
        let is_toplevel = with_handles!([(shell_surface: {&mut shell_surface})] => {
            match shell_surface.state().unwrap() {
                TopLevel(_) => true,
                _ => false
            }
        }).unwrap();

        with_handles!([(compositor: {compositor})] => {
            let server: &mut Server = compositor.into();
            let Server { ref mut seat,
                         ref mut views,
                         ref mut cursor,
                         ref mut xcursor_manager,
                         .. } = *server;
            if is_toplevel {
                let view = Arc::new(View::new(Shell::XdgV6(shell_surface.into())));
                views.push(view.clone());
                seat.focus_view(view.clone(), views);
                LUA.with(|lua| {
                    let lua = lua.borrow();
                    notify_client_add(&lua, view.clone()).expect("could not add lua client");
                });
            }

            with_handles!([(cursor: {cursor})] => {
                seat.update_cursor_position(cursor, xcursor_manager, views, None);
            }).unwrap();
        }).unwrap();
    }

    fn unmap_request(&mut self,
                     compositor: CompositorHandle,
                     _: SurfaceHandle,
                     shell_surface: XdgV6ShellSurfaceHandle) {
        with_handles!([(compositor: {compositor})] => {
            let server: &mut Server = compositor.into();
            let Server { ref mut seat,
                         ref mut views,
                         ref mut cursor,
                         ref mut xcursor_manager,
                         .. } = *server;
            let destroyed_shell = shell_surface.into();
            if let Some(pos) = views.iter().position(|view| view.shell == destroyed_shell) {
                LUA.with(|lua| {
                    let lua = lua.borrow();
                    notify_client_remove(&lua, views[pos].clone()).expect("could not remove lua client");
                });
                views.remove(pos);
            }

            if views.len() > 0 {
                seat.focus_view(views[0].clone(), views);
            } else {
                seat.clear_focus();
            }
            with_handles!([(cursor: {cursor})] => {
                seat.update_cursor_position(cursor, xcursor_manager, views, None);
            }).unwrap();
        }).unwrap();
    }
}

pub struct XdgV6ShellManager;

impl XdgV6ShellManagerHandler for XdgV6ShellManager {
    fn new_surface(&mut self,
                   _: CompositorHandle,
                   _: XdgV6ShellSurfaceHandle)
                   -> (Option<Box<XdgV6ShellHandler>>, Option<Box<SurfaceHandler>>) {
        (Some(Box::new(XdgV6::new())), None)
    }
}
