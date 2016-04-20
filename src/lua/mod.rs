//! Lua functionality

use hlua;
use hlua::{Lua, LuaError};
use hlua::any::AnyLuaValue;

use std::thread;

use std::fs::{File};
use std::path::Path;
use std::io::Write;

use std::sync::{Mutex, RwLock};
use std::sync::mpsc::{channel, Sender, Receiver};

#[macro_use]
mod funcs;
#[cfg(test)]
mod tests;

lazy_static! {
    /// Sends requests to the lua thread
    static ref SENDER: Mutex<Option<Sender<LuaQuery>>> = Mutex::new(None);

    /// Receives data back from the lua thread
    /// This should only be accessed by the lua thread itself.
    static ref RECEIVER: Mutex<Option<Receiver<LuaResponse>>> = Mutex::new(None);

    /// Whether the lua thread is currently running
    pub static ref RUNNING: RwLock<bool> = RwLock::new(false);
}

/// Messages sent to the lua thread
pub enum LuaQuery {
    /// Halt the lua thread
    Terminate,
    // Restart the lua thread
    Restart,
    /// Execute a string
    Execute(String),
    /// Execute a file
    ExecuteFile(String),
    /// Get a variable
    GetVariable(String),
    /// Set a value
    SetValue {
        name: Box<::std::borrow::Borrow<str> + Sized>,
        val: Box<hlua::Push<&'static mut Lua<'static>> + Sized>
    },
    /// Create a new array
    EmptyArray(String),
    /// Message to ping the lua thread
    Ping,
    /// Unused send type
    Unused,
}

/// Messages received from lua thread
pub enum LuaResponse {
    /// Lua variable obtained
    Variable(Option<AnyLuaValue>),
    /// Lua error
    Error(hlua::LuaError),
    /// A function is returned
    Function(hlua::functions_read::LuaFunction<String>),
    /// Pong response from lua ping
    Pong,
    /// Unused response type
    Unused,
}

unsafe impl Send for LuaQuery { }
unsafe impl Send for LuaResponse { }
unsafe impl Sync for LuaQuery { }
unsafe impl Sync for LuaResponse { }

/// Whether the lua thread is currently available
pub fn thread_running() -> bool {
    *RUNNING.read().unwrap()
}

/// Errors which may arise from attempting
/// to sending a message to the lua thread.
#[derive(Debug)]
pub enum LuaSendError {
    /// The thread crashed, was shut down, or rebooted.
    ThreadClosed,
    /// The thread has not been initialized yet (maybe not used)
    ThreadUninitialized,
    /// The sender had an issue, most likey because the thread panicked
    Sender
}

/// Attemps to send a LuaQuery to the lua thread.
pub fn try_send(query: LuaQuery) -> Result<(), LuaSendError> {
    if !thread_running() {
        Err(LuaSendError::ThreadClosed)
    }
    else if let Some(ref sender) = *SENDER.lock().unwrap() {
        match sender.send(query) {
            Ok(_) => Ok(()),
            Err(_) => Err(LuaSendError::Sender)
        }
    }
    else {
        Err(LuaSendError::ThreadUninitialized)
    }
}

/// Initialize the lua thread
pub fn init() {
    trace!("Initializing...");
    let (query_tx, query_rx) = channel::<LuaQuery>();
    let (answer_tx, answer_rx) = channel::<LuaResponse>();
    {
        let mut sender = SENDER.lock().unwrap();
        let mut receiver = RECEIVER.lock().unwrap();

        *sender = Some(query_tx);
        *receiver = Some(answer_rx);
    }

    thread::spawn(move || {
        thread_init(answer_tx, query_rx);
    });
    trace!("Created thread. Init finished.");
}

fn thread_init(sender: Sender<LuaResponse>, receiver: Receiver<LuaQuery>) {
    trace!("thread: initializing.");
    let mut lua = Lua::new();
    //unsafe {
    //    hlua_ffi::lua_atpanic(&mut lua.as_mut_lua().0, thread_on_panic);
    //}
    debug!("thread: Loading Lua libraries...");
    lua.openlibs();
    trace!("thread: Loading way-cooler lua extensions...");
    // We should have some good file handling, read files from /usr by default,
    // but for now we're reading directly from the source.
    lua.execute_from_reader::<(), File>(
        File::open("lib/lua/init.lua").unwrap()
    ).unwrap();
    trace!("thread: loading way-cooler libraries...");
    funcs::register_libraries(&mut lua);
    // Only ready after loading libs
    *RUNNING.write().unwrap() = true;
    debug!("thread: entering main loop...");
    thread_main_loop(sender, receiver, &mut lua);
}

fn thread_main_loop(sender: Sender<LuaResponse>, receiver: Receiver<LuaQuery>,
                    lua: &mut Lua) {
    loop {
        let request = receiver.recv();
        match request {
            Err(e) => {
                error!("Lua thread: unable to receive message: {}", e);
                error!("Lua thread: now panicking!");
                *RUNNING.write().unwrap() = false;

                panic!("Lua thread: lost contact with host, exiting!");
            }
            Ok(message) => {
                trace!("Handling a request");
                thread_handle_message(&sender, message, lua);
            }
        }
    }
}

fn thread_handle_message(sender: &Sender<LuaResponse>,
                         request: LuaQuery, lua: &mut Lua) {
    match request {
        LuaQuery::Terminate => {
            trace!("thread: Received terminate signal");
            *RUNNING.write().unwrap() = false;

            info!("thread: Lua thread terminating!");
            return;
        },

        LuaQuery::Restart => {
            trace!("thread: Received restart signal!");
            error!("thread: Lua thread restart not supported!");

            *RUNNING.write().unwrap() = false;

            panic!("Lua thread: Restart not supported!");
        },

        LuaQuery::Execute(code) => {
            trace!("thread: Received request to execute code");
            trace!("thread: Executing {:?}", code);

            match lua.execute::<()>(&code) {
                Err(error) => {
                    warn!("thread: Error executing code: {:?}", error);
                    let response = LuaResponse::Error(error);

                    thread_send(&sender, response);
                }
                Ok(_) => {
                    // This is gonna be really spammy one day
                    trace!("thread: Code executed okay.");
                }
            }
        },

        LuaQuery::ExecuteFile(name) => {
            trace!("thread: Received request to execute file {}", name);
            info!("thread: Executing {}", name);

            let path = Path::new(&name);
            let try_file = File::open(path);

            if let Ok(file) = try_file {
                let result = lua.execute_from_reader::<(), File>(file);
                if let Err(err) = result {
                    warn!("thread: Error executing {}!", name);

                    thread_send(&sender, LuaResponse::Error(err));
                }
                else {
                    trace!("thread: Execution of {} successful.", name);
                }
            }
            else { // Could not open file
                // Unwrap_err is used because we're in the else of let
                let read_error =
                    LuaError::ReadError(try_file.unwrap_err());

                thread_send(&sender, LuaResponse::Error(read_error));
            }
        },

        LuaQuery::GetVariable(varname) => {
            trace!("thread: Received request to get variable {}", varname);
            let var_result = lua.get(varname.as_str());

            match var_result {
                Some(var) => {
                    thread_send(&sender, LuaResponse::Variable(Some(var)));
                }
                None => {
                    warn!("thread: Unable to get variable {}", varname);

                    thread_send(&sender, LuaResponse::Variable(None));
                }
            }
        },

        LuaQuery::SetValue { name: _name, val: _val } => {
            panic!("thread: unimplemented LuaQuery::SetValue!");
        },

        LuaQuery::EmptyArray(_name) => {
            panic!("thread: unimplemented LuaQuery::EmptyArray!");
        },

        _ => {
            panic!("Unimplemented send type for lua thread!");
        }
    }
}

fn thread_send(sender: &Sender<LuaResponse>, response: LuaResponse) {
    trace!("Called thread_send");
    match sender.send(response) {
        Err(_) => {
            error!("thread: Unable to broadcast response!");
            error!("thread: Shutting down in response to inability \
                    to continue!");
            panic!("Lua thread unable to communicate with main thread, \
                    shutting down!");
        }
        Ok(_) => {}
    }
}
