//! MD122 — the quad's loader ([`quad::Load`]) against C28's four spawn images: every word SLUS-21104's
//! loaders write into the player's quad, its body, record, world, engine and wheels is what the port
//! loads from the quad's own `.phy` (the race image `atv_race250p`, the sport image `atv_s_race250p`),
//! but for the words the ride has moved on since; and of the eight files only that one gives them.
//!
//! The images are `context/mxvatv-ps2/md37a/c28-atv-{r,s}-{spawn,air}.ee.bin` (C28's, memory: md37a
//! ATV states live in s2), the files `context/mxvatv-ps2/md37a/phy` (MD64b's). What the loader takes
//! from the model — the half box and the wheels' mounts — is taken from the image: the half box
//! worked back from the record's inertia ([`load_box`]), the mounts off the wheels.

use super::{Ini, Kit, QUAD_FILES, QUAD_VTABLE, QUAD_WHEELS, image, skip};
use game_sim::camera::CurveTable;
use game_sim::handling::mass;
use game_sim::quad::{self, Load, LoadAt};
use ps2_float::{F, Vf};
use ps2_oracle::Ee;

/// The player's quad in all four C28 images, and the file each image's quad was made from.
const QUAD: u32 = 0x0150_a990;
const IMAGES: [(&str, &str); 4] = [
    ("c28-atv-r-spawn", "atv_race250p"),
    ("c28-atv-r-air", "atv_race250p"),
    ("c28-atv-s-spawn", "atv_s_race250p"),
    ("c28-atv-s-air", "atv_s_race250p"),
];
/// The curve table the record's facing and the no-hander's roll read, and its step.
const CURVE_TABLE: u32 = 0x003e_ab20;
const CURVE_STEP: u32 = 0x003e_a9c8;
const CURVE_INV_STEP: u32 = 0x003e_a9d0;

/// The image's own sine and cosine table, which must be the one the port builds.
fn curve(ee: &Ee) -> CurveTable {
    let built = CurveTable::build();
    assert_eq!(ee.f32_at(CURVE_STEP).to_bits(), 0.0010473721f32.to_bits(), "the curve table's step");
    assert_eq!(ee.f32_at(CURVE_INV_STEP).to_bits(), 954.77045f32.to_bits(), "the curve table's inverse step");
    let at = ee.u32_at(CURVE_TABLE);
    for (i, e) in built.entries().iter().enumerate().take(game_sim::camera::CURVE_ENTRIES) {
        let got = [ee.f32_at(at + 8 * i as u32), ee.f32_at(at + 8 * i as u32 + 4)];
        assert_eq!(got.map(f32::to_bits), e.map(f32::to_bits), "curve table entry {i}");
    }
    built
}

fn get_v(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

/// Where the image's quad keeps what the loader wrote.
fn layout(ee: &Ee) -> LoadAt {
    let o = QUAD;
    let engine = ee.u32_at(o + 0x142c);
    LoadAt {
        quad: o,
        body: ee.u32_at(o + 0xb8),
        record: ee.u32_at(o + 0xc8),
        world: ee.u32_at(o + 0xe8),
        engine,
        wheels: std::array::from_fn(|k| ee.u32_at(o + QUAD_WHEELS + 4 * k as u32)),
        curve_rpm: ee.u32_at(engine + 0x64),
        curve_torque: ee.u32_at(engine + 0x68),
        ratios: ee.u32_at(engine + 0x70),
    }
}

/// The model box the part reader was handed at load, as the sides' halves: the one whose block
/// inertia ([`mass::block_inertia`], then [`mass::record_inertia`]) is the record's, bit for bit. The
/// record keeps the load's inertia (the bike loader puts its own over the body's only), and the box
/// call the reader makes walks the model's nodes' world rows, which the spawn has posed since — on
/// C28's images it gives neither the load's box nor one at the origin. So, as for the MX bike
/// (tests/mxvatv_bike_ride.rs' `load_box`), the squares are solved for from the record and the floats
/// around them walked until every lane is the record's. `None` when no box near gives them.
fn load_box(part: &mass::Part, want: [u32; 3]) -> Option<Vf> {
    let (m, (cm, _)) = (part.body.mass, part.record.mass?);
    let (s, k) = (part.body.inertia_scale, f64::from(cm.0) * f64::from(part.body.inv_mass.0));
    let m12 = f64::from(m.0) / 12.0;
    let w = want.map(|b| f64::from(f32::from_bits(b)) / k);
    let sums = [w[0] / (f64::from(s.x()) * m12), w[1] / (f64::from(s.y()) * m12), w[2] / (f64::from(s.z()) * m12)];
    // y² + z², x² + z², x² + y²: each square the half of the other two sums less its own.
    let sq = [(sums[1] + sums[2] - sums[0]) / 2.0, (sums[0] + sums[2] - sums[1]) / 2.0, (sums[0] + sums[1] - sums[2]) / 2.0];
    if sq.iter().any(|v| !(*v > 0.0)) {
        return None;
    }
    let guess = sq.map(|v| (v.sqrt() / 2.0) as f32);
    let near = |g: f32, d: i32| f32::from_bits((g.to_bits() as i32 + d) as u32);
    const R: i32 = 24;
    for dx in -R..=R {
        for dy in -R..=R {
            for dz in -R..=R {
                let half = Vf::new(near(guess[0], dx), near(guess[1], dy), near(guess[2], dz), 1.0);
                let (i, _) = mass::record_inertia(mass::block_inertia(m, s, half), cm, part.body.inv_mass);
                if [i.x(), i.y(), i.z()].map(f32::to_bits) == want {
                    return Some(half);
                }
            }
        }
    }
    None
}

/// Each wheel's mount and sweep shape, the wheel setup's.
fn mounts(ee: &Ee, at: &LoadAt) -> [(Vf, bool); 4] {
    at.wheels.map(|w| (get_v(ee, w + 0x90), ee.u32_at(w + 0x1dc) != 0))
}

/// The loader's words a ride has moved on by the time of a C28 image, and why.
fn moved(at: &LoadAt, a: u32) -> Option<&'static str> {
    for &w in &at.wheels {
        match a.wrapping_sub(w) {
            0x48 => return Some("the spring the ground step swaps between stopped and moving"),
            0x60 => return Some("the steering torque, the steer step's input it clears every tick"),
            0x100..0x110 => return Some("the rest point the suspension moves"),
            _ => {}
        }
    }
    match a.wrapping_sub(at.engine) {
        0x5c | 0x74 | 0x78 => Some("the gear the gearbox has shifted to"),
        0x84 | 0x88 => Some("the curve's cursor the revs walk"),
        0xa8 => Some("the peak torque's revs, which the spawn's engine reset zeroes"),
        _ => None,
    }
}

/// What an address is, for the failure message.
fn place(at: &LoadAt, a: u32) -> String {
    let mut bases = vec![("quad", at.quad), ("body", at.body), ("record", at.record), ("world", at.world), ("engine", at.engine)];
    bases.extend(at.wheels.iter().enumerate().map(|(k, &w)| (["LF", "RF", "LR", "RR"][k], w)));
    bases.extend([("rpm", at.curve_rpm), ("torque", at.curve_torque), ("ratios", at.ratios)]);
    let (what, b) = bases.into_iter().filter(|&(_, b)| b <= a).min_by_key(|&(_, b)| a - b).unwrap_or(("?", 0));
    format!("{what} +{:#x}", a - b)
}

/// Of `words`, the ones the image differs on, but for [`moved`]: `(address, image, port)`.
fn misses(ee: &Ee, at: &LoadAt, words: &[(u32, u32, u32)]) -> (Vec<(u32, u32, u32)>, usize) {
    let mut want = std::collections::BTreeMap::new();
    for &(a, mask, v) in words {
        let w = want.entry(a).or_insert(ee.u32_at(a));
        *w = *w & !mask | v & mask;
    }
    let mut skipped = 0;
    let mut out = vec![];
    for (&a, &v) in &want {
        if moved(at, a).is_some() {
            skipped += 1;
        } else if ee.u32_at(a) != v {
            out.push((a, ee.u32_at(a), v));
        }
    }
    (out, want.len() - skipped)
}

/// Every word the loaders write is the image's, from the quad's own file and from no other of the
/// eight; the loaded tuning over the image's own ride ([`Load::tune`], [`Load::tune_body`],
/// [`Load::tune_quad`], [`Load::params`]) leaves it as it was.
#[test]
fn the_c28_quads_words_are_their_phy_files() {
    let Ok(kit) = Kit::load().map_err(|w| skip(&w)) else { return };
    let dir = kit.ctx("mxvatv-ps2/md37a");
    let phy = dir.join("phy");
    if !phy.exists() {
        skip(&format!(
            "{} is not there: it is MX vs. ATV's Veh\\atv_*250*.phy out of MXVSATV.PSR's vehicletype_1.res \
             (research/journal/2026-11-02-md64b-atv-tire-phy)",
            phy.display()
        ));
        return;
    }
    let mut seen = 0;
    for (name, file) in IMAGES {
        let path = dir.join(format!("{name}.ee{}", ".bin"));
        if !path.exists() {
            skip(&format!("{} is not there (C28's; memory: md37a ATV states live in s2)", path.display()));
            continue;
        }
        let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(ee.u32_at(QUAD + 4), QUAD_VTABLE, "{name}: no quad at {QUAD:#x}");
        let curve = curve(&ee);
        let at = layout(&ee);
        let mounts = mounts(&ee, &at);
        let want = std::array::from_fn(|k| ee.u32_at(at.record + 0x60 + 4 * k as u32));

        let mut matched = vec![];
        let mut ours = None;
        for prefix in ["atv_", "atv_s_"] {
            for f in QUAD_FILES {
                let candidate = format!("{prefix}{f}");
                let bytes = std::fs::read(phy.join(format!("{candidate}.phy"))).unwrap_or_else(|e| panic!("{candidate}: {e}"));
                let ini = Ini::parse_bytes(&bytes);
                let probe = Load::read(&ini, Vf::new(1.0, 1.0, 1.0, 1.0), mounts, &curve);
                let solved = load_box(&probe.part, want);
                let load = Load::read(&ini, solved.unwrap_or(Vf::new(1.0, 1.0, 1.0, 1.0)), mounts, &curve);
                let (bad, n) = misses(&ee, &at, &load.words(&at));
                if candidate == file {
                    for (a, theirs, port) in bad.iter().take(40) {
                        println!(
                            "{name}: {a:#010x} ({}): the original's {theirs:#010x} ({}) ours {port:#010x} ({})",
                            place(&at, *a),
                            f32::from_bits(*theirs),
                            f32::from_bits(*port)
                        );
                    }
                    assert!(bad.is_empty(), "{name}: {} of {n} words differ from {file}'s", bad.len());
                    println!("{name}: {n} words, bit-exact, from {file} (half box {solved:?})");
                    ours = Some(load);
                }
                if bad.is_empty() {
                    matched.push(candidate);
                }
            }
        }
        assert_eq!(matched, [file], "{name}: the files whose words the image holds");

        let load = ours.expect("the quad's file");
        let (mut ride, _) = image::read_ride(&ee, QUAD, F(1.0 / 60.0)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let past = ride.bike.engine.gear_ratios;
        let (past_rpm, past_torque) = (ride.bike.engine.curve_rpm, ride.bike.engine.curve_torque);
        let kept = format!("{:?}", (&ride.s.p, &ride.s.q, &ride.bike));
        ride.s.p = load.params();
        load.tune_quad(&mut ride.s.q);
        load.tune(&mut ride.bike);
        // The ride has swapped each wheel's spring; the rest is the load's.
        for (k, w) in ride.s.q.wheels.iter_mut().enumerate() {
            let _ = k;
            w.spring = F(ee.f32_at(at.wheels[k] + 0x48));
        }
        let e = &mut ride.bike.engine;
        // The image reader reads eight ratios off a table of `gears`; what lies past it is the heap's.
        for k in (e.gears.max(0) as usize).min(past.len())..past.len() {
            e.gear_ratios[k] = past[k];
        }
        // And sixteen points off each curve table of `points`.
        for k in (e.curve.points as usize).min(past_rpm.len())..past_rpm.len() {
            (e.curve_rpm[k], e.curve_torque[k]) = (past_rpm[k], past_torque[k]);
        }
        (e.gear, e.ratio, e.inv_ratio) = (ee.u32_at(at.engine + 0x5c) as i32, F(ee.f32_at(at.engine + 0x74)), F(ee.f32_at(at.engine + 0x78)));
        e.curve = game_sim::r#move::Curve::at(e.curve.points, ee.u32_at(at.engine + 0x84), F(ee.f32_at(at.engine + 0x88)));
        let tuned = format!("{:?}", (&ride.s.p, &ride.s.q, &ride.bike));
        assert!(kept == tuned, "{name}: the loaded tuning changes the image's ride:\n{}", first_change(&kept, &tuned));

        let mut body = game_sim::handling::seed::body(&|a| ee.u32_at(a), at.body);
        let was = format!("{body:?}");
        load.tune_body(&mut body);
        assert_eq!(was, format!("{body:?}"), "{name}: the loaded body");
        seen += 1;
    }
    assert!(seen > 0 || IMAGES.is_empty());
}

/// Where two debug strings first part, with some of each around it.
fn first_change(a: &str, b: &str) -> String {
    let at = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
    let from = at.saturating_sub(200);
    format!("image: …{}…\nport:  …{}…", &a[from..(at + 200).min(a.len())], &b[from..(at + 200).min(b.len())])
}

/// The constructor's wheel, as [`quad::WheelTire::constructed`] has it, on the words the tire reader
/// never reaches: the rears' lock (the right rear's is the left rear's copy) and the fronts'
/// `OtherTireNormalLoadScale`.
#[test]
fn the_wheels_the_reader_leaves_are_the_constructors() {
    let Ok(kit) = Kit::load().map_err(|w| skip(&w)) else { return };
    let path = kit.ctx("mxvatv-ps2/md37a").join(format!("c28-atv-r-spawn.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!("{} is not there (C28's; memory: md37a ATV states live in s2)", path.display()));
        return;
    }
    let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    let at = layout(&ee);
    let built = quad::WheelTire::constructed(Vf::ZERO, false);
    for (k, &w) in at.wheels.iter().enumerate() {
        if k >= quad::LEFT_REAR {
            assert_eq!(ee.u32_at(w + 0x70), built.max_steer_angle.to_bits(), "wheel {k}'s lock");
            assert_eq!(ee.u32_at(w + 0x74), built.min_steer_angle.to_bits(), "wheel {k}'s lock the other way");
        } else {
            assert_eq!(ee.u32_at(w + 0xb8), built.other_load_scale.to_bits(), "wheel {k}'s other load scale");
        }
    }
}

/// The quad's spawn point and heading, the placement the load reads the model in (MD129).
fn spawn(ee: &Ee) -> (Vf, Vf) {
    (get_v(ee, QUAD + 0x50), get_v(ee, QUAD + 0x60))
}

/// The R quad's chassis model, or why it is not there.
fn chassis(kit: &Kit) -> Option<quad::Model> {
    let path = kit.ctx("mxvatv-ps2/md37a").join("250r_atv_chasis.psg");
    let Ok(bytes) = std::fs::read(&path) else {
        skip(&format!(
            "{} is not there: it is Veh\\Anim's 250r_atv_chasis.psg out of MXVSATV.PSR's vehicletype_1.res \
             (research/journal/2026-11-11-md129-atv-model-load)",
            path.display()
        ));
        return None;
    };
    Some(quad::Model::parse(&bytes).unwrap_or_else(|e| panic!("250r_atv_chasis.psg: {e}")))
}

/// The wheel setup's mounts off the chassis model at the quad's spawn are each C28 wheel's, bit for
/// bit, and none has a sweep shape.
#[test]
fn the_models_mounts_are_the_c28_wheels() {
    let Ok(kit) = Kit::load().map_err(|w| skip(&w)) else { return };
    let Some(model) = chassis(&kit) else { return };
    let dir = kit.ctx("mxvatv-ps2/md37a");
    let mut seen = 0;
    for (name, _) in IMAGES {
        let path = dir.join(format!("{name}.ee{}", ".bin"));
        if !path.exists() {
            skip(&format!("{} is not there (C28's; memory: md37a ATV states live in s2)", path.display()));
            continue;
        }
        let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
        let want = mounts(&ee, &layout(&ee));
        let (at, heading) = spawn(&ee);
        let got = model.mounts(&quad::spawn_frame(at, heading)).unwrap_or_else(|e| panic!("{e}"));
        for k in 0..4 {
            assert_eq!(got[k].0.0.map(f32::to_bits), want[k].0.0.map(f32::to_bits), "{name}: wheel {k}'s mount {:?} {:?}", got[k].0, want[k].0);
            assert_eq!(got[k].1, want[k].1, "{name}: wheel {k}'s sweep shape");
        }
        seen += 1;
    }
    assert!(seen > 0 || IMAGES.is_empty());
}

/// The part reader's box off the chassis model at the quad's spawn, against the record's inertia:
/// its `x` and `y` are the only box's near that gives the record, its `z` one float under it (not
/// 1:1: the frame the loader saw is not pinned down, research/journal/2026-11-11-md129-atv-model-load).
/// So [`Load::read_model`]'s words are the image's but for the four record words that `z` feeds.
#[test]
fn the_models_box_is_the_c28_records_but_one_float() {
    let Ok(kit) = Kit::load().map_err(|w| skip(&w)) else { return };
    let Some(model) = chassis(&kit) else { return };
    let dir = kit.ctx("mxvatv-ps2/md37a");
    let phy = dir.join("phy");
    if !phy.exists() {
        skip(&format!("{} is not there (research/journal/2026-11-02-md64b-atv-tire-phy)", phy.display()));
        return;
    }
    let mut seen = 0;
    for (name, file) in IMAGES {
        let path = dir.join(format!("{name}.ee{}", ".bin"));
        if !path.exists() {
            skip(&format!("{} is not there (C28's; memory: md37a ATV states live in s2)", path.display()));
            continue;
        }
        let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
        let at = layout(&ee);
        let (spot, heading) = spawn(&ee);
        let half = model.half_box(&quad::heading_frame(spot, heading));
        let bytes = std::fs::read(phy.join(format!("{file}.phy"))).unwrap_or_else(|e| panic!("{file}: {e}"));
        let load = Load::read_model(&Ini::parse_bytes(&bytes), &model, spot, heading, &curve(&ee)).unwrap_or_else(|e| panic!("{e}"));
        let want: [u32; 3] = std::array::from_fn(|k| ee.u32_at(at.record + 0x60 + 4 * k as u32));
        let solved = load_box(&load.part, want).expect("a box that gives the record");
        let up = Vf::new(half.x(), half.y(), f32::from_bits(half.z().to_bits() + 1), 1.0);
        assert_eq!(up.0[..3].iter().map(|v| v.to_bits()).collect::<Vec<_>>(), solved.0[..3].iter().map(|v| v.to_bits()).collect::<Vec<_>>(), "{name}: the model's box {half:?}, the record's {solved:?}");
        let (bad, n) = misses(&ee, &at, &load.words(&at));
        for (a, theirs, port) in &bad {
            println!("{name}: {a:#010x} ({}): the original's {theirs:#010x} ours {port:#010x}", place(&at, *a));
        }
        // The record's inertia `x`, `y` (each over the box's `z`) and their inverses.
        let off: Vec<u32> = bad.iter().map(|&(a, _, _)| a - at.record).collect();
        assert_eq!(off, [0x60, 0x64, 0x70, 0x74], "{name}: of {n} words, the ones that differ from {file}'s");
        seen += 1;
    }
    assert!(seen > 0 || IMAGES.is_empty());
}
