//! P167: the mouse cursor is hidden while a ride runs with nothing over it, and drawn again for the
//! pause (any seat's), a menu, the front end and a lost focus. The decision is fed a real
//! [`menu::Menu`] walked into a ride by pad reports, as the ride loop feeds it the front end's.

#[path = "../src/cursor.rs"]
mod cursor;
#[path = "../src/menu.rs"]
#[allow(dead_code)]
mod menu;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;
#[path = "../src/roster.rs"]
#[allow(dead_code)]
mod roster;
// The look painted onto the ride (P86).
#[path = "../src/dress.rs"]
#[allow(dead_code)]
mod dress;
#[path = "../src/join.rs"]
#[allow(dead_code)]
mod join;
#[path = "../src/split.rs"]
#[allow(dead_code)]
mod split;
// The shell's pad layer, for P53's presses between frames: a binary crate's module, compiled in by
// path as tests/ride.rs does, its own unit tests coming with it.
#[allow(dead_code)]
#[path = "../src/window.rs"]
mod window;
#[allow(dead_code)]
#[path = "../src/pad.rs"]
mod pad;
// The shell's sound, for the pause's music page (P56).
#[allow(dead_code)]
#[path = "../src/audio.rs"]
mod audio;
// MD11's filters read each mod's source game.
#[path = "../src/track_mods.rs"]
#[allow(dead_code)]
mod track_mods;


use cursor::{Showing, shown};
use game_sim::input::{Control, Report};
use menu::{Controls, Menu, Reader, Screen, Settings};

/// One press and release of `control`, at a hand's pace (P109).
fn tap(menu: &mut Menu, reader: &mut Reader, control: Control) {
    for report in [Report::from_buttons(control.mask()), Report::idle(), Report::idle(), Report::idle()] {
        menu.step(reader.read(&report));
    }
}

#[test]
fn the_cursor_is_hidden_only_for_an_unpaused_ride_in_focus() {
    let mut reader = Reader::new();
    let mut m = Menu::new(Settings::default(), Controls::default());
    m.step(reader.read(&Report::idle()));

    // The front end's screens, and the loads between them and a ride, whatever the focus.
    for focused in [true, false] {
        assert!(shown(Showing::Front, focused));
    }
    // The menu over no ride yet: the main menu, then the track list.
    assert!(m.screen().is_some());
    assert!(shown(Showing::Ride(Some(&m)), true), "a menu is up");

    // Free Ride, Cross on the first track: riding, nothing over it.
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Tracks));
    assert!(shown(Showing::Ride(Some(&m)), true));
    tap(&mut m, &mut reader, Control::Cross);
    assert!(m.riding());
    assert!(!shown(Showing::Ride(Some(&m)), true), "an unpaused ride hides it");
    assert!(shown(Showing::Ride(Some(&m)), false), "a lost focus shows it");
    // A ride without the front end has no menu to put up.
    assert!(!shown(Showing::Ride(None), true));
    assert!(shown(Showing::Ride(None), false));

    // 1P's Start: the pause.
    tap(&mut m, &mut reader, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Pause));
    assert!(shown(Showing::Ride(Some(&m)), true), "the pause shows it");
    // Settings out of the pause, a menu over the ride.
    for _ in 0..4 {
        tap(&mut m, &mut reader, Control::Down);
    }
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Settings));
    assert!(shown(Showing::Ride(Some(&m)), true), "a menu over the ride shows it");
    tap(&mut m, &mut reader, Control::Circle);
    tap(&mut m, &mut reader, Control::Circle);
    assert!(m.riding(), "Circle on the pause resumes");
    assert!(!shown(Showing::Ride(Some(&m)), true));

    // Another seat's own pause (P84), with 1P riding on: every seat has to be riding.
    m.riders = 2;
    m.pause_for(1);
    assert!(m.paused(1) && !m.paused(0) && m.screen().is_none());
    assert!(shown(Showing::Ride(Some(&m)), true), "2P's pause shows it");
}
