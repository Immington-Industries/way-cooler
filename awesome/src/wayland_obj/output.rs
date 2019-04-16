//! Wrapper around a wl_output

use std::fmt;

use wayland_client::{protocol::wl_output::WlOutput, NewProxy, Proxy};
use wlroots::area::{Area, Origin, Size};

use crate::lua::LUA;
use crate::objects::screen::{self, Screen};

/// The minimum version of the wl_output global to bind to.
pub const WL_OUTPUT_VERSION: u32 = 2;

/// Wrapper around WlOutput.
#[derive(Clone, Eq, PartialEq)]
pub struct Output {
    proxy: Proxy<WlOutput>
}

/// The cached state for the WlOutput.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
struct OutputState {
    name: String,
    resolution: (i32, i32)
}

impl Output {
    pub fn new(new_proxy: Result<NewProxy<WlOutput>, u32>, _: ()) {
        let new_proxy = new_proxy.expect("Could not create WlOutput");
        let state = Box::new(OutputState::default());
        let proxy = new_proxy.implement(move |event, mut proxy: Proxy<WlOutput>| {
            use wayland_client::protocol::wl_output::Event;
            let output = Output { proxy: proxy.clone() };
            let state = unwrap_state_mut(&mut proxy);
            LUA.with(|lua| {
                let lua = lua.borrow();
                let lua = &*lua;
                match event {
                    Event::Geometry { make, model, .. } => {
                        state.name = format!("{} ({})", make, model);
                    },
                    Event::Mode { width, height, .. } => {
                        state.resolution = (width, height);
                        let geometry = Area {
                            origin: Origin { x: 0, y: 0 },
                            size: Size { width, height }
                        };
                        if let Ok(mut screen) = screen::get_screen(lua, output) {
                            screen
                                .set_geometry(lua, geometry)
                                .expect("could not set geometry");
                            screen
                                .set_workarea(lua, geometry)
                                .expect("could not set workarea ");
                        }
                    },
                    Event::Done => {
                        // TODO We may not always want to add a new screen
                        // see how awesome does it and fix this.
                        let mut screen = Screen::new(lua).expect("Could not allocate new screen");
                        screen
                            .init_screens(output.clone(), vec![output])
                            .expect("Could not initilize new output with a screen");
                        screen::add_screen(lua, screen).expect("Could not add screen to the list of screens");
                    },
                    _ => { /* TODO */ }
                }
            });
        });
        proxy.set_user_data(Box::into_raw(state) as _);
        let output = Output { proxy };
        LUA.with(|lua| {
            let lua = lua.borrow();
            let lua = &*lua;
            let mut screen = Screen::new(lua).expect("Could not allocate new screen");
            screen
                .init_screens(output.clone(), vec![output])
                .expect("Could not initilize new output with a screen");
            screen::add_screen(lua, screen).expect("Could not add screen to the list of screens");
        });
    }

    pub fn resolution(&self) -> (i32, i32) {
        unwrap_state(self).resolution
    }

    pub fn name(&self) -> &str {
        unwrap_state(self).name.as_str()
    }
}

impl fmt::Debug for Output {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.proxy.c_ptr())
    }
}

impl<'this> Into<&'this Proxy<WlOutput>> for &'this Output {
    fn into(self) -> &'this Proxy<WlOutput> {
        &self.proxy
    }
}

fn unwrap_state_mut<'this, I: Into<&'this mut Proxy<WlOutput>>>(proxy: I) -> &'this mut OutputState {
    unsafe {
        let user_data = proxy.into().get_user_data() as *mut OutputState;
        if user_data.is_null() {
            panic!("User data has not been set yet");
        }
        &mut *user_data
    }
}

fn unwrap_state<'this, I: Into<&'this Proxy<WlOutput>>>(proxy: I) -> &'this OutputState {
    unsafe {
        let user_data = proxy.into().get_user_data() as *const OutputState;
        if user_data.is_null() {
            panic!("User data has not been set yet");
        }
        &*user_data
    }
}
