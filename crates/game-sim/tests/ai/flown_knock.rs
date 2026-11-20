// ---------------------------------------------------------------------------------------------------
// S492 — an AI bike knocks a prop while player 1 flies: biplane.p2m2's frame run whole
// ---------------------------------------------------------------------------------------------------

use super::draws::{neutral_pad, through, whole_frame};
use super::*;
use ps2_oracle::{Ee, Kit};

/// biplane.p2m2's RAM image (B9, B9a): player 1 flies the biplane among three AI riders.
const BIPLANE_STATE: &str = "states/biplane.p2m2_SaveState.eeMemory.bin";
/// The riding AI rider the ball is put before (its bike's record is the rider's `+0x7d0`), and its
/// bike's collision search (`+0x7bc`), whose `+0x30` says the list is built.
const RIDER: u32 = 0x0188_1b10;
const BIKE_RECORD: u32 = 0x007f_fdf0;
const SEARCH: u32 = 0x7bc;
const BUILT: u32 = 0x30;
/// The other riders in the order their updates run after `RIDER`'s; the human (who flies) last.
const LATER_RIDERS: [u32; 3] = [0x019a_44b0, 0x018f_7290, 0x0189_0cd0];
/// The state's first EyeBall (S241): its physics object (`+0x110` body, `+0x120` record, `+0x128`
/// mesh collidable, `+0x13c` node).
const BALL: u32 = 0x00d3_f510;
/// Where the ball is put: at rest height on the ground where the AI bike rides through at tick 9.
const PUT: [f32; 3] = [2830.4, 243.56, 3104.23];
/// The tick the bike's own rays strike the ball.
const KNOCK: usize = 10;

const COLLIDE: u32 = 0x0027_08a0; // collision_record_collide (record)
const RESOLVE: u32 = 0x0027_0dc0; // collision resolve (record, first)
const PLAYER: u32 = 0x0027_c890; // the player update (rider)
const ENABLE: u32 = 0x0015_b790; // physics object enable (object)
const STRUCK: u32 = 0x0015_ad40; // physics_object_struck (60 bytes), which wakes through enable
const ACTIVATE: u32 = 0x003b_27a0; // physics_collection_activate (collection)
const PROPS: u32 = 0x003b_28a0; // physics_collection_update (collection): the props' step

/// A record's contact list: each entry's kind and the record it names (0 for the ground).
fn list(ee: &Ee, record: u32) -> Vec<(u32, u32)> {
    let (n, at) = (ee.u32_at(record + 0x100), ee.u32_at(record + 0x108));
    (0..n).map(|k| (ee.u32_at(at + 0x80 * k), ee.u32_at(at + 0x80 * k + 0x74))).collect()
}

fn v3(ee: &Ee, a: u32) -> [f32; 3] {
    [ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8)]
}

/// Put an asleep prop's physics object at `to`: every float triple within 12 ft of its body's place,
/// in its body, node and mesh collidable, shifted by one delta (research/s197b/put_ball.py's way);
/// then woken as a strike wakes it (enable, then its collection activated). Its tree key follows it
/// on its first tick. Answers its collection.
fn put_prop(ee: &mut Ee, object: u32, to: [f32; 3]) -> u32 {
    let (body, node, mesh) = (ee.u32_at(object + 0x110), ee.u32_at(object + 0x13c), ee.u32_at(object + 0x128));
    let at = v3(ee, body + 0x60);
    let d = [to[0] - at[0], to[1] - at[1], to[2] - at[2]];
    for (base, len) in [(body, 0x200), (node, 0xc0), (mesh, 0x200)] {
        let mut off = 0;
        while off + 12 <= len {
            let v = v3(ee, base + off);
            if (0..3).all(|k| (v[k] - at[k]).abs() < 12.0) {
                for (k, (v, d)) in v.iter().zip(d).enumerate() {
                    ee.set_f32(base + off + 4 * k as u32, v + d);
                }
                off += 12;
            } else {
                off += 4;
            }
        }
    }
    // The collidable's second listener is its collection's, at the collection's +0x30.
    let collection = ee.u32_at(ee.u32_at(mesh + 0xf4 + 8) + 4) - 0x30;
    ee.call(ENABLE, &[u64::from(object)], &[], 2_000_000_000);
    ee.call(ACTIVATE, &[u64::from(collection)], &[], 2_000_000_000);
    collection
}

/// What ran, in order; a record's list as its call left it.
#[derive(Clone, Debug, PartialEq)]
enum Call {
    Collide(u32, Vec<(u32, u32)>),
    Resolve(u32, Vec<(u32, u32)>),
    Player(u32),
    /// A wake: the object, and whether a strike's listener asked for it.
    Enable(u32, bool),
    Props(u32),
}

/// **An AI bike knocks a prop while player 1 flies** (S492): biplane.p2m2's frame run whole in the
/// interpreter (S328's recipe), neutral pad, with the state's first ball put on the ground where the
/// AI rider `RIDER` rides through. The bike's list is kept while it stays in its bound, so it is
/// gathered afresh once, on tick 1 (the ball's tree key follows it on tick 0): as if the ball had lain
/// there when the bike last gathered. On tick 10, in the plane's node update, the bike's own rays
/// strike the ball — a kind-0 entry on its list naming the ball, the ball's struck listener woken —
/// and the ball's list has the mirror at its head. The bike pushes the ball in its own frame (its
/// resolves name the ball), and **the props' step** (the ball's collection's update, its two
/// resolves) runs **after every rider's update**, the flying human's too: the order
/// `Sim::props_step_flown` keeps after `Sim::step_flown_riders`.
#[test]
fn s492_an_ai_bike_knocks_a_prop_while_player_1_flies() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let state = kit.ctx(BIPLANE_STATE);
    if !state.exists() {
        return skip(&format!("{} is not there: `./kit state ram` of biplane.p2m2_SaveState.p2s", state.display()));
    }
    let mut ee = Ee::from_ram(&state).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let app = ee.u32_at(addr("app_pointer").unwrap());
    let dt = F(ee.f32_at(app + 0x344));
    assert_eq!(ee.u32_at(RIDER + 0x7d0), BIKE_RECORD, "the AI rider's bike record");
    let (ball_body, ball_record) = (ee.u32_at(BALL + 0x110), ee.u32_at(BALL + 0x120));
    let collection = put_prop(&mut ee, BALL, PUT);
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for at in [COLLIDE, RESOLVE, PLAYER, ENABLE, PROPS] {
        let l = Rc::clone(&log);
        ee.hooks.insert(
            at,
            Box::new(move |ee: &mut Ee| {
                let (a, ra) = (ee.r(4) as u32, ee.r(31) as u32);
                let k = l.borrow().len();
                match at {
                    PLAYER => l.borrow_mut().push(Call::Player(a)),
                    ENABLE => l.borrow_mut().push(Call::Enable(a, (STRUCK..STRUCK + 60).contains(&ra))),
                    PROPS if a == collection => l.borrow_mut().push(Call::Props(a)),
                    _ => {}
                }
                through(ee);
                // A record's list as the call left it, in the call's place (ahead of what it called).
                match at {
                    COLLIDE => l.borrow_mut().insert(k, Call::Collide(a, list(ee, a))),
                    RESOLVE => l.borrow_mut().insert(k, Call::Resolve(a, list(ee, a))),
                    _ => {}
                }
            }),
        );
    }
    let names_ball = |l: &[(u32, u32)]| l.iter().any(|&(kind, other)| kind == 0 && other == ball_record);
    let mut clock = f64::from(ee.u32_at(addr("wall_ticks").unwrap())) * 100.0;
    for tick in 0..=KNOCK {
        if tick == 1 {
            let search = ee.u32_at(RIDER + SEARCH);
            assert_eq!(ee.u32_at(search + BUILT), 1, "the bike's list is built");
            ee.set_u32(search + BUILT, 0);
        }
        let before = v3(&ee, ball_body + 0x70);
        neutral_pad(&mut ee);
        whole_frame(&mut ee, dt, &mut clock);
        let got = std::mem::take(&mut *log.borrow_mut());
        let bike_collide = got
            .iter()
            .position(|c| matches!(c, Call::Collide(r, _) if *r == BIKE_RECORD))
            .unwrap_or_else(|| panic!("tick {tick}: the bike's record collides"));
        let Call::Collide(_, bike_list) = &got[bike_collide] else { unreachable!() };
        if tick < KNOCK {
            assert!(!names_ball(bike_list), "tick {tick}: the bike's rays strike the ball early: {bike_list:?}");
            continue;
        }
        // The knock: the bike's rays strike the ball in the node update, and wake it.
        assert!(names_ball(bike_list), "tick {tick}: the bike's rays strike the ball: {bike_list:?}");
        assert_eq!(got[bike_collide + 1], Call::Enable(BALL, true), "tick {tick}: the strike wakes the ball");
        let ball_collide = got
            .iter()
            .position(|c| matches!(c, Call::Collide(r, _) if *r == ball_record))
            .unwrap_or_else(|| panic!("tick {tick}: the ball collides"));
        assert!(ball_collide > bike_collide, "tick {tick}: the ball is filed after the bike");
        let Call::Collide(_, ball_list) = &got[ball_collide] else { unreachable!() };
        assert_eq!(ball_list.first(), Some(&(1, BIKE_RECORD)), "tick {tick}: the mirror heads the ball's list: {ball_list:?}");
        // Every collide before any rider's update.
        let first_player = got.iter().position(|c| matches!(c, Call::Player(_))).unwrap();
        let last_collide = got.iter().rposition(|c| matches!(c, Call::Collide(..))).unwrap();
        assert!(last_collide < first_player, "tick {tick}: the collides run before the riders");
        // The riders: the AI rider first, its bike's resolves (in its frame) pushing the ball.
        let players: Vec<u32> = got.iter().filter_map(|c| if let Call::Player(r) = c { Some(*r) } else { None }).collect();
        assert_eq!(players, [&[RIDER][..], &LATER_RIDERS].concat(), "tick {tick}: the riders' updates");
        let next_player = got.iter().position(|c| *c == Call::Player(LATER_RIDERS[0])).unwrap();
        let bike_resolves: Vec<usize> =
            got.iter().enumerate().filter(|(_, c)| matches!(c, Call::Resolve(r, _) if *r == BIKE_RECORD)).map(|(k, _)| k).collect();
        assert!(!bike_resolves.is_empty(), "tick {tick}: the bike resolves");
        for &k in &bike_resolves {
            assert!(k > first_player && k < next_player, "tick {tick}: the bike resolves in its own frame");
            let Call::Resolve(_, l) = &got[k] else { unreachable!() };
            assert!(names_ball(l), "tick {tick}: the bike's resolve names the ball: {l:?}");
        }
        // The props' step after every rider's update, the flying human's too: the ball's resolves in it.
        let last_player = got.iter().rposition(|c| matches!(c, Call::Player(_))).unwrap();
        let props = got.iter().position(|c| *c == Call::Props(collection)).unwrap_or_else(|| panic!("tick {tick}: the ball's collection steps"));
        assert!(props > last_player, "tick {tick}: the props' step runs after the riders: {got:?}");
        let ball_resolves: Vec<usize> =
            got.iter().enumerate().filter(|(_, c)| matches!(c, Call::Resolve(r, _) if *r == ball_record)).map(|(k, _)| k).collect();
        assert_eq!(ball_resolves.len(), 2, "tick {tick}: the ball's two resolves");
        assert!(ball_resolves.iter().all(|&k| k > props), "tick {tick}: the ball resolves in the props' step");
        let after = v3(&ee, ball_body + 0x70);
        assert!((after[2] - before[2]).abs() > 10.0, "tick {tick}: the ball is knocked: {before:?} -> {after:?}");
    }
}
