//! P37 — the garage: a picked bike and rider reach the ride that starts, per seat.
//!
//! `src/roster.rs` is compiled in by path, the way `tests/ride.rs` takes the shell's pad: this crate
//! is a binary and a test cannot `use` it. Everything here reads the user's own disc and skips
//! without it, saying so; [`a_pick_without_the_disc`] is the half that needs none.
//!
//! What is asserted is the chain from a seat's line in `garage.txt` to the numbers the simulation
//! steps on: the file read back as written, each seat's pick resolved to the bike archive and the
//! `.phy` the original's loader names for it (D7), that `.phy` read by the ported loader, and two
//! picks ridden flat out for three seconds coming out at different speeds — the same bytes in, so a
//! pick changes how it rides by exactly what the files differ by. The rider's half is the rider
//! archive the pick names loading with its own gear.

#[allow(dead_code)]
#[path = "../src/roster.rs"]
mod roster;

use game_render::Disc;
use game_render::rider::{Rider as Body, Sex};
use game_sim::collide::{Bundle, Record};
use game_sim::input::{Control, PADS, Report};
use game_sim::phy::Phy;
use game_sim::rider::{Rider, Tables};
use game_sim::trick::Set as Stunts;
use game_sim::world::World;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};
use roster::{Catalogue, Garage, Pick, Picks, ROWS, Row, SEATS, Set};
use std::sync::Arc;

const DT: F = F(1.0 / 60.0);

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            eprintln!("skipped: {why}");
            None
        }
    }
}

/// Flat ground at y = 0 and nothing solid on it.
struct Floor;

impl World for Floor {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// The pad with the throttle (Cross) held all the way down.
fn gas() -> Report {
    let mut r = Report::idle();
    let b = Control::Cross.binding();
    let low = !b.mask;
    r.0[2] = (low >> 8) as u8;
    r.0[3] = low as u8;
    r.0[b.pressure as usize] = 0xff;
    r
}

/// Ride a `.phy` flat out from a standstill for `frames`, and give back the speed it reached.
fn flat_out(phy_bytes: &[u8], frames: usize) -> f32 {
    let tables = Arc::new(Tables::build());
    let phy = Phy::read_bytes(phy_bytes, &tables.ride.curve);
    let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
    let rider = Rider::new(&phy, stock::mounts(&geom), stock::SPAWN, stock::FACING, Stunts::default());
    let mut sim = Sim::with_rider(0, tables, phy, rider);
    let reports: [Report; PADS] = [gas(), Report::missing()];
    for _ in 0..frames {
        let controls = sim.poll(DT, &reports);
        sim.step(DT, &controls, &mut Floor);
    }
    sim.rider.ride.chassis.speed.0
}

/// The ride's `.phy` for one seat's pick, the way the shell opens it: the pick's bike archive, then
/// `roster::physics` on its set.
fn ride_bytes(disc: &mut Disc, pick: &Pick) -> (Vec<u8>, String) {
    let (bike, _) = pick.members();
    let res = disc.res(&bike).unwrap_or_else(|e| panic!("{bike}: {e}"));
    let (bytes, member, note) = roster::physics(&res, pick.set, &pick.bike).unwrap();
    assert_eq!(note, None, "{bike} has no {}", pick.set.file());
    (bytes, member)
}

#[test]
fn a_pick_without_the_disc() {
    let picks = Picks::parse("seat2 = cr250_faisst pro250a fox2\nseat9 = x race50a y\nseat3 = 125sx nonsense\n");
    assert_eq!(picks.0[0], Pick::default());
    assert_eq!(picks.0[1].bike, "cr250_faisst");
    assert_eq!(picks.0[1].set.word(), "pro250a");
    assert_eq!(picks.0[1].rider, "fox2");
    assert_eq!(picks.0[2].set, Set::FREERIDE, "a set it cannot read keeps the stock one");
    assert_eq!(Picks::parse(&picks.text()), picks);
}

#[test]
fn the_disc_offers_every_set_and_the_rider_set() {
    let Some(mut disc) = disc() else { return };
    let c = Catalogue::read(&mut disc).unwrap();
    assert_eq!(c.bikes.len(), 54, "every Bike\\*.res carries one whole class");
    for engine in roster::ENGINES {
        assert!(!c.models(engine).is_empty(), "no {}cc model", roster::cc(engine));
    }
    // 40 riders in two colours each: the race riders less custom and the pack's ghost.
    assert_eq!(c.riders.len(), 80);
    assert_eq!(c.names().len(), 40);
    assert!(c.names().iter().all(|n| c.riders.iter().filter(|r| r.name == *n).count() == 2));
    assert!(c.rider("thq1").is_some() && c.bike("125sx").is_some());
    assert_eq!(c.fit(&Pick::default()), Pick::default(), "the stock pick is a pick the disc has");

    // Every one of the sixteen sets is in the archive of every model of its class, so each is
    // reached by a garage walked with the d-pad alone, and its own file answers.
    let mut g = Garage::new(&c, &Pick::default());
    let row = |r| ROWS.iter().position(|x| *x == r).unwrap();
    let mut reached = Vec::new();
    for _ in roster::ENGINES {
        g.row = row(Row::Class);
        g.turn(&c, 1);
        for _ in 0..2 {
            g.row = row(Row::Tier);
            g.turn(&c, 1);
            for _ in 0..2 {
                g.row = row(Row::Tuning);
                g.turn(&c, 1);
                let (_, member) = ride_bytes(&mut disc, &g.pick);
                assert_eq!(format!("bike\\{}.phy", g.pick.set.word()), member.to_ascii_lowercase());
                reached.push(g.pick.set);
            }
        }
    }
    reached.sort_by_key(|s| s.word());
    reached.dedup();
    assert_eq!(reached.len(), 16, "{reached:?}");
}

#[test]
fn a_picked_bike_and_rider_reach_the_ride_per_seat() {
    let Some(mut disc) = disc() else { return };
    let c = Catalogue::read(&mut disc).unwrap();
    let seat = |bike: &str, set: &str, rider: &str| {
        c.fit(&Pick { bike: bike.into(), set: Set::from_word(set).unwrap(), rider: rider.into(), ..Pick::default() })
    };
    let wanted = Picks([
        Pick::default(),
        seat("fast50_01", "race50a", "fox2"),
        seat("cr250_faisst", "pro250p", "reed1"),
        seat("500_01", "pro500a", "thor2"),
    ]);
    assert_eq!(SEATS, 4);

    // Persisted: written, read back as it was.
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("p37-garage");
    std::fs::create_dir_all(&dir).unwrap();
    wanted.save(&dir).unwrap();
    let picks = Picks::load(&dir);
    assert_eq!(picks, wanted);
    for p in &picks.0 {
        assert_eq!(c.fit(p), *p, "{p:?} is not a pick the disc has");
    }

    // The stock seat is the shell as it rode before the garage: the 125sx's Race125P.
    let (stock_bytes, stock_member) = ride_bytes(&mut disc, &picks.0[0]);
    assert_eq!(stock_member.to_ascii_lowercase(), "bike\\race125p.phy");

    let tables = Tables::build();
    let mut seen = Vec::new();
    for (i, p) in picks.0.iter().enumerate() {
        let (bytes, member) = ride_bytes(&mut disc, p);
        assert_eq!(member.to_ascii_lowercase(), format!("bike\\{}.phy", p.set.word()), "seat {}", i + 1);
        let phy = format!("{:?}", Phy::read_bytes(&bytes, &tables.ride.curve));
        // The rider: the archive named, loading with its own gear.
        let (_, rider) = p.members();
        let res = disc.res(&rider).unwrap_or_else(|e| panic!("{rider}: {e}"));
        let body = Body::load(&[res], &p.rider, Sex::Male).unwrap();
        assert_eq!(body.name, p.rider);
        seen.push((phy, body.suit.materials.clone()));
    }
    for i in 0..SEATS {
        for j in i + 1..SEATS {
            assert_ne!(seen[i].0, seen[j].0, "seats {} and {} ride the same .phy", i + 1, j + 1);
            assert_ne!(seen[i].1, seen[j].1, "seats {} and {} wear the same gear", i + 1, j + 1);
        }
    }

    // And it rides differently: three seconds flat out on the stock seat's file and on the 50's.
    let (small, _) = ride_bytes(&mut disc, &picks.0[1]);
    let (a, b) = (flat_out(&stock_bytes, 180), flat_out(&small, 180));
    println!("flat out for 3 s: Race125P {a:.2} ft/s, Race50A {b:.2} ft/s");
    assert!(a > 1.0 && b > 1.0, "a bike did not move: {a} {b}");
    assert_ne!(a, b, "the 125 and the 50 came out of three seconds at the same speed");
}
