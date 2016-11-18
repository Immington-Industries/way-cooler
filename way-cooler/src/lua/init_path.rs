//! Contains methods for initializing the lua config
#![allow(dead_code)]

use std::fs::{OpenOptions, File};
use std::env;
use std::path::Path;

use std::io::Result as IOResult;

pub const INIT_FILE: &'static str = "init.lua";
pub const INIT_FILE_FALLBACK_PATH: &'static str = "/etc/way-cooler/";

#[inline]
fn read_file<P: AsRef<Path>>(path: P) -> IOResult<File> {
    OpenOptions::new().read(true).open(path)
}

#[cfg(test)]
pub fn get_config() -> (bool, Result<File, &'static str>) {
    (false, Err("Loading config should be ignored during tests for now"))
}

#[cfg(not(test))]
pub fn get_config() -> (bool, Result<File, &'static str>) {
    (true, get_config_file())
}

/// Parses environment variables
fn get_config_file() -> Result<File, &'static str> {
    let home_var = env::var("HOME").expect("HOME environment variable not defined!");
    let home = home_var.as_str();

    if let Ok(path_env) = env::var("WAY_COOLER_INIT_FILE") {
        info!("Found $WAY_COOLER_INIT_FILE to be defined, will look for the init file there.");
        if let Ok(file) = read_file(Path::new(&path_env)) {
            info!("Reading init file from $WAY_COOLER_INIT_FILE: {}",
                  path_env.as_str().replace(home, "~"));
            return Ok(file)
        }
        warn!("Did not find an init file at $WAY_COOLER_INIT_FILE! It points to {}",
              path_env.as_str().replace(home, "~"));
    }

    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        let init_file_path = Path::new(&xdg).join("way-cooler").join(INIT_FILE);
        info!("Found $XDG_CONFIG_DIR to be defined, will look for the init file at $XDG_CONFIG_DIR/way-cooler/init.lua");
        if let Ok(file) = read_file(&init_file_path) {
            info!("Reading init file from $XDG_CONFIG_HOME/way-cooler/init.lua");
            return Ok(file)
        }
        else {
            warn!("Did not find an init file inside $XDG_CONFIG_HOME, no file {}.",
                  &init_file_path.to_string_lossy().replace(home, "~"));
        }
    }
    let dot_config = Path::new(home).join(".config").join("way-cooler").join(INIT_FILE);

    trace!("Looking for init file at ~/.config/way-cooler/init.lua");
    if let Ok(file) = read_file(&dot_config) {
        info!("Reading init file from ~/.config/way-cooler/init.lua");
        return Ok(file)
    } else {
        warn!("No init file found in ~/.config, will default to /etc as a last resort.");
    }

    let etc_config = Path::new(INIT_FILE_FALLBACK_PATH).join(INIT_FILE);

    if let Ok(file) = read_file(&etc_config) {
        info!("Reading init file from fallback {:?}", &etc_config);
        return Ok(file)
    }

    warn!("way-cooler was unable to find an init file. \
           This is currently required to use way-cooler.");
    warn!("Our default init.lua is available on GitHub, \
           it is included with the source.");
    warn!("You may place it in ~/.config/way-cooler/init.lua, \
           or a similar folder if you use $XDG_CONFIG_HOME, or /etc/way-cooler.");
    warn!("You may also use the environment variable $WAY_COOLER_INIT_FILE to point \
           directly to the file of your choice.");

    info!("The init file will be included with way-cooler in the next release. If you \
           do not have one or do not wish to customize we will always have the default \
           to fall back on.");

    return Err("Could not find an init file in any path!")
}
