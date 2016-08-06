//! IPC for way-cooler

use std::thread;
use std::env;
use std::path::{Path, PathBuf};
use std::fs;

use nix::unistd::getuid;
use unix_socket::UnixListener;

mod channel;
mod command;
mod event;

#[cfg(test)]
mod tests;

/// Versions are incremented.
pub const VERSION: u64 = 0u64; // Increment to 1 on release.

/// Socket over which synchronous communication is made with clients.
pub const COMMAND_SOCKET: &'static str = "command";
/// Socket over which events are sent to clients.
pub const EVENT_SOCKET: &'static str = "event";
/// Folder in which sockets are created
pub const PATH_VAR: &'static str = "WAY_COOLER_SOCKET_FOLDER";

pub struct Ipc {
    id: u32,
    base_path: PathBuf,
    socket_handles: Vec<thread::JoinHandle<()>>
}

impl Ipc {
    /// Starts up the command sockets in a unique folder path in /var/run
    ///
    /// # Panics
    /// Panics if the unique socket directory in /var/run cannot be created.
    pub fn new() -> Self {
        let mut id = unique_ish_id();
        let mut path = Ipc::socket_base_path();
        path.push("way-cooler");
        path.push(id.to_string());
        while path.exists() {
            path.pop();
            id = unique_ish_id();
            path.push(id.to_string());
        }
        // How can we handle not having a socket?
        // In the future, we could log and continue.
        // We could have a config option to not create/create-if-possible
        fs::create_dir_all(path.clone()).expect("Unable to create socket folder");
        let mut ipc = Ipc {id: id,
             base_path: path,
             socket_handles: vec!()
        };
        ipc.init_sockets();
        ipc
    }

    #[allow(dead_code)]
    /// Gets the unique id to make the path unique
    pub fn get_id(&self) -> u32 {
        self.id
    }

    #[allow(dead_code)]
    /// Gets the socket path used for IPC
    pub fn get_socket_path(&self) -> &Path {
        self.base_path.as_path()
    }

    /// Gets the base path which the socket uses
    fn socket_base_path() -> PathBuf {
        if let Ok(path) = env::var("XDG_RUNTIME_DIR") {
            return PathBuf::from(path)
        }
        let user_id = getuid();
        return Path::new("/var/run/user").join(user_id.to_string());
    }

    /// Initializes the command and event sockets for the ipc
    ///
    /// # Panics
    /// Panics if the sockets or their threads could not be created
    fn init_sockets(&mut self) {
        info!("Starting IPC with unique ID {}", self.id);

        let command_socket = UnixListener::bind(self.base_path.join(COMMAND_SOCKET))
            .expect("Unable to open command socket!");

        let event_socket = UnixListener::bind(self.base_path.join(EVENT_SOCKET))
            .expect("Unable to open event socket!");

        debug!("IPC initialized, now listening for clients.");

        let command_handle = thread::Builder::new()
            .name("Command socket listener".to_string())
            .spawn(move || { command_thread(command_socket) })
            .expect("Could not make command thread");

        let event_handle = thread::Builder::new()
            .name("Event socket listener".to_string())
            .spawn(move || { event_thread(event_socket) })
        .expect("Could not make event thread");

        self.socket_handles.push(command_handle);
        self.socket_handles.push(event_handle);

        env::set_var(PATH_VAR, self.base_path.clone());
    }

}

impl Drop for Ipc {
    fn drop(&mut self) {
        if let Some(path) = self.base_path.to_str() {
            trace!("Removing folder {}", path);
        }
        if let Err(ioerr) = fs::remove_dir_all(self.base_path.clone()) {
            error!("Unable to delete path: {}", ioerr);
        }
        info!("Cleaned up IPC folder");
    }
}

/// We need random folder names to place sockets in, but they don't need
/// to be _that_ random.
fn unique_ish_id() -> u32 {
    use std::hash::{Hash, Hasher, SipHasher};
    use std::time::Instant;

    // If you shift a u64 hash right by this you get a "checksum",
    // a number which retains some of the entropy of the hash but
    // is small enough to fit a more comfortable file name.
    const MAGIC_SHIFT_NUMBER: u64 = 0b110000;

    // Instant doesn't implement hash, and it's supposed to be an opaque
    // struct, but it does implement debug...
    let now = Instant::now();
    let mut hasher = SipHasher::new();
    format!("{:?}", now).hash(&mut hasher);
    (hasher.finish() >> MAGIC_SHIFT_NUMBER) as u32
}

/// Initialize the IPC socket.
pub fn init() -> Ipc{
    Ipc::new()
}

fn command_thread(socket: UnixListener) {
    for stream in socket.incoming() {
        trace!("Sever: new connection: {:?}", stream);
        match stream {
            Ok(mut stream) => {
                info!("Command: connected to {:?}", stream);
                let _handle = thread::Builder::new()
                    .name("IPC server helper".to_string())
                    .spawn(move || command::listen_loop(&mut stream));
            },
            Err(err) => {
                info!("Error receiving a stream: {}", err);
            }
        }
    }
}

fn event_thread(socket: UnixListener) {
    for stream in socket.incoming() {
        trace!("Event: new connection: {:?}", stream);
        match stream {
            Ok(mut stream) => {
                info!("Event: connected to {:?}", stream);
                let _handle = thread::Builder::new()
                    .name("IPC evemt helper".to_string())
                    .spawn(move || event::thread(&mut stream));
            },
            Err(err) => {
                info!("Error receiving a stream: {}", err);
            }
        }
    }
}
