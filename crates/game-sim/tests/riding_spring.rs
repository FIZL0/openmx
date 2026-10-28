//! S135 — **the riding spring after a put-back**, against S74's captures.
//!
//! From the third tick after a put-back on, the original's rider body is mode 1 again: its eighteen
//! part nodes are no longer the animation's joints but the riding ragdoll's bones, pulled towards
//! them by the riding pose spring (the body update's riding mode, the spring in the ragdoll's force
//! pass) and written back into the nodes after each step. S109 proved the port's
//! [`Actor::tick`](game_sim::ragdoll_body::Actor::tick) one tick at a time from RAM; this runs the
//! port's [`Riding`] on its own, carried from the put-back's hold, and holds its nodes against the
//! original's node world rows (`bones_watch.py`: node<j> +0x80..+0xbf) tick after tick, through the
//! 3 s ghost and the plain riding after it, on all nine put-backs of S74's five captures.
//!
//! What the port is handed each tick is only what the capture holds: the request block the body read,
//! the tick's step, and the animated skeleton built from the capture's 22 joints (each joint's
//! quaternion, the root's translation, every other node's fixed offset under its parent, under the
//! bike's node as it stood last tick: the part sync and the animation run before this tick's bike
//! moves its node), which is what the original's part sync and animation put in the nodes before the
//! step. The ground is free09's terrain from the disc — S66's states are all free09, the ones named
//! free06 too — with no props or scene meshes. Test only — the drawn rider stays the clip pose
//! (user, 2026-10-24).
//!
//! The captures are `context/s74/*.txt` (git-ignored, S74's journal); without them the test says so
//! and passes.

use game_sim::math::{self, Mat4};
use game_sim::ragdoll_body::Request;
use game_sim::ragdoll_pose::Bone;
use game_sim::ragdoll_ride::{Kit, Posed};
use game_sim::ragdoll_riding::Riding;
use ps2_float::{F, Vf};
use ps2_oracle::skip;
use std::collections::HashMap;

const JOINTS: usize = 22;

/// How far a part's node may be from the original's in the riding entry (T+2..T+5, the first
/// [`ENTRY`] riding ticks after a put-back), in feet (position) and in a row of its orientation (each
/// lane of the three unit rows). The original's bones enter about 0.15 ft further behind the animated
/// skeleton than the port's and close the gap over three or four ticks; why is not known (journal).
const ENTRY: u32 = 4;
const ENTRY_AT: f32 = 0.3;
const ENTRY_TURN: f32 = 0.15;
/// The same once settled, on every later tick of the ghost and the plain riding. The worst of both are
/// the arms after the two put-backs that come straight off a wreck (about 0.16 ft and 0.21), which
/// start here from a fresh body; the other seven and the openings stay within 0.08 ft and 0.1.
const NEAR_AT: f32 = 0.2;
const NEAR_TURN: f32 = 0.25;
/// The mean over every settled part and tick, in feet (steady riding sits near 0.002 ft).
const MEAN_AT: f32 = 0.01;

/// One tick of an S74 bones capture: each region the recorder read, by name, as its bytes.
struct Row {
    frame: u32,
    fields: HashMap<String, Vec<u8>>,
}

impl Row {
    fn bytes(&self, name: &str) -> &[u8] {
        self.fields.get(name).unwrap_or_else(|| panic!("frame {}: no {name}", self.frame))
    }
    fn word(&self, name: &str, at: usize) -> u32 {
        u32::from_le_bytes(self.bytes(name)[at..at + 4].try_into().unwrap())
    }
    fn float(&self, name: &str, at: usize) -> f32 {
        f32::from_bits(self.word(name, at))
    }
    fn quad(&self, name: &str, at: usize) -> Vf {
        Vf(std::array::from_fn(|k| self.float(name, at + 4 * k)))
    }
    fn mat(&self, name: &str, at: usize) -> Mat4 {
        Mat4(std::array::from_fn(|r| self.quad(name, at + 0x10 * r)))
    }
    /// Node `j`'s local and world rows.
    fn local(&self, j: usize) -> Mat4 {
        self.mat(&format!("node{j}"), 0x40)
    }
    fn world(&self, j: usize) -> Mat4 {
        self.mat(&format!("node{j}"), 0x80)
    }
    /// The bike's node, which the skeleton's root hangs under (its local rows: it is a root).
    fn bike(&self) -> Mat4 {
        self.mat("bike.node", 0)
    }
    /// The body's mode word (`+0x47c`): 0 held, 1 riding, 2 wrecked.
    fn mode(&self) -> u32 {
        self.word("body.47c", 0)
    }
    fn joint(&self, j: usize) -> (Vf, Vf) {
        (self.quad("joints", 0x60 * j + 0x40), self.quad("joints", 0x60 * j + 0x50))
    }
    /// The request block, as the body read it this tick (`ragdoll_riding.rs`' `req_at` offsets).
    fn request(&self) -> Request {
        let on = |o: usize| self.word("req", o) != 0;
        let q = |o: usize| self.quad("req", o);
        Request {
            live: on(0x08),
            held: on(0x0c),
            rows: [0x10, 0x20, 0x30].map(q),
            frame: self.mat("req", 0x50),
            velocity: q(0x90),
            rates: q(0xa0),
            hands: [q(0xb0), q(0xc0)],
            feet: [q(0xd0), q(0xe0)],
            // The box the request points at is the constructor's, which `Riding::new` already holds.
            bike_box: None,
            looking: on(0xf8),
            posed: on(0xfc),
            reason: self.word("req", 0x104) as i32,
            cut: on(0x140),
            hit_velocity: q(0x150),
            tucked: on(0x16c),
            pitch: F(self.float("req", 0x170)),
            speed: F(self.float("req", 0x178)),
            centre: q(0x180),
            quat: q(0x190),
            // Riding reads only that the collider is there (the riding entry's bike flag).
            bike: (self.word("req", 0x1ac) != 0).then_some((Vf::ZERO, F::ZERO)),
            ..Request::default()
        }
    }
}

/// A capture's rows, or `None` when this checkout has not got it.
fn capture(name: &str) -> Option<Vec<Row>> {
    let kit = ps2_oracle::Kit::load().map_err(|why| skip(&why)).ok()?;
    let local = kit.root.join("context").join("s74");
    let dir = if local.exists() { local } else { kit.ctx("s74") };
    let Ok(text) = std::fs::read_to_string(dir.join(format!("{name}.txt"))) else {
        println!("no context/s74/{name}.txt: no riding-spring verdict for it");
        return None;
    };
    let hex = |s: &str| -> Vec<u8> { (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect() };
    let mut rows: Vec<Row> = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') {
            continue;
        } else if let Some(f) = line.strip_prefix("f ") {
            rows.push(Row { frame: f.parse().unwrap(), fields: HashMap::new() });
        } else if let (Some((k, v)), Some(row)) = (line.split_once(' '), rows.last_mut()) {
            row.fields.insert(k.to_string(), hex(v));
        }
    }
    rows.retain(|r| r.fields.contains_key(&format!("node{}", JOINTS - 1)));
    Some(rows)
}

/// The ride kit off the disc as the shell builds it, and which skeleton node each part is.
fn ride_kit() -> Option<(Kit, Vec<usize>)> {
    use game_render::disc::Disc;
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let names: Vec<String> =
        disc.entries().iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".res")).map(|e| e.name.clone()).collect();
    let rider_name = names.iter().filter(|n| n.to_ascii_lowercase().ends_with("thq1.res")).min_by_key(|n| n.len())?.clone();
    let res = disc.res(&rider_name).expect("the rider archive opens");
    let rider_file = res.read_suffix("anim\\mrider.phy").expect("the rider's archive carries mrider.phy");
    let mut body = game_render::rider::Body::load(
        vec![disc.res(&rider_name).expect("the rider archive opens")],
        "thq1",
        game_render::rider::Sex::Male,
        "",
        &[],
        game_sim::motion::Params::default(),
    )
    .expect("the rider's body loads");
    let model = game_render::psg::Model::parse(&res.read_suffix(&format!("{}.psg", body.geometry())).expect("its model"))
        .expect("the model parses");
    let doll = body.ragdoll_rig(&model, &rider_file).expect("every part names a node");
    Some((Kit::new(&rider_file, doll.rig), doll.nodes))
}

/// A track's terrain off the disc (`Tracks\\<track>.res`): its scene's terrain file, which the
/// riding bones' collisions ask the ground of.
fn terrain_bytes(track: &str) -> Option<(game_sim::track::Scene, Vec<u8>)> {
    use game_sim::archive::{Psr, Res};
    let kit = ps2_oracle::Kit::load().ok()?;
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut file) = std::fs::File::open(&at) else {
        skip(&format!("no {} — `./kit extract MX.PSR`", at.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    std::io::Read::read_exact(&mut file, &mut front).expect("the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find(&format!("Tracks\\{track}.res"))?;
    std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    std::io::Read::read_exact(&mut file, &mut bytes).expect("read");
    let res = Res::parse(&bytes).ok()?;
    let read = |suffix: &str| {
        let e = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(suffix)).unwrap_or_else(|| panic!("{track} has no {suffix}"));
        res.read(&bytes, e).expect("a track member")
    };
    let scene = game_sim::track::Scene::parse(&read(".scn")).expect("the track's scene");
    let terrain = read(&scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn"));
    Some((scene, terrain))
}

/// The largest lane difference between two matrices' rows `rows`.
fn off(a: &Mat4, b: &Mat4, rows: std::ops::Range<usize>, lanes: usize) -> f32 {
    rows.flat_map(|r| (0..lanes).map(move |c| (a.0[r][c] - b.0[r][c]).abs())).fold(0.0, f32::max)
}

/// **The animated skeleton**: which node each node hangs under, and the fixed offsets of every node
/// but the root, read off a held tick, where the original's nodes are the animation's own (S74). Each
/// node's parent is the one its local rows carry it from to its world rows; the root's is the bike.
struct Skeleton {
    parent: [Option<usize>; JOINTS],
    offset: [Vf; JOINTS],
}

impl Skeleton {
    fn read(held: &Row) -> Skeleton {
        let mut parent = [None; JOINTS];
        let mut offset = [Vf::ZERO; JOINTS];
        for j in 0..JOINTS {
            let l = held.local(j);
            let w = held.world(j);
            let under = |p: &Mat4| off(&math::mul(&l, p), &w, 0..4, 3);
            let (best, by) = (0..JOINTS)
                .filter(|&p| p != j)
                .map(|p| (Some(p), under(&held.world(p))))
                .chain([(None, under(&held.bike()))])
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap();
            assert!(by < 1e-2, "frame {}: node {j} hangs under nothing ({by})", held.frame);
            parent[j] = best;
            offset[j] = l.0[3];
        }
        assert_eq!(parent[0], None, "the root hangs under the bike");
        Skeleton { parent, offset }
    }

    /// Each node in the world on `row`'s tick: its joint's turn, the root at its joint's translation
    /// and every other node at its offset, under the bike's node.
    fn world(&self, row: &Row) -> Vec<Mat4> {
        self.world_under(row, &row.bike())
    }

    fn world_under(&self, row: &Row, bike: &Mat4) -> Vec<Mat4> {
        let mut out: Vec<Option<Mat4>> = vec![None; JOINTS];
        // Parents come before children in neither order guaranteed: walk until all are placed.
        while out.iter().any(Option::is_none) {
            for j in 0..JOINTS {
                if out[j].is_some() {
                    continue;
                }
                let up = match self.parent[j] {
                    None => *bike,
                    Some(p) => match out[p] {
                        Some(m) => m,
                        None => continue,
                    },
                };
                let (t, q) = row.joint(j);
                let local = math::quat_to_mat4(q, if j == 0 { t } else { self.offset[j] });
                out[j] = Some(math::mul(&local, &up));
            }
        }
        out.into_iter().map(Option::unwrap).collect()
    }
}

/// `m` in `frame`'s frame, in f64: the test's glue, not the port's arithmetic.
fn under(m: &Mat4, frame: &Mat4) -> Bone {
    let f = |r: usize, c: usize| f64::from(frame.0[r][c]);
    let dot_row = |v: [f64; 3], r: usize| v[0] * f(r, 0) + v[1] * f(r, 1) + v[2] * f(r, 2);
    // The frame's rows are orthonormal to f32: its inverse is the transpose.
    let back = |v: [f64; 3], w: f64| -> Vf {
        let v = if w == 1.0 { [v[0] - f(3, 0), v[1] - f(3, 1), v[2] - f(3, 2)] } else { v };
        Vf::new(dot_row(v, 0) as f32, dot_row(v, 1) as f32, dot_row(v, 2) as f32, w as f32)
    };
    let row = |r: usize| [0, 1, 2].map(|c| f64::from(m.0[r][c]));
    Bone { at: back(row(3), 1.0), forward: back(row(2), 0.0), up: back(row(1), 0.0) }
}

/// A joint's quaternion as a rest: normalised, `w` not negative.
fn rest(q: Vf) -> Vf {
    let (x, y, z, w) = (f64::from(q.x()), f64::from(q.y()), f64::from(q.z()), f64::from(q.w()));
    let n = (x * x + y * y + z * z + w * w).sqrt();
    let k = if w < 0.0 { -1.0 / n } else { 1.0 / n };
    Vf::new((x * k) as f32, (y * k) as f32, (z * k) as f32, (w * k) as f32)
}

/// What a run of riding ticks came to against the original.
#[derive(Default)]
struct Tally {
    ticks: usize,
    parts: usize,
    sum_at: f64,
    worst_at: (f32, String),
    worst_turn: (f32, String),
}

impl Tally {
    fn add(&mut self, at: f32, turn: f32, what: impl Fn() -> String) {
        self.parts += 1;
        self.sum_at += f64::from(at);
        if at > self.worst_at.0 {
            self.worst_at = (at, what());
        }
        if turn > self.worst_turn.0 {
            self.worst_turn = (turn, what());
        }
    }
    fn merge(&mut self, t: Tally) {
        self.ticks += t.ticks;
        self.parts += t.parts;
        self.sum_at += t.sum_at;
        if t.worst_at.0 > self.worst_at.0 {
            self.worst_at = t.worst_at;
        }
        if t.worst_turn.0 > self.worst_turn.0 {
            self.worst_turn = t.worst_turn;
        }
    }
    fn line(&self) -> String {
        format!(
            "{} ticks, worst {:.4} ft at {}, worst turn {:.4} at {}, mean {:.4} ft",
            self.ticks, self.worst_at.0, self.worst_at.1, self.worst_turn.0, self.worst_turn.1, self.mean_at()
        )
    }
    fn mean_at(&self) -> f32 {
        if self.parts == 0 { 0.0 } else { (self.sum_at / self.parts as f64) as f32 }
    }
}

/// **One stretch, carried**: a fresh riding body stepped on `rows[from..]` while the original's body
/// is held or riding, each tick on the original's request, step and animated skeleton; from `compare`
/// on, every part's node is held against the original's, the first [`ENTRY`] ticks in `entry` and the
/// rest in `settled`. Stops at the first tick the original is in
/// any other mode, or at a gap.
fn ride(name: &str, ground: &mut dyn game_sim::world::World, kit: &Kit, nodes: &[usize], skel: &Skeleton, rows: &[Row], from: usize, compare: u32, entry: &mut Tally, settled: &mut Tally) -> usize {
    let mut riding = Riding::new(kit);
    let mut stepped = 0;
    for k in from..rows.len() {
        let (row, before) = (&rows[k], &rows[k - 1]);
        if row.frame != before.frame + 1 || !matches!(row.mode(), 0 | 1) || (k > from + 2 && row.mode() == 0 && before.mode() == 1) {
            break;
        }
        let r = row.request();
        // The part sync and the animation pose the skeleton before this tick's bike has moved its
        // node: the points the spring pulls to hang under last tick's bike.
        let animated = skel.world_under(row, &before.bike());
        let posed = Posed {
            bones: nodes.iter().map(|&j| under(&animated[j], &r.frame)).collect(),
            rests: nodes.iter().map(|&j| rest(row.joint(j).1)).collect(),
        };
        // The two words the mode machine reads off the vehicle animation are last tick's: this tick's
        // animation runs inside the physics update, after it.
        riding.actor.posed = before.word("body.680", 0) != 0;
        riding.actor.latched = before.word("body.680", 0x684 - 0x680) != 0;
        let clip = before.word("body.8dc", 0x8f0 - 0x8dc) as i32;
        riding.step(kit, Some(&posed), &r, clip, F(row.float("dt", 0)), ground);
        stepped += 1;
        let mode = match riding.actor.mode {
            game_sim::rider_anim::Mode::Held => 0,
            game_sim::rider_anim::Mode::Riding => 1,
            m => panic!("{name} frame {}: the port's body is {m:?}", row.frame),
        };
        assert_eq!(mode, row.mode(), "{name} frame {}: the body's mode", row.frame);
        if row.frame < compare || row.mode() != 1 {
            continue;
        }
        assert!(riding.placed(), "{name} frame {}: the riding body is not placed", row.frame);
        let tally = if row.frame < compare + ENTRY { &mut *entry } else { &mut *settled };
        tally.ticks += 1;
        for (i, (m, &j)) in riding.nodes(kit).iter().zip(nodes).enumerate() {
            let theirs = row.world(j);
            let at = off(m, &theirs, 3..4, 3);
            let turn = off(m, &theirs, 0..3, 3);
            tally.add(at, turn, || format!("{name} frame {} part {i} (node {j})", row.frame));
        }
    }
    stepped
}

/// **The riding spring after every put-back, and plain riding.** On each of S74's nine put-backs a
/// fresh riding body takes the original's two held ticks (the skeleton the riding entry poses the
/// bones from and measures their velocity against), and from T+2 — the first riding tick — on
/// through the 3 s ghost and the plain riding that follows, to the capture's end or the next wreck,
/// each of its 18 part nodes stays within [`ENTRY_AT`] ft and [`ENTRY_TURN`] per orientation lane of
/// the original's node over the first [`ENTRY`] ticks, and within [`NEAR_AT`] / [`NEAR_TURN`] and
/// [`MEAN_AT`] ft on average after. The capture's opening stretch of plain riding, which starts
/// mid-ride with no body state, is held to the settled bounds after a second's settle.
#[test]
fn the_riding_spring_follows_the_original_after_each_put_back() {
    let Some((kit, nodes)) = ride_kit() else { return };
    assert_eq!(nodes.len(), game_sim::ragdoll::PARTS, "one node per part");
    let (mut entry, mut settled) = (Tally::default(), Tally::default());
    let mut put_backs = 0;
    let mut lines = Vec::new();
    // S66's states are all free09, the ones named free06 too (their fog is free09's).
    let Some((scene, terrain)) = terrain_bytes("free09") else { return };
    let terrain = game_sim::track::Terrain::parse(&terrain).expect("free09's terrain parses");
    for name in ["free06-ride", "free06-down", "free06-downchord", "free09-ride", "free09-down"] {
        let Some(rows) = capture(name) else { continue };
        let mut ground = game_sim::world::Ground::new(&terrain, &scene.terrain);
        let held = rows.iter().find(|r| r.mode() == 0).expect("a held tick");
        let skel = Skeleton::read(held);
        // The held ticks' nodes are the animation's: the skeleton built from the joints is them.
        for row in rows.iter().filter(|r| r.mode() == 0) {
            let built = skel.world(row);
            for j in 0..JOINTS {
                let o = off(&built[j], &row.world(j), 0..4, 3);
                assert!(o < 1e-2, "{name} frame {}: the animated node {j} is {o} off the held one", row.frame);
            }
        }
        let mut k = 1;
        while k < rows.len() {
            if rows[k].mode() == 0 && rows[k - 1].mode() != 0 {
                put_backs += 1;
                let first = rows[k + 2].frame;
                let (mut e, mut t) = (Tally::default(), Tally::default());
                let n = ride(name, &mut ground, &kit, &nodes, &skel, &rows, k, first, &mut e, &mut t);
                lines.push(format!("{name} put-back {} (T+2 = {first}) entry: {}", rows[k].frame, e.line()));
                lines.push(format!("{name} put-back {} settled: {}", rows[k].frame, t.line()));
                entry.merge(e);
                settled.merge(t);
                k += n.max(1);
                continue;
            }
            k += 1;
        }
        // The opening's plain riding: no body state, so a second's settle before comparing.
        let settle = rows[0].frame + 1 + 30;
        let (mut e, mut t) = (Tally::default(), Tally::default());
        // Compared from `settle` on: the entry window ends there and its ticks are dropped.
        ride(name, &mut ground, &kit, &nodes, &skel, &rows, 1, settle - ENTRY, &mut e, &mut t);
        lines.push(format!("{name} opening ({settle}..): {}", t.line()));
        settled.merge(t);
    }
    for l in &lines {
        println!("{l}");
    }
    if put_backs == 0 {
        return;
    }
    assert_eq!(put_backs, 9, "S74's nine put-backs");
    for (what, t, near_at, near_turn, mean_at) in [
        ("the riding entries", &entry, ENTRY_AT, ENTRY_TURN, f32::INFINITY),
        ("settled riding", &settled, NEAR_AT, NEAR_TURN, MEAN_AT),
    ] {
        println!("{what}: {} part nodes; {}", t.parts, t.line());
        assert!(t.worst_at.0 <= near_at, "{what}: a node {} ft from the original's at {}", t.worst_at.0, t.worst_at.1);
        assert!(t.worst_turn.0 <= near_turn, "{what}: a node turned {} off the original's at {}", t.worst_turn.0, t.worst_turn.1);
        assert!(t.mean_at() <= mean_at, "{what}: the nodes are {} ft off the original's on average", t.mean_at());
    }
}
