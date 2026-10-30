//! P198: the mod store's screen as a pad walks it — the grid's arrows, the kind tabs on L1/R1,
//! Cross installing (or updating) only what it can, Square removing only the store's own, Triangle
//! reporting without leaving, Start reading the store again, Select downloading the tab, Circle
//! out; and no store to ask under `--mods off`. The shell's side (the store itself) is `src/modstore_tests.rs`'s. P199: the Upload
//! page's rules first, sign-in, Upload and Upload all, and My Mods' take-down asked twice
//! (`src/modstore_up_tests.rs` has the uploads themselves).

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
use menu::{Card, Controls, Got, Listed, Menu, PACKS_TAB, PackRow, Phase, Posted, Reader, STORE_PAGES, STORE_TABS, Screen, Settings, StoreAsk, TEXTURES_TAB, UPLOAD_TAB, Upload};

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
    // past Music, P230's Texture Packs and Pack Order, P199's Upload and My Mods pages, then round to Tracks
    tap(&mut m, &mut reader, Control::R1);
    assert_eq!((m.store.tab, STORE_TABS[m.store.tab], lit(&m)), (TEXTURES_TAB, "Texture Packs", None));
    tap(&mut m, &mut reader, Control::R1);
    assert_eq!(m.store.tab, PACKS_TAB);
    assert_eq!(m.take_store_asks(), [StoreAsk::Packs], "Pack Order reads the mods folder's packs");
    tap(&mut m, &mut reader, Control::R1);
    assert_eq!(m.store.tab, UPLOAD_TAB);
    assert_eq!(m.take_store_asks(), [StoreAsk::Locals], "Upload reads the player's mods");
    tap(&mut m, &mut reader, Control::R1);
    assert_eq!(m.store.tab, UPLOAD_TAB + 1);
    assert_eq!(m.take_store_asks(), [], "My Mods asks nothing signed out");
    tap(&mut m, &mut reader, Control::R1);
    assert_eq!((m.store.tab, lit(&m).as_deref()), (0, Some("a/t0")), "R1 round to Tracks, the cursor home");
    tap(&mut m, &mut reader, Control::L1);
    assert_eq!(m.store.tab, STORE_TABS.len() + STORE_PAGES.len() - 1);
    tap(&mut m, &mut reader, Control::R1);
    assert_eq!(m.store.tab, 0);

    // Start reads the store again, past the kept listing
    tap(&mut m, &mut reader, Control::Start);
    assert_eq!(m.take_store_asks(), [StoreAsk::List { fresh: true }]);
    assert_eq!(m.store.listed, Listed::Asking);

    // Select downloads the tab's mods not installed, past the one installing and the user's own
    tap(&mut m, &mut reader, Control::Select);
    assert_eq!(m.take_store_asks(), [StoreAsk::Install("a/t0".into()), StoreAsk::Install("a/t4".into())]);
    assert!(m.store.cards[4].progress.is_some());
    tap(&mut m, &mut reader, Control::Select);
    assert_eq!(m.take_store_asks(), [], "not twice while they go");

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

#[test]
fn the_upload_and_my_mods_pages_ask_only_when_they_may() {
    let mut reader = Reader::new();
    let mut m = Menu::new(Settings::default(), Controls::default());
    m.step(reader.read(&Report::idle()));
    m.open(Screen::Store);
    m.store_tab(UPLOAD_TAB);
    assert_eq!(m.take_store_asks(), [StoreAsk::Locals]);

    // the rules first: nothing to press until Cross agrees; Triangle is not back here
    assert_eq!(m.store.upload_rows(), []);
    tap(&mut m, &mut reader, Control::Triangle);
    tap(&mut m, &mut reader, Control::Down);
    assert_eq!((m.screen(), m.store.agreed), (Some(Screen::Store), false));
    tap(&mut m, &mut reader, Control::Cross);
    assert!(m.store.agreed);
    assert_eq!(m.take_store_asks(), [], "agreeing asks nothing");
    // and is kept in settings.txt, once ever: a fresh menu from it skips the rules
    assert!(m.take_changed() && m.settings.store_rules);
    let kept = Settings::parse(&m.settings.text());
    assert!(kept.store_rules && kept.text().contains("store_rules = agreed"));
    assert!(!Settings::default().text().contains("store_rules"), "an older file is written as it was");
    let mut fresh = Menu::new(kept, Controls::default());
    fresh.store_tab(UPLOAD_TAB);
    assert!(fresh.store.agreed && !fresh.store.upload_rows().is_empty());

    // signed out: the token page, then Paste (once while it is checked)
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.take_store_asks(), [StoreAsk::Tokens]);
    tap(&mut m, &mut reader, Control::Down);
    tap(&mut m, &mut reader, Control::Cross);
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.take_store_asks(), [StoreAsk::Paste]);

    // signed in, with two mods (one made from a disc goes up like any other)
    m.store.signing = false;
    m.store.account = Some("tester".into());
    m.store.locals = vec![
        Upload { id: "disc".into(), name: "Disc".into(), kind: "Tracks".into(), status: None },
        Upload { id: "mine".into(), name: "Mine".into(), kind: "Music".into(), status: None },
    ];
    // rows: Sign out, Upload all, Disc, Mine
    for _ in 0..3 {
        tap(&mut m, &mut reader, Control::Down);
    }
    tap(&mut m, &mut reader, Control::Down);
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.take_store_asks(), [StoreAsk::Upload(vec!["mine".into()])]);
    assert_eq!(m.store.locals[1].status.as_ref().map(|s| s.0), Some(Phase::Busy));
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.take_store_asks(), [], "not twice while it goes");
    tap(&mut m, &mut reader, Control::Up);
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.take_store_asks(), [StoreAsk::Upload(vec!["disc".into()])], "the disc's mod goes up too");
    // Upload all: what is not going already
    m.store.locals[0].status = Some((Phase::Done, "Uploaded".into()));
    m.store.locals[1].status = Some((Phase::Done, "Uploaded".into()));
    tap(&mut m, &mut reader, Control::Up);
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.take_store_asks(), [StoreAsk::Upload(vec!["disc".into(), "mine".into()])]);
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.take_store_asks(), [], "nothing left to send");
    tap(&mut m, &mut reader, Control::Start);
    assert_eq!(m.take_store_asks(), [StoreAsk::Locals], "Start reads the mods again");

    // My Mods: listed on the way in; Cross arms, Cross again takes down, moving disarms
    tap(&mut m, &mut reader, Control::R1);
    assert_eq!(m.take_store_asks(), [StoreAsk::Mine]);
    assert_eq!(m.store.mine_listed, Listed::Asking);
    m.store.mine = vec![
        Posted { key: "tester/a".into(), name: "A".into(), kind: "Music".into(), size: 1, verified: false, pr: Some(1), busy: false },
        Posted { key: "tester/b".into(), name: "B".into(), kind: "Music".into(), size: 1, verified: true, pr: None, busy: false },
    ];
    m.store.mine_listed = Listed::Shown;
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.store.arm.as_deref(), Some("tester/a"));
    tap(&mut m, &mut reader, Control::Down);
    assert_eq!(m.store.arm, None);
    tap(&mut m, &mut reader, Control::Cross);
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.take_store_asks(), [StoreAsk::TakeDown("tester/b".into())]);
    assert!(m.store.mine[1].busy);
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.take_store_asks(), [], "not while it goes");
    tap(&mut m, &mut reader, Control::Start);
    assert_eq!(m.take_store_asks(), [StoreAsk::Mine]);

    // signed out, My Mods asks nothing
    m.store.account = None;
    tap(&mut m, &mut reader, Control::Cross);
    tap(&mut m, &mut reader, Control::Start);
    assert_eq!(m.take_store_asks(), []);
    tap(&mut m, &mut reader, Control::Circle);
    assert_ne!(m.screen(), Some(Screen::Store));
}

/// P230: Pack Order: Cross turns the lit pack off and on, Left and Right move it up and down the stack
/// with the cursor, each written at once as settings.txt's lines (every pack in `texture_packs`, top
/// first; the off ones in `texture_packs_off`), and saved; Start reads the folder again; nothing under
/// `--mods off`.
#[test]
fn pack_order_switches_and_moves_packs() {
    let mut reader = Reader::new();
    let mut m = Menu::new(Settings::default(), Controls::default());
    m.step(reader.read(&Report::idle()));
    m.open(Screen::Store);
    m.store_tab(PACKS_TAB);
    assert_eq!(m.take_store_asks(), [StoreAsk::Packs]);
    let row = |f: &str| PackRow { folder: f.into(), name: f.into(), textures: 3, on: true };
    m.store.packs = vec![row("textures/a"), row("textures/b"), row("c")];
    let lines = |m: &Menu| (m.settings.texture_packs.join(","), m.settings.texture_packs_off.join(","));
    let _ = m.take_changed();

    tap(&mut m, &mut reader, Control::Down);
    tap(&mut m, &mut reader, Control::Cross);
    assert!(!m.store.packs[1].on);
    assert_eq!(lines(&m), ("textures/a,textures/b,c".into(), "textures/b".into()));
    assert!(m.take_changed(), "saved at once");
    // Left: up the stack, the cursor with it; Left on top stays
    tap(&mut m, &mut reader, Control::Left);
    assert_eq!(lines(&m), ("textures/b,textures/a,c".into(), "textures/b".into()));
    tap(&mut m, &mut reader, Control::Left);
    assert_eq!(lines(&m).0, "textures/b,textures/a,c");
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(lines(&m), ("textures/b,textures/a,c".into(), String::new()), "the lit one, on top, back on");
    // Right: down to the bottom, and no further
    for _ in 0..3 {
        tap(&mut m, &mut reader, Control::Right);
    }
    assert_eq!(lines(&m).0, "textures/a,c,textures/b");
    assert_eq!(m.cursor(Screen::Store), 2);
    // Start reads them again; Square, Triangle and Select do nothing here
    tap(&mut m, &mut reader, Control::Start);
    for c in [Control::Square, Control::Select] {
        tap(&mut m, &mut reader, c);
    }
    assert_eq!(m.take_store_asks(), [StoreAsk::Packs]);
    // the settings file carries both lines back
    let back = Settings::parse(&m.settings.text());
    assert_eq!((back.texture_packs, back.texture_packs_off), (m.settings.texture_packs.clone(), m.settings.texture_packs_off.clone()));

    let mut off = Menu::new(Settings::default(), Controls::default());
    off.store.off = true;
    off.store_tab(PACKS_TAB);
    assert_eq!(off.take_store_asks(), [], "no mods folder, no packs to read");
}

/// MD66: Browse's Tracks tab filters its cards by category, each card's from its `meta.json`: Up
/// from the grid's top row lights the filter, Left and Right step it (All Categories, then the
/// pick's order, Other last), Freestyle shows only the Freestyle cards, a card with no category is
/// under Other, and Down goes back to the grid; another tab has no filter.
#[test]
fn browse_filters_the_tracks_by_category() {
    let mut reader = Reader::new();
    let mut m = Menu::new(Settings::default(), Controls::default());
    m.step(reader.read(&Report::idle()));
    m.open(Screen::Store);
    let with = |key: &str, tab: usize, category: &str| Card { category: category.into(), ..card(key, tab, Got::No) };
    m.store.cards = vec![
        with("a/fs01", 0, "Freestyle"),
        with("a/sx01", 0, "Supercross"),
        with("a/none", 0, ""),
        with("b/fs02", 0, "Freestyle"),
        with("a/sx02", 0, "Supercross"),
        with("c/m0", 2, "Freestyle"),
    ];
    m.store.listed = Listed::Shown;
    assert_eq!(m.store.categories(), ["All Categories", "Freestyle", "Supercross", "Other"]);
    assert!(m.store.filtered());
    let keys = |m: &Menu| -> Vec<String> { m.store.shown().iter().map(|&i| m.store.cards[i].key.clone()).collect() };
    assert_eq!(keys(&m).len(), 5, "All Categories: every track");
    // Up from the top row is the filter: no card lit, so Cross installs nothing.
    tap(&mut m, &mut reader, Control::Right);
    tap(&mut m, &mut reader, Control::Up);
    assert!(m.store.filtering);
    assert_eq!(lit(&m), None);
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.take_store_asks(), []);
    tap(&mut m, &mut reader, Control::Right);
    assert_eq!(m.store.category, "Freestyle");
    assert_eq!(keys(&m), ["a/fs01", "b/fs02"], "Freestyle lists only its own");
    tap(&mut m, &mut reader, Control::Right);
    assert_eq!(keys(&m), ["a/sx01", "a/sx02"]);
    tap(&mut m, &mut reader, Control::Right);
    assert_eq!(m.store.category, "Other");
    assert_eq!(keys(&m), ["a/none"], "no category reads as Other, and is still listed");
    tap(&mut m, &mut reader, Control::Right);
    assert_eq!(m.store.category, "", "round to All Categories");
    tap(&mut m, &mut reader, Control::Left);
    tap(&mut m, &mut reader, Control::Left);
    tap(&mut m, &mut reader, Control::Left);
    assert_eq!(m.store.category, "Freestyle");
    // Down: the grid again, on the filter's first card, which Cross installs.
    tap(&mut m, &mut reader, Control::Down);
    assert!(!m.store.filtering);
    assert_eq!(lit(&m).as_deref(), Some("a/fs01"));
    tap(&mut m, &mut reader, Control::Right);
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.take_store_asks(), [StoreAsk::Install("b/fs02".into())]);
    // Select downloads the filter's cards alone.
    tap(&mut m, &mut reader, Control::Select);
    assert_eq!(m.take_store_asks(), [StoreAsk::Install("a/fs01".into())], "b/fs02 is going already");
    // Music has no filter: every card on it, and Up stays on the grid.
    tap(&mut m, &mut reader, Control::R1);
    tap(&mut m, &mut reader, Control::R1);
    assert_eq!(m.store.tab, 2);
    assert!(!m.store.filtered());
    assert_eq!(keys(&m), ["c/m0"]);
    tap(&mut m, &mut reader, Control::Up);
    assert!(!m.store.filtering);
    assert_eq!(lit(&m).as_deref(), Some("c/m0"));
}
