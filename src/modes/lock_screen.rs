//! Implementations of the default callbacks exposed by wlc.
//! These functions are the main entry points into Way Cooler from user action.
//! This is the default mode that Way Cooler is in at initilization
use rustwlc::*;
use rustwlc::wayland::wlc_view_get_wl_client;
use rustwlc::input::pointer;
use wayland_sys::server::wl_client;

use super::{Mode, Modes, Default, write_current_mode};
use ::layout::try_lock_tree;

use rustwlc::{ResizeEdge, WlcView, Geometry, Point};
use rustwlc::types::VIEW_ACTIVATED;

/// A lock screen program, that has been spawned by Way Cooler.
///
/// The PID is what is returned from spawning the subprocess.
///
/// The view is optional, because after spawning it we must find it in the
/// `view_created` callback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockScreen {
    /// Lists of clients that want to lock the screen.
    ///
    /// Multiple clients can attempt to lock a screen.
    /// The normal mode of operation is that they are the same client,
    /// but with different connections.
    ///
    ///The usize is the client id, and the view is the view it's associated with
    /// (if it has been spawned yet)
    ///
    /// The WlcOutput is the output the view should be associated with.
    pub clients: Vec<(usize, WlcOutput, Option<WlcView>)>,
    /// Flag that is set when a view is destroyed, either because the user
    /// closed it OR if the program failed to spawn.
    removed: bool
}

impl LockScreen {
    /// Creates a new Lock Screen from the PID
    ///
    /// The view is set to `None`, because we haven't found it yet in the
    /// `view_created` callback. This should be updated once it is found,
    /// so that we can start locking the screen.
    pub fn new(client: *mut wl_client, output: WlcOutput) -> Self {
        LockScreen {
            clients: vec![(client as _, output, None)],
            removed: false
        }
    }

    /// Adds the view to the `LockScreen` if it's PID matches the stored one.
    ///
    /// If there is already here, false is always returned.
    ///
    /// If the view's PID matches this PID, then it's automatically focused,
    /// and the geometry of it is set to the size of it's `WlcOutput`'s size.
    fn add_view_if_match(&mut self, view: WlcView) -> bool {
        let cur_client = unsafe {
            wlc_view_get_wl_client(view.0 as _) as _
        };
        for client in &mut self.clients {
            if client.0 == cur_client && client.2 == None {
                error!("Found a match, setting to output {:#?}", client.1);
                view.set_output(client.1);
                client.2 = Some(view);
                let output = view.get_output();
                let resolution = output.get_resolution()
                    .expect("Output was invalid: had no resolution");
                let geo = Geometry {
                    origin: Point { x: 0, y: 0 },
                    size: resolution
                };
                view.set_geometry(ResizeEdge::empty(), geo);
                view.set_state(VIEW_ACTIVATED, true);
                view.focus();
                view.set_mask(1);
                view.bring_to_front();
                return true
            }
        }
        false
    }

    /// Removes the lockscreen if provided view matches the stored view.
    ///
    /// Does not compare the PID, because the passed in `WlcView` should be
    /// considered invalidated because this function is probably being called
    /// from `view_destroyed`.
    ///
    /// To protect against failing programs, the child handler should be
    /// checked to see if the program is still running.
    /// HOWEVER, this is blocked because this is still a nightly feature.
    /// See https://github.com/rust-lang/rust/issues/38903
    fn remove_if_match(&mut self, cur_view: WlcView) -> Option<WlcView> {
        for &mut (_, _, ref mut view) in &mut self.clients {
            if *view == Some(cur_view) {
                self.removed = true;
                // invalidate this view so we can't do anything to it.
                return view.take()
            }
        }
        None
    }

    fn eval_if_lockscreen<F, T>(&mut self, cur_view: WlcView, eval: F) -> T
        where F: Fn(WlcView) -> T,
              T: ::std::default::Default
    {
        for &(_, _, ref view) in &self.clients {
            if *view == Some(cur_view) {
                return eval(view.unwrap())
            }
        }
        T::default()
    }
}

#[allow(unused)]
impl Mode for LockScreen {
    fn output_render_post(&mut self, output: WlcOutput) {
        Default.output_render_post(output);
        // TODO Uncomment
        // we should allow this some other way
        /*// Don't allow screen scraping during lock screen
        let need_to_fetch = read_screen_scrape_lock();
        if *need_to_fetch {
            if let Ok(mut scraped_pixels) = scraped_pixels_lock() {
                let resolution = output.get_resolution()
                    .expect("Output had no resolution");
                // Give them zero-ed out pixels
                *scraped_pixels = vec![0; resolution.w as usize * resolution.h as usize * 4];
                sync_scrape();
            }
        }*/
    }

    fn view_created(&mut self, view: WlcView) -> bool {
        // this will focus and set the size if necessary.
        if self.add_view_if_match(view) {
            trace!("Adding lockscreen");
            true
        } else {
            false
        }
    }

    fn view_destroyed(&mut self, view: WlcView) {
        // Always remove the destroyed view from the tree.
        Default.view_destroyed(view);
        // View is automatically removed from the internal lock screen struct.
        if self.remove_if_match(view).is_some() {
            trace!("Removing lock screen");
            // Update the active container in the tree to the active path end.
            if let Ok(mut tree) = try_lock_tree() {
                tree.reset_focus()
                    .unwrap_or_else(|_| {
                        warn!("Could not reset focus\
                              after removing lock screen");
                    })
            }
        }
    }

    fn view_focused(&mut self, current: WlcView, focused: bool) {
        // Do nothing
    }

    fn view_request_state(&mut self, view: WlcView, state: ViewState, toggle: bool) {
        // Do nothing
    }

    fn view_request_move(&mut self, view: WlcView, _dest: Point) {
        // Do nothing
    }

    fn view_request_resize(&mut self, view: WlcView, edge: ResizeEdge, point: Point) {
        // Do nothing
    }

    fn on_keyboard_key(&mut self, view: WlcView, _time: u32, mods: KeyboardModifiers,
                            key: u32, state: KeyState) -> bool {
        !self.eval_if_lockscreen(view, |view| {
            view.focus();
            true
        })
    }

    fn on_pointer_button(&mut self, view: WlcView, _time: u32,
                            mods: KeyboardModifiers, button: u32,
                                state: ButtonState, point: Point) -> bool {
        self.eval_if_lockscreen(view, |view| {
            view.focus();
            true
        })
    }

    fn on_pointer_scroll(&mut self, view: WlcView, _time: u32,
                            _mods_ptr: KeyboardModifiers, _axis: ScrollAxis,
                            _heights: [f64; 2]) -> bool {
        self.eval_if_lockscreen(view, |view| {
            view.focus();
            true
        })
    }

    fn on_pointer_motion(&mut self, view: WlcView, _time: u32, x: f64, y: f64) -> bool {
        !self.eval_if_lockscreen(view, |view| {
            view.focus();
            pointer::set_position_v2(x, y);
            // false because default is false, but true is the base case here
            false
        })
    }
}


pub fn lock_screen(client: *mut wl_client, output: WlcOutput) {
    let mut mode = write_current_mode();
    {
        match *mode {
            Modes::LockScreen(ref mut lock_mode) => {
                lock_mode.clients.push((client as _, output, None));
                return
            },
            _ => {}
        }
    }
    *mode = Modes::LockScreen(LockScreen::new(client, output));
}

pub fn unlock_screen(cur_client: *mut wl_client) {
    let mode = {
        write_current_mode().clone()
    };
    match mode {
        Modes::LockScreen(ref lock_mode) => {
            let mut seen = false;
            for &(client, _, view) in &lock_mode.clients {
                if client == cur_client as _ {
                    seen = true;
                    break;
                }
                view.map(WlcView::close);
            }
            if !seen {
                return
            }
        },
        _ => {}
    }
    *write_current_mode() = Modes::Default(Default);
}
