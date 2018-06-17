//! TODO Fill in
use super::class::{Class, ClassBuilder};
use super::object::{Object, Objectable};
use rlua::{self, Lua, Table, ToLua, UserData, Value, AnyUserData, UserDataMethods, MetaMethod};
use std::default::Default;
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;
use compositor::{Server, View};
use wlroots::{compositor_handle, Origin, Size, Area};

pub const CLIENTS_HANDLE: &'static str = "__clients";

#[derive(Clone, Debug, Default)]
pub struct ClientState {
    pub view: Arc<View>
}

#[derive(Clone, Debug)]
pub struct Client<'lua>(Object<'lua>);

impl <'lua> Client<'lua> {
    pub fn new(lua: &Lua) -> rlua::Result<Object> {
        let class = super::class::class_setup(lua, "client")?;
        Ok(Client::allocate(lua, class)?.build())
    }

    pub fn init_client(&mut self, view: Arc<View>) -> rlua::Result<()> {
        let mut state = self.get_object_mut()?;
        state.view = view;
        Ok(())
    }
}

impl Display for ClientState {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Client: {:p}", self)
    }
}

impl<'lua> ToLua<'lua> for Client<'lua> {
    fn to_lua(self, lua: &'lua Lua) -> rlua::Result<Value<'lua>> {
        self.0.to_lua(lua)
    }
}

impl UserData for ClientState {
    fn add_methods(methods: &mut UserDataMethods<Self>) {
        methods.add_method("geometry", |lua, state, (geometry,): (Value,)| {
            match geometry {
                Value::Table(table) => {
                    let area = Area {
                        origin: Origin { x: table.get("x")?, y: table.get("y")? },
                        size: Size { width: table.get("width")?, height: table.get("height")? }
                    };

                    state.view.move_resize(area);

                    Ok(Value::Nil.to_lua(lua))
                },
                _ => {
                    let table = lua.create_table()?;
                    let geometry = state.view.geometry();
                    let Origin { x, y } = *state.view.origin.lock().unwrap();
                    table.set("x", geometry.origin.x + x)?;
                    table.set("y", geometry.origin.y + y)?;
                    table.set("width", geometry.size.width)?;
                    table.set("height", geometry.size.height)?;
                    Ok(table.to_lua(lua))
                }
            }
        });

        methods.add_meta_function(MetaMethod::Index, |lua, (class, index): (ClientState, Value)| {
            match index {
                Value::String(value) => {
                    match value.to_str() {
                        Ok("title") => {
                            Ok(class.view.title().to_lua(lua))
                        },
                        _ => Ok(Value::Nil.to_lua(lua))
                    }
                },
                _ => Ok(Value::Nil.to_lua(lua))
            }
        });
    }
}

pub fn init<'lua>(lua: &'lua Lua, server: &Server) -> rlua::Result<Class<'lua>> {
    let clients: &mut Vec<Client> = &mut Vec::default();

    for view in &server.views {
        let mut client = Client::cast(Client::new(lua)?)?;
        client.init_client(view.clone())?;
        clients.push(client);
    }

    lua.set_named_registry_value(CLIENTS_HANDLE, clients.clone().to_lua(lua)?)?;

    method_setup(lua, Class::builder(lua, "client", None)?)?.save_class("client")?
                                                            .build()
}

fn method_setup<'lua>(lua: &'lua Lua,
                      builder: ClassBuilder<'lua>)
                      -> rlua::Result<ClassBuilder<'lua>> {
    // TODO Do properly
    use super::dummy;
    builder.method("connect_signal".into(), lua.create_function(dummy)?)?
           .method("get".into(), lua.create_function(get_clients)?)
}

impl_objectable!(Client, ClientState);

fn get_clients<'lua>(lua: &'lua Lua, _: rlua::Value) -> rlua::Result<Table<'lua>> {
    let clients: Vec<Client> = lua.named_registry_value::<Vec<AnyUserData>>(CLIENTS_HANDLE)?
                                      .into_iter()
                                      .map(|obj| Client::cast(obj.into()).unwrap())
                                      .collect();
    let table = lua.create_table()?;

    for (i, client) in clients.iter().enumerate() {
        let client = client.clone().to_lua(lua)?;
        table.set(i + 1, client)?;
    }

    Ok(table)
}

pub fn notify_client_add<'lua>(lua: &'lua Lua, view: Arc<View>) -> rlua::Result<()> {
    let mut clients: Vec<Client> = lua.named_registry_value::<Vec<AnyUserData>>(CLIENTS_HANDLE)?
                                      .into_iter()
                                      .map(|obj| Client::cast(obj.into()).unwrap())
                                      .collect();
    let mut client = Client::cast(Client::new(lua)?)?;
    client.init_client(view.clone())?;
    clients.push(client);

    lua.set_named_registry_value(CLIENTS_HANDLE, clients.clone().to_lua(lua)?)?;

    Ok(())
}

pub fn notify_client_remove<'lua>(lua: &'lua Lua, view: Arc<View>) -> rlua::Result<()> {
    let mut clients: Vec<Client> = lua.named_registry_value::<Vec<AnyUserData>>(CLIENTS_HANDLE)?
                                      .into_iter()
                                      .map(|obj| Client::cast(obj.into()).unwrap())
                                      .collect();

    if let Some(idx) = clients.iter().position(|client| client.state().unwrap().view == view) {
        clients.remove(idx);
    }

    lua.set_named_registry_value(CLIENTS_HANDLE, clients.clone().to_lua(lua)?)?;

    Ok(())
}
