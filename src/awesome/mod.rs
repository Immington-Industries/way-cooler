//! Awesome compatibilty modules
use xcb::{xkb, Connection};

use rlua::{self, Lua, Table, LightUserData};

use std::mem;

pub mod keygrabber;
pub mod mousegrabber;
pub mod awful;
mod awesome;
mod client;
mod screen;
mod button;
mod tag;
mod key;
mod drawin;
mod drawable;
mod mouse;
mod root;
pub mod signal;
mod object;
mod class;
mod property;
mod xproperty;

pub use self::object::Object;
pub use self::keygrabber::keygrabber_handle;
pub use self::mousegrabber::mousegrabber_handle;

pub const GLOBAL_SIGNALS: &'static str = "__awesome_global_signals";
pub const XCB_CONNECTION_HANDLE: &'static str = "__xcb_connection";

use std::env;
use std::path::PathBuf;

pub fn init(lua: &Lua) -> rlua::Result<()> {
    setup_awesome_path(lua)?;
    setup_global_signals(lua)?;
    setup_xcb_connection(lua)?;
    button::init(lua)?;
    awesome::init(lua)?;
    key::init(lua)?;
    client::init(lua)?;
    screen::init(lua)?;
    keygrabber::init(lua)?;
    root::init(lua)?;
    mouse::init(lua)?;
    tag::init(lua)?;
    drawin::init(lua)?;
    drawable::init(lua)?;
    mousegrabber::init(lua)?;
    awful::init(lua)?;
    Ok(())
}

fn setup_awesome_path(lua: &Lua) -> rlua::Result<()> {
    let globals = lua.globals();
    let package: Table = globals.get("package")?;
    let mut path = package.get::<_, String>("path")?;
    let mut cpath = package.get::<_, String>("cpath")?;
    let mut xdg_data_path: PathBuf = env::var("XDG_DATA_DIRS").unwrap_or("/usr/share".into()).into();
    xdg_data_path.push("awesome/lib");
    path.push_str(&format!(";{0}/?.lua;{0}/?/init.lua",
                             xdg_data_path.as_os_str().to_string_lossy()));
    package.set("path", path)?;
    let mut xdg_config_path: PathBuf = env::var("XDG_CONFIG_DIRS").unwrap_or("/etc/xdg".into()).into();
    xdg_config_path.push("awesome");
    cpath.push_str(&format!(";{}/?.so;{}/?.so",
                            xdg_config_path.into_os_string().to_string_lossy(),
                            xdg_data_path.into_os_string().to_string_lossy()));
    package.set("cpath", cpath)?;

    // NOTE The debug library does some powerful reflection that can do crazy things,
    // which is why it's unsafe to load.
    unsafe {
        lua.load_debug();
    }
    Ok(())
}

/// Set up global signals value
///
/// We need to store this in Lua, because this make it safer to use.
fn setup_global_signals(lua: &Lua) -> rlua::Result<()> {
    let globals = lua.globals();
    globals.set::<_, Table>(GLOBAL_SIGNALS, lua.create_table())
}

/// Sets up the xcb connection and stores it in Lua (for us to access it later)
fn setup_xcb_connection(lua: &Lua) -> rlua::Result<()> {
    let con = match Connection::connect(None) {
        Err(err) => {
            error!("xcb: Could not connect to xwayland instance. Is it running?");
            error!("{:?}", err);
            return Ok(())
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
    lua.globals().set(XCB_CONNECTION_HANDLE, LightUserData(con.get_raw_conn() as _))?;
    mem::forget(con);
    Ok(())
}

pub fn dummy<'lua>(_: &'lua Lua, _: rlua::Value) -> rlua::Result<()> { Ok(()) }
