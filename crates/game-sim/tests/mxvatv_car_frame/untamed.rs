// --- MD161a: MD160's Untamed monster-truck drives, SLUS-21701's frame and the port's beside it -------

use super::*;
use game_sim::atv_car::{Input, InputKeys};
use game_sim::bindings::Command;

#[path = "../mxvatv_car_replay/drive.rs"]
#[allow(dead_code)]
mod drive;
use drive::Row;

/// SLUS-21701's sim frame (the sim object's vt +0x44; SLUS-21104's +0x40), its `$gp`, the game's
/// tick, the sim object in MD160's images and the physics world's step (research/oracle.txt `# MD161a`).
pub(crate) const U_FRAME: u32 = 0x0020_4ab0;
pub(crate) const U_GP: u64 = 0x003f_d070;
pub(crate) const U_DT: u32 = 0x3d08_850a;
pub(crate) const U_SIM: u32 = 0x006d_2440;
pub(crate) const U_WORLD_STEP: u32 = 0x004b_d040;
pub(crate) const U_BUDGET: u64 = 400_000_000;
/// The monster truck's class vtable.
pub(crate) const U_TRUCK_VTABLE: u32 = 0x003f_1840;
/// The car block the recorder keeps, and the port's readers read.
pub(crate) const U_CAR_SIZE: u32 = 0x1800;

/// Where SLUS-21701 keeps the car word SLUS-21104 keeps at `o` (research/oracle.txt `# MD81c`, `# MD81d`:
/// +0x30 low, −0x10 from the `+0xb..` block; MD81b's quad: the run from +0x680 +0x24). Between, as MD160's
/// spawn image matches SLUS-21104's truck image word for word (`# MD161a`): +0x28 from +0x440, +0x24 from
/// +0x49c, +0x20 from about +0x718 (+0x734 and +0x8d0 matched), −0x10 from about +0x9c0 (+0x9c4 on matched).
/// The edges inside the unmatched gaps are guesses; the readers read nothing in them.
pub(crate) fn u(o: u32) -> u32 {
    match o {
        ..0x440 => o + 0x30,
        ..0x49c => o + 0x28,
        ..0x718 => o + 0x24,
        ..0x9c0 => o + 0x20,
        _ => o - 0x10,
    }
}

/// A drive of MD160's: its capture and tape's name, the state it starts from and the player's car.
pub(crate) struct UDrive {
    pub(crate) name: &'static str,
    pub(crate) image: &'static str,
    pub(crate) car: u32,
    pub(crate) vtable: u32,
}

pub(crate) const U_THROTTLE: UDrive = UDrive { name: "throttle", image: "spawn", car: 0x01be_47b0, vtable: U_TRUCK_VTABLE };
pub(crate) const U_STEER: UDrive = UDrive { name: "steer", image: "spawn", car: 0x01be_47b0, vtable: U_TRUCK_VTABLE };
pub(crate) const U_JUMP: UDrive = UDrive { name: "jump", image: "spawn", car: 0x01be_47b0, vtable: U_TRUCK_VTABLE };
pub(crate) const U_WRECK: UDrive = UDrive { name: "wreck", image: "flipped", car: 0x01be_47b0, vtable: U_TRUCK_VTABLE };

/// A state's RAM image and scratchpad (MD160's, `context/untamed/md160`; the flipped state's
/// pulled out of its `.p2s`).
pub(crate) fn u_image(name: &str) -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let dir = kit.ctx("untamed/md160");
    let (ram, spr) = (dir.join(format!("{name}.ee{}", ".bin")), dir.join(format!("{name}.spr{}", ".bin")));
    if !ram.exists() || !spr.exists() {
        skip(&format!("no {} and its .spr.bin (MD160's images; a copy is in openmx-slots/kept-context/md160)", ram.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&ram).unwrap();
    ee.spr = std::fs::read(spr).unwrap();
    Some(ee)
}

pub(crate) fn u_rows(name: &str) -> Option<Vec<Row>> {
    drive::csv(Kit::load().ok()?.ctx("capture/md160").join(format!("{name}{}", ".csv")))
}

pub(crate) fn u_tape(name: &str) -> Option<Vec<Row>> {
    drive::csv(Kit::load().ok()?.ctx("recordings/md160").join(format!("{name}.p2m2.ticks{}", ".csv")))
}

/// The syscall stubs (`addiu v1, zero, n; syscall; jr ra`) answer `a0`, GetThreadId 1 (F31's); the
/// sim frame reaches nothing else the interpreter lacks but the 64-bit multiply.
pub(crate) fn u_hooked(mut ee: Ee) -> Ee {
    for a in (0x0032_0000..0x0034_0000u32).step_by(4) {
        if ee.u32_at(a) >> 16 == 0x2403 && ee.u32_at(a + 4) == 0x0000_000c && ee.u32_at(a + 8) == 0x03e0_0008 {
            let n = ee.u32_at(a) & 0xffff;
            ee.hooks.insert(a, Box::new(move |ee: &mut Ee| {
                let v = if n == 0x2f { 1 } else { ee.r(4) };
                ee.set(2, v);
            }));
        }
    }
    // The compiler's 64-bit multiply (its `madd` is an MMI op the interpreter lacks): the low 64 bits.
    ee.hooks.insert(0x0034_a3e0, Box::new(|ee: &mut Ee| {
        let v = ee.r(4).wrapping_mul(ee.r(5));
        ee.set(2, v);
    }));
    ee
}

/// The stick word SLUS-21701's input step leaves for each stick the tapes hold, as MD160's captures
/// read it (steer.csv row 166 for the half stick; the rows after hold its square, the controls'
/// `SquareSteeringInput` written back).
fn stick_of(x: f32) -> F {
    match x {
        0.0 | 1.0 | -1.0 => F(x),
        0.5 => F(f32::from_bits(0x3e90_d9ff)),
        x => panic!("no stick word known for the tape's {x}"),
    }
}

fn keys_of(ee: &Ee, car: u32) -> InputKeys {
    let f = |o: u32| F(ee.f32_at(car + u(o)));
    InputKeys {
        max_analog_throttle: f(drive::at::MAX_ANALOG_THROTTLE),
        digital_throttle_speed: f(drive::at::DIGITAL_THROTTLE_SPEED),
        digital_side_slip_angle: f(drive::at::DIGITAL_SIDE_SLIP_ANGLE),
    }
}

fn input_of(ee: &Ee, car: u32) -> Input {
    use drive::at;
    let f = |o: u32| F(ee.f32_at(car + u(o)));
    Input {
        stick: f(at::STICK),
        lean: f(at::LEAN),
        pad_word: f(at::PAD_WORD),
        gas_pedal: f(at::GAS_PEDAL),
        vehicle_brake: f(at::VEHICLE_BRAKE),
        hand_brake: f(at::HAND_BRAKE),
        clutch: f(at::CLUTCH),
        dpad: ee.u32_at(car + u(at::DPAD)) != 0,
        gassed: ee.u32_at(car + u(at::GASSED)) != 0,
    }
}

fn put_input(ee: &mut Ee, car: u32, p: &Input) {
    use drive::at;
    for (o, v) in [
        (at::STICK, p.stick),
        (at::LEAN, p.lean),
        (at::PAD_WORD, p.pad_word),
        (at::GAS_PEDAL, p.gas_pedal),
        (at::VEHICLE_BRAKE, p.vehicle_brake),
        (at::HAND_BRAKE, p.hand_brake),
        (at::CLUTCH, p.clutch),
    ] {
        ee.set_u32(car + u(o), v.to_bits());
    }
    ee.set_u32(car + u(at::DPAD), u32::from(p.dpad));
    ee.set_u32(car + u(at::GASSED), u32::from(p.gassed));
}

/// The input step's words for frame `j` (the `j`th from the image) off tape tick `j - 2`, through
/// [`atv_car::input_untamed`]: Cross the gas at a strength of one (the tapes hold nothing else but
/// the stick), the stick [`stick_of`] the tape's, the pad word's command untouched; the image's own
/// before that.
fn u_feed(ee: &mut Ee, d: &UDrive, ticks: &[Row], j: usize) {
    let Some(t) = j.checked_sub(2).and_then(|t| ticks.get(t)) else { return };
    let held: Vec<&str> = t["buttons"].split(' ').collect();
    let mut p = input_of(ee, d.car);
    let (speed, side_slip) = (F(ee.f32_at(d.car + u(drive::at::SPEED))), F(ee.f32_at(d.car + u(drive::at::SIDE_SLIP))));
    let value = |c: Command| if c == Command::GAS && held.contains(&"cross") { F::ONE } else { F::ZERO };
    let stick = (stick_of(t["lx"].parse().unwrap()), stick_of(t["ly"].parse().unwrap()));
    atv_car::input_untamed(&keys_of(ee, d.car), &mut p, speed, side_slip, stick, F::ZERO, value);
    put_input(ee, d.car, &p);
}

/// The car's put-back spot (+0xd30 its place, +0xd40 and +0xd50 its turn's rows, +0xd60 the
/// look's): Free Ride's script keeps it where the truck last stood (its message 4 to the car,
/// 0x004ff110), outside the sim frame; the respawn stands the truck there.
pub(crate) const U_SPOT: std::ops::Range<u32> = 0xd30..0xd6c;

/// The words the game writes outside the sim frame, laid in before frame `j` off the row read after
/// frame `j - 1` (the image's before frame 2).
fn u_lay(ee: &mut Ee, d: &UDrive, rows: &[Row], j: usize) {
    let Some(row) = j.checked_sub(2).and_then(|r| rows.get(r)) else { return };
    let car = row_block(row, "car");
    for o in U_SPOT.step_by(4) {
        ee.set_u32(d.car + o, u32::from_le_bytes(car[o as usize..o as usize + 4].try_into().unwrap()));
    }
}

/// One frame of SLUS-21701's: the script's words off the capture, the input step's off the tape,
/// then the sim's frame.
fn u_step(ee: &mut Ee, d: &UDrive, rows: &[Row], ticks: &[Row], j: usize) {
    u_lay(ee, d, rows, j);
    u_feed(ee, d, ticks, j);
    ee.set(28, U_GP);
    ee.call(U_FRAME, &[u64::from(U_SIM)], &[f32::from_bits(U_DT)], U_BUDGET);
}

/// Runs SLUS-21701's world step from inside its hook, as drives.rs's `world_step_inside`.
pub(crate) fn u_world_step_inside(ee: &mut Ee) -> (u32, F) {
    let (world, dt) = (ee.r(4) as u32, F(ee.f(12)));
    u_run_inside(ee, U_WORLD_STEP);
    (world, dt)
}

/// Runs the hooked function at `addr` from inside its hook (the interpreter keeps the hook out while
/// it runs) on the eight integer and eight float argument registers it was called with.
pub(crate) fn u_run_inside(ee: &mut Ee, addr: u32) {
    let (sp, ra, stack) = (ee.r(29), ee.r(31), ee.stack);
    ee.stack = (sp as u32 - 0x100) & !0xf;
    let ints: Vec<u64> = (4..12).map(|k| ee.r(k)).collect();
    let floats: Vec<f32> = (12..20).map(|k| ee.f(k)).collect();
    let (steps, written) = (ee.steps, ee.written.clone());
    ee.call(addr, &ints, &floats, U_BUDGET);
    ee.written.extend(written);
    (ee.stack, ee.pc, ee.steps) = (stack, addr, steps + ee.steps);
    ee.set(29, sp);
    ee.set(31, ra);
}

fn row_bytes(row: &Row, key: &str) -> Vec<u8> {
    let s = &row[key];
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
}

/// The blocks the recorder keeps: the car (both halves), its body, engine and four tires; each its
/// address in RAM and its bytes in a row.
fn u_blocks(ee: &Ee, d: &UDrive) -> Vec<(&'static str, u32, usize)> {
    let mut out = vec![("car", d.car, 0x1800), ("body", ee.u32_at(d.car + 0xe8), 0x230), ("engine", ee.u32_at(d.car + u(0x1250)), 0x200)];
    for (k, name) in ["tire0", "tire1", "tire2", "tire3"].into_iter().enumerate() {
        out.push((name, ee.u32_at(d.car + u(0x1254) + 4 * k as u32), 0x300));
    }
    out
}

fn row_block(row: &Row, name: &str) -> Vec<u8> {
    match name {
        "car" => [row_bytes(row, "obj0"), row_bytes(row, "obj1")].concat(),
        "body" | "engine" => row_bytes(row, name),
        t => row_bytes(row, &format!("{t}.raw")),
    }
}

/// The interpreter's frames against MD160's capture: the body, engine and tires a row holds are the
/// interpreter's after that row's frame, bit for bit, but on early rows (the frame before's: the
/// recorder read before the frame ran) and torn ones (it read while the frame ran: words of the frames
/// around it and of steps between).
/// The rows after a torn one match again, and one of the last ten does, so the interpreter never
/// drifts off the game. Returns the rows it ran and the torn ones.
pub(crate) fn u_capture_check(d: &UDrive) -> Option<(usize, Vec<usize>, Vec<usize>)> {
    let pre = u_image(d.image)?;
    let rows = u_rows(d.name)?;
    let ticks = u_tape(d.name)?;
    assert_eq!(pre.u32_at(d.car + 4), d.vtable, "{}: not the truck at {:#x}", d.name, d.car);
    let mut ee = u_hooked(pre);
    let blocks = u_blocks(&ee, d)[1..].to_vec();
    let snap = |ee: &Ee| -> Vec<u32> { blocks.iter().flat_map(|&(_, at, len)| (0..len as u32).step_by(4).map(move |o| (at, o))).map(|(at, o)| ee.u32_at(at + o)).collect() };
    let (mut early, mut torn) = (Vec::new(), Vec::new());
    let mut before = snap(&ee);
    for (r, row) in rows.iter().enumerate() {
        u_step(&mut ee, d, &rows, &ticks, r + 1);
        let now = snap(&ee);
        let want: Vec<u32> = blocks.iter().flat_map(|b| row_block(row, b.0).chunks(4).take(b.2 / 4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect::<Vec<_>>()).collect();
        assert_eq!(want.len(), now.len(), "{} row {r}: short blocks", d.name);
        if want == before {
            early.push(r);
        } else if want != now {
            torn.push(r);
        }
        before = now;
    }
    let n = rows.len();
    let off = early.len() + torn.len();
    assert!(off * 3 < n, "{}: {off} of {n} rows are not the frame's: early {early:?}, torn {torn:?}", d.name);
    assert!((n - 10..n).any(|r| !early.contains(&r) && !torn.contains(&r)), "{}: none of the last ten rows match", d.name);
    Some((n, early, torn))
}

fn u_capture_test(d: &UDrive) {
    let _ = u_capture_check(d);
}

#[test]
fn untamed_throttle_capture() {
    u_capture_test(&U_THROTTLE);
}

#[test]
fn untamed_steer_capture() {
    u_capture_test(&U_STEER);
}

#[test]
fn untamed_jump_capture() {
    u_capture_test(&U_JUMP);
}

#[test]
fn untamed_wreck_capture() {
    u_capture_test(&U_WRECK);
}


// --- The port's frame beside SLUS-21701's ---------------------------------------------------------

/// SLUS-21701's ground query (`world, point in/out, normal out, 0, record byte out`; SLUS-21104's
/// 0x004833c0).
pub(crate) const U_GROUND_QUERY: u32 = 0x004b_0910;
/// SLUS-21701's two collidable classes' vtables: the ray probe and the mesh (SLUS-21104's 0x003b6c20
/// and 0x003b6d00).
/// SLUS-21701's smooth noise (`f12` the place, `f0` the noise; SLUS-21104's 0x00134580) and the
/// engine's call of it for the sound's pitch.
pub(crate) const U_SMOOTH_NOISE: u32 = 0x0013_24b0;
const U_NOISE_FROM: u32 = 0x004d_6cac;
/// The collision record's resolution (vt +0x88 of 0x003db860; `a0` the record).
const U_RESOLVE: u32 = 0x0020_6170;
/// The tire probe's ground query returns here (SLUS-21104's probe 0x004b51f0 is 0x004d7890).
const U_TIRE_PROBE_FROM: u32 = 0x004d_7af4;
const U_RAYS_VTABLE: u32 = 0x003d_9120;
const U_MESH_VTABLE: u32 = 0x003d_9200;

/// One ground query the original's frame made: the point asked, the point and normal it answered
/// and the record byte it left (none without a record).
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct UGround {
    from: u32,
    /// Where the answer's record goes (a tire's +0xf0 for the tire probes), 0 for none.
    to: u32,
    asked: [u32; 4],
    point: Vf,
    normal: Vf,
    record: Option<u8>,
}

/// `ee`'s RAM with the car block laid out as SLUS-21104 keeps it (`view[car + o] = ram[car + u(o)]`),
/// and its scene nodes' words moved back 0x30, so the readers of the SLUS-21104 harness read
/// SLUS-21701's car.
pub(crate) fn u_view(ee: &Ee, car: u32) -> Ee {
    let mut ram = ee.ram.clone();
    for o in 0..U_CAR_SIZE {
        ram[(car + o) as usize] = ee.ram.get((car + u(o)) as usize).copied().unwrap_or(0);
    }
    // The car's scene node, its parents and its subtree: their words +0x30 on (research/untamed-machines.md,
    // MD81b); parent, child and sibling at +0x120, +0x124, +0x128.
    let first = ee.u32_at(car + u(0xe4));
    let mut nodes = vec![first];
    while let n @ 1.. = ee.u32_at(nodes[nodes.len() - 1] + 0x120) {
        assert!(!nodes.contains(&n), "a loop of parents");
        nodes.push(n);
    }
    let mut todo = vec![ee.u32_at(first + 0x124)];
    while let Some(n) = todo.pop() {
        if n == 0 {
            continue;
        }
        assert!(!nodes.contains(&n), "a loop in the subtree");
        nodes.push(n);
        todo.push(ee.u32_at(n + 0x128));
        todo.push(ee.u32_at(n + 0x124));
    }
    for n in nodes {
        for o in 0x30..0x110 {
            ram[(n + o) as usize] = ee.ram[(n + o + 0x30) as usize];
        }
    }
    // The car's two collidables (the riding ray probe and the frame mesh, car +0xcc and +0xd0), and
    // those of the records its record's contacts are with: the database entry +0x20 at +0x4c, the
    // centre and reach +0x30 on, the owner, record and category +0x2c on, the frame and what follows
    // it +0x20 on (a ray probe's frame and tree, a mesh's frame to its mesh pointer); their vtables
    // told by SLUS-21104's.
    let record = ee.u32_at(car + u(0xc8));
    let mut collidables = vec![ee.u32_at(car + u(0xcc)), ee.u32_at(car + u(0xd0))];
    if record != 0 {
        let list = ee.u32_at(record + 0xf8);
        for i in 0..ee.u32_at(record + 0xf0).min(0x100) {
            let other = ee.u32_at(list + 0x90 * i + 0x80);
            if other != 0 {
                collidables.push(ee.u32_at(other + 0x208));
            }
        }
    }
    collidables.sort();
    collidables.dedup();
    for a in collidables {
        if a == 0 {
            continue;
        }
        let vt = match ee.u32_at(a + 0xc) {
            U_RAYS_VTABLE => 0x003b_6c20u32,
            U_MESH_VTABLE => 0x003b_6d00,
            vt => panic!("a collidable at {a:#x} of vtable {vt:#010x}"),
        };
        let mut mv = |at: u32, len: u32, from: u32| {
            for k in 0..len {
                ram[(a + at + k) as usize] = ee.ram[(a + from + k) as usize];
            }
        };
        mv(0x20, 4, 0x4c);
        mv(0x50, 0x24, 0x80);
        mv(0x88, 0x10, 0xb4);
        // A ray probe's tree lives in its own tail (+0x1e0), which the copy must not reach.
        mv(0x130, if vt == 0x003b_6c20 { 0x58 } else { 0x118 }, 0x150);
        ram[(a + 0xc) as usize..(a + 0x10) as usize].copy_from_slice(&vt.to_le_bytes());
    }
    // The tires' lever pointers point into the car: at the view's words.
    for k in 0..4 {
        let t = ee.u32_at(car + u(0x1254) + 4 * k);
        for o in [0x24, 0x28] {
            let x = ee.u32_at(t + o);
            if (car..car + U_CAR_SIZE).contains(&x) {
                let r = x - car;
                let back = (0..U_CAR_SIZE).find(|&o| u(o) == r).unwrap_or_else(|| panic!("a lever at car +{r:#x}"));
                ram[(t + o) as usize..(t + o + 4) as usize].copy_from_slice(&(car + back).to_le_bytes());
            }
        }
    }
    // The car's record's contacts (0x90 each): SLUS-21701 files the kind and the surface as halves of
    // +0, and the other and the mover at +0x80/+0x84; SLUS-21104 keeps the kind a word and the
    // surface, the other and the mover at +0x80/+0x84/+0x88.
    if record != 0 {
        let list = ee.u32_at(record + 0xf8);
        for i in 0..ee.u32_at(record + 0xf4).min(0x100) {
            let e = list + 0x90 * i;
            let (kind, surface) = (ee.u32_at(e) & 0xffff, ee.u32_at(e) as i32 >> 16);
            let (other, mover) = (ee.u32_at(e + 0x80), ee.u32_at(e + 0x84));
            for (o, x) in [(0, kind), (0x80, surface as u32), (0x84, other), (0x88, mover)] {
                ram[(e + o) as usize..(e + o + 4) as usize].copy_from_slice(&x.to_le_bytes());
            }
        }
    }
    // The vehicle state's pad (the rumble's), SLUS-21104's +0x5c, is at +0x54, and its wheel word,
    // SLUS-21104's +0x54, at +0x58 (the controls 0x4e7e90 read it).
    let vstate = ee.u32_at(car + u(0xbc4));
    if vstate != 0 {
        for o in 0..4 {
            ram[(vstate + 0x5c + o) as usize] = ee.ram[(vstate + 0x54 + o) as usize];
            ram[(vstate + 0x54 + o) as usize] = ee.ram[(vstate + 0x58 + o) as usize];
        }
    }
    // The terrain's extent pointer, SLUS-21104's +0x43c, is at +0x42c.
    let terrain = ee.u32_at(car + u(0x3b0));
    if terrain != 0 {
        for o in 0..4 {
            ram[(terrain + 0x43c + o) as usize] = ee.ram[(terrain + 0x42c + o) as usize];
        }
    }
    Ee::with_ram(ram)
}

/// The ground query hooked to run as it is and log what it was asked and answered.
fn u_hook_ground(ee: &mut Ee, log: Shared<Vec<UGround>>) {
    ee.hooks.insert(U_GROUND_QUERY, Box::new(move |ee: &mut Ee| {
        let (pt, n, rec) = (ee.r(5) as u32, ee.r(6) as u32, ee.r(8) as u32);
        assert_eq!(ee.r(7), 0, "the ground query's fourth argument");
        let (from, asked) = (ee.r(31) as u32, get_v(ee, pt).to_bits());
        u_run_inside(ee, U_GROUND_QUERY);
        log.borrow_mut().push(UGround { from, to: rec, asked, point: get_v(ee, pt), normal: get_v(ee, n), record: (rec != 0).then(|| ee.u8_at(rec)) });
    }));
}

/// The smooth noise hooked to run as it is and log the engine's places and answers, with the
/// engine asking (`s0`).
fn u_hook_noise(ee: &mut Ee, log: Shared<Vec<(u32, u32, F)>>) {
    ee.hooks.insert(U_SMOOTH_NOISE, Box::new(move |ee: &mut Ee| {
        let (from, engine, at) = (ee.r(31) as u32, ee.r(16) as u32, ee.f(12).to_bits());
        u_run_inside(ee, U_SMOOTH_NOISE);
        if from == U_NOISE_FROM {
            log.borrow_mut().push((engine, at, F(ee.f(0))));
        }
    }));
}

/// The sort probe's search as SLUS-21701 left it in RAM ([`super::drives::sort_answer`] at its
/// offsets: the probe's struck flag at +0x144 and list at +0x1b0, SLUS-21104's +0x126 and +0x7c; a
/// hit collidable's object and mover at +0xbc and +0xb8, SLUS-21104's +0x90 and +0x8c). A mover is
/// not met.
fn u_sort_answer(ee: &Ee, car: u32) -> SortAnswer {
    let probe = ee.u32_at(car + u(0x1264));
    let mut a = SortAnswer { found: ee.u8_at(probe + 0x144) & 0x20 != 0, ..SortAnswer::default() };
    if !a.found {
        return a;
    }
    let list = ee.u32_at(probe + 0x1b0);
    a.count = ee.u32_at(list + sort::COUNT) as usize;
    assert!(a.count <= SORT_HITS, "the sort struck {} times", a.count);
    let (froms, normals, points, records) = (ee.u32_at(list + 0x90), ee.u32_at(list + 0x94), ee.u32_at(list + 0x98), ee.u32_at(list + 0x9c));
    for (i, h) in a.hits.iter_mut().enumerate().take(a.count) {
        let record = ee.u32_at(records + 4 * i as u32);
        let mover = ee.u32_at(record + 0xb8);
        assert_eq!(mover, 0, "the sort struck a mover");
        *h = atv_car::SortHit {
            from: get_v(ee, froms + 0x20 * i as u32),
            point: get_v(ee, points + 0x10 * i as u32),
            normal: get_v(ee, normals + 0x10 * i as u32),
            object: ee.u32_at(record + 0xbc),
            mover,
            mover_kind: 0,
            record,
        };
    }
    a
}

/// The car's record's contacts when one is with another record, which the port's vehicle does not
/// hold ([`atv_car::Vehicle::held`]): the car's held as if none were, and the other records' (their
/// own lists left out, their bodies whole), which [`Call::GravityRecord`] resolves.
pub(crate) struct UHeld {
    car: game_sim::atv_contact::Held,
    others: Vec<game_sim::atv_contact::Held>,
}

/// [`UHeld`] read from `w` (the view) for the car's record at `record` with `contacts` contacts;
/// `None` when none is with another record. A mover is not met.
fn u_held(w: &Ee, record: u32, contacts: u32) -> Option<UHeld> {
    use game_sim::handling::record::held;
    let list = w.u32_at(record + 0xf8);
    let entries: Vec<u32> = (0..contacts).map(|i| list + 0x90 * i).collect();
    assert!(entries.iter().all(|&e| w.u32_at(e + 0x88) == 0), "a contact with a mover");
    let with: Vec<u32> = entries.iter().map(|&e| w.u32_at(e + 0x84)).collect();
    if with.iter().all(|&o| o == 0) {
        return None;
    }
    let mut records: Vec<u32> = with.iter().copied().filter(|&o| o != 0).collect();
    records.dedup();
    let mut car = held(&|a| if entries.iter().any(|&e| a == e + 0x84) { 0 } else { w.u32_at(a) }, record).expect("no other, no mover");
    for (e, &o) in car.entries.iter_mut().zip(&with) {
        e.contact.other = (o != 0).then(|| records.iter().position(|&r| r == o).unwrap());
    }
    let others = records.iter().map(|&r| held(&|a| if a == r + 0xf0 { 0 } else { w.u32_at(a) }, r).expect("an empty list")).collect();
    Some(UHeld { car, others })
}

/// The port's calls answered beside SLUS-21701's frame: the ground queries by the original's
/// answers to the same point, the sort's search by the original's; the rest listed.
pub(crate) struct UBeside {
    ground: Vec<UGround>,
    noise: Vec<(u32, F)>,
    used: Vec<UGround>,
    sort: SortAnswer,
    held: Option<UHeld>,
    resolves: usize,
    /// The tire steps handed out for a tire standing on an object, which the port leaves to its
    /// caller and this harness cannot answer: the frame is not compared.
    unported: Vec<Call>,
    /// The original unwrecked and put the truck back this frame (not compared): a query the original
    /// never asked is answered with its nearest.
    put_back: bool,
    /// The drive and frame, for the panics.
    at: String,
    pub(crate) asked: usize,
    pub(crate) rest: Vec<Call>,
}

impl UBeside {
    fn answer(&mut self, at: Vf) -> UGround {
        self.asked += 1;
        let mut k = self.ground.iter().position(|g| g.asked == at.to_bits());
        if k.is_none() && (self.put_back || !self.unported.is_empty()) {
            let d = |g: &UGround| { let v = Vf(g.asked.map(f32::from_bits)); (0..3).map(|i| (v.0[i] - at.0[i]).abs()).sum::<f32>() };
            k = (0..self.ground.len()).min_by(|&x, &y| d(&self.ground[x]).total_cmp(&d(&self.ground[y])));
        }
        let k = k.unwrap_or_else(|| panic!("{}: the port asked the ground at {:08x?}, the original never did (it asked {:08x?})", self.at, at.to_bits(), self.ground.iter().map(|g| g.asked).collect::<Vec<_>>()));
        let g = self.ground.remove(k);
        self.used.push(g);
        g
    }

    /// The original's queries the port did not make: none but repeats of one it made (the same
    /// caller, point and answer).
    fn unasked(&self) -> Vec<UGround> {
        self.ground.iter().filter(|g| !self.used.contains(g)).copied().collect()
    }
}

impl Steps for UBeside {
    fn call(&mut self, c: Call, car: &mut Car, body: &mut Body) {
        match c {
            Call::GravityRecord if self.held.is_some() => {
                use game_sim::atv_contact::{Weighed, resolve};
                use game_sim::contact::{Other, Own, Side};
                let UHeld { car: h, others } = self.held.as_mut().expect("tested");
                let mut me = Weighed { body, mass: h.mass, cofg: car.cofg };
                let mut bodies: Vec<_> = others
                    .iter_mut()
                    .map(|o| {
                        let cofg = o.record.collider.cofg;
                        (&mut o.record.collider, Weighed { body: o.body.as_mut().expect("the other's body"), mass: o.mass, cofg })
                    })
                    .collect();
                let mut others: Vec<Other<'_>> = bodies.iter_mut().map(|(c, b)| Other { collider: &mut **c, side: Side::Body(b) }).collect();
                resolve(&mut h.record, &mut Own::Body(&mut me), &mut h.entries, &mut others, &h.movers, true);
                self.resolves += 1;
            }
            Call::GroundUnder(at) => car.ground_normal = self.answer(at).normal,
            Call::TireGround(k, at) => {
                let g = self.answer(at);
                car.tires[k].query = (g.point, g.normal);
                if let Some(r) = g.record {
                    car.tires[k].surface = r;
                }
            }
            Call::Noise(at) => {
                let k = self.noise.iter().position(|&(x, _)| x == at.to_bits());
                let k = k.unwrap_or_else(|| panic!("the port asked the noise at {at:?}, the original's engine never did ({:?})", self.noise));
                car.noise = self.noise.remove(k).1;
            }
            Call::TireStep(..) => self.unported.push(c),
            Call::SortSearch => {
                car.sort = self.sort;
                self.rest.push(c);
            }
            c => self.rest.push(c),
        }
    }
}

/// What a drive beside came to.
#[derive(Default)]
pub(crate) struct USeen {
    pub(crate) frames: usize,
    pub(crate) words: usize,
    pub(crate) grounds: usize,
    /// The original's queries that repeat one the port made.
    pub(crate) repeats: usize,
    pub(crate) handed: std::collections::BTreeMap<String, usize>,
    pub(crate) other_worlds: usize,
    pub(crate) wrecked: usize,
    /// Frames not compared: a tire stood on an object ([`UBeside::unported`]).
    pub(crate) on_objects: Vec<usize>,
    /// Frames not compared: the truck unwrecked and was put back at its spot.
    pub(crate) respawned: Vec<usize>,
    /// Frames whose record held contacts with another record.
    pub(crate) with_others: usize,
}

/// Replays drive `d` as [`u_capture_check`] does, and at each frame's world step on the truck's world
/// reads the car (through [`u_view`]), body, engine, tires, keys, state and vehicle words, runs
/// SLUS-21701's step, then the port's Untamed frame ([`atv_car::frame_untamed`], or through
/// [`atv_car::Ride`] with its update Untamed's) on what it read: every word the port keeps must equal
/// the original's after it, every frame.
pub(crate) fn u_beside(d: &UDrive, ride: bool) -> Option<USeen> {
    let pre = u_image(d.image)?;
    let rows = u_rows(d.name)?;
    let ticks = u_tape(d.name)?;
    assert_eq!(pre.u32_at(d.car + 4), d.vtable, "{}: not the truck at {:#x}", d.name, d.car);
    THE_CAR.with(|c| c.set(d.car));
    let table: &'static AcosTable = Box::leak(Box::new(AcosTable::build()));
    let mut ee = u_hooked(pre);
    let ground: Shared<Vec<UGround>> = Rc::default();
    u_hook_ground(&mut ee, ground.clone());
    let noise: Shared<Vec<(u32, u32, F)>> = Rc::default();
    u_hook_noise(&mut ee, noise.clone());
    // The car's record's resolution, when a contact is with another record: the records and bodies
    // as they are when it runs (the others' may have stepped earlier in the world step).
    let resolving: Shared<Vec<UHeld>> = Rc::default();
    {
        let (log, car) = (resolving.clone(), d.car);
        ee.hooks.insert(U_RESOLVE, Box::new(move |ee: &mut Ee| {
            let rec = ee.u32_at(car + u(0xc8));
            if ee.r(4) as u32 == rec && ee.u32_at(rec + 0xf0) != 0 {
                let w = u_view(ee, car);
                if let Some(h) = u_held(&w, rec, w.u32_at(rec + 0xf0)) {
                    log.borrow_mut().push(h);
                }
            }
            u_run_inside(ee, U_RESOLVE);
        }));
    }
    let seen: Shared<USeen> = Rc::default();
    let frame_no: Shared<usize> = Rc::default();
    let (keep, at_frame, name, car_at_) = (seen.clone(), frame_no.clone(), d.name, d.car);
    let the_ride: Shared<Option<atv_car::Ride>> = Rc::default();
    ee.hooks.insert(U_WORLD_STEP, Box::new(move |ee: &mut Ee| {
        let k = *at_frame.borrow();
        let w = ee.r(4) as u32;
        let listeners: Vec<u32> = (0..u32::from(ee.u16_at(w + 0x50))).map(|i| ee.u32_at(ee.u32_at(w + 0x58) + 4 * i)).collect();
        if !listeners.contains(&(car_at_ + 0x60)) {
            u_run_inside(ee, U_WORLD_STEP);
            keep.borrow_mut().other_worlds += 1;
            return;
        }
        let vb = u_view(ee, car_at_);
        let p = ptrs(&vb);
        let o = own_ptrs(&vb, &p);
        let mut car = read_car(&vb, &p);
        let mut body = game_sim::handling::seed::body(&|a| vb.u32_at(a), p.body);
        let mut v = read_vehicle(&vb, &p, &o);
        v.untamed = true;
        v.scene.untamed = true;
        let contacts = vb.u32_at(p.record + 0xf0);
        if contacts != 0 {
            v.held = game_sim::handling::record::held(&|a| vb.u32_at(a), p.record);
        }
        let held = u_held(&vb, p.record, contacts);
        let mut m = read_mx(&vb, &p);
        m.keys.untamed = true;
        ground.borrow_mut().clear();
        noise.borrow_mut().clear();
        resolving.borrow_mut().clear();
        let (_, dt) = u_world_step_inside(ee);
        // The world steps every car; keep this car's tire probes and engine noise.
        let tires: Vec<u32> = (0..4).map(|k| ee.u32_at(car_at_ + u(0x1254) + 4 * k) + 0xf0).collect();
        ground.borrow_mut().retain(|g| g.from != U_TIRE_PROBE_FROM || tires.contains(&g.to));
        let engine = ee.u32_at(car_at_ + u(0x1250));
        noise.borrow_mut().retain(|&(e, _, _)| e == engine);
        let va = u_view(ee, car_at_);
        let mut resolved_with = std::mem::take(&mut *resolving.borrow_mut());
        assert_eq!(resolved_with.is_empty(), held.is_none(), "{name} frame {k}: the original resolved the record with others {} times", resolved_with.len());
        let resolves = resolved_with.len();
        let held = (!resolved_with.is_empty()).then(|| resolved_with.remove(0));
        let all_from: Vec<u32> = ground.borrow().iter().map(|g| g.from).collect();
        let beside = UBeside { ground: std::mem::take(&mut *ground.borrow_mut()), noise: noise.borrow_mut().drain(..).map(|(_, at, f)| (at, f)).collect(), used: Vec::new(), sort: u_sort_answer(ee, car_at_), held, resolves: 0, unported: Vec::new(), put_back: false, at: String::new(), asked: 0, rest: Vec::new() };
        let put_back = car.state != atv_car::RIDING && read_car(&va, &p).state == atv_car::RIDING;
        let mut pieces = Pieces { keys: m.keys(), state: m.state, rest: UBeside { at: format!("{name} frame {k}"), put_back, ..beside } };
        if ride {
            let mut r = the_ride.borrow_mut();
            let r = r.get_or_insert_with(|| super::drives::ride_of(table));
            r.update.untamed = true;
            (r.keys, r.state, r.car, r.body, r.vehicle) = (std::mem::take(&mut pieces.keys), pieces.state, car, body, v);
            let rest = std::mem::replace(&mut pieces.rest, UBeside { ground: Vec::new(), noise: Vec::new(), used: Vec::new(), sort: SortAnswer::default(), held: None, resolves: 0, unported: Vec::new(), put_back: false, at: String::new(), asked: 0, rest: Vec::new() });
            pieces.rest = r.frame_with(dt, rest);
            (pieces.keys, pieces.state) = (std::mem::take(&mut r.keys), r.state);
            (car, body) = (r.car.clone(), r.body.clone());
            v = r.vehicle.clone();
        } else {
            atv_car::frame_untamed(&atv_car::WORLD, &mut car, &mut body, dt, table, &mut atv_car::Own { v: &mut v, rest: &mut pieces });
        }
        if !pieces.rest.unported.is_empty() {
            keep.borrow_mut().on_objects.push(k);
            return;
        }
        // The unwreck's respawn stands the truck at its spot through Untamed's own put-back
        // (0x4ca900: a ground query under the spot, the node's rows, the body placed from the node),
        // not MX Unleashed's the port has.
        if put_back {
            keep.borrow_mut().respawned.push(k);
            return;
        }
        let pm = Mx { keys: Keys { tires: pieces.keys.tires.clone(), ..m.keys.clone() }, state: pieces.state, ..m };
        let resolved: Vec<(u32, u32)> = v
            .held
            .as_ref()
            .or(pieces.rest.held.as_ref().map(|h| &h.car))
            .filter(|_| contacts != 0)
            .map_or(Vec::new(), |h| game_sim::handling::record::held_words(&game_sim::atv_contact::Held { body: None, ..h.clone() }, p.record, &|a| va.u32_at(a)));
        let parts = [car_words(&car, &p), game_sim::handling::seed::body_words(&body, p.body), mx_words(&pm, &p), vehicle_words(&va, &v, &p, &o), resolved];
        let counts = parts.clone().map(|ws| ws.iter().filter(|&&(a, x)| va.u32_at(a) != x).count());
        let after: Vec<(u32, u32)> = parts.concat();
        let bad: Vec<String> = after
            .iter()
            .filter(|&&(a, x)| va.u32_at(a) != x)
            .take(40)
            .map(|&(a, x)| {
                let o = a.wrapping_sub(car_at_);
                let at = if o < U_CAR_SIZE { format!("car +{:#x} (21104 +{o:#x})", u(o)) } else { format!("{a:#x}") };
                format!("  {at}: port {x:#010x} ({}), original {:#010x} ({})", f32::from_bits(x), va.u32_at(a), va.f32_at(a))
            })
            .collect();
        assert!(
            bad.is_empty(),
            "{name} frame {k}: words of the port's Untamed frame differ from SLUS-21701's (car, body, pieces, vehicle, record: {counts:?}; {contacts} contacts, state {}, handed out {:?}):\n{}",
            car.state,
            pieces.rest.rest,
            bad.join("\n")
        );
        assert_eq!(pieces.rest.resolves, resolves, "{name} frame {k}: the port resolved the record with others {} times, the original {resolves}", pieces.rest.resolves);
        assert!(pieces.rest.noise.is_empty(), "{name} frame {k}: the original's engine asked the noise {} times the port did not", pieces.rest.noise.len());
        let unasked = pieces.rest.unasked();
        assert!(
            unasked.is_empty(),
            "{name} frame {k}: the original asked the ground {} times the port did not (the port {}): from {:x?} of {all_from:x?}",
            unasked.len(),
            pieces.rest.asked,
            unasked.iter().map(|g| g.from).collect::<Vec<_>>()
        );
        let mut s = keep.borrow_mut();
        for c in &pieces.rest.rest {
            let n = format!("{c:?}").split([' ', '(', '{']).next().unwrap().to_string();
            assert!(super::drives::HANDED_OUT.contains(&n.as_str()), "{name} frame {k}: the port's frame handed out {c:?}");
            *s.handed.entry(n).or_default() += 1;
        }
        s.frames += 1;
        s.with_others += usize::from(resolves != 0);
        s.words += after.len();
        s.grounds += pieces.rest.asked;
        s.repeats += pieces.rest.ground.len();
        s.wrecked += usize::from(car.state != atv_car::RIDING);
    }));
    for r in 0..rows.len() {
        *frame_no.borrow_mut() = r + 1;
        u_step(&mut ee, d, &rows, &ticks, r + 1);
    }
    let seen = std::mem::take(&mut *seen.borrow_mut());
    eprintln!(
        "MD161a {} ({}): {} frames, {} words in all equal to SLUS-21701's; {} ground queries answered; handed out {:?}; wrecked on {}; {} other world steps; contacts with another record on {}; not compared, a tire on an object: {:?}, put back: {:?}",
        d.name,
        if ride { "through Ride" } else { "frame_untamed" },
        seen.frames,
        seen.words,
        seen.grounds,
        seen.handed,
        seen.wrecked,
        seen.other_worlds,
        seen.with_others,
        seen.on_objects,
        seen.respawned
    );
    Some(seen)
}

/// Drive `d` beside, through the frame or the Ride: every frame compared but those with a tire on an
/// object, and none of those but where the original's tire stands on one.
fn u_beside_test(d: &UDrive, ride: bool) {
    let Some(s) = u_beside(d, ride) else { return };
    assert!(s.frames > 0, "{}: no frame compared", d.name);
}

#[test]
fn untamed_throttle_frame_beside() {
    u_beside_test(&U_THROTTLE, false);
}

#[test]
fn untamed_throttle_ride_beside() {
    u_beside_test(&U_THROTTLE, true);
}

#[test]
fn untamed_steer_frame_beside() {
    u_beside_test(&U_STEER, false);
}

#[test]
fn untamed_steer_ride_beside() {
    u_beside_test(&U_STEER, true);
}

#[test]
fn untamed_jump_frame_beside() {
    u_beside_test(&U_JUMP, false);
}

#[test]
fn untamed_jump_ride_beside() {
    u_beside_test(&U_JUMP, true);
}

#[test]
fn untamed_wreck_frame_beside() {
    u_beside_test(&U_WRECK, false);
}

#[test]
fn untamed_wreck_ride_beside() {
    u_beside_test(&U_WRECK, true);
}

