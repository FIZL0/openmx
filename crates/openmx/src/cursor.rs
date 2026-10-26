//! The mouse cursor over the window (P167): nothing in a ride is driven by the mouse, so while every
//! seat rides with nothing over it the cursor is hidden rather than left drawn over the picture.
//!
//! It is back the moment there is anything to point at or anywhere else to go: any rider's pause, any
//! menu or front-end screen, the window losing focus, and the shell quitting
//! ([`crate::screen::Screen`] shows it as it closes). A mouse moved during a ride does not bring it
//! back — the decision never reads the pointer.

use crate::menu::Menu;

/// What the window is showing, as far as the cursor cares.
#[derive(Clone, Copy)]
pub enum Showing<'a> {
    /// The front end's screens, and the loads between them and a ride.
    Front,
    /// A ride: under the front end its menu, which knows every seat's pause (P84); a ride without the
    /// front end has none, and no menu to put up.
    Ride(Option<&'a Menu>),
}

/// Whether the cursor is drawn over the window: hidden only for a ride with no menu over any seat,
/// in a window that has the focus.
pub fn shown(showing: Showing, focused: bool) -> bool {
    match showing {
        Showing::Front => true,
        Showing::Ride(menu) => !focused || menu.is_some_and(|m| !m.riding()),
    }
}
