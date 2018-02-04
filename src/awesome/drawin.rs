// NOTE need to store the drawable in lua, because it's a reference to a drawable a lua object


use std::default::Default;
use std::fmt::{self, Display, Formatter};
use std::rc::Rc;
use rustwlc::{Geometry, Point, Size};
use rlua::{self, Table, Lua, UserData, ToLua, Value};
use rlua::prelude::LuaInteger;
use super::drawable::Drawable;
use super::property::Property;

use super::class::{self, Class, ClassBuilder};
use super::object::{Object, Objectable, ObjectBuilder};

#[derive(Clone, Debug)]
pub struct DrawinState {
    // Note that the drawable is stored in Lua.
    // TODO WINDOW_OBJECT_HEADER??
    ontop: bool,
    visible: bool,
    cursor: String,
    geometry: Geometry,
    geometry_dirty: bool
}

#[derive(Clone, Debug)]
pub struct Drawin<'lua>(Table<'lua>);

impl UserData for DrawinState {}

impl Default for DrawinState {
    fn default() -> Self {
        DrawinState {
            ontop: false,
            visible: false,
            cursor: String::default(),
            geometry: Geometry::zero(),
            geometry_dirty: false
        }
    }
}

impl Display for DrawinState {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Drawin: {:p}", self)
    }
}

impl <'lua> Drawin<'lua> {
    fn new(lua: &Lua) -> rlua::Result<Object> {
        let class = class::class_setup(lua, "drawin")?;
        Ok(object_setup(lua, Drawin::allocate(lua, class)?)?.build())
    }

    fn update_drawing(&mut self) -> rlua::Result<()> {
        let state = self.state()?;
        let table = &self.0;
        let mut drawable = Drawable::cast(table.get::<_, Table>("drawable")?.into())?;
        drawable.set_geometry(state.geometry)?;
        table.raw_set::<_, Table>("drawable", drawable.get_table())?;
        Ok(())
    }

    fn get_visible(&mut self) -> rlua::Result<bool> {
        let drawin = self.state()?;
        Ok(drawin.visible)
    }

    fn set_visible(&mut self, val: bool) -> rlua::Result<()> {
        let mut drawin = self.state()?;
        drawin.visible = val;
        self.map()?;
        self.set_state(drawin)
    }

    fn map(&mut self) -> rlua::Result<()> {
        // TODO other things
        self.update_drawing()
    }

    fn get_geometry(&self) -> rlua::Result<Geometry> {
        Ok(self.state()?.geometry)
    }

    fn resize(&mut self, geometry: Geometry) -> rlua::Result<()> {
        let mut state = self.state()?;
        let old_geometry = state.geometry;
        state.geometry = geometry;
        if state.geometry.size.w <= 0 {
            state.geometry.size.w = old_geometry.size.w;
        }
        if state.geometry.size.h <= 0 {
            state.geometry.size.h = old_geometry.size.h
        }
        state.geometry_dirty = true;
        self.update_drawing()?;
        // TODO emit signals
        // TODO update screen workareas like in awesome? Might not be necessary
        self.set_state(state)
    }
}

impl <'lua> ToLua<'lua> for Drawin<'lua> {
    fn to_lua(self, lua: &'lua Lua) -> rlua::Result<Value<'lua>> {
        self.0.to_lua(lua)
    }
}

impl_objectable!(Drawin, DrawinState);

pub fn init(lua: &Lua) -> rlua::Result<Class> {
    property_setup(lua, method_setup(lua, Class::builder(lua, "drawin", Some(Rc::new(Drawin::new)), None, None)?)?)?
        .save_class("drawin")?
        .build()
}

fn method_setup<'lua>(lua: &'lua Lua, builder: ClassBuilder<'lua>) -> rlua::Result<ClassBuilder<'lua>> {
    // TODO Do properly
    use super::dummy;
    builder.method("connect_signal".into(), lua.create_function(dummy))?
            // TODO This should be adding properties, e.g like luaA_class_new
           .method("__call".into(), lua.create_function(|lua, _: Value| Drawin::new(lua)))
}

fn property_setup<'lua>(lua: &'lua Lua, builder: ClassBuilder<'lua>) -> rlua::Result<ClassBuilder<'lua>> {
    builder
        .property(Property::new("x".into(),
                                Some(lua.create_function(set_x)),
                                Some(lua.create_function(get_x)),
                                Some(lua.create_function(set_x))))?
        .property(Property::new("y".into(),
                                Some(lua.create_function(set_y)),
                                Some(lua.create_function(get_y)),
                                Some(lua.create_function(set_y))))?
        .property(Property::new("width".into(),
                                Some(lua.create_function(set_width)),
                                Some(lua.create_function(get_width)),
                                Some(lua.create_function(set_width))))?
        .property(Property::new("height".into(),
                                Some(lua.create_function(set_height)),
                                Some(lua.create_function(get_height)),
                                Some(lua.create_function(set_height))))?
        .property(Property::new("visible".into(),
                                Some(lua.create_function(set_visible)),
                                Some(lua.create_function(get_visible)),
                                Some(lua.create_function(set_visible))))
}

fn object_setup<'lua>(lua: &'lua Lua, builder: ObjectBuilder<'lua>) -> rlua::Result<ObjectBuilder<'lua>> {
    // TODO Do properly
    let table = lua.create_table();
    let drawable_table = Drawable::new(lua)?.to_lua(lua)?;
    table.set("drawable", drawable_table)?;
    table.set("geometry", lua.create_function(drawin_geometry))?;
    table.set("struts", lua.create_function(drawin_struts))?;
    builder.add_to_meta(table)
}

fn set_visible<'lua>(_: &'lua Lua, (table, visible): (Table<'lua>, bool))
                     -> rlua::Result<()> {
    let mut drawin = Drawin::cast(table.into())?;
    drawin.set_visible(visible)
    // TODO signal
}

fn get_visible<'lua>(_: &'lua Lua, table: Table<'lua>) -> rlua::Result<bool> {
    let mut drawin = Drawin::cast(table.into())?;
    drawin.get_visible()
    // TODO signal
}

fn drawin_geometry<'lua>(lua: &'lua Lua, (drawin, geometry): (Table<'lua>, Option<Table<'lua>>)) -> rlua::Result<Table<'lua>> {
    let mut drawin = Drawin::cast(drawin.into())?;
    if let Some(geometry) = geometry {
        let w = geometry.get::<_, i32>("width")?;
        let h = geometry.get::<_, i32>("height")?;
        let x = geometry.get::<_, i32>("x")?;
        let y = geometry.get::<_, i32>("y")?;
        if x > 0 && y > 0 {
            let geo = Geometry {
                origin: Point { x, y },
                size: Size { w: w as u32, h: h as u32 }
            };
            drawin.resize(geo)?;
        }
    }
    let new_geo = drawin.get_geometry()?;
    let Size { w, h } = new_geo.size;
    let Point { x, y } = new_geo.origin;
    let res = lua.create_table();
    res.set("x", x)?;
    res.set("y", y)?;
    res.set("height", h)?;
    res.set("width", w)?;
    Ok(res)
}

fn get_x<'lua>(_: &'lua Lua, drawin: Table<'lua>) -> rlua::Result<LuaInteger> {
    let drawin = Drawin::cast(drawin.into())?;
    let Point { x, .. } = drawin.get_geometry()?.origin;
    Ok(x as LuaInteger)
}

fn set_x<'lua>(_: &'lua Lua, (drawin, x): (Table<'lua>, LuaInteger)) -> rlua::Result<()> {
    let mut drawin = Drawin::cast(drawin.into())?;
    let mut geo = drawin.get_geometry()?;
    geo.origin.x = x as i32;
    drawin.resize(geo)?;
    Ok(())
}

fn get_y<'lua>(_: &'lua Lua, drawin: Table<'lua>) -> rlua::Result<LuaInteger> {
    let drawin = Drawin::cast(drawin.into())?;
    let Point { y, .. } = drawin.get_geometry()?.origin;
    Ok(y as LuaInteger)
}

fn set_y<'lua>(_: &'lua Lua, (drawin, y): (Table<'lua>, LuaInteger)) -> rlua::Result<()> {
    let mut drawin = Drawin::cast(drawin.into())?;
    let mut geo = drawin.get_geometry()?;
    geo.origin.y = y as i32;
    drawin.resize(geo)?;
    Ok(())
}

fn get_width<'lua>(_: &'lua Lua, drawin: Table<'lua>) -> rlua::Result<LuaInteger> {
    let drawin = Drawin::cast(drawin.into())?;
    let Size { w, .. } = drawin.get_geometry()?.size;
    Ok(w as LuaInteger)
}

fn set_width<'lua>(_: &'lua Lua, (drawin, width): (Table<'lua>, LuaInteger)) -> rlua::Result<()> {
    let mut drawin = Drawin::cast(drawin.into())?;
    let mut geo = drawin.get_geometry()?;
    if width > 0 {
        geo.size.w = width as u32;
        drawin.resize(geo)?;
    }
    Ok(())
}

fn get_height<'lua>(_: &'lua Lua, drawin: Table<'lua>) -> rlua::Result<LuaInteger> {
    let drawin = Drawin::cast(drawin.into())?;
    let Size { h, .. } = drawin.get_geometry()?.size;
    Ok(h as LuaInteger)
}

fn set_height<'lua>(_: &'lua Lua, (drawin, height): (Table<'lua>, LuaInteger)) -> rlua::Result<()> {
    let mut drawin = Drawin::cast(drawin.into())?;
    let mut geo = drawin.get_geometry()?;
    if height > 0 {
        geo.size.h = height as u32;
        drawin.resize(geo)?;
    }
    Ok(())
}

fn drawin_struts<'lua>(lua: &'lua Lua, _drawin: Table<'lua>) -> rlua::Result<Table<'lua>> {
    // TODO: Implement this properly. Struts means this drawin reserves some space on the screen
    // that it is visible on, shrinking the workarea in the specified directions.
    let res = lua.create_table();
    res.set("left", 0)?;
    res.set("right", 0)?;
    res.set("top", 0)?;
    res.set("bottom", 0)?;
    Ok(res)
}
