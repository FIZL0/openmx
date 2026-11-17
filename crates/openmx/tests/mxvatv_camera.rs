//! MD82b: MX vs. ATV Unleashed's follow camera ([`ModFollow`]) replayed against SLUS-21104's own, tick
//! by tick, behind each MD39a car and MD37a's ATV, and (MD82c) MD80a's biplane and Kiowa.
//!
//! The captures (`context/capture/md82b/<machine>.csv`, `research/md82b/play.sh`) play MD39a's drive
//! tape from each machine's Desert Oasis Free Ride state and read, every tick, the camera's target block
//! (cam +0x80: body point, forward, up, velocity, lean, airborne, speed), its distance, yaw and pitch,
//! the player's four offsets, its ground lift, its eight lags and the viewport's eye. MD82c's
//! (`context/capture/md82c/<flight>.csv`, `research/md82c/fly.sh`) read the same behind the biplane and
//! the Kiowa, both models, playing MD80a's flight tapes from its Desert Oasis states: the fliers follow
//! with the same camera and update, from `FC_BiplaneDefault.pb` and `FC_HeliDefault.pb`.
//!
//! The replay seeds the port's camera from the first clean row and from then on hands it nothing but
//! each row's target block (torn, stale and wrecked rows are handled in the loop, and counted). Its presets are the machine's camera file read as [`Preset::read_layout`] reads
//! it (MD82a), and its world is Desert Oasis's ground, [`atv_ground`] over `mc02.res`'s map (MD76) — the
//! terrain object the camera holds. Every row's look-at (the look lags), eye (the viewport's), distance,
//! yaw, pitch and lift must be the original's bit for bit.
//!
//! The recorder polls once a tick, at no fixed point in the frame, and under load it lands anywhere
//! (MD82, 2026-11-15): a row's camera can be read before or while this tick's camera step ran (each
//! field is then the port's from before the step or after it, bit for bit), and a row whose target
//! block is the last row's, or two ticks on, or a dropped tick, loses the target the camera stepped
//! on, so the replay reseeds after it. The counts are printed for every capture.
//!
//! R28's capture (`context/capture/r28/buggy.csv`) plays MD39a's drive with the right stick swinging
//! the camera's yaw off a half turn (`research/r28/swing.pad`): the row's player offsets are handed to
//! the camera before each step, as the stick leaves them.
//!
//! Each machine skips without its capture, its camera file or the map.

#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;
#[allow(dead_code)]
#[path = "../src/chase.rs"]
mod chase;

use std::collections::HashMap;
use std::path::PathBuf;

use chase::ModMachine;
use game_sim::archive::Res;
use game_sim::atv_ground::{self, Terrain};
use game_sim::camera::{Aspect, CurveTable, ModFollow, Preset, Race, Ridden, Surface, Switches, Target, Viewport};
use game_sim::phy::Ini;
use game_sim::track;
use ps2_float::{F, Vf};

/// Each capture, the folder under `context/capture` it is in, and the machine in it: MD82b's behind
/// the cars and the ATV, MD82c's (`research/md82c/fly.sh`) behind MD80a's four flights, and R28's
/// behind the buggy with the right stick swinging the camera round towards the front
/// (`research/r28/swing.pad`), where the rear and swing fades run below one.
const CAPTURES: [(&str, &str, ModMachine); 12] = [
    ("md82b", "buggy", ModMachine::Buggy),
    ("md82b", "baja", ModMachine::Baja),
    ("md82b", "sandrail", ModMachine::Sandrail),
    ("md82b", "golf", ModMachine::GolfCart),
    ("md82b", "truck", ModMachine::MonsterTruck),
    ("md82b", "atv-r-spawn", ModMachine::Atv),
    ("md82b", "atv-s-spawn", ModMachine::Atv),
    ("md82c", "biplane", ModMachine::Biplane),
    ("md82c", "biplane3", ModMachine::Biplane),
    ("md82c", "kiowa", ModMachine::Helicopter),
    ("md82c", "kiowa3", ModMachine::Helicopter),
    ("r28", "buggy", ModMachine::Buggy),
];

/// The frame time the camera is handed: the lags' alpha is this over their time constant in every row.
const DT: f32 = 0.03333;

fn context() -> PathBuf {
    match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context"),
    }
}

/// A capture's rows by column name; `None` without the file.
struct Capture {
    columns: HashMap<String, usize>,
    rows: Vec<Vec<String>>,
}

impl Capture {
    fn read(dir: &str, name: &str) -> Option<Capture> {
        let text = std::fs::read_to_string(context().join("capture").join(dir).join(format!("{name}.csv"))).ok()?;
        let mut lines = text.lines();
        let columns = lines.next()?.split(',').enumerate().map(|(i, c)| (c.to_string(), i)).collect();
        let rows = lines.map(|l| l.split(',').map(str::to_string).collect()).collect();
        Some(Capture { columns, rows })
    }

    fn num(&self, row: usize, col: &str) -> f32 {
        let i = *self.columns.get(col).unwrap_or_else(|| panic!("no column {col}"));
        self.rows[row][i].parse::<f64>().unwrap_or_else(|_| panic!("{col}: {:?}", self.rows[row][i])) as f32
    }

    fn word(&self, row: usize, col: &str) -> u32 {
        let i = self.columns[col];
        self.rows[row][i].parse().unwrap()
    }

    fn quad(&self, row: usize, col: &str) -> Vf {
        let l = |k: &str| self.num(row, &format!("{col}.{k}"));
        Vf::new(l("x"), l("y"), l("z"), l("w"))
    }

    /// A lag's three words: value, time constant, alpha.
    fn lag(&self, row: usize, col: &str) -> [F; 3] {
        let q = self.quad(row, col);
        [F(q.x()), F(q.y()), F(q.z())]
    }

    /// Whether row `row`'s target block is the last row's word for word under the next tick's vsync:
    /// the poll landed before the machine's frame ran (the camera may or may not have run), so this
    /// tick's target is lost and the next row is two steps on with no target block for the step
    /// between. A moving machine's block never repeats.
    fn doubled(&self, row: usize) -> bool {
        let target = ["a_point", "a_fwd", "a_up", "a_vel", "a_lean", "a_flag"];
        self.columns.iter().filter(|(k, _)| target.iter().any(|t| k.starts_with(&format!("{t}."))))
            .all(|(_, &i)| self.rows[row][i] == self.rows[row - 1][i])
            && self.num(row, "a_flag.y") >= 0.0001
    }

    /// Whether row `row`'s target block is two ticks on from the last row's: the poll read the next
    /// tick's machine, so the block the camera stepped on this tick is lost. Told by the body point
    /// moving about twice the distance the two rows' speeds cover in a tick (a step covers 0.8–1.2 of
    /// it in every capture).
    fn jumped(&self, row: usize) -> bool {
        let (p, q) = (self.quad(row, "a_point"), self.quad(row - 1, "a_point"));
        let moved = (0..3).map(|k| (p[k] - q[k]) * (p[k] - q[k])).sum::<f32>().sqrt();
        let speed = (self.num(row, "a_flag.y") + self.num(row - 1, "a_flag.y")) / 2.0;
        speed > 5.0 && moved > 1.6 * speed * DT
    }

    /// Whether the camera held a target block that tick (a wrecked ATV rider has none).
    fn has_actor(&self, row: usize) -> bool {
        !self.rows[row][self.columns["a_flag.x"]].is_empty()
    }

    fn rider(&self, row: usize) -> Ridden {
        let flag = self.quad(row, "a_flag");
        Ridden {
            target: Target {
                body_point: self.quad(row, "a_point"),
                forward: self.quad(row, "a_fwd"),
                up: self.quad(row, "a_up"),
                vel: self.quad(row, "a_vel"),
                lean: F(self.num(row, "a_lean.y")),
            },
            airborne: flag.x().to_bits() != 0,
            speed: F(flag.y()),
        }
    }
}

/// Desert Oasis's ground, as the camera's world answers it.
struct Ground<'a>(Terrain<'a>);

impl Surface for Ground<'_> {
    fn on_surface(&mut self, point: Vf) -> Vf {
        let mut p = point;
        atv_ground::query(&self.0, &mut p, None, None);
        p
    }
}

fn map_file() -> Option<Vec<u8>> {
    let file = std::fs::read(context().join("mxvatv-ps2/md37a/mc02.res")).ok()?;
    let archive = Res::parse(&file).unwrap();
    let entry = archive.find("Env\\mc02\\MC02.ps2.trn").expect("mc02.res holds MC02.ps2.trn");
    Some(archive.read(&file, entry).unwrap())
}

fn camera_file(name: &str) -> Option<Ini> {
    let base = name.rsplit('\\').next()?;
    let bytes = std::fs::read(context().join("mxvatv-ps2/md82/cam").join(base)).ok()?;
    Some(Ini::parse_bytes(&bytes))
}

/// The port's camera in the state row `row` holds.
fn seeded(c: &Capture, row: usize, presets: Vec<Preset>) -> ModFollow {
    let mut cam = ModFollow::new(presets);
    let f = &mut cam.race.cam;
    f.set_preset(c.word(row, "cam_preset") as usize);
    let dyp = c.quad(row, "cam_dyp");
    (f.distance, f.yaw, f.pitch) = (F(dyp.x()), F(dyp.y()), F(dyp.z()));
    let user = c.quad(row, "cam_user");
    (f.user_zoom, f.user_yaw, f.user_pitch, f.user_fov) = (F(user.x()), F(user.y()), F(user.z()), F(user.w()));
    f.snap = c.word(row, "cam_snap") != 0;
    f.ground_lift = F(c.num(row, "cam_lift"));
    f.pitch_down_locked = c.word(row, "cam_lock") != 0;
    let set = |lag: &mut game_sim::camera::Lag, w: [F; 3]| (lag.value, lag.tau, lag.alpha) = (w[0], w[1], w[2]);
    for k in 0..3 {
        set(&mut f.look_filter[k], c.lag(row, &format!("look{k}")));
        set(&mut f.pos_filter[k], c.lag(row, &format!("pos{k}")));
    }
    set(&mut f.fov_filter, c.lag(row, "fovlag"));
    cam
}

/// The player's four offsets (zoom, yaw, pitch, fov) as row `row` holds them: what the right stick
/// left in them before this tick's camera step, which the replay hands the camera with the target.
/// Every MD82b/MD82c row holds zeros; R28's tape swings the yaw.
fn stick(c: &Capture, row: usize, cam: &mut ModFollow) {
    let f = &mut cam.race.cam;
    let user = c.quad(row, "cam_user");
    (f.user_zoom, f.user_yaw, f.user_pitch, f.user_fov) = (F(user.x()), F(user.y()), F(user.z()), F(user.w()));
}

/// Every way row `row` and the port's camera after its frame differ.
fn differences(c: &Capture, row: usize, cam: &ModFollow, eye: Vf) -> Vec<String> {
    fields(c, row, cam, eye).into_iter().filter(|(_, want, got)| want.to_bits() != got.to_bits())
        .map(|(what, want, got)| format!("{what}: capture {want:?}, port {got:?}")).collect()
}

/// Whether each field is the port's after a tick's step or before it, bit for bit.
fn either(after: &[(String, f32, f32)], prior: &[(String, f32, f32)]) -> bool {
    after.iter().zip(prior).all(|((_, want, a), (_, _, b))| want.to_bits() == a.to_bits() || want.to_bits() == b.to_bits())
}

/// Each field the replay compares: its name, row `row`'s value and the port camera's.
fn fields(c: &Capture, row: usize, cam: &ModFollow, eye: Vf) -> Vec<(String, f32, f32)> {
    let f = &cam.race.cam;
    let mut bad = Vec::new();
    let mut check = |what: String, want: f32, got: F| bad.push((what, want, got.0));
    let dyp = c.quad(row, "cam_dyp");
    check("distance".into(), dyp.x(), f.distance);
    check("yaw".into(), dyp.y(), f.yaw);
    check("pitch".into(), dyp.z(), f.pitch);
    check("lift".into(), c.num(row, "cam_lift"), f.ground_lift);
    let vp = c.quad(row, "vp_eye");
    for (k, axis) in ["x", "y", "z"].into_iter().enumerate() {
        check(format!("look.{axis}"), c.lag(row, &format!("look{k}"))[0].0, f.look_filter[k].value);
        check(format!("look tau {axis}"), c.lag(row, &format!("look{k}"))[1].0, f.look_filter[k].tau);
        check(format!("pos lag.{axis}"), c.lag(row, &format!("pos{k}"))[0].0, f.pos_filter[k].value);
        check(format!("eye.{axis}"), vp[k], F(eye[k]));
    }
    check("fov".into(), c.lag(row, "fovlag")[0].0, f.fov_filter.value);
    bad
}

#[test]
fn each_mod_machines_eye_and_target_replay_slus_21104s() {
    let Some(trn) = map_file() else {
        eprintln!("skip: no context/mxvatv-ps2/md37a/mc02.res (Desert Oasis's map off MX vs. ATV's disc)");
        return;
    };
    let map = track::Terrain::parse(&trn).unwrap();
    let curve = CurveTable::build();
    let mut checked = 0;
    for (dir, name, machine) in CAPTURES {
        let Some(c) = Capture::read(dir, name) else {
            eprintln!("skip {name}: no context/capture/{dir}/{name}.csv");
            continue;
        };
        let file = chase::mod_presets(machine);
        let Some(ini) = camera_file(file) else {
            eprintln!("skip {name}: no {file} in context/mxvatv-ps2/md82/cam");
            continue;
        };
        let presets = Preset::read_layout(&ini, Aspect::Standard, Viewport::Full);
        assert!(c.rows.len() > 700, "{name}: {} rows", c.rows.len());
        // A row can be torn (the poll lands while the frame is still being written), the first one most
        // often, or doubled (the frame after it lost): seed from the first of twelve rows from `r` on that
        // steps into the next.
        let ground = || Ground(Terrain::of(&map));
        // The three rows after the seed may each be read late as any row can (below), but one of them
        // at least must hold this tick's lags: a seed one tick off never steps the lags onto a row's bits.
        let steps = |r: usize| {
            let mut cam = seeded(&c, r, presets.clone());
            let (mut eye, mut ground, mut on_time) = (c.quad(r, "vp_eye"), ground(), false);
            for row in r + 1..r + 4 {
                if c.word(row, "vsync") != c.word(row - 1, "vsync") + 2 || !c.has_actor(row) || c.doubled(row) || c.jumped(row) {
                    return false;
                }
                let (before, eye_before) = (ModFollow { race: Race { cam: cam.race.cam.clone() } }, eye);
                stick(&c, row, &mut cam);
                eye = cam.update(F(DT), &c.rider(row), F::ONE, &curve, Switches::default(), &mut ground).position;
                let after = fields(&c, row, &cam, eye);
                if !either(&after, &fields(&c, row, &before, eye_before)) {
                    return false;
                }
                on_time |= after.iter().filter(|(what, ..)| what.starts_with("look.") || what.starts_with("pos lag."))
                    .all(|(_, want, got)| want.to_bits() == got.to_bits());
            }
            on_time
        };
        let seed = |r: usize| (r..(r + 12).min(c.rows.len() - 4)).find(|&r| steps(r))
            .unwrap_or_else(|| panic!("{name}: none of rows {r}-{} steps into the next three", r + 11));
        let first = seed(0);
        let mut cam = seeded(&c, first, presets.clone());
        let mut eye = c.quad(first, "vp_eye");
        let mut ground = ground();
        let (mut replayed, mut airborne, mut still, mut wrecked, mut skipped) = (0, 0, 0, 0, 0);
        let (mut held, mut doubled) = (0, 0);
        let mut row = first + 1;
        while row < c.rows.len() {
            assert_eq!(c.word(row, "still") & 0x800, 0, "{name} row {row}: the still bit");
            if !c.has_actor(row) {
                // The rider is down (an ATV wreck): the camera holds no target block and the original
                // runs only the base camera's frame, so the follow update is not replayed. It picks the
                // respawned rider up again, from the first row after that steps cleanly.
                let end = (row..c.rows.len()).find(|&r| c.has_actor(r)).unwrap_or(c.rows.len());
                wrecked += end - row;
                if end + 16 > c.rows.len() {
                    break;
                }
                let at = seed(end);
                skipped += at - end + 1;
                cam = seeded(&c, at, presets.clone());
                eye = c.quad(at, "vp_eye");
                row = at + 1;
                continue;
            }
            assert_eq!(c.num(row, "vp_near"), 1.0, "{name} row {row}: the near clip");
            let gap = c.word(row, "vsync") != c.word(row - 1, "vsync") + 2;
            if gap || c.doubled(row) || c.jumped(row) {
                // The poll read the last frame's target again or the next one's, or the recorder fell
                // behind and skipped ticks: a tick has no target block, so the camera cannot be stepped through it. Reseed from the first row
                // from here on that steps cleanly.
                doubled += 1;
                if row + 16 > c.rows.len() {
                    break;
                }
                let at = seed(row);
                skipped += at - row + 1;
                cam = seeded(&c, at, presets.clone());
                eye = c.quad(at, "vp_eye");
                row = at + 1;
                continue;
            }
            let rider = c.rider(row);
            airborne += rider.airborne as usize;
            still += (rider.speed < 0.0001) as usize;
            let (before, eye_before) = (ModFollow { race: Race { cam: cam.race.cam.clone() } }, eye);
            stick(&c, row, &mut cam);
            eye = cam.update(F(DT), &rider, F::ONE, &curve, Switches::default(), &mut ground).position;
            let bad = differences(&c, row, &cam, eye);
            if bad.is_empty() {
                replayed += 1;
            } else {
                // The poll read this tick's target block but the camera, which steps after the machine,
                // before or while its frame ran: the lift is written first, the lags next and the
                // viewport's eye last, and a read can land between any two. Each field must then be the
                // port's after this tick's step or before it, bit for bit. Under load such late reads
                // come in runs, each row's camera a tick behind its target.
                let after = fields(&c, row, &cam, eye);
                let prior = fields(&c, row, &before, eye_before);
                let late = differences(&c, row, &before, eye_before);
                assert!(
                    either(&after, &prior),
                    "{name} row {row} (vsync {}), after this tick's step:\n  {}\nand before it:\n  {}",
                    c.word(row, "vsync"), bad.join("\n  "), late.join("\n  ")
                );
                held += 1;
            }
            row += 1;
        }
        eprintln!(
            "{name}: {file}, {replayed} rows replayed bit for bit from row {first} ({airborne} airborne, {still} standing; \
             {held} read late, {doubled} doubled or dropped, {wrecked} wrecked, {skipped} reseeded after)"
        );
        // Every row compared, on time or late, is bit for bit; the reseeds must leave most of the ride.
        assert!(
            (replayed + held) * 5 > (c.rows.len() - wrecked) * 4,
            "{name}: only {} of {} rows with a target compared", replayed + held, c.rows.len() - wrecked
        );
        checked += 1;
    }
    if checked == 0 {
        eprintln!("skip: no MD82b or MD82c capture with its camera file");
    }
}

