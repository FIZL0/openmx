//! MD65c — the quad's animation driver (vt `+0x3e4`), [`quad::animate`] against the original's.
//!
//! The original runs with every callee but the stunt table's lookup hooked: the biplane's own driver,
//! the animation stop, the clip play and blend, the node pose, the two frame reads, the hung nodes'
//! frame sets and the attachment's move are logged as [`Call`]s. Clips are compared as the handles
//! the quad's clip table (`+0x1534`) holds, so a table with repeated or empty handles (the image's)
//! compares as well as the generated one. What is compared besides the calls: the five words the
//! driver writes (the fade, the stunt it plays, the two pose blends and the put-back's settle word),
//! and that it wrote nothing else but the body's two self-copies and its stack.

use super::*;

const ANIMATE: u32 = 0x004b_e3c0;
/// The biplane's driver, through the thunk the quad's calls.
const BASE_ANIMATE: u32 = 0x004e_7bb0;
const ANIM_STOP: u32 = 0x0013_e650;
const PLAY_CLIP: u32 = 0x0013_eec0;
const BLEND_CLIPS: u32 = 0x0013_f5e0;
/// The four-argument blend, which no path reaches.
const BLEND_FOUR: u32 = 0x0014_0720;
const POSE_NODES: u32 = 0x004b_e8d0;
const ATTACHMENT_MOVED: u32 = 0x0026_6d30;

/// The generated cases' stand-ins: the animation controller (its vtable's `+0x88` is the mode no path
/// reaches), the stunt table (`+0x14f0`), each stunt's ready record, the clip handles and their info.
const CONTROLLER: u32 = 0x01d2_0000;
const CONTROLLER_VT: u32 = 0x01d2_0100;
const CONTROLLER_FN: u32 = 0x01cf_0900;
const STUNT_TABLE: u32 = 0x01d2_1000;
const READY_AT: u32 = 0x01d3_0000;
const CLIP_AT: u32 = 0x01d4_0000;
const INFO_AT: u32 = 0x01d5_0000;
/// The vehicle state object (`+0xbc4`, the state at `+0x28`).
const VSTATE: u32 = 0x01d2_0200;
const ANIM_BODY: u32 = 0x01d2_0400;

const STUNT_IDS: i32 = 46;
const CLIPS: usize = 49;

/// Where the driver's pointers lead, for the hooks' argument checks.
#[derive(Clone, Copy, Debug)]
struct At {
    quad: u32,
    controller: u32,
    attach: u32,
    frames: [u32; 2],
    hung_nodes: u32,
}

impl At {
    fn of(ee: &Ee, quad: u32) -> At {
        At {
            quad,
            controller: ee.u32_at(quad + 0x1428),
            attach: ee.u32_at(quad + 0x1480),
            frames: [ee.u32_at(quad + 0x1104), ee.u32_at(quad + 0x1108)],
            hung_nodes: ee.u32_at(quad + 0x1474),
        }
    }
}

/// A call as the log compares it: the port's [`Call`] with its clip places turned into the handles
/// the quad's clip table holds there, printed (so `-0` and `0` differ).
fn in_handles(ee: &Ee, quad: u32, c: &Call) -> String {
    let handle = |i: i32| ee.u32_at(quad.wrapping_add(0x1534).wrapping_add(4 * i as u32)) as i32;
    let c = match *c {
        Call::PlayClip { clip, weight, fade } => Call::PlayClip { clip: handle(clip), weight, fade },
        Call::BlendClips { clips, weight, mix, fade } => Call::BlendClips { clips: clips.map(handle), weight, mix, fade },
        c => c,
    };
    format!("{c:?}")
}

fn hook_animate(ee: &mut Ee, at: &Rc<Cell<At>>) -> Log<String> {
    let log: Log<String> = Rc::default();
    let zero = |ee: &Ee, regs: std::ops::Range<usize>, what: &str| {
        for r in regs {
            assert_eq!(ee.r(r), 0, "{what}: argument register {r}");
        }
    };
    let put = |log: &Log<String>, c: Call| log.borrow_mut().push(format!("{c:?}"));
    {
        let (l, at) = (log.clone(), at.clone());
        ee.hooks.insert(BASE_ANIMATE, Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, at.get().quad, "the base driver's quad");
            put(&l, Call::BaseAnimate(F(ee.f(12))));
        }));
    }
    {
        let (l, at) = (log.clone(), at.clone());
        ee.hooks.insert(ANIM_STOP, Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, at.get().controller, "the stop's controller");
            zero(ee, 5..9, "the stop");
            put(&l, Call::AnimStop);
        }));
    }
    {
        let (l, at) = (log.clone(), at.clone());
        ee.hooks.insert(PLAY_CLIP, Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, at.get().controller, "the play's controller");
            zero(ee, 6..12, "the play");
            put(&l, Call::PlayClip { clip: ee.r(5) as u32 as i32, weight: F(ee.f(12)), fade: F(ee.f(13)) });
        }));
    }
    {
        let (l, at) = (log.clone(), at.clone());
        ee.hooks.insert(BLEND_CLIPS, Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, at.get().controller, "the blend's controller");
            zero(ee, 7..12, "the blend");
            put(&l, Call::BlendClips {
                clips: [ee.r(5) as u32 as i32, ee.r(6) as u32 as i32],
                weight: F(ee.f(12)),
                mix: F(ee.f(13)),
                fade: F(ee.f(14)),
            });
        }));
    }
    ee.hooks.insert(BLEND_FOUR, Box::new(|_| panic!("the four-clip blend was reached")));
    ee.hooks.insert(CONTROLLER_FN, Box::new(|_| panic!("the controller's own update was reached")));
    {
        let (l, at) = (log.clone(), at.clone());
        ee.hooks.insert(POSE_NODES, Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, at.get().quad, "the node pose's quad");
            put(&l, Call::PoseNodes);
        }));
    }
    {
        let (l, at) = (log.clone(), at.clone());
        ee.hooks.insert(READ_FRAME, Box::new(move |ee| {
            let a = ee.r(4) as u32;
            let f = at.get().frames.iter().position(|&n| n == a).unwrap_or_else(|| panic!("a frame read from {a:#x}"));
            assert_eq!(ee.r(5), 0, "the frame read's second argument");
            let out = ee.r(6) as u32;
            for k in 0..16 {
                ee.set_u32(out + 4 * k, frame_word(f, k));
            }
            put(&l, Call::ReadFrame(f));
        }));
    }
    {
        let (l, at) = (log.clone(), at.clone());
        ee.hooks.insert(SET_FRAME, Box::new(move |ee| {
            let a = ee.r(4) as u32;
            let nodes = at.get().hung_nodes;
            let hung = (0..64).position(|i| ee.u32_at(nodes + 4 * i) == a).unwrap_or_else(|| panic!("a frame set on {a:#x}"));
            put(&l, Call::SetFrame { hung, frame: frame_of(ee, ee.r(5) as u32) });
        }));
    }
    {
        let (l, at) = (log.clone(), at.clone());
        ee.hooks.insert(ATTACHMENT_MOVED, Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, at.get().attach, "the attachment moved");
            put(&l, Call::AttachmentMoved);
        }));
    }
    log
}

/// What the port is handed, read off the quad at `quad`: the driver's words, the put-back's three, the
/// stick and the crossup.
fn read_anim(ee: &Ee, quad: u32) -> (quad::Anim, quad::PutBack, Quad) {
    let f = |o: u32| F(ee.f32_at(quad + o));
    let i = |o: u32| ee.u32_at(quad + o) as i32;
    let a = quad::Anim {
        vehicle_state: ee.u32_at(ee.u32_at(quad + 0xbc4) + 0x28),
        fade: f(0x14bc),
        fade_rate: f(0x14c0),
        stunt: i(0x1524),
        next_stunt: i(0x1528),
        balance_stunt: i(0x152c),
        restart: ee.u32_at(quad + 0x1650),
        stunt_time: f(0x14d8),
        crossup_range: f(0x14b0),
        crossup_blend: f(0x14b4),
        fore_aft_blend: f(0x14b8),
    };
    let n = ee.u32_at(quad + 0x1470) as i32;
    let kinds = ee.u32_at(quad + 0x1478);
    let pb = quad::PutBack {
        hung: (0..n.max(0) as u32).map(|k| ee.u32_at(kinds + 4 * k) as i32).collect(),
        settle: f(0x141c),
        request: ee.u32_at(quad + 0x31c),
        ..Default::default()
    };
    let q = Quad { lean: f(0x12e8), lean_fore_aft: f(0x12ec), crossup: f(0x330), ..Quad::default() };
    (a, pb, q)
}

/// The stunt table's answers, as the original reads them: the rider's set's entry for the stunt
/// (`+0x14f0`, 12 bytes each past `0x18`) points at a record whose byte `+0x59` says ready; the clip's
/// length is its handle's info's `+0x44`.
struct Table<'a>(&'a Ee, u32);

impl quad::StuntClips for Table<'_> {
    fn ready(&mut self, id: i32) -> bool {
        let Table(ee, quad) = *self;
        let entry = ee.u32_at(quad + 0x14f0).wrapping_add(0x18).wrapping_add(12 * id as u32);
        ee.u8_at(ee.u32_at(entry).wrapping_add(0x59)) != 0
    }
    fn length(&mut self, id: i32) -> F {
        let Table(ee, quad) = *self;
        let handle = ee.u32_at(quad.wrapping_add(0x1534).wrapping_add(4 * id as u32));
        F(ee.f32_at(ee.u32_at(handle.wrapping_add(4)).wrapping_add(0x44)))
    }
}

/// The words the driver writes, by bits: fade, stunt, crossup blend, fore-and-aft blend, settle.
fn written(ee: &Ee, quad: u32) -> [u32; 5] {
    [0x14bc, 0x1524, 0x14b4, 0x14b8, 0x141c].map(|o| ee.u32_at(quad + o))
}

fn port_written(a: &quad::Anim, pb: &quad::PutBack) -> [u32; 5] {
    [a.fade.0.to_bits(), a.stunt as u32, a.crossup_blend.0.to_bits(), a.fore_aft_blend.0.to_bits(), pb.settle.0.to_bits()]
}

/// Runs both on the quad at `quad` and compares the five words, the calls and the balance guard; the
/// original may write nothing but the five words, the body's two self-copies and its stack.
fn compare_animate(ee: &mut Ee, at: &Rc<Cell<At>>, log: &Log<String>, quad: u32, dt: F, label: &str) {
    at.set(At::of(ee, quad));
    let (mut a, mut pb, mut q) = read_anim(ee, quad);
    let start = (a, pb.clone(), q);
    log.borrow_mut().clear();
    ee.call(ANIMATE, &[u64::from(quad)], &[dt.0], 50_000);
    let body = ee.u32_at(quad + 0xb8);
    let frame = (ee.stack - 0x2000, ee.stack + 0x40);
    for (lo, hi) in ee.written_ranges() {
        for x in lo..hi {
            let word = [0x14bc, 0x1524, 0x14b4, 0x14b8, 0x141c].iter().any(|&o| (quad + o..quad + o + 4).contains(&x));
            let copies = [0x80, 0x190].iter().any(|&o| (body + o..body + o + 0x10).contains(&x));
            let stack = (frame.0..frame.1).contains(&x);
            assert!(word || copies || stack, "{label}: the original wrote {x:#010x} (in {lo:#010x}..{hi:#010x})");
        }
    }
    let mut calls = Vec::new();
    let mut b = Body::default();
    quad::animate(&mut a, &mut pb, &mut q, &mut b, dt, &mut Table(ee, quad), &mut calls);
    let port: Vec<String> = calls.iter().map(|c| in_handles(ee, quad, c)).collect();
    assert_eq!(
        written(ee, quad),
        port_written(&a, &pb),
        "{label}: fade, stunt, crossup blend, fore-aft blend, settle: the original's, then the port's\n{start:#?}"
    );
    assert_eq!(*log.borrow(), port, "{label}: the calls\n{start:#?}");
    assert_eq!(q.balancing, a.stunt == ee.u32_at(quad + 0x152c) as i32, "{label}: the balance guard");
}

/// The path a case takes, read off what it starts from.
fn animate_paths(ee: &Ee, quad: u32) -> Vec<&'static str> {
    let (a, pb, _) = read_anim(ee, quad);
    let mut p = vec![];
    let stopped = pb.request == 0 && (a.next_stunt != a.stunt || a.restart != 0);
    p.push(if stopped { "stopped" } else { "playing on" });
    let held = matches!(a.vehicle_state, 3 | 4);
    let id = a.next_stunt;
    if id < STUNT_IDS && quad::StuntClips::ready(&mut Table(ee, quad), id) {
        p.push(if stopped { "stunt clip, from its start" } else { "stunt clip" });
    } else if id == quad::RIDING_POSE {
        p.push(if held { "riding pose, held" } else if a.crossup_range == F::ZERO { "riding pose, no range" } else { "riding pose" });
    } else {
        p.push(if id < STUNT_IDS { "stunt not ready" } else { "another id" });
        if held {
            p.push("middle pose, held");
        }
    }
    p
}

impl Gen {
    fn stunt_id(&mut self) -> i32 {
        match self.bits() % 8 {
            0 | 1 | 2 => quad::RIDING_POSE,
            3 => 255,
            4 => 47 + (self.bits() % 14) as i32,
            _ => (self.bits() % STUNT_IDS as u32) as i32,
        }
    }
}

fn lay_animate(ee: &mut Ee, g: &mut Gen) -> F {
    for (base, len) in [(OBJ, 0x1800), (ANIM_BODY, 0x200)] {
        for o in (0..len).step_by(4) {
            ee.set_u32(base + o, 0);
        }
    }
    ee.set_u32(OBJ + 0x4, QUAD_VTABLE);
    ee.set_u32(OBJ + 0xb8, ANIM_BODY);
    put_v(ee, ANIM_BODY + 0x80, g.vec(-50.0, 50.0));
    put_v(ee, ANIM_BODY + 0x190, g.quaternion());
    ee.set_u32(OBJ + 0xbc4, VSTATE);
    ee.set_u32(VSTATE + 0x28, if g.one_in(3) { 3 + g.bits() % 2 } else { g.bits() % 7 });
    ee.set_u32(OBJ + 0x1428, CONTROLLER);
    ee.set_u32(CONTROLLER + 4, CONTROLLER_VT);
    ee.set_u32(CONTROLLER_VT + 0x88, CONTROLLER_FN);
    ee.set_u32(OBJ + 0x14f0, STUNT_TABLE);
    for id in 0..STUNT_IDS as u32 {
        let ready = READY_AT + 0x80 * id;
        ee.set_u32(STUNT_TABLE + 0x18 + 12 * id, ready);
        ee.write(ready + 0x59, 1, u128::from(!g.one_in(3)));
    }
    for i in 0..CLIPS as u32 {
        ee.set_u32(OBJ + 0x1534 + 4 * i, CLIP_AT + 0x20 * i);
        ee.set_u32(CLIP_AT + 0x20 * i + 4, INFO_AT + 0x80 * i);
        put_f(ee, INFO_AT + 0x80 * i + 0x44, if g.one_in(20) { F::ZERO } else { g.f(-0.5, 4.0) });
    }
    let next = g.stunt_id();
    let stunt = if g.b() { next } else { g.stunt_id() };
    ee.set_u32(OBJ + 0x1528, next as u32);
    ee.set_u32(OBJ + 0x1524, stunt as u32);
    ee.set_u32(OBJ + 0x152c, if g.b() { next as u32 } else { g.stunt_id() as u32 });
    ee.set_u32(OBJ + 0x1650, u32::from(g.one_in(4)));
    ee.set_u32(OBJ + 0x31c, if g.b() { 0 } else { 1 + g.bits() % 2 });
    let fade = match g.bits() % 6 {
        0 => F::ZERO,
        1 => F::ONE,
        _ => g.f(-0.5, 1.5),
    };
    put_f(ee, OBJ + 0x14bc, fade);
    put_f(ee, OBJ + 0x14c0, g.or_zero(0.0, 6.0));
    put_f(ee, OBJ + 0x14d8, g.f(-1.0, 5.0));
    let range = match g.bits() % 8 {
        0 => F::ZERO,
        1 => F(-0.0),
        _ => g.f(-2.0, 2.0),
    };
    put_f(ee, OBJ + 0x14b0, range);
    put_f(ee, OBJ + 0x330, g.f(-3.0, 3.0));
    put_f(ee, OBJ + 0x12ec, g.near(0.0, -1.2, 1.2));
    put_f(ee, OBJ + 0x12e8, g.f(-1.2, 1.2));
    put_f(ee, OBJ + 0x14b4, g.f(-0.5, 1.5));
    put_f(ee, OBJ + 0x14b8, g.near(0.5, -0.5, 1.5));
    put_f(ee, OBJ + 0x141c, g.f(-1.0, 1.0));
    ee.set_u32(OBJ + 0x1104, FRAME_NODE[0]);
    ee.set_u32(OBJ + 0x1108, FRAME_NODE[1]);
    ee.set_u32(OBJ + 0x1480, ATTACH);
    let n = if g.one_in(5) { 0 } else { g.bits() % 9 };
    ee.set_u32(OBJ + 0x1470, n);
    ee.set_u32(OBJ + 0x1474, HUNG_NODES);
    ee.set_u32(OBJ + 0x1478, HUNG_KINDS);
    for i in 0..n {
        ee.set_u32(HUNG_NODES + 4 * i, hung_node(i as usize));
        ee.set_u32(HUNG_KINDS + 4 * i, if g.one_in(4) { (g.bits() % 5).wrapping_sub(2) } else { g.bits() % 2 });
    }
    if g.one_in(4) { F(1.0 / 60.0) } else { g.f(0.0, 0.2) }
}

#[test]
fn the_quads_animation_driver_is_the_originals() {
    let Some(mut ee) = image() else { return };
    assert_eq!(ee.u32_at(QUAD_VTABLE + 0x3e4), ANIMATE, "the quad's animation slot");
    assert_eq!(ee.u32_at(QUAD_VTABLE + 0x554), POSE_NODES, "the quad's node-pose slot");
    let at = Rc::new(Cell::new(At::of(&ee, OBJ)));
    let log = hook_animate(&mut ee, &at);
    let mut g = Gen(0x0065_c001);
    let mut t = BTreeMap::new();
    for k in 0..20_000 {
        let dt = lay_animate(&mut ee, &mut g);
        for p in animate_paths(&ee, OBJ) {
            tally(&mut t, p);
        }
        compare_animate(&mut ee, &at, &log, OBJ, dt, &format!("case {k}"));
    }
    each_path_seen(
        &t,
        &[
            "stopped",
            "playing on",
            "stunt clip",
            "stunt clip, from its start",
            "stunt not ready",
            "another id",
            "riding pose",
            "riding pose, held",
            "riding pose, no range",
            "middle pose, held",
        ],
        200,
    );
}

/// The spawned quad's own words, clip table and stunt table, with each stunt id handed to it as the
/// stunt steps' next, a put-back pending or not, ridden and in a held state.
#[test]
fn the_spawned_quads_animation_driver_is_the_ports() {
    let Some(mut ee) = image() else { return };
    let quad = image::IMAGE_QUAD;
    let at = Rc::new(Cell::new(At::of(&ee, quad)));
    let log = hook_animate(&mut ee, &at);
    let state = ee.u32_at(quad + 0xbc4);
    let keep: Vec<(u32, u32)> = (0..0x1800).step_by(4).map(|o| (quad + o, ee.u32_at(quad + o))).collect();
    let kept_state = ee.u32_at(state + 0x28);
    let (spawned, _, _) = read_anim(&ee, quad);
    let mut t = BTreeMap::new();
    for id in (0..=60).chain([255]) {
        for request in [0, 1] {
            for vstate in [kept_state, 3] {
                for dt in [F(1.0 / 60.0), F(1.0 / 30.0)] {
                    for &(a, w) in &keep {
                        ee.set_u32(a, w);
                    }
                    ee.set_u32(state + 0x28, vstate);
                    ee.set_u32(quad + 0x1528, id as u32);
                    ee.set_u32(quad + 0x31c, request);
                    // The image's set has stunts ready that the clip table has no clip for: playing one
                    // reads the length through a null handle, which faults the original too.
                    let (a, pb, _) = read_anim(&ee, quad);
                    let stopped = pb.request == 0 && (a.next_stunt != a.stunt || a.restart != 0);
                    let clipless = ee.u32_at(quad + 0x1534 + 4 * id as u32) == 0;
                    if id < STUNT_IDS && quad::StuntClips::ready(&mut Table(&ee, quad), id) && clipless && !stopped {
                        tally(&mut t, "ready without a clip");
                        continue;
                    }
                    for p in animate_paths(&ee, quad) {
                        tally(&mut t, p);
                    }
                    compare_animate(&mut ee, &at, &log, quad, dt, &format!("stunt {id}, request {request}, state {vstate}, dt {dt}"));
                }
            }
        }
    }
    for &(a, w) in &keep {
        ee.set_u32(a, w);
    }
    ee.set_u32(state + 0x28, kept_state);
    eprintln!("the spawned quad: {spawned:?}, vehicle state {kept_state}; paths {t:?}");
    each_path_seen(&t, &["stopped", "playing on", "riding pose", "riding pose, held", "another id"], 1);
}
