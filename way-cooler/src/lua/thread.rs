//! Code for the internal Lua thread which handles all Lua requests.

use std::thread;
use std::fs::{File};
use std::path::Path;
use std::fmt::{Debug, Formatter};
use std::fmt::Result as FmtResult;
use std::sync::{Mutex, RwLock};
use std::sync::mpsc::{channel, Sender, Receiver};

use hlua::{Lua, LuaError};

use super::types::*;
use super::rust_interop;
use super::init_path;

lazy_static! {
    /// Sends requests to the Lua thread
    static ref SENDER: Mutex<Option<Sender<LuaMessage>>> = Mutex::new(None);

    /// Whether the Lua thread is currently running
    pub static ref RUNNING: RwLock<bool> = RwLock::new(false);
}

const ERR_LOCK_RUNNING: &'static str = "Lua thread: unable to lock RUNNING";
const ERR_LOCK_SENDER: &'static str = "Lua thread: unable to lock SENDER";

/// Struct sent to the Lua query
struct LuaMessage {
    reply: Sender<LuaResponse>,
    query: LuaQuery
}

unsafe impl Send for LuaMessage { }
unsafe impl Sync for LuaMessage { }


impl Debug for LuaMessage {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "LuaMessage({:?})", self.query)
    }
}

// Reexported in lua/mod.rs:11
/// Whether the Lua thread is currently available.
pub fn running() -> bool {
    *RUNNING.read().expect(ERR_LOCK_RUNNING)
}

// Reexported in lua/mod.rs:11
/// Errors which may arise from attempting
/// to sending a message to the Lua thread.
#[derive(Debug)]
pub enum LuaSendError {
    /// The thread crashed, was shut down, or rebooted.
    ThreadClosed,
    /// The thread has not been initialized yet (maybe not used)
    ThreadUninitialized,
    /// The sender had an issue, most likey because the thread panicked.
    /// Following the `Sender` API, the original value sent is returned.
    Sender(LuaQuery)
}

// Reexported in lua/mod.rs:11
/// Attemps to send a LuaQuery to the Lua thread.
pub fn send(query: LuaQuery) -> Result<Receiver<LuaResponse>, LuaSendError> {
    if !running() {
        return Err(LuaSendError::ThreadClosed);
    }
    let thread_sender: Sender<LuaMessage>;
    {
        let maybe_sender = SENDER.lock().expect(ERR_LOCK_SENDER);
        match *maybe_sender {
            Some(ref real_sender) => {
                // Senders are designed to be cloneable
                thread_sender = real_sender.clone();
            },
            // If the sender doesn't exist yet, the thread doesn't either
            None => {
                return Err(LuaSendError::ThreadUninitialized);
            }
        }
    }
    // Create a response channel
    let (response_tx, response_rx) = channel();
    let message = LuaMessage { reply: response_tx, query: query };
    match thread_sender.send(message) {
        Ok(_) => Ok(response_rx),
        Err(e) => Err(LuaSendError::Sender(e.0.query))
    }
}

/// Initialize the Lua thread.
pub fn init() {
    debug!("Initializing...");
    let (tx, receiver) = channel();
    *SENDER.lock().expect(ERR_LOCK_SENDER) = Some(tx);
    let mut lua = Lua::new();
    debug!("Loading Lua libraries...");
    lua.openlibs();
    debug!("Loading way-cooler libraries...");
    rust_interop::register_libraries(&mut lua);

    let (use_config, maybe_init_file) = init_path::get_config();
    if use_config {
        match maybe_init_file {
            Ok(init_file) => {
                // TODO defaults here are important
                let _: () = lua.execute_from_reader(init_file)
                    .expect("Unable to load init file");
                debug!("Read init.lua successfully");
            }
            Err(reason) => {
                panic!("Unable to load init file: {}", reason)
            }
        }
    }
    else {
        info!("Skipping config search");
    }

    // Only ready after loading libs
    *RUNNING.write().expect(ERR_LOCK_RUNNING) = true;
    debug!("Entering main loop...");
    let _lua_handle = thread::Builder::new()
        .name("Lua thread".to_string())
        .spawn(move || { main_loop(receiver, &mut lua) });
}

/// Main loop of the Lua thread:
///
/// ## Loop
/// * Wait for a message from the receiver
/// * Handle message
/// * Send response
fn main_loop(receiver: Receiver<LuaMessage>, lua: &mut Lua) {
    loop {
        trace!("Lua: awaiting request");
        let request = receiver.recv();
        match request {
            Err(e) => {
                error!("Lua thread: unable to receive message: {}", e);
                error!("Lua thread: now panicking!");
                *RUNNING.write().expect(ERR_LOCK_RUNNING) = false;

                panic!("Lua thread: lost contact with host, exiting!");
            }
            Ok(message) => {
                trace!("Handling a request");
                handle_message(message, lua);
            }
        }
    }
}

/// Handle each LuaQuery option sent to the thread
fn handle_message(request: LuaMessage, lua: &mut Lua) {
    match request.query {
        LuaQuery::Terminate => {
            trace!("Received terminate signal");
            *RUNNING.write().expect(ERR_LOCK_RUNNING) = false;
            thread_send(request.reply, LuaResponse::Pong);

            info!("Lua thread terminating!");
        },

        LuaQuery::Restart => {
            use std::time::Duration;
            trace!("Received restart signal!");
            error!("Lua thread restart not supported!");

            *RUNNING.write().expect(ERR_LOCK_RUNNING) = false;
            thread_send(request.reply, LuaResponse::Pong);

            // The only real way to restart
            let _new_handle = thread::Builder::new()
                .name("Lua re-init".to_string())
                .spawn(move || {
                    thread::sleep(Duration::from_secs(4));
                    init();
                });

            panic!("Lua thread: Restarting!");
        },

        LuaQuery::Execute(code) => {
            trace!("Received request to execute {}", code);

            match lua.execute::<()>(&code) {
                Err(error) => {
                    warn!("Error executing code: {:?}", error);
                    thread_send(request.reply, LuaResponse::Error(error));
                }
                Ok(_) => {
                    trace!("Code executed okay.");
                    thread_send(request.reply, LuaResponse::Pong);
                }
            }
        },

        LuaQuery::ExecFile(name) => {
            info!("Executing {}", name);

            let path = Path::new(&name);
            let try_file = File::open(path);

            if let Ok(file) = try_file {
                let result = lua.execute_from_reader::<(), File>(file);
                if let Err(err) = result {
                    warn!("Error executing {}!", name);

                    thread_send(request.reply, LuaResponse::Error(err));
                }
                else {
                    trace!("Execution of {} successful.", name);
                    thread_send(request.reply, LuaResponse::Pong);
                }
            }
            else { // Could not open file
                // Unwrap_err is used because we're in the else of let Ok
                let read_error =
                    LuaError::ReadError(try_file.unwrap_err());

                thread_send(request.reply, LuaResponse::Error(read_error));
            }
        },
        LuaQuery::ExecRust(func) => {
            let result = func(lua);
            thread_send(request.reply, LuaResponse::Variable(Some(result)));
        },
        LuaQuery::HandleKey(press) => {
            trace!("Lua: handling keypress {}", &press);
            let press_ix = press.get_lua_index_string();
            // Access the index
            let code = format!("__key_map['{}']()", press_ix);
            match lua.execute::<()>(&code) {
                Err(error) => {
                    warn!("Error handling {}: {:?}", &press, error);
                    thread_send(request.reply, LuaResponse::Error(error));
                }
                Ok(_) => {
                    trace!("Handled keypress okay.");
                    thread_send(request.reply, LuaResponse::Pong);
                }
            }
        }
        LuaQuery::Ping => {
            thread_send(request.reply, LuaResponse::Pong);
        },
    }
}

fn thread_send(sender: Sender<LuaResponse>, response: LuaResponse) {
    match sender.send(response) {
        Err(err) => {
            match err.0 {
                LuaResponse::Pong => {}, // Those are boring
                _ => {
                    warn!("thread: Someone dropped an important Lua response!");
                }
            }
        }
        Ok(_) => {}
    }
}
