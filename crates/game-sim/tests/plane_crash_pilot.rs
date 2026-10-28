//! S143: the biplane's crash and its pilot, against four crashes captured in the original
//! (`context/capture/s143/{nose,wing,belly,waggle}.csv`, research/journal/2026-10-26-s143-plane-crash-ragdoll).
//!
//! Each capture flies biplane.p2m2's state off free06's runway and into the ground — nose first, a
//! wing tip in a hard bank, the belly at speed, and the nose again with the stick moved through the
//! wreck — with the plane's rigid body, its pad and its wreck word every tick, and the pilot's 22
//! joint nodes in the plane's frame.
//!
//! What they show: the original never throws its pilot. The body stays in the seat, posed by the
//! plane's clips, the lean following the stick, through the wreck to the put-back; there is no pilot
//! ragdoll. What tumbles is the plane, and the port's wreck tumble is the original's bit for bit a
//! tick at a time; flown free from three ticks before the wreck, the port wrecks on the game's tick
//! and is put back after the game's 151.

use game_sim::archive::{Psr, Res};
use game_sim::biplane::{CRASHED, Pad};
use game_sim::flight::{self, Flight};
use game_sim::flight_search::{self, Rays};
use game_sim::phy::{self, Ini, Plane};
use game_sim::track::{self, Scene, Terrain};
use game_sim::world::{Ground, Place, Props, World};
use ps2_float::{F, Vf};
use ps2_oracle::{Kit, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// The crashes ridden out to the put-back. `waggle` (the nose crash with the stick moved through the
/// wreck) ends before its put-back.
const CRASHES: [&str; 3] = ["nose", "wing", "belly"];
/// How many ticks before the wreck tick a run is seeded from the capture.
const BACK: usize = 3;
/// `[Wrecks]`' `WreckTime`, 5 s, in 30 Hz ticks: the wreck clock passes it on the 151st.
const WRECK_TICKS: usize = 151;

/// One captured tick.
#[derive(Clone, Debug)]
struct Row {
    frame: u32,
    dt: F,
    wreck: u32,
    /// The wreck clock's bits.
    clock: u32,
    last_pos: Vf,
    pos: Vf,
    vel: Vf,
    last_quat: Vf,
    quat: Vf,
    rates: Vf,
    pad: Pad,
    throttle: F,
    bones: Vec<[f32; 3]>,
}

fn csv(name: &str) -> Option<Vec<Row>> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("capture").join("s143").join(format!("{name}.csv"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!("no {} — research/journal/2026-10-26-s143-plane-crash-ragdoll/capture records it", path.display()));
        return None;
    };
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let col = |n: &str| head.iter().position(|h| *h == n).unwrap_or_else(|| panic!("{name}.csv has no {n}"));
    let rows = lines
        .map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            let bits = |n: &str| f32::from_bits(u32::from_str_radix(c[col(n)], 16).expect("hex"));
            let dec = |n: &str| c[col(n)].parse::<f32>().expect("decimal");
            let v3 = |n: &str, w: f32| Vf::new(bits(&format!("{n}.x")), bits(&format!("{n}.y")), bits(&format!("{n}.z")), w);
            let v4 = |n: &str| Vf::new(bits(&format!("{n}.x")), bits(&format!("{n}.y")), bits(&format!("{n}.z")), bits(&format!("{n}.w")));
            Row {
                frame: c[col("frame")].parse().expect("frame"),
                dt: F(bits("dt")),
                wreck: c[col("wreck")].parse().expect("wreck"),
                clock: u32::from_str_radix(c[col("clock")], 16).expect("hex"),
                last_pos: v3("last_pos", 1.0),
                pos: v3("pos", 1.0),
                vel: v3("vel", 1.0),
                last_quat: v4("last_quat"),
                quat: v4("quat"),
                rates: v3("rates", 1.0),
                pad: Pad {
                    forward: F(dec("forward")),
                    back: F(dec("back")),
                    elevator: F(dec("elevator")),
                    aileron: F(dec("aileron")),
                    rudder: F(dec("rudder")),
                },
                throttle: F(dec("throttle")),
                bones: (0..22).map(|j| ["x", "y", "z"].map(|a| dec(&format!("bone{j}.{a}")))).collect(),
            }
        })
        .collect();
    Some(rows)
}

/// A member of `MX.PSR`, read whole.
fn psr_member(name: &str) -> Option<Vec<u8>> {
    let kit = Kit::load().ok()?;
    let path = kit.ctx("iso").join("MX.PSR");
    let Ok(mut f) = File::open(&path) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", path.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find(name)?;
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    Some(bytes)
}

fn entry(res: &Res, bytes: &[u8], suffix: &str) -> Option<Vec<u8>> {
    let want = suffix.to_ascii_lowercase();
    let e = res.entries.iter().find(|e| {
        let n = e.name.to_ascii_lowercase();
        n == want || n.ends_with(&format!("\\{want}"))
    })?;
    res.read(bytes, e).ok()
}

/// The disc's plane with its ray probe, standing where free06's `machine.txt` parks the first
/// player's (`[Biplane1]`) as the original's placement stands it — its body on the ground query's
/// answer there raised by the model's bounds ([`flight::bay_spot`]) — which is where a recovered
/// wreck is put back.
fn the_plane(acos: &game_sim::air::AcosTable) -> Option<Flight> {
    let bytes = psr_member("machine.res")?;
    let res = Res::parse(&bytes).expect("machine.res");
    let plane = Plane::read_bytes(&entry(&res, &bytes, phy::BIPLANE_PHY).expect("biplane.phy"));
    let rays = track::Rays::parse(&entry(&res, &bytes, flight_search::BIPLANE_RAYS).expect("biplanem.col")).expect("rays");
    let mut f = Flight::new(&plane).with_rays(Rays::of(&rays));
    let ini = Ini::parse_bytes(&entry(&res, &bytes, "free06\\machine.txt").expect("free06's machine.txt"));
    let bay = ini.section("Biplane1");
    let at = bay.vector("Position", Vf::default());
    let facing = bay.vector("Direction", Vf::new(0.0, 0.0, 1.0, 0.0));
    let lift = flight::spawn_lift(&entry(&res, &bytes, flight::BIPLANE_MODEL).expect("biplane.psg")).expect("biplane.psg's nodes");
    let (terrain, scene, _) = free06()?;
    let terrain = Terrain::parse(&terrain).expect("free06's terrain");
    let (ground, _) = Ground::new(&terrain, &scene.terrain).ground(Vf::new(at.x(), at.y(), at.z(), 1.0));
    f.place_body(flight::bay_spot(ground, lift), Vf::new(facing.x(), facing.y(), facing.z(), 0.0), acos);
    f.attitude.flown = true;
    Some(f)
}

/// free06's terrain bytes, its scene, and its props, as the shell reads them.
fn free06() -> Option<(Vec<u8>, Scene, Props)> {
    let bytes = psr_member("Tracks\\free06.res")?;
    let res = Res::parse(&bytes).ok()?;
    let scn = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(".scn"))?;
    let scene = Scene::parse(&res.read(&bytes, scn).ok()?).ok()?;
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain = entry(&res, &bytes, &leaf)?;
    let second = psr_member("Tracks\\free06_2.res");
    let mut parts = vec![(bytes.clone(), res)];
    if let Some(b) = second
        && let Ok(r) = Res::parse(&b)
    {
        parts.push((b, r));
    }
    let (props, missing) = Props::load(&scene, |want| {
        parts.iter().find_map(|(b, r)| entry(r, b, want)).ok_or_else(|| "no archive of this track has it".to_string())
    });
    assert!(missing.is_empty(), "free06's props the archives lack: {missing:?}");
    Some((terrain, scene, props))
}

/// The plane's flight put where `row` has it: its body, its throttle and the pad its controls read
/// next, then measured.
fn seed(f: &mut Flight, row: &Row, acos: &game_sim::air::AcosTable) {
    f.attitude.flown = true;
    let b = &mut f.body;
    (b.last_position, b.position, b.velocity) = (row.last_pos, row.pos, row.vel);
    (b.last_quat, b.quat, b.rates) = (row.last_quat, row.quat, row.rates);
    f.surfaces.throttle = row.throttle;
    f.pad = row.pad;
    f.remeasure(acos);
}

fn dist(a: Vf, b: Vf) -> f32 {
    let d = [a.x() - b.x(), a.y() - b.y(), a.z() - b.z()];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// How many game ticks lie between row `i - 1` and row `i`. Through the wreck the wreck clock says:
/// a row read before the game wrote its tick holds the last one's clock (none), and one read late has
/// run two (the frame-counter trigger's tears; only belly has any). Elsewhere one.
fn ticks(rows: &[Row], i: usize) -> usize {
    let (a, b) = (&rows[i - 1], &rows[i]);
    if a.wreck != CRASHED || b.wreck != CRASHED {
        return 1;
    }
    let run = f32::from_bits(b.clock) - f32::from_bits(a.clock);
    (run / b.dt.0).round() as usize
}

/// Everything a test flies: the plane at its bay, free06, the tables.
struct Flown {
    plane: Flight,
    terrain: Vec<u8>,
    scene: Scene,
    tables: game_sim::ride::Tables,
}

fn kit() -> Option<Flown> {
    let tables = game_sim::ride::Tables::build();
    let plane = the_plane(&tables.acos)?;
    let (terrain, scene, _) = free06()?;
    Some(Flown { plane, terrain, scene, tables })
}

/// Flies `rows` from [`BACK`] ticks before the wreck tick, as many ticks a row as the game ran
/// ([`ticks`]), `reseed` putting the port's body where the capture had it before every row's, and
/// hands each row's flight to `each` beside the row and how many ticks it ran.
fn fly(kit: &Flown, rows: &[Row], reseed: bool, mut each: impl FnMut(usize, &Flight, usize)) {
    let terrain = Terrain::parse(&kit.terrain).expect("free06's terrain");
    let (_, _, props) = free06().expect("free06");
    let mut place = Place { ground: Ground::new(&terrain, &kit.scene.terrain), props };
    let wreck = rows.iter().position(|r| r.wreck == CRASHED).expect("a wreck");
    let mut f = kit.plane.clone();
    seed(&mut f, &rows[wreck - BACK], &kit.tables.acos);
    for i in wreck - BACK + 1..rows.len() {
        if reseed {
            let p = &rows[i - 1];
            let b = &mut f.body;
            (b.last_position, b.position, b.velocity) = (p.last_pos, p.pos, p.vel);
            (b.last_quat, b.quat, b.rates) = (p.last_quat, p.quat, p.rates);
            f.remeasure(&kit.tables.acos);
        }
        let n = ticks(rows, i);
        for _ in 0..n {
            f.frame(rows[i].dt, rows[i].pad, &mut place, &kit.tables);
        }
        each(i, &f, n);
    }
}

/// The port's plane wrecks on the original's tick in each crash, seeded three ticks before it, with
/// the original's velocity off the ground within 0.5 ft/s and its position within 0.05 ft.
#[test]
fn each_crash_wrecks_on_the_originals_tick() {
    let Some(kit) = kit() else { return };
    for name in CRASHES {
        let Some(rows) = csv(name) else { return };
        let wreck = rows.iter().position(|r| r.wreck == CRASHED).expect("a wreck");
        let mut first = None;
        fly(&kit, &rows[..wreck + 2], false, |i, f, _| {
            if f.crash.reason != 0 && first.is_none() {
                first = Some(i);
                let (dv, dp) = (dist(f.body.velocity, rows[i].vel), dist(f.body.position, rows[i].pos));
                println!("{name}: wrecked on {} at {:.2} ft/s, {dv:.3} ft/s and {dp:.4} ft off the game", rows[i].frame, f.measure.airspeed.0);
                assert!(dv < 0.5, "{name}: the wreck tick's velocity {:?} is {dv} ft/s off the game's {:?}", f.body.velocity, rows[i].vel);
                assert!(dp < 0.05, "{name}: the wreck tick's position is {dp} ft off the game's");
            }
        });
        assert_eq!(first.map(|i| rows[i].frame), Some(rows[wreck].frame), "{name}: the port wrecks on another tick");
    }
}

/// The wreck's tumble, a tick at a time: the port's body put where the capture had it before each tick
/// of every wreck and stepped once lands where the game's did, bit for bit on all but a handful of
/// ticks and within 0.001 ft and 0.001 ft/s on those. Only rows one game tick after the last are
/// compared ([`ticks`]): from a row read late the port's body is put where the game was a tick before.
#[test]
fn the_wreck_tumbles_as_the_originals_a_tick_at_a_time() {
    let Some(kit) = kit() else { return };
    for name in CRASHES {
        let Some(rows) = csv(name) else { return };
        let (mut steps, mut exact, mut worst) = (0, 0, (0f32, 0f32));
        fly(&kit, &rows, true, |i, f, n| {
            if rows[i].wreck != CRASHED || rows[i - 1].wreck != CRASHED || n != 1 || (i >= 2 && ticks(&rows, i - 1) != 1) {
                return;
            }
            steps += 1;
            let b = &f.body;
            if (0..3).all(|k| b.position[k].to_bits() == rows[i].pos[k].to_bits() && b.velocity[k].to_bits() == rows[i].vel[k].to_bits()) {
                exact += 1;
            }
            let (dp, dv) = (dist(b.position, rows[i].pos), dist(b.velocity, rows[i].vel));
            worst = (worst.0.max(dp), worst.1.max(dv));
            assert!(dp < 0.001 && dv < 0.001, "{name} {}: the port's step is {dp} ft and {dv} ft/s off the game's", rows[i].frame);
        });
        println!("{name}: {exact} of {steps} wreck ticks bit-exact, the worst {:.6} ft {:.6} ft/s off", worst.0, worst.1);
        assert!(steps >= 120, "{name}: only {steps} wreck ticks to compare");
        assert!(exact * 10 >= steps * 9, "{name}: only {exact} of {steps} wreck ticks are bit-exact");
    }
}

/// The put-back: the game's wreck clock runs [`WRECK_TICKS`] ticks, its last wrecked row holding 150
/// of them, and the port's plane, flown on from three ticks before the wreck, is wrecked as many ticks
/// with the same clock on its last, then put back on the game's tick, at rest and facing as the game's.
///
/// And where (S164): on the spot the original's placement made of free06's `[Biplane1]`, the body's
/// centre of gravity on the ground there raised by the model's bounds, bit for bit.
#[test]
fn each_wreck_is_put_back_after_151_ticks() {
    let Some(kit) = kit() else { return };
    for name in CRASHES {
        let Some(rows) = csv(name) else { return };
        let wreck = rows.iter().position(|r| r.wreck == CRASHED).expect("a wreck");
        let back = wreck + rows[wreck..].iter().position(|r| r.wreck == 0).expect("a put-back");
        let game: usize = (wreck + 1..=back).map(|i| ticks(&rows, i)).sum();
        let (mut wrecked, mut last, mut put, mut next) = (0, F::ZERO, None, None);
        fly(&kit, &rows[..(back + 3).min(rows.len())], false, |i, f, n| {
            if i == back + 1 {
                next = Some(f.body.position);
            }
            if f.crash.reason != 0 {
                (wrecked, last) = (wrecked + n, f.crash.clock);
            } else if wrecked > 0 && put.is_none() {
                put = Some((i, f.body.clone()));
            }
        });
        let (at, body) = put.expect("the port puts the plane back");
        println!(
            "{name}: the game wrecked {} to {} ({} rows, {game} ticks), its last clock {:?}; the port {} ticks, its last clock {last:?}, back on {}",
            rows[wreck].frame, rows[back].frame, back - wreck, F(f32::from_bits(rows[back - 1].clock)), wrecked, rows[at].frame
        );
        assert_eq!(game, WRECK_TICKS, "{name}: the game's wreck is not {WRECK_TICKS} ticks");
        assert_eq!(last.to_bits(), rows[back - 1].clock, "{name}: the port's last wrecked clock is not the game's");
        assert_eq!(rows[at].frame, rows[back].frame, "{name}: the port is put back on another tick");
        // belly's put-back row was read a tick late, the plane already settling on its wheels.
        if ticks(&rows, back + 1) == 1 && rows[back].last_pos == rows[back].pos {
            assert_eq!(body.last_quat, rows[back].last_quat, "{name}: the put-back faces another way");
            assert_eq!((body.velocity, body.rates), (rows[back].vel, rows[back].rates), "{name}: the put-back is not at rest");
        }
        let (p, g) = (body.last_position, rows[back].last_pos);
        println!("{name}: put back at {:.5} {:.5} {:.5}, the game at {:.5} {:.5} {:.5}", p.x(), p.y(), p.z(), g.x(), g.y(), g.z());
        if rows[back].last_pos == rows[back].pos {
            assert!((0..3).all(|k| p[k].to_bits() == g[k].to_bits()), "{name}: put back at {p:?}, the game at {g:?}");
        } else {
            // The row was read a tick late: the game's plane has settled since its put-back.
            assert!(dist(p, g) < 0.01, "{name}: put back {} ft off the game's", dist(p, g));
        }
        if let Some(n) = next {
            // The first frame off the spot settles onto the wheels as the game's does: nose and belly
            // bit for bit, wing 0.0007 ft aside in x and z (not chased; its stick is held full over
            // through the put-back).
            let off = dist(n, rows[back + 1].pos);
            println!("{name}: a tick on at {n:?}, the game at {:?}, {off} ft off", rows[back + 1].pos);
            assert!(off < 0.001, "{name}: a tick after the put-back the port is {off} ft off the game");
        }
    }
}

/// The pilot through every captured wreck (`waggle`'s too): never thrown. Every joint stays within
/// 0.01 ft of where it rode before the wreck, measured from the body's root, while the stick is held
/// (nose, wing, belly), and within 0.5 ft while it is moved (`waggle`'s full stick, both ways: the
/// lean's reach, 0.32 ft as captured); and the
/// root within 0.01 ft of its seat in the plane's frame, on all but a few rows. Those few are torn
/// (the plane's model node and the pilot's nodes read a tick apart, so the whole body is off the seat
/// together, up to half a foot) and are left out. So the original has no
/// pilot ragdoll to port, and the port's pilot drawn in the seat (`openmx` `pilots`) is the original's.
#[test]
fn the_pilot_rides_every_wreck_out_in_the_seat() {
    for name in CRASHES.iter().chain(&["waggle"]) {
        let Some(rows) = csv(name) else { return };
        let wreck = rows.iter().position(|r| r.wreck == CRASHED).expect("a wreck");
        let seat = rows[wreck - 1].bones.clone();
        let less = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let len = |d: [f32; 3]| (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let (mut rows_read, mut torn, mut joint) = (0, 0, 0f32);
        for r in rows[wreck..].iter().take_while(|r| r.wreck == CRASHED) {
            rows_read += 1;
            if len(less(r.bones[0], seat[0])) >= 0.01 {
                torn += 1;
                continue;
            }
            for j in 1..seat.len() {
                joint = joint.max(len(less(less(r.bones[j], r.bones[0]), less(seat[j], seat[0]))));
            }
        }
        println!("{name}: {rows_read} wrecked rows, the root off its seat on {torn}, a joint at most {joint:.4} ft from its place");
        let reach = if *name == "waggle" { 0.5 } else { 0.01 };
        assert!(joint < reach, "{name}: a joint of the pilot moved {joint} ft");
        assert!(torn * 10 <= rows_read, "{name}: the root left the seat on {torn} of {rows_read} rows");
    }
}

/// The pilot's lean keeps following the stick through a wreck in the original (`waggle`: the stick
/// hard back, then hard forward, then centred, all while wrecked, and the pilot request's targets
/// with it), and the port's flight keeps storing the pad the lean reads
/// (`openmx::vehicle::PilotLean`) through the wreck: every wrecked tick's pad is the game's.
#[test]
fn the_pilots_lean_follows_the_stick_through_the_wreck() {
    let Some(kit) = kit() else { return };
    let Some(rows) = csv("waggle") else { return };
    let (mut moved, mut ticks) = (0, 0);
    fly(&kit, &rows, false, |i, f, _| {
        if f.crash.reason == 0 || i + 1 >= rows.len() {
            return;
        }
        ticks += 1;
        // The pad stored after the step is the one the game's +0x338.. hold on the next row (S94).
        let want = rows[i].pad;
        assert_eq!((f.pad.elevator, f.pad.aileron), (want.elevator, want.aileron), "{}: the port's stored pad", rows[i].frame);
        if rows[i].pad.elevator != rows[i - 1].pad.elevator {
            moved += 1;
        }
    });
    assert!(ticks > 100 && moved >= 2, "waggle: {ticks} wrecked ticks, the stick moved {moved} times");
}

