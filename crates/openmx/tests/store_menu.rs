//! P198: the mod store's screen as a pad walks it — the grid's arrows, the kind tabs on L1/R1,
//! Cross installing (or updating) only what it can, Square removing only the store's own, Triangle
//! reporting without leaving, Start reading the store again, Circle out; and no store to ask under
//! `--mods off`. The shell's side (the store itself) is `src/modstore_tests.rs`'s.

// the browser's own clock, threads and files, which are std's here (P196)
#[allow(unused)]
#[path = "../src/clock.rs"]
mod clock;
#[allow(unused)]
#[path = "../src/thread.rs"]
mod thread;
#[allow(unused)]
#[path = "../src/files.rs"]
mod files;
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


use game_sim::input::{Control, Report};
use menu::{Card, Controls, Got, Listed, Menu, Reader, STORE_TABS, Screen, Settings, StoreAsk};

/// One press and release of `control`, at a hand's pace (P109).
fn tap(menu: &mut Menu, reader: &mut Reader, control: Control) {
    for report in [Report::from_buttons(control.mask()), Report::idle(), Report::idle(), Report::idle()] {
        menu.step(reader.read(&report));
    }
}

fn card(key: &str, tab: usize, got: Got) -> Card {
    Card { key: key.into(), tab, name: key.into(), got, ..Card::default() }
}

/// The lit card's key.
fn lit(m: &Menu) -> Option<String> {
    m.store_lit().map(|i| m.store.cards[i].key.clone())
}

#[test]
fn the_store_screen_walks_and_asks() {
    let mut reader = Reader::new();
    let mut m = Menu::new(Settings::default(), Controls::default());
    m.step(reader.read(&Report::idle()));
    m.open(Screen::Store);
    m.ask_store_listing();
    assert_eq!(m.screen(), Some(Screen::Store));
    assert_eq!(m.store.listed, Listed::Asking);
    assert_eq!(m.take_store_asks(), [StoreAsk::List { fresh: false }]);
    assert_eq!(m.take_store_asks(), [], "taken once");

    // five tracks (two rows of the four-wide grid) and a music pack
    m.store.cards = vec![
        card("a/t0", 0, Got::No),
        card("a/t1", 0, Got::Installed),
        card("a/t2", 0, Got::Update),
        card("a/t3", 0, Got::Own),
        card("a/t4", 0, Got::No),
        card("b/m0", 2, Got::No),
    ];
    m.store.listed = Listed::Shown;
    assert_eq!(lit(&m).as_deref(), Some("a/t0"));
    tap(&mut m, &mut reader, Control::Down);
    assert_eq!(lit(&m).as_deref(), Some("a/t4"), "a row down");
    tap(&mut m, &mut reader, Control::Down);
    assert_eq!(lit(&m).as_deref(), Some("a/t4"), "no row under it");
    tap(&mut m, &mut reader, Control::Up);
    tap(&mut m, &mut reader, Control::Right);
    tap(&mut m, &mut reader, Control::Right);
    assert_eq!(lit(&m).as_deref(), Some("a/t2"));
    tap(&mut m, &mut reader, Control::Down);
    assert_eq!(lit(&m).as_deref(), Some("a/t2"), "the row under it ends before it");

    // Cross: an update, once, its bar up at once
    tap(&mut m, &mut reader, Control::Cross);
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.take_store_asks(), [StoreAsk::Install("a/t2".into())]);
    assert!(m.store.cards[2].progress.is_some());
    // nor Square while it installs
    tap(&mut m, &mut reader, Control::Square);
    assert_eq!(m.take_store_asks(), []);

    // the user's own folder of that id: neither installed over nor removed
    tap(&mut m, &mut reader, Control::Right);
    assert_eq!(lit(&m).as_deref(), Some("a/t3"));
    tap(&mut m, &mut reader, Control::Cross);
    tap(&mut m, &mut reader, Control::Square);
    assert_eq!(m.take_store_asks(), []);

    // installed: Cross does nothing, Square removes
    for _ in 0..2 {
        tap(&mut m, &mut reader, Control::Left);
    }
    assert_eq!(lit(&m).as_deref(), Some("a/t1"));
    tap(&mut m, &mut reader, Control::Cross);
    tap(&mut m, &mut reader, Control::Square);
    assert_eq!(m.take_store_asks(), [StoreAsk::Remove("a/t1".into())]);

    // Triangle reports it by key and stays on the store, though Triangle is also Back
    tap(&mut m, &mut reader, Control::Triangle);
    assert_eq!(m.take_store_asks(), [StoreAsk::Report("a/t1".into())]);
    assert_eq!(m.screen(), Some(Screen::Store));

    // the tabs: R1 to the empty Trick Packs, R1 to Music; L1 round past Tracks
    tap(&mut m, &mut reader, Control::R1);
    assert_eq!((m.store.tab, lit(&m)), (1, None));
    tap(&mut m, &mut reader, Control::Cross);
    tap(&mut m, &mut reader, Control::Triangle);
    assert_eq!(m.take_store_asks(), [], "nothing lit, nothing asked");
    tap(&mut m, &mut reader, Control::R1);
    assert_eq!((m.store.tab, lit(&m).as_deref()), (2, Some("b/m0")));
    tap(&mut m, &mut reader, Control::R1);
    assert_eq!((m.store.tab, lit(&m).as_deref()), (0, Some("a/t0")), "R1 round to Tracks, the cursor home");
    tap(&mut m, &mut reader, Control::L1);
    assert_eq!(m.store.tab, STORE_TABS.len() - 1);

    // Start reads the store again, past the kept listing
    tap(&mut m, &mut reader, Control::Start);
    assert_eq!(m.take_store_asks(), [StoreAsk::List { fresh: true }]);
    assert_eq!(m.store.listed, Listed::Asking);

    tap(&mut m, &mut reader, Control::Circle);
    assert_ne!(m.screen(), Some(Screen::Store));
}

#[test]
fn no_store_under_mods_off() {
    let mut reader = Reader::new();
    let mut m = Menu::new(Settings::default(), Controls::default());
    m.step(reader.read(&Report::idle()));
    m.store.off = true;
    m.open(Screen::Store);
    m.ask_store_listing();
    tap(&mut m, &mut reader, Control::Start);
    assert_eq!(m.take_store_asks(), []);
    assert_eq!(m.screen(), Some(Screen::Store));
}
