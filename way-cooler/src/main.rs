//! Main module of way-cooler

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate bitflags;

#[cfg(not(test))]
extern crate rustwlc;

extern crate getopts;

#[cfg(test)]
extern crate dummy_rustwlc as rustwlc;

#[macro_use]
extern crate log;
extern crate env_logger;

#[macro_use]
extern crate hlua;
extern crate rustc_serialize;
#[macro_use]
extern crate json_macro;

extern crate nix;

extern crate petgraph;

extern crate uuid;

#[macro_use]
extern crate wayland_client;

extern crate tempfile;

extern crate byteorder;

use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use getopts::Options;
use log::LogLevel;

use nix::sys::signal::{SigHandler, SigSet, SigAction, SaFlags};
use nix::sys::signal;

use rustwlc::types::LogType;

#[macro_use] // As it happens, it's important to declare the macros first.
mod macros;
mod convert;

mod callbacks;
mod keys;

mod lua;
mod registry;
mod commands;
mod ipc;

mod layout;
mod background;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const MODULE_PATH: &'static str = "/proc/modules";

/// Callback to route wlc logs into env_logger
fn log_handler(level: LogType, message: &str) {
    match level {
        LogType::Info => info!("wlc: {}", message),
        LogType::Warn => warn!("wlc: {}", message),
        LogType::Error => error!("wlc: {}", message),
        LogType::Wayland => info!("wayland: {}", message)
    }
}

/// Formats the log strings properly
fn log_format(record: &log::LogRecord) -> String {
    let color = match record.level() {
        LogLevel::Info => "",
        LogLevel::Trace => "\x1B[37m",
        LogLevel::Debug => "\x1B[37m",
        LogLevel::Warn =>  "\x1B[33m",
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
    format!("{} {} [{}] \x1B[37m{}:{}\x1B[0m{0} {} \x1B[0m",
            color, record.level(), module_path, file, line, record.args())
}

/// Checks the loaded modules, and reports any problematic proprietary ones
fn detect_proprietary() {
    if env::var("DISPLAY").is_ok() {
        // If DISPLAY is present, we are running embedded
        return
    }
    match File::open(Path::new(MODULE_PATH)) {
        Ok(file) => {
            let reader = BufReader::new(&file);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if line.contains("nvidia") {
                        error!("Warning: Proprietary nvidia graphics drivers are installed, \
                               but they are not compatible with Wayland. Consider using nouveau drivers for Wayland.");
                        break;
                    } else if line.contains("fglrx") {
                        error!("Warning: Proprietary AMD graphics drivers are installed, but they are not compatible with Wayland. \
                               Consider using radeon drivers for Waylad.");
                        break;
                    }
                }
            }
        },
        Err(err) => {
            warn!("Could not read proprietary modules at \"{}\", because: {:#?}",
                  MODULE_PATH, err);
        }
    }
}

/// Initializes the logging system.
/// Can be called from within test methods.
pub fn init_logs() {
    // Prepare log builder
    let mut builder = env_logger::LogBuilder::new();
    builder.format(log_format);
    builder.filter(None, log::LogLevelFilter::Trace);
    if env::var("WAY_COOLER_LOG").is_ok() {
        builder.parse(&env::var("WAY_COOLER_LOG").expect("Asserted unwrap!"));
    }
    builder.init().expect("Unable to initialize logging!");
    info!("Logger initialized, setting wlc handlers.");
}

/// Handler for signals, should close the ipc
extern "C" fn sig_handle(_: nix::libc::c_int) {
    rustwlc::terminate();
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut opts = Options::new();
    opts.optflag("", "version", "show version information");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            println!("{}", f.to_string());
            std::process::exit(1);
        }
    };
    if matches.opt_present("version") {
        println!("Way Cooler {}", VERSION);
        println!("Way Cooler IPC version {}", ipc::VERSION);
        return
    }
    println!("Launching way-cooler...");

    let sig_action = SigAction::new(SigHandler::Handler(sig_handle), SaFlags::empty(), SigSet::empty());
    unsafe {signal::sigaction(signal::SIGINT, &sig_action).unwrap() };

    // Start logging first
    init_logs();
    // Detect proprietary modules loaded, and warn about them.
    detect_proprietary();

    // Initialize callbacks
    callbacks::init();

    // Handle wlc logs
    rustwlc::log_set_rust_handler(log_handler);

    // Prepare to launch wlc
    let run_wlc = rustwlc::init2().expect("Unable to initialize wlc!");

    // (Future config initialization goes here)
    // Initialize commands
    commands::init();
    // Add API to registry
    registry::init();
    // Register Alt+Esc keybinding
    keys::init();
    // Start listening for clients
    let _ipc = ipc::init();

    // Hand control over to wlc's event loop
    info!("Running wlc...");
    run_wlc();
}
