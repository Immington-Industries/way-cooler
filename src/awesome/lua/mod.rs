//! Lua functionality

mod init_path;
pub mod rust_interop;
mod types;
mod utils;

pub use self::types::{LuaQuery, LuaResponse};
pub use self::utils::*;

use glib::MainLoop;
use rlua;
use wlroots::CompositorHandle;

use std::cell::RefCell;
use std::io::Read;

use awesome::signal;

thread_local! {
    // NOTE The debug library does some powerful reflection that can do crazy things,
    // which is why it's unsafe to load.

    /// Global Lua state.
    pub static LUA: RefCell<rlua::Lua> = RefCell::new(unsafe { rlua::Lua::new_with_debug() });

    /// Main GLib loop
    static MAIN_LOOP: RefCell<MainLoop> = RefCell::new(MainLoop::new(None, false));
}

/// Sets up the Lua environment before running the compositor.
pub fn setup_lua(mut compositor: CompositorHandle) {
    LUA.with(|lua| {
                 rust_interop::register_libraries(&*lua.borrow(), &mut compositor).expect("Could not \
                                                                                  register lua \
                                                                                  libraries");
                 info!("Initializing lua...");
                 load_config(&mut *lua.borrow_mut(), &mut compositor);
             });
}

fn emit_refresh(lua: &rlua::Lua) {
    if let Err(err) = signal::global_emit_signal(lua, ("refresh".to_owned(), rlua::Value::Nil)) {
        error!("Internal error while emitting 'refresh' signal: {}", err);
    }
}

/// Main loop of the Lua thread:
///
/// * Initialise the Lua state
/// * Run a GMainLoop
pub fn enter_glib_loop() {
    MAIN_LOOP.with(|main_loop| main_loop.borrow().run());
}

pub fn terminate() {
    MAIN_LOOP.with(|main_loop| main_loop.borrow().quit())
}

pub fn load_config(mut lua: &mut rlua::Lua, compositor: &mut CompositorHandle) {
    info!("Loading way-cooler libraries...");

    let maybe_init_file = init_path::get_config();
    match maybe_init_file {
        Some((init_dir, mut init_file)) => {
            if init_dir.components().next().is_some() {
                // Add the config directory to the package path.
                let globals = lua.globals();
                let package: rlua::Table =
                    globals.get("package").expect("package not defined in Lua");
                let paths: String = package.get("path")
                                           .expect("package.path not defined in Lua");
                package.set("path",
                            paths + ";"
                            + init_dir.join("?.lua")
                                      .to_str()
                                      .expect("init_dir not a valid UTF-8 string"))
                       .expect("Failed to set package.path");
            }
            let mut init_contents = String::new();
            init_file.read_to_string(&mut init_contents)
                     .expect("Could not read contents");
            lua.exec(init_contents.as_str(), Some("init.lua".into()))
                .map(|_:()| info!("Read init.lua successfully"))
                .or_else(|err| {
                    fn recursive_callback_print(error: ::std::sync::Arc<rlua::Error>) {
                        match *error {
                            rlua::Error::CallbackError {traceback: ref err, ref cause } => {
                                error!("{}", err);
                                recursive_callback_print(cause.clone())
                            },
                            ref err => error!("{:?}", err)
                        }
                    }
                    match err {
                        rlua::Error::RuntimeError(ref err) => {
                            error!("{}", err);
                        }
                        rlua::Error::CallbackError{traceback: ref err, ref cause } => {
                            error!("traceback: {}", err);
                            recursive_callback_print(cause.clone());
                        },
                        err => {
                            error!("init file error: {:?}", err);
                        }
                    }
                    // Keeping this an error, so that it is visible
                    // in release builds.
                    info!("Defaulting to pre-compiled init.lua");
                    unsafe { *lua = rlua::Lua::new_with_debug(); }
                    rust_interop::register_libraries(&mut lua, compositor)?;
                    lua.exec(init_path::DEFAULT_CONFIG,
                             Some("init.lua <DEFAULT>".into()))
                })
                .expect("Unable to load pre-compiled init file");
        }
        None => {
            warn!("Could not find an init file in any path!");
            warn!("Defaulting to pre-compiled init.lua");
            let _: () = lua.exec(init_path::DEFAULT_CONFIG, Some("init.lua <DEFAULT>".into()))
                           .or_else(|err| {
                               fn recursive_callback_print(error: ::std::sync::Arc<rlua::Error>) {
                                   match *error {
                                       rlua::Error::CallbackError { traceback: ref err,
                                                                    ref cause } => {
                                           error!("{}", err);
                                           recursive_callback_print(cause.clone())
                                       }
                                       ref err => error!("{:?}", err)
                                   }
                               }
                               match err.clone() {
                                   rlua::Error::RuntimeError(ref err) => {
                                       error!("{}", err);
                                   }
                                   rlua::Error::CallbackError { traceback: ref err,
                                                                ref cause } => {
                                       error!("traceback: {}", err);
                                       recursive_callback_print(cause.clone());
                                   }
                                   err => {
                                       error!("init file error: {:?}", err);
                                   }
                               }
                               Err(err)
                           })
                           .expect("Unable to load pre-compiled init file");
        }
    }
    emit_refresh(lua);
}
