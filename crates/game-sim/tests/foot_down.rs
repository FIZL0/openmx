//! V36 — the riding body's foot switch on its own ([`game_sim::foot_down::Feet`]) against the body
//! actor's ([`game_sim::ragdoll_body::Actor::feet`]), which S109's `tests/ragdoll_riding.rs` proves
//! tick by tick against the original's own update.
//!
//! Generated rides, not a capture: the switch is a pure function of its request and its own three
//! fields, so the cheapest proof that the two are one switch is every corner of its inputs, over runs
//! long enough for the fades to finish and restart.

use game_sim::foot_down::{Feet, Input};
use game_sim::ragdoll::{Ragdoll, Records};
use game_sim::ragdoll_body::{Actor, Request};
use game_sim::ragdoll_riding::{SLOW_SPEED, STAND_SPEED};
use game_sim::vehicle_anim::TURN_RIDING;
use ps2_float::{F, Vf};

/// The foot pins and the actor's fields, in its order: left, right.
const FOOT_HOLDS: [usize; 2] = [2, 3];

/// A small LCG, so the run is the same every time.
struct Gen(u64);

impl Gen {
    fn unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    fn one_in(&mut self, n: u32) -> bool {
        self.unit() * (n as f32) < 1.0
    }
    /// A speed near one of the switch's two thresholds as often as anywhere else.
    fn speed(&mut self) -> f32 {
        match (self.unit() * 4.0) as u32 {
            0 => STAND_SPEED.0 + self.range(-0.5, 0.5),
            1 => SLOW_SPEED.0 + self.range(-2.0, 2.0),
            2 => self.range(-3.0, 3.0),
            _ => self.range(-10.0, 110.0),
        }
    }
    /// The bike turned about a random axis by up to 70°, so its up axis is either side of the tip.
    fn quat(&mut self) -> Vf {
        let (x, y, z) = (self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0));
        let n = (x * x + y * y + z * z).sqrt().max(1e-3);
        let half = self.range(0.0, 70f32.to_radians()) / 2.0;
        let s = half.sin() / n;
        Vf::new(x * s, y * s, z * s, half.cos())
    }
}

fn actor() -> Actor {
    let doll = Ragdoll::new(Records { collisions: vec![0, 0], ..Records::default() });
    let mut a = Actor::new(doll, 2);
    a.stand_speed = STAND_SPEED;
    a.slow_speed = SLOW_SPEED;
    a.ragdoll.ride_enter(false);
    a
}

/// **The switch is the actor's**: 200 rides of 120 ticks, each tick's request drawn afresh or held
/// from the last, and after every tick the two agree on each foot's hold, fade and weight to the
/// bit, and the actor's pin gain is the weight.
#[test]
fn the_foot_switch_is_the_body_actors() {
    let mut g = Gen(0x5eed_f007);
    let (mut steps, mut stops, mut lifts, mut fading) = (0usize, 0usize, [0usize; 2], 0usize);
    for _ in 0..200 {
        let mut a = actor();
        let mut feet = Feet::default();
        let mut r = Request::default();
        for t in 0..120 {
            if t == 0 || g.one_in(3) {
                r.looking = g.one_in(20);
                r.tucked = g.one_in(20);
                r.speed = F(g.speed());
                r.rates = Vf::new(g.range(-0.2, 0.2), g.range(-2.5, 2.5), g.range(-0.5, 0.5), 0.0);
                r.pitch = F(g.unit());
                r.quat = g.quat();
            }
            let clip = if g.one_in(5) { TURN_RIDING + 1 } else { TURN_RIDING };
            let dt = F(if g.one_in(10) { 1.0 / 60.0 } else { 1.0 / 30.0 });
            a.clip = clip;
            a.feet(dt, &r);
            feet.step(dt, &Input::of_request(&r, clip));
            steps += 1;
            for k in 0..2 {
                let h = FOOT_HOLDS[k];
                assert_eq!(feet.held[k], a.ragdoll.holds[h], "tick {t} foot {k}: held");
                assert_eq!(feet.fade[k].0.to_bits(), a.foot_fade[k].0.to_bits(), "tick {t} foot {k}: fade");
                assert_eq!(feet.weight[k].0.to_bits(), a.foot_weight[k].0.to_bits(), "tick {t} foot {k}: weight");
                if feet.held[k] {
                    assert_eq!(a.ragdoll.pins[h].gains[2].to_bits(), feet.weight[k].0.to_bits(), "tick {t} foot {k}: gain");
                }
            }
            match feet.held {
                [false, false] => stops += 1,
                [false, true] => lifts[0] += 1,
                [true, false] => lifts[1] += 1,
                _ if feet.weight[0].0 < 1.0 || feet.weight[1].0 < 1.0 => fading += 1,
                _ => {}
            }
        }
    }
    println!("{steps} ticks: {stops} both feet down, {lifts:?} one foot out, {fading} fading back in");
    assert!(stops > 2000 && lifts[0] > 500 && lifts[1] > 500 && fading > 1000, "a corner went untested");
}

/// The original's capture, `context/capture/v36/feet.csv` (git-ignored), made by
/// `research/journal/2026-10-24-v36-foot-down/capture/run.py`: the first recording's state ridden
/// off, braked to a stand, backed up and ridden off again, one row a tick.
fn capture() -> Option<Vec<std::collections::HashMap<String, String>>> {
    use ps2_oracle::{Kit, skip};
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let rel = "capture/v36/feet.csv";
    let local = kit.root.join("context").join(rel);
    let path = if local.exists() { local } else { kit.ctx(rel) };
    if !path.exists() {
        skip(&format!("{} is not there (V36's capture: research/journal/2026-10-24-v36-foot-down)", path.display()));
        return None;
    }
    let text = std::fs::read_to_string(path).expect("read the capture");
    let mut lines = text.lines();
    let cols: Vec<String> = lines.next().expect("a header").split(',').map(str::to_string).collect();
    Some(lines.map(|l| cols.iter().cloned().zip(l.split(',').map(str::to_string)).collect()).collect())
}

/// **The original's own stop**: stepped from each captured tick's riding request, the switch lets go
/// of the original's two foot pins on the very ticks it did, and its fades and the held pins' gains
/// are the original's to the bit, through a ride off, a stop, a stand, backing up and a ride off again.
#[test]
fn the_original_lets_its_feet_go_where_the_switch_does() {
    let Some(cap) = capture() else { return };
    let f = |r: &std::collections::HashMap<String, String>, c: &str| F(r[c].parse::<f64>().unwrap_or_else(|_| panic!("{c}: {:?}", r[c])) as f32);
    let on = |r: &std::collections::HashMap<String, String>, c: &str| r[c] != "0";
    let first = &cap[0];
    let mut feet = Feet {
        held: [on(first, "pin2_on"), on(first, "pin3_on")],
        fade: [f(first, "foot_fade.x"), f(first, "foot_fade.y")],
        weight: [f(first, "pin2_gains.z"), f(first, "pin3_gains.z")],
    };
    let (mut stops, mut changes) = (0usize, 0usize);
    for (i, r) in cap.iter().enumerate().skip(1) {
        assert_eq!(r["bike_ctl_src"], "1", "row {i} is not the human's bike");
        let gap = r["frame_counter"].parse::<u32>().unwrap() - cap[i - 1]["frame_counter"].parse::<u32>().unwrap();
        assert_eq!(gap, 1, "row {i}: a tick is missing");
        let input = Input {
            looking: on(r, "req_looking"),
            tucked: on(r, "req_tucked"),
            speed: f(r, "req_speed"),
            yaw: f(r, "req_rates.y"),
            clip: r["clip"].parse().unwrap(),
            mix: f(r, "req_pitch"),
            quat: Vf::new(f(r, "req_quat.x").0, f(r, "req_quat.y").0, f(r, "req_quat.z").0, f(r, "req_quat.w").0),
        };
        let was = feet.held;
        // The original's own tick, 0.03333 s: a hair under a thirtieth, which is what its fades step by.
        feet.step(F(0.033_33), &input);
        let want = [on(r, "pin2_on"), on(r, "pin3_on")];
        assert_eq!(feet.held, want, "row {i} (frame {}): {input:?}", r["frame_counter"]);
        assert_eq!(feet.fade.map(|x| x.0.to_bits()), [f(r, "foot_fade.x").0.to_bits(), f(r, "foot_fade.y").0.to_bits()], "row {i}: fades");
        for (k, pin) in ["pin2_gains.z", "pin3_gains.z"].into_iter().enumerate() {
            if feet.held[k] {
                assert_eq!(feet.weight[k].0.to_bits(), f(r, pin).0.to_bits(), "row {i}: {pin}");
            }
        }
        stops += usize::from(!feet.held[0]);
        changes += usize::from(feet.held != was);
    }
    println!("{} ticks: {stops} with the feet let go, {changes} switches", cap.len());
    assert!(stops > 100 && changes >= 3, "the capture never stopped and moved off ({stops} stopped ticks, {changes} switches)");
}
