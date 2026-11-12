// ---------------------------------------------------------------------------------------------------
// S309: the tick's path branch on sx01's real AI net, the race image's three riders.
// ---------------------------------------------------------------------------------------------------

use super::*;

/// A track object's paths read out of RAM as the port holds them, with the RAM addresses of the
/// paths and of each path's nodes.
fn s309_paths(ee: &ps2_oracle::Ee, track: u32) -> (game_sim::race::TrackPaths, Vec<u32>, Vec<Vec<u32>>) {
    use game_sim::race::{PathNode, RacePath, TrackPaths};
    let f = |a: u32| F(f32::from_bits(ee.u32_at(a)));
    let quad = |a: u32| Vf(core::array::from_fn(|k| f32::from_bits(ee.u32_at(a + 4 * k as u32))));
    let count = ee.u32_at(track + 0x40) as usize;
    let recs: Vec<u32> = (0..count as u32).map(|k| ee.u32_at(ee.u32_at(track + 0x50) + 4 * k)).collect();
    let index = |a: u32| recs.iter().position(|&r| r == a);
    let mut node_addrs = Vec::new();
    let mut paths = Vec::new();
    for &r in &recs {
        let mut addrs = Vec::new();
        let mut n = ee.u32_at(r + 0x10);
        while n != 0 {
            addrs.push(n);
            n = ee.u32_at(n + 0x54);
        }
        let nodes = addrs
            .iter()
            .map(|&a| PathNode {
                id: ee.u32_at(a) as i32,
                word_04: ee.u32_at(a + 4),
                points: [quad(a + 0x10), quad(a + 0x20), quad(a + 0x30)],
                span: f(a + 0x40),
                word_44: ee.u32_at(a + 0x44),
                word_48: ee.u32_at(a + 0x48),
                word_4c: ee.u32_at(a + 0x4c),
            })
            .collect();
        let list = ee.u32_at(r + 0x2c);
        let links = (0..ee.u32_at(r + 0x18)).map(|k| index(ee.u32_at(list + 4 * k)).expect("a link on the net")).collect();
        let best = ee.u32_at(r + 0x28);
        paths.push(RacePath {
            flags: ee.u32_at(r),
            length: f(r + 8),
            word_0c: ee.u32_at(r + 0xc),
            nodes,
            links,
            dist: f(r + 0x1c),
            best: if best == 0 { None } else { Some(index(best).expect("a best link on the net")) },
        });
        node_addrs.push(addrs);
    }
    let pool = |head: u32, next: u32| {
        let (mut n, mut e) = (0, ee.u32_at(head));
        while e != 0 {
            n += 1;
            e = ee.u32_at(e + next);
        }
        n
    };
    let t = TrackPaths {
        word_34: ee.u32_at(track + 0x34),
        ahead: f(track + 0x38),
        behind: f(track + 0x3c),
        lap_length: f(track + 0x54),
        pick: ee.u32_at(track + 0xac),
        paths,
        free_routes: pool(track + 0x98, 0xc),
        free_parents: pool(track + 0xa0, 8),
        free_candidates: pool(track + 0xa8, 0xc),
    };
    (t, recs, node_addrs)
}

/// A spot's three words in RAM as the port's, `None` for a node that is not on the spot's path.
fn s309_spot(ee: &ps2_oracle::Ee, at: u32, recs: &[u32], nodes: &[Vec<u32>]) -> Option<game_sim::race::Spot> {
    let (path, node) = (ee.u32_at(at), ee.u32_at(at + 4));
    let along = F(f32::from_bits(ee.u32_at(at + 8)));
    if path == 0 {
        return Some(game_sim::race::Spot { path: 0, piece: 0, along });
    }
    let p = recs.iter().position(|&r| r == path)?;
    let n = if node == 0 { 0 } else { nodes[p].iter().position(|&a| a == node)? as u32 + 1 };
    Some(game_sim::race::Spot { path: p as u32 + 1, piece: n, along })
}

/// The follower's words of a brain in RAM.
fn s309_follower(ee: &ps2_oracle::Ee, ai: u32, recs: &[u32], nodes: &[Vec<u32>]) -> Option<game_sim::ai::follow::Follower> {
    let w = |o: u32| F(f32::from_bits(ee.u32_at(ai + o)));
    let q = |o: u32| Vf(got(ee, ai + o).map(f32::from_bits));
    Some(game_sim::ai::follow::Follower {
        flags: ee.read(ai + 0x30, 8) as u64,
        state: ee.u32_at(ai + 0x354) as i32,
        position: q(0xc0),
        forward: q(0xb0),
        own_speed: w(0x80),
        spot: s309_spot(ee, ai + 0x220, recs, nodes)?,
        draws: core::array::from_fn(|k| w(0x22c + 4 * k as u32)),
        aim: q(0x1b0),
        heading: q(0x1d0),
        speed: w(0x1e0),
        angles: q(0x1f0),
        drift: w(0x164),
        kick: w(0x168),
        kick_time: w(0x16c),
        kick_decay: w(0x170),
        kick_clock: w(0x174),
        drift_target: w(0x178),
        drift_low: w(0x17c),
        drift_high: w(0x180),
        drift_rate: w(0x184),
        drift_max: w(0x188),
        drift_clock: w(0x18c),
        period_low: w(0x190),
        period_high: w(0x194),
        period: w(0x198),
        reach: game_sim::ai::seek::Reach { base: w(0x210), max: w(0x214), gain: w(0x218) },
        cruise: w(0x2e0),
        look: w(0x2f0),
        look_cone: w(0x2f4),
    })
}

/// A follower as words, `-0` told from `0`.
fn s309_bits(f: &game_sim::ai::follow::Follower) -> Vec<u32> {
    let mut v = vec![f.flags as u32, (f.flags >> 32) as u32, f.state as u32, f.spot.path, f.spot.piece, f.spot.along.0.to_bits()];
    for q in [f.position, f.forward, f.aim, f.heading, f.angles] {
        v.extend(q.to_bits());
    }
    v.extend(f.draws.iter().map(|d| d.0.to_bits()));
    let words = [
        f.own_speed, f.speed, f.drift, f.kick, f.kick_time, f.kick_decay, f.kick_clock, f.drift_target, f.drift_low, f.drift_high, f.drift_rate,
        f.drift_max, f.drift_clock, f.period_low, f.period_high, f.period, f.reach.base, f.reach.max, f.reach.gain, f.cruise, f.look, f.look_cone,
    ];
    v.extend(words.iter().map(|w| w.0.to_bits()));
    v
}

/// The tick's path branch on the sx01 race image (research/c31's race start run on to frame 604): each
/// of its three AI riders, as the image holds it, on the AI net the image holds (ground-snapped
/// heights, the race's scratch words and pools), from `ai_seek_path_branch` to `ai_seek_path_end`
/// against `ai::follow::path`, then 400 more branches with the rider moved along its own heading at
/// its own target speed between them (a stand-in for the ride, the same on both sides). Each branch:
/// the follower's words, the net's scratch words and pools, and the generator.
#[test]
fn the_path_branch_rides_sx01s_ai_net_as_the_race_image() {
    use game_sim::ai::follow;
    let kit = match ps2_oracle::Kit::load() {
        Ok(k) => k,
        Err(why) => return skip(&why),
    };
    let image = kit.ctx("states").join(S282C_STATE);
    if !image.exists() {
        return skip(&format!(
            "{} is not there: run research/c31's start state on to frame ~600, save, `./kit state ram`",
            image.display()
        ));
    }
    let Some(at) = named(&["boot_gp", "ai_seek_path_branch", "ai_seek_path_end", "rand_state"]) else { return };
    let [gp, branch, end, rand_state] = at[..] else { unreachable!() };
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    for r in S282C_RIDERS {
        let mut ee = ps2_oracle::Ee::from_ram(&image).unwrap_or_else(|e| panic!("{e}"));
        ee.set(28, u64::from(gp));
        ee.hooks.insert(end, Box::new(|ee| ee.set(31, ps2_oracle::DONE as i32 as i64 as u64)));
        let net = ee.u32_at(r + 0xdc);
        let (mut t, recs, nodes) = s309_paths(&ee, net);
        assert_eq!(ee.u32_at(net + 0x4c), recs[0], "the spot search's first path is path 0");
        assert_eq!((ee.u32_at(r + 0x354), ee.u32_at(r + 0x34c)), (0, 0), "rider {r:#x}: on the path, no goal");
        let mut f = s309_follower(&ee, r, &recs, &nodes).expect("the image's spot is on its path");
        let race = ee.u32_at(r + 0xd8);
        let race_word = (race != 0).then(|| ee.u32_at(race + 0x5c));
        let mut rng = Rng::from_state(ee.read(rand_state, 8) as u64);
        let dt = F(1.0 / 30.0);
        for step in 0..=400 {
            let before = f;
            ee.set(19, u64::from(r));
            ee.set_f(22, dt.0);
            ee.call(branch, &[u64::from(net)], &[], BUDGET * 20);
            follow::path(&mut f, &mut t, dt, None, race_word, &mut rng);
            let what = format!("rider {r:#x} step {step}: {before:?}");
            let Some(theirs) = s309_follower(&ee, r, &recs, &nodes) else {
                panic!("{what}: the original's spot names another path's node");
            };
            assert_eq!(s309_bits(&theirs), s309_bits(&f), "{what}\n original {theirs:?}\n port     {f:?}");
            let (now, _, _) = s309_paths(&ee, net);
            for (k, (a, b)) in now.paths.iter().zip(&t.paths).enumerate() {
                assert_eq!((a.flags, a.dist.0.to_bits(), a.best), (b.flags, b.dist.0.to_bits(), b.best), "{what}: path {k}'s scratch words");
            }
            assert_eq!(
                (now.free_routes, now.free_parents, now.free_candidates),
                (t.free_routes, t.free_parents, t.free_candidates),
                "{what}: the pools"
            );
            assert_eq!(rng.state(), ee.read(rand_state, 8) as u64, "{what}: the generator");

            let mut count = |k: &'static str| *seen.entry(k).or_default() += 1;
            count("branches");
            if f.spot.path != before.spot.path {
                count("changed path");
            }
            if f.draws != before.draws {
                count("drew again");
            }
            if f.drift_clock == F(0.0) {
                count("new drift target");
            }
            if f.speed != before.speed {
                count("speed changed");
            }
            if f.kick_clock != before.kick_clock {
                count("side step");
            }
            if f.drift == F(0.0) && before.drift != F(0.0) {
                count("drift dropped");
            }

            // Ride on: along the heading at the target speed, on both sides alike.
            let go = f.speed.0 * dt.0;
            let p = f.position;
            f.position = Vf([p.x() + f.heading.x() * go, p.y(), p.z() + f.heading.z() * go, p.w()]);
            f.forward = f.heading;
            f.own_speed = f.speed;
            put(&mut ee, r + 0xc0, f.position);
            put(&mut ee, r + 0xb0, f.forward);
            ee.set_f32(r + 0x80, f.own_speed.0);
        }
    }
    eprintln!("sx01 path branch: {seen:?}");
    assert!(seen["branches"] == 3 * 401 && seen.get("changed path").copied().unwrap_or(0) > 3, "{seen:?}");
}
