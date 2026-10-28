//! **MD36:** MX vs. ATV's 360 clip state ([`air::spin_clip`]) and the turn it reads
//! ([`air::spin_turned`]), clause by clause against the PC exe's 360 step (R10): what starts, plays
//! and stops the rider's `threesixtyR`/`threesixtyL`, and why one 360 can play it twice.

use game_sim::air::{self, Spin, SPIN_CLIP};
use ps2_float::F;

const DEG: f32 = std::f32::consts::PI / 180.0;

fn deg(d: f32) -> F {
    F(d * DEG)
}

/// The clip's state after `turns`, from none, one airborne substep each.
fn run(turns: &[f32]) -> (Vec<u8>, bool) {
    let mut spin = Spin::default();
    let states = turns
        .iter()
        .map(|&t| {
            air::spin_clip(&mut spin, deg(t));
            spin.clip
        })
        .collect();
    (states, spin.came_round)
}

/// Each transition the exe has, and the ones it has not.
#[test]
fn the_clip_starts_over_35_plays_at_or_over_and_stops_under() {
    // Over 35° starts it, the next substep at or over plays it, under stops it and comes round.
    assert_eq!(run(&[10.0, 36.0, 40.0, 30.0]), (vec![0, 1, 2, 0], true));
    // Exactly 35° does not start it, and does play a started one.
    let mut spin = Spin::default();
    air::spin_clip(&mut spin, F(SPIN_CLIP));
    assert_eq!(spin.clip, 0, "35° is not over it");
    spin.clip = 1;
    air::spin_clip(&mut spin, F(SPIN_CLIP));
    assert_eq!(spin.clip, 2, "a started clip plays at 35°");
    // Exactly 35° keeps a playing clip playing: only under stops it.
    air::spin_clip(&mut spin, F(SPIN_CLIP));
    assert_eq!((spin.clip, spin.came_round), (2, false));
    // A clip only started and back under 35° stays started: not stopped, not come round — and a
    // landing on it is the mid-clip wreck (`spin.clip != 0`).
    assert_eq!(run(&[36.0, 20.0, 10.0]), (vec![1, 1, 1], false));
    // No latch: a clip that stopped starts again the next time the turn is over the angle.
    assert_eq!(run(&[36.0, 40.0, 20.0, 36.0, 40.0]), (vec![1, 2, 0, 1, 2], true));
}

/// The measure folds only a heading that wrapped downwards: a bike that took off at 352° and
/// turned up through 0 reads nearly a whole turn off, and the clip starts ~10° in.
#[test]
fn the_turn_folds_one_way_only() {
    let turned = |s: f32, h: f32| air::spin_turned(deg(s), deg(h)).0 / DEG;
    // Turning down from 8°: through 0 to 350° folds to 18°.
    assert!((turned(8.0, 350.0) - 18.0).abs() < 1e-3);
    // Turning up from 352°: 2.4° does not fold, it reads 349.6°.
    assert!((turned(352.0, 2.4) - 349.6).abs() < 1e-3);
    // Inside one lap the measure is the plain difference either way.
    assert!((turned(100.0, 140.0) - 40.0).abs() < 1e-3);
    assert!((turned(140.0, 100.0) - 40.0).abs() < 1e-3);
}
