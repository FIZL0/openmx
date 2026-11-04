//! F27 — the material table's collision sounds against the original's table update
//! (`material_hit_play`), run on S195's free09 knock state with its own table.
//!
//! Each case lays a small world in the image's free RAM — probes, their collision records, bodies or
//! chassis, the collidables they strike — and a queue of hits on the table, then runs the original's
//! update with the sound object's calls (volume, position, 3D, start) recorded instead of sounding.
//! The port's [`material_hit::play`] on the same hits must start the same sounds, in the same order,
//! at the same levels and places, and leave the queue empty.
//!
//! Without the state the tests skip (and say so).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use game_sim::material_hit::{self, Hit, Material, BIKE};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

/// The table's listener's vtable, which finds the table in the image (`+0x34`).
const LISTENER_VT: u32 = 0x0045_f694;
/// The global foliage mask the update tests a struck category against.
const FOLIAGE_MASK: u32 = 0x0046_8f48;

/// The table's fields (`material_hit_play`'s reads).
mod table {
    pub const LISTENER: u32 = 0x34;
    pub const COUNT: u32 = 0x44;
    pub const LIST: u32 = 0x4c;
    pub const FOLIAGE: u32 = 0x65;
    pub const OWNERS: u32 = 0x80;
    pub const STRUCK: u32 = 0x98;
    pub const SPEEDS: u32 = 0xb0;
}

/// A material entry's.
mod entry {
    pub const SOUND: u32 = 0x04;
    pub const NAME: u32 = 0x08;
    pub const NEAR: u32 = 0x10c;
    pub const FAR: u32 = 0x110;
    pub const MIN: u32 = 0x114;
    pub const MAX: u32 = 0x118;
}

/// Free RAM in the knock states (a 2.7 MB run of zeros).
const AREA: u32 = 0x01d0_0000;
const BUDGET: u64 = 2_000_000;

fn context(kit: &Kit, rel: &str) -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context").join(rel);
    if here.exists() { here } else { kit.ctx(rel) }
}

/// The knock state and its table; `None` with a skip line where it is not there.
fn image() -> Option<(Ee, u32)> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let state = context(&kit, &format!("states/s195-knock-2881.eeMemory{}", ".bin"));
    let mut ee = Ee::from_ram(&state)
        .map_err(|why| skip(&format!("{why} — S195's knock state (memory: s195 knock captures live in s3)")))
        .ok()?;
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let tables: Vec<u32> = (0x0010_0000..0x0200_0000u32)
        .step_by(4)
        .filter(|&a| ee.u32_at(a) == LISTENER_VT)
        .map(|a| a - table::LISTENER)
        .collect();
    assert_eq!(tables.len(), 1, "one material table in the state: {tables:x?}");
    Some((ee, tables[0]))
}

/// The table's materials as the update reads them: `(entry, id, min, max)`.
fn materials(ee: &Ee, t: u32) -> Vec<(u32, Material)> {
    let list = ee.u32_at(t + table::LIST);
    (0..ee.u32_at(t + table::COUNT))
        .map(|k| {
            let e = ee.u32_at(list + 4 * k);
            let m = Material {
                id: ee.u8_at(e),
                min_impulse: F(ee.f32_at(e + entry::MIN)),
                max_impulse: F(ee.f32_at(e + entry::MAX)),
            };
            (e, m)
        })
        .collect()
}

fn text(ee: &Ee, at: u32) -> String {
    (0..0x104).map(|k| ee.u8_at(at + k)).take_while(|&b| b != 0).map(|b| b as char).collect()
}

/// The free09 table as the game loaded it: the materials its collision meshes use, the bike's and
/// the plants', every one heard 30..125 ft, MinImpulse 5 and MaxImpulse 25 but the bike's 2 and 9.
#[test]
fn the_states_table_is_the_discs() {
    let Some((ee, t)) = image() else { return };
    let ms = materials(&ee, t);
    let ids: Vec<u8> = ms.iter().map(|(_, m)| m.id).collect();
    assert_eq!(ids, [1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 14, 16, 19, 21, 22]);
    assert_eq!(ee.u8_at(t + table::FOLIAGE), 22);
    for (e, m) in &ms {
        assert_eq!((ee.f32_at(e + entry::NEAR), ee.f32_at(e + entry::FAR)), (30.0, 125.0), "material {}", m.id);
        let want = if m.id == BIKE { (2.0, 9.0) } else { (5.0, 25.0) };
        assert_eq!((m.min_impulse.0, m.max_impulse.0), want, "material {}", m.id);
        assert!(ee.u32_at(e + entry::SOUND) != 0, "material {} has its sound", m.id);
    }
    let names: Vec<String> = ms.iter().map(|(e, _)| text(&ee, e + entry::NAME)).collect();
    assert_eq!(names[ids.iter().position(|&i| i == BIKE).unwrap()], "Impact\\bikehit1.vag");
    assert_eq!(names[ids.iter().position(|&i| i == 21).unwrap()], "Impact\\ball.vag");
    assert_eq!(names[ids.iter().position(|&i| i == 22).unwrap()], "Impact\\treehit.vag");
}

/// What one start of a sound object recorded: the sound, its volume, the place it was put.
#[derive(Clone, Debug, Default, PartialEq)]
struct Started {
    sound: u32,
    level: i32,
    at: [u32; 4],
}

#[derive(Default)]
struct Calls {
    level: Option<(u32, i32)>,
    at: Option<(u32, [u32; 4])>,
    placed: Option<u32>,
    started: Vec<Started>,
}

/// The sound object's volume, position and start, and the 3D placement, recorded instead of run.
fn hook(ee: &mut Ee, sound: u32) -> Rc<RefCell<Calls>> {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let vt = ee.u32_at(sound + 8);
    let c = calls.clone();
    ee.hooks.insert(ee.u32_at(vt + 0x30), Box::new(move |ee| {
        c.borrow_mut().level = Some((ee.r(4) as u32, ee.r(5) as i32));
    }));
    let c = calls.clone();
    ee.hooks.insert(ee.u32_at(vt + 0x5c), Box::new(move |ee| {
        let p = ee.r(5) as u32;
        c.borrow_mut().at = Some((ee.r(4) as u32, [0, 4, 8, 12].map(|k| ee.u32_at(p + k))));
    }));
    let c = calls.clone();
    ee.hooks.insert(addr("sound_3d").unwrap(), Box::new(move |ee| {
        c.borrow_mut().placed = Some(ee.r(5) as u32);
        ee.set(2, 1);
    }));
    let c = calls.clone();
    ee.hooks.insert(ee.u32_at(vt + 0x1c), Box::new(move |ee| {
        let mut c = c.borrow_mut();
        let sound = ee.r(4) as u32;
        assert_eq!(ee.r(5), 4, "started with the flag that keeps the last voice");
        let (ls, level) = c.level.take().expect("a volume before the start");
        let (ps, at) = c.at.take().expect("a position before the start");
        assert_eq!((ls, ps, c.placed.take()), (sound, sound, Some(sound)));
        c.started.push(Started { sound, level, at });
    }));
    calls
}

/// A deterministic generator: inputs only.
struct Gen(u64);

impl Gen {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 7) as f64 / (1u64 << 24) as f64
    }
}

/// One object of a case: a probe, its record, a body or a chassis, and (mostly) its own collidable.
struct Thing {
    probe: u32,
    record: u32,
    mover: u32,
    body: bool,
    collidable: Option<u32>,
    velocity: [f32; 3],
    at: [f32; 4],
}

/// A speed near the rule's edges: the thresholds, the full scale, zero, and anything up to 45.
fn speed(g: &mut Gen) -> f32 {
    match g.below(8) {
        0 => [0.0, 2.0, 5.0, 9.0, 25.0, 4.999_999_5, 1.999_999_9, 9.000_001][g.below(8)],
        1 => g.below(46) as f32,
        _ => (g.unit() * 45.0) as f32,
    }
}

fn velocity(g: &mut Gen) -> [f32; 3] {
    let s = speed(g);
    if g.chance(0.4) {
        let mut v = [0.0; 3];
        v[g.below(3)] = if g.chance(0.5) { s } else { -s };
        return v;
    }
    let d = [g.unit() - 0.5, g.unit() - 0.5, g.unit() - 0.5];
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-3);
    d.map(|c| (c / n * s as f64) as f32)
}

fn material(g: &mut Gen) -> u8 {
    match g.below(6) {
        0 => BIKE,
        1 => 0,
        2 => [8, 12, 13, 15, 17, 18, 20, 23, 200][g.below(9)],
        _ => [1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 14, 16, 19, 21, 22][g.below(15)],
    }
}

fn set_vec(ee: &mut Ee, a: u32, v: [f32; 4]) {
    for (k, c) in v.iter().enumerate() {
        ee.set_f32(a + 4 * k as u32, *c);
    }
}

fn list(ee: &mut Ee, at: u32, ptr: u32, items: &[u32]) {
    ee.set_u32(at, items.len() as u32);
    ee.set_u32(at + 8, ptr);
    ee.set_u32(at + 0x10, 64);
    for (k, &w) in items.iter().enumerate() {
        ee.set_u32(ptr + 4 * k as u32, w);
    }
}

#[test]
fn the_hit_rule_is_the_originals() {
    let Some((mut ee, t)) = image() else { return };
    let ms = materials(&ee, t);
    let table: Vec<Material> = ms.iter().map(|(_, m)| *m).collect();
    let foliage = ee.u8_at(t + table::FOLIAGE);
    let mask = ee.u32_at(FOLIAGE_MASK);
    assert_ne!(mask, 0, "a foliage mask in the state");
    let calls: Vec<_> = ms.iter().map(|(e, _)| ee.u32_at(e + entry::SOUND)).collect();
    let rec = hook(&mut ee, calls[0]);
    for &s in &calls[1..] {
        assert_eq!(ee.u32_at(s + 8), ee.u32_at(calls[0] + 8), "every material's sound is one class");
    }
    let play = addr("material_hit_play").unwrap();
    let mut g = Gen(0x0f27);
    let (mut played, mut paired, mut bike, mut carried, mut plants, mut quiet) = (0, 0, 0, 0, 0, 0);
    for case in 0..12_000 {
        // The world: 1..5 things, and 0..3 collidables nothing owns (scenery).
        let n = 1 + g.below(5);
        let mut things = Vec::new();
        let mut collidables: Vec<(u32, u8, u32)> = Vec::new();
        for k in 0..n as u32 {
            let base = AREA + k * 0x1000;
            let body = g.chance(0.5);
            let collidable = (!g.chance(0.1)).then_some(base + 0x800);
            let v = velocity(&mut g);
            let at = [(g.unit() * 400.0 - 200.0) as f32, (g.unit() * 50.0) as f32, (g.unit() * 400.0 - 200.0) as f32, 1.0];
            things.push(Thing { probe: base, record: base + 0x100, mover: base + 0x400, body, collidable, velocity: v, at });
            if let Some(c) = collidable {
                let cat = if g.chance(0.1) { mask } else { [0x8, 0x10008, 0x18008, 0x220][g.below(4)] };
                collidables.push((c, material(&mut g), cat));
            }
        }
        for k in 0..g.below(4) as u32 {
            let cat = if g.chance(0.25) { mask } else { 0x220 };
            collidables.push((AREA + 0x8000 + k * 0x100, material(&mut g), cat));
        }
        for th in &things {
            ee.set_u32(th.probe + 0x80, th.record);
            ee.set_u32(th.record + 0xf4, if th.body { th.mover } else { 0 });
            ee.set_u32(th.record + 0xf0, if th.body { 0 } else { th.mover });
            ee.set_u32(th.record + 0x1f8, th.collidable.unwrap_or(0));
            let v = [th.velocity[0], th.velocity[1], th.velocity[2], 0.0];
            set_vec(&mut ee, th.mover + if th.body { 0xb0 } else { 0x170 }, v);
            set_vec(&mut ee, th.mover + 0x60, th.at);
        }
        for &(c, m, cat) in &collidables {
            ee.set_u32(c + 0x84, cat);
            ee.write(c + 0x89, 1, u128::from(m));
        }
        if collidables.is_empty() {
            continue;
        }
        // The queue: 0..8 hits, a third answered by the struck thing's own hit back.
        let mut hits: Vec<(usize, usize)> = Vec::new();
        for _ in 0..g.below(9) {
            let a = g.below(things.len());
            let b = g.below(collidables.len());
            hits.push((a, b));
            if g.chance(0.35) {
                if let (Some(back), Some(c)) = (things.iter().position(|o| o.collidable == Some(collidables[b].0)), things[a].collidable) {
                    hits.push((back, collidables.iter().position(|x| x.0 == c).unwrap()));
                }
            }
        }
        let mut order: Vec<usize> = (0..hits.len()).collect();
        for i in (1..order.len()).rev() {
            order.swap(i, g.below(i + 1));
        }
        let hits: Vec<(usize, usize)> = order.iter().map(|&i| hits[i]).collect();
        let owners: Vec<u32> = hits.iter().map(|&(a, _)| things[a].probe).collect();
        let struck: Vec<u32> = hits.iter().map(|&(_, b)| collidables[b].0).collect();
        list(&mut ee, t + table::OWNERS, AREA + 0xa000, &owners);
        list(&mut ee, t + table::STRUCK, AREA + 0xa100, &struck);
        list(&mut ee, t + table::SPEEDS, AREA + 0xa200, &vec![0; hits.len()]);

        let mut queue: Vec<Hit> = hits
            .iter()
            .map(|&(a, b)| {
                let th = &things[a];
                let (c, m, cat) = collidables[b];
                Hit {
                    owner: th.collidable,
                    owner_material: th.collidable.map_or(0, |o| collidables.iter().find(|x| x.0 == o).unwrap().1),
                    struck: c,
                    struck_material: m,
                    struck_foliage: cat & mask != 0,
                    velocity: Vf::new(th.velocity[0], th.velocity[1], th.velocity[2], 0.0),
                    at: Vf::new(th.at[0], th.at[1], th.at[2], th.at[3]),
                }
            })
            .collect();
        for h in &queue {
            bike += usize::from(h.struck_material == BIKE);
            carried += usize::from(h.struck_material == BIKE && h.owner.is_none());
            plants += usize::from(h.struck_foliage && h.struck_material != BIKE);
        }
        let n = queue.len();
        // The register a struck bike's ownerless striker leaves the material in starts at 0.
        ee.set(23, 0);
        rec.borrow_mut().started.clear();
        ee.call(play, &[u64::from(t)], &[], BUDGET);
        let got = std::mem::take(&mut rec.borrow_mut().started);
        let want: Vec<Started> = material_hit::play(&mut queue, &table, foliage)
            .iter()
            .map(|s| Started { sound: calls[s.material], level: s.level, at: s.at.to_bits() })
            .collect();
        assert_eq!(got, want, "case {case}: hits {hits:?}");
        assert!(queue.is_empty());
        for l in [table::OWNERS, table::STRUCK, table::SPEEDS] {
            assert_eq!(ee.u32_at(t + l), 0, "case {case}: the queue is emptied");
        }
        played += got.len();
        quiet += n - got.len();
        if got.len() < n && n >= 2 {
            paired += 1;
        }
    }
    eprintln!("played {played}, not played {quiet}; cases that lost a hit {paired}; struck bikes {bike} ({carried} ownerless), plants {plants}");
    assert!(played > 5_000 && quiet > 5_000 && bike > 1_000 && carried > 50 && plants > 500);
}

/// The level and the threshold over generated speeds, against the rule written out by hand.
#[test]
fn the_level_and_threshold() {
    let m = Material { id: 1, min_impulse: F(5.0), max_impulse: F(25.0) };
    let b = Material { id: BIKE, min_impulse: F(2.0), max_impulse: F(9.0) };
    assert_eq!(material_hit::level(F(5.0), m.max_impulse), 20);
    assert_eq!(material_hit::level(F(12.5), m.max_impulse), 50);
    assert_eq!(material_hit::level(F(25.0), m.max_impulse), 100);
    assert_eq!(material_hit::level(F(80.0), m.max_impulse), 100);
    assert_eq!(material_hit::level(F(2.0), b.max_impulse), 22);
    assert_eq!(material_hit::level(F(4.5), b.max_impulse), 50);
    let mut g = Gen(27);
    for _ in 0..10_000 {
        let s = speed(&mut g);
        for t in [m, b] {
            let mut q = vec![Hit {
                owner: Some(1),
                owner_material: BIKE,
                struck: 2,
                struck_material: t.id,
                struck_foliage: false,
                velocity: Vf::new(0.0, s, 0.0, 0.0),
                at: Vf::new(1.0, 2.0, 3.0, 1.0),
            }];
            let got = material_hit::play(&mut q, &[t], 22);
            if s < t.min_impulse.0 {
                assert!(got.is_empty(), "{s} under {:?}", t.min_impulse);
            } else {
                let want = ((s / t.max_impulse.0 * 100.0) as i32).clamp(0, 100);
                assert_eq!(got.len(), 1);
                assert!((got[0].level - want).abs() <= 1, "{s}: {} against {want}", got[0].level);
            }
        }
    }
}

/// **What the table reads as a speed is the frame's impulse** (B27): f27a1's knock, frame by frame. A
/// body's speed is its shock peak (`+0xb0`), a chassis' its impact accumulator (`+0x170`), both
/// emptied every frame, so the strike sounds once though the ball and the bike stay queued on the
/// two frames after it. On 2903 the table's last entry (left in its arrays past the count) is the
/// ball's probe on the bike's frame, at the ball's shock peak to the bit, and `bbhit` was started
/// that frame at the level it gives; on 2904..2906 both impulses are under `MinImpulse` and the sound
/// object plays its one voice on: the same voice, the same place, older by a frame each time. The
/// port's [`material_hit::play`] on the pair, with those impulses, plays the same: once, then not.
#[test]
fn a_knock_sounds_once() {
    let Ok(kit) = Kit::load().map_err(|w| skip(&w)) else { return };
    let mut last: Option<(u32, [u32; 3], f32)> = None;
    for frame in 2903..=2906 {
        let state = context(&kit, &format!("states/s195-f27a1-{frame}.eeMemory{}", ".bin"));
        let Ok(ee) = Ee::from_ram(&state).map_err(|why| skip(&format!("{why} — F27a1's knock images (memory: s195 knock captures live in s3)"))) else { return };
        let t = (0x0010_0000..0x0200_0000u32).step_by(4).find(|&a| ee.u32_at(a) == LISTENER_VT).expect("the table") - table::LISTENER;
        let (e, ball_mat) = materials(&ee, t).into_iter().find(|(_, m)| m.id == 21).expect("material 21");
        let sound = ee.u32_at(e + entry::SOUND);
        assert_eq!(ee.u32_at(t + table::OWNERS), 0, "the queue is empty between frames");
        // The last entry the update read, left past the count: the ball's probe on the bike's frame.
        let ball_probe = ee.u32_at(ee.u32_at(t + table::OWNERS + 8) + 4);
        let bike_frame = ee.u32_at(ee.u32_at(t + table::STRUCK + 8) + 4);
        assert_eq!(ee.u8_at(bike_frame + 0x89), BIKE);
        let ball_body = ee.u32_at(ee.u32_at(ball_probe + 0x80) + 0xf4);
        let bike_rec = ee.u32_at(bike_frame + 0x80);
        assert_eq!(ee.u32_at(bike_rec + 0xf4), 0, "the bike's record has no body: its chassis is read");
        let chassis = ee.u32_at(bike_rec + 0xf0);
        let quad = |a: u32| Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12));
        let (peak, impact) = (quad(ball_body + 0xb0), quad(chassis + 0x170));
        let ball_col = ee.u32_at(ee.u32_at(ball_probe + 0x80) + 0x1f8);
        let pair = [
            Hit { owner: Some(bike_frame), owner_material: BIKE, struck: ball_col, struck_material: 21, struck_foliage: false, velocity: impact, at: quad(chassis + 0x60) },
            Hit { owner: Some(ball_col), owner_material: 21, struck: bike_frame, struck_material: BIKE, struck_foliage: false, velocity: peak, at: quad(ball_body + 0x60) },
        ];
        let played = material_hit::play(&mut pair.to_vec(), &[ball_mat], 22);
        let voice = ee.u32_at(sound + 0x34);
        let at = [0, 4, 8].map(|k| ee.u32_at(sound + 0x70 + k));
        let age = ee.f32_at(sound + 0x28);
        eprintln!("{frame}: ball peak {:?}, bike impact {:?}, voice {voice:x} age {age}, played {played:?}", material_hit::speed(peak), material_hit::speed(impact));
        match last {
            None => {
                let speed = ee.f32_at(ee.u32_at(t + table::SPEEDS + 8) + 4);
                assert_eq!(speed.to_bits(), material_hit::speed(peak).0.to_bits(), "the update's speed is the ball's shock peak");
                assert_eq!(age, 0.0, "bbhit started this frame");
                let level = material_hit::level(F(speed), ball_mat.max_impulse);
                assert_eq!((ee.u32_at(sound + 0x3c) as i32, level), (67, 67));
                assert_eq!(at, [0, 4, 8].map(|k| ee.u32_at(ball_body + 0x60 + k)), "placed at the ball");
                assert_eq!(played.len(), 1);
                assert_eq!((played[0].level, played[0].at.to_bits()), (67, pair[1].at.to_bits()));
            }
            Some((v, a, was)) => {
                assert!(material_hit::speed(peak) < ball_mat.min_impulse && material_hit::speed(impact) < ball_mat.min_impulse);
                assert_eq!((voice, at), (v, a), "no second play: the one voice, where it was put");
                assert!(age > was, "playing on, not restarted");
                assert!(played.is_empty());
            }
        }
        last = Some((voice, at, age));
    }
}
