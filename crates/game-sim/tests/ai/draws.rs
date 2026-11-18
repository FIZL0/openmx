// ---------------------------------------------------------------------------------------------------
// S328 — C33's draws that are not the AI's: the original's whole frame in the interpreter
// ---------------------------------------------------------------------------------------------------

use super::race4::{C33_NAMES, C33_RIDERS, C33_STATE, c33_exhaust_node, c33_front_node, c33_rear_node};
use super::sim::s12e_u64;
use super::*;
use ps2_oracle::{Ee, Kit};

/// `sceSifSetDma` and `sceSifDmaStat`, and the data cache's write-back over a range, which libpad's
/// per-frame command send calls (as tests/ragdoll_wrecks.rs's whole frame hooks them).
const SIF_SET_DMA: u32 = 0x003f_c3a0;
const SIF_DMA_STAT: u32 = 0x003f_c380;
const DCACHE_RANGE: u32 = 0x003f_c8d8;
/// The `lui $r, 0x1000` instructions in the frame that form Timer 0's register address.
const TIMER_READS: [u32; 2] = [0x0016_7dc0, 0x0013_21fc];
const TIMER_COUNT: u32 = 0;
const IOP_RPC_SEND_LEN: u32 = 344;

/// Run the hooked function itself from inside its hook, and come back to the hook's caller.
fn through(ee: &mut Ee) {
    let (pc, ra, sp, steps, stack) = (ee.pc, ee.r(31), ee.r(29), ee.steps, ee.stack);
    let args: Vec<u64> = (4..12).map(|k| ee.r(k)).collect();
    ee.stack = (sp as u32 - 0x800) & !0xf;
    ee.call(pc, &args, &[], 2_000_000_000);
    ee.set(31, ra);
    ee.set(29, sp);
    (ee.steps, ee.stack, ee.pc) = (steps, stack, pc);
}

/// Stub the interrupt mask, the SIF DMA and the cache write-back around a frame, and patch Timer 0's
/// register reads to RAM; the stubs are returned for [`unstub_hardware`].
fn stub_hardware(ee: &mut Ee) -> [u32; 6] {
    let stubs = [addr("interrupts_off").unwrap(), addr("interrupts_on").unwrap(), SIF_SET_DMA, SIF_DMA_STAT, DCACHE_RANGE, addr("sceSifCheckStatRpc").unwrap()];
    ee.hooks.insert(stubs[0], Box::new(|ee: &mut Ee| ee.set(2, 1)));
    ee.hooks.insert(stubs[1], Box::new(|_: &mut Ee| {}));
    ee.hooks.insert(SIF_SET_DMA, Box::new(|ee: &mut Ee| ee.set(2, 1)));
    ee.hooks.insert(SIF_DMA_STAT, Box::new(|ee: &mut Ee| ee.set(2, u64::MAX)));
    ee.hooks.insert(DCACHE_RANGE, Box::new(|_: &mut Ee| {}));
    let (check, send) = (addr("sceSifCheckStatRpc").unwrap(), addr("iop_rpc_send").unwrap());
    let send_end = send + IOP_RPC_SEND_LEN;
    ee.hooks.insert(
        check,
        Box::new(move |ee: &mut Ee| {
            let ra = ee.r(31) as u32;
            if (send..send_end).contains(&ra) { ee.set(2, 0) } else { through(ee) }
        }),
    );
    for at in TIMER_READS {
        let word = ee.u32_at(at);
        assert!(word & 0xffe0_ffff == 0x3c00_1000 || word & 0xffe0_ffff == 0x3c00_0000, "{at:#x} is not a `lui $r, 0x1000`: a different build");
        ee.set_u32(at, word & 0xffff_0000);
    }
    stubs
}

fn unstub_hardware(ee: &mut Ee, stubs: [u32; 6]) {
    for at in stubs {
        ee.hooks.remove(&at);
    }
}

/// One whole frame of the original, as the engine tick runs it after the pad read: the frame marker,
/// the counter's step, the simulation, the second marker. The render after it is [`render`].
fn whole_frame(ee: &mut Ee, dt: F, clock: &mut f64) {
    let dt = dt.0;
    let stubs = stub_hardware(ee);
    *clock += f64::from(dt) * 1000.0;
    let wall = (*clock / 100.0).floor();
    ee.set_u32(addr("wall_ticks").unwrap(), wall as u32);
    ee.set_u32(TIMER_COUNT, ((*clock - wall * 100.0) * 576.0) as u32);
    let app = ee.u32_at(addr("app_pointer").unwrap());
    let virt = |ee: &Ee, obj: u32, table: u32, slot: u32| ee.u32_at(ee.u32_at(obj + table) + slot);
    let dev = ee.u32_at(app + 0x104);
    ee.call(virt(ee, dev, 0x65c, 0x10), &[u64::from(dev), 1], &[dt], 4_000_000_000);
    let marker = virt(ee, app, 0x3c4, 0x80);
    ee.call(marker, &[u64::from(app), 1], &[], 4_000_000_000);
    ee.set_u32(app + 0x32c, 2);
    ee.set_f32(app + 0x344, dt);
    ee.set_f32(app + 0x348, ps2_float::fpu::div(1.0, dt));
    ee.set_u32(app + 0x374, ee.u32_at(app + 0x374) + 1);
    let root = ee.u32_at(app + 0x378);
    ee.call(virt(ee, root, 0x8, 0x38), &[u64::from(root)], &[dt], 4_000_000_000);
    assert_ne!(ee.v0(), 0, "the simulation asked to stop");
    ee.set_u32(app + 0x32c, 0);
    ee.call(marker, &[u64::from(app), 2], &[], 4_000_000_000);
    unstub_hardware(ee, stubs);
}

/// Where a draw on the shared generator came from, by its return address.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Site {
    /// The body's look timer re-armed (the rider's animation selection).
    Look,
    /// The engine note's sound pick and its accel choice.
    Note,
    /// A dust emitter placing a particle.
    Dust,
    /// The AI's own draws (ported in `crate::ai`).
    Ai,
    /// A tyre track laying a row (in the render pass).
    Track,
    Other,
}

/// The named functions and their lengths in bytes (from `./kit fn`).
const SITES: [(&str, u32, Site); 12] = [
    ("ai_feed", 704, Site::Ai),
    ("ai_habits", 2404, Site::Ai),
    ("rider_animation", 2068, Site::Look),
    ("sound_pick", 156, Site::Note),
    ("engine_note_accel", 1164, Site::Note),
    ("engine_note_power", 372, Site::Note),
    ("engine_note_sputter", 372, Site::Note),
    ("engine_note_decel", 336, Site::Note),
    ("engine_note_rev", 768, Site::Note),
    ("emit", 3424, Site::Dust),
    ("emit_other", 2748, Site::Dust),
    ("track_strip_add", 1776, Site::Track),
];

/// The original's human holds a neutral analog pad in C33; the image's buffers still hold an old
/// digital report, which the human's stick reads as (0, -1). Lay a fresh neutral report in both halves.
fn neutral_pad(ee: &mut Ee) {
    const PAD: u32 = 0x0047_a980;
    for half in [0u32, 0x80] {
        let report = [0x00u8, 0x73, 0xff, 0xff, 0x80, 0x80, 0x80, 0x80];
        for i in 0..20 {
            ee.write(PAD + half + i, 1, u128::from(report.get(i as usize).copied().unwrap_or(0)));
        }
        ee.set_u32(PAD + half + 0x58, ee.u32_at(PAD + half + 0x58) + 1);
    }
}

/// How many LCG steps take `from` to `to`, up to 64.
fn steps_between(mut from: u64, to: u64) -> Option<usize> {
    (0..=64).find(|_| {
        let hit = from == to;
        from = from.wrapping_mul(0x5851_f42d_4c95_7f2d).wrapping_add(1);
        hit
    })
}

/// The ticks the whole frame is run through.
const TICKS: usize = 60;

/// S328a: the bodies whose look-around clock re-arms in C33 before tick 15, by tick, in the order they
/// draw: the three AI bodies at 7 and the human's at 8. The bodies walk after the riders in the game's
/// player order (the human 0, then ai2, ai0, ai1 as their interfaces number them). The original's
/// (`s328_…`, off each body's own look word) and the Sim's (race4.rs's replay) are both held to it.
pub(crate) const C33_LOOKS: [(u64, &[&str]); 2] = [(7, &["ai2", "ai0", "ai1"]), (8, &["human"])];

/// What each body of [`C33_LOOKS`] re-arms to at `tick`: one unit draw each, in their order, off the
/// generator as the row before left it (`rng`). Empty on a tick no body re-arms.
pub(crate) fn c33_looks(tick: u64, rng: u64) -> Vec<(&'static str, F)> {
    let mut g = Rng::from_state(rng);
    let p = game_sim::rider_anim::Params::built();
    C33_LOOKS.iter().filter(|(t, _)| *t == tick).flat_map(|(_, names)| names.iter()).map(|&n| (n, p.rearm(g.unit()))).collect()
}

/// S328b: the engine notes C33 switches on before 16, by tick: the rider's update counts the delay its
/// loader drew (rider `+0x14a4`) down and switches the note (rider `+0x1494`) on, its `+0x29` bit 0.
/// The human's and ai1's run out later.
pub(crate) const C33_NOTES_ON: [(u64, &str); 2] = [(9, "ai2"), (14, "ai0")];

/// S328b: the notes' picks through 16: at 15 ai0 and ai2 open the throttle, and each picks the low
/// acceleration's sample 1 (its one draw) and draws its pitch (the other). The note's state is at
/// `+0x2d8`, its last low pick at `+0x2c4`.
pub(crate) const C33_NOTE_PICKS: (u64, [&str; 2], usize) = (15, ["ai0", "ai2"], 1);

/// S328c: the emitters that draw on the shared generator in C33, two draws each a tick: each run is
/// a rider, its entry in its manager's list, and the first and last ticks it draws (63 is the window's
/// end). 5 is the exhaust, 6 the generic rear-tyre dust, 8, 10 and 11 the rear sprays; the front
/// sprays (7, 9), the foot dust (4) and the human's (no throttle) never draw here.
pub(crate) const C33_DUST: [(&str, usize, u64, u64); 15] = [
    ("ai0", 5, 17, 23),
    ("ai0", 6, 18, 63),
    ("ai0", 8, 20, 51),
    ("ai0", 10, 20, 51),
    ("ai0", 11, 20, 51),
    ("ai1", 5, 21, 28),
    ("ai1", 6, 22, 63),
    ("ai1", 8, 24, 63),
    ("ai1", 10, 24, 63),
    ("ai1", 11, 24, 63),
    ("ai2", 5, 17, 23),
    ("ai2", 6, 18, 54),
    ("ai2", 8, 20, 54),
    ("ai2", 10, 20, 54),
    ("ai2", 11, 20, 54),
];

/// The emitters [`C33_DUST`] has drawing at `tick`, sorted.
pub(crate) fn c33_dust(tick: u64) -> Vec<(&'static str, usize)> {
    let mut v: Vec<(&'static str, usize)> = C33_DUST.iter().filter(|r| (r.2..=r.3).contains(&tick)).map(|r| (r.0, r.1)).collect();
    v.sort();
    v
}

/// S328d: the tyre tracks that lay a row in C33's render pass, one draw each (the row's height
/// jitter): each run is a rider, its entry in its manager's list (12 the front tyre's track, 13 the
/// rear's) and the first and last ticks it lays one. The manager's render walk updates its tracks in
/// rider order ai1, ai0, ai2, human; a track lays a row each 2 ft its tyre moves, so a slow tick
/// lays none. The human (no throttle) never lays one here.
pub(crate) const C33_TRACKS: [(&str, usize, u64, u64); 10] = [
    ("ai0", 12, 57, 60),
    ("ai0", 13, 17, 26),
    ("ai0", 13, 28, 35),
    ("ai0", 13, 37, 42),
    ("ai0", 13, 62, 63),
    ("ai1", 13, 21, 28),
    ("ai1", 13, 30, 63),
    ("ai2", 13, 17, 26),
    ("ai2", 13, 28, 38),
    ("ai2", 13, 40, 54),
];

/// The tracks [`C33_TRACKS`] has laying a row at `tick`, sorted.
pub(crate) fn c33_tracks(tick: u64) -> Vec<(&'static str, usize)> {
    let mut v: Vec<(&'static str, usize)> = C33_TRACKS.iter().filter(|r| (r.2..=r.3).contains(&tick)).map(|r| (r.0, r.1)).collect();
    v.sort();
    v
}

/// A body's animation object: its look-around clock (`+0xa30`), and the request block it reads
/// (`+0x488`) with the gate's cut flag at `+0x140`.
const BODY_LOOK: u32 = 0xa30;
const BODY_REQUEST: u32 = 0x488;
const REQUEST_CUT: u32 = 0x140;

/// S328: run C33's frames whole in the interpreter, with a neutral pad, and say which functions draw
/// on the shared generator. The four bodies stay bit for bit with the capture on every row, and the
/// generator through tick 16. The draws that are not the AI's are the body look timers (tick 7: the
/// three AI riders, tick 8: the human), the engine note (tick 15: two each for ai0 and ai2), and the
/// dust emitters from 17 (S328a: each re-armed look clock is the port's remap of its draw, the
/// bodies drawing in the game's player order). S328d: the render runs after each frame too, and its
/// draws are the tyre tracks' rows; with them the frame draws exactly what the capture steps on every
/// tick, and the generator agrees through the window's end.
#[test]
fn s328_c33s_other_draws_are_looks_notes_and_dust() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let state = kit.ctx(C33_STATE);
    let csv = kit.ctx("capture/c33/free09-ai4-agreed.csv");
    if !state.exists() || !csv.exists() {
        return skip("no C33");
    }
    let Some(at) = named(&["rand", "rand_state", "wall_ticks"]) else { return };
    let [rand, rand_state, wall_ticks] = at[..] else { unreachable!() };
    let Some(starts) = named(&SITES.map(|s| s.0)) else { return };
    let site = |ra: u32| SITES.iter().zip(&starts).find(|((_, len, _), a)| (**a..**a + len).contains(&ra)).map_or(Site::Other, |((_, _, k), _)| *k);
    let mut ee = Ee::from_ram(&state).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let text = std::fs::read_to_string(&csv).unwrap();
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap().split(',').collect();
    let col = |n: &str| head.iter().position(|h| *h == n).unwrap();
    let (fc, dtc, rc) = (col("frame_counter"), col("dt"), col("rng_state"));
    let raws = C33_NAMES.map(|n| col(&format!("{n}_body_raw")));
    let rows: Vec<(u64, F, u64, Vec<String>)> = lines
        .take(TICKS + 1)
        .map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            (c[fc].parse().unwrap(), F(c[dtc].parse::<f64>().unwrap() as f32), c[rc].parse().unwrap(), raws.iter().map(|&k| c[k].to_string()).collect())
        })
        .collect();
    assert_eq!(rows[0].2, s12e_u64(&ee, rand_state), "row 0 is the image");
    // Each draw's return address and the animation object (a0) of the selection it came from.
    let log: Rc<RefCell<Vec<u32>>> = Rc::default();
    let l = Rc::clone(&log);
    ee.hooks.insert(
        rand,
        Box::new(move |ee: &mut Ee| {
            let n = s12e_u64(ee, rand_state).wrapping_mul(0x5851_f42d_4c95_7f2d).wrapping_add(1);
            ee.write(rand_state, 8, u128::from(n));
            ee.set(2, (n >> 32) & 0x7fff_ffff);
            l.borrow_mut().push(ee.r(31) as u32);
        }),
    );
    let looked: Rc<RefCell<Vec<u32>>> = Rc::default();
    let (w, l) = (Rc::clone(&looked), Rc::clone(&log));
    ee.hooks.insert(
        starts[2],
        Box::new(move |ee: &mut Ee| {
            let (before, a0) = (l.borrow().len(), ee.r(4) as u32);
            through(ee);
            if l.borrow().len() > before {
                w.borrow_mut().push(a0);
            }
        }),
    );
    // S328c: each emitter's draws, by its rider and its entry in the rider's manager (`+0x1490`).
    let emitters: Vec<(u32, &str, usize)> = c33_emitters(&ee);
    let emitted: Rc<RefCell<Vec<(u32, usize)>>> = Rc::default();
    let (e, l) = (Rc::clone(&emitted), Rc::clone(&log));
    // S421: each rear tyre emitter's point, against the Sim's node chain on what the original holds
    // as it emits: the root's world, Bike's local, the rear mount and the snapshot's compression.
    let rears: Vec<(u32, u32, game_sim::dust::RearNode)> = C33_RIDERS.iter().map(|&(r, _, _)| {
        let (node, rear) = c33_rear_node(&ee, r);
        (r, node, rear)
    }).collect();
    let rear_points: Rc<RefCell<Vec<(u32, F, [u32; 16], u32)>>> = Rc::default();
    let rp = Rc::clone(&rear_points);
    // S446: and each front tyre and exhaust emitter's, against the Sim's chains on the nodes' locals as
    // the original holds them when it emits; the drive that sets those locals is proved per tick below.
    let others: Vec<(u32, u32, game_sim::dust::FrontNode, u32, game_sim::dust::ExhaustNode)> = C33_RIDERS
        .iter()
        .map(|&(r, _, _)| {
            let ((front_node, front), (exhaust_node, exhaust)) = (c33_front_node(&ee, r), c33_exhaust_node(&ee, r));
            (r, front_node, front, exhaust_node, exhaust)
        })
        .collect();
    let other_points: Rc<RefCell<Vec<(u32, bool)>>> = Rc::default();
    let op = Rc::clone(&other_points);
    ee.hooks.insert(
        addr("emit").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let (before, a0) = (l.borrow().len(), ee.r(4) as u32);
            let rear = rears.iter().find(|n| ee.u32_at(a0 + 0xa4) == n.1).map(|&(r, node, rear)| {
                let vf = |a: u32| Vf([0, 4, 8, 12].map(|o| ee.f32_at(a + o)));
                let mat = |a: u32| game_sim::math::Mat4([0, 0x10, 0x20, 0x30].map(|o| vf(a + o)));
                let (arm, c) = (ee.u32_at(node + 0x100), F(ee.f32_at(r + 0x87c)));
                let bike = ee.u32_at(arm + 0x100);
                let root = mat(ee.u32_at(bike + 0x100) + 0x80);
                let now = game_sim::dust::RearNode { bike: mat(bike + 0x40), ..rear };
                assert_eq!(now.swingarm(c).to_bits(), mat(arm + 0x40).to_bits(), "{r:#x}: SwingArm aimed at a compression of {c:?}");
                (r, now.point(&root, c), c, root.to_bits(), ee.u32_at(bike + 0x100))
            });
            let node = ee.u32_at(a0 + 0xa4);
            let other = others.iter().find(|o| node == o.1 || node == o.3).map(|&(r, front_node, front, _, exhaust)| {
                let vf = |a: u32| Vf([0, 4, 8, 12].map(|o| ee.f32_at(a + o)));
                let mat = |a: u32| game_sim::math::Mat4([0, 0x10, 0x20, 0x30].map(|o| vf(a + o)));
                let local = |n: u32| mat(n + 0x40);
                let parent = |n: u32| ee.u32_at(n + 0x100);
                if node == front_node {
                    let (shocks, bars) = (parent(node), parent(parent(node)));
                    let bike = parent(bars);
                    let now = game_sim::dust::FrontNode { bike: local(bike), bars: local(bars), shocks: local(shocks), node: local(node), ..front };
                    (r, true, now.point(&mat(parent(bike) + 0x80)))
                } else {
                    (r, false, game_sim::dust::ExhaustNode { pipe: local(parent(node)), node: local(node), ..exhaust }.point())
                }
            });
            through(ee);
            if let Some((r, front, want)) = other {
                let got = Vf([0, 4, 8, 12].map(|o| ee.f32_at(a0 + 0x40 + o)));
                assert_eq!(want.to_bits(), got.to_bits(), "{r:#x}: emitter {a0:#x}'s point (front {front})");
                op.borrow_mut().push((r, front));
            }
            e.borrow_mut().push((a0, l.borrow().len() - before));
            if let Some((r, want, c, root, root_node)) = rear {
                let got = Vf([0, 4, 8, 12].map(|o| ee.f32_at(a0 + 0x40 + o)));
                assert_eq!(want.to_bits(), got.to_bits(), "{r:#x}: emitter {a0:#x}'s point");
                rp.borrow_mut().push((r, c, root, root_node));
            }
        }),
    );
    // S446: the clip's Handlebars and Bike as the drive places Shocks, by rider, for the drive's proof.
    let posed: Rc<RefCell<Vec<(u32, game_sim::math::Mat4, game_sim::math::Mat4)>>> = Rc::default();
    let ps = Rc::clone(&posed);
    ee.hooks.insert(
        addr("transform_set_position").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let a0 = ee.r(4) as u32;
            if let Some(&(r, _, _)) = C33_RIDERS.iter().find(|&&(r, _, _)| ee.u32_at(r + 0x970) == a0) {
                let local = |n: u32| game_sim::math::Mat4([0, 0x10, 0x20, 0x30].map(|o| Vf([0, 4, 8, 12].map(|l| ee.f32_at(n + 0x40 + o + l)))));
                ps.borrow_mut().push((r, local(ee.u32_at(r + 0x968)), local(ee.u32_at(r + 0x964))));
            }
            through(ee);
        }),
    );
    // A body's animation object is held at its rider's +0xdb4.
    let anim = |ee: &Ee, k: usize| ee.u32_at(C33_RIDERS[k].0 + 0xdb4);
    // No body is at the gate: the Sim's riders are seeded without a start line (race4.rs).
    for k in 0..C33_RIDERS.len() {
        assert_eq!(ee.u32_at(ee.u32_at(anim(&ee, k) + BODY_REQUEST) + REQUEST_CUT), 0, "{}'s body is at the gate", C33_NAMES[k]);
    }
    let look_owner = |ee: &Ee, obj: u32| C33_RIDERS.iter().position(|&(r, _, _)| ee.u32_at(r + 0xdb4) == obj).map_or("?", |k| C33_NAMES[k]);
    let mut clock = f64::from(ee.u32_at(wall_ticks)) * 100.0;
    render_setup(&mut ee);
    // S328d: each tyre track's update, by its rider and its entry in the rider's manager.
    let entries: Vec<(u32, &str, usize)> = c33_entries(&ee);
    let laid: Rc<RefCell<Vec<(u32, usize)>>> = Rc::default();
    let (t, l) = (Rc::clone(&laid), Rc::clone(&log));
    ee.hooks.insert(
        addr("vehicle_fx_track_update").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let (before, a0) = (l.borrow().len(), ee.r(4) as u32);
            through(ee);
            t.borrow_mut().push((a0, l.borrow().len() - before));
        }),
    );
    let mut generator_through = rows[0].0;
    let mut last_rear: Vec<f32> = C33_RIDERS.iter().map(|&(r, _, _)| ee.f32_at(ee.u32_at(r + 0x7b4) + 0xf0)).collect();
    let curve = game_sim::camera::CurveTable::build();
    let (mut turned, mut steered, mut slid, mut bent) = (0, 0, 0, 0);
    let image_pose: Vec<[u32; 16]> = C33_RIDERS.iter().map(|&(r, _, _)| c33_front_node(&ee, r).1.pose.to_bits()).collect();
    for w in rows.windows(2) {
        let tick = w[1].0;
        // S446: the nodes the drive sets, as the frame before left them.
        let before: Vec<_> = C33_RIDERS.iter().map(|&(r, _, _)| (c33_front_node(&ee, r).1, c33_exhaust_node(&ee, r).1)).collect();
        neutral_pad(&mut ee);
        whole_frame(&mut ee, w[0].1, &mut clock);
        let sim_draws = log.borrow().len();
        render(&mut ee);
        for (k, &(_, body, _)) in C33_RIDERS.iter().enumerate() {
            let mine: String = (0..0x2c0).map(|i| format!("{:02x}", ee.read(body + i, 1) as u8)).collect();
            for (o, what) in [(0x60usize, "centre"), (0x1a0, "quat"), (0x260, "velocity"), (0x280, "spin")] {
                assert_eq!(mine[2 * o..2 * o + 32], w[1].3[k][2 * o..2 * o + 32], "tick {tick}: {}'s {what} parts", C33_NAMES[k]);
            }
        }
        let calls = std::mem::take(&mut *log.borrow_mut());
        let looks: Vec<&str> = std::mem::take(&mut *looked.borrow_mut()).iter().map(|&o| look_owner(&ee, o)).collect();
        let count = |s: Site| calls.iter().filter(|&&ra| site(ra) == s).count();
        let theirs = steps_between(w[0].2, w[1].2);
        println!(
            "tick {tick}: look {} {looks:?}, note {}, dust {}, ai {}, track {}, other {}; the capture steps {theirs:?}",
            count(Site::Look),
            count(Site::Note),
            count(Site::Dust),
            count(Site::Ai),
            count(Site::Track),
            count(Site::Other)
        );
        assert_eq!(count(Site::Other), 0, "tick {tick}: a draw from an unnamed site: {calls:x?}");
        let rearmed: Vec<&str> = c33_looks(tick, w[0].2).iter().map(|l| l.0).collect();
        let want: (usize, usize, usize, &[&str]) = match tick {
            7 | 8 => (0, 0, 0, &rearmed),
            15 => (4, 0, 0, &[]),
            ..17 => (0, 0, 0, &[]),
            _ => (count(Site::Note), count(Site::Dust), count(Site::Ai), &looks),
        };
        assert_eq!((count(Site::Note), count(Site::Dust), count(Site::Ai), &looks[..]), want, "tick {tick}");
        if tick >= 17 {
            assert!(count(Site::Dust) > 0, "tick {tick}: no dust");
        }
        // S421: every rear tyre emitter took its point from the node chain; the swingarm's compression is
        // the rear wheel's of the frame before, and the bike's root is not moved after the emitters run.
        let points = std::mem::take(&mut *rear_points.borrow_mut());
        for (k, &(r, _, _)) in C33_RIDERS.iter().enumerate() {
            let mine: Vec<_> = points.iter().filter(|p| p.0 == r).collect();
            assert_eq!(mine.len(), 4, "tick {tick}: {}'s rear emitters", C33_NAMES[k]);
            for p in mine {
                assert_eq!(p.1.0.to_bits(), last_rear[k].to_bits(), "tick {tick}: {}'s swingarm compression is the last frame's", C33_NAMES[k]);
                let after: [u32; 16] = core::array::from_fn(|i| ee.u32_at(p.3 + 0x80 + 4 * i as u32));
                assert_eq!(p.2, after, "tick {tick}: {}'s root moved after its emitters", C33_NAMES[k]);
                assert_eq!(p.3, ee.u32_at(ee.u32_at(r + 0x7a4) + 0x254), "{}'s root is the chassis' node", C33_NAMES[k]);
            }
            last_rear[k] = ee.f32_at(ee.u32_at(r + 0x7b4) + 0xf0);
        }
        // S446: every front tyre and exhaust emitter took its point from its chain; and the drive, run on
        // the frame before's nodes with this frame's snapshot, the root as the frame left it and Bike and
        // Handlebars as the clip posed them, sets Shocks, Handlebars and the pipe's root as the original does.
        let points = std::mem::take(&mut *other_points.borrow_mut());
        let poses = std::mem::take(&mut *posed.borrow_mut());
        for (k, &(r, _, _)) in C33_RIDERS.iter().enumerate() {
            let name = C33_NAMES[k];
            let n = |front: bool| points.iter().filter(|p| p.0 == r && p.1 == front).count();
            assert_eq!((n(true), n(false)), (2, 1), "tick {tick}: {name}'s front and exhaust emitters");
            let (now_front, now_exhaust) = (c33_front_node(&ee, r).1, c33_exhaust_node(&ee, r).1);
            let root = game_sim::math::Mat4([0, 0x10, 0x20, 0x30].map(|o| Vf([0, 4, 8, 12].map(|l| ee.f32_at(ee.u32_at(ee.u32_at(r + 0x7a4) + 0x254) + 0x80 + o + l)))));
            let feed = game_sim::dust::Feed {
                front_compression: F(ee.f32_at(r + 0x86c)),
                steer: F(ee.f32_at(r + 0x874)),
                grounded: ee.u32_at(r + 0x93c) != 0,
                ..Default::default()
            };
            let pose: Vec<_> = poses.iter().filter(|p| p.0 == r).collect();
            assert_eq!(pose.len(), 1, "tick {tick}: {name}'s Shocks placed once");
            assert_eq!(pose[0].2.to_bits(), now_front.bike.to_bits(), "tick {tick}: {name}'s Bike posed before Shocks is placed");
            let (mut front, mut exhaust) = before[k];
            (front.bike, front.pose, exhaust.bike) = (now_front.bike, pose[0].1, now_exhaust.bike);
            front.drive(&root, &feed, &curve);
            exhaust.drive(&root);
            assert_eq!(front.shocks.to_bits(), now_front.shocks.to_bits(), "tick {tick}: {name}'s Shocks slid by a front compression of {:?}", feed.front_compression);
            assert_eq!(front.bars.to_bits(), now_front.bars.to_bits(), "tick {tick}: {name}'s Handlebars at a steer of {:?}, grounded {}", feed.steer, feed.grounded);
            assert_eq!(exhaust.pipe.to_bits(), now_exhaust.pipe.to_bits(), "tick {tick}: {name}'s pipe took Bike's world");
            turned += usize::from(feed.grounded);
            steered += usize::from(feed.grounded && feed.steer != F(0.0));
            slid += usize::from(feed.front_compression != F(0.0));
            bent += usize::from(pose[0].1.to_bits() != image_pose[k]);
        }
        // S328c: the emitters that drew, two draws each, as C33_DUST says.
        let mut dust = Vec::new();
        for (em, n) in std::mem::take(&mut *emitted.borrow_mut()) {
            let &(_, name, entry) = emitters.iter().find(|e| e.0 == em).unwrap_or_else(|| panic!("tick {tick}: emitter {em:#x} is no rider's"));
            assert!(n == 0 || n == 2, "tick {tick}: {name}'s entry {entry} drew {n}");
            if n == 2 {
                dust.push((name, entry));
            }
        }
        dust.sort();
        assert_eq!(dust, c33_dust(tick), "tick {tick}: the emitters that drew");
        assert_eq!(2 * dust.len(), count(Site::Dust), "tick {tick}: the dust draws are the emitters'");
        // S328b: the notes switch on, and pick, as C33_NOTES_ON and C33_NOTE_PICKS say.
        if tick <= 16 {
            for (k, &(r, _, _)) in C33_RIDERS.iter().enumerate() {
                let (name, note) = (C33_NAMES[k], ee.u32_at(r + 0x1494));
                if let Some(&(on, _)) = C33_NOTES_ON.iter().find(|(_, n)| *n == name) {
                    assert_eq!(ee.read(note + 0x29, 1) & 1 != 0, tick >= on, "tick {tick}: {name}'s note switched on");
                }
                let (at, picked, index) = C33_NOTE_PICKS;
                let want = if tick >= at && picked.contains(&name) { (2, index as i32) } else { (1, -1) };
                assert_eq!((ee.i32_at(note + 0x2d8), ee.i32_at(note + 0x2c4)), want, "tick {tick}: {name}'s note and its last low pick");
            }
        }
        // S328a: each re-armed body's clock holds the port's remap of its own draw, in that order.
        for (name, look) in c33_looks(tick, w[0].2) {
            let k = C33_NAMES.iter().position(|n| *n == name).unwrap();
            assert_eq!(ee.u32_at(anim(&ee, k) + BODY_LOOK), look.0.to_bits(), "tick {tick}: {name}'s look-around clock");
        }
        if generator_through + 1 == tick && s12e_u64(&ee, rand_state) == w[1].2 {
            generator_through = tick;
        }
        // S328d: the render's draws are the tyre tracks' rows, one draw each, as C33_TRACKS says,
        // and nothing in the sim half lays a row.
        assert!(calls[..sim_draws].iter().all(|&ra| site(ra) != Site::Track), "tick {tick}: a track row in the sim half");
        assert!(calls[sim_draws..].iter().all(|&ra| site(ra) == Site::Track), "tick {tick}: a render draw that is not a track row: {:x?}", &calls[sim_draws..]);
        let mut rows_laid = Vec::new();
        for (en, n) in std::mem::take(&mut *laid.borrow_mut()) {
            let &(_, name, entry) = entries.iter().find(|e| e.0 == en).unwrap_or_else(|| panic!("tick {tick}: track {en:#x} is no rider's"));
            assert!(n <= 1, "tick {tick}: {name}'s entry {entry} drew {n}");
            if n == 1 {
                rows_laid.push((name, entry));
            }
        }
        rows_laid.sort();
        assert_eq!(rows_laid, c33_tracks(tick), "tick {tick}: the tracks that laid a row");
        assert_eq!(rows_laid.len(), count(Site::Track), "tick {tick}: the track draws are the rows'");
        // The sim half and the render together are the whole frame's draws: exactly the capture's.
        assert_eq!(theirs, Some(calls.len()), "tick {tick}: the capture's steps against the frame's draws");
    }
    assert_eq!(generator_through, rows[TICKS].0, "the generator agrees through");
    println!("S446: {turned} rider-ticks turned the bars ({steered} off straight), {slid} slid the fork; the clip's bars differ from the image's on {bent}");
    assert!(turned > 0 && slid > 0, "the drive's turn and slide were both exercised");
}


/// Every rider's emitters: the emitter, its rider and its entry in the rider's manager (`+0x1490`,
/// the entries' count at `+0x34` and their list at `+0x3c`; an entry's emitter at `+0x14`).
pub(crate) fn c33_emitters(ee: &Ee) -> Vec<(u32, &'static str, usize)> {
    c33_entries(ee).into_iter().map(|(entry, name, i)| (ee.u32_at(entry + 0x14), name, i)).collect()
}

/// Every rider's manager entries (`+0x1490`): the entry, its rider and its place in the list.
pub(crate) fn c33_entries(ee: &Ee) -> Vec<(u32, &'static str, usize)> {
    let mut out = Vec::new();
    for (k, &(r, _, _)) in C33_RIDERS.iter().enumerate() {
        let m = ee.u32_at(r + 0x1490);
        for i in 0..ee.u32_at(m + 0x34) {
            out.push((ee.u32_at(ee.u32_at(m + 0x3c) + 4 * i), C33_NAMES[k], i as usize));
        }
    }
    out
}

/// The render's waits on the hardware, stubbed to return 0.
const RENDER_WAITS: [&str; 3] = ["dma_channel_sync", "dma_vif1_drain", "vblank_wait"];

/// Make the render runnable in the interpreter: the hardware registers as storage (DMA over at
/// once), the kernel's calls returning 0, and the render's waits stubbed.
fn render_setup(ee: &mut Ee) {
    ee.io = Some(Default::default());
    for at in (0x0010_0000..0x0041_0000).step_by(4) {
        let (li, sys) = (ee.u32_at(at), ee.u32_at(at + 4));
        if li >> 16 == 0x2403 && sys == 0xc && !ee.hooks.contains_key(&at) {
            ee.hooks.insert(at, Box::new(|ee: &mut Ee| ee.set(2, 0)));
        }
    }
    for name in RENDER_WAITS {
        ee.hooks.insert(addr(name).unwrap(), Box::new(|ee: &mut Ee| ee.set(2, 0)));
    }
}

/// The frame's render (the application's vt+0x38 after the second marker).
fn render(ee: &mut Ee) {
    let stubs = stub_hardware(ee);
    let app = ee.u32_at(addr("app_pointer").unwrap());
    let at = ee.u32_at(ee.u32_at(app + 0x3c4) + 0x38);
    assert_eq!(at, addr("app_render").unwrap(), "the application's render is not where it was");
    ee.call(at, &[u64::from(app)], &[], 4_000_000_000);
    unstub_hardware(ee, stubs);
}

