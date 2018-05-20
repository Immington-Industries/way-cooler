use compositor::View;
use wlroots::{Origin, SeatHandle, SeatHandler};

#[derive(Debug, Default)]
pub struct SeatManager;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Action {
    /// We are moving a view.
    ///
    /// The start is the surface level coordinates of where the first click was
    Moving { start: Origin }
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct Seat {
    pub seat: SeatHandle,
    pub focused: Option<View>,
    pub action: Option<Action>,
    pub meta: bool
}

impl Seat {
    pub fn new(seat: SeatHandle) -> Seat {
        Seat { seat,
               meta: false,
               ..Seat::default() }
    }
}

impl SeatHandler for SeatManager {}

impl SeatManager {
    pub fn new() -> Self {
        SeatManager::default()
    }
}
