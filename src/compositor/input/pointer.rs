use compositor::{self, Action, Server, Shell, View};
use std::time::Duration;
use compositor::seat::Seat;
use wlroots::types::Cursor;
use wlroots::{CompositorHandle, HandleResult, KeyboardHandle, Origin, PointerHandle,
              PointerHandler, pointer_events::*, WLR_BUTTON_RELEASED, SurfaceHandle};

#[derive(Debug, Default)]
pub struct Pointer;

impl PointerHandler for Pointer {

    fn on_motion_absolute(&mut self, compositor: CompositorHandle, _: PointerHandle, event: &AbsoluteMotionEvent) {
        with_handles!([(compositor: {compositor})] => {
            let server: &mut Server = compositor.data.downcast_mut().unwrap();
            let Server { ref mut cursor,
                         ref mut seat,
                         ref mut views,
                         .. } = *server;
            with_handles!([(cursor: {&mut *cursor})] => {
                let (x, y) = event.pos();
                cursor.warp_absolute(event.device(), x, y);
                update_view_position(cursor, seat, views, event.time_msec());
            }).expect("Cursor was destroyed");
        }).unwrap();
    }

    fn on_motion(&mut self, compositor: CompositorHandle, _: PointerHandle, event: &MotionEvent) {
        with_handles!([(compositor: {compositor})] => {
            let server: &mut Server = compositor.into();
            let Server { ref mut cursor,
                         ref mut seat,
                         ref mut views,
                         .. } = *server;
            with_handles!([(cursor: {&mut *cursor})] => {
                let (x, y) = event.delta();
                cursor.move_to(event.device(), x, y);
                update_view_position(cursor, seat, views, event.time_msec());
            }).expect("Cursor was destroyed");
        }).unwrap();
    }

    fn on_button(&mut self, compositor: CompositorHandle, _: PointerHandle, event: &ButtonEvent) {
        with_handles!([(compositor: {compositor})] => {
            let server: &mut Server = compositor.into();
            let Server { ref mut cursor,
                         ref mut views,
                         ref mut seat,
                         ref mut keyboards,
                         .. } = *server;
            with_handles!([(cursor: {&mut *cursor})] => {
                if event.state() == WLR_BUTTON_RELEASED {
                    seat.action = None;
                    send_pointer_button(seat, event).expect("Could not send pointer button");
                    return
                }
                let (view, _surface, _sx, _sy) = view_at_pointer(views, cursor);
                match view {
                    Some(view) => {
                        focus_under_pointer(seat, &mut **keyboards, { &mut *view }).expect("Could not focus \
                                                                                    view");
                        let meta_held_down = seat.meta;
                        if meta_held_down && event.button() == BTN_LEFT {
                            move_view(seat, cursor, view, None);
                        }
                        send_pointer_button(seat, event).expect("Could not send pointer button");
                    },
                    None => {
                        focus_under_pointer(seat, &mut **keyboards, None).expect("Could not focus view");
                    }
                }
            }).unwrap()
        }).unwrap()
    }
}

/// After the cursor has been warped, send pointer motion events to the view under the pointer or
/// update the position of a view that might have been affected by an ongoing interactive
/// move/resize operation
fn update_view_position(cursor: &mut Cursor, seat: &mut Seat, views: &mut [View], time_msec: u32) {
    match seat.action {
        Some(Action::Moving { start }) => {
            let (view, _surface, _sx, _sy) = view_at_pointer(views, cursor);
            match view {
                Some(mut view) => {
                    let meta_held_down = seat.meta;
                    if meta_held_down {
                        move_view(seat, cursor, &mut view, start);
                    }
                },
                None => ()
            }
        }
        _ => {
            let (_view, surface, sx, sy) = view_at_pointer(views, cursor);
            let ref mut seat = seat.seat;
            match surface {
                Some(surface) => {
                    with_handles!([(surface: {surface}), (seat: {seat})] => {
                        seat.pointer_notify_enter(surface, sx, sy);
                        seat.pointer_notify_motion(Duration::from_millis(time_msec as u64), sx, sy);
                    }).unwrap();
                },
                None => {
                    with_handles!([(seat: {seat})] => {
                        seat.pointer_clear_focus();
                    }).unwrap();
                }
            }
		}
    }
}

fn view_at_pointer<'view>(views: &'view mut [View],
                          cursor: &mut Cursor)
                          -> (Option<&'view mut View>, Option<SurfaceHandle>, f64, f64) {
    for view in views {
        match view.shell.clone() {
            Shell::XdgV6(ref mut shell) => {
                let (mut sx, mut sy) = (0.0, 0.0);
                let surface = with_handles!([(shell: {&mut *shell})] => {
                    let (lx, ly) = cursor.coords();
                    let Origin {x: shell_x, y: shell_y} = view.origin;
                    let (view_sx, view_sy) = (lx - shell_x as f64, ly - shell_y as f64);
                    shell.surface_at(view_sx, view_sy, &mut sx, &mut sy)
                }).unwrap();
                if surface.is_some() {
                    return (Some(view), surface, sx, sy);
                }
            }
        }
    }
    (None, None, 0.0, 0.0)
}

/// Focus the view under the pointer.
fn focus_under_pointer<'view, V>(seat: &mut compositor::Seat,
                                 keyboards: &mut [KeyboardHandle],
                                 view: V)
                                 -> HandleResult<()>
    where V: Into<Option<&'view mut View>>
{
    match view.into() {
        None => {
            if let Some(mut focused_view) = seat.focused.take() {
                focused_view.activate(false);
            }
            with_handles!([(seat: {&mut seat.seat})] => {
                seat.keyboard_clear_focus()
            })
        }
        Some(view) => {
            seat.focused = Some(view.clone());
            view.activate(true);
            for keyboard in { &mut *keyboards } {
                with_handles!([(seat: {&mut seat.seat}),
                (surface: {view.surface()}),
                (keyboard: {keyboard})] => {
                    seat.keyboard_notify_enter(surface,
                                               &mut keyboard.keycodes(),
                                               &mut keyboard.get_modifier_masks())
                })?;
            }
            Ok(())
        }
    }
}

/// Start moving a view if you haven't already by passing `start: None`.
///
/// Otherwise, update the view position relative to where the move started,
/// which is provided by Action::Moving.
fn move_view<O>(seat: &mut compositor::Seat,
                cursor: &mut Cursor,
                view: &mut View,
                start: O)
    where O: Into<Option<Origin>>
{
    let Origin { x: shell_x,
    y: shell_y } = view.origin;
    let (lx, ly) = cursor.coords();
    match start.into() {
        None => {
            let (view_sx, view_sy) = (lx - shell_x as f64, ly - shell_y as f64);
            let start = Origin::new(view_sx as _, view_sy as _);
            seat.action = Some(Action::Moving{start});
        }
        Some(start) => {
            let pos = Origin::new(lx as i32 - start.x, ly as i32 - start.y);
            view.origin = pos;
        }
    };
}

fn send_pointer_button(seat: &mut compositor::Seat, event: &ButtonEvent) -> HandleResult<()> {
    with_handles!([(seat: {&mut seat.seat})] => {
        seat.pointer_notify_button(Duration::from_millis(event.time_msec() as _),
                                   event.button(),
                                   event.state() as u32);
    })
}
