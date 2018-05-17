use compositor::{self, Server};
use wlroots::{Capability, CompositorHandle, InputManagerHandler, KeyboardHandler, KeyboardHandle,
              PointerHandler, PointerHandle};

pub struct InputManager;

impl InputManager {
    pub fn new() -> Self {
        InputManager
    }
}

impl InputManagerHandler for InputManager {
    fn keyboard_added(&mut self,
                      compositor: CompositorHandle,
                      keyboard: KeyboardHandle)
                      -> Option<Box<KeyboardHandler>> {
        with_handles!([(compositor: {compositor}), (keyboard: {keyboard})] => {
            let server: &mut Server = compositor.into();
            server.keyboards.push(keyboard.weak_reference());
            if server.keyboards.len() == 1 {
                // Now that we have at least one keyboard, update the seat capabilities.
                with_handles!([(seat: {&mut server.seat.seat})] => {
                    let mut capabilities = seat.capabilities();
                    capabilities.insert(Capability::Keyboard);
                    seat.set_capabilities(capabilities);
                    seat.set_keyboard(keyboard.input_device());
                }).expect("Seat was destroyed");
            }
        }).unwrap();
        Some(Box::new(compositor::Keyboard))
    }

    fn pointer_added(&mut self,
                     compositor: CompositorHandle,
                     pointer: PointerHandle)
                     -> Option<Box<PointerHandler>> {
        with_handles!([(compositor: {compositor}), (pointer: {pointer})] => {
            let server: &mut Server = compositor.data.downcast_mut().unwrap();
            server.pointers.push(pointer.weak_reference());
            if server.pointers.len() == 1 {
                // Now that we have at least one keyboard, update the seat capabilities.
                with_handles!([(seat: {&mut server.seat.seat})] => {
                    let mut capabilities = seat.capabilities();
                    capabilities.insert(Capability::Pointer);
                    seat.set_capabilities(capabilities);
                }).expect("Seat was destroyed");
            }

            with_handles!([(cursor: {&mut server.cursor})] => {
                cursor.attach_input_device(pointer.input_device());
            }).expect("Cursor was destroyed");
        }).unwrap();
        Some(Box::new(compositor::Pointer))
    }

    fn keyboard_removed(&mut self, compositor: CompositorHandle, keyboard: KeyboardHandle) {
        with_handles!([(compositor: {compositor}), (keyboard: {keyboard})] => {
            let server: &mut Server = compositor.data.downcast_mut().unwrap();
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

    fn pointer_removed(&mut self, compositor: CompositorHandle, pointer: PointerHandle) {
        with_handles!([(compositor: {compositor}), (pointer: {pointer})] => {
            let server: &mut Server = compositor.data.downcast_mut().unwrap();
            let weak_reference = pointer.weak_reference();
            if let Some(index) = server.pointers.iter().position(|p| *p == weak_reference) {
                server.pointers.remove(index);
                if server.pointers.len() == 0 {
                    with_handles!([(seat: {&mut server.seat.seat})] => {
                        let mut capabilities = seat.capabilities();
                        capabilities.remove(Capability::Pointer);
                        seat.set_capabilities(capabilities);
                    }).expect("Seat was destroyed")
                }
            }
            // TODO Double check this isn't a safety hole actually,
            // because if it isn't then we may not have to do this here...
            with_handles!([(cursor: {&mut server.cursor})] => {
                cursor.deattach_input_device(pointer.input_device());
            }).expect("Cursor was destroyed");
        }).unwrap();
    }
}
