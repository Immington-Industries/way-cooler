//! Main module of way-cooler

// TODO FIXME Remove
#![allow(dead_code)]
#![allow(non_upper_case_globals)]

extern crate bitflags;
extern crate cairo;
extern crate cairo_sys;
extern crate env_logger;
extern crate gdk_pixbuf;
extern crate getopts;
extern crate glib;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate nix;
extern crate rlua;
#[macro_use]
extern crate wlroots;
extern crate xcb;

#[macro_use]
mod macros;
mod awesome;
mod compositor;

pub use awesome::lua;

use std::{env, fs::File, io::{BufRead, BufReader}, os::raw::c_void, path::Path, process::exit};

use log::LogLevel;
use nix::sys::signal::{self, SaFlags, SigAction, SigHandler, SigSet};

const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const GIT_VERSION: &'static str = include_str!(concat!(env!("OUT_DIR"), "/git-version.txt"));
const DRIVER_MOD_PATH: &'static str = "/proc/modules";
const DEVICE_MOD_PATH: &'static str = "/sys/firmware/devicetree/base/model";

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
        return
    }
    println!("Launching way-cooler...");

    let sig_action = SigAction::new(SigHandler::Handler(sig_handle),
                                    SaFlags::empty(),
                                    SigSet::empty());
    unsafe {
        signal::sigaction(signal::SIGINT, &sig_action).expect("Could not set SIGINT catcher");
    }

    init_logs();
    log_environment();
    detect_proprietary();
    detect_raspi();
    ensure_good_env();
    let compositor = compositor::init();
    assert!(compositor.xwayland.is_some());
    unsafe {
        #[link(name = "wayland_glib_interface", kind = "static")]
        extern "C" {
            fn wayland_glib_interface_init(display: *mut c_void);
        }
        wayland_glib_interface_init(compositor.display as *mut c_void);
    }
    compositor.run_with(|_| awesome::lua::enter_glib_loop());
}

/// Formats the log strings properly
fn log_format(record: &log::LogRecord) -> String {
    let color = match record.level() {
        LogLevel::Info => "",
        LogLevel::Trace => "\x1B[37m",
        LogLevel::Debug => "\x1B[44m",
        LogLevel::Warn => "\x1B[33m",
        LogLevel::Error => "\x1B[31m"
    };
    let location = record.location();
    let file = location.file();
    let line = location.line();
    let mut module_path = location.module_path();
    if let Some(index) = module_path.find("way_cooler::") {
        let index = index + "way_cooler::".len();
        module_path = &module_path[index..];
    }
    format!("{} {} [{}] \x1B[37m{}:{}\x1B[0m{0} {} \x1B[0m",
            color,
            record.level(),
            module_path,
            file,
            line,
            record.args())
}

/// Ensures that the environment is set up correctly. E.g:
/// * XDG_RUNTIME_DIR is set to a directory that exists.
fn ensure_good_env() {
    use std::env::VarError;
    match env::var("XDG_RUNTIME_DIR") {
        Ok(_) => { /* Do nothing, logged in `log_environment` */ }
        Err(VarError::NotUnicode(string)) => {
            error!("The value set for XDG_RUNTIME_DIR ({:?}) is not valid unicode!",
                   string);
            exit(1);
        }
        Err(VarError::NotPresent) => {
            error!("Please set the XDG_RUNTIME_DIR environment variable.");
            error!("e.g export XDG_RUNTIME_DIR=/run/user/1000");
            exit(1);
        }
    }
}

/// Checks the loaded modules, and reports any problematic proprietary ones
fn detect_proprietary() {
    // If DISPLAY is present, we are running embedded
    if env::var("DISPLAY").is_ok() {
        return
    }
    match File::open(Path::new(DRIVER_MOD_PATH)) {
        Ok(file) => {
            let reader = BufReader::new(&file);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if line.contains("nvidia") {
                        error!("Error: Proprietary nvidia graphics drivers are installed, but \
                                they are not compatible with Wayland. Consider using nouveau \
                                drivers for Wayland.");
                        exit(1);
                    }
                }
            }
        }
        Err(err) => {
            warn!("Could not read proprietary modules at \"{}\", because: {:#?}",
                  DRIVER_MOD_PATH, err);
            warn!("If you are running proprietary Nvidia graphics drivers, Way Cooler will not \
                   work for you");
        }
    }
}

/// Checks the loaded modules to ensure vc4 is loaded if we are running on a
/// raspi.
fn detect_raspi() {
    let raspi = match File::open(Path::new(DEVICE_MOD_PATH)) {
        Ok(f) => {
            let reader = BufReader::new(&f);
            let mut raspi = false;
            for line in reader.lines() {
                if let Ok(line) = line {
                    if line.contains("Raspberry Pi") {
                        raspi = true;
                        break
                    }
                }
            }
            raspi
        }
        Err(_) => return
    };
    let vc4 = match File::open(Path::new(DRIVER_MOD_PATH)) {
        Ok(f) => {
            let reader = BufReader::new(&f);
            let mut vc4 = false;
            for line in reader.lines() {
                if let Ok(line) = line {
                    if line.contains("vc4") {
                        vc4 = true;
                        break
                    }
                }
            }
            vc4
        }
        Err(err) => {
            warn!("Could not read file \"{}\", because {:#?}",
                  DRIVER_MOD_PATH, err);
            return
        }
    };
    if !vc4 && raspi {
        error!("You are running on a Raspberry Pi, but the vc4 module is not loaded!");
        error!("Set 'dtoverlay=vc4-kms-v3d' in /boot/config.txt and reboot!");
    }
}

/// Initializes the logging system.
pub fn init_logs() {
    wlroots::utils::init_logging(wlroots::utils::L_DEBUG, None);
    let mut builder = env_logger::LogBuilder::new();
    builder.format(log_format);
    builder.filter(None, log::LogLevelFilter::Trace);
    if env::var("WAY_COOLER_LOG").is_ok() {
        builder.parse(&env::var("WAY_COOLER_LOG").expect("WAY_COOLER_LOG not defined"));
    }
    builder.init().expect("Unable to initialize logging!");
    info!("Logger initialized");
}

fn log_environment() {
    for (key, value) in env::vars() {
        info!("{}: {}", key, value);
    }
}

/// Handler for signals
extern "C" fn sig_handle(_: nix::libc::c_int) {
    wlroots::terminate();
    awesome::lua::terminate();
}
