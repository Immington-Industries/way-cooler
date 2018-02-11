//! Lua thread test.
//! Spawns a lua thread!

use rlua::{self, Lua};

use super::*;

#[test]
fn activate_thread() {
    super::init();
}

#[test]
fn thread_exec_okay() {
    let hello_receiver = send(LuaQuery::Execute(
        "hello = 'hello world'".to_string()));
    let hello_result = hello_receiver.recv()
        .expect("Unable to receive hello world");
    assert!(hello_result.is_ok());
    match hello_result {
        LuaResponse::Pong => {},
        _ => panic!("Wrong type")
    }

    let assert_receiver = send(LuaQuery::Execute(
        "assert(hello == 'hello world')".to_string()));
    let assert_result = assert_receiver.recv()
        .expect("Unabel to receive hello assertion");
    assert!(hello_result.is_ok());
    match assert_result {
        LuaResponse::Pong => {},
        _ => panic!("Wrong response")
    }
}

#[test]
fn thread_exec_err() {
    let assert_receiver = send(LuaQuery::Execute(
        "assert(true == false, 'Logic works')".to_string()));
    let assert_result = assert_receiver.recv()
        .expect("receive assertion error result");
    assert!(assert_result.is_err(), "expected error from syntax error");
    if let LuaResponse::Error(err) = assert_result {
        if let rlua::Error::RuntimeError(lua_err) = err {
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
        "local variable_err = 'sequence\\y'".to_string()));
    let syn_err_result = syn_err_rx.recv()
        .expect("receive assertion error result");
    assert!(syn_err_result.is_err(), "expected error from syntax error");
    if let LuaResponse::Error(lua_err) = syn_err_result {
        if let rlua::Error::SyntaxError{message, .. } = lua_err {
            assert!(message.contains("escape sequence"), "Got wrong error type");
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
    let file_receiver = send(LuaQuery::ExecFile(
        "lib/test/lua-exec-file.lua".to_string()));
    let file_result = file_receiver.recv().unwrap();
    if let LuaResponse::Error(err) = file_result {
        if let rlua::Error::RuntimeError(ioerr) = err {
            panic!("Lua thread was unable to execute file: {:?}", ioerr);
        }
        else {
            panic!("Unexpected error executing file: {:?}", err);
        }
    }
    assert!(file_result.is_ok());
    match file_result {
        LuaResponse::Pong => {},
        _ => panic!("Wrong response")
    }

    // Print the method from the file
    let test_receiver = send(LuaQuery::Execute(
        "confirm_file()".to_string()));
    let test_result = test_receiver.recv().unwrap();
    assert!(test_result.is_ok());
    match test_result {
        LuaResponse::Pong => {},
        _ => panic!("Wrong response")
    }
}

#[test]
fn thread_exec_file_err() {
    let run_receiver = send(LuaQuery::ExecFile(
        "lib/test/lua-bad-assert.lua".to_string()));
    let run_result = run_receiver.recv()
        .expect("Unable to receive lua-bad-assert.lua");
    assert!(run_result.is_err());
    match run_result {
        LuaResponse::Error(err) => {
            match err {
                rlua::Error::RuntimeError(ex) => {
                    assert!(ex.contains("Dude, 1 is totally equal to 0"));
                },
                _ => panic!("Got wrong hlua error type: {:?}", err)
            }
        },
        _ => panic!("Got wrong LuaResponse type: {:?}", run_result)
    }

    let syntax_receiver = send(LuaQuery::ExecFile(
        "lib/test/lua-syntax-err.txt".to_string()));
    let syntax_result = syntax_receiver.recv()
        .expect("Unable to receive lua-syntax-err.txt");
    assert!(syntax_result.is_err());
    match syntax_result {
        LuaResponse::Error(err) => {
            match err {
                rlua::Error::SyntaxError{message, .. } => {
                    assert!(message.contains("expected"));
                },
                _ => panic!("Got wrong hlua error type: {:?}", err)
            }
        }
        _ => panic!("Got wrong LuaResponse type: {:?}", syntax_result)
    }
}

#[test]
fn test_rust_exec() {
    let rust_receiver = send(LuaQuery::ExecRust(rust_lua_fn));
    let rust_result = rust_receiver.recv()
        .expect("Unable to receive rust func exec");
    assert!(rust_result.is_ok());

    match rust_result {
        LuaResponse::Variable(Some(var)) => {
            match var {
                rlua::Value::Boolean(b) => {
                    if !b {
                        panic!("Rust function failed!");
                    }
                },
                _ => panic!("Rust function returned wrong Lua value")
            }
        },
        _ => panic!("Got wrong LuaResponse from Rust function!")
    }
}

fn rust_lua_fn(lua: &Lua) -> rlua::Value<'static> {
    {
        let globals = lua.globals();
        let foo = lua.create_table().unwrap();
        foo.set("bar", 12.0).unwrap();
        globals.set::<String, rlua::Table>("foo".into(), foo).unwrap();
    }
    let globals = lua.globals();
    let maybe_foo = globals.get::<String, rlua::Table>("foo".into());
    assert!(maybe_foo.is_ok());
    let foo = maybe_foo
        .expect("asserted maybe_foo.is_some()");
    assert!(foo.get::<String, f64>("bar".into()).is_ok());
    rlua::Value::Boolean(true)
}
