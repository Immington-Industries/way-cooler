//! Tests for IPC

//use rustc_serialize::json::Json;

// Unit tests for methods in channel.rs
mod channel;
// Unit tests for individual packets in command.rs
mod commands;

// Integration tests for running the command socket
mod command_loop;
