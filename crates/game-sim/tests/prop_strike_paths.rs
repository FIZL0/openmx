//! S197a — the bike's strike on the knockable props ([`prop_contact`], S197) in every step path, not
//! only [`Sim::step`]'s: a session of several riders' [`Sim::step_all`], and the one-player Sims an
//! online or split-screen seat is ([`Sim::step_bumped`], [`Sim::step_among`]).
//!
//! No 2p knock of the original was captured, so these drive the port: a free09 beach ball off the disc
//! and a bike stood against it, the bike's rays striking it. In each path the struck ball is woken
//! and carries the mirror naming its striker — in a session of several, the striking player; in a
//! seat, the seat's own bike as player 1 of its Sim — and the ball's body after the tick is the one
//! the one-player [`Sim::step`] leaves, bit for bit, the other rider standing far off.
//!
//! Skipped with a line where the disc is not there.

use std::sync::Arc;

use game_render::disc::Disc;
use game_sim::bike_col::{self, BikeCol};
use game_sim::biplane::{Area, Body};
use game_sim::phy::{Ini, Phy};
use game_sim::prop_set::PropSet;
use game_sim::ride::{Controls, Tables};
use game_sim::rider::Rider;
use game_sim::rider_contact;
use game_sim::track::Scene;
use game_sim::trick::Set;
use game_sim::world::Flat;
use game_sim::Sim;
use ps2_float::{F, Vf};
use ps2_oracle::skip;

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);

/// free09's terrain: three root cells by three, nine feet a sample (tests/prop_knock.rs).
const FREE09: (f32, f32, f32, f32) = (3.0, 3.0, 9.0, 9.0);

/// How far off the rider who strikes nothing stands.
const AWAY: f32 = 200.0;

/// free09's props as the ride loads them, race125p's phy and the bike's collision files, off the disc.
fn disc() -> Option<(PropSet, Phy, Arc<BikeCol>)> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let member = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("Tracks\\free09.res")).expect("free09.res").name.clone();
    let res = disc.res(&member).expect("free09's archive");
    let toy = Ini::parse_bytes(&res.read_suffix("free09.toy").expect("free09.toy"));
    let scene = Scene::parse(&res.read_suffix("free09.scn").expect("free09.scn")).expect("free09.scn parses");
    let (x, z, sx, sz) = FREE09;
    let (set, said) = PropSet::load(
        &toy,
        "Tracks\\Free09\\",
        &scene.models,
        &mut |p| res.read(p).ok(),
        &mut |p| res.read_suffix(p).ok(),
        Some(Area::new(F(x), F(z), F(sx), F(sz))),
    );
    assert!(said.is_empty(), "free09 loads whole: {said:?}");
    let split = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
    let read = |name: String| split.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
    let col = BikeCol::load(&read(bike_col::rays_file(bike_col::MODEL)), &read(bike_col::shape_file(bike_col::MODEL)))
        .expect("the bike's collision files");
    let bikes: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("bike\\") && n.ends_with(".res")
        })
        .collect();
    let phy = bikes.iter().find_map(|b| disc.res(b).ok()?.read_suffix("race125p.phy").ok()).expect("race125p.phy in a bike archive");
    Some((set, Phy::read_bytes(&phy, &Tables::build().curve), Arc::new(col)))
}

/// A one-player Sim on the stand-in bike with race125p's `[Collisions]` and the bike's files, free09's
/// props in its traffic, wrecks off.
fn one(set: &PropSet, phy: &Phy, col: &Arc<BikeCol>) -> Sim {
    let mut sim = Sim::new(0x5197);
    Arc::make_mut(&mut sim.phy).collisions = phy.collisions;
    sim.traffic.props = set.clone();
    sim.rider.sense.wrecks_enabled = false;
    sim.rider.carries_bike_col(Arc::clone(col));
    sim
}

/// A rider on `sim`'s bike, with the files, wrecks off.
fn rider(sim: &Sim, col: &Arc<BikeCol>) -> Rider {
    let s = game_sim::stock::SPAWN;
    let mut r = Rider::new(&sim.phy, game_sim::stock::mounts(&sim.phy), s, game_sim::stock::FACING, Set::default());
    r.sense.wrecks_enabled = false;
    r.carries_bike_col(Arc::clone(col));
    r
}

/// Where a bike stands to strike a ball: the first collection whose body the bike's rays meet with the
/// bike `back` feet behind it along +x, `up` above. Answers the collection and the bike's spot.
fn against(sim: &Sim) -> Option<(usize, Vf)> {
    let facing = Vf::new(1.0, 0.0, 0.0, 0.0);
    let mut probe = sim.clone();
    for (c, coll) in sim.traffic.props.collections.iter().enumerate() {
        let Some(o) = coll.objects.first().filter(|o| o.collision.is_some()) else { continue };
        let ball = o.node.origin;
        for back in (2..24).map(|k| k as f32 * 0.25) {
            for up in [0.0, 0.5, 1.0, -0.5, 1.5] {
                let at = Vf::new(ball.x() - back, ball.y() + up, ball.z(), 1.0);
                probe.rider.place(at, facing);
                let strike = probe.prop_strike(0, DT);
                if strike.struck.iter().any(|&(k, _, _)| k == c) && !strike.contacts.is_empty() {
                    return Some((c, at));
                }
            }
        }
    }
    None
}

/// The ball of collection `c`: whether it is woken, the striker each mirror on its list names, and its body.
fn ball(sim: &Sim, c: usize) -> (bool, Vec<Option<usize>>, Body) {
    let coll = &sim.traffic.props.collections[c];
    let rc = &coll.objects[0].collision.as_ref().expect("a ball's collision object").record;
    let mirrors = rc.contacts.iter().filter(|e| e.kind == 1).map(|e| e.other).collect();
    (coll.active && coll.bones[0].body.enabled, mirrors, coll.bones[0].body)
}

/// The set-up every path shares: the props, a one-player Sim, and the ball its bike strikes from where.
fn setup() -> Option<(Sim, Arc<BikeCol>, usize, Vf)> {
    let (set, phy, col) = disc()?;
    let sim = one(&set, &phy, &col);
    let (c, at) = against(&sim).expect("some spot by some ball where the bike's rays strike it");
    assert!(!ball(&sim, c).0, "the ball asleep as loaded");
    Some((sim, col, c, at))
}

fn facing() -> Vf {
    Vf::new(1.0, 0.0, 0.0, 0.0)
}

/// The ball after one [`Sim::step`] of the one-player Sim, its bike at `at`: what every other path
/// should leave.
fn alone(sim: &Sim, c: usize, at: Vf) -> (bool, Vec<Option<usize>>, Body) {
    let mut sim = sim.clone();
    sim.rider.place(at, facing());
    sim.step(DT, &Controls::default(), &mut Flat::default());
    let got = ball(&sim, c);
    assert!(got.0, "Sim::step: the struck ball woken");
    assert!(!got.1.is_empty() && got.1.iter().all(|&o| o == Some(0)), "Sim::step: the ball's mirrors name player 1: {:?}", got.1);
    got
}

fn away(at: Vf) -> Vf {
    Vf::new(at.x(), at.y(), at.z() + AWAY, 1.0)
}

/// [`Sim::step_all`]: whichever player's bike strikes the ball, the ball wakes, its mirror names that
/// player, and it goes as the one-player Sim's.
#[test]
fn every_player_in_a_session_strikes() {
    let Some((base, col, c, at)) = setup() else { return };
    let want = alone(&base, c, at);
    for striker in 0..2 {
        let mut sim = base.clone();
        let r = rider(&sim, &col);
        let p = (*sim.phy).clone();
        sim.add_player(p, r);
        for i in 0..2 {
            let spot = if i == striker { at } else { away(at) };
            let r = if i == 0 { &mut sim.rider } else { &mut sim.others[0].rider };
            r.place(spot, facing());
        }
        sim.step_all(DT, &[Controls::default(); 2], &mut Flat::default());
        let (woken, mirrors, body) = ball(&sim, c);
        assert!(woken, "player {} strikes: the ball woken", striker + 1);
        assert_eq!(mirrors, vec![Some(striker); want.1.len()], "player {} strikes: the mirror names them", striker + 1);
        assert_eq!(body, want.2, "player {} strikes: the ball as the one-player step leaves it", striker + 1);
    }
}

/// Two seats, one Sim each, the striker `striker`: the seats' lists and the stepped seats, the last
/// first, as online steps them; `among` true for [`Sim::step_among`] (with the rivals), else
/// [`Sim::step_bumped`].
fn seats(base: &Sim, col: &Arc<BikeCol>, at: Vf, striker: usize, among: bool) -> Vec<Sim> {
    let mut sims: Vec<Sim> = (0..2)
        .map(|i| {
            let mut s = base.clone();
            s.rider = rider(base, col);
            s.rider.place(if i == striker { at } else { away(at) }, facing());
            s
        })
        .collect();
    let lists = rider_contact::among(&sims, &[true, true], DT).expect("both carry the files");
    for (i, l) in lists.into_iter().enumerate().rev() {
        let mut bump = rider_contact::bump_among(&sims, i, l);
        let on: Vec<(&Rider, bool)> = sims.iter().map(|s| (&s.rider, true)).collect();
        let rivals = rider_contact::rivals_among(&on, i, &Flat::default());
        drop(on);
        if among {
            sims[i].step_among(DT, &Controls::default(), &mut Flat::default(), Some(&mut bump), &rivals);
        } else {
            sims[i].step_bumped(DT, &Controls::default(), &mut Flat::default(), Some(&mut bump));
        }
        rider_contact::bumped_among(&mut sims, &bump);
    }
    sims
}

/// [`Sim::step_bumped`] and [`Sim::step_among`] with the other seats' bump: the striking seat's own
/// props are struck, its mirror naming its own bike (player 1 of its Sim), and the ball goes as the
/// one-player Sim's; the other seat's copy sleeps on.
#[test]
fn a_seat_strikes_its_own_props() {
    let Some((base, col, c, at)) = setup() else { return };
    let want = alone(&base, c, at);
    for among in [false, true] {
        let path = if among { "step_among" } else { "step_bumped" };
        for striker in 0..2 {
            let sims = seats(&base, &col, at, striker, among);
            let (woken, mirrors, body) = ball(&sims[striker], c);
            assert!(woken, "{path}, seat {} strikes: the ball woken", striker + 1);
            assert_eq!(mirrors, want.1, "{path}, seat {} strikes: the mirror names the seat's bike", striker + 1);
            assert_eq!(body, want.2, "{path}, seat {} strikes: the ball as the one-player step leaves it", striker + 1);
            assert!(!ball(&sims[1 - striker], c).0, "{path}: the other seat's ball sleeps on");
        }
    }
}

/// [`seats`] with the seats sharing one prop set (S197c, [`game_sim::prop_seats`]): each seat stepped
/// by [`Sim::step_shared`], the set stepped once after them all.
fn shared_seats(base: &Sim, col: &Arc<BikeCol>, at: Vf, striker: usize) -> Vec<Sim> {
    let mut sims: Vec<Sim> = (0..2)
        .map(|i| {
            let mut s = base.clone();
            s.rider = rider(base, col);
            s.rider.place(if i == striker { at } else { away(at) }, facing());
            s
        })
        .collect();
    let mut props = game_sim::prop_seats::Shared::take(&mut sims.iter_mut().collect::<Vec<_>>()).expect("two seats on a track with props");
    let lists = rider_contact::among(&sims, &[true, true], DT).expect("both carry the files");
    for (i, l) in lists.into_iter().enumerate().rev() {
        let mut bump = rider_contact::bump_among(&sims, i, l);
        let on: Vec<(&Rider, bool)> = sims.iter().map(|s| (&s.rider, true)).collect();
        let rivals = rider_contact::rivals_among(&on, i, &Flat::default());
        drop(on);
        sims[i].step_shared(DT, &Controls::default(), &mut Flat::default(), Some(&mut bump), &rivals, &mut props, i);
        rider_contact::bumped_among(&mut sims, &bump);
    }
    props.finish(&mut sims.iter_mut().collect::<Vec<_>>(), DT, &mut Flat::default());
    sims
}

/// **A 2p knock** (S197c): two seats sharing one prop set, whichever seat's bike strikes the ball, the
/// ball is woken in both seats' worlds — seat 0's, which the shell draws, too — its mirror names the
/// striking seat, and it goes as [`Sim::step_all`]'s session of the same two riders leaves it, bit
/// for bit. Every seat holds the same set after the tick, and a seat's snapshot carries it.
#[test]
fn seats_share_one_prop_set() {
    let Some((base, col, c, at)) = setup() else { return };
    for striker in 0..2 {
        let mut session = base.clone();
        let r = rider(&session, &col);
        let p = (*session.phy).clone();
        session.add_player(p, r);
        for i in 0..2 {
            let spot = if i == striker { at } else { away(at) };
            let r = if i == 0 { &mut session.rider } else { &mut session.others[0].rider };
            r.place(spot, facing());
        }
        session.step_all(DT, &[Controls::default(); 2], &mut Flat::default());
        let want = ball(&session, c);
        assert!(want.0 && want.1.iter().all(|&o| o == Some(striker)), "step_all: player {} knocks the ball", striker + 1);

        let sims = shared_seats(&base, &col, at, striker);
        for (k, sim) in sims.iter().enumerate() {
            let (woken, mirrors, body) = ball(sim, c);
            assert!(woken, "seat {} strikes: the ball woken in seat {}'s world", striker + 1, k + 1);
            assert_eq!(mirrors, want.1, "seat {} strikes: seat {}'s mirror names the striking seat", striker + 1, k + 1);
            assert_eq!(body, want.2, "seat {} strikes: seat {}'s ball as step_all leaves it", striker + 1, k + 1);
        }
        let bytes = game_sim::snapshot::encode(&sims[1]);
        let mut back = base.clone();
        game_sim::snapshot::decode(&mut back, &bytes).expect("seat 2's state decodes");
        assert_eq!(ball(&back, c), ball(&sims[0], c), "seat 2's snapshot carries the shared ball");
    }
}

/// [`Sim::step_among`] alone but for a rival far off (no bump: its own list is built for the strike).
#[test]
fn a_seat_with_only_rivals_strikes() {
    let Some((base, col, c, at)) = setup() else { return };
    let want = alone(&base, c, at);
    let mut sim = base.clone();
    sim.rider.place(at, facing());
    let mut other = rider(&base, &col);
    other.place(away(at), facing());
    let on = [(&sim.rider, true), (&other, true)];
    let rivals = rider_contact::rivals_among(&on, 0, &Flat::default());
    assert!(!rivals.is_empty(), "the far rider is a rival");
    sim.step_among(DT, &Controls::default(), &mut Flat::default(), None, &rivals);
    let (woken, mirrors, body) = ball(&sim, c);
    assert!(woken);
    assert_eq!(mirrors, want.1);
    assert_eq!(body, want.2);
}

// --- S210: a wheel on the ball ----------------------------------------------------------------------

/// The owner words a bike's two wheels' contacts left, front then rear.
fn owners(r: &Rider) -> [u32; 2] {
    [r.ride.front.mv.fresh.scalars[2], r.ride.rear.mv.fresh.scalars[2]]
}

/// Where a bike stands with a wheel on a ball and its body rays striking nothing: the first ball and
/// spot about it where one [`Sim::step`] leaves the ball's owner word ([`prop_wheel::handle`]) on a
/// wheel. Answers the collection, the spot, and the word.
fn on_a_ball(sim: &Sim) -> Option<(usize, Vf, u32)> {
    for (c, coll) in sim.traffic.props.collections.iter().enumerate() {
        let Some(o) = coll.objects.first().filter(|o| o.collision.is_some()) else { continue };
        let ball = o.node.origin;
        let handle = game_sim::prop_wheel::handle(&sim.traffic.props, c, 0);
        for dx in (-12..=12).map(|k| k as f32 * 0.25) {
            for dy in (2..=14).map(|k| k as f32 * 0.25) {
                let at = Vf::new(ball.x() + dx, ball.y() + dy, ball.z(), 1.0);
                let mut probe = sim.clone();
                probe.rider.place(at, facing());
                if !probe.prop_strike(0, DT).contacts.is_empty() {
                    continue;
                }
                probe.step(DT, &Controls::default(), &mut Flat::default());
                if owners(&probe.rider).contains(&handle) {
                    return Some((c, at, handle));
                }
            }
        }
    }
    None
}

/// **A wheel stands on a ball, in every step path** (S210): a bike stood over a free09 prop (the first
/// collection [`on_a_ball`] finds one on), its body rays clear of it, has a wheel's contact on the ball's `f.col` — the wheel's owner word the
/// ball's ([`prop_wheel::handle`]) — and the ball, asleep as loaded, is woken by the wheel's hit, in
/// [`Sim::step`], [`Sim::step_all`] (whichever player is over it) and a seat's
/// [`Sim::step_bumped`] and [`Sim::step_among`]. With no props in the Sim the same spot has no such
/// owner (the wheel went through the ball before S210). And the ball's motion is what the wheel
/// stands on: the same tick with the ball rolling under it leaves the bike going differently.
#[test]
fn a_wheel_stands_on_the_ball_in_every_path() {
    let Some((set, phy, col)) = disc() else { return };
    let base = one(&set, &phy, &col);
    let (c, at, handle) = on_a_ball(&base).expect("some spot over some ball where a wheel stands on it");
    eprintln!("collection {c}, the bike at {at:?}, handle {handle}");
    assert!(!ball(&base, c).0, "the ball asleep as loaded");

    let mut sim = base.clone();
    sim.rider.place(at, facing());
    sim.step(DT, &Controls::default(), &mut Flat::default());
    assert!(owners(&sim.rider).contains(&handle), "Sim::step: a wheel on the ball");
    assert!(ball(&sim, c).0, "Sim::step: the wheel's hit wakes the ball");
    assert!(sim.rider.ride.props.is_none() && !sim.traffic.props.is_empty(), "the props are back in the traffic");

    let mut none = base.clone();
    none.traffic.props = PropSet::default();
    none.rider.place(at, facing());
    none.step(DT, &Controls::default(), &mut Flat::default());
    assert_eq!(owners(&none.rider), [0, 0], "no props, no owner");

    // The ball woken and rolling across under the wheel, against woken and still.
    let rolled = |v: Vf| {
        let mut sim = base.clone();
        sim.traffic.props.struck(c, 0);
        sim.traffic.props.collections[c].bones[0].body.velocity = v;
        sim.rider.place(at, facing());
        sim.step(DT, &Controls::default(), &mut Flat::default());
        assert!(owners(&sim.rider).contains(&handle), "a wheel on the ball going {v:?}");
        sim.rider.ride.chassis.vel
    };
    let (still, rolling) = (rolled(Vf::new(0.0, 0.0, 0.0, 0.0)), rolled(Vf::new(0.0, 0.0, 8.0, 0.0)));
    assert_ne!(still.to_bits(), rolling.to_bits(), "the ball's motion reaches the bike: {still:?} both ways");

    for over in 0..2 {
        let mut sim = base.clone();
        let r = rider(&sim, &col);
        let p = (*sim.phy).clone();
        sim.add_player(p, r);
        for i in 0..2 {
            let spot = if i == over { at } else { away(at) };
            let r = if i == 0 { &mut sim.rider } else { &mut sim.others[0].rider };
            r.place(spot, facing());
        }
        sim.step_all(DT, &[Controls::default(); 2], &mut Flat::default());
        let (on, off) = if over == 0 { (&sim.rider, &sim.others[0].rider) } else { (&sim.others[0].rider, &sim.rider) };
        assert!(owners(on).contains(&handle), "step_all, player {} over the ball: a wheel on it", over + 1);
        assert_eq!(owners(off), [0, 0], "step_all: the other player on nothing");
        assert!(ball(&sim, c).0, "step_all, player {} over the ball: woken", over + 1);
        assert!(sim.rider.ride.props.is_none() && sim.others[0].rider.ride.props.is_none() && !sim.traffic.props.is_empty());
    }

    for among in [false, true] {
        let path = if among { "step_among" } else { "step_bumped" };
        for over in 0..2 {
            let sims = seats(&base, &col, at, over, among);
            assert!(owners(&sims[over].rider).contains(&handle), "{path}, seat {} over the ball: a wheel on it", over + 1);
            assert!(ball(&sims[over], c).0, "{path}, seat {} over the ball: woken", over + 1);
            assert!(!ball(&sims[1 - over], c).0, "{path}: the other seat's ball sleeps on");
        }
    }
}
