//! Wrapper around a wl_output

use std::cell::RefCell;

use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::{NewProxy, Proxy};

thread_local! {
    pub static OUTPUTS: RefCell<Vec<Output>> = RefCell::new(Vec::new());
}

/// Wrapper around WlOutput.
#[derive(Clone, Eq, PartialEq)]
pub struct Output {
    proxy: Proxy<WlOutput>,
}

/// The cached state for the WlOutput.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
struct OutputState {
    name: String,
    resolution: (i32, i32),
}

impl<'this> Into<&'this Proxy<WlOutput>> for &'this Output {
    fn into(self) -> &'this Proxy<WlOutput> {
        &self.proxy
    }
}

impl Output {
    pub fn new(new_proxy: Result<NewProxy<WlOutput>, u32>, _: ()) {
        let new_proxy = new_proxy.expect("Could not create WlOutput");
        let state = Box::new(OutputState::default());
        let proxy = new_proxy.implement(move |event, mut proxy| {
            use wayland_client::protocol::wl_output::Event;
            let state = unwrap_state_mut(&mut proxy);
            match event {
                Event::Geometry { make, model, .. } => {
                    state.name = format!("{} ({})", make, model);
                }
                Event::Mode { width, height, .. } => {
                    state.resolution = (width, height);
                }
                _ => { /* TODO */ }
            }
        });
        proxy.set_user_data(Box::into_raw(state) as _);
        let output = Output { proxy };
        OUTPUTS.with(|outputs| outputs.borrow_mut().push(output));
    }

    pub fn resolution(&self) -> (i32, i32) {
        unwrap_state(self).resolution
    }

    pub fn name(&self) -> &str {
        unwrap_state(self).name.as_str()
    }
}

fn unwrap_state_mut<'this, I: Into<&'this mut Proxy<WlOutput>>>(
    proxy: I,
) -> &'this mut OutputState {
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
