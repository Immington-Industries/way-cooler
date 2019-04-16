//! Wrapper around a wl_shm.

use std::{cell::RefCell, os::unix::io::RawFd};

use wayland_client::{
    protocol::{
        wl_buffer::WlBuffer,
        wl_shm::{self, RequestsTrait as WlShmTrait, WlShm},
        wl_shm_pool::RequestsTrait as WlShmPoolTrait
    },
    NewProxy, Proxy
};

use wlroots::area::Size;

/// The minimum version of the wl_shm global to bind to.
pub const WL_SHM_VERSION: u32 = 1;

thread_local! {
    static WL_SHM: RefCell<Option<Proxy<WlShm>>> = RefCell::new(None);
}

pub fn wl_shm_init(new_proxy: Result<NewProxy<WlShm>, u32>, _: ()) {
    let new_proxy = new_proxy.expect("Could not create wl_shm");
    let proxy = new_proxy.implement(|_event, _proxy| {});
    WL_SHM.with(|wl_shm| {
        *wl_shm.borrow_mut() = Some(proxy);
    });
}

/// Create a buffer from the raw file descriptor in the given size.
///
/// This should be called from a shell and generally should not be used
/// directly by the Awesome objects.
pub fn create_buffer(fd: RawFd, size: Size) -> Result<Proxy<WlBuffer>, ()> {
    let Size { width, height } = size;
    WL_SHM.with(|wl_shm| {
        let wl_shm = wl_shm.borrow();
        let wl_shm = wl_shm.as_ref().expect("WL_SHM was not initilized");
        let pool = wl_shm.create_pool(fd, width * height * 4)?.implement(|_, _| {});
        // TODO ARb32 instead
        Ok(pool
            .create_buffer(0, width, height, width * 4, wl_shm::Format::Argb8888)?
            .implement(|_, _| {}))
    })
}
