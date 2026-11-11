//! MD86 — the quad's contacts on Desert Oasis: the Force body with the distributor unhooked, so that
//! the original's own record clear (which stands the trigger probe at its node's world frame) and its
//! `atv_collide_against_search` run over the image's own world, and [`quad::Steps`] answering
//! [`Call::Contacts`] with [`atv_search::Oasis`] in the port.
//!
//! The harness is [`each_unhooked_in`]'s, on the image's trigger probe (quad `+0x1430`) and nearby
//! list (`+0xc4`) in place of the harness's stand-ins: the probe hangs on the harness's node 0
//! (`+0x174`), and before each run the list is gathered round the node as the PostProcess's
//! collision sync leaves it (the gather probe's centre at the node's position, then the original's
//! gather). The world is `tests/atv_search.rs`'s, read the same way. The search writes the world's
//! collidables and the probe's record, which are not the quad's: the regions compared are the quad's,
//! the body's and the wheels' ([`snap_all`]), with the hit bit and the record's count beside them.

use super::*;
use game_sim::archive::Res;
use game_sim::atv_search;
use game_sim::atv_world;
use game_sim::camera::CurveTable;
use game_sim::eco::{Ecosystem, Trees};
use game_sim::flight_search::Rays;
use game_sim::ray_probe;
use game_sim::world::{Prop, Props};
use game_sim::world_search::{Categories, Class, Collidable, Objects};
use std::path::Path;

/// SLUS-21104's `$gp`, which the search needs (crt0 sets it at `0x001001ac`).
const GP: u64 = 0x003d_baf0;
/// The image's quad, its nearby list, its gather probe (the active probe, `+0xd8`) and its trigger
/// probe (`+0x1430`).
const IMAGE_LIST: u32 = 0x0162_7a40;
pub(super) const PROBE_AT: u32 = 0x0159_c500;
const TRIGGER: u32 = 0x0162_cd30;
/// What the trigger's stand writes: its centre, its reach and its frame.
const TRIGGER_STOOD: [(u32, u32); 3] = [(0x50, 0x10), (0x70, 4), (0x130, 0x40)];
/// The record clear (the search's vt `+0xc8`): `atv_node_frame` of the probe's node, then the stand.
const RECORD_CLEAR: u32 = 0x001b_d180;

/// A collidable's fields, as `tests/atv_search.rs` names them.
mod at {
    pub const NEARBY: u32 = 0x45;
    pub const CENTRE: u32 = 0x50;
    pub const RADIUS: u32 = 0x70;
    pub const RECORD: u32 = 0x7c;
    pub const CATEGORY: u32 = 0x94;
    pub const COLLIDES_WITH: u32 = 0xdc;
    pub const OBJECTS: u32 = 0xe0;
    pub const MORE_OBJECTS: u32 = 0xec;
    pub const CATEGORIES: u32 = 0xf8;
    pub const FLAGS: u32 = 0x126;
    pub const NODE: u32 = 0x174;
    pub const RAYS: u32 = 0x178;
    pub const SEGMENTS: u32 = 0x180;
    /// The record's contact count.
    pub const COUNT: u32 = 0x8c;
}

/// Desert Oasis's static world, the trigger probe's rays, and MD37a's race captures' rows.
pub(super) struct Desert {
    pub(super) world: atv_world::World,
    pub(super) props: Vec<Prop>,
    pub(super) eco: Ecosystem,
    pub(super) trees: Trees,
    pub(super) rays: Rays,
    pub(super) search: usize,
    pub(super) first_part: usize,
    rows: Vec<Row>,
}

/// One row of a race capture: the body's place, and which wheels hold a swept contact.
#[derive(Clone, Debug)]
struct Row {
    name: String,
    frame: u32,
    pos: [f32; 3],
    quat: [f32; 4],
    swept: [bool; 4],
}

fn desert(ee: &Ee) -> Option<Desert> {
    let mut d = desert_world(ee)?;
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    d.rows = race_rows(&kit.ctx("capture/md37a"))?;
    Some(d)
}

/// MD75e: [`desert`] without MD37a's race rows, which the whole frame does not need.
pub(super) fn desert_world(ee: &Ee) -> Option<Desert> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let dir = kit.ctx("mxvatv-ps2/md37a");
    let res = dir.join("mc02.res");
    let esb = dir.join("MC02.esb");
    for (p, what) in [
        (&res, "Desert Oasis's mc02.res off MX vs. ATV's disc (Env/MC02)"),
        (&esb, "Env\\MC02\\MC02.esb out of MXVSATV.PSR on MX vs. ATV's disc"),
    ] {
        if !p.exists() {
            skip(&format!("{} is not there: {what}", p.display()));
            return None;
        }
    }
    let h = image::heap();
    if ee.u32_at(h.quad + 0xc4) != h.list || ee.u32_at(h.quad + 0xd8) != h.probe || ee.u32_at(h.quad + 0x1430) != h.trigger {
        panic!("the image's quad does not hold the list and probes these addresses are from");
    }
    let file = std::fs::read(&res).unwrap();
    let archive = Res::parse(&file).unwrap();
    let read = |name: &str| -> Result<Vec<u8>, String> {
        let e = archive.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).ok_or(format!("no {name}"))?;
        archive.read(&file, e)
    };
    let scene = game_sim::track::Scene::parse(&read("Env\\mc02\\MC02.scn").unwrap()).unwrap();
    let (props, missed) = Props::load(&scene, |n| read(&format!("Col\\{n}")));
    assert!(missed.is_empty(), "MC02's meshes not read: {missed:?}");
    let eco = Ecosystem::parse(&std::fs::read(&esb).unwrap()).unwrap();
    let (groups, models) = atv_world::groups(&eco);
    let field = atv_world::terrain_box(&scene.terrain);
    let (trees, missed) = Trees::new(&eco, atv_world::plant_cell(field), &CurveTable::build());
    assert!(missed.is_empty(), "MC02's plant solids not built: {missed:?}");
    let mut world = atv_world::World::new(collidable(ee, h.probe), &props.props, &groups, models, atv_world::plant_cell(field), field);
    let search = world.objects.len();
    world.objects.push(collidable(ee, h.trigger));
    let first_part = world.objects.len();
    world.objects.extend(atv_search::part_collidables(&eco));
    Some(Desert { world, props: props.props, eco, trees, rays: image_rays(ee), search, first_part, rows: Vec::new() })
}

/// The image's collidable at `c` as the port's (`tests/atv_search.rs`'s): its reach, flags and filter.
/// The collidables its lists name are not in the port's world; each stands for a number past its end.
pub(super) fn collidable(ee: &Ee, c: u32) -> Collidable {
    let mut named = BTreeMap::new();
    let mut objects = |off: u32| {
        let (mode, at, n) = (ee.u32_at(c + off), ee.u32_at(c + off + 4), ee.u32_at(c + off + 8));
        let list = (0..n)
            .map(|i| {
                let p = ee.u32_at(at + 4 * i);
                let next = usize::MAX - named.len();
                *named.entry(p).or_insert(next)
            })
            .collect();
        Objects { mode, list }
    };
    let (o, m) = (objects(at::OBJECTS), objects(at::MORE_OBJECTS));
    let k = c + at::CATEGORIES;
    let categories = Categories { mode: ee.u32_at(k), list: (0..ee.u32_at(k + 8)).map(|i| ee.u32_at(ee.u32_at(k + 4) + 4 * i)).collect() };
    Collidable {
        class: Class::Object,
        flags: ee.u8_at(c + at::FLAGS),
        nearby: ee.u8_at(c + at::NEARBY) & 1 != 0,
        radius: F(ee.f32_at(c + at::RADIUS)),
        category: ee.u32_at(c + at::CATEGORY),
        collides_with: ee.u32_at(c + at::COLLIDES_WITH),
        objects: o,
        more_objects: m,
        categories,
        ..Collidable::default()
    }
}

fn quad_at(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

/// The trigger probe's four rays by wheel (`x` below zero the left, `z` above it the front) and the
/// port's tree of them.
pub(super) fn image_rays(ee: &Ee) -> Rays {
    let trigger = image::heap().trigger;
    let n = ee.u32_at(trigger + at::RAYS);
    assert_eq!(n, 4, "the trigger probe has a ray a wheel");
    let segs = ee.u32_at(trigger + at::SEGMENTS);
    let mut by_wheel = [None; 4];
    for k in 0..n {
        let s = ray_probe::Segment { from: quad_at(ee, segs + 0x20 * k), to: quad_at(ee, segs + 0x20 * k + 0x10) };
        let wheel = usize::from(s.from.x() > 0.0) + if s.from.z() > 0.0 { 0 } else { 2 };
        assert!(by_wheel[wheel].replace(s).is_none(), "one ray a wheel");
    }
    game_sim::kiowa::ray_tree(&by_wheel.map(Option::unwrap))
}

/// The race captures' rows (Desert Oasis's: the `race-` ones).
fn race_rows(dir: &Path) -> Option<Vec<Row>> {
    let mut names: Vec<_> = std::fs::read_dir(dir).ok().into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "csv") && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("race-")))
        .collect();
    names.sort();
    if names.len() < 8 {
        skip(&format!("{} does not hold MD37a's race captures", dir.display()));
        return None;
    }
    let mut out = Vec::new();
    for name in names {
        let text = std::fs::read_to_string(&name).unwrap();
        let mut lines = text.lines();
        let head: Vec<&str> = lines.next().unwrap().split(',').collect();
        let col = |h: &str| head.iter().position(|c| *c == h).unwrap_or_else(|| panic!("{}: no {h}", name.display()));
        let stem = name.file_stem().unwrap().to_string_lossy().into_owned();
        let pos = ["pos.x", "pos.y", "pos.z"].map(col);
        let quat = ["quat.x", "quat.y", "quat.z", "quat.w"].map(col);
        let raw = [0, 1, 2, 3].map(|k| col(&format!("wheel{k}.raw")));
        let frame = col("frame");
        for line in lines {
            let f: Vec<&str> = line.split(',').collect();
            // The wheel's +0xd4, the first word of its raw dump's eight hex digits there.
            let swept = raw.map(|c| u32::from_str_radix(&f[c][2 * 0xd4..2 * 0xd4 + 8], 16).unwrap() != 0);
            out.push(Row {
                name: stem.clone(),
                frame: f[frame].parse().unwrap(),
                pos: pos.map(|c| f[c].parse().unwrap()),
                quat: quat.map(|c| f[c].parse().unwrap()),
                swept,
            });
        }
    }
    Some(out)
}

/// The body at `pos` turned by `quat` (normalised), and the quad's node placed where the body is:
/// the body's frame less its centre of gravity (`(0, 1, 0)`) along its up row.
pub(super) fn place(r: &mut Ride, pos: [f32; 3], quat: [f32; 4]) {
    let n = quat.iter().map(|v| v * v).sum::<f32>().sqrt();
    let q = Vf::new(quat[0] / n, quat[1] / n, quat[2] / n, quat[3] / n);
    let p = Vf::new(pos[0], pos[1], pos[2], 1.0);
    r.s.body.position = p;
    r.s.body.orientation = q;
    let up = game_sim::atv_moto::quat_frame(q, p)[1];
    let node = Vf::new(pos[0] - up.x(), pos[1] - up.y(), pos[2] - up.z(), 1.0);
    r.bike.parts.scene.place(0, q, node);
    r.scene = true;
    // What the backflip reads of the node: its world rows as laid (the place leaves them stale).
    let w = r.bike.parts.scene.nodes[0].world;
    (r.bike.right_y, r.bike.up_y) = (F(w[1].y()), F(w[2].y()));
}

/// A deterministic generator for the aimed frames.
struct Aim(u64);

impl Aim {
    fn unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}

/// A quaternion turning by `yaw` about up, then `pitch` about across and `roll` about along
/// (`tests/atv_search.rs`'s).
fn euler_quat(yaw: f32, pitch: f32, roll: f32) -> [f32; 4] {
    let (sy, cy) = (yaw * 0.5).sin_cos();
    let (sp, cp) = (pitch * 0.5).sin_cos();
    let (sr, cr) = (roll * 0.5).sin_cos();
    let mul = |a: [f32; 4], b: [f32; 4]| {
        [
            a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
            a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
            a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
            a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
        ]
    };
    mul(mul([0.0, sy, 0.0, cy], [sp, 0.0, 0.0, cp]), [0.0, 0.0, sr, cr])
}

/// Frames that put each wheel's ray across each scene mesh, as `tests/atv_search.rs` aims them: a
/// triangle's centre (give or take 0.6 ft), the ray's middle there, turned any way about up and tipped
/// a little. The body where the node then is (its up row added back).
fn aimed(props: &[Prop], rays: &Rays) -> Vec<(String, [f32; 3], [f32; 4], bool)> {
    let mut g = Aim(0x4d44_3836);
    let mut out = Vec::new();
    for (o, p) in props.iter().enumerate() {
        let verts = game_sim::collide::MeshRef::of(&p.mesh);
        let tris = &p.mesh.triangles;
        for wheel in 0..4 {
            let tri = &tris[(g.unit() * tris.len() as f32) as usize % tris.len()];
            let m = p.to_world.0;
            let world: Vec<[f32; 3]> = tri
                .vertices
                .map(|v| verts.vertex(v))
                .iter()
                .map(|v| [0, 1, 2].map(|k| v.x() * m[0].0[k] + v.y() * m[1].0[k] + v.z() * m[2].0[k] + m[3].0[k]))
                .collect();
            let target = [0, 1, 2].map(|k| (world[0][k] + world[1][k] + world[2][k]) / 3.0 + g.signed() * 0.6);
            let quat = euler_quat(g.unit() * std::f32::consts::TAU, g.signed() * 0.35, g.signed() * 0.35);
            let r = game_sim::atv_moto::quat_frame(Vf::new(quat[0], quat[1], quat[2], quat[3]), Vf::ZERO);
            let s = rays.segments[wheel];
            let mid = [0, 1, 2].map(|k| (s.from.0[k] + s.to.0[k]) * 0.5);
            let at = |k: usize| target[k] - (mid[0] * r[0].0[k] + mid[1] * r[1].0[k] + mid[2] * r[2].0[k]) + r[1].0[k];
            out.push((format!("mesh {o} wheel {wheel}"), [at(0), at(1), at(2)], quat, true));
        }
    }
    out
}

/// The frames the rows give: every eighth row, and for a row with a swept contact, the eighths of the
/// way from the row before (the rows are two frames apart; the sweep ran in between).
fn frames(rows: &[Row]) -> Vec<(String, [f32; 3], [f32; 4], bool)> {
    let mut out = Vec::new();
    for (k, row) in rows.iter().enumerate() {
        let swept = row.swept.iter().any(|&s| s);
        if k % 8 == 0 && !swept {
            out.push((format!("{}@{}", row.name, row.frame), row.pos, row.quat, false));
        }
        if !swept || k == 0 || rows[k - 1].name != row.name {
            continue;
        }
        let a = &rows[k - 1];
        let dot: f32 = (0..4).map(|j| a.quat[j] * row.quat[j]).sum();
        let sign = if dot < 0.0 { -1.0 } else { 1.0 };
        for e in 0..=8 {
            let f = e as f32 / 8.0;
            let q = [0, 1, 2, 3].map(|j| a.quat[j] + (sign * row.quat[j] - a.quat[j]) * f);
            let p = [0, 1, 2].map(|j| a.pos[j] + (row.pos[j] - a.pos[j]) * f);
            out.push((format!("{}@{} {e}/8", row.name, row.frame), p, q, true));
        }
    }
    out
}

/// What a struck collidable is, on either side: a mesh by its centre's and reach's bits.
pub(super) fn image_mesh(ee: &Ee, c: u32) -> [u32; 4] {
    [ee.u32_at(c + at::CENTRE), ee.u32_at(c + at::CENTRE + 4), ee.u32_at(c + at::CENTRE + 8), ee.u32_at(c + at::RADIUS)]
}

pub(super) fn port_mesh(w: &atv_world::World, o: usize) -> [u32; 4] {
    assert!(w.meshes.contains(&o), "the port's rear contact {o} is not a mesh");
    let c = w.broad.places[o].centre;
    [c[0].0.to_bits(), c[1].0.to_bits(), c[2].0.to_bits(), w.objects[o].radius.0.to_bits()]
}

/// The Force body on the image's world, from the rows' frames: the original with `picks` unhooked
/// (the distributor among them), the port with [`quad::Steps`] answering them and
/// [`atv_search::Oasis`] its sweep. `ride` makes each frame's ride before it is placed.
fn on_desert_oasis(seed: u64, picks: &[u32], ride: impl Fn(&mut Gen, &Ride) -> Ride, want: &[&str], at_least: u32) {
    let Some(mut ee) = image() else { return };
    let Some(d) = desert(&ee) else { return };
    ee.set(28, GP);
    assert_eq!(ee.u32_at(ee.u32_at(TRIGGER + 0xc) + 0xc8), RECORD_CLEAR, "the trigger probe's record clear");
    let (spawn, _) = image::read_ride(&ee, image::IMAGE_QUAD, F(1.0 / 120.0)).unwrap_or_else(|e| panic!("{e}"));
    let answers: Answers = Rc::default();
    let echo = Rc::new(Cell::new(false));
    let log = hook_steps(&mut ee, picks, &answers, &echo);
    let Desert { world, props, eco, trees, rays, search, first_part, rows } = d;
    let mut oasis = atv_search::Oasis::new(world, &props, &eco, &trees, rays, search, first_part);
    let frames = [frames(&rows), aimed(&props, &oasis.rays)].concat();
    let mut g = Gen(seed);
    let mut t = BTreeMap::new();
    for (k, (name, pos, quat, near)) in frames.iter().enumerate() {
        let mut r = ride(&mut g, &spawn);
        place(&mut r, *pos, *quat);
        log.borrow_mut().clear();
        *answers.borrow_mut() = r.answers.iter().copied().collect();
        echo.set(r.echo);
        reset_all(&mut ee);
        poke_ride(&mut ee, &r);
        ee.set_u32(OBJ + 0x1430, TRIGGER);
        ee.set_u32(OBJ + 0xc4, IMAGE_LIST);
        ee.set_u32(TRIGGER + at::NODE, node_at(0));
        // The list as the PostProcess's gather left it: round the node, where it put the gather probe.
        let mut scene = r.bike.parts.scene;
        let node = scene.position(0);
        for j in 0..3 {
            ee.set_f32(PROBE_AT + at::CENTRE + 4 * j, node.0[j as usize]);
        }
        ee.call(GATHER, &[u64::from(IMAGE_LIST), u64::from(PROBE_AT), 3], &[], 50_000_000);
        // MD75e: the original stands its own trigger probe; the port stands the ride's. The ride's is
        // laid there first and what the stand left carried back to the harness's.
        let trigger = collidable_at(quad::Part::Trigger);
        for (o, len) in TRIGGER_STOOD {
            for w in (0..len).step_by(4) {
                ee.set_u32(TRIGGER + o + w, ee.u32_at(trigger + o + w));
            }
        }
        ee.call(GROUND_STEP, &[u64::from(OBJ)], &[r.s.dt.0], 400_000_000);
        for (o, len) in TRIGGER_STOOD {
            for w in (0..len).step_by(4) {
                ee.set_u32(trigger + o + w, ee.u32_at(TRIGGER + o + w));
            }
        }
        let was = snap_all(&ee);
        let want_hit = ee.u8_at(TRIGGER + at::FLAGS) & 0x20 != 0;
        let want_count = ee.u32_at(ee.u32_at(TRIGGER + at::RECORD) + at::COUNT);
        let want_rear = ee.u32_at(OBJ + 0x143c);

        let mut u = r.clone();
        let mut queue: std::collections::VecDeque<(Vf, Vf, Option<u8>)> = r.answers.iter().copied().collect();
        let echo_now = r.echo;
        let ground = move |q: Vf, record: &mut u8| {
            let (p, n, r) = queue.pop_front().expect("more ground queries than answers");
            if let Some(r) = r {
                *record = r;
            }
            (if echo_now { q } else { p }, n)
        };
        let mut m = Only { picks, steps: quad::Steps { bike: u.bike.clone(), rest: Vec::new(), ground, sweep: &mut oasis } };
        quad::ground_step(&u.s.p, &mut u.s.q, &mut u.s.body, u.s.dt, &mut m);
        u.bike = m.steps.bike;
        let rest = m.steps.rest;
        let ran = r.s.q.active;
        let got_hit = oasis.world.objects[search].flags & game_sim::world_search::flags::HIT != 0;
        let got_count = oasis.record.contacts.len() as u32;
        // The rear contact's collidable: the image's address in the original, the world's index in
        // the port; the same mesh.
        if ran {
            assert_eq!(got_count, want_count, "{k} {name}: the record's count");
            if got_hit != want_hit {
                // A plant's sphere, which the port does not test (`tests/atv_search.rs`): struck, and
                // no contact made, so nothing for the distributor to spread.
                assert!(want_hit && want_count == 0, "{k} {name}: the hit bit (original {want_hit})");
                tally(&mut t, "struck only a plant's sphere");
            }
            u.s.rear_node = match u.bike.parts.rear_node {
                0 => 0,
                p => {
                    assert_eq!(image_mesh(&ee, want_rear), port_mesh(&oasis.world, p as usize), "{k} {name}: the rear contact's mesh");
                    want_rear
                }
            };
        }
        reset_all(&mut ee);
        poke_ride(&mut ee, &u);
        ee.set_u32(OBJ + 0x1430, TRIGGER);
        ee.set_u32(OBJ + 0xc4, IMAGE_LIST);
        let now = snap_all(&ee);
        if let Some(i) = was.iter().zip(&now).position(|(a, b)| a != b) {
            panic!(
                "{k} {name}: {} is {:#010x} ({}) in the original and {:#010x} ({}) in the port\n{r:#?}",
                where_in_all(i),
                was[i],
                f32::from_bits(was[i]),
                now[i],
                f32::from_bits(now[i]),
            );
        }
        assert_eq!(*log.borrow(), rest, "{k} {name}: the calls");
        if !ran {
            tally(&mut t, "off");
            continue;
        }
        tally(&mut t, match (*near, name.starts_with("mesh")) {
            (_, true) => "aimed at a mesh",
            (true, _) => "near a captured contact",
            _ => "a captured row",
        });
        tally(&mut t, if want_hit { "struck" } else { "nothing struck" });
        for (i, w) in u.bike.wheels.iter().enumerate() {
            if w.swept_contact != 0 {
                tally(&mut t, ["left front swept", "right front swept", "left rear swept", "right rear swept"][i]);
            }
        }
        if u.bike.parts.rear_node != 0 {
            tally(&mut t, "rear contact kept");
        }
        if want_count > 1 {
            tally(&mut t, "two contacts or more");
        }
    }
    eprintln!("MD86: {} frames: {t:?}", frames.len());
    each_path_seen(&t, want, at_least);
}

const CONTACT_PATHS: [&str; 10] = [
    "a captured row",
    "aimed at a mesh",
    "near a captured contact",
    "struck",
    "nothing struck",
    "left front swept",
    "right front swept",
    "left rear swept",
    "rear contact kept",
    "two contacts or more",
];

/// **The contacts on Desert Oasis are the original's**: the spawned quad (`atv-r-spawn.ee.bin`)
/// placed at MD37a's race rows, the wheel clears and the distributor unhooked.
#[test]
fn the_quads_contacts_on_desert_oasis_are_the_originals() {
    on_desert_oasis(0x0086_0001, &[WHEEL_CLEAR, WHEEL_CONTACTS], |_, spawn| spawn.clone(), &CONTACT_PATHS, 3);
}

/// **The whole Force body on Desert Oasis**: every ported step and the distributor, generated rides
/// (on, the suspension answered by the harness's ground) placed at the same rows.
#[test]
fn the_quads_force_body_on_desert_oasis_is_the_originals() {
    let picks = [
        GRAVITY, PRELOAD, BACKFLIP, ENGINE_UPDATE, AIR_DRAG, AIRBORNE_CONTROL, WHEEL_CLEAR, WHEEL_CONTACTS, SUSPENSION, TIRE_FORCES,
        WHEEL_INTEGRATE, SURFACE_DRAG, SPIN_INTEGRATE, REV_LIMIT, PEAK_RATES,
    ];
    let mask = image().map_or(0, |ee| ee.u32_at(STRUCK_MASK));
    let ride = move |g: &mut Gen, spawn: &Ride| {
        let mut r = g.ride(mask);
        r.s.vstate = Some(g.bits() % 3);
        r.s.q.active = true;
        r.bike.parts.scene = spawn.bike.parts.scene;
        // MD75e: the probe the contacts stand is the image's trigger.
        let k = game_sim::atv_sync::slot(quad::Part::Trigger);
        r.bike.parts.collidables.states[k] = spawn.bike.parts.collidables.states[k];
        r
    };
    on_desert_oasis(0x0086_0002, &picks, ride, &CONTACT_PATHS, 3);
}

// --- MD75c2: the collision sync on Desert Oasis -----------------------------------------------------

/// The image's frame mesh (quad `+0xd0`); its riding probe is [`PROBE_AT`], the active one too.
const MESH_AT: u32 = 0x0162_7370;

/// What a list's candidate is on either side (`tests/atv_gather.rs`'s ids): a mesh by its centre's and
/// reach's bits, a plant by its cell, model and blend; with its distance's bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Found {
    Mesh([u32; 4], u32),
    Plant(u16, u16, u8, i8, u32),
}

/// The image's list, but for the collidables the port does not build (tallied as left out).
fn image_list(ee: &Ee, t: &mut BTreeMap<&'static str, u32>) -> Vec<Found> {
    let (at, n) = (ee.u32_at(IMAGE_LIST), ee.u32_at(IMAGE_LIST + 4));
    let mut out = Vec::new();
    for i in 0..n {
        let (e, d) = (ee.u32_at(at + 8 * i), ee.u32_at(at + 8 * i + 4));
        match ee.u8_at(e + 2) {
            2 => {
                let c = e - 0x18;
                if ee.u32_at(c + 0xc) == MESH_VTABLE && ee.u32_at(c + at::CATEGORY) == atv_world::MESH {
                    out.push(Found::Mesh(image_mesh(ee, c), d));
                } else {
                    tally(t, "a collidable the port does not build left out");
                }
            }
            3 => out.push(Found::Plant(ee.u16_at(e + 0xc), ee.u16_at(e + 0x10), ee.u8_at(e + 0x12), ee.u8_at(e + 0x15) as i8, d)),
            k => panic!("a list entry of kind {k}"),
        }
    }
    out
}

/// The port's list, meshes and plants only (its trigger probe and parts stand for nothing listed).
fn port_list(w: &atv_world::World, list: &atv_world::Nearby) -> Vec<Found> {
    let mut out = Vec::new();
    for c in &list.found {
        let (o, d) = (c.object, c.distance.0.to_bits());
        match w.objects[o].class {
            Class::Object if w.meshes.contains(&o) => out.push(Found::Mesh(port_mesh(w, o), d)),
            Class::Object => {}
            _ => {
                let p = &w.broad.places[o];
                out.push(Found::Plant(p.cell[0], p.cell[1], p.model, p.blend, d));
            }
        }
    }
    out
}

/// A ride's collidable on the image's part at `a`: the words the sync reads and writes (the node the
/// harness's), its database (`+0x20`, `db` or none) as the ride says, and a probe's tree's half extent.
/// The rest of its kind's words are the image's.
fn on_image_part(ee: &mut Ee, a: u32, c: &game_sim::atv_sync::Collidable, db: u32) {
    use game_sim::atv_sync::Kind;
    ee.set_u32(a + 0x20, if c.spatial { db } else { 0 });
    put_v(ee, a + at::CENTRE, c.centre);
    put_f(ee, a + at::RADIUS, c.reach);
    let rows = |ee: &mut Ee, o: u32, m: &[Vf; 4]| (0..4).for_each(|k| put_v(ee, a + o + 0x10 * k as u32, m[k]));
    rows(ee, 0x130, &c.frame);
    match &c.kind {
        Kind::Rays { tree, .. } => {
            ee.set_u32(a + at::NODE, node_at(0));
            // Its tree's box, which the test may grow.
            if let Some((_, half)) = tree {
                let t = ee.u32_at(a + 0x17c);
                [half.x(), half.y(), half.z()].iter().enumerate().for_each(|(j, h)| put_f(ee, t + 0x18 + 4 * j as u32, F(*h)));
            }
        }
        Kind::Mesh { local, stood, before, .. } => {
            rows(ee, 0x170, local);
            rows(ee, 0x1b0, stood);
            rows(ee, 0x1f0, before);
            ee.set_u32(a + 0x240, node_at(0));
        }
    }
}

/// **The PostProcess's collision sync on Desert Oasis** (MD75c2): the spawned quad placed at the rows'
/// frames and the aimed ones, its riding (and active) probe and frame mesh the image's own, and the
/// vehicle post, its three calls, the part sync and the gather unhooked. The original syncs the parts
/// and gathers the image's list round the probe; the port syncs [`atv_moto::Collidables::states`] and
/// [`atv_search::Oasis`] answers the gather. The quad, the scene, both parts, the calls (the parts'
/// box moves among them) and the list, entry for entry with its distances, are equal.
#[test]
fn the_quads_collision_sync_on_desert_oasis_is_the_originals() {
    use quad::Part;
    let Some(mut ee) = image() else { return };
    let Some(d) = desert(&ee) else { return };
    ee.set(28, GP);
    assert_eq!(ee.u32_at(image::IMAGE_QUAD + 0xd0), MESH_AT, "the image's frame mesh");
    let (spawn, held) = image::read_ride(&ee, image::IMAGE_QUAD, F(1.0 / 120.0)).unwrap_or_else(|e| panic!("{e}"));
    let c = &spawn.bike.parts.collidables;
    assert_eq!((c.riding, c.frame, c.active), (Some(Part::Riding), Some(Part::Frame), Some(Part::Riding)), "the spawned quad's collidables");
    assert_eq!((held.parts[0], held.parts[1]), (PROBE_AT, MESH_AT), "the riding probe and the frame mesh");
    let dbs = [ee.u32_at(PROBE_AT + 0x20), ee.u32_at(MESH_AT + 0x20)];
    assert!(dbs.iter().all(|&d| d != 0), "both parts are in the database at the spawn");

    let answers: Answers = Rc::default();
    let echo = Rc::new(Cell::new(false));
    let log = hook_steps(&mut ee, &[], &answers, &echo);
    let picks = [POST_STEPS.as_slice(), &[GATHER]].concat();
    hook_post(&mut ee, &log, &picks);
    // The parts' own entries (`+0x18`), which the post's hook does not know.
    let l = log.clone();
    ee.hooks.insert(
        SPATIAL_MOVE,
        Box::new(move |ee| {
            let v = |a: u64| Vf::new(ee.f32_at(a as u32), ee.f32_at(a as u32 + 4), ee.f32_at(a as u32 + 8), ee.f32_at(a as u32 + 12));
            let (centre, half, reach) = (v(ee.r(5)), v(ee.r(6)), F(ee.f(12)));
            l.borrow_mut().push(match ee.r(4) as u32 {
                OBJ => Call::SpatialMove { node: false, centre, half, reach },
                n if n == node_at(0) => Call::SpatialMove { node: true, centre, half, reach },
                n if n == PROBE_AT + 0x18 => Call::PartMove { part: Part::Riding, centre, half, reach },
                n if n == MESH_AT + 0x18 => Call::PartMove { part: Part::Frame, centre, half, reach },
                n => panic!("a spatial move of {n:#x}"),
            });
        }),
    );
    let table = AcosTable::build();
    let Desert { world, props, eco, trees, rays, search, first_part, rows } = d;
    let mut oasis = atv_search::Oasis::new(world, &props, &eco, &trees, rays, search, first_part);
    let frames = [frames(&rows), aimed(&props, &oasis.rays)].concat();
    let mut g = Gen(0x0075_c202);
    let mut t = BTreeMap::new();
    for (k, (name, pos, quat, _)) in frames.iter().enumerate() {
        let mut r = spawn.clone();
        place(&mut r, *pos, *quat);
        let x = &mut r.bike.parts;
        (x.body_on, x.collidables.sync) = (true, true);
        r.s.q.active = true;
        // The mesh missing one time in eight, and out of the database one time in four. The probe stays
        // in: the gather queries its database (`+0x20`).
        if g.one_in(8) {
            x.collidables.frame = None;
        }
        x.collidables.states[1].spatial = !g.one_in(4);
        // The probe's tree's box grown half the time (to a reach past the list's 30), so the gather is
        // round a reach the image's probe never had.
        if let game_sim::atv_sync::Kind::Rays { tree: Some((_, half)), .. } = &mut x.collidables.states[0].kind {
            if g.b() {
                let k = g.f(1.0, 8.0).0;
                *half = Vf::new(half.x() * k, half.y() * k, half.z() * k, half.w());
                tally(&mut t, "the probe's box grown");
            }
        }
        let lay = |ee: &mut Ee, r: &Ride| {
            reset_all(ee);
            poke_ride(ee, r);
            let c = &r.bike.parts.collidables;
            ee.set_u32(OBJ + 0xcc, PROBE_AT);
            ee.set_u32(OBJ + 0xd8, PROBE_AT);
            ee.set_u32(OBJ + 0xd0, if c.frame.is_some() { MESH_AT } else { 0 });
            ee.set_u32(OBJ + 0xc4, IMAGE_LIST);
            ee.set_u32(OBJ + 0x1430, TRIGGER);
            on_image_part(ee, PROBE_AT, &c.states[0], dbs[0]);
            on_image_part(ee, MESH_AT, &c.states[1], dbs[1]);
        };
        log.borrow_mut().clear();
        *answers.borrow_mut() = r.answers.iter().copied().collect();
        echo.set(r.echo);
        lay(&mut ee, &r);
        ee.call(AIR_STATE, &[u64::from(OBJ)], &[r.s.dt.0], 400_000_000);
        // The image's parts, read back onto the harness's, where the snap compares them.
        for (p, a) in [(Part::Riding, PROBE_AT), (Part::Frame, MESH_AT)] {
            let mut part = image::collidable(&ee, a, &mut Vec::new());
            part.spatial = ee.u32_at(a + 0x20) != 0;
            lay_collidable(&mut ee, p, &part);
        }
        let was = snap_all(&ee);
        let want_list = image_list(&ee, &mut t);
        let want_bound = ee.u32_at(IMAGE_LIST + 0x20);
        let want_centre: [u32; 4] = std::array::from_fn(|j| ee.u32_at(IMAGE_LIST + 0x10 + 4 * j as u32));

        let mut u = r.clone();
        let mut queue: std::collections::VecDeque<(Vf, Vf, Option<u8>)> = r.answers.iter().copied().collect();
        let echo_now = r.echo;
        let ground = move |q: Vf, record: &mut u8| {
            let (p, n, r) = queue.pop_front().expect("more ground queries than answers");
            if let Some(r) = r {
                *record = r;
            }
            (if echo_now { q } else { p }, n)
        };
        let mut m = Only { picks: &picks, steps: quad::Steps { bike: u.bike.clone(), rest: Vec::new(), ground, sweep: &mut oasis } };
        quad::air_state(&mut u.s.q, &mut u.s.body, u.s.dt, &table, &mut m);
        u.bike = m.steps.bike;
        let rest = m.steps.rest;
        lay(&mut ee, &u);
        for p in [Part::Riding, Part::Frame] {
            lay_collidable(&mut ee, p, &u.bike.parts.collidables.states[game_sim::atv_sync::slot(p)]);
        }
        let now = snap_all(&ee);
        if let Some(i) = was.iter().zip(&now).position(|(a, b)| a != b) {
            panic!(
                "{k} {name}: {} is {:#010x} ({}) in the original and {:#010x} ({}) in the port",
                where_in_all(i),
                was[i],
                f32::from_bits(was[i]),
                now[i],
                f32::from_bits(now[i]),
            );
        }
        assert_eq!(*log.borrow(), rest, "{k} {name}: the calls");
        assert!(oasis.gathered, "{k} {name}: the port gathered");
        assert_eq!(oasis.list.bound.0.to_bits(), want_bound, "{k} {name}: the list's bound");
        assert_eq!(oasis.list.centre.map(|f| f.0.to_bits()), want_centre, "{k} {name}: the list's centre");
        let got_list = port_list(&oasis.world, &oasis.list);
        assert_eq!(got_list, want_list, "{k} {name}: the list");

        tally(&mut t, if u.bike.parts.collidables.frame.is_some() { "the mesh synced" } else { "no frame mesh" });
        for c in &rest {
            if let Call::PartMove { part, .. } = c {
                tally(&mut t, if *part == Part::Riding { "the probe's box moved" } else { "the mesh's box moved" });
            }
        }
        tally(&mut t, if got_list.is_empty() { "nothing gathered" } else { "something gathered" });
        if got_list.iter().any(|f| matches!(f, Found::Mesh(..))) {
            tally(&mut t, "a mesh gathered");
        }
        if got_list.iter().any(|f| matches!(f, Found::Plant(..))) {
            tally(&mut t, "a plant gathered");
        }
    }
    eprintln!("MD75c2: {} frames: {t:?}", frames.len());
    let want = ["the probe's box grown", "the mesh synced", "no frame mesh", "the probe's box moved", "the mesh's box moved", "something gathered", "a mesh gathered", "a plant gathered"];
    each_path_seen(&t, &want, 10);
}
