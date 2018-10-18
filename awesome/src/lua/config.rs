//! Contains methods for initializing the lua config

use std::env;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rlua::{self, Lua, Table};

const INIT_FILE: &'static str = "rc.lua";
const INIT_FILE_FALLBACK_PATH: &'static str = "/etc/way-cooler/";

pub const DEFAULT_CONFIG: &'static str = include_str!("../../../config/rc.lua");

/// Finds the configuration file and then loads it.
///
/// If a configuration file could not be found, the pre-compiled version is used.
pub fn load_config(mut lua: &mut Lua) {
    info!("Loading way-cooler libraries...");

    let maybe_init_file = get_config();
    match maybe_init_file {
        Some((init_dir, mut init_file)) => {
            if init_dir.components().next().is_some() {
                // Add the config directory to the package path.
                let globals = lua.globals();
                let package: Table = globals.get("package").expect("package not defined in Lua");
                let paths: String = package
                    .get("path")
                    .expect("package.path not defined in Lua");
                package
                    .set(
                        "path",
                        paths + ";" + init_dir
                            .join("?.lua")
                            .to_str()
                            .expect("init_dir not a valid UTF-8 string"),
                    ).expect("Failed to set package.path");
            }
            let mut init_contents = String::new();
            init_file
                .read_to_string(&mut init_contents)
                .expect("Could not read contents");
            lua.exec(init_contents.as_str(), Some("init.lua".into()))
                .map(|_: ()| info!("Read init.lua successfully"))
                .or_else(|err| {
                    log_error(err);
                    info!("Defaulting to pre-compiled init.lua");
                    unsafe {
                        *lua = Lua::new_with_debug();
                    }
                    ::lua::register_libraries(&mut lua)?;
                    lua.exec(DEFAULT_CONFIG, Some("init.lua <DEFAULT>".into()))
                }).expect("Unable to load pre-compiled init file");
        }
        None => {
            warn!("Could not find an init file in any path!");
            warn!("Defaulting to pre-compiled init.lua");
            let _: () = lua
                .exec(DEFAULT_CONFIG, Some("init.lua <DEFAULT>".into()))
                .or_else(|err| {
                    log_error(err.clone());
                    Err(err)
                }).expect("Unable to load pre-compiled init file");
        }
    }
    ::lua::emit_refresh(lua);
}

fn get_config() -> Option<(PathBuf, File)> {
    let home_var = env::var("HOME").expect("HOME environment variable not defined!");
    let home = home_var.as_str();

    let mut paths: [Option<PathBuf>; 4] = [
        None,
        None,
        Some(Path::new(home).join(".config").join("way-cooler")),
        Some(INIT_FILE_FALLBACK_PATH.into()),
    ];

    if let Ok(path) = env::var("WAY_COOLER_INIT_FILE").map(|path| PathBuf::from(path)) {
        let path = path.parent().map_or(PathBuf::new(), Path::to_path_buf);
        paths[0] = Some(path);
    }

    if let Ok(path) = env::var("XDG_CONFIG_HOME").map(|path| PathBuf::from(path)) {
        paths[1] = Some(path);
    }

    for path in paths.iter_mut() {
        let (original_path, path) = match path.take() {
            Some(path) => (path.clone(), path.join(INIT_FILE)),
            None => continue,
        };
        if let Ok(file) = OpenOptions::new().read(true).open(path.clone()) {
            info!("Found init file @ {:?}", path);
            return Some((original_path, file));
        }
    }
    return None;
}

fn log_error(err: rlua::Error) {
    fn recursive_callback_print(error: Arc<rlua::Error>) {
        match *error {
            rlua::Error::CallbackError {
                traceback: ref err,
                ref cause,
            } => {
                error!("{}", err);
                recursive_callback_print(cause.clone())
            }
            ref err => error!("{:?}", err),
        }
    }
    match err {
        rlua::Error::RuntimeError(ref err) => {
            error!("{}", err);
        }
        rlua::Error::CallbackError {
            traceback: ref err,
            ref cause,
        } => {
            error!("traceback: {}", err);
            recursive_callback_print(cause.clone());
        }
        err => {
            error!("init file error: {:?}", err);
        }
    }
}
