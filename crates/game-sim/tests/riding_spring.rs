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
//! free06 too — with no props or scene meshes. Since S90 this body is the drawn riding rider, and
//! [`the_riding_body_gives_as_the_originals`] holds its springiness — how far it hangs off the clip
//! and how long it takes to settle — to the original's.
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
/// lane of the three unit rows), and on average. Since S201 the entry starts where the original's does
/// (the nodes the last held tick left, each bone moving by its centre of mass turned back over the
/// frame): seven entries stay within 0.006 ft, the two off the stick-held `down` runs within 0.045.
const ENTRY: u32 = 4;
const ENTRY_AT: f32 = 0.06;
const ENTRY_TURN: f32 = 0.1;
const ENTRY_MEAN_AT: f32 = 0.01;
/// The same once settled, on every later tick of the ghost and the plain riding. The worst of both are
/// the arms after the two put-backs that come straight off a wreck (about 0.15 ft and 0.2), which
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
    capture_in("s74", name)
}

fn capture_in(task: &str, name: &str) -> Option<Vec<Row>> {
    let kit = ps2_oracle::Kit::load().map_err(|why| skip(&why)).ok()?;
    let local = kit.root.join("context").join(task);
    let dir = if local.exists() { local } else { kit.ctx(task) };
    let Ok(text) = std::fs::read_to_string(dir.join(format!("{name}.txt"))) else {
        println!("no context/{task}/{name}.txt: no riding-spring verdict for it");
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
    body_kit().map(|(kit, nodes, _)| (kit, nodes))
}

/// [`ride_kit`], and the rider's body it was built from (its skeleton names the nodes).
fn body_kit() -> Option<(Kit, Vec<usize>, game_render::rider::Body)> {
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
    Some((Kit::new(&rider_file, doll.rig), doll.nodes, body))
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
    ride_watched(name, ground, kit, nodes, skel, rows, from, compare, entry, settled, &mut |_, _| {})
}

/// [`ride`], showing `watch` each riding tick's row and the port's part nodes (S90).
#[allow(clippy::too_many_arguments)]
fn ride_watched(
    name: &str,
    ground: &mut dyn game_sim::world::World,
    kit: &Kit,
    nodes: &[usize],
    skel: &Skeleton,
    rows: &[Row],
    from: usize,
    compare: u32,
    entry: &mut Tally,
    settled: &mut Tally,
    watch: &mut dyn FnMut(&Row, &[Mat4]),
) -> usize {
    let mut riding = Riding::new(kit);
    if rows[from].mode() == 0 {
        // A put-back follows a wreck: the held entry clears the placement, as the original's does
        // (its placed word reads 0 on every held tick), so T+2 runs the riding entry's pose (S201).
        riding.actor.prev = game_sim::rider_anim::Mode::Wrecked;
    }
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
        // The held body is put back under the bike's node as this tick leaves it (S201).
        let after = skel.world(row);
        let moved = Posed { bones: nodes.iter().map(|&j| under(&after[j], &r.frame)).collect(), rests: posed.rests.clone() };
        riding.step_moved(kit, Some(&posed), Some(&moved), &r, clip, F(row.float("dt", 0)), ground);
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
        let port = riding.nodes(kit);
        watch(row, &port);
        for (i, (m, &j)) in port.iter().zip(nodes).enumerate() {
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
        ("the riding entries", &entry, ENTRY_AT, ENTRY_TURN, ENTRY_MEAN_AT),
        ("settled riding", &settled, NEAR_AT, NEAR_TURN, MEAN_AT),
    ] {
        println!("{what}: {} part nodes; {}", t.parts, t.line());
        assert!(t.worst_at.0 <= near_at, "{what}: a node {} ft from the original's at {}", t.worst_at.0, t.worst_at.1);
        assert!(t.worst_turn.0 <= near_turn, "{what}: a node turned {} off the original's at {}", t.worst_turn.0, t.worst_turn.1);
        assert!(t.mean_at() <= mean_at, "{what}: the nodes are {} ft off the original's on average", t.mean_at());
    }
}

/// **Where the riding entry starts the bones** (S201), on `context/s201/free09-ride3.txt`: S74's ride
/// script on free09 with each part body's start (the position and velocity the entry's pose writes)
/// captured too. On both put-backs, at T+2 every bone starts at the node the last held tick (T+1)
/// left, not at this tick's animation, and with the velocity the pose measures from the drawn
/// matrices that tick's still repose wrote: those stand at the body's centre of mass, so the bone
/// moves by its centre of mass turned back over the frame. The port's pose from T+1's nodes gives both
/// bit for bit, and its riding body from the put-back on stays with the original's.
#[test]
fn the_riding_entry_starts_from_the_last_held_tick() {
    let Some((kit, nodes)) = ride_kit() else { return };
    let Some(rows) = capture_in("s201", "free09-ride3") else { return };
    let Some((scene, terrain)) = terrain_bytes("free09") else { return };
    let terrain = game_sim::track::Terrain::parse(&terrain).expect("free09's terrain parses");
    let held = rows.iter().find(|r| r.mode() == 0).expect("a held tick");
    let skel = Skeleton::read(held);
    let (t, c) = (&kit.tables.acos, &kit.tables.curve);
    let mut entries = 0;
    for k in 3..rows.len() {
        if !(rows[k].mode() == 1 && rows[k - 1].mode() == 0) {
            continue;
        }
        entries += 1;
        let (row, last) = (&rows[k], &rows[k - 1]);
        let inv_dt = F(1.0 / row.float("dt", 0));
        let (mut at, mut moving) = (0f32, 0f32);
        for (i, &j) in nodes.iter().enumerate() {
            let m = last.world(j);
            let bone = game_sim::ragdoll_pose::Bone { at: m.0[3], forward: m.0[2], up: m.0[1] };
            let cofg = kit.rig.parts[i].cofg;
            let (mut drawn, mut start) = (Mat4([Vf::ZERO; 4]), game_sim::ragdoll_pose::Start::default());
            game_sim::ragdoll_pose::pose(&bone, cofg, &mut drawn, &mut start, None, t, c);
            game_sim::ragdoll_pose::pose(&bone, cofg, &mut drawn, &mut start, Some(inv_dt), t, c);
            let (p, v) = (row.quad(&format!("start{i}"), 0), row.quad(&format!("start{i}"), 0x10));
            for l in 0..3 {
                at = at.max((start.position[l] - p[l]).abs());
                moving = moving.max((start.velocity[l] - v[l]).abs());
            }
        }
        println!("free09-ride3 entry {}: starts {at} ft, {moving} ft/s off the original's", row.frame);
        assert_eq!(at, 0.0, "frame {}: a bone starts {at} ft off the original's", row.frame);
        assert_eq!(moving, 0.0, "frame {}: a bone starts moving {moving} ft/s off the original's", row.frame);
    }
    assert_eq!(entries, 2, "the ride script's two put-backs");
    let mut ground = game_sim::world::Ground::new(&terrain, &scene.terrain);
    let (mut entry, mut settled) = (Tally::default(), Tally::default());
    let mut k = 1;
    while k < rows.len() {
        if rows[k].mode() == 0 && rows[k - 1].mode() != 0 {
            let (mut e, mut s) = (Tally::default(), Tally::default());
            k += ride("free09-ride3", &mut ground, &kit, &nodes, &skel, &rows, k, rows[k + 2].frame, &mut e, &mut s).max(1);
            entry.merge(e);
            settled.merge(s);
            continue;
        }
        k += 1;
    }
    for (what, t, near_at, near_turn, mean_at) in [
        ("the riding entries", &entry, ENTRY_AT, ENTRY_TURN, ENTRY_MEAN_AT),
        ("settled riding", &settled, NEAR_AT, NEAR_TURN, MEAN_AT),
    ] {
        println!("free09-ride3 {what}: {}", t.line());
        assert!(t.worst_at.0 <= near_at, "{what}: a node {} ft from the original's at {}", t.worst_at.0, t.worst_at.1);
        assert!(t.worst_turn.0 <= near_turn, "{what}: a node turned {} off the original's at {}", t.worst_turn.0, t.worst_turn.1);
        assert!(t.mean_at() <= mean_at, "{what}: the nodes are {} ft off the original's on average", t.mean_at());
    }
}

// --- S90: the riding body's give, as the original's ---------------------------------------------------

/// One riding tick's give: how far each part's node hangs off the animated skeleton's, the
/// original's and the port's, in feet, and the bike's height.
struct Give {
    frame: u32,
    bike_y: f32,
    dt: f32,
    theirs: Vec<f32>,
    ours: Vec<f32>,
}

impl Give {
    fn mean(v: &[f32]) -> f32 {
        v.iter().sum::<f32>() / v.len() as f32
    }
}

/// Every stretch of S74's and S201's captures carried as [`the_riding_spring_follows_the_original_after_each_put_back`]
/// carries them, as each riding tick's [`Give`]: the put-backs from T+2, and each opening after its
/// second's settle. Each stretch is named and says whether it starts at a put-back.
fn gives(kit: &Kit, nodes: &[usize]) -> Option<Vec<(String, bool, Vec<Give>)>> {
    let (scene, terrain) = terrain_bytes("free09")?;
    let terrain = game_sim::track::Terrain::parse(&terrain).expect("free09's terrain parses");
    let mut out = Vec::new();
    for (task, name) in [
        ("s74", "free06-ride"),
        ("s74", "free06-down"),
        ("s74", "free06-downchord"),
        ("s74", "free09-ride"),
        ("s74", "free09-down"),
        ("s201", "free09-ride"),
        ("s201", "free09-ride2"),
        ("s201", "free09-ride3"),
    ] {
        let Some(rows) = capture_in(task, name) else { continue };
        let mut ground = game_sim::world::Ground::new(&terrain, &scene.terrain);
        // S201's first two captures carry no node rows: nothing to read the skeleton from.
        let Some(held) = rows.iter().find(|r| r.mode() == 0) else { continue };
        let skel = Skeleton::read(held);
        let mut stretches: Vec<(usize, u32, bool)> = vec![(1, rows[0].frame + 1 + 30 - ENTRY, false)];
        for k in 1..rows.len().saturating_sub(2) {
            if rows[k].mode() == 0 && rows[k - 1].mode() != 0 {
                stretches.push((k, rows[k + 2].frame, true));
            }
        }
        for (from, compare, put_back) in stretches {
            let mut g = Vec::new();
            let (mut e, mut s) = (Tally::default(), Tally::default());
            ride_watched(name, &mut ground, kit, nodes, &skel, &rows, from, compare, &mut e, &mut s, &mut |row, port| {
                let anim = skel.world(row);
                let give = |m: &Mat4, j: usize| off(m, &anim[j], 3..4, 3);
                g.push(Give {
                    frame: row.frame,
                    bike_y: row.bike().0[3][1],
                    dt: row.float("dt", 0),
                    theirs: nodes.iter().map(|&j| give(&row.world(j), j)).collect(),
                    ours: port.iter().zip(nodes).map(|(m, &j)| give(m, j)).collect(),
                });
            });
            out.push((format!("{task}/{name} {}", rows[from].frame), put_back, g));
        }
    }
    Some(out)
}

/// **The riding body gives as the original's does** (S90): not only node by node (the test above), but
/// in the two numbers the eye reads as springiness — how far the body hangs off the clip, and how long it
/// takes to settle. Over every stretch of S74's and S201's captures, carried free from the put-back (or
/// the opening's settle), each part's give (its node's distance from the animated skeleton's) is
/// measured for the original and the port:
///
/// - the give's root mean square over each stretch, per part: the port's within [`GIVE_RMS`] (as a
///   share) of the original's, or [`GIVE_FLOOR`] ft where both are smaller than that;
/// - its peak per part per stretch, within [`GIVE_PEAK`] ft;
/// - after each put-back, the tick the body's mean give last stands above half its ghost peak (its
///   settle), within [`SETTLE_TICKS`] of the original's;
/// - at each jolt (the bike's vertical speed changing by more than [`JOLT`] ft/s in a tick: a bump or a
///   landing), the body's peak give in the [`AFTER_JOLT`] ticks that follow within [`GIVE_PEAK`] ft,
///   and its settle (the tick it last stands above half that peak) within [`SETTLE_TICKS`].
#[test]
fn the_riding_body_gives_as_the_originals() {
    let Some((kit, nodes)) = ride_kit() else { return };
    let Some(stretches) = gives(&kit, &nodes) else { return };
    let mut lines = Vec::new();
    let mut bad = Vec::new();
    let (mut put_backs, mut jolts) = (0, 0);
    for (name, put_back, g) in &stretches {
        if g.is_empty() {
            continue;
        }
        let parts = g[0].theirs.len();
        let rms = |pick: &dyn Fn(&Give) -> f32| (g.iter().map(|t| f64::from(pick(t)).powi(2)).sum::<f64>() / g.len() as f64).sqrt() as f32;
        let mut worst_rms = (0f32, 0f32, 0usize);
        let mut worst_peak = (0f32, 0usize);
        for i in 0..parts {
            let (a, b) = (rms(&|t| t.theirs[i]), rms(&|t| t.ours[i]));
            let off = (b - a).abs();
            if off > GIVE_FLOOR && off > GIVE_RMS * a {
                bad.push(format!("{name} part {i}: give rms {b:.4} ft, the original's {a:.4}"));
            }
            if off > (worst_rms.1 - worst_rms.0).abs() {
                worst_rms = (a, b, i);
            }
            let peak = |pick: &dyn Fn(&Give) -> f32| g.iter().map(pick).fold(0f32, f32::max);
            let (pa, pb) = (peak(&|t| t.theirs[i]), peak(&|t| t.ours[i]));
            if (pb - pa).abs() > GIVE_PEAK {
                bad.push(format!("{name} part {i}: give peak {pb:.4} ft, the original's {pa:.4}"));
            }
            if (pb - pa).abs() > worst_peak.0 {
                worst_peak = ((pb - pa).abs(), i);
            }
        }
        let body = |v: &dyn Fn(&Give) -> f32| g.iter().map(v).collect::<Vec<f32>>();
        let (theirs, ours) = (body(&|t| Give::mean(&t.theirs)), body(&|t| Give::mean(&t.ours)));
        lines.push(format!(
            "{name}: {} ticks, body give rms {:.4} ft (original {:.4}), worst part {} rms {:.4} vs {:.4}, worst peak off {:.4} ft (part {})",
            g.len(),
            rms(&|t| Give::mean(&t.ours)),
            rms(&|t| Give::mean(&t.theirs)),
            worst_rms.2,
            worst_rms.1,
            worst_rms.0,
            worst_peak.0,
            worst_peak.1
        ));
        if *put_back {
            put_backs += 1;
            let ghost = g.len().min(GHOST);
            let (st, so) = (settle(&theirs[..ghost]), settle(&ours[..ghost]));
            lines.push(format!("  put-back settle: tick {} (original {}), peak {:.4} ft (original {:.4})", so.0, st.0, so.1, st.1));
            if so.0.abs_diff(st.0) > SETTLE_TICKS {
                bad.push(format!("{name}: the put-back settles at T+2+{}, the original's at T+2+{}", so.0, st.0));
            }
        }
        for k in 2..g.len() {
            let v = |a: &Give, b: &Give| (b.bike_y - a.bike_y) / b.dt;
            let jolt = v(&g[k - 1], &g[k]) - v(&g[k - 2], &g[k - 1]);
            if jolt.abs() <= JOLT || g[k].frame != g[k - 2].frame + 2 || k + AFTER_JOLT > g.len() {
                continue;
            }
            jolts += 1;
            let (st, so) = (settle(&theirs[k..k + AFTER_JOLT]), settle(&ours[k..k + AFTER_JOLT]));
            lines.push(format!(
                "  jolt at {} ({:+.2} ft/s): peak {:.4} ft (original {:.4}), settle {} (original {})",
                g[k].frame, jolt, so.1, st.1, so.0, st.0
            ));
            if (so.1 - st.1).abs() > GIVE_PEAK || so.0.abs_diff(st.0) > SETTLE_TICKS {
                bad.push(format!("{name}: the jolt at {} gives {:.4} ft and settles in {}, the original {:.4} in {}", g[k].frame, so.1, so.0, st.1, st.0));
            }
        }
    }
    for l in &lines {
        println!("{l}");
    }
    println!("{put_backs} put-backs, {jolts} jolts");
    assert!(bad.is_empty(), "{} gives off the original's:\n{}", bad.len(), bad.join("\n"));
}

/// How far, as a share of the original's, a part's give may be off over a stretch (root mean square).
const GIVE_RMS: f32 = 0.1;
/// ...unless it is within this many feet anyway.
const GIVE_FLOOR: f32 = 0.005;
/// How far a part's (or the body's, at a jolt) peak give may be off the original's, in feet.
const GIVE_PEAK: f32 = 0.07;
/// How many ticks a settle may be off the original's.
const SETTLE_TICKS: usize = 3;
/// The ghost's ticks: 3 s at 30 Hz.
const GHOST: usize = 90;
/// A change in the bike's vertical speed in one tick, in ft/s, counted as a bump or a landing.
const JOLT: f32 = 4.0;
/// How many ticks after a jolt its give is read over.
const AFTER_JOLT: usize = 30;

/// A run of the body's mean give: the last tick it stands above half its peak, and the peak.
fn settle(v: &[f32]) -> (usize, f32) {
    let peak = v.iter().copied().fold(0f32, f32::max);
    (v.iter().rposition(|&x| x > peak / 2.0).unwrap_or(0), peak)
}

/// Where the whip is all the way out: the body's crossup (its phase lag record's value) within a
/// hundredth of the sweep's end, as `whip_tricks.rs` reads the port's.
const FULL_WHIP: f32 = 0.01;
/// How far the port's riding pelvis may be from the original's through the full whip, in feet.
/// Measured on C20's capture: 0.002 at worst.
const WHIP_APART: f32 = 0.01;
/// The mean over C20's whole stretch, every part and tick, in feet. Measured 0.0137: steadier than
/// that on the whip itself, but the crossup's sweep in after the take-off (2652..2675) leaves the
/// pelvis up to 0.044 ft off, where steady riding sits near 0.004 ([`MEAN_AT`]).
const WHIP_MEAN_AT: f32 = 0.02;

/// `d`, a direction in the world, in `frame`'s axes (its rows are orthonormal to f32).
fn axes_of(d: [f32; 3], frame: &Mat4) -> [f32; 3] {
    std::array::from_fn(|r| (0..3).map(|c| d[c] * frame.0[r][c]).sum())
}

/// From `b`'s position to `a`'s, in the world.
fn apart(a: &Mat4, b: &Mat4) -> [f32; 3] {
    std::array::from_fn(|c| a.0[3][c] - b.0[3][c])
}

fn length(v: [f32; 3]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// **C20: the riding body through a full whip**, against the original's. `context/c20/whip-right.txt`
/// is recording2 on free09 with jump 2's cancan made a full whip: the stick hard right from just
/// after the take-off to the landing, with no trick held (C20's `make_tape.py`), logged once a tick by
/// S201's `entry_watch.py`. The crossup is all the way out for 36 ticks before the bike lands crossed
/// up and wrecks. The rider is S74's thq1 (its end joints' fixed offsets are S74's held tick's), so
/// S74's skeleton rebuilds the clip's target from the capture's joints.
///
/// The port's riding body, carried from the opening's settle on the original's request, step and
/// clip, keeps its pelvis within [`WHIP_APART`] of the original's on every full-whip tick, and the
/// stretch stays inside the settled bounds. What the original does there: the clips take the hands
/// 0.23 and 0.37 ft off the grips, and its body does not slide towards the bars but **rises**: its
/// pelvis stands about 0.38 ft off the clip's, in the bike's frame (-0.09, +0.35, +0.13) on
/// average, mostly up (along the bike's own up, while the world's up swings round in that frame as
/// the bike rolls), and its hands end 0.12 and 0.13 ft off the grips.
#[test]
fn a_full_whip_hangs_the_riding_body_as_the_originals() {
    let Some((kit, nodes, body)) = body_kit() else { return };
    let Some(rows) = capture_in("c20", "whip-right") else { return };
    let Some(s74) = capture("free09-ride") else { return };
    let Some((scene, terrain)) = terrain_bytes("free09") else { return };
    let terrain = game_sim::track::Terrain::parse(&terrain).expect("free09's terrain parses");
    let mut ground = game_sim::world::Ground::new(&terrain, &scene.terrain);
    let skel = Skeleton::read(s74.iter().find(|r| r.mode() == 0).expect("a held tick"));
    let names = &body.rig.set.skeleton;
    let pelvis = names.find("Pelvis").expect("a pelvis");
    let hands = game_render::rider::HAND_ANCHORS.map(|n| names.find(n).expect("a hand anchor"));
    // The end joints the ragdoll never moves under their parents: S74's offsets if it is S74's rider.
    for row in &rows {
        for j in (0..JOINTS).filter(|&j| !nodes.contains(&j)) {
            let o = (0..3).map(|c| (row.local(j).0[3][c] - skel.offset[j].0[c]).abs()).fold(0.0, f32::max);
            assert!(o < 1e-3, "frame {}: node {j} hangs {o} ft off S74's: not S74's rider", row.frame);
        }
    }
    let part = nodes.iter().position(|&n| n == pelvis).expect("the pelvis is a part");
    let mut port: HashMap<u32, Mat4> = HashMap::new();
    let (mut entry, mut settled) = (Tally::default(), Tally::default());
    let settle = rows[0].frame + 1 + 30;
    ride_watched("c20/whip-right", &mut ground, &kit, &nodes, &skel, &rows, 1, settle - ENTRY, &mut entry, &mut settled, &mut |row, p| {
        port.insert(row.frame, p[part]);
    });
    println!("c20/whip-right ({settle}..): {}", settled.line());
    assert!(settled.worst_at.0 <= NEAR_AT, "a node {} ft from the original's at {}", settled.worst_at.0, settled.worst_at.1);
    assert!(settled.worst_turn.0 <= NEAR_TURN, "a node turned {} off the original's at {}", settled.worst_turn.0, settled.worst_turn.1);
    assert!(settled.mean_at() <= WHIP_MEAN_AT, "the nodes are {} ft off the original's on average", settled.mean_at());
    // The last unbroken run of airborne ticks with the crossup all the way out: the whip (the take-off
    // tick also reads 0, off the human's steer to the left on the lip, then the whip sweeps it back).
    let mut full: Vec<&Row> = Vec::new();
    for r in &rows {
        if !(r.word("req", 0xf8) != 0 && r.float("lag.phase", 0) < FULL_WHIP && port.contains_key(&r.frame)) {
            continue;
        }
        if full.last().is_some_and(|l| l.frame + 1 != r.frame) {
            full.clear();
        }
        full.push(r);
    }
    assert!(full.len() >= 30, "the whip was all the way out for {} ticks", full.len());
    let (mut theirs_sum, mut ours_sum, mut worst) = ([0f32; 3], [0f32; 3], (0f32, 0u32));
    let (mut held, mut clip) = ([0f32; 2], [0f32; 2]);
    for row in &full {
        let r = row.request();
        let anim = skel.world(row);
        let theirs = axes_of(apart(&row.world(pelvis), &anim[pelvis]), &r.frame);
        let ours = axes_of(apart(&port[&row.frame], &anim[pelvis]), &r.frame);
        let off = length(std::array::from_fn(|c| theirs[c] - ours[c]));
        if off > worst.0 {
            worst = (off, row.frame);
        }
        for c in 0..3 {
            theirs_sum[c] += theirs[c] / full.len() as f32;
            ours_sum[c] += ours[c] / full.len() as f32;
        }
        for i in 0..2 {
            let to = |m: &Mat4| length(std::array::from_fn(|c| m.0[3][c] - r.hands[i].0[c]));
            held[i] = held[i].max(to(&row.world(hands[i])));
            clip[i] = clip[i].max(to(&anim[hands[i]]));
        }
    }
    let round = |v: [f32; 3]| v.map(|x| (x * 1000.0).round() / 1000.0);
    println!(
        "the full whip {}..={} ({} ticks): the original's pelvis {:?} ({:.3} ft) off the clip's on average in the bike's frame, the port's {:?}, \
         at worst {:.4} ft apart at {}; the original's hands at most {:.3} {:.3} ft off the grips, the clip's {:.3} {:.3}",
        full[0].frame,
        full[full.len() - 1].frame,
        full.len(),
        round(theirs_sum),
        length(theirs_sum),
        round(ours_sum),
        worst.0,
        worst.1,
        held[0],
        held[1],
        clip[0],
        clip[1]
    );
    assert!(worst.0 <= WHIP_APART, "the port's riding pelvis is {:.4} ft from the original's at {}", worst.0, worst.1);
    // What the original's body does there, which `whip_tricks.rs` holds the port's own whip to.
    assert!(clip.iter().any(|&c| c > 0.3), "the clips no longer take a hand off its grip: {clip:?}");
    assert!(length(theirs_sum) > 0.3 && theirs_sum[1] > 2.0 * theirs_sum[0].abs().max(theirs_sum[2].abs()), "the original's body leans {theirs_sum:?}");
}
