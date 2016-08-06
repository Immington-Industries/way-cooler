//! Contains methods for initializing the lua config

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
    if let Ok(path_env) = env::var("WAY_COOLER_CONFIG") {
        if let Ok(file) = read_file(Path::new(&path_env).join(INIT_FILE)) {
            info!("Reading init file from $WAY_COOLER_INIT_FILE: {}", path_env);
            return Ok(file)
        }
        warn!("Error reading from $WAY_COOLER_INIT_FILE, defaulting...");
    }

    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        if let Ok(file) = read_file(Path::new(&xdg).join(INIT_FILE)) {
            info!("Reading init file from $XDG_CONFIG_HOME: {}", xdg);
            return Ok(file)
        }
        warn!("Error reading from $XDG_CONFIG_HOME, defaulting...");
    }
    let dot_config = Path::new(&env::var("HOME")
                               .expect("HOME environment variable not defined!"))
        .join(".config").join("way-cooler").join(INIT_FILE);
    trace!("Looking at {:?}", dot_config);
    if let Ok(file) = read_file(&dot_config) {
        info!("Reading init file from {:?}", &dot_config);
        return Ok(file)
    }

    let etc_config = Path::new(INIT_FILE_FALLBACK_PATH).join(INIT_FILE);

    if let Ok(file) = read_file(&etc_config) {
        info!("Reading init file from fallback {:?}", &etc_config);
        return Ok(file)
    }

    return Err("Could not find an init file in any path!")
}
