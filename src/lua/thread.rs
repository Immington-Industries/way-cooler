//! Code for the internal Lua thread which handles all Lua requests.

use std::collections::btree_map::BTreeMap;
use std::thread;
use std::fs::{File};
use std::path::Path;
use std::fmt::{Debug, Formatter};
use std::fmt::Result as FmtResult;
use std::sync::{Mutex, RwLock};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::io::Read;
use super::LUA;

use convert::json::lua_to_json;

use rustc_serialize::json::Json;
use uuid::Uuid;
use rlua;

use super::types::*;
use super::rust_interop;
use super::init_path;
use super::super::keys;

use registry::{self};

use ::layout::{lock_tree, ContainerType};

lazy_static! {
    /// Sends requests to the Lua thread
    static ref SENDER: Mutex<Option<Sender<LuaMessage>>> = Mutex::new(None);

    /// Whether the Lua thread is currently running
    pub static ref RUNNING: RwLock<bool> = RwLock::new(false);

    /// Requests to update the registry state from Lua
    static ref REGISTRY_QUEUE: RwLock<Vec<String>> = RwLock::new(vec![]);
}

pub const ERR_LOCK_RUNNING: &'static str = "Lua thread: unable to lock RUNNING";
pub const ERR_LOCK_SENDER: &'static str = "Lua thread: unable to lock SENDER";
pub const ERR_LOCK_QUEUE: &'static str =
    "Lua thread: unable to lock REGISTRY_QUEUE";

const INIT_LUA_FUNC: &'static str = "way_cooler.on_init()";
const LUA_TERMINATE_CODE: &'static str = "way_cooler.on_terminate()";
const LUA_RESTART_CODE: &'static str = "way_cooler.on_restart()";

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

/// Appends this combination of category and key to the registry queue.
pub fn update_registry_value(category: String) {
    let mut queue = REGISTRY_QUEUE.write().expect(ERR_LOCK_QUEUE);
    queue.push(category);
}

// Reexported in lua/mod.rs
/// Run a closure with the Lua state. The closure will execute in the Lua thread.
pub fn run_with_lua<F, G>(func: F) -> G
    where F: FnOnce(&mut rlua::Lua) -> G
{
    let mut lua = LUA.lock().expect("LUA was poisoned!");
    let lua = &mut lua.0;
    func(lua)
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
pub fn init() -> Result<(), rlua::Error> {
    info!("Initializing lua...");
    let (tx, receiver) = channel();
    *SENDER.lock().expect(ERR_LOCK_SENDER) = Some(tx);
    let mut lua = LUA.lock().expect("LUA was poisoned!");
    let mut lua = &mut lua.0;
    info!("Loading way-cooler libraries...");
    rust_interop::register_libraries(&mut lua)?;

    let (use_config, maybe_init_file) = init_path::get_config();
    if use_config {
        match maybe_init_file {
            Ok((init_dir, mut init_file)) => {
                if init_dir.components().next().is_some() {
                    // Add the config directory to the package path.
                    let globals = lua.globals();
                    let package: rlua::Table = globals.get("package")
                        .expect("package not defined in Lua");
                    let paths: String = package.get("path")
                        .expect("package.path not defined in Lua");
                    package.set("path", paths + ";" + init_dir.join("?.lua").to_str()
                                .expect("init_dir not a valid UTF-8 string"))?;
                }
                let mut init_contents = String::new();
                init_file.read_to_string(&mut init_contents)
                    .expect("Could not read contents");
                lua.exec(init_contents.as_str(), Some("init.lua".into()))
                    .map(|_:()| info!("Read init.lua successfully"))
                    .or_else(|err| {
                        match err {
                            rlua::Error::RuntimeError(ref err) => {
                                error!("{}", err);
                            }
                            rlua::Error::CallbackError{traceback: ref err, ref cause } => {
                                error!("traceback: {}", err);
                                error!("cause: {}", *cause)
                            },
                            err => {
                                error!("init file error: {:?}", err);
                            }
                        }
                        // Keeping this an error, so that it is visible
                        // in release builds.
                        info!("Defaulting to pre-compiled init.lua");
                        *lua = rlua::Lua::new();
                        rust_interop::register_libraries(&mut lua)?;
                        lua.exec(init_path::DEFAULT_CONFIG,
                                 Some("init.lua <DEFAULT>".into()))
                        })
                    .expect("Unable to load pre-compiled init file");
            }
            Err(_) => {
                warn!("Defaulting to pre-compiled init.lua");
                let _: () = lua.exec(init_path::DEFAULT_CONFIG,
                                     Some("init.lua <DEFAULT>".into()))
                    .expect("Unable to load pre-compiled init file");
            }
        }
    }
    else {
        info!("Skipping config search");
    }

    // Only ready after loading libs
    *RUNNING.write().expect(ERR_LOCK_RUNNING) = true;
    info!("Entering main loop...");
    let _lua_handle = thread::Builder::new()
        .name("Lua thread".to_string())
        .spawn(move || main_loop(receiver));
    // Immediately update all the values that the init file set
    send(LuaQuery::UpdateRegistryFromCache)
        .expect("Could not update registry from cache");

    // Re-tile the layout tree, to make any changes appear immediantly.
    if let Ok(mut tree) = lock_tree() {
        tree.layout_active_of(ContainerType::Root)
            .unwrap_or_else(|_| {
                warn!("Lua thread could not re-tile the layout tree");
            });
        // Yeah this is silly, it's so the active border can be updated properly.
        if let Some(active_id) = tree.active_id() {
            tree.focus(active_id)
                .expect("Could not focus on the focused id");
        }
    }
    Ok(())
}

pub fn on_compositor_ready() {
    info!("Running lua on_init()");
    // Call the special init hook function that we read from the init file
    ::lua::init()
        .expect("Could not initialize lua thread!");
    send(LuaQuery::Execute(INIT_LUA_FUNC.to_owned())).err()
        .map(|error| warn!("Lua init callback returned an error: {:?}", error));
}

/// Main loop of the Lua thread:
///
/// ## Loop
/// * Wait for a message from the receiver
/// * Handle message
/// * Send response
fn main_loop(receiver: Receiver<LuaMessage>) {
    loop {
        trace!("Lua: awaiting request");
        let request = receiver.recv();
        let mut lua = LUA.lock().expect("LUA was poisoned!");
        match request {
            Err(e) => {
                error!("Lua thread: unable to receive message: {}", e);
                error!("Lua thread: now panicking!");
                *RUNNING.write().expect(ERR_LOCK_RUNNING) = false;

                panic!("Lua thread: lost contact with host, exiting!");
            }
            Ok(message) => {
                trace!("Handling a request");
                if !handle_message(message, &mut lua.0) {
                    return
                }
            }
        }
    }
}

/// Handle each LuaQuery option sent to the thread
fn handle_message(request: LuaMessage, lua: &mut rlua::Lua) -> bool {
    match request.query {
        LuaQuery::Terminate => {
            trace!("Received terminate signal");
            if let Err(error) = lua.exec::<()>(LUA_TERMINATE_CODE,
                                               Some("custom terminate code".into())) {
                warn!("Lua termination callback returned an error: {:?}", error);
                warn!("However, termination will continue");
            }
            *RUNNING.write().expect(ERR_LOCK_RUNNING) = false;
            thread_send(request.reply, LuaResponse::Pong);

            info!("Lua thread terminating!");
            return false
        },
        LuaQuery::Restart => {
            trace!("Received restart signal!");
            if let Err(error) = lua.exec::<()>(LUA_RESTART_CODE,
                                         Some("custom restart code".into())) {
                warn!("Lua restart callback returned an error: {:?}", error);
                warn!("However, Lua will be restarted");
            }
            *RUNNING.write().expect(ERR_LOCK_RUNNING) = false;
            thread_send(request.reply, LuaResponse::Pong);

            // The only real way to restart
            let _new_handle = thread::Builder::new()
                .name("Lua re-init".to_string())
                .spawn(move || {
                    init().expect("Could not init");
                    keys::init();
                });

            info!("Lua thread restarting");
            return false
        },
        LuaQuery::Execute(code) => {
            trace!("Received request to execute {}", code);

            match lua.exec::<()>(&code, None) {
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

            if let Ok(mut file) = try_file {
                let mut file_contents = String::new();
                file.read_to_string(&mut file_contents)
                    .expect("Could not read file contents");
                let result = lua.exec::<rlua::Value>(file_contents.as_str(),
                                      Some(name.as_str()));
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
                    rlua::Error::RuntimeError(format!("{:#?}", try_file.unwrap_err()));
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
            match lua.exec::<()>(&code, Some(&format!("{}", press))) {
                Err(error) => {
                    if let rlua::Error::RuntimeError(ref err) = error {
                        warn!("Error handling {}:\n {}", press, err);
                    } else {
                        warn!("Error handling {}: {:?}", press, error);
                    }
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
        LuaQuery::UpdateRegistryFromCache => {
            let lock = registry::clients_read();
            // Lua has access to everything
            let client = lock.client(Uuid::nil()).unwrap();
            let mut handle = registry::WriteHandle::new(&client);

            let mut queue = REGISTRY_QUEUE.write().expect(ERR_LOCK_QUEUE);
            for category in queue.drain(0..) {
                let globals = lua.globals();
                let registry_cache: rlua::Table = globals.get("__registry_cache")
                    .expect("__registry_cache wasn't defined");
                let category_str = lua.create_string(category.as_str());
                if let Ok(mut category_table) = registry_cache.get::<rlua::String, rlua::Table>(category_str) {
                    let cat_table = match handle.write(category.clone()) {
                        Ok(cat) => cat,
                        Err(err) => {
                            warn!("Could not lock {}: {:?}", category, err);
                            break;
                        }
                    };
                    update_values(&mut category_table, cat_table);
                }
                drop(registry_cache)
            }
            lua.exec::<()>("__registry_cache = {}", None)
                .expect("Could not clear __registry_cache");
        },
    }
    return true
}

fn update_values<'lua>(table: &mut rlua::Table<'lua>,
                    category: &mut registry::Category) {
    use rlua::Value;
    let mut keys = Vec::new();
    for entry in table.clone().pairs::<rlua::String, Value>() {
        if let Ok((key, value)) = entry {
            match value {
                rlua::Value::Table(_) => {
                    keys.push(key.clone());
                    category.insert(key.to_str().unwrap().into(),
                                    Json::Object(BTreeMap::new()));
                },
                value => {
                    match lua_to_json(value) {
                        Ok(val) => {
                            trace!("Updating {}:{:?} = {:#?}", category.name(), key, &val);
                            category.insert(key.to_str().unwrap().into(), val);
                        },
                        Err(value) => {
                            warn!("Could not translate {:?} to JSON", value);
                        }
                    }
                }
            }
        }
    }
    for key in keys {
        let inner_mapping = category.get_mut(key.to_str().unwrap())
            .expect("Could not get the value we just made")
            .as_object_mut()
            .expect("The inner value was not an object!");
        if let Ok(inner_table) = table.get::<rlua::String, rlua::Table>(key) {
            for entry in inner_table.pairs::<rlua::String, Value>() {
                if let Ok((key, value)) = entry {
                    match value {
                        rlua::Value::Table(_) => {
                            warn!("Dropping inner table {:?}", key);
                        },
                        value => {
                            if let Ok(val) = lua_to_json(value) {
                                inner_mapping.insert(key.to_str().unwrap().into(),
                                                     val);
                            }
                        }
                    }
                }
            }
        }
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
