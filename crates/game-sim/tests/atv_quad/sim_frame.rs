//! MD105 — **the ATV's whole sim frame**, F31's way: SLUS-21104's sim object frame (`atv_sim_frame`,
//! the sim object's vt `+0x40`) over `atv-r-spawn.ee.bin`, run in the interpreter with F31's hooks
//! (DIntr/EIntr, the syscall stubs, the stunt packet's counter read) and the ground query answered by
//! the port's terrain, chained frame after frame over `frame.rs`'s palm ride ([`at_the_palm`]).
//!
//! The world step alone takes the quad through the palm with no contact (MD37b part 3). The whole
//! frame does not: the collision record's two virtuals on the sim's update list (`+0x7c` the clear,
//! `+0x84` the pass, the same record class as the bike's, F31b4) file the palm's contacts, the quad
//! wrecks on them, the frame's tail puts it back at its spot, and it strikes again.
//!
//! [`palm`] runs two machines side by side: the original's frame whole, and the frame in F31a's
//! pieces with the quad's record's virtuals the port's ([`record::clear`], [`record::pass`]). Every
//! word of RAM outside the interpreter's stack is equal after every frame.

use super::*;
use game_sim::contact_search::Ground;
use game_sim::handling::ground;
use game_sim::handling::record::{self, Collides, Hit, Record, Struck, Swept};
use game_sim::handling::seed;
use game_sim::track;

#[path = "sim_tail.rs"]
mod sim_tail;

/// The image's sim object (vtable `0x003b9320` at `+4`) and its frame (vt `+0x40`). Its world is the
/// object's `+0x6c` on, the update list at `+0x24`, the timer list at `+0x3c`.
const SIM: u32 = 0x0065_bba0;
const SIM_FRAME: u32 = 0x0020_4040;
const SKIP_FLAG: u32 = 0x003e_ab60;
const LIST24_GET: u32 = 0x0020_8390;
const TAIL: u32 = 0x0012_8af0;
const LIST3C_GET: u32 = 0x0020_80f0;
const LIST3C_REMOVE: u32 = 0x0020_8040;
const RECORD_CLEAR: u32 = 0x0020_4c30;
const RECORD_PASS: u32 = 0x0020_51f0;
const IRQ_OFF: u32 = 0x0034_8fa0;
const IRQ_ON: u32 = 0x0034_8ff0;
const COUNTER_LUI: u32 = 0x004e_7f44;
const COUNTER_LW: u32 = 0x004e_7f50;
const COUNTER_K: u32 = 250;
/// The record's callees F31b4 names.
const NODE_ROTATE: u32 = 0x0019_5230;
const NODE_SET: u32 = 0x0019_4b10;
const NODE_TIP: u32 = 0x0019_52a0;
const NODE_POSITION: u32 = 0x0019_4c20;
const PROBE_SET: u32 = 0x001b_9290;
const PROBE_PLACE: u32 = 0x0011_4360;
const SEARCH_NEAR: u32 = 0x0011_51c0;
const SEARCH_ALL: u32 = 0x0011_48e0;
const TERRAIN_QUERY: u32 = 0x0048_38f0;
/// The global whose `+4` is the terrain service (F31c).
const SERVICE: u32 = 0x003e_ab60;
/// The ground query's ring of read cells (10 of 0x70 bytes), and the terrain's ring words (`+0x9e0`
/// set up, `+0x9e4` index): the port reads the map straight and keeps no ring (`atv_ground`), so these
/// are left out of the compare, as S278b does.
const RING: (u32, u32) = (0x0054_1550, 10 * 0x70);
const TERRAIN_RING: [u32; 2] = [TERRAIN + 0x9e0, TERRAIN + 0x9e4];
const BODY_DAMP: [u32; 2] = [0x20, 0x68];
const BUDGET: u64 = 400_000_000;
const DT: u32 = 0x3d08_850a;

/// F31's hooks: the kernel's interrupt switches, every syscall stub (the answer a0, the thread id 1),
/// and the stunt packet's read of RCNT0 turned to a stand-in word.
fn hooked(ee: &mut Ee) {
    ee.hooks.insert(IRQ_OFF, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.hooks.insert(IRQ_ON, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    let at = ee.scratch(COUNTER_K);
    assert_eq!([ee.u32_at(COUNTER_LUI), ee.u32_at(COUNTER_LW)], [0x3c02_1000, 0x8c50_0000], "the stunt packet's counter read");
    ee.set_u32(COUNTER_LUI, 0x3c02_0000 | (at.wrapping_add(0x8000) >> 16));
    ee.set_u32(COUNTER_LW, 0x8c50_0000 | (at & 0xffff));
    ee.set_u32(at, 0);
    for a in (0x0030_0000..0x0036_0000u32).step_by(4) {
        if ee.u32_at(a) >> 16 == 0x2403 && ee.u32_at(a + 4) == 0x0000_000c && ee.u32_at(a + 8) == 0x03e0_0008 {
            let n = ee.u32_at(a) & 0xffff;
            ee.hooks.insert(a, Box::new(move |ee: &mut Ee| {
                let v = if n == 0x2f { 1 } else { ee.r(4) };
                ee.set(2, v);
            }));
        }
    }
}

/// The ground query answered by the port's terrain (MD76a), counting the queries.
fn ground_hooked(ee: &mut Ee, terrain: &atv_ground::Terrain<'static>, asked: Rc<Cell<u32>>) {
    let t = terrain.clone();
    ee.hooks.insert(GROUND_QUERY, Box::new(move |ee| {
        let (p, n, rec) = (ee.r(5) as u32, ee.r(6) as u32, ee.r(8) as u32);
        let mut record = ee.u8_at(rec);
        let (point, normal) = t.ground(get_v(ee, p), Vf::ZERO, &mut record);
        put_v(ee, p, point);
        put_v(ee, n, normal);
        ee.write(rec, 1, u128::from(record));
        asked.set(asked.get() + 1);
    }));
}

fn run(ee: &mut Ee, f: u32, ints: &[u64], x: F) -> u32 {
    ee.set(28, GP);
    ee.call(f, ints, &[x.0], BUDGET);
    ee.r(2) as u32
}

/// The frame in F31a's pieces: the update list's every vt `+0x7c`, then every vt `+0x84`, the
/// world step on `+0x6c`, the tail, and the timer list's vt `+8` (an element answering non-zero
/// taken out). With `port`, the record at `port`'s two virtuals are [`port_record`]'s (with `spoil`,
/// its first contact's normal one bit off in each lane). Answers the
/// update list's elements, and the terrain queries the port's record made.
fn frame_in_pieces(ee: &mut Ee, dt: F, port: Option<(u32, &track::Terrain<'_>)>, spoil: bool, tail: bool) -> (Vec<u32>, u32) {
    assert_eq!(ee.u8_at(ee.u32_at(SKIP_FLAG) + 0x225) & 8, 0, "the frame is not skipped");
    let (mut elements, mut queries) = (Vec::new(), 0);
    for (slot, original) in [(0x7c, RECORD_CLEAR), (0x84, RECORD_PASS)] {
        for i in 0..ee.u32_at(SIM + 0x24) {
            let e = run(ee, LIST24_GET, &[u64::from(SIM + 0x24), u64::from(i)], F::ZERO);
            if slot == 0x7c {
                elements.push(e);
            }
            let f = ee.u32_at(ee.u32_at(e + 4) + slot);
            match port {
                Some((mine, map)) if e == mine => {
                    assert_eq!(f, original, "the quad's record's vt +{slot:#x}");
                    let t = ee.u32_at(e + 0xec);
                    let terrain = (t != 0).then(|| ground::read_terrain(&|a| ee.u32_at(a), t, Some(map)));
                    queries += port_record(ee, e, slot, dt, terrain.as_ref(), spoil);
                }
                _ => {
                    run(ee, f, &[u64::from(e)], dt);
                }
            }
        }
    }
    run(ee, WORLD_STEP, &[u64::from(SIM + 0x6c)], dt);
    if tail {
        let (quads, _) = sim_tail::port_tail(ee, image::IMAGE_QUAD, dt);
        assert_eq!(quads, 1, "the quad's listener steps once a frame");
    } else {
        run(ee, TAIL, &[u64::from(SIM)], dt);
    }
    let mut i = 0;
    while i < ee.u32_at(SIM + 0x3c) {
        let e = run(ee, LIST3C_GET, &[u64::from(SIM + 0x3c), u64::from(i)], F::ZERO);
        if run(ee, ee.u32_at(ee.u32_at(e) + 8), &[u64::from(e)], dt) != 0 {
            run(ee, LIST3C_REMOVE, &[u64::from(SIM + 0x3c), u64::from(i)], F::ZERO);
        } else {
            i += 1;
        }
    }
    (elements, queries)
}

fn vf_at(ee: &Ee, a: u32) -> Vf {
    seed::Words::v(&|x| ee.u32_at(x), a)
}

/// The quad's [`quad::Body`] off the ATV body at `at` (`tests/atv_quad.rs`' map).
const QUAD_BODY: [u32; 8] = [0xa0, 0x1b0, 0x190, 0x1d0, 0x80, 0xc0, 0x70, 0xd0];

fn quad_body(ee: &Ee, at: u32) -> Body {
    let v = |o: u32| vf_at(ee, at + o);
    let [force, moment, orientation, ang_vel, velocity, reset_each_frame, position, correction] = QUAD_BODY.map(v);
    Body { force, moment, orientation, ang_vel, velocity, reset_each_frame, position, correction }
}

fn quad_body_words(b: &Body, at: u32) -> Vec<(u32, u32)> {
    let vs = [b.force, b.moment, b.orientation, b.ang_vel, b.velocity, b.reset_each_frame, b.position, b.correction];
    let mut out = Vec::new();
    for (o, v) in QUAD_BODY.into_iter().zip(vs) {
        for (k, w) in v.to_bits().into_iter().enumerate() {
            out.push((at + o + 4 * k as u32, w));
        }
    }
    out
}

/// The record at `at` in the interpreter's RAM as [`record`]'s port sees its outside
/// (`tests/mxvatv_bike_ride.rs`' F31b4 glue): each call is the original's (scratch slots 0..2 hold
/// the arguments), the other records a world ray hands a contact to are read on first use and held,
/// and the terrain query is the port's [`ground::under`].
struct RecordInRam<'a, 't> {
    ee: &'a mut Ee,
    at: u32,
    terrain: Option<&'t atv_ground::Terrain<'t>>,
    others: BTreeMap<u32, (Record, Record)>,
    queries: u32,
}

impl RecordInRam<'_, '_> {
    fn run(&mut self, f: u32, ints: &[u64]) {
        self.ee.set(28, GP);
        self.ee.call(f, ints, &[], BUDGET);
    }
    fn put(&mut self, k: u32, v: Vf) -> u64 {
        let a = self.ee.scratch(k);
        put_v(self.ee, a, v);
        u64::from(a)
    }
    fn word(&self, o: u32) -> u32 {
        self.ee.u32_at(self.at + o)
    }
    fn struck(&self, obj: u32) -> Struck {
        let ee = &*self.ee;
        let m = ee.u32_at(obj + 0x8c);
        let mover = if m != 0 && ee.u32_at(m + 0x1a0) == record::MOVER_KIND { m } else { 0 };
        Struck { object: obj, other: ee.u32_at(obj + 0x90), mover, category: ee.u32_at(obj + 0x94) }
    }
    fn search_with(&mut self, probe: u32, nearby: bool) {
        let list = self.word(0xfc);
        if nearby {
            self.run(SEARCH_NEAR, &[u64::from(probe), u64::from(list), 1]);
        } else {
            self.run(SEARCH_ALL, &[u64::from(probe), 0]);
        }
    }
    /// The words of the record and of the others back into RAM where they changed (and the last
    /// segment, whole), and the body at `body`.
    fn finish(self, read: &Record, rec: &Record, body: Option<(&Body, u32)>) {
        let RecordInRam { ee, at, others, .. } = self;
        let mut words = Vec::new();
        for (id, was, now) in std::iter::once((at, read, rec)).chain(others.iter().map(|(&id, (was, now))| (id, was, now))) {
            let list = ee.u32_at(id + 0xf8);
            let old: std::collections::HashMap<u32, u32> = record::words(was, id, list).into_iter().collect();
            let segment = id + 0xc0..id + 0xe0;
            words.extend(record::words(now, id, list).into_iter().filter(|(a, w)| old.get(a) != Some(w) || segment.contains(a)));
        }
        if let Some((b, a)) = body {
            words.extend(quad_body_words(b, a));
        }
        for (a, w) in words {
            ee.set_u32(a, w);
        }
    }
}

impl Collides for RecordInRam<'_, '_> {
    fn damp(&mut self, linear: Vf, angular: Vf) {
        let body = self.word(0xe4);
        for (slot, v) in BODY_DAMP.into_iter().zip([linear, angular]) {
            let p = self.put(0, v);
            let f = self.ee.u32_at(self.ee.u32_at(body + 0x110) + slot);
            self.run(f, &[u64::from(body), p]);
        }
    }
    fn node_rotate(&mut self, v: Vf) -> Vf {
        let (out, p) = (self.ee.scratch(1), self.put(0, v));
        let node = self.word(0xe8);
        self.run(NODE_ROTATE, &[u64::from(out), u64::from(node), p]);
        vf_at(self.ee, out)
    }
    fn node_position(&mut self) -> Vf {
        let node = self.word(0xe8);
        self.run(NODE_POSITION, &[u64::from(node), 0, u64::from(self.at + 0xd0)]);
        vf_at(self.ee, self.at + 0xd0)
    }
    fn node_place(&mut self, v: Vf) -> Vf {
        let (out, p) = (self.ee.scratch(1), self.put(0, v));
        let node = self.word(0xe8);
        self.run(NODE_TIP, &[u64::from(out), u64::from(node), p]);
        vf_at(self.ee, out)
    }
    fn node_move(&mut self, at: Vf) {
        let p = self.put(0, at);
        let node = self.word(0xe8);
        self.run(NODE_SET, &[u64::from(node), 0, p]);
    }
    fn node_up(&self) -> [F; 3] {
        let node = self.word(0xe8);
        [0x74, 0x84, 0x94].map(|o| F(self.ee.f32_at(node + o)))
    }
    fn chassis_velocity(&self) -> Vf {
        vf_at(self.ee, self.word(0xe0) + 0x250)
    }
    fn sweep(&mut self, from: Vf, to: Vf, nearby: bool) -> Option<Swept> {
        put_v(self.ee, self.at + 0xc0, from);
        put_v(self.ee, self.at + 0xd0, to);
        let probe = self.word(0x204);
        self.run(PROBE_SET, &[u64::from(probe), u64::from(self.at + 0xc0), 0]);
        self.run(PROBE_PLACE, &[u64::from(probe)]);
        self.search_with(probe, nearby);
        if self.ee.u8_at(probe + 0x126) & 0x20 == 0 {
            return None;
        }
        let hits = self.ee.u32_at(probe + 0x7c);
        let obj = self.ee.u32_at(self.ee.u32_at(hits + 0x9c));
        let struck = if obj == 0 { Struck::default() } else { self.struck(obj) };
        let normal = vf_at(self.ee, self.ee.u32_at(hits + 0x94));
        let point = vf_at(self.ee, self.ee.u32_at(hits + 0x98));
        Some(Swept { normal, point, struck })
    }
    fn place_part(&mut self) {
        let part = self.word(0x200);
        self.run(PROBE_PLACE, &[u64::from(part)]);
    }
    fn part_rays(&self) -> Vec<[Vf; 2]> {
        let part = self.word(0x200);
        let (n, rays) = (self.ee.u32_at(part + 0x178) as i32, self.ee.u32_at(part + 0x180));
        (0..n.max(0) as u32).map(|i| [vf_at(self.ee, rays + 0x20 * i), vf_at(self.ee, rays + 0x20 * i + 0x10)]).collect()
    }
    fn search(&mut self, nearby: bool) -> Vec<Hit> {
        let part = self.word(0x200);
        self.search_with(part, nearby);
        let h = self.ee.u32_at(part + 0x7c);
        let n = self.ee.u32_at(h + 0x8c) as i32;
        let [rays, normals, points, objects] = [0x90, 0x94, 0x98, 0x9c].map(|o| self.ee.u32_at(h + o));
        (0..n.max(0) as u32)
            .map(|i| Hit {
                from: vf_at(self.ee, rays + 0x20 * i),
                to: vf_at(self.ee, rays + 0x20 * i + 0x10),
                normal: vf_at(self.ee, normals + 0x10 * i),
                point: vf_at(self.ee, points + 0x10 * i),
                struck: self.struck(self.ee.u32_at(objects + 4 * i)),
            })
            .collect()
    }
    fn terrain_box(&self) -> [Vf; 2] {
        let t = self.word(0xec);
        [vf_at(self.ee, t + 0x30), vf_at(self.ee, t + 0x40)]
    }
    fn under(&mut self, g: &mut Ground) {
        self.queries += 1;
        if let Some(t) = self.terrain {
            ground::under(t, g);
            // The terrain service's vt +8 keeps a copy of the answer (point, normal, the surface
            // word, weights), which the record then reads back through its other slots.
            let service = self.ee.u32_at(self.ee.u32_at(SERVICE) + 4);
            put_v(self.ee, service, g.point);
            put_v(self.ee, service + 0x10, g.normal);
            self.ee.set_u32(service + 0x20, u32::from_le_bytes(g.surfaces));
            put_v(self.ee, service + 0x30, g.weights);
            return;
        }
        let buf = self.ee.scratch(2);
        put_v(self.ee, buf, g.point);
        put_v(self.ee, buf + 0x10, g.normal);
        self.ee.set_u32(buf + 0x20, u32::from_le_bytes(g.surfaces));
        put_v(self.ee, buf + 0x30, g.weights);
        let terrain = self.word(0xec);
        self.run(TERRAIN_QUERY, &[u64::from(terrain), u64::from(buf)]);
        g.point = vf_at(self.ee, buf);
        g.normal = vf_at(self.ee, buf + 0x10);
        g.surfaces = self.ee.u32_at(buf + 0x20).to_le_bytes();
        g.weights = vf_at(self.ee, buf + 0x30);
    }
    fn velocity(&mut self, s: &Struck) -> Vf {
        if s.other != 0 {
            let b = self.ee.u32_at(s.other + 0xe4);
            return if b != 0 { vf_at(self.ee, b + 0x80) } else { vf_at(self.ee, self.ee.u32_at(s.other + 0xe0) + 0x250) };
        }
        let out = self.ee.scratch(1);
        let f = self.ee.u32_at(self.ee.u32_at(s.mover + 4) + 0xc0);
        self.run(f, &[u64::from(out), u64::from(s.mover)]);
        vf_at(self.ee, out)
    }
    fn other(&mut self, id: u32) -> &mut Record {
        let ee = &*self.ee;
        let o = self.others.entry(id).or_insert_with(|| {
            let r = record::read(&|a| ee.u32_at(a), id);
            (r.clone(), r)
        });
        &mut o.1
    }
}

/// The record at `at`'s virtual `slot` (`+0x7c` or `+0x84`) as the port's, on the record in RAM, its
/// body the quad's off `+0xe4`. Answers how many terrain queries it made.
fn port_record(ee: &mut Ee, at: u32, slot: u32, dt: F, terrain: Option<&atv_ground::Terrain<'_>>, spoil: bool) -> u32 {
    let mut rec = record::read(&|a| ee.u32_at(a), at);
    let read = rec.clone();
    let body_at = ee.u32_at(at + 0xe4);
    let mut body = (body_at != 0).then(|| quad_body(ee, body_at));
    let mut w = RecordInRam { ee, at, terrain, others: BTreeMap::new(), queries: 0 };
    if slot == 0x7c {
        record::clear(&mut rec, &mut w);
    } else {
        record::pass(&mut rec, at, body.as_mut(), dt, &mut w);
        if spoil && let Some(c) = rec.contacts.first_mut() {
            c.normal = Vf::from_bits(c.normal.to_bits().map(|w| w ^ 1));
        }
    }
    let queries = w.queries;
    w.finish(&read, &rec, body.as_ref().map(|b| (b, body_at)));
    queries
}

/// Every RAM word that differs between the two machines, outside the interpreter's stack (frames
/// and scratch slots, which the pieces' separate calls lay differently) and the ground query's ring
/// ([`RING`], [`TERRAIN_RING`]). A word that holds an address
/// in the stack on both is equal too: two globals (`0x003dce40`, `0x003dce48`) keep a pointer into a
/// caller's frame, which sits deeper or shallower as the calls nest.
fn ram_differs(a: &Ee, b: &Ee) -> Vec<(u32, u32, u32)> {
    let stack = (a.stack as usize).saturating_sub(0x10_0000);
    let in_stack = |w: u32| (stack..a.ram.len()).contains(&(w as usize & 0x1fff_ffff));
    let mut out = Vec::new();
    for (k, (x, y)) in a.ram[..stack].chunks(0x1000).zip(b.ram[..stack].chunks(0x1000)).enumerate() {
        if x == y {
            continue;
        }
        for i in (0..x.len()).step_by(4) {
            let (p, q) = (u32::from_le_bytes(x[i..i + 4].try_into().unwrap()), u32::from_le_bytes(y[i..i + 4].try_into().unwrap()));
            let a = (k * 0x1000 + i) as u32;
            let ring = (RING.0..RING.0 + RING.1).contains(&a) || TERRAIN_RING.contains(&a);
            if p != q && !(in_stack(p) && in_stack(q)) && !ring {
                out.push((a, p, q));
            }
        }
    }
    out
}

/// What the original's whole frames went through on the palm ride: per frame, the record's contact
/// count, whether its active part is the riding one, the quad's wreck count (`+0x6e4`), and where
/// the body was.
#[derive(Debug, Default)]
struct Palm {
    contacts: Vec<u32>,
    riding: Vec<bool>,
    wrecks: Vec<u32>,
    at: Vec<Vf>,
    queries: u32,
    port_queries: u32,
    /// The words that differed on the spoiled frame.
    spoiled: Vec<u32>,
}

impl Palm {
    /// The frames where the record's part swapped (riding to wreck or back).
    fn swaps(&self) -> Vec<usize> {
        (1..self.riding.len()).filter(|&k| self.riding[k] != self.riding[k - 1]).collect()
    }
}

/// The palm ride, `frames` frames, the original's whole sim frame on one machine; on a second, with
/// `port` given, the frame in pieces (the quad's record's virtuals the port's when it is `true`; on
/// frame `spoil`, spoiled, and the run stops there with the words that differ), every RAM word outside
/// the stack equal to the first's after every frame. Both get [`at_the_palm`]'s pad off the first's
/// body before each frame.
fn palm(frames: usize, port: Option<bool>, spoil: Option<usize>, tail: bool) -> Option<Palm> {
    let pre = image()?;
    let trn = terrain_file()?;
    let trn: &'static [u8] = Box::leak(trn.into_boxed_slice());
    let map: &'static track::Terrain<'static> = Box::leak(Box::new(track::Terrain::parse(trn).unwrap()));
    let terrain = atv_ground::Terrain::of(map);
    let (_, at) = image::read_ride(&pre, image::IMAGE_QUAD, F::ZERO).unwrap_or_else(|e| panic!("{e}"));
    let quad = image::IMAGE_QUAD;
    assert_eq!(pre.u32_at(SIM + 4), 0x003b_9320, "the image's sim object");
    assert_eq!(pre.u32_at(pre.u32_at(SIM + 4) + 0x40), SIM_FRAME, "the sim object's frame");
    assert_eq!(SIM + 0x6c, IMAGE_WORLD, "the sim object's world");
    assert_eq!(pre.u32_at(quad + 0xc8), at.record, "the quad's collision record");
    assert_eq!(pre.u32_at(pre.u32_at(at.record + 4) + 0x7c), RECORD_CLEAR, "the record is F31b4's class");
    let riding_part = pre.u32_at(at.record + 0x200);

    let mut ee = pre;
    hooked(&mut ee);
    let asked = Rc::new(Cell::new(0));
    ground_hooked(&mut ee, &terrain, asked.clone());
    let mut twin = port.map(|_| {
        let mut t = image().unwrap();
        hooked(&mut t);
        ground_hooked(&mut t, &terrain, Rc::new(Cell::new(0)));
        t
    });
    let dt = F::from_bits(DT);
    let mut seen = Palm::default();
    for k in 0..frames {
        let p = at_the_palm(k, get_v(&ee, at.body + 0x70), get_v(&ee, at.body + 0x80));
        lay_pad(&mut ee, quad, p);
        run(&mut ee, SIM_FRAME, &[u64::from(SIM)], dt);
        if let Some(t) = twin.as_mut() {
            lay_pad(t, quad, p);
            let (elements, q) = frame_in_pieces(t, dt, (port == Some(true)).then_some((at.record, map)), spoil == Some(k), tail);
            assert!(elements.contains(&at.record), "frame {k}: the quad's record is not on the update list: {elements:#x?}");
            seen.port_queries += q;
            let bad = ram_differs(&ee, t);
            if spoil == Some(k) {
                seen.spoiled = bad.iter().map(|&(a, ..)| a).collect();
                return Some(seen);
            }
            let show = bad
                .iter()
                .take(80)
                .map(|&(a, w, p)| format!("  {a:#010x} (quad +{:#x}, record +{:#x}): original {w:#010x}, port {p:#010x}", a.wrapping_sub(quad), a.wrapping_sub(at.record)))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(bad.is_empty(), "frame {k} ({p:?}): {} words differ from the original's whole frame:\n{show}", bad.len());
        }
        seen.contacts.push(ee.u32_at(at.record + 0xf0));
        let part = ee.u32_at(at.record + 0x200);
        seen.riding.push(part == riding_part);
        seen.wrecks.push(ee.u32_at(quad + 0x6e4));
        seen.at.push(get_v(&ee, at.body + 0x70));
    }
    seen.queries = asked.get();
    eprintln!(
        "MD105: {frames} frames, {} ground queries, {} by the port's record; first contact frame {:?}, part swaps {:?}, wrecks {:?}",
        seen.queries,
        seen.port_queries,
        seen.contacts.iter().position(|&c| c != 0),
        seen.swaps(),
        seen.wrecks.last()
    );
    Some(seen)
}

const PALM_RIDE: usize = 300;

/// **The original's whole sim frame strikes the palm**: [`at_the_palm`]'s ride, frame after frame,
/// files the palm's contacts from frame 89, wrecks on frame 96 (the record's active part becomes the
/// wreck's and the quad's wreck count 1), puts the quad back at its spot on frame 164, and wrecks on
/// the palm a second time on frame 256. The frame in F31a's pieces (the update list's two virtuals,
/// the world step, the tail, the timer list), all the original's, leaves RAM as the whole frame does
/// every frame.
#[test]
fn the_quads_whole_sim_frame_strikes_the_palm_wrecks_and_is_put_back() {
    let Some(seen) = palm(PALM_RIDE, Some(false), None, false) else { return };
    check_palm(&seen);
}

fn check_palm(seen: &Palm) {
    assert_eq!(seen.contacts.iter().position(|&c| c != 0), Some(89), "the first frame with the palm's contacts");
    assert_eq!(seen.swaps(), [96, 164, 256], "wrecked, put back and wrecked again");
    assert_eq!((seen.wrecks[95], seen.wrecks[96], seen.wrecks[255], seen.wrecks[256]), (0, 1, 1, 2), "the wreck count");
    let (spawn, back) = (seen.at[0], seen.at[164]);
    let d = ((spawn.x() - back.x()).powi(2) + (spawn.z() - back.z()).powi(2)).sqrt();
    assert!(d < 0.5, "the put-back is not at the spawn: {spawn:?} vs {back:?}");
}

/// MD105 — **the quad's collision record is the port's through the strike, wreck and put-back**:
/// the palm ride with the record's clear and pass ([`record::clear`], [`record::pass`]) run in Rust
/// on the record in RAM, every other piece of the frame the original's, against the original's whole
/// frame: every RAM word outside the interpreter's stack equal after every one of the 300 frames,
/// the palm's contacts filed by the port.
#[test]
fn the_quads_record_is_the_ports_through_the_palm_strike_wreck_and_put_back() {
    let Some(seen) = palm(PALM_RIDE, Some(true), None, false) else { return };
    check_palm(&seen);
}

/// The comparison above goes red where the port's record is wrong: the first contact's normal one
/// bit off in each lane on the strike's first frame (89) shows in those four words of the record's
/// list, and in the quad the same frame's world step pushes with it.
#[test]
fn a_spoiled_contact_on_the_strike_goes_red() {
    let Some(seen) = palm(90, Some(true), Some(89), false) else { return };
    let pre = image().unwrap();
    let list = pre.u32_at(image::read_ride(&pre, image::IMAGE_QUAD, F::ZERO).unwrap().1.record + 0xf8);
    let normal: Vec<u32> = (0..4).map(|k| list + 0x30 + 4 * k).collect();
    assert!(normal.iter().all(|a| seen.spoiled.contains(a)), "the spoiled normal's words are not red: {:#x?}", seen.spoiled);
    assert!(seen.spoiled.iter().any(|&a| (image::IMAGE_QUAD..image::IMAGE_QUAD + 0x1800).contains(&a)), "the push did not reach the quad");
}

/// MD105a — **the quad's frame tail is the port's through the strike, wreck and put-back**: the palm
/// ride with the tail ([`sim_tail::port_tail`]: the walk, the quad's update and under it the vehicle's
/// inner update, the hazard clock, the wreck test, entry and unwreck, the ray swaps, the put-back and
/// the animation driver) run in Rust on the quad in RAM, beside the port's record, against the
/// original's whole frame: every RAM word outside the stack and the ground ring equal after every one
/// of the 300 frames.
#[test]
fn the_quads_tail_is_the_ports_through_the_palm_strike_wreck_and_put_back() {
    let Some(seen) = palm(PALM_RIDE, Some(true), None, true) else { return };
    check_palm(&seen);
}
