// ---------------------------------------------------------------------------------------------------
// S479 — what the original runs for the AI riders while player 1 flies: biplane.p2m2's frame whole
// ---------------------------------------------------------------------------------------------------

use super::draws::{neutral_pad, through, whole_frame};
use super::*;
use ps2_oracle::{Ee, Kit};

/// biplane.p2m2's RAM image (B9, B9a): player 1 flies the biplane on free09 among three AI riders.
const BIPLANE_STATE: &str = "states/biplane.p2m2_SaveState.eeMemory.bin";
/// Its riders in the actor list's order (research/c33/riders.py): ai, ai, the human, ai.
const RIDERS: [u32; 4] = [0x018f_7290, 0x019a_44b0, 0x0189_0cd0, 0x0188_1b10];
const HUMAN: u32 = RIDERS[2];
/// The human's body's animation object, at the human rider's `+0x64` (an AI rider's is at `+0xdb4`).
const HUMAN_LOOK: u32 = 0x009f_fc50;
/// The plane's collision node (B9a) and its five records: the plane's, then the AI bikes'; the
/// human's bike record (`0x018bfc30`) is not on it.
const PLANE_NODE: u32 = 0x0060_b200;
const NODE_RECORDS: [u32; 5] = [0x01e7_3b00, 0x018c_1bd0, 0x019a_9790, 0x0197_0060, 0x007f_fdf0];

/// The functions logged, by their first argument.
const LOGGED: [(&str, u32); 7] = [
    ("collide", 0x0027_08a0),   // collision_record_collide (record)
    ("player", 0x0027_c890),    // the player update (rider)
    ("look", 0x0024_8a70),      // rider_animation (the body's animation object)
    ("feed", 0x0027_dc80),      // score_card_feed (rider)
    ("score", 0x0026_7a90),     // score_grow (the score manager)
    ("note", 0x0030_fcb0),      // engine_sound_frame (the note)
    ("race", 0x002f_cff0),      // race_manager_tick
];

/// **What the original runs while player 1 flies** (S479): biplane.p2m2's frame run whole in the
/// interpreter (S328's recipe), twelve ticks on a neutral pad. Every tick: the plane's node collides
/// its records (the plane's, then the AI bikes': they strike what any riding bike strikes, the
/// knockable props too); every rider's update runs (the three AI riders, then the human's, whose
/// bike is out of the world); then every body's look-around — the human's first, then the AI
/// riders'; the AI riders' cards are fed and the score manager ticks; then every rider's engine note,
/// the human's parked bike's too. No race manager: a free ride has none. The human's bike's levers
/// are all 0 while they fly.
#[test]
fn s479_the_ai_riders_actors_run_while_player_1_flies() {
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
    let records: Vec<u32> = (0..ee.u32_at(PLANE_NODE + 0x30)).map(|k| ee.u32_at(ee.u32_at(PLANE_NODE + 0x38) + 4 * k)).collect();
    assert_eq!(records, NODE_RECORDS, "the plane's node");
    assert_eq!(ee.u32_at(HUMAN + 0x64), HUMAN_LOOK, "the human's body");
    for o in [0x628, 0x62c, 0x63c, 0x640, 0x644, 0x658, 0x890] {
        assert_eq!(ee.f32_at(HUMAN + o), 0.0, "the flying human's bike lever +{o:#x}");
    }
    let log: Rc<RefCell<Vec<(&'static str, u32)>>> = Rc::default();
    for (name, at) in LOGGED {
        let l = Rc::clone(&log);
        ee.hooks.insert(
            at,
            Box::new(move |ee: &mut Ee| {
                l.borrow_mut().push((name, ee.r(4) as u32));
                through(ee);
            }),
        );
    }
    let ai = [RIDERS[3], RIDERS[1], RIDERS[0]];
    let look = |r: u32| ee.u32_at(r + 0xdb4);
    let note = |r: u32| ee.u32_at(r + 0x1494);
    let mut want: Vec<(&str, u32)> = NODE_RECORDS.iter().map(|&r| ("collide", r)).collect();
    want.extend(ai.iter().chain([&HUMAN]).map(|&r| ("player", r)));
    want.push(("look", HUMAN_LOOK));
    want.extend(RIDERS.iter().filter(|&&r| r != HUMAN).map(|&r| ("look", look(r))));
    want.extend(RIDERS.iter().filter(|&&r| r != HUMAN).map(|&r| ("feed", r)));
    want.extend(RIDERS.iter().map(|&r| ("note", note(r))));
    for &r in &RIDERS {
        // Every note is on (its +0x29 bit 0), its start delay spent.
        assert_eq!(ee.u8_at(note(r) + 0x29) & 1, 1, "{r:#x}'s note is on");
    }
    let mut clock = f64::from(ee.u32_at(addr("wall_ticks").unwrap())) * 100.0;
    for tick in 0..12 {
        neutral_pad(&mut ee);
        whole_frame(&mut ee, dt, &mut clock);
        let mut got = std::mem::take(&mut *log.borrow_mut());
        // The score manager's own object: one tick, between the feeds and the notes.
        let score = got.iter().position(|c| c.0 == "score").unwrap_or_else(|| panic!("tick {tick}: no score manager tick"));
        got.remove(score);
        assert_eq!(score, want.len() - RIDERS.len(), "tick {tick}: the score manager ticks after the feeds");
        assert_eq!(got, want, "tick {tick}: what ran");
        assert!(!got.iter().any(|c| c.0 == "race"), "tick {tick}: a race manager ran");
    }
}
