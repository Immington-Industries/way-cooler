//! Awesome compatibility modules

extern crate cairo;
extern crate cairo_sys;
extern crate env_logger;
extern crate exec;
extern crate gdk_pixbuf;
extern crate getopts;
extern crate glib;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate nix;
extern crate rlua;
extern crate xcb;
#[macro_use]
extern crate wayland_client;

// TODO remove
extern crate wlroots;
use wlroots::{key_events::KeyEvent, wlr_key_state::*, KeyboardModifier};

#[macro_use]
mod macros;

mod common;
mod objects;
mod wayland_obj;

mod awesome;
mod keygrabber;
mod lua;
mod mousegrabber;
mod root;

use std::{env, mem, path::PathBuf, process::exit};

use exec::Command;
use log::LogLevel;
use lua::setup_lua;
use nix::sys::signal::{self, SaFlags, SigAction, SigHandler, SigSet};
use rlua::{LightUserData, Lua, Table};
use wayland_client::protocol::{wl_display::RequestsTrait, wl_output};
use wayland_client::sys::client::wl_display;
use wayland_client::{Display, GlobalManager};
use xcb::{xkb, Connection};

use self::lua::{LUA, NEXT_LUA};

use self::common::{
    object::{Object, Objectable},
    signal::*,
};
use self::objects::key::Key;
use self::root::ROOT_KEYS_HANDLE;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const GIT_VERSION: &'static str = include_str!(concat!(env!("OUT_DIR"), "/git-version.txt"));
pub const GLOBAL_SIGNALS: &'static str = "__awesome_global_signals";
pub const XCB_CONNECTION_HANDLE: &'static str = "__xcb_connection";

/// Called from `wayland_glib_interface.c` after every call back into the
/// wayland event loop.
///
/// This restarts the Lua thread if there is a new one pending
#[no_mangle]
pub extern "C" fn refresh_awesome() {
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

fn main() {
    let mut opts = getopts::Options::new();
    opts.optflag("", "version", "show version information");
    let matches = match opts.parse(env::args().skip(1)) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("{}", f.to_string());
            exit(1);
        }
    };
    if matches.opt_present("version") {
        if !GIT_VERSION.is_empty() {
            println!("Way Cooler {} @ {}", VERSION, GIT_VERSION);
        } else {
            println!("Way Cooler {}", VERSION);
        }
        return;
    }
    init_logs();
    let sig_action = SigAction::new(
        SigHandler::Handler(sig_handle),
        SaFlags::empty(),
        SigSet::empty(),
    );
    unsafe {
        signal::sigaction(signal::SIGINT, &sig_action).expect("Could not set SIGINT catcher");
    }
    init_wayland();
    lua::setup_lua();
    lua::enter_glib_loop();
}

fn init_wayland() {
    let (display, mut event_queue) = match Display::connect_to_env() {
        Ok(res) => res,
        Err(err) => {
            error!("Could not connect to Wayland server. Is it running?");
            exit(1);
        }
    };
    unsafe {
        #[link(name = "wayland_glib_interface", kind = "static")]
        extern "C" {
            fn wayland_glib_interface_init(display: *mut wl_display);
        }
        wayland_glib_interface_init(display.c_ptr() as *mut wl_display);
    }
    let _globals = GlobalManager::new_with_cb(
        display.get_registry().unwrap(),
        global_filter!([wl_output::WlOutput, 2, wayland_obj::Output::new]),
    );
    // TODO Remove
    event_queue.sync_roundtrip().unwrap();
    event_queue.sync_roundtrip().unwrap();
    event_queue.sync_roundtrip().unwrap();
}

fn setup_awesome_path(lua: &Lua) -> rlua::Result<()> {
    let globals = lua.globals();
    let package: Table = globals.get("package")?;
    let mut path = package.get::<_, String>("path")?;
    let mut cpath = package.get::<_, String>("cpath")?;

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
fn setup_global_signals(lua: &Lua) -> rlua::Result<()> {
    lua.set_named_registry_value(GLOBAL_SIGNALS, lua.create_table()?)
}

/// Sets up the xcb connection and stores it in Lua (for us to access it later)
fn setup_xcb_connection(lua: &Lua) -> rlua::Result<()> {
    let con = match Connection::connect(None) {
        Err(err) => {
            error!("Way Cooler requires XWayland in order to function");
            error!("However, xcb could not connect to it. Is it running?");
            error!("{:?}", err);
            panic!("Could not connect to XWayland instance");
        }
        Ok(con) => con.0,
    };
    // Tell xcb we are using the xkb extension
    match xkb::use_extension(&con, 1, 0).get_reply() {
        Ok(r) => {
            if !r.supported() {
                panic!("xkb-1.0 is not supported");
            }
        }
        Err(err) => {
            panic!("Could not get xkb extension supported version {:?}", err);
        }
    }
    lua.set_named_registry_value(
        XCB_CONNECTION_HANDLE,
        LightUserData(con.get_raw_conn() as _),
    )?;
    mem::forget(con);
    Ok(())
}

/// Emits the Awesome keybindinsg.
fn emit_awesome_keybindings(
    lua: &Lua,
    event: &KeyEvent,
    event_modifiers: KeyboardModifier,
) -> rlua::Result<()> {
    let state_string = if event.key_state() == WLR_KEY_PRESSED {
        "press"
    } else {
        "release"
    };
    // TODO Should also emit by current focused client so we can
    // do client based rules.
    let keybindings = lua.named_registry_value::<Vec<rlua::AnyUserData>>(ROOT_KEYS_HANDLE)?;
    for event_keysym in event.pressed_keys() {
        for binding in &keybindings {
            let obj: Object = binding.clone().into();
            let key = Key::cast(obj.clone()).unwrap();
            let keycode = key.keycode()?;
            let keysym = key.keysym()?;
            let modifiers = key.modifiers()?;
            let binding_match = (keysym != 0 && keysym == event_keysym
                || keycode != 0 && keycode == event.keycode())
                && modifiers == 0
                || modifiers == event_modifiers.bits();
            if binding_match {
                emit_object_signal(&*lua, obj, state_string.into(), event_keysym)?;
            }
        }
    }
    Ok(())
}

/// Formats the log strings properly
fn log_format(record: &log::LogRecord) -> String {
    let color = match record.level() {
        LogLevel::Info => "",
        LogLevel::Trace => "\x1B[37m",
        LogLevel::Debug => "\x1B[44m",
        LogLevel::Warn => "\x1B[33m",
        LogLevel::Error => "\x1B[31m",
    };
    let location = record.location();
    let file = location.file();
    let line = location.line();
    let mut module_path = location.module_path();
    if let Some(index) = module_path.find("way_cooler::") {
        let index = index + "way_cooler::".len();
        module_path = &module_path[index..];
    }
    format!(
        "{} {} [{}] \x1B[37m{}:{}\x1B[0m{0} {} \x1B[0m",
        color,
        record.level(),
        module_path,
        file,
        line,
        record.args()
    )
}

fn init_logs() {
    let mut builder = env_logger::LogBuilder::new();
    builder.format(log_format);
    builder.filter(None, log::LogLevelFilter::Trace);
    if env::var("WAY_COOLER_LOG").is_ok() {
        builder.parse(&env::var("WAY_COOLER_LOG").expect("WAY_COOLER_LOG not defined"));
    }
    builder.init().expect("Unable to initialize logging!");
    info!("Logger initialized");
}

/// Handler for SIGINT signal
extern "C" fn sig_handle(_: nix::libc::c_int) {
    lua::terminate();
    exit(130);
}
