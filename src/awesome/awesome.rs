//! TODO Fill in

use super::xproperty::{XProperty, XPropertyType, PROPERTIES};
use super::{signal, XCB_CONNECTION_HANDLE};
use cairo::{self, ImageSurface, ImageSurfaceData};
use gdk_pixbuf::{Pixbuf, PixbufExt};
use glib::translate::{ToGlibPtr, FromGlibPtrNone};
use nix::{self, libc};
use rlua::{self, AnyUserData, LightUserData, Lua, MetaMethod, Table, ToLua, UserData,
           UserDataMethods, Value};
use std::default::Default;
use std::ffi::{CStr, CString};
use std::fmt::{self, Display, Formatter};
use std::process::{Command, Stdio};
use std::thread;
use std::{mem, ptr};
use xcb::ffi::{self, xproto};
use xcb::{xkb, Connection};

// TODO FIXME
// Often we are getting some raw pointers from the xcb replies
// we need to free them because the memory management for them is manual.

// TODO this isn't defined in the xcb crate, even though it should be.
// A patch should be open adding this to its generation scheme
extern "C" {
    fn xcb_xkb_get_names_value_list_unpack(buffer: *mut libc::c_void,
                                           nTypes: u8,
                                           indicators: u32,
                                           virtualMods: u16,
                                           groupNames: u8,
                                           nKeys: u8,
                                           nKeyAliases: u8,
                                           nRadioGroups: u8,
                                           which: u32,
                                           aux: *mut ffi::xkb::xcb_xkb_get_names_value_list_t)
                                           -> libc::c_int;
}

#[derive(Clone, Debug)]
pub struct AwesomeState {
    preferred_icon_size: u32
}

impl Default for AwesomeState {
    fn default() -> Self {
        AwesomeState { preferred_icon_size: 0 }
    }
}

impl Display for AwesomeState {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Awesome: {:p}", self)
    }
}

impl UserData for AwesomeState {
    fn add_methods(methods: &mut UserDataMethods<Self>) {
        fn index<'lua>(_: &'lua Lua,
                       (awesome, index): (AnyUserData<'lua>, Value<'lua>))
                       -> rlua::Result<rlua::Value<'lua>> {
            let table = awesome.get_user_value::<Table>()?;
            table.get::<_, Value>(index)
        };
        methods.add_meta_function(MetaMethod::Index, index);
    }
}

pub fn init(lua: &Lua) -> rlua::Result<()> {
    let awesome_table = lua.create_table()?;
    method_setup(lua, &awesome_table)?;
    property_setup(lua, &awesome_table)?;
    let globals = lua.globals();
    let awesome = lua.create_userdata(AwesomeState::default())?;
    awesome.set_user_value(awesome_table)?;
    globals.set("awesome", awesome)
}

fn method_setup<'lua>(lua: &'lua Lua, awesome_table: &Table<'lua>) -> rlua::Result<()> {
    // TODO Fill in rest
    awesome_table.set("connect_signal",
                       lua.create_function(signal::global_connect_signal)?)?;
    awesome_table.set("disconnect_signal",
                       lua.create_function(signal::global_disconnect_signal)?)?;
    awesome_table.set("emit_signal",
                       lua.create_function(signal::global_emit_signal)?)?;
    awesome_table.set("xrdb_get_value", lua.create_function(xrdb_get_value)?)?;
    awesome_table.set("xkb_set_layout_group",
                       lua.create_function(xkb_set_layout_group)?)?;
    awesome_table.set("xkb_get_layout_group",
                       lua.create_function(xkb_get_layout_group)?)?;
    awesome_table.set("set_preferred_icon_size",
                       lua.create_function(set_preferred_icon_size)?)?;
    awesome_table.set("register_xproperty",
                       lua.create_function(register_xproperty)?)?;
    awesome_table.set("xkb_get_group_names",
                       lua.create_function(xkb_get_group_names)?)?;
    awesome_table.set("set_xproperty", lua.create_function(set_xproperty)?)?;
    awesome_table.set("get_xproperty", lua.create_function(get_xproperty)?)?;
    awesome_table.set("systray", lua.create_function(systray)?)?;
    awesome_table.set("restart", lua.create_function(restart)?)?;
    awesome_table.set("load_image", lua.create_function(load_image)?)?;
    awesome_table.set("pixbuf_to_surface", lua.create_function(pixbuf_to_surface)?)?;
    awesome_table.set("sync", lua.create_function(sync)?)?;
    awesome_table.set("exec", lua.create_function(exec)?)?;
    awesome_table.set("kill", lua.create_function(kill)?)?;
    awesome_table.set("quit", lua.create_function(quit)?)
}

fn property_setup<'lua>(lua: &'lua Lua, awesome_table: &Table<'lua>) -> rlua::Result<()> {
    // TODO Do properly
    awesome_table.set("version", "0".to_lua(lua)?)?;
    awesome_table.set("themes_path", "/usr/share/awesome/themes".to_lua(lua)?)?;
    awesome_table.set("conffile", "".to_lua(lua)?)
}

/// Registers a new X property
fn register_xproperty<'lua>(lua: &'lua Lua,
                            (name_rust, v_type): (String, String))
                            -> rlua::Result<()> {
    let name = CString::new(name_rust.clone()).expect("XProperty was not CString");
    let arg_type = XPropertyType::from_string(v_type.clone())
        .ok_or(rlua::Error::RuntimeError(format!("{} not a valid xproperty", v_type)))?;
    unsafe {
        let raw_con = lua.named_registry_value::<LightUserData>(XCB_CONNECTION_HANDLE)?
                         .0 as _;
        let atom_c = xproto::xcb_intern_atom_unchecked(raw_con,
                                                       false as u8,
                                                       name.to_bytes().len() as u16,
                                                       name.as_ptr());
        let atom_r = xproto::xcb_intern_atom_reply(raw_con, atom_c, ptr::null_mut());
        if atom_r.is_null() {
            return Ok(())
        }
        let new_property = XProperty::new(name_rust.clone(), arg_type, (*atom_r).atom);
        let mut properties = PROPERTIES.lock().expect("Could not lock properties list");
        if let Some(found) = properties.iter()
                                       .find(|&property| property == &new_property)
        {
            if found.type_ != new_property.type_ {
                return Err(rlua::Error::RuntimeError(format!("property '{}' already \
                                                              registered with \
                                                              different type",
                                                             name_rust)))
            }
            return Ok(())
        }
        properties.push(new_property);
    }
    Ok(())
}

/// Get layout short names
fn xkb_get_group_names<'lua>(lua: &'lua Lua, _: ()) -> rlua::Result<Value<'lua>> {
    let xcb_con = lua.named_registry_value::<LightUserData>(XCB_CONNECTION_HANDLE)?
                     .0;
    unsafe {
        let con = Connection::from_raw_conn(xcb_con as _);
        let raw_con = con.get_raw_conn();
        let id = xkb::ID_USE_CORE_KBD as _;
        // The structure here looks weird because we need to ensure Connection
        // isn't cleaned up even in the event of an error...
        let names_r = {
            let names_cookie = xkb::get_names_unchecked(&con, id, xkb::NAME_DETAIL_SYMBOLS);
            names_cookie.get_reply()
        };
        mem::forget(con);
        let names_r = match names_r {
            Ok(names_r) => {
                if names_r.ptr.is_null() {
                    warn!("Failed to get xkb symbols name");
                    return Ok(Value::Nil)
                }
                names_r
            }
            Err(err) => {
                warn!("Failed to get xkb symbols name {:?}", err);
                return Ok(Value::Nil)
            }
        };
        let buffer = ffi::xkb::xcb_xkb_get_names_value_list(names_r.ptr);
        if buffer.is_null() {
            warn!("Returned buffer was NULL");
            return Ok(Value::Nil)
        }
        let names_r_ptr = names_r.ptr;
        if names_r_ptr.is_null() {
            warn!("Name reply pointer was NULL");
            return Ok(Value::Nil)
        }
        let mut names_list: ffi::xkb::xcb_xkb_get_names_value_list_t = mem::uninitialized();
        xcb_xkb_get_names_value_list_unpack(buffer,
                                            (*names_r_ptr).nTypes,
                                            (*names_r_ptr).indicators,
                                            (*names_r_ptr).virtualMods,
                                            (*names_r_ptr).groupNames,
                                            (*names_r_ptr).nKeys,
                                            (*names_r_ptr).nKeyAliases,
                                            (*names_r_ptr).nRadioGroups,
                                            (*names_r_ptr).which,
                                            &mut names_list);
        let atom_name_c = ffi::xproto::xcb_get_atom_name_unchecked(raw_con, names_list.symbolsName);
        let atom_name_r =
            ffi::xproto::xcb_get_atom_name_reply(raw_con, atom_name_c, ptr::null_mut());
        if atom_name_r.is_null() {
            warn!("Failed to get atom symbols name");
            return Ok(Value::Nil)
        }
        let name_c = ffi::xproto::xcb_get_atom_name_name(atom_name_r);
        CStr::from_ptr(name_c).to_string_lossy()
                              .into_owned()
                              .to_lua(lua)
    }
}

/// Query & set information about the systray
fn systray<'lua>(_: &'lua Lua, _: ()) -> rlua::Result<(u32, Value)> {
    Ok((0, Value::Nil))
}

/// Restart Awesome by restarting the Lua thread
fn restart<'lua>(_: &'lua Lua, _: ()) -> rlua::Result<()> {
    error!("Lua thread cannot restart.");
    error!("See https://github.com/way-cooler/way-cooler/issues/555 for more details.");
    error!("Shutting down..");
    super::lua::terminate();
    Ok(())
}

/// Load an image from the given path
/// Returns either a cairo surface as light user data, nil and an error message
fn load_image<'lua>(lua: &'lua Lua, file_path: String) -> rlua::Result<Value<'lua>> {
    let pixbuf = Pixbuf::new_from_file(file_path.as_str())
        .map_err(|err| rlua::Error::RuntimeError(format!("{}", err)))?;
    let surface = load_surface_from_pixbuf(pixbuf);
    // UGH, I wanted to do to_glib_full, but that isn't defined apparently
    // So now I have to ignore the lifetime completely and just forget about the
    // surface.
    let surface_ptr = surface.to_glib_none().0;
    ::std::mem::forget(surface);
    rlua::LightUserData(surface_ptr as _).to_lua(lua)
}

/// Convert a pixbuf to a cairo image surface.
/// Returns either a cairo surface as light user data, nil and an error message
fn pixbuf_to_surface<'lua>(lua: &'lua Lua, pixbuf: LightUserData) -> rlua::Result<Value<'lua>> {
    let pixbuf = unsafe { Pixbuf::from_glib_none(pixbuf.0 as *const _) };
    let surface = load_surface_from_pixbuf(pixbuf);
    // UGH, I wanted to do to_glib_full, but that isn't defined apparently
    // So now I have to ignore the lifetime completely and just forget about the
    // surface.
    let surface_ptr = surface.to_glib_none().0;
    ::std::mem::forget(surface);
    rlua::LightUserData(surface_ptr as _).to_lua(lua)
}

fn exec(_: &Lua, command: String) -> rlua::Result<()> {
    trace!("exec: \"{}\"", command);
    thread::Builder::new().name(command.clone())
                          .spawn(|| {
                                     Command::new(command).stdout(Stdio::null())
                                                          .spawn()
                                                          .expect("Could not spawn command")
                                 })
                          .expect("Unable to spawn thread");
    Ok(())
}

/// Kills a PID with the given signal
///
/// Returns false if it could not send the signal to that process
fn kill(_: &Lua, (pid, sig): (libc::pid_t, libc::c_int)) -> rlua::Result<bool> {
    Ok(nix::sys::signal::kill(pid, sig).is_ok())
}

fn set_preferred_icon_size(lua: &Lua, val: u32) -> rlua::Result<()> {
    let awesome_state = lua.globals().get::<_, AnyUserData>("awesome")?;
    let mut awesome_state = awesome_state.borrow_mut::<AwesomeState>()?;
    awesome_state.preferred_icon_size = val;
    Ok(())
}

fn quit(_: &Lua, _: ()) -> rlua::Result<()> {
    ::wlroots::terminate();
    ::awesome::lua::terminate();
    Ok(())
}

/// No need to sync in Wayland
fn sync(_: &Lua, _: ()) -> rlua::Result<()> {
    Ok(())
}

fn set_xproperty(_: &Lua, _: Value) -> rlua::Result<()> {
    warn!("set_xproperty not supported");
    Ok(())
}

fn get_xproperty(_: &Lua, _: Value) -> rlua::Result<()> {
    warn!("get_xproperty not supported");
    Ok(())
}

fn xkb_set_layout_group(_: &Lua, _group: i32) -> rlua::Result<()> {
    warn!("xkb_set_layout_group not supported; Wait until wlroots");
    Ok(())
}

fn xkb_get_layout_group<'lua>(lua: &'lua Lua, _: ()) -> rlua::Result<Value<'lua>> {
    use xcb::ffi::xkb;
    unsafe {
        let raw_con = lua.named_registry_value::<LightUserData>(XCB_CONNECTION_HANDLE)?
                         .0 as _;
        let state_c = xkb::xcb_xkb_get_state_unchecked(raw_con, xkb::XCB_XKB_ID_USE_CORE_KBD as _);
        let state_r = xkb::xcb_xkb_get_state_reply(raw_con, state_c, ptr::null_mut());
        if state_r.is_null() {
            warn!("State reply was NULL");
            return Ok(Value::Nil)
        }
        (*state_r).group.to_lua(lua)
    }
}

fn xrdb_get_value(_lua: &Lua,
                  (_resource_class, _resource_name): (String, String))
                  -> rlua::Result<Value> {
    warn!("xrdb_get_value not supported");
    Ok(Value::Nil)
}

#[cfg(target_endian = "big")]
fn write_u32(data: &mut ImageSurfaceData, i: usize, a: u8, r: u8, g: u8, b: u8) {
    data[i + 0] = a;
    data[i + 1] = r;
    data[i + 2] = g;
    data[i + 3] = b;
}

#[cfg(target_endian = "little")]
fn write_u32(data: &mut ImageSurfaceData, i: usize, a: u8, r: u8, g: u8, b: u8) {
    data[i + 3] = a;
    data[i + 2] = r;
    data[i + 1] = g;
    data[i + 0] = b;
}

/// Using a Pixbuf buffer, loads the data into a Cairo surface.
pub fn load_surface_from_pixbuf(pixbuf: Pixbuf) -> ImageSurface {
    let width = pixbuf.get_width();
    let height = pixbuf.get_height();
    let channels = pixbuf.get_n_channels() as usize;
    let pix_stride = pixbuf.get_rowstride() as usize;
    // NOTE This is safe because we aren't modifying the bytes, but there's no
    // immutable view
    let pixels = unsafe { pixbuf.get_pixels() };
    let format = if channels == 3 {
        cairo::Format::Rgb24
    } else {
        cairo::Format::ARgb32
    };
    let mut surface =
        ImageSurface::create(format, width, height).expect("Could not create image of that size");
    let cairo_stride = surface.get_stride() as usize;
    {
        let mut cairo_data = surface.get_data().unwrap();
        for y in 0..height as usize {
            let mut pix_pixels_index = y * pix_stride;
            let mut cairo_pixels_index = y * cairo_stride;
            for _ in 0..width {
                let mut r = pixels[pix_pixels_index];
                let mut g = pixels[pix_pixels_index + 1];
                let mut b = pixels[pix_pixels_index + 2];
                let mut a = 1;
                if channels == 4 {
                    a = pixels[pix_pixels_index + 3];
                    let alpha = a as f64 / 255.0;
                    r = (r as f64 * alpha) as u8;
                    g = (g as f64 * alpha) as u8;
                    b = (b as f64 * alpha) as u8;
                }
                write_u32(&mut cairo_data, cairo_pixels_index, a, r, g, b);
                pix_pixels_index += channels;
                cairo_pixels_index += 4;
            }
        }
    }
    surface
}
