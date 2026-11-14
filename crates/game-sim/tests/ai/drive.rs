// ---------------------------------------------------------------------------------------------------
// S314 — the base AI's mode drivers: the race driver (mode 2), the gate driver (mode 4), the gate
// driver's reach along its spot, and an AI brain's listener list
// ---------------------------------------------------------------------------------------------------
//
// The two drivers are the original's own, run in the interpreter against their ports over ten thousand
// cases each, every query and brain slot they call hooked at its real address: the hook logs what it was
// handed (pointers by what they point at) and plays scripted answers and writes, which the port's side
// plays too. The race driver runs once more on sx01's race image, on the three brains as they stand.
//
// S314b joins both into `ai::Racer`: on the same image the original's drivers run whole, nothing
// hooked, against `Racer::drive` on racers read from it — the race driver as the brains stand, the gate
// driver on them put in mode 4, with a stand-in gate, on the AI net and without it.

use super::*;
use game_sim::ai::drive::{self, Calls, Card, Gate, Race};
use game_sim::race::{Crossing, Seen, Spot};
use ps2_oracle::{Ee, Kit};

const D_BRAIN: u32 = 0x01c0_0000;
const D_CARD: u32 = 0x01c0_1000;
const D_RACE: u32 = 0x01c0_2000;
const D_TRACK: u32 = 0x01c0_3000;
const D_LINE: u32 = 0x01c0_4000;
const D_NET: u32 = 0x01c0_5000;
const D_GATE: u32 = 0x01c0_6000;
const D_IFACE: u32 = 0x01c0_7000;
const D_IFACE_VT: u32 = 0x01c0_8000;
/// Not code: the interface's rider assist lands here and is hooked.
const D_ASSIST: u32 = 0x01c0_9000;
const D_FIRST_PATH: u32 = 0x01c0_a000;
const D_FIRST_NODE: u32 = 0x01c0_b000;
/// The two brain words the avoidance is handed as they are.
const W150: u32 = 0x5150_0150;
const W154: u32 = 0x5154_0154;

/// What a pointer argument points at, as both sides log it.
const ID_BRAIN: u32 = 1;
const ID_NET: u32 = 2;
const ID_TRACK: u32 = 3;
const ID_SPOT: u32 = 4;
const ID_POS: u32 = 5;
const ID_SPEED: u32 = 6;
const ID_HEADING: u32 = 7;
const ID_GATE: u32 = 8;
const ID_NOW: u32 = 9;
const ID_LAST: u32 = 10;
const ID_DRAWS: u32 = 11;
const ID_IFACE: u32 = 12;

/// The objects one brain's drivers reach, by address.
#[derive(Clone, Copy, Default)]
struct Places {
    brain: u32,
    net: u32,
    track: u32,
    gate: u32,
    card: u32,
    iface: u32,
    /// The net's first path and its first node: the gate driver's look-ahead starts at them.
    first: (u32, u32),
}

impl Places {
    fn id(&self, a: u64) -> u32 {
        let a = a as u32;
        let b = self.brain;
        match a {
            _ if a == b => ID_BRAIN,
            _ if a == b + 0x220 => ID_SPOT,
            _ if a == b + 0xc0 => ID_POS,
            _ if a == b + 0x1e0 => ID_SPEED,
            _ if a == b + 0x1d0 => ID_HEADING,
            _ if a == b + 0x22c => ID_DRAWS,
            _ if a != 0 && a == self.net => ID_NET,
            _ if a != 0 && a == self.track => ID_TRACK,
            _ if a != 0 && a == self.gate => ID_GATE,
            _ if a != 0 && a == self.card + 0x60 => ID_NOW,
            _ if a != 0 && a == self.card + 0x70 => ID_LAST,
            _ if a != 0 && a == self.iface => ID_IFACE,
            _ => a,
        }
    }

    /// A spot's three words; the net's first path's start reads as the port names it.
    fn spot(&self, ee: &Ee, a: u64) -> [u32; 3] {
        let a = a as u32;
        let w = [ee.u32_at(a), ee.u32_at(a + 4), ee.u32_at(a + 8)];
        if w == [self.first.0, self.first.1, 0] { spot_words(Spot::on(0, 0, F(0.0))) } else { w }
    }
}

fn spot_words(s: Spot) -> [u32; 3] {
    [s.path, s.piece, s.along.0.to_bits()]
}

fn words_spot(w: [u32; 3]) -> Spot {
    Spot { path: w[0], piece: w[1], along: F(f32::from_bits(w[2])) }
}

fn raw(w: [u32; 4]) -> Vf {
    Vf(w.map(f32::from_bits))
}

fn bits(v: Vf) -> Vec<u32> {
    v.to_bits().to_vec()
}

fn fb(x: F) -> u32 {
    x.0.to_bits()
}

/// What the hooked calls answer and write, and what they were handed.
#[derive(Clone, Default)]
struct DriveScript {
    log: Vec<(&'static str, Vec<u32>)>,
    point: Vf,
    placed: [u32; 3],
    reach: u32,
    path_reach: u32,
    route: (Vf, u32),
    crossing: u32,
    distances: Vec<u32>,
    next_distance: usize,
    looked: [u32; 3],
    seek_reach: u32,
    hand_off: (Vf, Vf, u32),
    avoided: (Vf, u32),
    slowed: u32,
    /// The brain's two words the avoidance is handed as they are.
    words: (u32, u32),
}

impl DriveScript {
    fn distance(&mut self) -> u32 {
        let d = self.distances[self.next_distance % self.distances.len()];
        self.next_distance += 1;
        d
    }
}

/// The port's side of [`DriveScript`].
struct Mine(DriveScript);

impl Calls for Mine {
    fn spot_point(&mut self, spot: Spot) -> Vf {
        let mut w = vec![ID_TRACK];
        w.extend(spot_words(spot));
        self.0.log.push(("spot point", w));
        self.0.point
    }
    fn place(&mut self, b: &mut drive::Brain) {
        let mut w = vec![ID_NET];
        w.extend(bits(b.position));
        w.extend([ID_SPOT, 0, 1, 0]);
        self.0.log.push(("place", w));
        b.spot = words_spot(self.0.placed);
    }
    fn reach(&mut self, b: &mut drive::Brain, point: Vf, a: F, bb: F) -> F {
        let mut w = vec![ID_BRAIN, ID_NET, b.goal_word];
        w.extend(bits(point));
        w.extend([fb(a), fb(bb)]);
        self.0.log.push(("reach", w));
        F(f32::from_bits(self.0.reach))
    }
    fn path_reach(&mut self, _: &mut drive::Brain, a: F) -> F {
        self.0.log.push(("path reach", vec![ID_BRAIN, ID_NET, ID_POS, fb(a)]));
        F(f32::from_bits(self.0.path_reach))
    }
    fn route_follow(&mut self, b: &mut drive::Brain, follow: u32, dt: F, reach: F) {
        self.0.log.push(("route", vec![ID_BRAIN, ID_NET, follow, ID_SPEED, ID_HEADING, 0, fb(dt), fb(reach)]));
        b.heading = self.0.route.0;
        b.target_speed = F(f32::from_bits(self.0.route.1));
    }
    fn crossed(&mut self, _: &Gate, _: Seen) -> Crossing {
        self.0.log.push(("crossed", vec![ID_GATE, ID_NOW, ID_LAST]));
        match self.0.crossing {
            0 => Crossing::Not,
            1 => Crossing::Through,
            2 => Crossing::Beside,
            _ => Crossing::Open,
        }
    }
    fn distance(&mut self, from: Spot, to: Spot) -> F {
        let mut w = vec![ID_NET];
        w.extend(spot_words(from));
        w.extend(spot_words(to));
        w.push(1);
        self.0.log.push(("distance", w));
        F(f32::from_bits(self.0.distance()))
    }
    fn look_ahead(&mut self, b: &mut drive::Brain, from: Spot, reach: F, speed: F, limit: F) {
        let mut w = vec![fb(reach), fb(speed), fb(limit), ID_NET];
        w.extend(spot_words(from));
        w.extend([ID_SPOT, ID_POS, 0, ID_DRAWS, 0, 0]);
        self.0.log.push(("look ahead", w));
        b.spot = words_spot(self.0.looked);
    }
    fn seek_reach(&mut self, _: &mut drive::Brain, point: Vf, dir: Vf) -> F {
        let mut w = vec![ID_BRAIN];
        w.extend(bits(point));
        w.extend(bits(dir));
        self.0.log.push(("seek reach", w));
        F(f32::from_bits(self.0.seek_reach))
    }
    fn seek_hand_off(
        &mut self, b: &mut drive::Brain, point: Vf, dir: Vf, speed: F, distance: F, lead: F, cone: F, reach: F,
    ) {
        let mut w = vec![ID_BRAIN];
        w.extend(bits(point));
        w.extend(bits(dir));
        w.extend([fb(speed), fb(distance), fb(lead), fb(cone), fb(reach)]);
        self.0.log.push(("hand off", w));
        (b.aim, b.heading, b.target_speed) = (self.0.hand_off.0, self.0.hand_off.1, F(f32::from_bits(self.0.hand_off.2)));
    }
    fn avoid(&mut self, b: &mut drive::Brain) {
        self.0.log.push(("avoid", vec![ID_BRAIN, self.0.words.0, self.0.words.1, ID_POS, ID_HEADING, ID_HEADING, ID_SPEED, ID_SPEED]));
        (b.heading, b.target_speed) = (self.0.avoided.0, F(f32::from_bits(self.0.avoided.1)));
    }
    fn plan(&mut self, _: &mut drive::Brain, dt: F) {
        self.0.log.push(("plan", vec![ID_BRAIN, fb(dt)]));
    }
    fn slow(&mut self, b: &mut drive::Brain) {
        self.0.log.push(("slow", vec![ID_BRAIN, ID_SPEED]));
        b.target_speed = F(f32::from_bits(self.0.slowed));
    }
    fn assist(&mut self, b: &mut drive::Brain, dt: F) {
        self.0.log.push(("assist", vec![ID_IFACE, fb(dt), fb(b.assist_x), fb(b.assist_z)]));
    }
}

/// The hooks, at the queries' and the brain slots' real addresses, plus the interface's assist at
/// `assist`. Each logs as [`Mine`] does and plays the script.
fn hook_calls(ee: &mut Ee, script: &Rc<RefCell<DriveScript>>, places: &Rc<RefCell<Places>>, assist: u32) -> bool {
    let Some(at) = named(&[
        "path_spot_point", "path_spot", "ai_path_reach", "ai_own_path_reach", "ai_route_follow", "start_gate_crossed",
        "path_distance", "path_look_ahead", "ai_seek_reach", "ai_seek_hand_off", "ai_update_mx", "ai_trick_plan",
        "ai_speed_step",
    ]) else {
        return false;
    };
    let [spot_point, place, reach, path_reach, route, crossed, distance, look, seek_reach, hand_off, avoid, plan, slow] =
        at[..]
    else {
        unreachable!()
    };
    type Body = fn(&mut Ee, &mut DriveScript, &Places);
    let bodies: [(u32, Body); 14] = [
        (spot_point, |ee, s, p| {
            let mut w = vec![p.id(ee.r(4))];
            w.extend(p.spot(ee, ee.r(5)));
            s.log.push(("spot point", w));
            put(ee, ee.r(6) as u32, s.point);
            ee.set(2, 1);
        }),
        (place, |ee, s, p| {
            let mut w = vec![p.id(ee.r(4))];
            w.extend(got(ee, ee.r(5) as u32));
            w.extend([p.id(ee.r(6)), ee.r(7) as u32, ee.r(8) as u32, ee.r(9) as u32]);
            s.log.push(("place", w));
            let out = ee.r(6) as u32;
            for (k, v) in s.placed.iter().enumerate() {
                ee.set_u32(out + 4 * k as u32, *v);
            }
            ee.set(2, 1);
        }),
        (reach, |ee, s, p| {
            let mut w = vec![p.id(ee.r(4)), p.id(ee.r(5)), ee.r(6) as u32];
            w.extend(got(ee, ee.r(7) as u32));
            w.extend([ee.fpr[12], ee.fpr[13]]);
            s.log.push(("reach", w));
            ee.fpr[0] = s.reach;
        }),
        (path_reach, |ee, s, p| {
            s.log.push(("path reach", vec![p.id(ee.r(4)), p.id(ee.r(5)), p.id(ee.r(6)), ee.fpr[12]]));
            ee.fpr[0] = s.path_reach;
        }),
        (route, |ee, s, p| {
            let w = vec![
                p.id(ee.r(4)), p.id(ee.r(5)), ee.r(6) as u32, p.id(ee.r(7)), p.id(ee.r(8)), ee.r(9) as u32, ee.fpr[12],
                ee.fpr[13],
            ];
            s.log.push(("route", w));
            put(ee, ee.r(8) as u32, s.route.0);
            ee.set_u32(ee.r(7) as u32, s.route.1);
            ee.set(2, 1);
        }),
        (crossed, |ee, s, p| {
            s.log.push(("crossed", vec![p.id(ee.r(4)), p.id(ee.r(5)), p.id(ee.r(6))]));
            ee.set(2, u64::from(s.crossing));
        }),
        (distance, |ee, s, p| {
            let mut w = vec![p.id(ee.r(4))];
            w.extend(p.spot(ee, ee.r(5)));
            w.extend(p.spot(ee, ee.r(6)));
            w.push(ee.r(7) as u32);
            s.log.push(("distance", w));
            ee.fpr[0] = s.distance();
        }),
        (look, |ee, s, p| {
            let mut w = vec![ee.fpr[12], ee.fpr[13], ee.fpr[14], p.id(ee.r(4))];
            w.extend(p.spot(ee, ee.r(5)));
            w.extend([p.id(ee.r(6)), p.id(ee.r(7)), ee.r(8) as u32, p.id(ee.r(9)), ee.r(10) as u32, ee.r(11) as u32]);
            s.log.push(("look ahead", w));
            let out = ee.r(6) as u32;
            for (k, v) in s.looked.iter().enumerate() {
                ee.set_u32(out + 4 * k as u32, *v);
            }
            ee.set(2, 1);
        }),
        (seek_reach, |ee, s, p| {
            let mut w = vec![p.id(ee.r(4))];
            w.extend(got(ee, ee.r(5) as u32));
            w.extend(got(ee, ee.r(6) as u32));
            s.log.push(("seek reach", w));
            ee.fpr[0] = s.seek_reach;
        }),
        (hand_off, |ee, s, p| {
            let mut w = vec![p.id(ee.r(4))];
            w.extend(got(ee, ee.r(5) as u32));
            w.extend(got(ee, ee.r(6) as u32));
            w.extend([ee.fpr[12], ee.fpr[13], ee.fpr[14], ee.fpr[15], ee.fpr[16]]);
            s.log.push(("hand off", w));
            let b = p.brain;
            put(ee, b + 0x1b0, s.hand_off.0);
            put(ee, b + 0x1d0, s.hand_off.1);
            ee.set_u32(b + 0x1e0, s.hand_off.2);
            ee.set(2, 1);
        }),
        (avoid, |ee, s, p| {
            let w = vec![
                p.id(ee.r(4)), ee.r(5) as u32, ee.r(6) as u32, p.id(ee.r(7)), p.id(ee.r(8)), p.id(ee.r(9)),
                p.id(ee.r(10)), p.id(ee.r(11)),
            ];
            s.log.push(("avoid", w));
            put(ee, ee.r(8) as u32, s.avoided.0);
            ee.set_u32(ee.r(10) as u32, s.avoided.1);
            ee.set(2, 1);
        }),
        (plan, |ee, s, p| {
            s.log.push(("plan", vec![p.id(ee.r(4)), ee.fpr[12]]));
            ee.set(2, 1);
        }),
        (slow, |ee, s, p| {
            s.log.push(("slow", vec![p.id(ee.r(4)), p.id(ee.r(5))]));
            ee.set_u32(ee.r(5) as u32, s.slowed);
            ee.set(2, 1);
        }),
        (assist, |ee, s, p| {
            let b = p.brain;
            s.log.push(("assist", vec![p.id(ee.r(4)), ee.fpr[12], ee.u32_at(b + 0x1c0), ee.u32_at(b + 0x1c8)]));
            ee.set(2, 1);
        }),
    ];
    for (pc, body) in bodies {
        let (s, p) = (script.clone(), places.clone());
        ee.hooks.insert(pc, Box::new(move |ee| body(ee, &mut s.borrow_mut(), &p.borrow())));
    }
    true
}

/// The drivers' inputs as the port takes them, read off a brain in RAM: the brain, its race, its card,
/// whether it has a path net, and the addresses of what it reaches.
fn read_drive(ee: &Ee, b: u32) -> (drive::Brain, Option<Race>, Option<Card>, bool, Places) {
    let q = |a: u32| raw(got(ee, a));
    let f = |a: u32| F(ee.f32_at(a));
    let spot = |a: u32| words_spot([ee.u32_at(a), ee.u32_at(a + 4), ee.u32_at(a + 8)]);
    let brain = drive::Brain {
        flags: ee.read(b + 0x30, 8) as u64,
        speed: f(b + 0x80),
        forward: q(b + 0xb0),
        position: q(b + 0xc0),
        throttle: f(b + 0xe0),
        brake: f(b + 0xe4),
        steer: f(b + 0xf0),
        pitch: f(b + 0xf4),
        aim: q(b + 0x1b0),
        heading: q(b + 0x1d0),
        target_speed: f(b + 0x1e0),
        assist_x: f(b + 0x1c0),
        assist_z: f(b + 0x1c8),
        spot: spot(b + 0x220),
        reach_max: f(b + 0x214),
        gate_speed: f(b + 0x254),
        gate_gain: f(b + 0x258),
        gate_lead: f(b + 0x25c),
        goal_word: ee.u32_at(b + 0x31c),
        goal_speed: f(b + 0x320),
        last_speed: f(b + 0x324),
    };
    let (card_at, race_at, net) = (ee.u32_at(b + 0xd4), ee.u32_at(b + 0xd8), ee.u32_at(b + 0xdc));
    let mut places = Places { brain: b, net, card: card_at, iface: ee.u32_at(b + 0x40), ..Places::default() };
    if net != 0 {
        let first = ee.u32_at(net + 0x4c);
        places.first = (first, if first != 0 { ee.u32_at(first + 0x10) } else { 0 });
    }
    let race = (race_at != 0).then(|| {
        let track = ee.u32_at(race_at + 0x68);
        places.track = track;
        let line = ee.u32_at(track + 0x5c);
        Race {
            kind: ee.i32_at(race_at + 0x60),
            track_reach: f(track + 0x38),
            line_spot: Spot { path: ee.u32_at(line + 0x94), piece: ee.u32_at(line + 0x98), along: F(0.0) },
        }
    });
    let card = (card_at != 0).then(|| {
        let g = ee.u32_at(card_at + 0x80);
        places.gate = g;
        Card {
            flags: ee.read(card_at + 8, 8) as u64,
            to_line_kept: f(card_at + 0x10),
            to_line: f(card_at + 0x14),
            on_track: ee.u32_at(card_at + 0x40),
            seen: Seen { now: q(card_at + 0x60), last: q(card_at + 0x70) },
            kept_spot: spot(card_at + 0x94),
            gate: (g != 0).then(|| Gate {
                flags: ee.u32_at(g + 0x30),
                centre: q(g + 0x40),
                normal: q(g + 0x50),
                side: q(g + 0xb0),
                speed: f(g + 0xa4),
                spot: spot(g + 0x94),
            }),
        }
    });
    (brain, race, card, net != 0, places)
}

/// The brain's words the drivers or their scripted calls write, as RAM holds them and as the port's
/// brain holds them.
fn written(ee: &Ee, b: u32) -> Vec<(&'static str, Vec<u32>)> {
    let w = |a: u32, n: u32| (0..n).map(|k| ee.u32_at(a + 4 * k)).collect::<Vec<_>>();
    vec![
        ("flags", w(b + 0x30, 2)),
        ("controls", [w(b + 0xe0, 2), w(b + 0xf0, 2)].concat()),
        ("aim", w(b + 0x1b0, 4)),
        ("heading", w(b + 0x1d0, 4)),
        ("target speed", w(b + 0x1e0, 1)),
        ("assist", vec![ee.u32_at(b + 0x1c0), ee.u32_at(b + 0x1c8)]),
        ("spot", w(b + 0x220, 3)),
        ("goal", w(b + 0x31c, 2)),
        ("last speed", w(b + 0x324, 1)),
    ]
}

fn ours(b: &drive::Brain) -> Vec<(&'static str, Vec<u32>)> {
    vec![
        ("flags", vec![b.flags as u32, (b.flags >> 32) as u32]),
        ("controls", vec![fb(b.throttle), fb(b.brake), fb(b.steer), fb(b.pitch)]),
        ("aim", bits(b.aim)),
        ("heading", bits(b.heading)),
        ("target speed", vec![fb(b.target_speed)]),
        ("assist", vec![fb(b.assist_x), fb(b.assist_z)]),
        ("spot", spot_words(b.spot).to_vec()),
        ("goal", vec![b.goal_word, fb(b.goal_speed)]),
        ("last speed", vec![fb(b.last_speed)]),
    ]
}

/// A spot's words: none (path word 0), or two made-up pointers and a distance along.
fn some_spot(pick: &mut Pick, none: u32) -> [u32; 3] {
    if pick.chance(none) {
        let piece = if pick.chance(2) { 0 } else { 0x0100_0000 + pick.below(0x1000) as u32 * 0x10 };
        [0, piece, float(pick, 1.0).to_bits()]
    } else {
        let piece = if pick.chance(8) { 0 } else { 0x0110_0000 + pick.below(0x1000) as u32 * 0x60 };
        [0x0120_0000 + pick.below(64) as u32 * 0x40, piece, pick.range(0.0, 1.0).to_bits()]
    }
}

/// One generated world for the drivers: the brain and everything it reaches, laid out in RAM.
fn lay_out(ee: &mut Ee, pick: &mut Pick, vt: u32) -> DriveScript {
    for at in [D_BRAIN, D_CARD, D_RACE, D_TRACK, D_LINE, D_NET, D_GATE, D_IFACE, D_FIRST_PATH] {
        ee.load(at, &[0u8; 0x400]);
    }
    let b = D_BRAIN;
    ee.set_u32(b + 8, vt);
    let mut flags = pick.next();
    for bit in [0x1, 0x400, 0x800, 0x1000, drive::WATCH_GATE, 0x8000_0000] {
        if pick.chance(2) {
            flags &= !bit;
        }
    }
    if pick.chance(3) {
        flags &= 0x7fff_ffff;
    }
    ee.write(b + 0x30, 8, u128::from(flags));
    ee.set_u32(b + 0x40, D_IFACE);
    ee.set_u32(D_IFACE + 8, D_IFACE_VT);
    ee.set_u32(D_IFACE_VT + 0x18, D_ASSIST);
    ee.set_u32(b + 0x150, W150);
    ee.set_u32(b + 0x154, W154);
    let forward = if pick.chance(10) { Vf::new(1.0, 0.0, 0.0, 0.0) } else { pick.vec(1.0, 0.0) };
    put(ee, b + 0xb0, forward);
    put(ee, b + 0xc0, pick.vec(500.0, 1.0));
    for (off, span) in [(0x80, 60.0), (0xe0, 1.0), (0xe4, 1.0), (0xf0, 1.0), (0xf4, 1.0), (0x1e0, 60.0), (0x1c0, 9.0)] {
        ee.set_f32(b + off, float(pick, span));
    }
    for off in [0x1b0, 0x1d0] {
        put(ee, b + off, any_w(pick, 100.0));
    }
    for (k, w) in some_spot(pick, 3).iter().enumerate() {
        ee.set_u32(b + 0x220 + 4 * k as u32, *w);
    }
    let lead = pick.range(10.0, 80.0);
    for (off, v) in [(0x214, pick.range(20.0, 100.0)), (0x254, pick.range(5.0, 30.0)), (0x258, float(pick, 20.0)), (0x25c, lead)]
    {
        ee.set_f32(b + off, v);
    }
    ee.set_u32(b + 0x31c, pick.next() as u32);
    ee.set_f32(b + 0x320, float(pick, 100.0));
    ee.set_f32(b + 0x324, float(pick, 100.0));

    ee.set_u32(b + 0xd8, if pick.chance(12) { 0 } else { D_RACE });
    ee.set_u32(b + 0xdc, if pick.chance(4) { 0 } else { D_NET });
    let card = !pick.chance(12);
    let kind = [0, 1, 2, 3, 4, 4, 5][pick.below(7) as usize];
    // The original reads the card without a test in a fast-lap race: no card there.
    ee.set_u32(b + 0xd4, if card || kind == 4 { D_CARD } else { 0 });
    ee.set_u32(D_RACE + 0x60, kind as u32);
    ee.set_u32(D_RACE + 0x68, D_TRACK);
    ee.set_f32(D_TRACK + 0x38, if pick.chance(2) { 200.0 } else { float(pick, 300.0) });
    ee.set_u32(D_TRACK + 0x5c, D_LINE);
    ee.set_u32(D_LINE + 0x94, if pick.chance(10) { 0 } else { 0x0130_0000 });
    ee.set_u32(D_LINE + 0x98, if pick.chance(10) { 0 } else { 0x0131_0000 });
    ee.set_u32(D_NET + 0x4c, D_FIRST_PATH);
    ee.set_u32(D_FIRST_PATH + 0x10, D_FIRST_NODE);

    let mut card_flags = pick.next();
    for bit in [0x1, 0x2, 0x100] {
        if pick.chance(3) {
            card_flags &= !bit;
        }
    }
    ee.write(D_CARD + 8, 8, u128::from(card_flags));
    let to_line = if pick.chance(6) { lead } else { pick.range(0.0, 150.0) };
    ee.set_f32(D_CARD + 0x10, float(pick, 300.0));
    ee.set_f32(D_CARD + 0x14, to_line);
    ee.set_u32(D_CARD + 0x40, [0, 1, pick.next() as u32][pick.below(3) as usize]);
    put(ee, D_CARD + 0x60, pick.vec(500.0, 1.0));
    put(ee, D_CARD + 0x70, pick.vec(500.0, 1.0));
    for (k, w) in some_spot(pick, 3).iter().enumerate() {
        ee.set_u32(D_CARD + 0x94 + 4 * k as u32, *w);
    }
    ee.set_u32(D_CARD + 0x80, if pick.chance(4) { 0 } else { D_GATE });
    let gate_flags = (pick.next() as u32) & !(if pick.chance(2) { drive::SIDE_GATE } else { 0 });
    ee.set_u32(D_GATE + 0x30, gate_flags);
    put(ee, D_GATE + 0x40, any_w(pick, 500.0));
    let normal = |pick: &mut Pick| {
        if pick.chance(10) { Vf::new(0.0, 0.0, if pick.chance(2) { 1.0 } else { -1.0 }, 0.0) } else { any_w(pick, 1.0) }
    };
    let n = normal(pick);
    put(ee, D_GATE + 0x50, n);
    let s = normal(pick);
    put(ee, D_GATE + 0xb0, s);
    ee.set_f32(D_GATE + 0xa4, pick.range(5.0, 40.0));
    for (k, w) in some_spot(pick, 6).iter().enumerate() {
        ee.set_u32(D_GATE + 0x94 + 4 * k as u32, *w);
    }

    let there = pick.range(0.0, 200.0);
    let mut distances: Vec<u32> = vec![there, pick.range(0.0, 200.0), pick.range(0.0, 400.0)].iter().map(|x| x.to_bits()).collect();
    match pick.below(6) {
        0 => distances[0] = lead.to_bits(),
        1 => distances[1] = distances[0],
        _ => {}
    }
    DriveScript {
        log: Vec::new(),
        point: any_w(pick, 500.0),
        placed: some_spot(pick, 100),
        reach: pick.range(0.0, 100.0).to_bits(),
        path_reach: pick.range(0.0, 100.0).to_bits(),
        route: (any_w(pick, 1.0), pick.range(0.0, 60.0).to_bits()),
        crossing: [0, 1, 2, 2, 4][pick.below(5) as usize],
        distances,
        next_distance: 0,
        looked: some_spot(pick, 100),
        seek_reach: pick.range(0.0, 100.0).to_bits(),
        hand_off: (pick.vec(500.0, 1.0), pick.vec(1.0, 0.0), pick.range(0.0, 60.0).to_bits()),
        avoided: (any_w(pick, 1.0), pick.range(0.0, 60.0).to_bits()),
        slowed: pick.range(0.0, 60.0).to_bits(),
        words: (W150, W154),
    }
}

/// Runs one driver both ways on the world in RAM and compares the calls, the answer and the brain.
fn run_both(
    ee: &mut Ee, at: u32, four: bool, script: &Rc<RefCell<DriveScript>>, places: &Rc<RefCell<Places>>, played: DriveScript,
    dt: f32, what: &str,
) -> (bool, Vec<(&'static str, Vec<u32>)>) {
    let (mut brain, race, card, net, p) = read_drive(ee, places.borrow().brain);
    let mut played = played;
    played.words = (ee.u32_at(p.brain + 0x150), ee.u32_at(p.brain + 0x154));
    *places.borrow_mut() = p;
    *script.borrow_mut() = played.clone();
    ee.call(at, &[u64::from(p.brain)], &[dt], BUDGET);

    let before = brain;
    let mut mine = Mine(played);
    let ok = if four {
        drive::mode_four(&mut brain, race.is_some(), card.as_ref(), net, &mut mine, F(dt))
    } else {
        drive::mode_two(&mut brain, race.as_ref(), card.as_ref(), net, &mut mine, F(dt))
    };
    let theirs = script.borrow();
    assert_eq!(theirs.log, mine.0.log, "{what}: the calls, in order ({before:?}, {race:?}, {card:?})");
    assert_eq!(ee.v0(), u64::from(ok), "{what}: the answer");
    for ((name, got), (_, want)) in written(ee, p.brain).into_iter().zip(ours(&brain)) {
        assert_eq!(got, want, "{what}: {name} ({before:?}, {race:?}, {card:?})");
    }
    (ok, mine.0.log)
}

fn has(log: &[(&'static str, Vec<u32>)], name: &str) -> bool {
    log.iter().any(|(n, _)| *n == name)
}

/// `ai_race_drive` against `ai::drive::mode_two` over ten thousand ticks: no race, no card, no net, a
/// fast-lap race, a card out, the line unplaced, a goal kept and not, every tail flag on and off.
#[test]
fn the_race_driver_matches_over_ten_thousand_ticks() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_race_drive", "mx_vehicle_ai_vtable"]) else { return };
    let [two, vt] = at[..] else { unreachable!() };
    let script = Rc::new(RefCell::new(DriveScript::default()));
    let places = Rc::new(RefCell::new(Places { brain: D_BRAIN, ..Places::default() }));
    if !hook_calls(&mut ee, &script, &places, D_ASSIST) {
        return;
    }
    let mut pick = Pick(0x5314_0002_0ba5_e001);
    // held, false, out, unplaced line, goal kept, no goal, sweep up, sweep down, avoid, slow, assist
    let mut seen = [0u32; 11];
    for case in 0..10_000 {
        let played = lay_out(&mut ee, &mut pick, vt);
        let dt = if pick.chance(2) { 0.033_333_335 } else { pick.range(0.0, 0.1) };
        let before = read_drive(&ee, D_BRAIN);
        let (ok, log) = run_both(&mut ee, two, false, &script, &places, played, dt, &format!("case {case}"));
        let (b, race, card) = (before.0, before.1, before.2);
        let held = race.is_some_and(|r| r.kind == 4) && card.is_some_and(|c| c.flags & 0x100 == 0);
        seen[0] += u32::from(held);
        seen[1] += u32::from(!ok);
        seen[2] += u32::from(ok && !held && !has(&log, "reach"));
        seen[3] += u32::from(!ok && race.is_some_and(|r| r.line_spot.path == 0 || r.line_spot.piece == 0));
        seen[4] += u32::from(has(&log, "spot point"));
        seen[5] += u32::from(has(&log, "reach") && !has(&log, "spot point"));
        let routed = has(&log, "route");
        let on = card.is_some_and(|c| c.on_track != 0);
        seen[6] += u32::from(routed && b.flags & 0x1000 != 0 && on);
        seen[7] += u32::from(routed && b.flags & 0x1000 != 0 && !on);
        seen[8] += u32::from(has(&log, "avoid"));
        seen[9] += u32::from(has(&log, "slow"));
        seen[10] += u32::from(has(&log, "assist"));
    }
    println!("race driver: held, false, out, unplaced line, goal, no goal, sweep up/down, avoid, slow, assist {seen:?}");
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 100, "branch {k} reached only {n} times: {seen:?}");
    }
}

/// `ai_gate_drive` against `ai::drive::mode_four` over ten thousand ticks: no race, no card, a card out,
/// neither net nor gate; on a net placed and not, the gate watched and passed beside, the turn back
/// held or dropped; with no net plain and side gates, near and far, facing either way.
#[test]
fn the_gate_driver_matches_over_ten_thousand_ticks() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_gate_drive", "mx_vehicle_ai_vtable"]) else { return };
    let [four, vt] = at[..] else { unreachable!() };
    let script = Rc::new(RefCell::new(DriveScript::default()));
    let places = Rc::new(RefCell::new(Places { brain: D_BRAIN, ..Places::default() }));
    if !hook_calls(&mut ee, &script, &places, D_ASSIST) {
        return;
    }
    let mut pick = Pick(0x5314_0004_0ba5_e001);
    // false, out, placed, unplaced, passed beside, look back, turn back dropped, side gate, plain gate,
    // near, far, avoid, slow, assist
    let mut seen = [0u32; 14];
    for case in 0..10_000 {
        let played = lay_out(&mut ee, &mut pick, vt);
        let dt = if pick.chance(2) { 0.033_333_335 } else { pick.range(0.0, 0.1) };
        let crossing = played.crossing;
        let before = read_drive(&ee, D_BRAIN);
        let (ok, log) = run_both(&mut ee, four, true, &script, &places, played, dt, &format!("case {case}"));
        let (b, card, net) = (before.0, before.2, before.3);
        let after = ee.read(D_BRAIN + 0x30, 8) as u64;
        seen[0] += u32::from(!ok);
        seen[1] += u32::from(ok && !has(&log, "route") && !has(&log, "seek reach"));
        seen[2] += u32::from(has(&log, "route") && !has(&log, "place"));
        seen[3] += u32::from(has(&log, "place"));
        seen[4] += u32::from(has(&log, "crossed") && crossing == 2);
        seen[5] += u32::from(has(&log, "look ahead"));
        seen[6] += u32::from(has(&log, "distance") && !has(&log, "look ahead") && after & drive::TURNING_BACK == 0);
        let gate = card.and_then(|c| c.gate);
        let aimed = !net && has(&log, "seek reach");
        let side = gate.is_some_and(|g| g.flags & drive::SIDE_GATE != 0);
        seen[7] += u32::from(aimed && side);
        seen[8] += u32::from(aimed && !side);
        let near = card.is_some_and(|c| c.to_line.0 <= b.gate_lead.0);
        seen[9] += u32::from(aimed && near);
        seen[10] += u32::from(aimed && !near);
        seen[11] += u32::from(has(&log, "avoid"));
        seen[12] += u32::from(has(&log, "slow"));
        seen[13] += u32::from(has(&log, "assist"));
    }
    println!(
        "gate driver: false, out, placed, unplaced, beside, look back, dropped, side, plain, near, far, avoid, slow, \
         assist {seen:?}"
    );
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 100, "branch {k} reached only {n} times: {seen:?}");
    }
}

/// `ai_own_path_reach` against `ai::drive::path_reach` over ten thousand calls, the net's point hooked:
/// under, at and over the longest reach, with and without the card's out bit (which keeps the
/// position).
#[test]
fn the_own_path_reach_matches_over_ten_thousand_calls() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_own_path_reach", "path_spot_point"]) else { return };
    let [own, spot_point] = at[..] else { unreachable!() };
    let point = Rc::new(RefCell::new((Vf::default(), Vec::<u32>::new())));
    let p = point.clone();
    ee.hooks.insert(
        spot_point,
        Box::new(move |ee| {
            let mut p = p.borrow_mut();
            p.1 = vec![ee.r(4) as u32, ee.r(5) as u32];
            put(ee, ee.r(6) as u32, p.0);
            ee.set(2, 1);
        }),
    );
    let mut pick = Pick(0x5314_007c_0ba5_e001);
    let mut seen = [0u32; 4]; // capped, under, kept, not kept
    for case in 0..10_000 {
        ee.load(D_BRAIN, &[0u8; 0x400]);
        ee.load(D_CARD, &[0u8; 0x100]);
        let b = D_BRAIN;
        let position = any_w(&mut pick, 500.0);
        let last = pick.vec(500.0, 1.0);
        let at_point = if pick.chance(10) { position } else { add_v(position, any_w(&mut pick, 80.0)) };
        let (base, gain) = (float(&mut pick, 30.0), float(&mut pick, 5.0));
        let max = if pick.chance(8) {
            let dx = at_point.x() - position.x();
            let dz = at_point.z() - position.z();
            base + gain * (dx * dx + dz * dz).sqrt()
        } else {
            pick.range(0.0, 150.0)
        };
        let keep = pick.chance(2);
        put(&mut ee, b + 0xc0, position);
        put(&mut ee, b + 0x290, last);
        ee.set_u32(b + 0xd4, D_CARD);
        ee.set_u32(b + 0xdc, D_NET);
        ee.write(D_CARD + 8, 8, u128::from(pick.next() & !1 | u64::from(keep)));
        for (off, v) in [(0x210, base), (0x214, max), (0x218, gain)] {
            ee.set_f32(b + off, v);
        }
        point.borrow_mut().0 = at_point;
        ee.call(own, &[u64::from(b)], &[float(&mut pick, 300.0)], BUDGET);

        assert_eq!(point.borrow().1, vec![D_NET, b + 0x220], "case {case}: the point's query");
        let mut kept = last;
        let want = drive::path_reach(position, at_point, F(base), F(gain), F(max), keep, &mut kept);
        assert_eq!(ee.f0().to_bits(), want.0.to_bits(), "case {case}: the reach");
        assert_eq!(got(&ee, b + 0x290), kept.to_bits(), "case {case}: the kept position");
        seen[0] += u32::from(want.0.to_bits() == max.to_bits());
        seen[1] += u32::from(want.0.to_bits() != max.to_bits());
        seen[2] += u32::from(keep);
        seen[3] += u32::from(!keep);
    }
    println!("own path reach: capped, under, kept, not kept {seen:?}");
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 500, "branch {k} reached only {n} times: {seen:?}");
    }
}

fn add_v(a: Vf, b: Vf) -> Vf {
    Vf::new(a.x() + b.x(), a.y() + b.y(), a.z() + b.z(), a.w())
}

/// sx01's race image (S282c), its gp set, or a skip.
fn race_image() -> Option<Ee> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let path = kit.ctx("states").join(S282C_STATE);
    if !path.exists() {
        skip(&format!("{} is not there (research/c31's race start, S282c)", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").ok()?));
    Some(ee)
}

/// The race driver on sx01's race image: all three AI brains are in mode 2, and each one's tick, its
/// calls hooked as above, matches the port's on the brain, card, race and net as they stand.
#[test]
fn the_race_driver_matches_on_sx01s_race_image() {
    let Some(ee) = race_image() else { return };
    let Some(at) = named(&["ai_race_drive", "mx_vehicle_ai_vtable"]) else { return };
    let [two, vt] = at[..] else { unreachable!() };
    for brain in S282C_RIDERS {
        assert_eq!(ee.u32_at(brain + 8), vt, "{brain:#x}: a motocross brain");
        assert_eq!(ee.u32_at(brain + 0xd0), 2, "{brain:#x}: mode 2, the race driver");
        let iface = ee.u32_at(brain + 0x40);
        let assist = ee.u32_at(ee.u32_at(iface + 8) + 0x18);
        let script = Rc::new(RefCell::new(DriveScript::default()));
        let places = Rc::new(RefCell::new(Places { brain, ..Places::default() }));
        let Some(mut each) = race_image() else { return };
        if !hook_calls(&mut each, &script, &places, assist) {
            return;
        }
        let mut pick = Pick(0x5314_0282_0000_0000 | u64::from(brain));
        let mut played = lay_out_script_only(&mut pick);
        played.point = raw(got(&each, brain + 0xc0));
        let (ok, log) = run_both(&mut each, two, false, &script, &places, played, 0.033_333_335, &format!("{brain:#x}"));
        println!("{brain:#x}: {:?}", log.iter().map(|(n, _)| *n).collect::<Vec<_>>());
        assert!(ok && has(&log, "route"), "{brain:#x}: the driver rode ({log:?})");
    }
}

/// A script with no world laid out (the race image brings its own).
fn lay_out_script_only(pick: &mut Pick) -> DriveScript {
    DriveScript {
        reach: pick.range(10.0, 80.0).to_bits(),
        route: (pick.vec(1.0, 0.0), pick.range(10.0, 60.0).to_bits()),
        distances: vec![pick.range(0.0, 200.0).to_bits()],
        avoided: (pick.vec(1.0, 0.0), pick.range(10.0, 60.0).to_bits()),
        slowed: pick.range(10.0, 60.0).to_bits(),
        ..DriveScript::default()
    }
}

/// No AI brain carries a listener: in sx01's race image and in free09's free ride, every AI brain's list
/// is empty and its listening bit down, so the tick's listener walk tells no one.
#[test]
fn ai_brains_carry_no_listeners() {
    let Some(kit) = Kit::load().map_err(|why| skip(&why)).ok() else { return };
    let images: [(&str, Vec<u32>); 2] = [
        (S282C_STATE, S282C_RIDERS.to_vec()),
        ("recording2.p2m2_SaveState.eeMemory.bin", super::sim::S12E_RIDERS.iter().map(|r| r.0).collect()),
    ];
    for (name, brains) in images {
        let path = kit.ctx("states").join(name);
        if !path.exists() {
            skip(&format!("{} is not there", path.display()));
            continue;
        }
        let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
        for b in brains {
            assert_eq!(ee.u32_at(b + 8), addr("mx_vehicle_ai_vtable").unwrap(), "{name} {b:#x}: a motocross brain");
            assert_eq!(ee.u32_at(b + 0x24) & 2, 0, "{name} {b:#x}: the listening bit");
            assert_eq!(ee.u32_at(b + 0x18), 0, "{name} {b:#x}: the listener list");
        }
    }
}

/// A vector within `r` whose w is anything in -1..1.
fn any_w(pick: &mut Pick, r: f32) -> Vf {
    let w = pick.range(-1.0, 1.0);
    pick.vec(r, w)
}

/// A world the race driver must not ask.
struct NoGround;

impl World for NoGround {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        panic!("the joined race driver asked the ground at {at:?}")
    }
    fn solid(&mut self, _: &Bundle, _: &mut Record) -> bool {
        panic!("the joined race driver swept the scene")
    }
}

/// The joined race driver on sx01's race image (S314b): `ai_race_drive` run whole in the interpreter,
/// nothing hooked — the point query on the race track, the path reach and route follower on the AI
/// net, the motocross avoidance, the trick plan and the interface's assist all the original's own —
/// against `ai::Racer::drive` on a racer read from the same image, its race, card, track and net. Each
/// of the three brains drives once as it stands and then 200 more times, moved along its own heading at
/// its own target speed between (the same on both sides). Each drive: the answer, the brain's words,
/// the follower's, the trick clocks, the body's yaw rate and velocity, the net's scratch words and
/// pools, and the generator.
#[test]
fn the_joined_race_driver_rides_sx01s_race_image() {
    let Some(seen) = ride_sx01_joined(2, true, false) else { return };
    assert!(seen["drives"] == 3 * 201 && seen.get("changed path").copied().unwrap_or(0) > 3, "{seen:?}");
    assert!(seen["avoided"] > 0 && seen["kept goal"] > 0, "{seen:?}");
}

/// The joined gate driver on the same image, its brains put in mode 4 with the gate watch up: no brain
/// is in mode 4 in any image, so the race's brains stand in. `ai_gate_drive` whole — the own path reach,
/// the route follower, the start gate's crossing test, the net's distance and look-ahead, the seek
/// reach and hand-off all the original's — against `ai::Racer::drive`, as the race driver above, the
/// card's step following the ride and the seeker's words compared too.
#[test]
fn the_joined_gate_driver_rides_sx01s_race_image() {
    let Some(seen) = ride_sx01_joined(4, true, true) else { return };
    assert!(seen["drives"] == 3 * 201 && seen.get("changed path").copied().unwrap_or(0) > 3, "{seen:?}");
    assert!(seen.get("turning back").copied().unwrap_or(0) > 0, "{seen:?}");
}

/// The joined gate driver with the brains' AI net taken away, so it aims at the card's next gate and
/// the seek reach and hand-off run.
#[test]
fn the_joined_gate_driver_aims_at_the_gate_on_sx01s_race_image() {
    let Some(seen) = ride_sx01_joined(4, false, true) else { return };
    assert!(seen["drives"] == 3 * 201 && seen.get("handed off").copied().unwrap_or(0) > 0, "{seen:?}");
}

/// Drives each of sx01's race brains in `mode` 201 times, the original's driver against the joined
/// port's, with or without the brain's AI `net`, the gate watch up with `watch`. What it rode through.
fn ride_sx01_joined(mode: i32, with_net: bool, watch: bool) -> Option<std::collections::BTreeMap<&'static str, u32>> {
    use super::path_branch::{s309_bits, s309_follower, s309_paths, s309_spot};
    use super::sim::{S12eLists, s12e_bike, s12e_f, s12e_racer};
    use game_sim::ai::{Around, DriverWords, Racing};
    let image = race_image()?;
    let at = named(&["ai_race_drive", "ai_gate_drive", "rand_state"])?;
    let [two, four, rand_state] = at[..] else { unreachable!() };
    let driver = if mode == 4 { four } else { two };
    let tables = super::controls::tables(&image);
    let dt = 0.033_333_335_f32;
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    for b in S282C_RIDERS {
        let mut ee = race_image()?;
        // The stand-ins' changes, made alike to the image and (read after) the port's racer: the mode;
        // the gate watch; the avoidance down in mode 4, where it is the generic vehicle pass, whose
        // scene this test does not build;
        let mut flags = ee.read(b + 0x30, 8) as u64 | if watch { drive::WATCH_GATE } else { 0 };
        if mode == 4 {
            flags &= !game_sim::ai::follow::AVOIDS;
        }
        // no net, and the seeker's set and target words, which index the free-ride seek world, which a
        // race has none of.
        ee.set_u32(b + 0xd0, mode as u32);
        ee.set_u32(b + 0x30, flags as u32);
        ee.set_u32(b + 0x34, (flags >> 32) as u32);
        let net_at = ee.u32_at(b + 0xdc);
        if !with_net {
            ee.set_u32(b + 0xdc, 0);
        }
        for o in [0x340, 0x344, 0x348, 0x34c] {
            ee.set_u32(b + o, 0);
        }
        let iface = ee.u32_at(b + 0x40);
        let r = ee.u32_at(iface + 0xc);
        let body = ee.u32_at(r + 0x7a4);
        let race_at = ee.u32_at(b + 0xd8);
        let (track_at, card_at) = (ee.u32_at(race_at + 0x68), ee.u32_at(b + 0xd4));
        let line_at = ee.u32_at(track_at + 0x5c);
        let (mut net, recs, nodes) = s309_paths(&ee, net_at);
        let (track, track_recs, track_nodes) = s309_paths(&ee, track_at);
        let lists = S12eLists { sets: Vec::new(), targets: Vec::new() };
        let mut racer = s12e_racer(&ee, &lists, b, r);
        racer.follower = s309_follower(&ee, b, &recs, &nodes).expect("the image's spot is on the AI net");
        racer.driver = DriverWords {
            gate_gain: s12e_f(&ee, b + 0x258),
            goal_word: ee.u32_at(b + 0x31c),
            goal_speed: s12e_f(&ee, b + 0x320),
            last_speed: s12e_f(&ee, b + 0x324),
        };
        let mut bike = s12e_bike(&ee, b, r);
        if mode == 4 {
            // No card has a next gate: a copy of the track's line gate stands in, at the brain's own
            // spot on the AI net.
            let stand = 0x01f8_0000;
            assert!((0..0xc0).step_by(4).all(|k| ee.u32_at(stand + k) == 0), "{stand:#x} is free in the image");
            for k in (0..0xc0).step_by(4) {
                ee.set_u32(stand + k, ee.u32_at(line_at + k));
            }
            for k in (0..12).step_by(4) {
                ee.set_u32(stand + 0x94 + k, ee.u32_at(b + 0x220 + k));
            }
            ee.set_u32(card_at + 0x80, stand);
        }
        let gate_at = ee.u32_at(card_at + 0x80);
        let (_, race, card, _, _) = read_drive(&ee, b);
        let on_track = |at: u32| s309_spot(&ee, at, &track_recs, &track_nodes).expect("a spot on the race track");
        let race = race.map(|r| Race { line_spot: on_track(line_at + 0x94), ..r });
        let on_net = |at: u32| s309_spot(&ee, at, &recs, &nodes).expect("a spot on the AI net");
        let mut card = card.map(|c| Card {
            kept_spot: on_track(card_at + 0x94),
            gate: c.gate.map(|g| Gate { spot: on_net(gate_at + 0x94), ..g }),
            ..c
        });
        let gate = (gate_at != 0).then(|| game_sim::race::Gate {
            flags: ee.u32_at(gate_at + 0x30),
            centre: raw(got(&ee, gate_at + 0x40)),
            normal: raw(got(&ee, gate_at + 0x50)),
            across: raw(got(&ee, gate_at + 0x60)),
            width: s12e_f(&ee, gate_at + 0x70),
            height: s12e_f(&ee, gate_at + 0x74),
            reach_sq: s12e_f(&ee, gate_at + 0x7c),
        });
        assert!(mode != 4 || gate.is_some(), "{b:#x}: the card has a next gate");
        let line = (raw(got(&ee, line_at + 0x40)), raw(got(&ee, line_at + 0x50)));
        let race_word = ee.u32_at(race_at + 0x5c);
        assert_eq!((racer.brain.mode, racer.brain.flags), (mode, flags), "{b:#x}: the stand-in's words");
        bike.path_net = with_net;
        assert_eq!(racer.brain.flags & flag::KEEP_ON_TRACK, 0, "{b:#x}: the avoidance's edge push needs the track's edge");
        assert!(bike.card.is_some_and(|c| c.bike_word != 0), "{b:#x}: the motocross avoidance");
        let mut rng = Rng::from_state(ee.read(rand_state, 8) as u64);
        let mut world = NoGround;
        let mut seek_world = sk::World::default();

        for step in 0..=200 {
            let before = racer.clone();
            ee.call(driver, &[u64::from(b)], &[dt], BUDGET * 20);
            let told = {
                let mut around = Around {
                    rider: bike.rider,
                    world: &mut world,
                    seek_world: &mut seek_world,
                    tables: &tables,
                    speed_scale: bike.speed_scale,
                    slow_radius: bike.slow_radius,
                    slow_speed: bike.slow_speed,
                    player: bike.player,
                    air_cone: bike.air_cone,
                    card: bike.card,
                    race_phase: bike.race_phase,
                    path_net: bike.path_net,
                    others: &bike.others,
                    edge: None,
                    vehicle: None,
                    jump: None,
                    node: None,
                    spot: None,
                    tricks: None,
                    race_state: None,
                    live: bike.live,
                    recovery: bike.recovery,
                    levers: &mut bike.levers,
                    yaw_rate: &mut bike.yaw_rate,
                    velocity: &mut bike.velocity,
                    racing: Some(Racing {
                        race,
                        card,
                        net: with_net.then_some(&mut net),
                        track: Some(&track),
                        gate,
                        line: Some(line),
                        race_word: Some(race_word),
                    }),
                };
                racer.drive(&mut around, &mut rng, F(dt))
            };
            let what = format!("brain {b:#x} step {step}");
            assert!(told.missing.is_empty(), "{what}: {:?}", told.missing);
            assert_eq!(ee.v0(), u64::from(told.answer), "{what}: the answer");

            let mut theirs = written(&ee, b);
            theirs.retain(|(n, _)| *n != "spot");
            let rc = &racer;
            let mine = vec![
                ("flags", vec![rc.brain.flags as u32, (rc.brain.flags >> 32) as u32]),
                ("controls", vec![fb(rc.brain.throttle), fb(rc.brain.brake), fb(rc.brain.steer), fb(rc.brain.pitch)]),
                ("aim", bits(rc.seeker.aim)),
                ("heading", bits(rc.seeker.heading)),
                ("target speed", vec![fb(rc.seeker.speed)]),
                ("assist", vec![fb(rc.assist_x), fb(rc.assist_z)]),
                ("goal", vec![rc.driver.goal_word, fb(rc.driver.goal_speed)]),
                ("last speed", vec![fb(rc.driver.last_speed)]),
            ];
            for ((name, got), (_, want)) in theirs.into_iter().zip(mine) {
                assert_eq!(got, want, "{what}: {name}");
            }
            let theirs = s309_follower(&ee, b, &recs, &nodes).expect("the original's spot is on the AI net");
            let mut f = rc.follower;
            f.flags = rc.brain.flags;
            f.state = rc.seeker.state;
            (f.position, f.forward, f.own_speed) = (rc.senses.frame[3], rc.senses.frame[2], rc.senses.speed);
            (f.aim, f.heading, f.speed) = (rc.seeker.aim, rc.seeker.heading, rc.seeker.speed);
            f.reach = rc.reach;
            f.angles = Vf::new(rc.controls.aim.0, rc.controls.heading.0, rc.controls.roll.0, f.angles.w());
            assert_eq!(s309_bits(&theirs), s309_bits(&f), "{what}: the follower\n original {theirs:?}\n port     {f:?}");
            assert_eq!(
                [ee.u32_at(b + 0x15c), ee.u32_at(b + 0x200)],
                [fb(rc.plan.since), fb(rc.plan.air)],
                "{what}: the trick clocks"
            );
            assert_eq!(ee.u32_at(body + 0x284), fb(bike.yaw_rate), "{what}: the yaw rate");
            assert_eq!(got(&ee, body + 0x260).to_vec(), bits(bike.velocity), "{what}: the velocity");
            let (now, _, _) = s309_paths(&ee, net_at);
            for (k, (x, y)) in now.paths.iter().zip(&net.paths).enumerate() {
                assert_eq!((x.flags, x.dist.0.to_bits(), x.best), (y.flags, y.dist.0.to_bits(), y.best), "{what}: path {k}'s scratch words");
            }
            assert_eq!(
                (now.free_routes, now.free_parents, now.free_candidates),
                (net.free_routes, net.free_parents, net.free_candidates),
                "{what}: the pools"
            );
            assert_eq!(rng.state(), ee.read(rand_state, 8) as u64, "{what}: the generator");
            // The seeker's flag words are the brain's, compared above, and its follow word the spot's.
            let theirs = lists.seeker(&ee, b);
            assert_eq!(theirs.flags2, rc.moves, "{what}: the moves");
            let theirs = sk::Seeker { follow: rc.seeker.follow, flags: rc.seeker.flags, flags2: rc.seeker.flags2, ..theirs };
            assert_eq!(theirs, rc.seeker, "{what}: the seeker");
            assert_eq!(got(&ee, b + 0x290).to_vec(), bits(rc.senses.last_position), "{what}: the last position");

            let mut count = |k: &'static str| *seen.entry(k).or_default() += 1;
            count("drives");
            if rc.follower.spot.path != before.follower.spot.path {
                count("changed path");
            }
            if rc.follower.draws != before.follower.draws {
                count("drew again");
            }
            if rc.brain.flags & game_sim::ai::follow::AVOIDS != 0 {
                count("avoided");
            }
            if card.is_some_and(|c| c.kept_spot.path != 0) {
                count("kept goal");
            }
            if rc.brain.flags & drive::TURNING_BACK != 0 {
                count("turning back");
            }
            if rc.seeker != before.seeker && !with_net {
                count("handed off");
            }

            // Ride on: along the heading at the target speed, on both sides alike.
            let go = rc.seeker.speed.0 * dt;
            let (p, h) = (rc.senses.frame[3], rc.seeker.heading);
            let (position, speed) = (Vf::new(p.x() + h.x() * go, p.y(), p.z() + h.z() * go, p.w()), rc.seeker.speed);
            (racer.senses.frame[3], racer.senses.frame[2], racer.senses.speed) = (position, h, speed);
            put(&mut ee, b + 0xc0, position);
            put(&mut ee, b + 0xb0, h);
            ee.set_f32(b + 0x80, speed.0);
            // The card's step, which the gate test reads, is the ride's.
            if let Some(c) = card.as_mut() {
                c.seen = Seen { now: position, last: p };
                put(&mut ee, card_at + 0x60, position);
                put(&mut ee, card_at + 0x70, p);
            }
        }
    }
    eprintln!("sx01 joined mode {mode} (net {with_net}): {seen:?}");
    Some(seen)
}
