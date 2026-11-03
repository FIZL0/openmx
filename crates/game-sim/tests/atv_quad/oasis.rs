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
const PROBE_AT: u32 = 0x0159_c500;
const TRIGGER: u32 = 0x0162_cd30;
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
struct Desert {
    world: atv_world::World,
    props: Vec<Prop>,
    eco: Ecosystem,
    trees: Trees,
    rays: Rays,
    search: usize,
    first_part: usize,
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
    let q = image::IMAGE_QUAD;
    if ee.u32_at(q + 0xc4) != IMAGE_LIST || ee.u32_at(q + 0xd8) != PROBE_AT || ee.u32_at(q + 0x1430) != TRIGGER {
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
    let mut world = atv_world::World::new(collidable(ee, PROBE_AT), &props.props, &groups, models, atv_world::plant_cell(field), field);
    let search = world.objects.len();
    world.objects.push(collidable(ee, TRIGGER));
    let first_part = world.objects.len();
    world.objects.extend(atv_search::part_collidables(&eco));
    let rows = race_rows(&kit.ctx("capture/md37a"))?;
    Some(Desert { world, props: props.props, eco, trees, rays: image_rays(ee), search, first_part, rows })
}

/// The image's collidable at `c` as the port's (`tests/atv_search.rs`'s): its reach, flags and filter.
/// The collidables its lists name are not in the port's world; each stands for a number past its end.
fn collidable(ee: &Ee, c: u32) -> Collidable {
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
fn image_rays(ee: &Ee) -> Rays {
    let n = ee.u32_at(TRIGGER + at::RAYS);
    assert_eq!(n, 4, "the trigger probe has a ray a wheel");
    let segs = ee.u32_at(TRIGGER + at::SEGMENTS);
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
fn place(r: &mut Ride, pos: [f32; 3], quat: [f32; 4]) {
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
fn image_mesh(ee: &Ee, c: u32) -> [u32; 4] {
    [ee.u32_at(c + at::CENTRE), ee.u32_at(c + at::CENTRE + 4), ee.u32_at(c + at::CENTRE + 8), ee.u32_at(c + at::RADIUS)]
}

fn port_mesh(w: &atv_world::World, o: usize) -> [u32; 4] {
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
        ee.call(GROUND_STEP, &[u64::from(OBJ)], &[r.s.dt.0], 400_000_000);
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
        r
    };
    on_desert_oasis(0x0086_0002, &picks, ride, &CONTACT_PATHS, 3);
}
