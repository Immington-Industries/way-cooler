//! Main module in way-cooler

#[macro_use]
extern crate lazy_static;

extern crate rustwlc;

#[macro_use]
extern crate log;
extern crate env_logger;

#[macro_use]
extern crate hlua;

extern crate rustc_serialize;

use rustwlc::callback;
use rustwlc::types::LogType;

use std::env;

mod registry;
mod keys;
mod callbacks;
mod lua;

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
    format!("{} [{}] {}", record.level(), record.location().module_path(),
            record.args())
}

fn main() {
    callback::output_created(callbacks::output_created);
    callback::output_destroyed(callbacks::output_destroyed);
    callback::output_focus(callbacks::output_focus);
    callback::output_resolution(callbacks::output_resolution);
        //.output_render_pre(callbacks::output_render_pre)
        //.output_render_post(callbacks::output_render_post)
    callback::view_created(callbacks::view_created);
    callback::view_destroyed(callbacks::view_destroyed);
    callback::view_focus(callbacks::view_focus);
    callback::view_move_to_output(callbacks::view_move_to_output);
    callback::view_request_geometry(callbacks::view_request_geometry);
    callback::view_request_state(callbacks::view_request_state);
    callback::view_request_move(callbacks::view_request_move);
    callback::view_request_resize(callbacks::view_request_resize);
    callback::keyboard_key(callbacks::keyboard_key);
    callback::pointer_button(callbacks::pointer_button);
    callback::pointer_scroll(callbacks::pointer_scroll);
    callback::pointer_motion(callbacks::pointer_motion);
        //.touch_touch(callbacks::touch)
    callback::compositor_ready(callbacks::compositor_ready);
        //.input_created(input_created)
        //.input_destroyed(input_destroyed);

    let mut builder = env_logger::LogBuilder::new();
    builder.format(log_format);
    builder.filter(None, log::LogLevelFilter::Trace);
    if env::var("WAY_COOLER_LOG").is_ok() {
        builder.parse(&env::var("WAY_COOLER_LOG").unwrap());
    }
    builder.init().unwrap();
    info!("Logger initialized, setting wlc handler.");
    rustwlc::log_set_handler(log_handler);

    let run_wlc = rustwlc::init2().expect("Unable to initialize wlc!");

    info!("Started logger");

    run_wlc();
}
