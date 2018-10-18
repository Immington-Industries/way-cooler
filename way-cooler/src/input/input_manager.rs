use wlroots::{
    Capability, CompositorHandle, InputManagerHandler, KeyboardHandle, KeyboardHandler,
    PointerHandle, PointerHandler,
};

pub struct InputManager;

impl InputManager {
    pub fn new() -> Self {
        InputManager
    }
}

impl InputManagerHandler for InputManager {
    fn keyboard_added(
        &mut self,
        compositor: CompositorHandle,
        keyboard: KeyboardHandle,
    ) -> Option<Box<KeyboardHandler>> {
        dehandle!(
            @compositor = {compositor};
            @keyboard = {keyboard};
            let server: &mut ::Server = compositor.into();
            server.keyboards.push(keyboard.weak_reference());
            // Now that we have at least one keyboard, update the seat capabilities.
            @seat = {&server.seat.seat};
            let mut capabilities = seat.capabilities();
            capabilities.insert(Capability::Keyboard);
            seat.set_capabilities(capabilities);
            seat.set_keyboard(keyboard.input_device()));
        Some(Box::new(::Keyboard))
    }

    fn pointer_added(
        &mut self,
        compositor: CompositorHandle,
        pointer: PointerHandle,
    ) -> Option<Box<PointerHandler>> {
        dehandle!(
            @compositor = {compositor};
            @pointer = {pointer};
            let server: &mut ::Server = compositor.into();
            server.pointers.push(pointer.weak_reference());
            if server.pointers.len() == 1 {
                // Now that we have at least one keyboard, update the seat capabilities.
                with_handles!([(seat: {&mut server.seat.seat})] => {
                    let mut capabilities = seat.capabilities();
                    capabilities.insert(Capability::Pointer);
                    seat.set_capabilities(capabilities);
                }).expect("Seat was destroyed");
            };

            @cursor = {&server.cursor};
            cursor.attach_input_device(pointer.input_device())
        );
        Some(Box::new(::Pointer))
    }
}
