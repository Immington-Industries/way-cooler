//! Awesome compatibility modules

#![cfg_attr(
    test,
    deny(
        bad_style,
        const_err,
        dead_code,
        improper_ctypes,
        legacy_directory_ownership,
        non_shorthand_field_patterns,
        no_mangle_generic_items,
        overflowing_literals,
        path_statements,
        patterns_in_fns_without_body,
        plugin_as_library,
        private_in_public,
        safe_extern_statics,
        unconditional_recursion,
        unions_with_drop_fields,
        unused,
        unused_allocation,
        unused_comparisons,
        unused_parens,
        while_true
    )
)]
// Allowed by default
#![cfg_attr(
    test,
    deny(
        missing_docs,
        trivial_numeric_casts,
        unused_extern_crates,
        unused_import_braces
    )
)]

use env_logger;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

#[macro_use]
mod macros;
mod area;
mod awesome;
mod awful;
mod common;
mod dbus;
mod keygrabber;
mod lua;
mod mousegrabber;
mod objects;
mod root;
mod wayland_obj;

use std::{
    env,
    io::{self, Write},
    mem,
    os::unix::io::RawFd,
    path::PathBuf,
    process::exit
};

use clap::{App, Arg};
use exec::Command;
use log::Level;
use nix::sys::signal::{self, SaFlags, SigAction, SigHandler, SigSet};
use rlua::{LightUserData, Table};
use wayland_client::{
    global_filter,
    protocol::{wl_compositor, wl_output, wl_shm},
    sys::client::wl_display,
    ConnectError, Display, EventQueue, GlobalError, GlobalManager
};
use xcb::xkb;

// So the C code can link to these Rust functions.
pub use crate::dbus::{dbus_session_refresh, dbus_system_refresh};

use crate::lua::{LUA, NEXT_LUA};
pub use wayland_protocols::xdg_shell::client::xdg_wm_base;

const GIT_VERSION: &'static str = include_str!(concat!(env!("OUT_DIR"), "/git-version.txt"));
pub const GLOBAL_SIGNALS: &'static str = "__awesome_global_signals";
pub const XCB_CONNECTION_HANDLE: &'static str = "__xcb_connection";

#[link(name = "wayland_glib_interface", kind = "static")]
extern "C" {
    pub fn wayland_glib_interface_init(
        display: *mut wl_display,
        session_fd: RawFd,
        system_fd: RawFd,
        wayland_state: *mut libc::c_void
    );
    pub fn remove_dbus_from_glib();
}

/// The state passed into C to store it during the glib loop.
///
/// It's passed back to us when Awesome needs a refresh so we can
/// construct any Wayland objects.
#[repr(C)]
struct WaylandState {
    pub display: Display,
    pub event_queue: EventQueue
}

/// Called from `wayland_glib_interface.c` after every call back into the
/// wayland event loop.
///
/// This restarts the Lua thread if there is a new one pending
#[no_mangle]
pub extern "C" fn awesome_refresh(wayland_state: *mut libc::c_void) {
    // NOTE
    // This is safe because it's way back up the stack where we can't access it.
    //
    // The moment that stack is accessible this pointer will be lost.
    //
    // The only way it's unsafe is if we destructure `WaylandState`,
    // which we can't do because it's borrowed.
    let _wayland_state = unsafe { &mut *(wayland_state as *mut WaylandState) };
    NEXT_LUA.with(|new_lua_check| {
        if new_lua_check.get() {
            new_lua_check.set(false);
            let awesome = env::args().next().unwrap();
            let args: Vec<_> = env::args().skip(1).collect();
            let err = Command::new(awesome).args(args.as_slice()).exec();
            error!("error: {:?}", err);
            panic!("Could not restart Awesome");
        }
    });
}

struct AwesomeVersion;

impl<'a> Into<&'a str> for AwesomeVersion {
    fn into(self) -> &'a str {
        if !GIT_VERSION.is_empty() {
            concat!(
                "Awesome ",
                env!("CARGO_PKG_VERSION"),
                " @ ",
                include_str!(concat!(env!("OUT_DIR"), "/git-version.txt"))
            )
        } else {
            concat!("Awesome ", env!("CARGO_PKG_VERSION"))
        }
    }
}

fn main() {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(AwesomeVersion)
        .version_short("v")
        .author(crate_authors!("\n"))
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .help("configuration file to use")
                .takes_value(true)
        )
        .arg(
            Arg::with_name("lua lib search")
                .long("search")
                .value_name("DIR")
                .help("add a directory to the library search path")
                .takes_value(true)
                .multiple(true)
        )
        .arg(
            Arg::with_name("lua syntax check")
                .short("k")
                .long("check")
                .help("check configure file syntax")
        )
        .arg(
            Arg::with_name("client transparency")
                .short("a")
                .long("no-argb")
                .help("disable client transparency support")
        )
        .arg(
            Arg::with_name("replace wm")
                .short("r")
                .long("replace")
                .help("replace an existing window manager")
        )
        .get_matches();
    init_logs();
    let sig_action = SigAction::new(SigHandler::Handler(sig_handle), SaFlags::empty(), SigSet::empty());
    unsafe {
        signal::sigaction(signal::SIGINT, &sig_action).expect("Could not set SIGINT catcher");
    }
    if matches.is_present("client transparency") {
        unimplemented!()
    }
    if matches.is_present("replace wm") {
        unimplemented!()
    }
    if matches.is_present("lua syntax check") {
        let config = matches.value_of("config");
        use crate::lua::SyntaxCheckError::*;

        match lua::syntax_check(config) {
            Err(IoError(err)) => {
                error!("Could not read configuration files");
                error!("{}", err);
                exit(1)
            },
            Err(LuaError(lua_error)) => {
                error!("✘ Configuration file syntax error.");
                error!("{}", lua_error);
                exit(1)
            },
            Ok(_) => {
                info!("✔ Configuration file syntax OK.");
                exit(0)
            }
        }
    }
    let lib_paths = matches
        .values_of("lua lib search")
        .unwrap_or_default()
        .collect::<Vec<_>>();
    lua::init_awesome_libraries(&lib_paths);
    let (display, event_queue, _globals) = init_wayland();
    let (session_fd, system_fd) = dbus::connect().expect("Could not set up dbus connection");
    init_glib(display, event_queue, session_fd, system_fd);
    let config = matches.value_of("config");
    lua::run_awesome(&lib_paths, config);
}

fn init_wayland() -> (Display, EventQueue, GlobalManager) {
    let (display, mut event_queue) = Display::connect_to_env().unwrap_or_else(|err| {
        match err {
            ConnectError::NoWaylandLib => {
                error!("Could not find Wayland library, is it installed and in PATH?")
            },
            ConnectError::NoCompositorListening => {
                error!("Could not connect to Wayland server. Is it running?");
                error!(
                    "WAYLAND_DISPLAY={}",
                    env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "".into())
                );
            },
            ConnectError::InvalidName => error!("Invalid socket name provided in WAYLAND_SOCKET"),
            ConnectError::XdgRuntimeDirNotSet => error!("XDG_RUNTIME_DIR must be set"),
            ConnectError::InvalidFd => error!("Invalid socket provided in WAYLAND_SOCKET")
        }
        exit(1);
    });
    let globals = GlobalManager::new_with_cb(
        &display,
        global_filter!(
            [
                wl_output::WlOutput,
                wayland_obj::WL_OUTPUT_VERSION,
                wayland_obj::WlOutputManager {}
            ],
            [
                wl_compositor::WlCompositor,
                wayland_obj::WL_COMPOSITOR_VERSION,
                wayland_obj::WlCompositorManager {}
            ],
            [
                wl_shm::WlShm,
                wayland_obj::WL_SHM_VERSION,
                wayland_obj::WlShmManager {}
            ]
        )
    );
    event_queue.sync_roundtrip().unwrap();

    globals
        .instantiate_exact(wayland_obj::XDG_WM_BASE_VERSION, wayland_obj::xdg_shell_init)
        .unwrap_or_else(|err| {
            match err {
                GlobalError::Missing => {
                    error!(
                        "Missing xdg_wm_base global (version {})",
                        wayland_obj::XDG_WM_BASE_VERSION
                    );
                    error!("Your compositor doesn't support the xdg shell protocol");
                    error!("This protocol is necessary for Awesome to function");
                },
                GlobalError::VersionTooLow(version) => {
                    error!(
                        "Got xdg_wm_base version {}, expected version {}",
                        version,
                        wayland_obj::XDG_WM_BASE_VERSION
                    );
                    error!(
                        "Your compositor doesn't support version {} \
                         of the xdg shell protocol",
                        wayland_obj::XDG_WM_BASE_VERSION
                    );
                    error!("Ensure your compositor is up to date");
                }
            }
            exit(1);
        });

    event_queue.sync_roundtrip().unwrap();
    (display, event_queue, globals)
}

/// Sets up the glib main loop to call back into Rust whenever the
/// Wayland triggers an event.
///
/// Note this doesn't actually start it yet, see `lua::run_awesome` for that.
fn init_glib(display: Display, event_queue: EventQueue, session_fd: RawFd, system_fd: RawFd) {
    let mut wayland_state = WaylandState { display, event_queue };
    let display_ptr = wayland_state.display.get_display_ptr();
    unsafe {
        wayland_glib_interface_init(
            display_ptr,
            session_fd,
            system_fd,
            &mut wayland_state as *mut _ as _
        );
        ::std::mem::forget(wayland_state);
    }
}

fn setup_awesome_path(lua: rlua::Context, lib_paths: &[&str]) -> rlua::Result<()> {
    let globals = lua.globals();
    let package: Table = globals.get("package")?;
    let mut path = package.get::<_, String>("path")?;
    let mut cpath = package.get::<_, String>("cpath")?;

    for lib_path in lib_paths {
        path.push_str(&format!(";{0}/?.lua;{0}/?/init.lua", lib_path));
        cpath.push_str(&format!(";{}/?.so", lib_path));
    }

    for mut xdg_data_path in env::var("XDG_DATA_DIRS")
        .unwrap_or("/usr/local/share:/usr/share".into())
        .split(':')
        .map(PathBuf::from)
    {
        xdg_data_path.push("awesome/lib");
        path.push_str(&format!(
            ";{0}/?.lua;{0}/?/init.lua",
            xdg_data_path.as_os_str().to_string_lossy()
        ));
        cpath.push_str(&format!(
            ";{}/?.so",
            xdg_data_path.into_os_string().to_string_lossy()
        ));
    }

    for mut xdg_config_path in env::var("XDG_CONFIG_DIRS")
        .unwrap_or("/etc/xdg".into())
        .split(':')
        .map(PathBuf::from)
    {
        xdg_config_path.push("awesome");
        cpath.push_str(&format!(
            ";{}/?.so",
            xdg_config_path.into_os_string().to_string_lossy()
        ));
    }

    package.set("path", path)?;
    package.set("cpath", cpath)?;

    Ok(())
}

/// Set up global signals value
///
/// We need to store this in Lua, because this make it safer to use.
fn setup_global_signals(lua: rlua::Context) -> rlua::Result<()> {
    lua.set_named_registry_value(GLOBAL_SIGNALS, lua.create_table()?)
}

/// Sets up the xcb connection and stores it in Lua (for us to access it later)
fn setup_xcb_connection(lua: rlua::Context) -> rlua::Result<()> {
    let con = match xcb::Connection::connect(None) {
        Err(err) => {
            error!("Way Cooler requires XWayland in order to function");
            error!("However, xcb could not connect to it. Is it running?");
            error!("{:?}", err);
            panic!("Could not connect to XWayland instance");
        },
        Ok(con) => con.0
    };
    // Tell xcb we are using the xkb extension
    match xkb::use_extension(&con, 1, 0).get_reply() {
        Ok(r) => {
            if !r.supported() {
                panic!("xkb-1.0 is not supported");
            }
        },
        Err(err) => {
            panic!("Could not get xkb extension supported version {:?}", err);
        }
    }
    lua.set_named_registry_value(XCB_CONNECTION_HANDLE, LightUserData(con.get_raw_conn() as _))?;
    mem::forget(con);
    Ok(())
}

/// Formats the log strings properly
fn log_format(buf: &mut env_logger::fmt::Formatter, record: &log::Record) -> Result<(), io::Error> {
    let color = match record.level() {
        Level::Info => "",
        Level::Trace => "\x1B[37m",
        Level::Debug => "\x1B[44m",
        Level::Warn => "\x1B[33m",
        Level::Error => "\x1B[31m"
    };
    let mut module_path = record.module_path().unwrap_or("?");
    if let Some(index) = module_path.find("way_cooler::") {
        let index = index + "way_cooler::".len();
        module_path = &module_path[index..];
    }
    writeln!(
        buf,
        "{} {} [{}] \x1B[37m{}:{}\x1B[0m{0} {} \x1B[0m",
        color,
        record.level(),
        module_path,
        record.file().unwrap_or("?"),
        record.line().unwrap_or(0),
        record.args()
    )
}

fn init_logs() {
    let env = env_logger::Env::default().filter_or("WAY_COOLER_LOG", "trace");
    env_logger::Builder::from_env(env).format(log_format).init();
    info!("Logger initialized");
}

/// Handler for SIGINT signal
extern "C" fn sig_handle(_: nix::libc::c_int) {
    lua::terminate();
    exit(130);
}
