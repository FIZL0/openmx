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
//!
//! MD105b makes more of the pieces the port's ([`Pieces::Port`]): the world step is [`quad::frame`]
//! ([`WorldStep`]), its record held with the contacts the port's pass filed that frame
//! ([`atv_contact::Held::refill`]), and the record's part search, sweep and node points are answered
//! by the port's Desert Oasis ([`WorldStep::search`], [`WorldStep::sweep`], [`WorldStep::node_point`]),
//! each checked word for word against the original's call, which still runs for what it leaves in RAM.
//!
//! MD105a's tail ([`sim_tail::port_tail`]) with MD105b's pieces is the task's own chain: the record, its
//! outside, the world step and the tail all the port's, against the original's whole frame
//! (`the_quads_frame_is_the_ports_through_the_palm_strike_wreck_and_put_back`).

use super::*;
use game_sim::contact_search::Ground;
use game_sim::handling::ground;
use game_sim::handling::record::{self, Collides, Hit, Record, Struck, Swept};
use game_sim::handling::seed;
use game_sim::track;
use game_sim::world::Prop;
use game_sim::world_search::Collidable;
use game_sim::{atv_moto, atv_part, atv_sweep, atv_world};

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
/// its first contact's normal one bit off in each lane; with `world` too, only the port's step's held
/// copy of it). Answers the
/// update list's elements, and the terrain queries the port's record made.
fn frame_in_pieces(
    ee: &mut Ee,
    dt: F,
    port: Option<(u32, &track::Terrain<'_>)>,
    spoil: bool,
    mut world: Option<&mut WorldStep<'_>>,
    tail: bool,
) -> (Vec<u32>, u32) {
    assert_eq!(ee.u8_at(ee.u32_at(SKIP_FLAG) + 0x225) & 8, 0, "the frame is not skipped");
    let (mut elements, mut queries) = (Vec::new(), 0);
    let mut held = None;
    if let (Some(w), Some((mine, _))) = (world.as_deref_mut(), port) {
        w.list(ee, mine);
        w.spoil = spoil;
    }
    let spoil = spoil && world.is_none();
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
                    let (q, h) = port_record(ee, e, slot, dt, terrain.as_ref(), spoil, world.as_deref_mut());
                    queries += q;
                    held = held.or(h);
                }
                _ => {
                    run(ee, f, &[u64::from(e)], dt);
                }
            }
        }
    }
    match world.as_deref_mut() {
        Some(w) => w.step(ee, dt, held),
        None => {
            run(ee, WORLD_STEP, &[u64::from(SIM + 0x6c)], dt);
        }
    }
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

/// The quad's control-off word.
const CONTROL_OFF: u32 = 0xc80;

/// MD105b — **the world step the port's**: [`quad::frame`] with [`quad::Steps`] on the ride
/// [`image::read_ride`] reads off the machine before the step, its record held as the pass left it
/// ([`atv_contact::Held`]), the ground the port's terrain and the sweeps and gather Desert Oasis's
/// ([`atv_search::Oasis`], kept from frame to frame as `frame.rs`'s chain keeps it). The original's
/// step then runs on the machine (the tail after it reads its RAM), and every word
/// [`image::compare`] reads of the port's ride, every word of the rigid body and every word the
/// record's resolution writes ([`record::held_words`]) equals what it left.
struct WorldStep<'a> {
    oasis: atv_search::Oasis<'a>,
    terrain: atv_ground::Terrain<'a>,
    table: AcosTable,
    fresh: Ee,
    /// Frames stepped, and frames whose record held contacts that the port's step resolved.
    frames: u32,
    resolved: u32,
    /// The props' triangles and the plants' trunks' (built on first use), for the record's part search.
    facets: Vec<Vec<game_sim::ray_probe::Facet>>,
    trunks: Vec<Option<Vec<game_sim::ray_probe::Facet>>>,
    /// The record's probes the port's world does not build (the wreck part, the sweep), by their
    /// collidable in RAM: each one's collidable in [`atv_search::Oasis::world`].
    probes: BTreeMap<u32, usize>,
    /// Each of the port world's collidables in the list, the image's collidable it is (a mesh) or its
    /// list entry (a plant group); 0 for the rest.
    items: Vec<u32>,
    /// The quad's scene as the pass found it, and its node 0 in RAM.
    scene: Option<(atv_moto::Scene, u32)>,
    /// Spoil the next step's held contacts: the first one's normal one bit off in each lane.
    spoil: bool,
    /// How often the record's outside asked the port: part searches, the contacts they filed, sweeps,
    /// the sweeps' pairs, and node points.
    asked: [u32; 5],
}

impl WorldStep<'_> {
    /// The record's nearby list is the quad's, which the port's own step gathers ([`quad::frame`]'s
    /// collision sync, [`quad::Sweep::gather`]): as the frame starts, the port's list is the image's,
    /// entry for entry with its distances and its centre, and [`WorldStep::items`] maps its
    /// collidables to the image's.
    fn list(&mut self, ee: &Ee, record_at: u32) {
        let k = self.frames;
        assert_eq!(ee.u32_at(record_at + 0xfc), IMAGE_LIST, "the record's nearby list");
        if ee.u32_at(IMAGE_LIST + 4) != 0 {
            let centre: [u32; 4] = std::array::from_fn(|j| ee.u32_at(IMAGE_LIST + 0x10 + 4 * j as u32));
            assert_eq!(self.oasis.list.centre.map(|c| c.0.to_bits()), centre, "frame {k}: the list's centre");
        }
        let w = &self.oasis.world;
        self.items = vec![0; w.objects.len()];
        let (arr, n) = (ee.u32_at(IMAGE_LIST), ee.u32_at(IMAGE_LIST + 4));
        let found = &self.oasis.list.found;
        assert_eq!(found.len() as u32, n, "frame {k}: the port's list is {} long, the image's {n}", found.len());
        for (i, c) in found.iter().enumerate() {
            let (e, d) = (ee.u32_at(arr + 8 * i as u32), ee.u32_at(arr + 8 * i as u32 + 4));
            assert_eq!(c.distance.0.to_bits(), d, "frame {k}: entry {i}'s distance");
            let o = c.object;
            match (ee.u8_at(e + 2), w.objects[o].class) {
                (2, game_sim::world_search::Class::Object) if w.meshes.contains(&o) => {
                    let at = e - 0x18;
                    let place = w.broad.places[o].centre;
                    let port = [place[0], place[1], place[2], w.objects[o].radius].map(|f| f.0.to_bits());
                    let image = [0x50, 0x54, 0x58, 0x70].map(|x| ee.u32_at(at + x));
                    assert_eq!(port, image, "frame {k}: entry {i}'s mesh");
                    self.items[o] = at;
                }
                (3, game_sim::world_search::Class::Group) => {
                    let p = &w.broad.places[o];
                    let image = (ee.u16_at(e + 0xc), ee.u16_at(e + 0x10), ee.u8_at(e + 0x12), ee.u8_at(e + 0x15) as i8);
                    assert_eq!((p.cell[0], p.cell[1], p.model, p.blend), image, "frame {k}: entry {i}'s plant");
                    self.items[o] = e;
                }
                (kind, class) => panic!("frame {k}: entry {i} is of kind {kind} in the image and {class:?} in the port"),
            }
        }
    }

    /// The record's probe `at` (its collidable in RAM) as one of the port world's collidables: the
    /// riding part the world's own probe, the others added on first use; its reach, flags and filter
    /// read off RAM as the part's place left them.
    fn probe(&mut self, ee: &Ee, at: u32) -> usize {
        let objects = &mut self.oasis.world.objects;
        let i = if at == oasis::PROBE_AT {
            atv_world::PROBE
        } else {
            *self.probes.entry(at).or_insert_with(|| {
                objects.push(game_sim::world_search::Collidable::default());
                objects.len() - 1
            })
        };
        objects[i] = oasis::collidable(ee, at);
        i
    }

    /// The part search over Desert Oasis: the active part (`part`, stood where the place put it) over
    /// the port's list, its narrow phase [`atv_part::PartRays`] on the props' meshes and the plants'
    /// parts ([`OasisParts`]); the record's hits with each struck mesh the image's collidable.
    fn search(&mut self, ee: &Ee, part: u32) -> Vec<Hit> {
        assert_eq!(ee.u32_at(part + 0x188), 0, "the part has a ray filter");
        let probe = self.probe(ee, part);
        let root = ee.u16_at(part + 0x170);
        let nodes = tree_at(ee, ee.u32_at(part + 0x17c), root);
        let segs = ee.u32_at(part + 0x180);
        let segments: Vec<game_sim::ray_probe::Segment> = (0..ee.u32_at(part + 0x178))
            .map(|i| game_sim::ray_probe::Segment { from: vf_at(ee, segs + 0x20 * i), to: vf_at(ee, segs + 0x20 * i + 0x10) })
            .collect();
        let transform = [0, 1, 2, 3].map(|r| vf_at(ee, part + 0x130 + 0x10 * r));
        let rays = game_sim::ray_probe::RayProbe { transform, root, nodes: &nodes, segments: &segments };
        let record = part_record(ee, ee.u32_at(part + 0x7c));
        if self.facets.is_empty() {
            self.facets = self.oasis.props.iter().map(|p| game_sim::flight_search::facets(&p.mesh)).collect();
            self.trunks = vec![None; self.oasis.trees.props.props.len()];
        }
        let o = &mut self.oasis;
        let plants = atv_search::Plants::new(o.trees, o.eco, &o.world, o.first_part);
        let mut parts = OasisParts { props: o.props, first_mesh: o.world.meshes.start, facets: &self.facets, plants, trunks: &mut self.trunks, trunk: None, items: &self.items };
        let mut n = atv_part::PartRays { probe: rays, record, world: &mut parts, pairs: 0 };
        atv_search::search(&mut o.world.objects, probe, &o.list.found, &|_| false, &mut n);
        let objects = &o.world.objects;
        let by_id: BTreeMap<u32, usize> = self.items.iter().enumerate().filter(|&(_, &a)| a != 0).map(|(i, &a)| (a, i)).collect();
        let hits: Vec<Hit> = n
            .record
            .contacts
            .iter()
            .map(|c| {
                let struck = by_id.get(&c.object).map_or(Struck { object: c.object, ..Struck::default() }, |&i| Struck { object: c.object, category: objects[i].category, ..Struck::default() });
                Hit { from: c.from, to: c.to, normal: c.normal, point: c.point, struck }
            })
            .collect();
        self.asked[0] += 1;
        self.asked[1] += hits.len() as u32;
        hits
    }

    /// The sweep over Desert Oasis: the segment set on the sweep probe `probe` ([`atv_sweep::set`]), the
    /// probe searched over the port's list with [`OasisSweep`]. Answers the segment's bounds and the
    /// hit bit.
    fn sweep(&mut self, ee: &Ee, probe: u32, from: Vf, to: Vf) -> (atv_sweep::Bounds, bool) {
        let i = self.probe(ee, probe);
        let (segments, bounds) = atv_sweep::set(&[[from, to]], None);
        let o = &mut self.oasis;
        let probe = game_sim::sweep_probe::SweepProbe { segments, ..Default::default() };
        let mut n = OasisSweep { probe, props: o.props, first_mesh: o.world.meshes.start, places: &o.world.broad.places, pairs: 0 };
        let hit = atv_search::search(&mut o.world.objects, i, &o.list.found, &|_| false, &mut n);
        self.asked[2] += 1;
        self.asked[3] += n.pairs;
        (bounds, hit)
    }

    /// A point carried into the world by the quad's scene node, the port's scene's
    /// ([`atv_moto::Scene::tip`]).
    fn node_point(&mut self, node: u32, v: Vf) -> Vf {
        let (scene, at) = self.scene.as_mut().expect("the pass's scene");
        assert_eq!(node, *at, "the record's node is the quad's");
        self.asked[4] += 1;
        scene.tip(0, v)
    }

    fn step(&mut self, ee: &mut Ee, dt: F, held: Option<atv_contact::Held>) {
        let k = self.frames;
        let (mut r, mut at) = image::read_ride(ee, image::IMAGE_QUAD, dt).unwrap_or_else(|e| panic!("frame {k}: {e}"));
        // The control-off word is a mode (the wreck writes 8; other readers compare it with 0xd): the
        // world step tests it for not 0 and never writes it, and the port's quad keeps that.
        let control = ee.u32_at(image::IMAGE_QUAD + CONTROL_OFF);
        if control > 1 {
            at.gaps.push((image::IMAGE_QUAD + CONTROL_OFF, 0, format!("the control-off word holds {control:#x}: the port keeps whether it is set")));
        }
        let touching = held.as_ref().is_some_and(|h| !h.entries.is_empty());
        let mut held = held;
        if std::mem::take(&mut self.spoil) {
            let e = held.as_mut().and_then(|h| h.entries.first_mut()).expect("a held contact to spoil");
            e.contact.normal = Vf::from_bits(e.contact.normal.to_bits().map(|w| w ^ 1));
        }
        r.bike.contacts = held;
        let mut body = seed::body(&|a| ee.u32_at(a), at.body);
        let terrain = &self.terrain;
        let ground = |q: Vf, record: &mut u8| terrain.ground(q, Vf::ZERO, record);
        let mut m = quad::Steps { bike: r.bike.clone(), rest: Vec::<Call>::new(), ground, sweep: &mut self.oasis };
        quad::frame(&GAME_WORLD, &r.s.p, &mut r.s.q, &mut body, dt, &self.table, &mut m);
        let named: Vec<String> = m.rest.iter().map(|c| format!("{c:?}").split([' ', '(']).next().unwrap().to_string()).collect();
        let held = m.bike.contacts.is_some();
        assert!(named.iter().all(|n| n == "PartMove" || (n == "GravityRecord" && !held)), "frame {k}: handed on {:?}", m.rest);
        r.bike = m.bike;
        r.s.body = body.view();
        run(ee, WORLD_STEP, &[u64::from(SIM + 0x6c)], dt);

        let (bad, n) = image::compare(ee, &mut self.fresh, &r, &at);
        let show = bad
            .iter()
            .take(20)
            .map(|&(t, ia, p, w)| format!("  test {t:#010x} = image {ia:#010x} (quad +{:#x}): port {p:#010x}, original {w:#010x}", ia.wrapping_sub(image::IMAGE_QUAD)))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(bad.is_empty(), "frame {k}: {} of {n} words of the port's step differ from the original's:\n{show}", bad.len());
        let mut words = seed::body_words(&body, at.body);
        if let Some(h) = &r.bike.contacts {
            words.extend(record::held_words(h, at.record, &|a| ee.u32_at(a)));
        }
        let wrong: Vec<String> = words
            .into_iter()
            .filter(|&(a, v)| ee.u32_at(a) != v)
            .map(|(a, v)| format!("  {a:#010x} (body +{:#x}, record +{:#x}): port {v:#010x}, original {:#010x}", a.wrapping_sub(at.body), a.wrapping_sub(at.record), ee.u32_at(a)))
            .collect();
        assert!(wrong.is_empty(), "frame {k}: the port's step left the body or the record otherwise:\n{}", wrong.join("\n"));
        assert_eq!(r.s.q.control_off, ee.u32_at(image::IMAGE_QUAD + CONTROL_OFF) != 0, "frame {k}: the control-off word");
        self.resolved += u32::from(touching);
        self.frames += 1;
    }
}

/// The part search's world on Desert Oasis, as [`atv_search::Wheels`] pairs it: a prop's mesh (its id
/// the image's collidable, [`WorldStep::items`]) or a plant's part stood at the plant: a trunk's mesh,
/// or nothing a contact is made of (a sphere's ray test writes none). Nobody listens to the props or
/// the parts, and none has a callback.
struct OasisParts<'a> {
    props: &'a [Prop],
    first_mesh: usize,
    facets: &'a [Vec<game_sim::ray_probe::Facet>],
    plants: atv_search::Plants<'a>,
    trunks: &'a mut [Option<Vec<game_sim::ray_probe::Facet>>],
    trunk: Option<usize>,
    items: &'a [u32],
}

impl atv_part::Outside for OasisParts<'_> {
    fn shape(&mut self, other: usize) -> game_sim::ray_probe::Shape<'_> {
        use game_sim::ray_probe::{Facet, Mesh, Shape};
        fn mesh<'m>(id: u32, p: &'m Prop, facets: &'m [Facet]) -> Shape<'m> {
            Shape::Mesh(Mesh { id, transform: p.to_world.0, root: 0, nodes: &p.mesh.nodes, facets, vertices: &p.mesh.vertices })
        }
        if other < self.first_mesh + self.props.len() {
            let i = other - self.first_mesh;
            return mesh(self.items[other], &self.props[i], &self.facets[i]);
        }
        let Some(j) = self.trunk else { return Shape::Never };
        let p = &self.plants.trees.props.props[j];
        mesh(u32::MAX, p, self.trunks[j].get_or_insert_with(|| game_sim::flight_search::facets(&p.mesh)))
    }
}

impl game_sim::world_search::Narrow for OasisParts<'_> {
    fn clear(&mut self, _: &mut [Collidable], _: usize) {
        unreachable!("the part's own clear")
    }
    fn prepare(&mut self, _: &mut [Collidable], _: usize) {
        panic!("the part search prepared a grid")
    }
    fn pair(&mut self, _: &mut [Collidable], _: usize, _: usize) -> bool {
        unreachable!("the part's own pair")
    }
    fn parts(&mut self, _: &mut [Collidable], group: usize) -> usize {
        self.plants.trees.groups()[group - self.plants.first_group].parts.len()
    }
    fn part(&mut self, _: &mut [Collidable], group: usize, i: usize) -> Option<usize> {
        let g = group - self.plants.first_group;
        self.trunk = self.plants.trees.groups()[g].parts[i];
        Some(self.plants.slots[usize::from(self.plants.models[g])] + i)
    }
    fn heard_hit(&mut self, _: &mut [Collidable], _: usize, _: usize, _: usize) {
        unreachable!("nobody listens")
    }
    fn heard_struck(&mut self, _: &mut [Collidable], _: usize, _: usize, _: usize) {
        unreachable!("nobody listens")
    }
    fn on_hit(&mut self, _: &mut [Collidable], _: usize, _: usize, _: usize) {
        unreachable!("no callback")
    }
    fn on_struck(&mut self, _: &mut [Collidable], _: usize, _: usize, _: usize) {
        unreachable!("no callback")
    }
}

/// The sweep's narrow phase on Desert Oasis: [`atv_sweep::Sweep`]'s clear (the hit bit down, the
/// segments stood in the world, the reach the bundle's radius), and for the pair, which
/// [`atv_sweep`] does not port, MX Unleashed's ([`game_sim::sweep_probe::pair`]) on the prop's mesh,
/// taken as the ATV's: the palm ride checks its hit bit against the original's on every search.
struct OasisSweep<'a> {
    probe: game_sim::sweep_probe::SweepProbe,
    props: &'a [Prop],
    first_mesh: usize,
    places: &'a [game_sim::broad_phase::Placement],
    pairs: u32,
}

impl game_sim::world_search::Narrow for OasisSweep<'_> {
    fn clear(&mut self, objects: &mut [Collidable], probe: usize) {
        objects[probe].flags &= !game_sim::world_search::flags::HIT;
        game_sim::sweep_probe::clear(&mut self.probe);
        objects[probe].radius = self.probe.world.radius;
    }
    fn prepare(&mut self, _: &mut [Collidable], _: usize) {
        panic!("the sweep prepared a grid")
    }
    fn pair(&mut self, objects: &mut [Collidable], _: usize, other: usize) -> bool {
        use game_sim::sweep_probe::{Placed, Shape, Target};
        let i = other.checked_sub(self.first_mesh).filter(|&i| i < self.props.len()).unwrap_or_else(|| panic!("the sweep paired collidable {other}, not a prop"));
        let p = &self.props[i];
        let c = self.places[other].centre;
        let target = Target {
            nearby: objects[other].nearby,
            centre: Vf::new(c[0].0, c[1].0, c[2].0, c[3].0),
            radius: objects[other].radius,
            shape: Shape::Mesh(Placed { transform: p.to_world.0, mesh: game_sim::collide::MeshRef::of(&p.mesh), object: other }),
        };
        self.pairs += 1;
        game_sim::sweep_probe::pair(&mut self.probe, Some(&target))
    }
    fn parts(&mut self, _: &mut [Collidable], group: usize) -> usize {
        panic!("the sweep searched group {group}")
    }
    fn part(&mut self, _: &mut [Collidable], group: usize, _: usize) -> Option<usize> {
        panic!("the sweep searched group {group}")
    }
    fn heard_hit(&mut self, _: &mut [Collidable], _: usize, _: usize, _: usize) {
        unreachable!("nobody listens")
    }
    fn heard_struck(&mut self, _: &mut [Collidable], _: usize, _: usize, _: usize) {
        unreachable!("nobody listens")
    }
    fn on_hit(&mut self, _: &mut [Collidable], _: usize, _: usize, _: usize) {
        unreachable!("no callback")
    }
    fn on_struck(&mut self, _: &mut [Collidable], _: usize, _: usize, _: usize) {
        unreachable!("no callback")
    }
}

/// A ray probe's tree in RAM from `root` (36-byte nodes: the volume, two child words, the centre and
/// the half extent), as `tests/mxvatv_bike_ride.rs`' `tree_in_ram` reads it.
fn tree_at(ee: &Ee, nodes: u32, root: u16) -> Vec<track::Node> {
    let blank = track::Node { volume: F::ZERO, centre: [F::ZERO; 3], half_extent: [F::ZERO; 3], children: [0, 0] };
    let (mut out, mut todo) = (Vec::new(), vec![root]);
    while let Some(w) = todo.pop() {
        if w & 0x8000 != 0 {
            continue;
        }
        let n = nodes + 36 * u32::from(w);
        if out.len() <= usize::from(w) {
            out.resize(usize::from(w) + 1, blank);
        }
        let three = |a: u32| [0, 4, 8].map(|k| F(ee.f32_at(a + k)));
        let children = [ee.u16_at(n + 8), ee.u16_at(n + 10)];
        out[usize::from(w)] = track::Node { volume: F(ee.f32_at(n)), centre: three(n + 12), half_extent: three(n + 24), children };
        todo.extend(children);
    }
    out
}

/// The part's record (`+0x7c`) in RAM as its clear finds it (`tests/mxvatv_bike_ride.rs`'
/// `part_record_in_ram`): the nearest normal and point, the pair's first point, the struck part, the
/// flags, and the list's shape.
fn part_record(ee: &Ee, r: u32) -> game_sim::ray_probe::Record {
    game_sim::ray_probe::Record {
        normal: vf_at(ee, r + 0x20),
        point: vf_at(ee, r + 0x30),
        first_point: vf_at(ee, r + 0x70),
        part: ee.u32_at(r + 0x84),
        listed: ee.u32_at(r + 0x90) != 0,
        capacity: ee.u32_at(r + 0x88) as usize,
        one_per_ray: ee.u8_at(r + 0xbc) != 0,
        flags: ee.u8_at(r + 0xb8),
        nearest: F::ONE,
        ..game_sim::ray_probe::Record::default()
    }
}

/// A hit's words, for comparing two bit for bit.
fn hit_bits(h: &Hit) -> ([[u32; 4]; 4], Struck) {
    ([h.from, h.to, h.normal, h.point].map(|v| v.to_bits()), h.struck)
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
/// and the terrain query is the port's [`ground::under`]. With `port` (MD105b), the part search, the
/// sweep and the node's point are answered by the port's Desert Oasis ([`WorldStep::search`],
/// [`WorldStep::sweep`], [`WorldStep::node_point`]); the original's call still runs after each, for
/// what it leaves in RAM, and its answer must be the port's word for word.
struct RecordInRam<'a, 't, 'w> {
    ee: &'a mut Ee,
    at: u32,
    terrain: Option<&'t atv_ground::Terrain<'t>>,
    others: BTreeMap<u32, (Record, Record)>,
    queries: u32,
    port: Option<&'a mut WorldStep<'w>>,
}

impl RecordInRam<'_, '_, '_> {
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
    /// The original's part search, its record read back as the pass's hits.
    fn original_search(&mut self, part: u32, nearby: bool) -> Vec<Hit> {
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

impl Collides for RecordInRam<'_, '_, '_> {
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
        let node = self.word(0xe8);
        let port = self.port.as_mut().map(|w| w.node_point(node, v));
        let (out, p) = (self.ee.scratch(1), self.put(0, v));
        self.run(NODE_TIP, &[u64::from(out), u64::from(node), p]);
        let original = vf_at(self.ee, out);
        if let Some(port) = port {
            assert_eq!(port.to_bits(), original.to_bits(), "the node's point of {v:?}");
            return port;
        }
        original
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
        let port = self.port.as_mut().map(|w| {
            assert!(nearby, "a sweep over the whole world");
            w.sweep(self.ee, probe, from, to)
        });
        self.run(PROBE_SET, &[u64::from(probe), u64::from(self.at + 0xc0), 0]);
        self.run(PROBE_PLACE, &[u64::from(probe)]);
        self.search_with(probe, nearby);
        let hit = self.ee.u8_at(probe + 0x126) & 0x20 != 0;
        if let Some((b, port_hit)) = port {
            let image = [0x50, 0x54, 0x58, 0x70].map(|o| self.ee.u32_at(probe + o));
            let mine = [b.centre.x(), b.centre.y(), b.centre.z(), b.reach.0].map(f32::to_bits);
            assert_eq!(mine, image, "the sweep probe's centre and reach");
            assert_eq!(port_hit, hit, "the sweep's hit bit");
        }
        if !hit {
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
        let port = self.port.as_mut().map(|w| {
            assert!(nearby, "a part search over the whole world");
            w.search(self.ee, part)
        });
        let original = self.original_search(part, nearby);
        let Some(port) = port else { return original };
        let bits = |h: &[Hit]| h.iter().map(hit_bits).collect::<Vec<_>>();
        assert_eq!(bits(&port), bits(&original), "the part search's hits");
        port
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
/// body the quad's off `+0xe4`. Answers how many terrain queries it made, and after the pass the
/// record as its resolution holds it ([`record::held`], read before the pass with the list empty)
/// with the pass's contacts in its list ([`atv_contact::Held::refill`]); `None` if one is with another
/// record or a mover.
fn port_record(
    ee: &mut Ee,
    at: u32,
    slot: u32,
    dt: F,
    terrain: Option<&atv_ground::Terrain<'_>>,
    spoil: bool,
    mut port: Option<&mut WorldStep<'_>>,
) -> (u32, Option<atv_contact::Held>) {
    let mut rec = record::read(&|a| ee.u32_at(a), at);
    let mut held = (slot == 0x84).then(|| record::held(&|a| ee.u32_at(a), at)).flatten();
    let read = rec.clone();
    let body_at = ee.u32_at(at + 0xe4);
    let mut body = (body_at != 0).then(|| quad_body(ee, body_at));
    if let Some(p) = port.as_deref_mut() {
        let (r, image) = image::read_ride(ee, image::IMAGE_QUAD, dt).unwrap_or_else(|e| panic!("{e}"));
        p.scene = Some((r.bike.parts.scene, image.nodes[0]));
    }
    let mut w = RecordInRam { ee, at, terrain, others: BTreeMap::new(), queries: 0, port };
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
    if let Some(h) = held.as_mut()
        && !h.refill(&rec.contacts)
    {
        held = None;
    }
    (queries, held)
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
    /// MD105b: frames the port's world step ran, and how many of them resolved contacts.
    port_frames: u32,
    port_resolved: u32,
    /// [`WorldStep::asked`].
    port_asked: [u32; 5],
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
fn palm(frames: usize, port: Option<Pieces>, spoil: Option<usize>, tail: bool) -> Option<Palm> {
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
    let desert = if port == Some(Pieces::Port) { Some(oasis::desert_world(&pre)?) } else { None };
    let (searched, scene) = match desert {
        Some(oasis::Desert { world, props, eco, trees, rays, search, first_part, .. }) => (Some((world, rays, search, first_part)), Some((props, eco, trees))),
        None => (None, None),
    };
    let mut world = searched.zip(scene.as_ref()).map(|((world, rays, search, first_part), (props, eco, trees))| {
        let mut oasis = atv_search::Oasis::new(world, props, eco, trees, rays, search, first_part);
        assert_eq!(pre.u32_at(IMAGE_LIST + 4), 0, "the image's nearby list is empty");
        oasis.gathered = true;
        WorldStep {
            oasis,
            terrain: terrain.clone(),
            table: AcosTable::build(),
            fresh: image().unwrap(),
            frames: 0,
            resolved: 0,
            facets: Vec::new(),
            trunks: Vec::new(),
            probes: BTreeMap::new(),
            items: Vec::new(),
            scene: None,
            spoil: false,
            asked: [0; 5],
        }
    });

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
            let (elements, q) = frame_in_pieces(t, dt, (port != Some(Pieces::Original)).then_some((at.record, map)), spoil == Some(k), world.as_mut(), tail);
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
    if let Some(w) = &world {
        (seen.port_frames, seen.port_resolved, seen.port_asked) = (w.frames, w.resolved, w.asked);
    }
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

/// What the twin's frame in pieces runs as the port's: nothing (all the original's), the quad's
/// record's two virtuals (MD105), or those and the world step (MD105b).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Pieces {
    Original,
    Record,
    Port,
}

/// **The original's whole sim frame strikes the palm**: [`at_the_palm`]'s ride, frame after frame,
/// files the palm's contacts from frame 89, wrecks on frame 96 (the record's active part becomes the
/// wreck's and the quad's wreck count 1), puts the quad back at its spot on frame 164, and wrecks on
/// the palm a second time on frame 256. The frame in F31a's pieces (the update list's two virtuals,
/// the world step, the tail, the timer list), all the original's, leaves RAM as the whole frame does
/// every frame.
#[test]
fn the_quads_whole_sim_frame_strikes_the_palm_wrecks_and_is_put_back() {
    let Some(seen) = palm(PALM_RIDE, Some(Pieces::Original), None, false) else { return };
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
    let Some(seen) = palm(PALM_RIDE, Some(Pieces::Record), None, false) else { return };
    check_palm(&seen);
}

/// The comparison above goes red where the port's record is wrong: the first contact's normal one
/// bit off in each lane on the strike's first frame (89) shows in those four words of the record's
/// list, and in the quad the same frame's world step pushes with it.
#[test]
fn a_spoiled_contact_on_the_strike_goes_red() {
    let Some(seen) = palm(90, Some(Pieces::Record), Some(89), false) else { return };
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
    let Some(seen) = palm(PALM_RIDE, Some(Pieces::Record), None, true) else { return };
    check_palm(&seen);
}

/// MD105b — the port's step goes red when its held contacts are wrong: the first one's normal one bit
/// off in each lane on frame 89, in the port's [`atv_contact::Held`] alone (the record in RAM, and so
/// the original's step, keep the pass's), and the port's step differs from the original's there.
#[test]
fn a_spoiled_held_contact_turns_the_ports_step_red() {
    let ran = std::panic::catch_unwind(|| palm(90, Some(Pieces::Port), Some(89), false));
    match ran {
        Ok(None) => {}
        Ok(Some(_)) => panic!("the spoiled held contact went unseen"),
        Err(e) => {
            let msg = e.downcast_ref::<String>().cloned().unwrap_or_default();
            assert!(msg.starts_with("frame 89: ") && msg.contains("of the port's step differ"), "red for another reason: {msg}");
        }
    }
}

/// MD105b — **the palm strike's contacts push the port's quad**: the palm ride with the quad's
/// record's virtuals and the world step both the port's ([`WorldStep`]): the contacts
/// [`record::pass`] files each frame are handed to [`quad::Steps`]' [`atv_contact::Held`]
/// ([`atv_contact::Held::refill`]), and [`quad::frame`] resolves them. Every word the port's step
/// holds equals the original's step's, and every RAM word outside the stack the original's whole
/// frame's, every one of the 300 frames through the strike, wreck, put-back and second strike.
#[test]
fn the_palm_strikes_contacts_push_the_ports_quad() {
    let Some(seen) = palm(PALM_RIDE, Some(Pieces::Port), None, false) else { return };
    check_palm(&seen);
    assert_eq!(seen.port_frames as usize, PALM_RIDE, "the port's world step ran every frame");
    assert!(seen.port_resolved > 0, "the port's step never resolved a contact: {seen:?}");
    let [searches, hits, sweeps, pairs, points] = seen.port_asked;
    assert!(hits > 0 && pairs > 0 && points > 0, "the port's outside answered {:?}", seen.port_asked);
    assert_eq!((searches, sweeps), (PALM_RIDE as u32, PALM_RIDE as u32), "a part search and a sweep a frame");
}

/// MD105 — **the quad's frame is the port's through the strike, wreck and put-back**: the palm ride
/// with every piece the task names in Rust at once: the record's clear and pass ([`port_record`]), the
/// record's outside answered by Desert Oasis and the world step [`quad::frame`] holding the pass's
/// contacts ([`WorldStep`], MD105b), and the tail's walk, the quad's update, wreck, put-back and
/// animate ([`sim_tail::port_tail`], MD105a), against the original's whole frame: every RAM word
/// outside the stack and the ground ring equal after every one of the 300 frames, the port's step
/// resolving the palm's contacts.
#[test]
fn the_quads_frame_is_the_ports_through_the_palm_strike_wreck_and_put_back() {
    let Some(seen) = palm(PALM_RIDE, Some(Pieces::Port), None, true) else { return };
    check_palm(&seen);
    assert_eq!(seen.port_frames as usize, PALM_RIDE, "the port's world step ran every frame");
    assert!(seen.port_resolved > 0, "the port's step never resolved a contact: {seen:?}");
    let [searches, hits, sweeps, pairs, points] = seen.port_asked;
    assert!(hits > 0 && pairs > 0 && points > 0, "the port's outside answered {:?}", seen.port_asked);
    assert_eq!((searches, sweeps), (PALM_RIDE as u32, PALM_RIDE as u32), "a part search and a sweep a frame");
}
