//! The wreck page (S72): the screen's fade to black a wreck's reset asks for, and the bike put back
//! by the fader's draw on its third update — against the original's own fader, live, and wired into
//! the rider.

use game_sim::phy::Phy;
use game_sim::ride::Controls;
use game_sim::rider::{self, Rider};
use game_sim::state::{self, Owner};
use game_sim::trick::{Set, Wreck};
use game_sim::wreck_page::{Fade, Page, Shade};
use ps2_float::{F, Vf};

/// One capture's cells by column name.
struct Cells {
    cap: game_sim::replay::Capture,
    at: std::collections::HashMap<String, usize>,
}

impl Cells {
    fn f(&self, row: usize, col: &str) -> F {
        let cell = &self.cap.rows[row][self.at[col]];
        F(cell.parse::<f64>().unwrap_or_else(|_| panic!("{col} row {row}: '{cell}'")) as f32)
    }
    fn u(&self, row: usize, col: &str) -> u64 {
        self.cap.rows[row][self.at[col]].parse().unwrap_or_else(|_| panic!("{col} row {row}"))
    }
    /// The fader's updates in its current half, off its elapsed time: whole milliseconds, 33 or 34
    /// a tick.
    fn updates(&self, row: usize) -> u64 {
        (f64::from(self.f(row, "f0_elapsed").0) * 30.0).round() as u64
    }
}

/// **The original's page, live** (`research/s72_page.py`, `watch/s72-page.toml`: S59's edge ride
/// from free09's save state; context/capture/s72/{north,east,west}.csv). The state starts with the
/// rider down (reason 6), west wrecks once more on the way (reason 2), and each ends in the
/// cannonball: seven pages, every one asked for. On each:
///
/// - The request's row is the first fading out. The fade it starts is 100 ms out and 100 ms in, with
///   two listeners on it: the bike and the screen.
/// - Every row from it to the put-back is still down and still fading out, with one more of the
///   fader's updates than ticks since the request (the request's own tick's) — or as many, for a row
///   read between the bike's update and the fader's.
/// - The rows back on the bike are held and fading in, and the ghost's clock (started at the put-back
///   and stepped first thing in each bike update) counts as many ticks since the put-back as the fade
///   in has had updates, or one more for a row read between the two. One row (west's first page) is
///   read inside the black draw itself: the bike back, the fade in not started.
/// - The port's machine and [`Page`], from the row before the request, ask for the page on the
///   request's tick and put the bike back on the page's third update, with the clock the put-back
///   left on the bike — the frozen clock of every row after, to the fade's end — bit for bit.
/// - The fade back in is three updates, and lets the listeners go.
///
/// Each capture missing is skipped.
#[test]
fn the_originals_page_puts_back_on_its_third_update() {
    let dir = match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => std::path::PathBuf::from(dir).join("capture"),
        None => std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/capture"),
    }
    .join("s72");
    let mut pages = 0;
    for name in ["north", "east", "west"] {
        let path = dir.join(format!("{name}.csv"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            println!("skipped {name}: no {} — research/s72_page.py makes it", path.display());
            continue;
        };
        let cap = game_sim::replay::Capture::parse(&text).expect("the recorder's format");
        let at = cap.columns.iter().enumerate().map(|(i, c)| (c.clone(), i)).collect();
        let c = Cells { cap, at };
        let out = |r: usize| c.u(r, "f0_out") != 0;
        let starts: Vec<usize> = (1..c.cap.rows.len()).filter(|&r| out(r) && !out(r - 1)).collect();
        assert!(!starts.is_empty(), "{name}: no page");
        for r0 in starts {
            page_against(&c, name, r0);
            pages += 1;
        }
    }
    println!("{pages} pages");
}

/// **The chord inside the fade out, live** (S84: `research/s84_chord.py`, `watch/s84-chord.toml`;
/// context/capture/s84/*.csv). Free09's state starts with the rider down for reason 6. L1 is held,
/// and R1 goes down as soon as a fader reads fading out. That is wall-clock, so each capture shows for
/// itself where the chord landed. On every capture whose chord came inside the fade out (the bike
/// riding on a row that is still fading out, not yet held):
///
/// - The chord's put-back came first, onto the record as it stood (`Ride::place_back`'s S66
///   proof).
/// - The bike is set down a second time, and by the black draw: the first row after the chord whose
///   applied point moves is held, with the fade in begun.
/// - It is set down on the put-back record as it stood at the draw, bit for bit: this row's record,
///   or the row before's for a row read past the next tick's keep. The port's `place_back` on it gives
///   the same point.
/// - The ghost started over: higher than the row before, or 3.0.
///
/// A chord that came after the page's own put-back (in the fade in) is counted but not checked here:
/// it is the ordinary chord. Each capture missing is skipped.
#[test]
fn the_originals_draw_sets_a_chorded_bike_down_again() {
    let dir = match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => std::path::PathBuf::from(dir).join("capture"),
        None => std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/capture"),
    }
    .join("s84");
    let (mut inside, mut after) = (0, 0);
    for name in ["d0a", "d0b", "d1a", "d1b"] {
        let path = dir.join(format!("{name}.csv"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            println!("skipped {name}: no {} — research/s84_chord.py makes it", path.display());
            continue;
        };
        let cap = game_sim::replay::Capture::parse(&text).expect("the recorder's format");
        let at = cap.columns.iter().enumerate().map(|(i, c)| (c.clone(), i)).collect();
        let c = Cells { cap, at };
        let v = |r: usize, col: &str| {
            Vf::new(c.f(r, &format!("{col}.x")).0, c.f(r, &format!("{col}.y")).0, c.f(r, &format!("{col}.z")).0, c.f(r, &format!("{col}.w")).0)
        };
        let bits = |a: Vf, b: Vf| (0..4).all(|k| a[k].to_bits() == b[k].to_bits());
        let n = c.cap.rows.len();
        let held = |r: usize| c.u(r, "f0_held") != 0;
        let out = |r: usize| c.u(r, "f0_out") != 0;
        let r0 = (1..n).find(|&r| out(r) && !out(r - 1)).unwrap_or_else(|| panic!("{name}: no page"));
        let up = (r0..n).find(|&r| c.u(r, "reason") == 0).unwrap_or_else(|| panic!("{name}: not put back"));
        if held(up) {
            // Back on the bike only with the black drawn: the page's own put-back, the chord after it.
            after += 1;
            continue;
        }
        assert!(out(up), "{name} row {up}: the chord's put-back while fading out");
        assert!(
            bits(v(up, "applied"), v(up, "back")) || bits(v(up, "applied"), v(up - 1, "back")),
            "{name} row {up}: the chord set the bike on its record"
        );
        let r = (up + 1..n)
            .find(|&r| !bits(v(r, "applied"), v(r - 1, "applied")))
            .unwrap_or_else(|| panic!("{name}: set down once only"));
        assert!(held(r) && c.u(r - 1, "reason") == 0, "{name} row {r}: the riding bike set down again by the black draw");
        let rec = [r, r - 1]
            .into_iter()
            .find(|&k| bits(v(r, "applied"), v(k, "back")))
            .unwrap_or_else(|| panic!("{name} row {r}: set down at {:?}, off the record {:?}", v(r, "applied"), v(r, "back")));
        let (_, _, mut rider) = rider_down(Wreck::ATTITUDE, true);
        rider.ride.back = game_sim::ride::Back { at: v(rec, "back"), facing: v(rec, "back_facing"), up: v(rec, "back_up") };
        rider.ride.place_back();
        assert!(bits(rider.ride.position(), v(r, "applied")), "{name} row {r}: the port's place_back");
        let ghost = |r: usize| c.f(r, "ghost").0;
        assert!(ghost(r) > ghost(r - 1) || ghost(r).to_bits() == 3.0f32.to_bits(), "{name} row {r}: the ghost started over");
        println!("{name}: chord on row {up}, set down again on row {r} (record of row {rec})");
        inside += 1;
    }
    println!("{inside} chords inside the fade out, {after} after the put-back");
    if inside + after > 0 {
        assert!(inside >= 2, "only {inside} chords inside the fade out");
    }
}

/// One page of a capture, from the row its fade to black starts on.
fn page_against(c: &Cells, name: &str, r0: usize) {
    let n = c.cap.rows.len();
    let fading_out = |r: usize| c.u(r, "f0_out") != 0;
    let dt = c.f(r0, "dt");
    let reason = c.u(r0, "reason");
    let limit = c.f(r0, if reason == 13 { "cannonball_wreck_time" } else { "max_wreck_time" });

    // The request.
    assert_ne!(reason, 0, "{name} row {r0}: a page for a rider who is down");
    assert!(c.f(r0, "clock") > limit && !(c.f(r0 - 1, "clock") > limit), "{name} row {r0}: asked as the clock passed the limit");
    assert_eq!(c.u(r0, "screen"), c.u(r0, "screen_current"), "{name} row {r0}: the primary screen's fader, the first");
    assert_eq!(c.f(r0, "f0_dur").0.to_bits(), 0.1f32.to_bits(), "{name} row {r0}: 100 ms out");
    assert_eq!((c.u(r0, "f0_in_after"), c.u(r0, "f0_in_ms"), c.u(r0, "f0_listeners")), (1, 100, 2), "{name} row {r0}: then 100 ms in, two listening");

    // Fading out, still down.
    let back = (r0..n).find(|&r| c.u(r, "reason") == 0).unwrap_or_else(|| panic!("{name} row {r0}: not put back"));
    for r in r0..back {
        let ticks = ((c.f(r, "clock") - c.f(r0, "clock")).0 / dt.0).round() as u64;
        assert!(fading_out(r) && c.u(r, "f0_held") == 0, "{name} row {r}: fading out");
        let k = c.updates(r);
        assert!(k == ticks + 1 || k == ticks, "{name} row {r}: {k} updates {ticks} ticks after the request");
        assert!(k <= 3, "{name} row {r}: still down after {k} updates");
    }

    // Back on the bike, fading in.
    let mut held_off = None;
    for r in back..n {
        if c.u(r, "f0_held") == 0 {
            held_off = Some(r);
            break;
        }
        assert!(!fading_out(r), "{name} row {r}: fading out after the put-back");
        let k = c.updates(r);
        if r == back && c.u(r, "f0_in") == 0 && k == 3 {
            continue; // read inside the black draw
        }
        assert_ne!(c.u(r, "f0_in"), 0, "{name} row {r}: fading in");
        let since = ((game_sim::ghost::Ghost::LENGTH - c.f(r, "ghost")).0 / dt.0).round() as u64;
        assert!(k == since || k + 1 == since, "{name} row {r}: {k} updates in, {since} ticks after the put-back");
        assert!(k <= 3, "{name} row {r}: held after {k} updates in");
    }
    let held_off = held_off.unwrap_or_else(|| panic!("{name} row {r0}: the fade in never ended"));
    assert_eq!(c.u(held_off, "f0_listeners"), 0, "{name} row {held_off}: the listeners let go");
    assert_eq!(c.updates(held_off), 3, "{name} row {held_off}: three updates in");

    // The port, from the row before the request.
    let mut m = state::Ride {
        wreck: Some(Wreck(reason as u8)),
        counting: true,
        elapsed: c.f(r0 - 1, "clock"),
        ..state::Ride::default()
    };
    let sense = state::Sense {
        ai_route: false,
        owner: Some(Owner { human: true, screen: true }),
        wrecks_enabled: true,
        air_wrecks: true,
        settles: true,
    };
    let p = state::Params {
        max_wreck_time: c.f(r0, "max_wreck_time"),
        cannonball_wreck_time: c.f(r0, "cannonball_wreck_time"),
    };
    let mut page = Page::default();
    let mut ticks = 0;
    loop {
        let up = page.up();
        let s = m.step(dt, sense, p, |_| None, || up);
        if ticks == 0 {
            assert_eq!(s.reset, Some(state::Reset::Page), "{name} row {r0}: the request's tick");
            assert_eq!(m.elapsed.0.to_bits(), c.f(r0, "clock").0.to_bits(), "{name} row {r0}: the request's clock");
            page.raise();
        } else {
            assert_eq!(s.reset, None, "{name} row {r0}: asked again on tick {ticks}");
        }
        ticks += 1;
        if page.update() {
            m.respawn();
            break;
        }
    }
    assert_eq!(ticks, 3, "{name} row {r0}: put back on the request's tick and the two after");
    for r in back..=held_off {
        assert_eq!(m.elapsed.0.to_bits(), c.f(r, "clock").0.to_bits(), "{name} row {r}: the clock the put-back left");
    }
    let mut fades = 0;
    while page.up() {
        page.update();
        fades += 1;
    }
    assert_eq!(fades, 3, "{name} row {r0}: three updates back in");
    println!(
        "{name}: reason {reason} asked at frame {}, back on the bike from frame {} ({} rows), the fade in over by frame {}",
        c.u(r0, "frame_counter"),
        c.u(back, "frame_counter"),
        back - r0,
        c.u(held_off, "frame_counter"),
    );
}

/// The page alone: three updates to black, which puts the bike back, then three to clear.
#[test]
fn three_updates_out_and_three_in() {
    let mut page = Page::default();
    assert!(!page.up() && !page.update(), "an idle fader does nothing");
    page.raise();
    let out: Vec<bool> = (0..3).map(|_| (page.up(), page.update()).1).collect();
    assert_eq!(out, [false, false, true]);
    assert_eq!(page.fade, Fade::In);
    for k in 0..3 {
        assert!(page.up(), "held on update {k} of the fade in");
        assert!(!page.update());
    }
    assert!(!page.up());
    assert_eq!(page, Page::default());
}

/// **The fade's picture against the original's fader, live** (S85; S72's captures,
/// context/capture/s72/{north,east,west}.csv). Every row the fader is fading out, held or fading in,
/// the port's [`Shade::update`] — fed the whole milliseconds between that row's elapsed and the row
/// before's, or from nothing where a half begins — leaves the elapsed and the t bit-equal to the
/// original's. Every millisecond step is 33 or 34. And the two draws' alphas off those t: the fade out
/// is under 255 until its third update, which draws 255 (the black draw, the put-back), and the fade
/// in is over 0 until its third, which draws 0.
///
/// Each capture missing is skipped.
#[test]
fn the_originals_fade_steps_by_whole_milliseconds() {
    let dir = match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => std::path::PathBuf::from(dir).join("capture"),
        None => std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/capture"),
    }
    .join("s72");
    let (mut updates, mut blacks, mut clears, mut read) = (0, 0, 0, 0);
    for name in ["north", "east", "west"] {
        let path = dir.join(format!("{name}.csv"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            println!("skipped {name}: no {} — research/s72_page.py makes it", path.display());
            continue;
        };
        let cap = game_sim::replay::Capture::parse(&text).expect("the recorder's format");
        let at = cap.columns.iter().enumerate().map(|(i, c)| (c.clone(), i)).collect();
        let c = Cells { cap, at };
        read += 1;
        let live = |r: usize| c.u(r, "f0_out") != 0 || c.u(r, "f0_held") != 0;
        let (mut elapsed, mut t) = (F::ZERO, F::ZERO);
        for r in 1..c.cap.rows.len() {
            if !live(r) {
                continue;
            }
            let (e, te) = (c.f(r, "f0_elapsed"), c.f(r, "f0_t"));
            assert_eq!(c.f(r, "f0_dur").0.to_bits(), Shade::DURATION.0.to_bits(), "{name} row {r}: 100 ms a half");
            let before = if live(r - 1) { c.f(r - 1, "f0_elapsed") } else { F::ZERO };
            if e.0.to_bits() == before.0.to_bits() && live(r - 1) {
                continue; // no update between the two reads
            }
            // A half begins where the elapsed falls back: from nothing, then the row's own update.
            let from = if !live(r - 1) || e.0 < before.0 { F::ZERO } else { before };
            if from == F::ZERO {
                (elapsed, t) = (F::ZERO, F::ZERO);
            }
            if e == F::ZERO {
                continue; // the black draw restarted the half, and nothing has updated it yet
            }
            // One update between the reads, or two where the row before was read ahead of its
            // tick's: each 33 or 34 ms, in whichever order the original's bits say.
            let ms = (f64::from((e - from).0) * 1000.0).round() as i32;
            let splits: Vec<Vec<i32>> = match ms {
                33 | 34 => vec![vec![ms]],
                66..=68 => [[33, ms - 33], [ms - 34, 34]].iter().filter(|s| s.iter().all(|m| *m == 33 || *m == 34)).map(|s| s.to_vec()).collect(),
                _ => panic!("{name} row {r}: a frame of {ms} ms"),
            };
            let stepped = |split: &Vec<i32>| {
                split.iter().fold((elapsed, t), |(e, t), &m| {
                    assert!(t < F::ONE, "{name} row {r}: an update past t = 1");
                    Shade::update(e, m)
                })
            };
            let found = splits.iter().map(stepped).find(|(pe, pt)| (pe.0.to_bits(), pt.0.to_bits()) == (e.0.to_bits(), te.0.to_bits()));
            let Some(port) = found else {
                panic!("{name} row {r}: elapsed {e:?} t {te:?}, port {:?} over {splits:?}", splits.iter().map(stepped).collect::<Vec<_>>());
            };
            (elapsed, t) = port;
            updates += splits[0].len();
            let third = ((e.0 * 30.0).round() as u32) >= 3;
            match c.u(r, "f0_in") != 0 {
                false => {
                    assert_eq!(Shade::out_alpha(t) == 255, third, "{name} row {r}: black on the third update out, t {t:?}");
                    blacks += usize::from(third);
                }
                true => {
                    assert_eq!(Shade::in_alpha(t) == 0, third, "{name} row {r}: clear on the third update in, t {t:?}");
                    clears += usize::from(third);
                }
            }
        }
    }
    println!("{updates} updates, {blacks} black, {clears} clear");
    if read == 3 {
        assert!(updates >= 37 && blacks >= 3 && clears >= 6, "all three captures: {updates} updates, {blacks} black, {clears} clear");
    }
}

/// The shade over a whole page, the way the shell steps it: the page's tick with the front end's
/// milliseconds, 34/33/33 then 34/33/33 (S72's east, second page), and 33/33/34 twice (the first
/// page of every capture). The fade out draws 87 or 84, then 171 or 168, then the black; the fade in
/// the mirror; and the last draw is clear. A tick with no page update draws as the one before.
#[test]
fn the_shade_follows_the_page() {
    for (ms, want) in [([34, 33, 33, 34, 33, 33], [87, 171, 255, 168, 84, 0]), ([33, 33, 34, 33, 33, 34], [84, 168, 255, 171, 87, 0])] {
        let (mut page, mut shade) = (Page::default(), Shade::default());
        shade.step(&page, 33);
        assert_eq!(shade.alpha(), 0, "no page, no shade");
        page.raise();
        let mut drawn = Vec::new();
        for &ms in &ms {
            page.update();
            shade.step(&page, ms);
            drawn.push(shade.alpha());
            shade.step(&page, 33);
            assert_eq!(shade.alpha(), *drawn.last().unwrap(), "no update, the same draw");
        }
        assert_eq!(drawn, want, "{ms:?}");
        assert_eq!(page, Page::default());
    }
}

/// The fader told ahead of time (S108): before each update of a whole page, out and in,
/// `puts_back_next` says what that update answers.
#[test]
fn the_page_says_its_put_back_before_the_update() {
    let mut page = Page::default();
    page.raise();
    let mut put = 0;
    while page.up() {
        let told = page.puts_back_next();
        assert_eq!(told, page.update(), "told before the update at {page:?}");
        put += usize::from(told);
    }
    assert_eq!(put, 1, "one put-back a page");
    assert!(!page.puts_back_next(), "an idle fader puts nothing back");
}

fn rider_down(reason: Wreck, screen: bool) -> (Phy, rider::Tables, Rider) {
    let tables = rider::Tables::build();
    let p = Phy::read_bytes(include_str!("freeride-bike.phy.txt").as_bytes(), &tables.ride.curve);
    let mut r = Rider::new(&p, game_sim::stock::MOUNTS, Vf::new(0.0, 1.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    r.sense.owner = Some(Owner { human: true, screen });
    // Down, the clock armed and on the limit: the next tick passes it.
    r.state.enter(reason);
    r.state.counting = true;
    r.state.elapsed = p.wreck_times.max_wreck_time;
    (p, tables, r)
}

/// **Wired into the rider.** A local human with a screen is put back by the page, on the request's
/// tick and two more; the page is up from the request to the end of the fade back in. One without a
/// screen, and the lost driver's reset, are put back on the request's tick, with no page.
#[test]
fn the_rider_is_put_back_by_the_page() {
    let mut world = game_sim::world::Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 0.0) };
    let dt = F(1.0 / 30.0);
    let idle = Controls::default();

    let (p, t, mut r) = rider_down(Wreck::ATTITUDE, true);
    let back: Vec<bool> = (0..3).map(|_| r.frame(&p, &t, &idle, dt, &mut world).respawned).collect();
    assert_eq!(back, [false, false, true], "the page's put-back");
    assert_eq!(r.state.wreck, None);
    assert!(r.ghost.on, "the put-back's ghost");
    assert_eq!(r.ride.chassis.max_speed_sq.0.to_bits(), (game_sim::off_map::RIDING_MAX_SPEED * game_sim::off_map::RIDING_MAX_SPEED).0.to_bits());
    let up: Vec<bool> = (0..3).map(|_| (r.page.up(), r.frame(&p, &t, &idle, dt, &mut world)).0).collect();
    assert_eq!(up, [true, true, true], "held through the fade in");
    assert!(!r.page.up(), "and let go after it");

    for (reason, screen) in [(Wreck::ATTITUDE, false), (Wreck::NO_CONTROL, true)] {
        let (p, t, mut r) = rider_down(reason, screen);
        assert!(r.frame(&p, &t, &idle, dt, &mut world).respawned, "reason {reason:?}, screen {screen}: on the request's tick");
        assert!(!r.page.up(), "reason {reason:?}, screen {screen}: no page");
    }

    // The chord in the fade's first ticks puts the bike back at once; the page's listener is still
    // on the fader and its draw sets the riding bike down again, on the record as the tick's physics
    // kept it, ghost and all (S84). The record is moved off the bike and the bike given a speed
    // before the draw's tick, so where it rides to and the record differ.
    let (p, t, mut r) = rider_down(Wreck::ATTITUDE, true);
    assert!(!r.frame(&p, &t, &idle, dt, &mut world).respawned);
    let chord = Controls { respawn: true, ..Controls::default() };
    assert!(r.frame(&p, &t, &chord, dt, &mut world).respawned, "the chord");
    r.ghost.clock = F(1.0);
    r.ride.chassis.vel = Vf::new(0.0, 0.0, 30.0, 0.0);
    r.ride.back.at = Vf::new(500.0, 0.0, 500.0, 1.0);
    assert!(r.frame(&p, &t, &idle, dt, &mut world).respawned, "the page's own put-back");
    assert_eq!(r.ghost.clock.0.to_bits(), 3.0f32.to_bits(), "the ghost started again");
    let (at, now) = (r.ride.back.at, r.ride.position());
    assert!((0..4).all(|k| at[k].to_bits() == now[k].to_bits()), "set down on the record {at:?}, not left at {now:?}");
    assert!((0..3).all(|k| r.ride.chassis.vel[k] == F::ZERO), "from rest");
}
