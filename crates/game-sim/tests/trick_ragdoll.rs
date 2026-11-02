//! S257 — **tricks under the riding ragdoll**, against a capture of recording2 played back.
//!
//! While riding, the original draws its rider from the riding ragdoll: mode 1, the bones pulled
//! towards the animation by the riding pose spring (S90, S135). A trick does not pull the bones
//! towards the stunt clip. The body actor's mode chain sends it to its posed mode (3) whenever the
//! vehicle animation's pose came off the clip table (`from_table`, as the last tick's animation left
//! it) or it is holding a pose (`holding`, which a backflip's request raises on the tick itself). The
//! posed mode's entry clears the placement, so the ragdoll is neither stepped nor drawn, and the 22
//! nodes are the clip's as the animation wrote them, cross-fade and all. When the table lets go, the
//! next tick is mode 1 again: the riding entry poses the bones where the animation has them (mid
//! cross-fade from the stunt back to the air row or the riding grid) and the spring takes over from
//! there.
//!
//! This steps the port's [`Riding`] through every trick of the capture, as S135 steps it through the
//! put-backs: each tick on the request the original's body read, the tick's step, the two words the
//! mode chain reads from the previous row, and the animated skeleton built from the capture's joints.
//! It holds the port's mode and placement to the original's on every tick, the original's posed nodes
//! to the clip it drew, and the port's riding nodes to the original's through each trick's entry tick
//! and its exit.
//!
//! The capture is `context/s257/recording2.txt` (git-ignored; the journal's `capture/play.sh`).
//! Without it the test says so and passes.

use game_sim::math::{self, Mat4};
use game_sim::ragdoll_body::Request;
use game_sim::ragdoll_pose::Bone;
use game_sim::ragdoll_ride::{Kit, Posed};
use game_sim::ragdoll_riding::Riding;
use game_sim::rider_anim::Mode;
use ps2_float::{F, Vf};
use ps2_oracle::skip;
use std::collections::HashMap;

const JOINTS: usize = 22;

/// One tick of a bones capture (S74's format): each region the recorder read, by name, as its bytes.
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
    /// The body's mode word (`+0x47c`): [`HELD`], [`RIDING`], 2 wrecked, [`POSED`].
    fn mode(&self) -> u32 {
        self.word("body.47c", 0)
    }
    /// The body's placement (`+0x2ac`): the riding entry's, cleared by the held and posed entries.
    fn placed(&self) -> bool {
        self.word("body.2a4", 8) != 0
    }
    /// The vehicle animation's two words the mode chain reads: the pose came off the clip table
    /// (`+0x680`) and it is holding one (`+0x684`).
    fn from_table(&self) -> bool {
        self.word("body.680", 0) != 0
    }
    fn holding(&self) -> bool {
        self.word("body.680", 4) != 0
    }
    /// The animation state running (`+0x8f0`).
    fn state(&self) -> i32 {
        self.word("body.8dc", 0x8f0 - 0x8dc) as i32
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
    fn merge(&mut self, t: &Tally) {
        self.ticks += t.ticks;
        self.parts += t.parts;
        self.sum_at += t.sum_at;
        if t.worst_at.0 > self.worst_at.0 {
            self.worst_at = t.worst_at.clone();
        }
        if t.worst_turn.0 > self.worst_turn.0 {
            self.worst_turn = t.worst_turn.clone();
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

/// The original's body modes (`+0x47c`) this test steps through.
const HELD: u32 = 0;
const RIDING: u32 = 1;
const POSED: u32 = 3;

/// How many riding ticks after a trick hands the body back are its exit.
const EXIT: usize = 4;
/// The capture opens mid-ride with no body state: a second's settle before its nodes are compared.
const SETTLE: u32 = 30;

/// One trick: the posed run, the animation states it ran, its entry tick (the table up, the body
/// still riding) and its exit's first [`EXIT`] riding ticks.
#[derive(Default)]
struct Trick {
    from: u32,
    to: u32,
    states: Vec<i32>,
    held: bool,
    entry: Tally,
    exit: Tally,
}

/// What the stretches came to.
#[derive(Default)]
struct Run {
    tricks: Vec<Trick>,
    riding: Tally,
    posed: usize,
    stepped: usize,
}

/// **One stretch**: a fresh riding body stepped from `rows[from]` while the original's body is held,
/// riding or posed, each tick on the original's request, step, animated skeleton and last tick's
/// two table words, as S135's `ride_watched` steps it. Every tick the port's mode and placement are
/// the original's; from `compare` on every riding tick's part nodes are held against the original's,
/// in the trick's entry or exit when the tick is one, else in plain riding. Stops at the first tick
/// the original is in any other mode, at a gap, or at the next put-back.
#[allow(clippy::too_many_arguments)]
fn stretch(ground: &mut dyn game_sim::world::World, kit: &Kit, nodes: &[usize], skel: &Skeleton, rows: &[Row], from: usize, compare: u32, run: &mut Run) -> usize {
    let mut riding = Riding::new(kit);
    if rows[from].mode() == HELD {
        // A put-back follows a wreck: the held entry clears the placement (S135, S201).
        riding.actor.prev = Mode::Wrecked;
    }
    let mut stepped = 0;
    let mut since_posed = usize::MAX;
    for k in from..rows.len() {
        let (row, before) = (&rows[k], &rows[k - 1]);
        if row.frame != before.frame + 1
            || !matches!(row.mode(), HELD | RIDING | POSED)
            || (k > from + 2 && row.mode() == HELD && before.mode() != HELD)
        {
            break;
        }
        let r = row.request();
        let animated = skel.world_under(row, &before.bike());
        let posed = Posed {
            bones: nodes.iter().map(|&j| under(&animated[j], &r.frame)).collect(),
            rests: nodes.iter().map(|&j| rest(row.joint(j).1)).collect(),
        };
        // The mode chain reads the vehicle animation's two words as last tick's animation left them.
        riding.actor.posed = before.from_table();
        riding.actor.latched = before.holding();
        let after = skel.world(row);
        let moved = Posed { bones: nodes.iter().map(|&j| under(&after[j], &r.frame)).collect(), rests: posed.rests.clone() };
        riding.step_moved(kit, Some(&posed), Some(&moved), &r, before.state(), F(row.float("dt", 0)), ground);
        stepped += 1;
        run.stepped += 1;
        let mode = match riding.actor.mode {
            Mode::Held => HELD,
            Mode::Riding => RIDING,
            Mode::Posed => POSED,
            m => panic!("frame {}: the port's body is {m:?}", row.frame),
        };
        assert_eq!(mode, row.mode(), "frame {}: the body's mode", row.frame);
        assert_eq!(riding.placed(), row.placed(), "frame {}: the body's placement", row.frame);

        // A trick starts on the tick the table goes up while the body is still riding, or on the
        // posed tick a backflip's request latches straight from riding.
        let starts = (row.mode() == RIDING && row.from_table() && !before.from_table())
            || (row.mode() == POSED && before.mode() == RIDING && !before.from_table());
        if starts {
            run.tricks.push(Trick { from: row.frame, ..Trick::default() });
        }
        if row.mode() == POSED {
            run.posed += 1;
            since_posed = 0;
            if let Some(t) = run.tricks.last_mut() {
                t.to = row.frame;
                t.held |= row.holding();
                if t.states.last() != Some(&row.state()) {
                    t.states.push(row.state());
                }
            }
            continue;
        }
        since_posed = since_posed.saturating_add(1);
        if row.frame < compare || row.mode() != RIDING {
            continue;
        }
        let trick = run.tricks.last_mut().filter(|t| t.to == 0 || since_posed <= EXIT);
        let tally = match trick {
            Some(t) if t.to == 0 => &mut t.entry,
            Some(t) => &mut t.exit,
            None => &mut run.riding,
        };
        tally.ticks += 1;
        for (i, (m, &j)) in riding.nodes(kit).iter().zip(nodes).enumerate() {
            let theirs = row.world(j);
            tally.add(off(m, &theirs, 3..4, 3), off(m, &theirs, 0..3, 3), || format!("frame {} part {i} (node {j})", row.frame));
        }
    }
    stepped
}

/// **Every trick lets the riding ragdoll go and takes it back as the original's does.** On
/// recording2 played back (its tricks: a taunt, a cancan, a lazyboy, a stalelin and two backflips,
/// V61), the port's riding body is stepped through every tick the original's is held, riding or
/// posed. Each tick its mode and placement are the original's. Every posed tick's 22 nodes are the
/// animated skeleton's (the clip drawn as written). The riding nodes on each trick's entry tick, in
/// its first [`EXIT`] riding ticks after, and in the plain riding round them are held to the
/// original's.
#[test]
fn a_trick_lets_the_riding_ragdoll_go_and_takes_it_back_as_the_originals() {
    let Some((kit, nodes)) = ride_kit() else { return };
    let Some(rows) = capture_in("s257", "recording2") else { return };
    // recording2 rides free09.
    let Some((scene, terrain)) = terrain_bytes("free09") else { return };
    let terrain = game_sim::track::Terrain::parse(&terrain).expect("free09's terrain parses");
    let mut ground = game_sim::world::Ground::new(&terrain, &scene.terrain);
    let held = rows.iter().find(|r| r.mode() == HELD).expect("a held tick");
    let skel = Skeleton::read(held);

    // The posed body is the clip: its nodes are the animated skeleton's.
    let mut clip_off = 0f32;
    for row in rows.iter().filter(|r| r.mode() == POSED) {
        let built = skel.world(row);
        for j in 0..JOINTS {
            clip_off = clip_off.max(off(&built[j], &row.world(j), 0..4, 3));
        }
    }

    let mut run = Run::default();
    let mut k = 1;
    while k < rows.len() {
        let (row, before) = (&rows[k], &rows[k - 1]);
        let starts = row.frame == before.frame + 1 && matches!(row.mode(), HELD | RIDING | POSED);
        if !starts {
            k += 1;
            continue;
        }
        let compare = if row.mode() == HELD { rows[k + 2].frame } else { row.frame + SETTLE };
        k += stretch(&mut ground, &kit, &nodes, &skel, &rows, k, compare, &mut run).max(1);
    }

    let (mut entries, mut exits) = (Tally::default(), Tally::default());
    for t in &run.tricks {
        println!("trick {}..{} states {:?}{}: entry {}; exit {}", t.from, t.to, t.states, if t.held { " (held)" } else { "" }, t.entry.line(), t.exit.line());
        entries.merge(&t.entry);
        exits.merge(&t.exit);
    }
    println!("{} stepped ticks, {} posed; the posed nodes are at most {clip_off} off the clip", run.stepped, run.posed);
    println!("entry ticks: {}", entries.line());
    println!("exits: {}", exits.line());
    println!("riding: {}", run.riding.line());
    assert_eq!((run.tricks.len(), run.posed), (6, 368), "the capture's six tricks and their posed ticks");
    assert!(clip_off < 1e-2, "a posed node is {clip_off} ft off the clip");
    // The entry tick is the riding step's last before the table takes over (none for the backflip,
    // posed on the tick itself): the spring's own error.
    assert!(entries.ticks == 5 && entries.worst_at.0 < 0.15 && entries.worst_turn.0 < 0.25, "entry ticks: {}", entries.line());
    // Out of the posed run the bones start at the clip's nodes as last tick left them, moving by their
    // last tick's move (journal): within 0.03 ft of the original's on all five exits.
    assert!(exits.ticks == 5 * EXIT && exits.worst_at.0 < 0.05 && exits.worst_turn.0 < 0.08, "exits: {}", exits.line());
    let r = &run.riding;
    assert!(r.ticks > 1000 && r.worst_at.0 < 0.3 && r.worst_turn.0 < 0.5 && r.mean_at() < 0.04, "riding: {}", r.line());
}
