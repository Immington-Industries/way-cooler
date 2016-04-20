//! Lua thread test.
//! Spawns a lua thread!

use std::time::Duration;
use std::thread;

use super::*;

#[test]
fn big_lua_test() {
    println!("Testing lua thread...");
    super::init();
    thread::sleep(Duration::from_secs(5));
    test_variable();

    try_send(LuaQuery::Terminate).unwrap();
    thread::sleep(Duration::from_millis(500u64));
    assert!(!thread_running())
}

fn test_variable() {
    try_send(LuaQuery::Execute("hello = 'hello world!'".to_string()))
        .unwrap();

    try_send(LuaQuery::Execute("assert(hello == 'hello world!')".to_string()))
        .unwrap();
}

fn test_bad_code() {
    
}
