// ---------------------------------------------------------------------------------------------------
// S12e — the rider-to-brain copy
// ---------------------------------------------------------------------------------------------------

use super::*;

/// The rider, its bike's frame object (rider +0x18, rows at +0x80), its body (rider +0x7a4) and its
/// brain (rider +0x10d0), in RAM nothing else uses.
const FEED_RIDER: u32 = 0x0196_0000;
const FEED_FRAME: u32 = 0x0198_0000;
const FEED_BODY: u32 = 0x0198_1000;
const FEED_BRAIN: u32 = 0x0198_2000;
/// Never read: the ground query is hooked.
const FEED_WORLD: u32 = 0x0198_3000;

/// The ground both sides are asked: one answer per query in turn, and the points they were asked at.
#[derive(Default)]
struct Answers {
    heights: Vec<f32>,
    normals: Vec<Vf>,
    asked: Vec<[u32; 4]>,
}

/// The port's side of [`Answers`].
struct Asked(Answers);

impl World for Asked {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let k = self.0.asked.len();
        self.0.asked.push(at.to_bits());
        (Vf::new(at.x(), self.0.heights[k], at.z(), at.w()), self.0.normals[k])
    }
    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// The rider-to-brain copy against the port over ten thousand frames: the original `ai_feed` with
/// `ground_query` hooked to answer generated grounds (the query itself is S64's) and the original's
/// `rand()`. Every brain field it writes is compared bit for bit, with the generator's state, and the
/// points the ground was asked at.
#[test]
fn the_rider_to_brain_copy_matches_over_ten_thousand_frames() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_feed", "ground_query", "rand_state"]) else { return };
    let [feed_fn, query, rand_state] = at[..] else { unreachable!() };

    let script = Rc::new(RefCell::new(Answers::default()));
    let s = script.clone();
    ee.hooks.insert(
        query,
        Box::new(move |ee| {
            let mut s = s.borrow_mut();
            let (point, normal) = (ee.r(5) as u32, ee.r(6) as u32);
            let k = s.asked.len();
            s.asked.push(got(ee, point));
            ee.set_f32(point + 4, s.heights[k]);
            if normal != 0 {
                put(ee, normal, s.normals[k]);
            }
            ee.set(2, 1);
        }),
    );

    let mut pick = Pick(0x51e2_feed_0b0d_ca7e);
    // on the ground; rising past 5; forward past 30; the roll passes; the no-brain case is not the port's
    let mut seen = [0u32; 6];
    for case in 0..10_000 {
        let frame: [Vf; 4] = core::array::from_fn(|_| {
            let w = float(&mut pick, 2.0);
            pick.vec(400.0, w)
        });
        let w = float(&mut pick, 2.0);
        let mut velocity = pick.vec(60.0, w);
        if pick.chance(3) {
            let y = [5.0, -5.0, 5.000_001, 4.999_999, 0.0, -0.0][pick.below(6) as usize];
            velocity = Vf::new(velocity.x(), y, velocity.z(), velocity.w());
        } else if pick.chance(2) {
            velocity = Vf::new(velocity.x(), pick.range(4.0, 30.0), velocity.z(), velocity.w());
        }
        let w = float(&mut pick, 2.0);
        let acceleration = pick.vec(100.0, w);
        let w = float(&mut pick, 2.0);
        let spin = pick.vec(8.0, w);
        let forward_speed = match pick.below(4) {
            0 => [30.0, 30.000_002, 29.999_998, -30.0][pick.below(4) as usize],
            1 => float(&mut pick, 80.0),
            _ => pick.range(20.0, 80.0),
        };
        let rider = Rider {
            frame,
            velocity,
            acceleration,
            spin,
            forward_speed: F(forward_speed),
            on_ground: pick.below(3) != 0,
            tricking: pick.chance(3),
            look_ahead: F(float(&mut pick, 1.0)),
            bump_chance: F([0.0, 100.0, 10.0, 50.0][pick.below(4) as usize] + float(&mut pick, 1.0)),
        };
        let mut brain = Senses {
            last_position: pick.vec(400.0, 1.0),
            frame: core::array::from_fn(|_| pick.vec(400.0, 1.0)),
            velocity: pick.vec(60.0, 0.0),
            acceleration: pick.vec(60.0, 0.0),
            spin: pick.vec(8.0, 0.0),
            speed: F(pick.range(0.0, 60.0)),
            ground_normal: pick.vec(1.0, 0.0),
            ground: F(pick.range(-50.0, 50.0)),
            flags: pick.next(),
            bumps: pick.next(),
        };
        let heights = vec![pick.range(-400.0, 400.0), float(&mut pick, 400.0)];
        let w = float(&mut pick, 1.0);
        let normals = vec![pick.vec(1.0, w), pick.vec(1.0, 0.0)];
        let state = pick.next();

        ee.load(FEED_RIDER, &[0u8; 0x1200]);
        ee.load(FEED_BRAIN, &[0u8; 0x400]);
        ee.load(FEED_FRAME, &[0u8; 0x100]);
        ee.load(FEED_BODY, &[0u8; 0x300]);
        ee.set_u32(FEED_RIDER + 0x18, FEED_FRAME);
        ee.set_u32(FEED_RIDER + 0x7a4, FEED_BODY);
        ee.set_u32(FEED_RIDER + 0x7ac, FEED_WORLD);
        ee.set_u32(FEED_RIDER + 0x10d0, FEED_BRAIN);
        ee.set_u32(FEED_RIDER + 0x574, u32::from(rider.on_ground));
        ee.set_u32(FEED_RIDER + IN_PROGRESS, u32::from(rider.tricking));
        ee.set_f32(FEED_RIDER + 0x11d0, rider.look_ahead.to_f32());
        ee.set_f32(FEED_RIDER + 0x1100, rider.bump_chance.to_f32());
        for (k, row) in rider.frame.iter().enumerate() {
            put(&mut ee, FEED_FRAME + 0x80 + 0x10 * k as u32, *row);
        }
        put(&mut ee, FEED_BODY + 0x260, rider.velocity);
        put(&mut ee, FEED_BODY + 0x70, rider.acceleration);
        put(&mut ee, FEED_BODY + 0x290, rider.spin);
        ee.set_f32(FEED_BODY + 0x278, forward_speed);
        put(&mut ee, FEED_BRAIN + 0x290, brain.last_position);
        for (k, row) in brain.frame.iter().enumerate() {
            put(&mut ee, FEED_BRAIN + 0x90 + 0x10 * k as u32, *row);
        }
        put(&mut ee, FEED_BRAIN + 0x50, brain.velocity);
        put(&mut ee, FEED_BRAIN + 0x60, brain.acceleration);
        put(&mut ee, FEED_BRAIN + 0x70, brain.spin);
        ee.set_f32(FEED_BRAIN + 0x80, brain.speed.to_f32());
        put(&mut ee, FEED_BRAIN + 0x2c0, brain.ground_normal);
        ee.set_f32(FEED_BRAIN + 0x204, brain.ground.to_f32());
        ee.write(FEED_BRAIN + 0x30, 8, u128::from(brain.flags));
        ee.write(FEED_BRAIN + 0x38, 8, u128::from(brain.bumps));
        ee.write(rand_state, 8, u128::from(state));
        *script.borrow_mut() = Answers { heights: heights.clone(), normals: normals.clone(), asked: Vec::new() };

        ee.call(feed_fn, &[u64::from(FEED_RIDER)], &[], BUDGET);
        assert_eq!(ee.v0(), 1, "case {case}: the copy returns 1 with a brain");

        let mut rng = Rng::from_state(state);
        let mut world = Asked(Answers { heights, normals, asked: Vec::new() });
        feed(&rider, &mut brain, &mut world, &mut rng);

        let theirs = script.borrow();
        assert_eq!(theirs.asked.len(), 2, "case {case}: the original asks the ground twice");
        assert_eq!(world.0.asked.len(), 2, "case {case}: the port asks the ground twice");
        // The first point's w is the original's stack, never set.
        assert_eq!(theirs.asked[0][..3], world.0.asked[0][..3], "case {case}: the look-ahead point");
        assert_eq!(theirs.asked[1], world.0.asked[1], "case {case}: the point under the bike");
        let quads = [
            ("last position", 0x290, brain.last_position),
            ("side", 0x90, brain.frame[0]),
            ("up", 0xa0, brain.frame[1]),
            ("forward", 0xb0, brain.frame[2]),
            ("position", 0xc0, brain.frame[3]),
            ("velocity", 0x50, brain.velocity),
            ("acceleration", 0x60, brain.acceleration),
            ("spin", 0x70, brain.spin),
            ("ground normal", 0x2c0, brain.ground_normal),
        ];
        for (name, off, ours) in quads {
            assert_eq!(got(&ee, FEED_BRAIN + off), ours.to_bits(), "case {case}: {name}");
        }
        assert_eq!(ee.u32_at(FEED_BRAIN + 0x80), brain.speed.to_f32().to_bits(), "case {case}: speed");
        assert_eq!(ee.u32_at(FEED_BRAIN + 0x204), brain.ground.to_f32().to_bits(), "case {case}: ground");
        assert_eq!(ee.read(FEED_BRAIN + 0x30, 8) as u64, brain.flags, "case {case}: flags");
        assert_eq!(ee.read(FEED_BRAIN + 0x38, 8) as u64, brain.bumps, "case {case}: bump flags");
        assert_eq!(ee.read(rand_state, 8) as u64, rng.state(), "case {case}: the generator");

        let v = rider.velocity;
        let rising = !(v.y() <= 5.0);
        let fast = !(forward_speed <= 30.0);
        seen[0] += u32::from(rider.on_ground);
        seen[1] += u32::from(rider.on_ground && rising);
        seen[2] += u32::from(rider.on_ground && rising && fast);
        seen[3] += u32::from(brain.bumps & (bump::A | bump::B) != 0);
        seen[4] += u32::from(rider.on_ground && rising && fast && brain.bumps & bump::A == 0);
        seen[5] += u32::from(rider.tricking);
    }
    println!(
        "on the ground {}, rising {}, rolled {}, bumped {}, roll failed {}, tricking {}",
        seen[0], seen[1], seen[2], seen[3], seen[4], seen[5]
    );
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 100, "branch {k} reached only {n} times: {seen:?}");
    }

    // No brain attached: nothing is copied and nothing is drawn.
    ee.set_u32(FEED_RIDER + 0x10d0, 0);
    ee.write(rand_state, 8, 0x1234);
    script.borrow_mut().asked.clear();
    ee.call(feed_fn, &[u64::from(FEED_RIDER)], &[], BUDGET);
    assert_eq!(ee.v0(), 0);
    assert_eq!(ee.read(rand_state, 8), 0x1234);
    assert!(script.borrow().asked.is_empty());
}
