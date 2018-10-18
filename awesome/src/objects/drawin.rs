//! A wrapper around a drawable. This controls all the other state about
//! the surface, such as the cursor used or where it on the screen.

// NOTE need to store the drawable in lua, because it's a reference to a
// drawable a lua object
use std::cell::RefMut;
use std::fmt::{self, Display, Formatter};

use cairo::ImageSurface;
use rlua::prelude::LuaInteger;
use rlua::{self, AnyUserData, Lua, Table, ToLua, UserData, UserDataMethods, Value};
use wlroots::{Area, Origin, Size, Texture};

use common::{
    class::{self, Class, ClassBuilder},
    object::{self, Object, ObjectBuilder, Objectable},
    property::Property,
};
use objects::drawable::Drawable;

pub const DRAWINS_HANDLE: &'static str = "__drawins";

#[derive(Debug, Default)]
pub struct DrawinState {
    // Note that the drawable is stored in Lua.
    // TODO WINDOW_OBJECT_HEADER??
    ontop: bool,
    visible: bool,
    cursor: String,
    geometry: Area,
    geometry_dirty: bool,
    texture: Option<Texture<'static>>,
    surface: Option<ImageSurface>,
}

unsafe impl Send for DrawinState {}

#[derive(Clone, Debug)]
pub struct Drawin<'lua>(Object<'lua>);

impl UserData for DrawinState {
    fn add_methods(methods: &mut UserDataMethods<Self>) {
        object::default_add_methods(methods);
    }
}

impl Display for DrawinState {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Drawin: {:p}", self)
    }
}

impl<'lua> Drawin<'lua> {
    fn new(lua: &'lua Lua, args: Table) -> rlua::Result<Object<'lua>> {
        let class = class::class_setup(lua, "drawin")?;
        let mut drawins = lua.named_registry_value::<Vec<AnyUserData>>(DRAWINS_HANDLE)?;
        let drawin = object_setup(lua, Drawin::allocate(lua, class)?)?
            .handle_constructor_argument(args)?
            .build();
        let cloned_drawin = drawin.clone().object;
        drawins.push(cloned_drawin);
        lua.set_named_registry_value(DRAWINS_HANDLE, drawins.to_lua(lua)?)?;
        Ok(drawin)
    }

    /// Get the drawable associated with this drawin.
    ///
    /// It has the surface that is needed to render to the screen.
    pub fn drawable(&mut self) -> rlua::Result<Drawable> {
        let table = self.0.table()?;
        Drawable::cast(table.get::<_, AnyUserData>("drawable")?.into())
    }

    pub fn texture(&mut self) -> rlua::Result<RefMut<Option<Texture<'static>>>> {
        Ok(RefMut::map(self.get_object_mut()?, |state| {
            &mut state.texture
        }))
    }

    fn update_drawing(&mut self) -> rlua::Result<()> {
        let table = self.0.table()?;
        let user_data: AnyUserData = table.get::<_, AnyUserData>("drawable")?.clone();
        let mut drawable = Drawable::cast(user_data.into())?;
        let mut state = self.get_object_mut()?;
        if state.geometry_dirty {
            drawable.set_geometry(state.geometry)?;
            state.geometry_dirty = false;
        }
        state.surface = drawable.state()?.surface.clone();
        table.raw_set("drawable", drawable)?;
        Ok(())
    }

    pub fn get_visible(&mut self) -> rlua::Result<bool> {
        let drawin = self.state()?;
        Ok(drawin.visible)
    }

    fn set_visible(&mut self, val: bool) -> rlua::Result<()> {
        {
            let mut drawin = self.get_object_mut()?;
            drawin.visible = val;
        }
        if val {
            self.map()
        } else {
            self.unmap()
        }
    }

    fn map(&mut self) -> rlua::Result<()> {
        // TODO other things
        self.update_drawing()?;
        Ok(())
    }

    fn unmap(&mut self) -> rlua::Result<()> {
        // TODO?
        Ok(())
    }

    pub fn get_geometry(&self) -> rlua::Result<Area> {
        Ok(self.state()?.geometry)
    }

    fn resize(&mut self, geometry: Area) -> rlua::Result<()> {
        {
            let mut state = self.get_object_mut()?;
            let old_geometry = state.geometry;
            state.geometry = geometry;
            {
                let Size {
                    ref mut width,
                    ref mut height,
                } = state.geometry.size;
                if *width <= 0 {
                    *width = old_geometry.size.width;
                }
                if *height <= 0 {
                    *height = old_geometry.size.height
                }
            }
            state.geometry_dirty = true;
            // TODO emit signals
            // TODO update screen workareas like in awesome? Might not be necessary
        }
        self.update_drawing()
    }
}

impl<'lua> ToLua<'lua> for Drawin<'lua> {
    fn to_lua(self, lua: &'lua Lua) -> rlua::Result<Value<'lua>> {
        self.0.to_lua(lua)
    }
}

impl_objectable!(Drawin, DrawinState);

pub fn init(lua: &Lua) -> rlua::Result<Class> {
    let drawins: Vec<Drawin> = Vec::new();
    lua.set_named_registry_value(DRAWINS_HANDLE, drawins.to_lua(lua)?)?;
    property_setup(
        lua,
        method_setup(lua, Class::builder(lua, "drawin", None)?)?,
    )?.save_class("drawin")?
    .build()
}

fn method_setup<'lua>(
    lua: &'lua Lua,
    builder: ClassBuilder<'lua>,
) -> rlua::Result<ClassBuilder<'lua>> {
    // TODO Do properly
    use super::dummy;
    builder.method("connect_signal".into(), lua.create_function(dummy)?)?
           // TODO This should be adding properties, e.g like luaA_class_new
           .method("__call".into(), lua.create_function(|lua, args: Table|
                                                        Drawin::new(lua, args))?)
}

fn property_setup<'lua>(
    lua: &'lua Lua,
    builder: ClassBuilder<'lua>,
) -> rlua::Result<ClassBuilder<'lua>> {
    builder
        .property(Property::new(
            "x".into(),
            Some(lua.create_function(set_x)?),
            Some(lua.create_function(get_x)?),
            Some(lua.create_function(set_x)?),
        ))?.property(Property::new(
            "y".into(),
            Some(lua.create_function(set_y)?),
            Some(lua.create_function(get_y)?),
            Some(lua.create_function(set_y)?),
        ))?.property(Property::new(
            "width".into(),
            Some(lua.create_function(set_width)?),
            Some(lua.create_function(get_width)?),
            Some(lua.create_function(set_width)?),
        ))?.property(Property::new(
            "height".into(),
            Some(lua.create_function(set_height)?),
            Some(lua.create_function(get_height)?),
            Some(lua.create_function(set_height)?),
        ))?.property(Property::new(
            "visible".into(),
            Some(lua.create_function(set_visible)?),
            Some(lua.create_function(get_visible)?),
            Some(lua.create_function(set_visible)?),
        ))
}

fn object_setup<'lua>(
    lua: &'lua Lua,
    builder: ObjectBuilder<'lua>,
) -> rlua::Result<ObjectBuilder<'lua>> {
    // TODO Do properly
    let table = lua.create_table()?;
    let drawable_table = Drawable::new(lua)?.to_lua(lua)?;
    table.set("drawable", drawable_table)?;
    table.set("geometry", lua.create_function(drawin_geometry)?)?;
    table.set("struts", lua.create_function(drawin_struts)?)?;
    table.set("buttons", lua.create_function(super::dummy)?)?;
    builder.add_to_meta(table)
}

fn set_visible<'lua>(_: &'lua Lua, (obj, visible): (AnyUserData<'lua>, bool)) -> rlua::Result<()> {
    let mut drawin = Drawin::cast(obj.into())?;
    drawin.set_visible(visible)
    // TODO signal
}

fn get_visible<'lua>(_: &'lua Lua, obj: AnyUserData<'lua>) -> rlua::Result<bool> {
    let mut drawin = Drawin::cast(obj.into())?;
    drawin.get_visible()
    // TODO signal
}

fn drawin_geometry<'lua>(
    lua: &'lua Lua,
    (drawin, geometry): (AnyUserData<'lua>, Option<Table<'lua>>),
) -> rlua::Result<Table<'lua>> {
    let mut drawin = Drawin::cast(drawin.into())?;
    if let Some(geometry) = geometry {
        let width = geometry.get::<_, i32>("width")?;
        let height = geometry.get::<_, i32>("height")?;
        let x = geometry.get::<_, i32>("x")?;
        let y = geometry.get::<_, i32>("y")?;
        if width > 0 && height > 0 {
            let geo = Area::new(Origin { x, y }, Size { width, height });
            drawin.resize(geo)?;
        }
    }
    let new_geo = drawin.get_geometry()?;
    let Size { width, height } = new_geo.size;
    let Origin { x, y } = new_geo.origin;
    let res = lua.create_table()?;
    res.set("x", x)?;
    res.set("y", y)?;
    res.set("height", height)?;
    res.set("width", width)?;
    Ok(res)
}

fn get_x<'lua>(_: &'lua Lua, drawin: AnyUserData<'lua>) -> rlua::Result<LuaInteger> {
    let drawin = Drawin::cast(drawin.into())?;
    let Origin { x, .. } = drawin.get_geometry()?.origin;
    Ok(x as LuaInteger)
}

fn set_x<'lua>(_: &'lua Lua, (drawin, x): (AnyUserData<'lua>, LuaInteger)) -> rlua::Result<()> {
    let mut drawin = Drawin::cast(drawin.into())?;
    let mut geo = drawin.get_geometry()?;
    geo.origin.x = x as i32;
    drawin.resize(geo)?;
    Ok(())
}

fn get_y<'lua>(_: &'lua Lua, drawin: AnyUserData<'lua>) -> rlua::Result<LuaInteger> {
    let drawin = Drawin::cast(drawin.into())?;
    let Origin { y, .. } = drawin.get_geometry()?.origin;
    Ok(y as LuaInteger)
}

fn set_y<'lua>(_: &'lua Lua, (drawin, y): (AnyUserData<'lua>, LuaInteger)) -> rlua::Result<()> {
    let mut drawin = Drawin::cast(drawin.into())?;
    let mut geo = drawin.get_geometry()?;
    geo.origin.y = y as i32;
    drawin.resize(geo)?;
    Ok(())
}

fn get_width<'lua>(_: &'lua Lua, drawin: AnyUserData<'lua>) -> rlua::Result<LuaInteger> {
    let drawin = Drawin::cast(drawin.into())?;
    let Size { width, .. } = drawin.get_geometry()?.size;
    Ok(width as LuaInteger)
}

fn set_width<'lua>(
    _: &'lua Lua,
    (drawin, width): (AnyUserData<'lua>, LuaInteger),
) -> rlua::Result<()> {
    let mut drawin = Drawin::cast(drawin.into())?;
    let mut geo = drawin.get_geometry()?;
    if width > 0 {
        geo.size.width = width as i32;
        drawin.resize(geo)?;
    }
    Ok(())
}

fn get_height<'lua>(_: &'lua Lua, drawin: AnyUserData<'lua>) -> rlua::Result<LuaInteger> {
    let drawin = Drawin::cast(drawin.into())?;
    let Size { height, .. } = drawin.get_geometry()?.size;
    Ok(height as LuaInteger)
}

fn set_height<'lua>(
    _: &'lua Lua,
    (drawin, height): (AnyUserData<'lua>, LuaInteger),
) -> rlua::Result<()> {
    let mut drawin = Drawin::cast(drawin.into())?;
    let mut geo = drawin.get_geometry()?;
    if height > 0 {
        geo.size.height = height as i32;
        drawin.resize(geo)?;
    }
    Ok(())
}

fn drawin_struts<'lua>(lua: &'lua Lua, _drawin: AnyUserData<'lua>) -> rlua::Result<Table<'lua>> {
    // TODO: Implement this properly. Struts means this drawin reserves some space
    // on the screen that it is visible on, shrinking the workarea in the
    // specified directions.
    let res = lua.create_table()?;
    res.set("left", 0)?;
    res.set("right", 0)?;
    res.set("top", 0)?;
    res.set("bottom", 0)?;
    Ok(res)
}
