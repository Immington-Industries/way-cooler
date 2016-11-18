//! Lua thread test.
//! Spawns a lua thread!

use std::time::Duration;
use std::thread;

use hlua::any::AnyLuaValue;
use hlua::{Lua, LuaError, LuaTable};

use super::*;

fn wait_for_thread() {
    for _ in 0..20 {
        if !running() {
            thread::sleep(Duration::from_millis(200));
        }
        else { return; }
    }
    panic!("Thread didn't run!");
}

#[test]
fn activate_thread() {
    super::init();
}

#[test]
fn thread_exec_okay() {
    wait_for_thread();

    let hello_receiver = send(LuaQuery::Execute(
        "hello = 'hello world'".to_string()))
        .expect("Unable to send hello world");
    let hello_result = hello_receiver.recv()
        .expect("Unable to receive hello world");
    assert!(hello_result.is_ok());
    assert_eq!(hello_result, LuaResponse::Pong);

    let assert_receiver = send(LuaQuery::Execute(
        "assert(hello == 'hello world')".to_string()))
        .expect("Unble to send hello assertion");
    let assert_result = assert_receiver.recv()
        .expect("Unabel to receive hello assertion");
    assert!(hello_result.is_ok());
    assert_eq!(assert_result, LuaResponse::Pong);
}

#[test]
fn thread_exec_err() {
    wait_for_thread();

    let assert_receiver = send(LuaQuery::Execute(
        "assert(true == false, 'Logic works')".to_string()))
        .expect("send assertion error");
    let assert_result = assert_receiver.recv()
        .expect("receive assertion error result");
    assert!(assert_result.is_err(), "expected error from syntax error");
    if let LuaResponse::Error(err) = assert_result {
        if let LuaError::ExecutionError(lua_err) = err {
            assert!(lua_err.contains("Logic works"));
        }
        else {
            panic!("thread_exec_err result was not an ExecutionError: {:?}",
                   err);
        }
    }
    else {
        panic!("thread_exec_err result was not an error: {:?}", assert_result);
    }

    let syn_err_rx = send(LuaQuery::Execute(
        "local variable_err = 'sequence\\y'".to_string()))
        .expect("send syntax error");
    let syn_err_result = syn_err_rx.recv()
        .expect("receive assertion error result");
    assert!(syn_err_result.is_err(), "expected error from syntax error");
    if let LuaResponse::Error(lua_err) = syn_err_result {
        if let LuaError::SyntaxError(s_err) = lua_err {
            assert!(s_err.contains("escape sequence"), "Got wrong error type");
        }
        else {
            panic!("Wrong type of lua error!");
        }
    }
    else {
        panic!("Got the wrong LuaResponse type!");
    }
}

#[test]
fn thread_exec_file_ok() {
    wait_for_thread();

    let file_receiver = send(LuaQuery::ExecFile(
        "lib/test/lua-exec-file.lua".to_string())).unwrap();
    let file_result = file_receiver.recv().unwrap();
    if let LuaResponse::Error(err) = file_result {
        if let LuaError::ReadError(ioerr) = err {
            panic!("Lua thread was unable to execute file: {:?}", ioerr);
        }
        else {
            panic!("Unexpected error executing file: {:?}", err);
        }
    }
    assert!(file_result.is_ok());
    assert_eq!(file_result, LuaResponse::Pong);

    // Print the method from the file
    let test_receiver = send(LuaQuery::Execute(
        "confirm_file()".to_string())).unwrap();
    let test_result = test_receiver.recv().unwrap();
    assert!(test_result.is_ok());
    assert!(test_result == LuaResponse::Pong);
}

#[test]
fn thread_exec_file_err() {
    wait_for_thread();

    let run_receiver = send(LuaQuery::ExecFile(
        "lib/test/lua-bad-assert.lua".to_string()))
        .expect("Unable to request lua-bad-assert.lua");
    let run_result = run_receiver.recv()
        .expect("Unable to receive lua-bad-assert.lua");
    assert!(run_result.is_err());
    match run_result {
        LuaResponse::Error(err) => {
            match err {
                LuaError::ExecutionError(ex) => {
                    assert!(ex.contains("Dude, 1 is totally equal to 0"));
                },
                _ => panic!("Got wrong hlua error type: {:?}", err)
            }
        },
        _ => panic!("Got wrong LuaResponse type: {:?}", run_result)
    }

    let syntax_receiver = send(LuaQuery::ExecFile(
        "lib/test/lua-syntax-err.txt".to_string()))
        .expect("Unable to request lua-syntax-err.txt");
    let syntax_result = syntax_receiver.recv()
        .expect("Unable to receive lua-syntax-err.txt");
    assert!(syntax_result.is_err());
    match syntax_result {
        LuaResponse::Error(err) => {
            match err {
                LuaError::SyntaxError(serr) => {
                    assert!(serr.contains("expected"));
                },
                _ => panic!("Got wrong hlua error type: {:?}", err)
            }
        }
        _ => panic!("Got wrong LuaResponse type: {:?}", syntax_result)
    }
}

#[test]
fn test_rust_exec() {
    wait_for_thread();
    let rust_receiver = send(LuaQuery::ExecRust(rust_lua_fn))
        .expect("Unable to request rust func exec");
    let rust_result = rust_receiver.recv()
        .expect("Unable to receive rust func exec");
    assert!(rust_result.is_ok());

    match rust_result {
        LuaResponse::Variable(Some(var)) => {
            match var {
                AnyLuaValue::LuaBoolean(b) => {
                    if !b {
                        panic!("Rust function failed!");
                    }
                },
                _ => panic!("Rust function returned wrong AnyLuaValue")
            }
        },
        _ => panic!("Got wrong LuaResponse from Rust function!")
    }
}

fn rust_lua_fn(lua: &mut Lua) -> AnyLuaValue {
    {
        let mut foo = lua.empty_array("foo");
        foo.set("bar", 12.0);
    }
    let maybe_foo = lua.get::<LuaTable<_>, _>("foo");
    assert!(maybe_foo.is_some());
    let mut foo = maybe_foo
        .expect("asserted maybe_foo.is_some()");
    assert!(foo.get::<f64, _>("bar").is_some());
    AnyLuaValue::LuaBoolean(true)
}

