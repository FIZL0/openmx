//! S73 — the map edge's gate, proved against the original.
//!
//! The bike builder decides once whether S59's edge clause runs for the bike: off for a bike built
//! for the AI, otherwise on for a player's bike at an outdoor event and off in a stadium. The harness
//! runs the builder's own gate — the stretch between the speed-range copy and the attachment call,
//! with the bike in the register the builder keeps it in and the fourth argument in its own — over
//! generated arguments, events and flags left on the bike, and compares the word it leaves with
//! [`off_map::armed`]. The rest of the builder loads files and allocates, and is not this test's.

use game_sim::off_map::{self, Built, League};
use ps2_float::{F, Vf};
use ps2_oracle::{addr, game_or_skip};

const BIKE: u32 = 0x0120_0000;
const APP: u32 = 0x0121_0000;
const SETTINGS: u32 = 0x0121_1000;
const EVENT: u32 = 0x0121_2000;
/// The bike's armed word.
const ARMED: u32 = 0x570;

/// The gate's first instruction (`a3 == 1`), and the two places it comes out, all off the builder.
const GATE: u32 = 0x1dc;
const GATE_END: [u32; 2] = [0x238, 0x23c];
/// The registers the builder keeps the bike and its fourth argument in.
const S3: usize = 19;
const S1: usize = 17;

struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// Mostly the words the game uses, sometimes any word at all.
    fn word(&mut self, used: u32) -> u32 {
        if self.next() % 6 == 0 { self.next() as u32 } else { (self.next() % u64::from(used)) as u32 }
    }
}

fn built(a3: u32) -> Option<Built> {
    [Built::Player, Built::Ai, Built::Ghost].get(a3 as usize).copied()
}

fn league(w: u32) -> Option<League> {
    [League::Supercross, League::Nationals, League::FreeRide].get(w as usize).copied()
}

#[test]
fn the_builders_gate_is_the_originals() {
    let Some(mut ee) = game_or_skip() else { return };
    let builder = addr("bike_edge_armed_by").unwrap();
    for end in GATE_END {
        // The return address is the harness's own: coming out of the gate is the end of the call.
        ee.hooks.insert(builder + end, Box::new(|_| {}));
    }
    ee.set_u32(addr("app_pointer").unwrap(), APP);
    ee.set_u32(APP + 0x3d4, SETTINGS);
    ee.set_u32(SETTINGS + 0xc, EVENT);

    let mut pick = Pick(0x5e73_a2b1_0bad_cafe);
    let (mut on, mut off, mut kept) = (0, 0, 0);
    const CASES: u32 = 12_000;
    for case in 0..CASES {
        let a3 = pick.word(3);
        let lg = pick.word(4);
        let before = if pick.next() % 2 == 0 { 0 } else { pick.next() as u32 };
        ee.set_u32(EVENT, lg);
        ee.set_u32(BIKE + ARMED, before);
        ee.set(S3, u64::from(BIKE));
        // The builder's fourth argument came in sign-extended, as every 32-bit word does on the EE.
        ee.set(S1, a3 as i32 as i64 as u64);
        ee.call(builder + GATE, &[], &[], 100);
        let got = ee.u32_at(BIKE + ARMED);

        match (built(a3), league(lg)) {
            (Some(b), Some(l)) => {
                let want = u32::from(off_map::armed(b, l));
                assert_eq!(got, want, "case {case}: built {b:?}, {l:?}");
                if want == 1 { on += 1 } else { off += 1 }
            }
            // Words the game never passes: the original's answer is pinned so that a port of them
            // would know it. The AI's bike is disarmed before the league is read; past the three
            // leagues the builder leaves the word as it found it (the bike's constructor clears it).
            _ => {
                let want = if a3 == 1 {
                    0
                } else if lg == 1 || lg == 2 {
                    u32::from(a3 == 0)
                } else if lg == 0 {
                    0
                } else {
                    kept += 1;
                    before
                };
                assert_eq!(got, want, "case {case}: a3 {a3:#x}, league {lg:#x}");
            }
        }
    }
    println!("{CASES} cases: {on} armed, {off} disarmed, {kept} left as found");
    assert!(on > 1000 && off > 4000 && kept > 2000, "{on} armed, {off} disarmed, {kept} left as found");
}

/// The table [`off_map::armed`] is, written out: the only armed bikes are a player's outdoors.
#[test]
fn only_a_players_bike_outdoors_is_armed() {
    for b in [Built::Player, Built::Ai, Built::Ghost] {
        for l in [League::Supercross, League::Nationals, League::FreeRide] {
            let want = b == Built::Player && l != League::Supercross;
            assert_eq!(off_map::armed(b, l), want, "{b:?} {l:?}");
        }
    }
}

/// A flat floor with free09's extent.
struct Edged(game_sim::world::Flat);

impl game_sim::world::World for Edged {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.0.ground(at)
    }
    fn solid(&mut self, bundle: &game_sim::collide::Bundle, record: &mut game_sim::collide::Record) -> bool {
        self.0.solid(bundle, record)
    }
    fn area(&self) -> Option<game_sim::biplane::Area> {
        Some(game_sim::biplane::Area::new(F(3.0), F(3.0), F(9.0), F(9.0)))
    }
}

/// **The rider reads the arm.** The same bike past the margin is thrown when armed — which a new
/// rider is: a player's bike on a free-ride area — and rides on when not, whoever owns it.
#[test]
fn a_disarmed_bike_rides_past_the_edge() {
    use game_sim::phy::Phy;
    use game_sim::ride::Controls;
    use game_sim::rider::{self, Rider};
    use game_sim::trick::{Set, Wreck};

    let tables = rider::Tables::build();
    let p = Phy::read_bytes(include_str!("freeride-bike.phy.txt").as_bytes(), &tables.ride.curve);
    let at = Vf::new(3456.0, 1.0, 100.0, 1.0);
    let dt = F(1.0 / 60.0);
    let mut world = Edged(game_sim::world::Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 0.0) });
    let new = || Rider::new(&p, game_sim::stock::MOUNTS, at, Vf::new(0.0, 0.0, -1.0, 0.0), Set::default());

    let mut armed = new();
    assert!(armed.edge_armed, "a new rider is a player's bike on a free-ride area");
    assert_eq!(armed.frame(&p, &tables, &Controls::default(), dt, &mut world).wreck, Some(Wreck::CANNONBALL));

    for (b, l) in [(Built::Ai, League::FreeRide), (Built::Ghost, League::Nationals), (Built::Player, League::Supercross)] {
        let mut r = new();
        r.edge_armed = off_map::armed(b, l);
        for k in 0..30 {
            let f = r.frame(&p, &tables, &Controls::default(), dt, &mut world);
            assert_ne!(f.wreck, Some(Wreck::CANNONBALL), "{b:?} {l:?}: thrown at frame {k}");
        }
        assert!(r.ride.chassis.max_speed_sq.0 < (off_map::THROWN_MAX_SPEED * off_map::THROWN_MAX_SPEED).0);
    }
}
