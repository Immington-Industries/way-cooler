use rlua;
use std::{
    process::{Command, Stdio},
    thread
};
pub fn init(lua: rlua::Context) -> rlua::Result<()> {
    // TODO Do properly
    use crate::objects::dummy;

    let spawn = lua.create_table()?;
    spawn.set("with_shell", lua.create_function(with_shell)?)?;
    spawn.set("with_line_callback", lua.create_function(dummy)?)?;
    spawn.set("easy_async", lua.create_function(dummy)?)?;
    spawn.set("easy_async_with_shell", lua.create_function(dummy)?)?;
    spawn.set("read_lines", lua.create_function(dummy)?)?;
    spawn.set("once", lua.create_function(dummy)?)?;
    spawn.set("single_instance", lua.create_function(dummy)?)?;
    spawn.set("raise_or_spawn", lua.create_function(dummy)?)?;

    lua.globals().set("spawn", spawn)
}

fn with_shell(_: rlua::Context<'_>, command: String) -> rlua::Result<()> {
    // TODO use shell from awful.util.shell
    thread::Builder::new()
        .name(command.clone())
        .spawn(|| {
            Command::new(command)
                .stdout(Stdio::null())
                .spawn()
                .expect("Could not spawn command")
                .wait()
        })
        .expect("Unable to spawn thread");
    Ok(())
}
