use wlroots::{
    key_events::KeyEvent,
    xkbcommon::xkb::{keysym_get_name, KEY_Escape, KEY_Super_L, KEY_Super_R},
    Capability, CompositorHandle, KeyboardHandle, KeyboardHandler, KeyboardModifier,
    WLR_KEY_PRESSED,
};

pub struct Keyboard;

fn key_is_meta(key: u32) -> bool {
    // TODO configure meta key
    key == KEY_Super_L || key == KEY_Super_R
}

impl KeyboardHandler for Keyboard {
    fn on_key(&mut self, compositor: CompositorHandle, keyboard: KeyboardHandle, event: &KeyEvent) {
        let modifiers = dehandle!(
            @compositor = {compositor};
            if event.key_state() == WLR_KEY_PRESSED {
                for key in event.pressed_keys() {
                    // TODO Keep it hardcoded, make this configurable by awesome
                    if key == KEY_Escape {
                        ::wlroots::terminate();
                    }
                    if key_is_meta(key) {
                        let server: &mut ::Server = compositor.into();
                        server.seat.meta = true;
                    }
                }
            } else {
                for key in event.pressed_keys() {
                    if key_is_meta(key) {
                        let server: &mut ::Server = compositor.into();
                        server.seat.meta = false;
                    }
                }
            };
            let server: &mut ::Server = compositor.into();
            @seat = {&server.seat.seat};
            @keyboard = {keyboard};
            seat.set_keyboard(keyboard.input_device());
            seat.keyboard_notify_key(event.time_msec(),
                                        event.keycode(),
                                        event.key_state() as u32);
            seat.keyboard_send_modifiers(&mut keyboard.get_modifier_masks());
            keyboard.get_modifiers()
        );
        // TODO
        //LUA.with(|lua| {
        //             let lua = lua.borrow();
        //             if let Err(err) = emit_awesome_keybindings(&*lua, event, modifiers) {
        //                 warn!("Could not emit binding for {}: {:?}", event.keycode(), err);
        //             }
        //         });
    }

    fn modifiers(&mut self, compositor: CompositorHandle, keyboard: KeyboardHandle) {
        dehandle!(
            @compositor = {compositor};
            let server: &mut ::Server = compositor.into();
            @seat = {&server.seat.seat};
            @keyboard = {keyboard};
            seat.keyboard_notify_modifiers(&mut keyboard.get_modifier_masks())
        );
    }

    fn destroyed(&mut self, compositor: CompositorHandle, keyboard: KeyboardHandle) {
        with_handles!([(compositor: {compositor}), (keyboard: {keyboard})] => {
            let server: &mut ::Server = compositor.into();
            let weak_reference = keyboard.weak_reference();
            if let Some(index) = server.keyboards.iter().position(|k| *k == weak_reference) {
                server.keyboards.remove(index);
                if server.keyboards.len() == 0 {
                    with_handles!([(seat: {&mut server.seat.seat})] => {
                        let mut capabilities = seat.capabilities();
                        capabilities.remove(Capability::Keyboard);
                        seat.set_capabilities(capabilities);
                    }).expect("Seat was destroyed")
                }
            }
        }).unwrap();
    }
}
