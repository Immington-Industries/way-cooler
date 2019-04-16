//! A wrapper around a Cairo image surface.

use std::{default::Default, fs::File, io::Write};

use cairo::{Format, ImageSurface};
use glib::translate::ToGlibPtr;
use rlua::{self, LightUserData, Lua, Table, UserData, UserDataMethods, Value};
use tempfile;
use wlroots::area::{Area, Origin, Size};

use crate::common::{
    class::{self, Class},
    object::{self, Object},
    property::Property,
    signal::emit_object_signal
};
use crate::wayland_obj::{self, XdgToplevel};

#[derive(Debug)]
pub struct DrawableState {
    temp_file: File,
    wayland_shell: Option<XdgToplevel>,
    pub surface: Option<ImageSurface>,
    geo: Area,
    // TODO Use this to determine whether we draw this or not
    refreshed: bool
}

pub type Drawable<'lua> = Object<'lua, DrawableState>;

impl Default for DrawableState {
    fn default() -> Self {
        let temp_file = tempfile::tempfile().expect("Could not make a temp file for the buffer");
        DrawableState {
            temp_file,
            wayland_shell: None,
            surface: None,
            geo: Area::default(),
            refreshed: false
        }
    }
}

impl<'lua> Drawable<'lua> {
    pub fn new(lua: &Lua) -> rlua::Result<Drawable> {
        let class = class::class_setup(lua, "drawable")?;
        let builder = Drawable::allocate(lua, class)?;
        // TODO Do properly
        let table = lua.create_table()?;
        table.set("geometry", lua.create_function(geometry)?)?;
        table.set("refresh", lua.create_function(refresh)?)?;
        Ok(builder.add_to_meta(table)?.build())
    }

    pub fn get_geometry(&self) -> rlua::Result<Area> {
        let drawable = self.state()?;
        Ok(drawable.geo)
    }

    pub fn get_surface(&self) -> rlua::Result<Value<'lua>> {
        let drawable = self.state()?;
        Ok(match drawable.surface {
            None => Value::Nil,
            Some(ref image) => {
                let stash = image.to_glib_none();
                let ptr = stash.0;
                // NOTE
                // We bump the reference count because now Lua has a reference which
                // it manages via LGI.
                //
                // If there's a bug, worst case scenario there's a memory leak.
                unsafe {
                    ::cairo_sys::cairo_surface_reference(ptr);
                }
                Value::LightUserData(LightUserData(ptr as _))
            }
        })
    }

    /// Sets the geometry, and allocates a new surface.
    pub fn set_geometry(&mut self, lua: &Lua, geometry: Area) -> rlua::Result<()> {
        use rlua::Error::RuntimeError;
        let obj_clone = self.clone();
        let mut drawable = self.state_mut()?;
        let size_changed = drawable.geo != geometry;
        drawable.geo = geometry;
        if size_changed {
            drawable.refreshed = false;
            drawable.surface = None;
            drawable.wayland_shell = Some(
                wayland_obj::create_xdg_toplevel(None)
                    .expect("Could not construct an xdg toplevel for a drawable")
            );
            let size: Size = geometry.size;

            if size.width > 0 && size.height > 0 {
                let temp_file = tempfile::tempfile().expect("Could not make new temp file");
                temp_file
                    .set_len(size.width as u64 * size.height as u64 * 4)
                    .expect("Could not set file length");
                drawable.surface = Some(
                    ImageSurface::create(Format::ARgb32, size.width, size.height)
                        .map_err(|err| RuntimeError(format!("Could not allocate {:?}", err)))?
                );
                {
                    let shell = drawable.wayland_shell.as_mut().unwrap();
                    shell.set_size(size);
                    shell
                        .set_surface(&temp_file, size)
                        .map_err(|_| RuntimeError(format!("Could not set surface for drawable")))?;
                }
                drawable.temp_file = temp_file;
                emit_object_signal(lua, obj_clone.into(), "property::surface".into(), ())?;
            }
        }
        Ok(())
    }

    /// Signals that the drawable's surface was updated.
    pub fn refresh(&mut self) -> rlua::Result<()> {
        let mut drawable = self.state_mut()?;
        let drawable = &mut *drawable;
        if let Some(data) = drawable.surface.as_mut().map(get_data) {
            drawable
                .temp_file
                .write(&*data)
                .expect("Could not write data to buffer");
            drawable.temp_file.flush().expect("Could not flush buffer");
            drawable.refreshed = true;
        }
        Ok(())
    }
}

impl UserData for DrawableState {
    fn add_methods(methods: &mut UserDataMethods<Self>) {
        object::default_add_methods(methods);
    }
}

pub fn init(lua: &Lua) -> rlua::Result<Class<DrawableState>> {
    Class::<DrawableState>::builder(lua, "drawable", None)?
        .method("geometry".into(), lua.create_function(geometry)?)?
        .property(Property::new(
            "surface".into(),
            None,
            Some(lua.create_function(get_surface)?),
            None
        ))?
        .save_class("drawable")?
        .build()
}

fn get_surface<'lua>(_: &'lua Lua, drawable: Drawable<'lua>) -> rlua::Result<Value<'lua>> {
    drawable.get_surface()
}

fn geometry<'lua>(lua: &'lua Lua, drawable: Drawable<'lua>) -> rlua::Result<Table<'lua>> {
    let geometry = drawable.get_geometry()?;
    let Origin { x, y } = geometry.origin;
    let Size { width, height } = geometry.size;
    let table = lua.create_table()?;
    table.set("x", x)?;
    table.set("y", y)?;
    table.set("width", width)?;
    table.set("height", height)?;
    Ok(table)
}

fn refresh<'lua>(_: &'lua Lua, mut drawable: Drawable<'lua>) -> rlua::Result<()> {
    drawable.refresh()
}

/// Get the data associated with the ImageSurface.
fn get_data(surface: &mut ImageSurface) -> &[u8] {
    // NOTE This is safe to do because there's one thread.
    //
    // We know Lua is not modifying it because it's not running.
    use cairo_sys;
    use std::slice;
    unsafe {
        let len = surface.get_stride() as usize * surface.get_height() as usize;
        let surface = surface.to_glib_none().0;
        slice::from_raw_parts(cairo_sys::cairo_image_surface_get_data(surface as _), len)
    }
}
