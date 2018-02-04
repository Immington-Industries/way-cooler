//! TODO Fill in

use cairo::{Format, ImageSurface};
use glib::translate::ToGlibPtr;
use rustwlc::Geometry;
use std::fmt::{self, Display, Formatter};
use std::default::Default;
use std::rc::Rc;
use rlua::{self, Table, Lua, UserData, ToLua, Value, LightUserData};
use super::object::{Object, Objectable};
use super::class::{self, Class};
use super::property::Property;

#[derive(Clone, Debug)]
pub struct DrawableState {
    pub surface: Option<ImageSurface>,
    geo: Geometry,
    // TODO Use this to determine whether we draw this or not
    refreshed: bool,
}

pub struct Drawable<'lua>(Table<'lua>);

impl_objectable!(Drawable, DrawableState);

impl Default for DrawableState {
    fn default() -> Self {
        DrawableState {
            surface: None,
            geo: Geometry::zero(),
            refreshed: false
        }
    }
}

impl <'lua> Drawable<'lua> {
    pub fn new(lua: &Lua) -> rlua::Result<Object> {
        let class = class::class_setup(lua, "drawable")?;
        let builder = Drawable::allocate(lua, class)?;
        // TODO Do properly
        let table = lua.create_table();
        table.set("geometry", lua.create_function(geometry))?;
        table.set("refresh", lua.create_function(refresh))?;
        Ok(builder.add_to_meta(table)?.build())
    }

    pub fn get_geometry(&self) -> rlua::Result<Geometry> {
        let drawable = self.state()?;
        Ok(drawable.geo)
    }

    pub fn get_surface(&self) -> rlua::Result<Value<'lua>> {
        let drawable = self.state()?;
        Ok(match drawable.surface {
            None => Value::Nil,
            Some(ref surface) => {
                let stash = surface.to_glib_none();
                let ptr = stash.0 as _;
                // So that it lives _forever_ heheheh.
                ::std::mem::forget(stash);
                Value::LightUserData(LightUserData(ptr))
            }
        })
    }

    /// Sets the geometry, and allocates a new surface.
    pub fn set_geometry(&mut self, geometry: Geometry) -> rlua::Result<()> {
        use rlua::Error::RuntimeError;
        let mut drawable = self.state()?;
        let size_changed = drawable.geo != geometry;
        drawable.geo = geometry;
        if size_changed {
            drawable.surface = None;
            drawable.refreshed = false;
            let size = geometry.size;
            if size.w > 0 && size.h > 0 {
                drawable.surface = Some(ImageSurface::create(Format::ARgb32,
                                                        size.w as i32,
                                                        size.h as i32)
                    .map_err(|err| RuntimeError(format!("Could not allocate {:?}", err)))?);
                // TODO emity property::surface
            }
        }
        self.set_state(drawable)
    }

    /// Signals that the drawable's surface was updated.
    pub fn refresh(&mut self) -> rlua::Result<()> {
        let mut drawable = self.state()?;
        drawable.refreshed = true;
        self.set_state(drawable)
    }
}

impl Display for DrawableState {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Drawable: {:p}", self)
    }
}

impl <'lua> ToLua<'lua> for Drawable<'lua> {
    fn to_lua(self, lua: &'lua Lua) -> rlua::Result<Value<'lua>> {
        self.0.to_lua(lua)
    }
}

impl UserData for DrawableState {}

pub fn init(lua: &Lua) -> rlua::Result<Class> {
    Class::builder(lua, "drawable", Some(Rc::new(Drawable::new)), None, None)?
        .method("geometry".into(), lua.create_function(geometry))?
        .property(Property::new("surface".into(),
                                None,
                                Some(lua.create_function(get_surface)),
                                None))?
        .save_class("drawable")?
        .build()
}


fn get_surface<'lua>(_: &'lua Lua, table: Table<'lua>) -> rlua::Result<Value<'lua>> {
    let drawable = Drawable::cast(table.clone().into())?;
    drawable.get_surface()
}

fn geometry<'lua>(lua: &'lua Lua, table: Table<'lua>) -> rlua::Result<Table<'lua>> {
    use rustwlc::{Point, Size};
    let drawable = Drawable::cast(table.into())?;
    let geometry = drawable.get_geometry()?;
    let Point { x, y } = geometry.origin;
    let Size { w, h } = geometry.size;
    let table = lua.create_table();
    table.set("x", x)?;
    table.set("y", y)?;
    table.set("width", w)?;
    table.set("height", h)?;
    Ok(table)
}

fn refresh<'lua>(_: &'lua Lua, table: Table<'lua>) -> rlua::Result<()> {
    Drawable::cast(table.into())?.refresh()
}
