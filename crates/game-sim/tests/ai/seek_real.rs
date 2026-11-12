// --- S305: the target seeker on free09's real world --------------------------------------------------
//
// recording2's free09 image holds three AI riders riding targets (state 1, no path net, no course).
// Their world is free09's own: the jumps, targets and groups `sk::load` reads off the disc (S313 proved
// them field for field against this image), with each set's pick list as the image holds it. The
// original's `ai_tick_flag` runs on each real brain in the image, stopped at the path branch and the
// tail as in S282a's oracle; nothing else is hooked, so vt+0x80/vt+0x94, rand() and the pick-list build
// are the image's own code, reached through the brain's own vtable. The port's `seek` runs on what is
// read back out of the same RAM. Then each rider is walked over its own jump — past the setup point,
// up to the lip, through the air, onto the landing short, long, wide, dead on, unsettled, out of tries,
// with a done word set — by writing its position, flags and bike into the image, and both sides step
// again. Compared every step: how it ended, the whole seeker, the four controls, every leg of every
// jump, every set's pick list, every done word, and the generator.

use super::*;
use super::sim::{S12E_RIDERS, S12eLists, s12e_seek_world};
use ps2_oracle::{Ee, Kit};

const S305_TAIL: u64 = 0x7a11;
const S305_PATH: u64 = 0x9a7;

/// The image, the world read for it, and the addresses a step needs.
struct S305 {
    ee: Ee,
    lists: S12eLists,
    /// The start zones' records, which a set's pick list points at.
    groups: Vec<u32>,
    tick: u32,
    rand_state: u32,
}

/// What the original's brain fields hold, as the seeker reads them.
fn s305_ride(ee: &Ee, b: u32) -> sk::Ride {
    let (iface, race, bike) = (ee.u32_at(b + 0x40), ee.u32_at(b + 0xd8), ee.u32_at(b + 0xd4));
    sk::Ride {
        position: sk_vf(ee, b + 0xc0),
        last_position: sk_vf(ee, b + 0x290),
        forward: sk_vf(ee, b + 0xb0),
        speed: sk_f(ee, b + 0x80),
        min_speed: sk_f(ee, b + 0x254),
        speed_scale: sk_f(ee, iface + 0x110),
        slow_radius: sk_f(ee, iface + 0x114),
        slow_speed: sk_f(ee, iface + 0x118),
        player: ee.u32_at(iface + 0x1e4) as usize,
        reach: sk::Reach { base: sk_f(ee, b + 0x210), max: sk_f(ee, b + 0x214), gain: sk_f(ee, b + 0x218) },
        race_phase: (race != 0).then(|| ee.u32_at(race + 0x60) as i32),
        bike: (bike != 0).then(|| sk::Mount { flags: ee.read(bike + 8, 8) as u64, settle: sk_f(ee, bike + 0xfc) }),
        path_net: ee.u32_at(b + 0xdc) != 0,
    }
}

impl S305 {
    /// Every set's pick list in the image, by group.
    fn picks(&self) -> Vec<Option<usize>> {
        self.lists
            .sets
            .iter()
            .map(|&s| {
                let at = self.ee.u32_at(s + 0x11c);
                (at != 0).then(|| self.groups.iter().position(|&g| g == at).unwrap_or_else(|| panic!("pick list {at:#x} is no group")))
            })
            .collect()
    }

    /// Every target's done words in the image, per slot.
    fn done(&self, slots: usize) -> Vec<Vec<bool>> {
        let ee = &self.ee;
        self.lists
            .targets
            .iter()
            .map(|&t| (0..slots as u32).map(|p| ee.u16_at(ee.u32_at(t + 0x40) + 0x30 * p + 0x20) & 2 != 0).collect())
            .collect()
    }

    /// Sets target `t`'s done word for `player`, in the image and in the port's world.
    fn set_done(&mut self, world: &mut sk::World, t: usize, player: usize) {
        let at = self.ee.u32_at(self.lists.targets[t] + 0x40) + 0x30 * player as u32 + 0x20;
        let w = self.ee.u16_at(at);
        self.ee.write(at, 2, u128::from(w | 2));
        world.targets[t].done[player] = true;
    }

    /// One tick of brain `b` on both sides, compared; gives how it ended and the seeker before it.
    fn step(&mut self, world: &mut sk::World, b: u32, what: &str) -> (sk::Seek, sk::Seeker, sk::Seeker) {
        let ee = &mut self.ee;
        let mut seeker = self.lists.seeker(ee, b);
        let before = seeker;
        let ride = s305_ride(ee, b);
        let mut out = ai::Controls {
            steer: sk_f(ee, b + 0xf0),
            pitch: sk_f(ee, b + 0xf4),
            throttle: sk_f(ee, b + 0xe0),
            brake: sk_f(ee, b + 0xe4),
            heading: F(0.0),
            aim: F(0.0),
            roll: F(0.0),
            flat_forward: Vf::new(0.0, 0.0, 0.0, 0.0),
            target_speed: F(0.0),
        };
        let state = ee.read(self.rand_state, 8) as u64;
        ee.call(self.tick, &[u64::from(b)], &[1.0 / 30.0], BUDGET);

        let mut rng = Rng::from_state(state);
        let how = sk::seek(&mut seeker, &ride, world, &mut rng, &mut out);
        let what = format!("{what}: {how:?}, state {} -> {}", before.state, seeker.state);
        let want = match how {
            sk::Seek::Still => 1,
            sk::Seek::Lost => 0,
            sk::Seek::Path => S305_PATH,
            sk::Seek::Steered => S305_TAIL,
        };
        let ee = &self.ee;
        assert_eq!(ee.v0() as u32 as u64, want, "{what}: how it ended");
        let theirs = self.lists.seeker(ee, b);
        assert_eq!(theirs, seeker, "{what}: the seeker\n  theirs {theirs:?}\n  ours   {seeker:?}");
        for (at, f, name) in [(0xf0, out.steer, "steer"), (0xf4, out.pitch, "pitch"), (0xe0, out.throttle, "throttle"), (0xe4, out.brake, "brake")] {
            assert_eq!(ee.u32_at(b + at), f.0.to_bits(), "{what}: {name}");
        }
        assert_eq!(ee.read(self.rand_state, 8) as u64, rng.state(), "{what}: the generator");
        for (i, (j, &t)) in world.jumps.iter().zip(&self.lists.targets).enumerate() {
            let hit = ee.u32_at(t + 0x3c);
            for (off, leg, name) in [(0, &j.take_off, "take-off"), (0x60, &j.landing, "landing"), (0xc0, &j.setup, "setup")] {
                let mut theirs = [0; 17];
                for k in 0..4u32 {
                    theirs[4 * k as usize..4 * k as usize + 4].copy_from_slice(&got(ee, hit + off + 0x10 * k));
                }
                theirs[16] = ee.u32_at(hit + off + 0x4c);
                let mut ours = [0; 17];
                for (k, v) in [leg.velocity, leg.aim, leg.point, leg.facing].iter().enumerate() {
                    ours[4 * k..4 * k + 4].copy_from_slice(&v.to_bits());
                }
                ours[16] = leg.learnt.0.to_bits();
                assert_eq!(theirs, ours, "{what}: jump {i} {name}");
            }
        }
        let picks: Vec<_> = world.sets.iter().map(|s| s.picks).collect();
        assert_eq!(self.picks(), picks, "{what}: the pick lists");
        let done: Vec<_> = world.targets.iter().map(|t| t.done.clone()).collect();
        assert_eq!(self.done(world.targets[0].done.len()), done, "{what}: the done words");
        (how, before, seeker)
    }

    /// Writes the rider's position (and last position) and forward, with `ground` setting or
    /// clearing the ground flag.
    fn place(&mut self, b: u32, position: Vf, last: Vf, forward: Vf, ground: bool) {
        put(&mut self.ee, b + 0xc0, position);
        put(&mut self.ee, b + 0x290, last);
        put(&mut self.ee, b + 0xb0, forward);
        let flags = self.ee.read(b + 0x30, 8) as u64;
        let flags = if ground { flags | flag::ON_GROUND } else { flags & !flag::ON_GROUND };
        self.ee.write(b + 0x30, 8, u128::from(flags));
    }

    fn set_state(&mut self, b: u32, state: i32, tries: i32) {
        self.ee.set_u32(b + 0x354, state as u32);
        self.ee.set_u32(b + 0x358, tries as u32);
    }

    fn set_settle(&mut self, b: u32, settle: f32) {
        let bike = self.ee.u32_at(b + 0xd4);
        self.ee.set_f32(bike + 0xfc, settle);
    }

    fn set_flag(&mut self, b: u32, bit: u64, on: bool) {
        let flags = self.ee.read(b + 0x30, 8) as u64;
        self.ee.write(b + 0x30, 8, u128::from(if on { flags | bit } else { flags & !bit }));
    }
}

/// A point `d` ft from `p` along the flat unit `dir` (host arithmetic: these are inputs, made once).
fn s305_off(p: Vf, dir: Vf, d: f32) -> Vf {
    Vf::new(p.x() + dir.x() * d, p.y(), p.z() + dir.z() * d, 1.0)
}

/// The flat unit from `a` to `b`, as a forward (w 0).
fn s305_dir(a: Vf, b: Vf) -> Vf {
    let (x, z) = (b.x() - a.x(), b.z() - a.z());
    let l = (x * x + z * z).sqrt();
    Vf::new(x / l, 0.0, z / l, 0.0)
}

/// **The target seeker is the original's on free09's real world, for each of recording2's three AI
/// riders, as the image holds them and walked over their own jumps.** See the head of this file.
#[test]
fn s305_the_seeker_is_the_originals_on_free09s_real_world() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let path = kit.ctx("states").join("recording2.p2m2_SaveState.eeMemory.bin");
    if !path.exists() {
        return skip(&format!("{} is not there: `./kit state ram` of recording2.p2m2_SaveState.p2s", path.display()));
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let Some(at) = named(&["ai_tick_flag", "rand_state", "ai_seek_path_branch", "ai_seek_tail", "ai_seek_reach", "ai_seek_hand_off", "ai_seek_list_build"]) else {
        return;
    };
    let [tick, rand_state, path_branch, tail, reach, hand_off, build] = at[..] else { unreachable!() };
    ee.hooks.insert(path_branch, Box::new(|ee| {
        ee.set(2, S305_PATH);
        ee.set(31, ps2_oracle::DONE as i32 as i64 as u64);
    }));
    ee.hooks.insert(tail, Box::new(|ee| {
        ee.set(2, S305_TAIL);
        ee.set(31, ps2_oracle::DONE as i32 as i64 as u64);
    }));
    let Some((loaded, lists)) = s12e_seek_world(&kit, &ee) else { return };
    let root = ee.u32_at(addr("ai_seek_sequence_root").unwrap());
    let zones = ee.u32_at(root + 0x44);
    let group_items = ee.u32_at(zones + 0x38);
    let groups = (0..ee.u32_at(zones + 0x30)).map(|i| ee.u32_at(group_items + 4 * i)).collect();
    let mut t = S305 { ee, lists, groups, tick, rand_state };

    // The image's own riders: their brains reach the seeker through the original's reach and
    // hand-off, all three are in state 1 riding a target of their own set, no path net, no course.
    let mut world = loaded.clone();
    let picks = t.picks();
    for (set, p) in world.sets.iter_mut().zip(&picks) {
        set.picks = *p;
    }
    assert!(picks.iter().all(|p| p.is_some() && *p == picks[0]), "the image's pick lists: {picks:?}");
    for &(b, _) in &S12E_RIDERS {
        let vt = t.ee.u32_at(b + 8);
        assert_eq!((t.ee.u32_at(vt + 0x80), t.ee.u32_at(vt + 0x94)), (reach, hand_off), "brain {b:#x}'s vtable");
        let s = t.lists.seeker(&t.ee, b);
        assert!(s.state == 1 && s.target.is_some() && s.target == s.jump, "brain {b:#x}: {s:?}");
        assert!(!s305_ride(&t.ee, b).path_net);
        // Every one of them has the no-learning bit: as they are, a landing teaches them nothing.
        assert!(s.flags & sk::NO_LEARN != 0, "brain {b:#x}: learns");
    }
    assert!(world.sets.iter().all(|s| s.course.is_none()) && world.sequence.is_none());
    // What free09's legs hold: only the take-off has a velocity (and a learnt count) of its own; the
    // setup and the landing are a point and a facing, so the seeker rides them at its flat-out 150.
    let zero = |v: Vf| (v.x(), v.y(), v.z()) == (0.0, 0.0, 0.0);
    assert!(world.jumps.iter().all(|j| !zero(j.take_off.velocity)), "a take-off with no velocity");
    assert!(
        world.jumps.iter().all(|j| zero(j.setup.velocity) && zero(j.landing.velocity) && j.setup.learnt.0 == 0.0 && j.landing.learnt.0 == 0.0),
        "a setup or landing with a velocity"
    );
    let pristine = (t.ee.ram.clone(), world.clone());
    let restore = |t: &mut S305, world: &mut sk::World| {
        t.ee.ram.copy_from_slice(&pristine.0);
        *world = pristine.1.clone();
    };

    // The pick-list build on the image's real owners and zones: the original's (with the owner's own
    // answer to "HitFreeride") against the port's (which the load leaves at no).
    for &s in &t.lists.sets {
        t.ee.set_u32(s + 0x11c, 0);
    }
    t.ee.call(build, &[u64::from(root)], &[], BUDGET);
    let mut built = loaded.clone();
    sk::build(&mut built);
    let theirs = t.picks();
    assert_eq!(theirs, built.sets.iter().map(|s| s.picks).collect::<Vec<_>>(), "the pick-list build");
    assert_eq!(theirs, picks, "the build now gives the list the image holds");
    restore(&mut t, &mut world);

    let mut seen = std::collections::BTreeMap::<String, u32>::new();
    let mut count = |k: &str| *seen.entry(k.to_string()).or_default() += 1;
    for (k, &(b, _)) in S12E_RIDERS.iter().enumerate() {
        let r = format!("rider {k}");
        let s0 = t.lists.seeker(&t.ee, b);
        let (target, j) = (s0.target.unwrap(), s0.jump.unwrap());
        let player = s305_ride(&t.ee, b).player;
        let jump = world.jumps[j];
        let line = s305_dir(jump.setup.point, jump.take_off.point);
        let along = s305_dir(jump.take_off.point, jump.landing.point);
        let side = Vf::new(-along.z(), 0.0, along.x(), 0.0);
        let back = |v: Vf| Vf::new(-v.x(), 0.0, -v.z(), 0.0);

        // As the image holds it, three ticks running (the rider is frozen; only the seeker moves).
        for n in 0..3 {
            let (how, _, after) = t.step(&mut world, b, &format!("{r} as is, tick {n}"));
            assert_eq!((how, after.state, after.target), (sk::Seek::Steered, 1, Some(target)), "{r} as is");
        }
        count("as is");

        // State 1, the setup crossing: 5 ft past the setup point on the ground facing the jump moves on;
        // facing away, in the air, or 25 ft past (outside the -20 cone) does not.
        for (name, d, fwd, ground, moves) in [
            ("past the setup", 5.0, line, true, true),
            ("on the setup, just past", 0.01, line, true, true),
            ("past the setup facing away", 5.0, back(line), true, false),
            ("past the setup in the air", 5.0, line, false, false),
            ("25 ft past the setup", 25.0, line, true, false),
            ("short of the setup", -5.0, line, true, false),
        ] {
            restore(&mut t, &mut world);
            let p = s305_off(jump.setup.point, line, d);
            t.place(b, p, p, fwd, ground);
            let (_, _, after) = t.step(&mut world, b, &format!("{r} {name}"));
            assert_eq!(after.state, if moves { 2 } else { 1 }, "{r} {name}");
            if moves {
                assert!(after.flags2 & sk::LEG_DONE != 0 && after.lead == F(60.0), "{r} {name}: {after:?}");
            }
            count(name);
        }

        // State 2, the run-in: inside 20 ft of the lip it takes off.
        for (name, d, flies) in [("30 ft short of the lip", -30.0, false), ("10 ft short of the lip", -10.0, true), ("on the lip", 0.0, true)] {
            restore(&mut t, &mut world);
            t.set_state(b, 2, 0);
            let p = s305_off(jump.take_off.point, line, d);
            t.place(b, p, p, line, true);
            let (_, _, after) = t.step(&mut world, b, &format!("{r} {name}"));
            assert_eq!(after.state, if flies { 3 } else { 2 }, "{r} {name}");
            if flies {
                assert!(after.flags & sk::CLOSING != 0, "{r} {name}: closing");
            }
            count(name);
        }

        // State 3, the flight: in the air nothing moves on, and the rider aims at the landing.
        restore(&mut t, &mut world);
        t.set_state(b, 3, 0);
        let mid = Vf::new(
            (jump.take_off.point.x() + jump.landing.point.x()) / 2.0,
            jump.take_off.point.y() + 20.0,
            (jump.take_off.point.z() + jump.landing.point.z()) / 2.0,
            1.0,
        );
        t.place(b, mid, s305_off(mid, along, -1.0), along, false);
        let (_, _, after) = t.step(&mut world, b, &format!("{r} in the air"));
        assert_eq!(after.state, 3, "{r} in the air");
        count("in the air");

        // State 3, landings on the ground with the bike settled (its +0xfc above 0.5): learnt from
        // when off by more than the tolerance; dropped when on it, or out of tries; a done word does
        // not count without a course.
        for (name, at, tries, settle, no_learn, done, dropped) in [
            ("a short landing", s305_off(jump.landing.point, along, -40.0), 0, 1.0, false, false, false),
            ("a long landing", s305_off(jump.landing.point, along, 40.0), 0, 1.0, false, false, false),
            ("a wide landing", s305_off(jump.landing.point, side, 40.0), 1, 1.0, false, false, false),
            ("a landing on the point", jump.landing.point, 0, 1.0, false, false, true),
            ("a landing 19 ft off", s305_off(jump.landing.point, along, -19.0), 0, 1.0, false, false, true),
            ("a short landing, out of tries", s305_off(jump.landing.point, along, -40.0), 3, 1.0, false, false, true),
            ("a short landing, done", s305_off(jump.landing.point, along, -40.0), 0, 1.0, false, true, false),
            ("a short landing, the image's settle", s305_off(jump.landing.point, along, -40.0), 0, 0.099, false, false, false),
            ("a short landing, no learning", s305_off(jump.landing.point, along, -40.0), 0, 1.0, true, false, false),
        ] {
            restore(&mut t, &mut world);
            t.set_state(b, 3, tries);
            t.set_settle(b, settle);
            t.set_flag(b, sk::NO_LEARN, no_learn);
            if done {
                t.set_done(&mut world, target, player);
            }
            t.place(b, at, s305_off(at, along, -1.0), along, true);
            let lip = world.jumps[j].take_off;
            let (_, _, after) = t.step(&mut world, b, &format!("{r} {name}"));
            let learns = settle > 0.5 && !no_learn;
            if dropped {
                assert_eq!((after.state, after.target), (0, None), "{r} {name}");
                // ...and the next tick draws a new target from the set's pick list.
                let (how, _, next) = t.step(&mut world, b, &format!("{r} {name}, then"));
                assert!(next.target.is_some() && how == sk::Seek::Steered, "{r} {name}, then: {next:?}");
                count(if next.target == Some(target) { "drew the same target" } else { "drew another target" });
            } else if learns {
                assert_eq!((after.state, after.target), (1, Some(target)), "{r} {name}");
                assert_ne!(world.jumps[j].take_off, lip, "{r} {name}: nothing learnt");
            } else {
                // Not learnt from: the landing (1 ft past it, on its line) is only crossed when the
                // landing faces the way the jump goes.
                assert_eq!(world.jumps[j].take_off, lip, "{r} {name}: learnt");
            }
            count(name);
        }

        // State 3, a landing not learnt from (the image's own settle, and the no-learning bit) that
        // crosses the landing the way it faces: back to the setup leg, the target kept. Short of it, the
        // flight goes on.
        let facing = Vf::new(jump.landing.facing.x(), 0.0, jump.landing.facing.z(), 0.0);
        for (name, from, to, crosses) in [
            ("an unsettled landing across the point", -1.0, 1.0, true),
            ("an unsettled landing short of the point", -3.0, -1.0, false),
        ] {
            restore(&mut t, &mut world);
            t.set_state(b, 3, 0);
            let (p, last) = (s305_off(jump.landing.point, facing, to), s305_off(jump.landing.point, facing, from));
            t.place(b, p, last, facing, true);
            let (_, _, after) = t.step(&mut world, b, &format!("{r} {name}"));
            assert_eq!((after.state, after.target), (if crosses { 1 } else { 3 }, Some(target)), "{r} {name}");
            count(name);
        }

        // A run of landings: eight short, eight long, then alternately, so the learnt count runs past
        // 100 (and the lip is re-aimed) and the steps halve.
        restore(&mut t, &mut world);
        t.set_flag(b, sk::NO_LEARN, false);
        for n in 0..20 {
            t.set_state(b, 3, 0);
            t.set_settle(b, 1.0);
            let d = if n < 8 || n >= 16 && n % 2 == 0 { -30.0 } else { 30.0 };
            let at = s305_off(jump.landing.point, along, d);
            t.place(b, at, s305_off(at, along, -1.0), along, true);
            let lip = world.jumps[j].take_off;
            let (how, before, after) = t.step(&mut world, b, &format!("{r} landing {n} of a run"));
            if after.aim_step != before.aim_step {
                count("steps halved");
            }
            let a = world.jumps[j].take_off.aim;
            if (a.x(), a.z()) != (lip.aim.x(), lip.aim.z()) {
                count("lip re-aimed");
            }
        }
        // And one more tick as it stands, back in state 1 with what it learnt.
        let (_, _, after) = t.step(&mut world, b, &format!("{r} after the run"));
        assert_eq!(after.state, 1, "{r} after the run");
        restore(&mut t, &mut world);
    }
    println!("S305: {seen:?}");
    for k in ["steps halved", "lip re-aimed"] {
        assert!(seen.get(k).copied().unwrap_or(0) >= 3, "only {:?} of {k}", seen.get(k));
    }
}
