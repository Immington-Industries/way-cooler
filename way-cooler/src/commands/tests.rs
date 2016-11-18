//! Tests on the command API
use std::sync::Arc;
use std::collections::HashMap;

use commands::{self, ComMap};

// Commands for the commands tests

pub fn command_map() -> ComMap {
    let mut map: ComMap = HashMap::new();
    map.insert("command".to_string(), Arc::new(command));
    map.insert("panic_command".to_string(), Arc::new(panic_command));
    map
}

#[test]
pub fn command() {
    println!("command being run!");
}

pub fn panic_command() {
    panic!("panic_command panic")
}

#[test]
fn add_commands() {
    // Command
    assert!(commands::set("new_command".to_string(), Arc::new(command))
        .is_none(), "New command was duplicate!");

    assert!(commands::get("new_command").is_some(),
            "Unable to add command")
}

#[test]
fn get_command() {
    assert!(commands::get("command").is_some(), "Could not get command!");
}

#[test]
fn run_command() {
    let command = commands::get("command")
        .expect("Command not found");
    command();
}

#[test]
#[should_panic(expected = "command panic")]
fn run_panic_command() {
    let command = commands::get("panic_command")
        .expect("Command not found");
    command();
}
