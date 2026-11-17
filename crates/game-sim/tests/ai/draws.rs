// ---------------------------------------------------------------------------------------------------
// S328 — C33's draws that are not the AI's: the original's whole frame in the interpreter
// ---------------------------------------------------------------------------------------------------

use super::race4::{C33_NAMES, C33_RIDERS, C33_STATE};
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

/// One whole frame of the original, as the engine tick runs it after the pad read: the frame marker,
/// the counter's step, the simulation, the second marker. The render is not run: it needs VU0's clip
/// flags, which the interpreter lacks.
fn whole_frame(ee: &mut Ee, dt: F, clock: &mut f64) {
    let dt = dt.0;
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
    for at in stubs {
        ee.hooks.remove(&at);
    }
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
    Other,
}

/// The named functions and their lengths in bytes (from `./kit fn`).
const SITES: [(&str, u32, Site); 11] = [
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
/// bodies drawing in the game's player order). From 17 the capture steps further than the frame's sim half: the rest is in
/// the render, which is not run here.
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
    // A body's animation object is held at its rider's +0xdb4.
    let anim = |ee: &Ee, k: usize| ee.u32_at(C33_RIDERS[k].0 + 0xdb4);
    // No body is at the gate: the Sim's riders are seeded without a start line (race4.rs).
    for k in 0..C33_RIDERS.len() {
        assert_eq!(ee.u32_at(ee.u32_at(anim(&ee, k) + BODY_REQUEST) + REQUEST_CUT), 0, "{}'s body is at the gate", C33_NAMES[k]);
    }
    let look_owner = |ee: &Ee, obj: u32| C33_RIDERS.iter().position(|&(r, _, _)| ee.u32_at(r + 0xdb4) == obj).map_or("?", |k| C33_NAMES[k]);
    let mut clock = f64::from(ee.u32_at(wall_ticks)) * 100.0;
    let mut generator_through = rows[0].0;
    for w in rows.windows(2) {
        let tick = w[1].0;
        neutral_pad(&mut ee);
        whole_frame(&mut ee, w[0].1, &mut clock);
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
            "tick {tick}: look {} {looks:?}, note {}, dust {}, ai {}, other {}; the capture steps {theirs:?}",
            count(Site::Look),
            count(Site::Note),
            count(Site::Dust),
            count(Site::Ai),
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
        // The generator parts at 17 only because the render's draws are missing: the capture always
        // steps at least as far as this frame's sim half.
        if let Some(t) = theirs {
            assert!(t >= calls.len(), "tick {tick}: the capture steps {t}, the frame draws {}", calls.len());
        }
    }
    assert_eq!(generator_through, 16, "the generator agrees through");
}

