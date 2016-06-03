//! Main module of way-cooler

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate bitflags;

#[cfg(not(test))]
extern crate rustwlc;

#[cfg(test)]
extern crate dummy_rustwlc as rustwlc;

#[macro_use]
extern crate log;
extern crate env_logger;

#[macro_use]
extern crate hlua;
extern crate rustc_serialize;

extern crate petgraph;

use std::env;

use log::LogLevel;

use rustwlc::types::LogType;

#[macro_use] // As it happens, it's important to declare the macros first.
mod macros;

mod callbacks;
mod keys;

mod lua;
mod registry;
mod convert;

mod layout;
mod compositor;

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
    let mut location = record.location().module_path();
    if let Some(index) = location.find("way_cooler::") {
        let index = index + "way_cooler::".len();
        location = &location[index..];
    }
    format!("{} {} [{}] {} \x1B[0m", color, record.level(), location, record.args())
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

fn main() {
    println!("Launching way-cooler...");

    // Start logging first
    init_logs();

    // Initialize callbacks
    callbacks::init();

    // Handle wlc logs
    rustwlc::log_set_rust_handler(log_handler);

    // Prepare to launch wlc
    let run_wlc = rustwlc::init2().expect("Unable to initialize wlc!");

    // (Future config initialization goes here)

    // Add commands to registry
    registry::init();
    // And bind the defaults
    keys::init();

    // Hand control over to wlc's event loop
    info!("Running wlc...");
    run_wlc();
}