//! MD150: MX vs. ATV's car built from its files ([`atv_car::Load`]), word for word against MD39a's five
//! spawn images (research/oracle.txt `# MD150`).
//!
//! The car's setup reads two things off the chassis model in a frame no spawn image keeps (the car has
//! settled since): the tire nodes' points and the part reader's half box. The main test hands in the
//! image's mounts (less `TireWidth`) and the box solved from the record's inertia, as MD122 did for the
//! quad; the two side tests bound what the model gives.
use game_sim::atv_body::Body;
use game_sim::atv_car::{self, Car, EngineKeys, InputKeys, Keys, Lever, Load, TireKeys, Vehicle, Wing};
use game_sim::camera::CurveTable;
use game_sim::car::{MxvatvMachine, Shift, Surface};
use game_sim::handling::{mass, seed};
use game_sim::quad;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};

/// Each car: its image, its file, its chassis model, and where the image's car lives.
const CARS: [(&str, MxvatvMachine, &str, &str, u32); 5] = [
    ("buggy", MxvatvMachine::Buggy, "buggy.phy", "buggy", 0x0159_8e10),
    ("baja", MxvatvMachine::Baja, "baja.phy", "baja", 0x0159_8e10),
    ("sandrail", MxvatvMachine::Sandrail, "sandrail.phy", "sandrail", 0x01b1_58a0),
    ("golf", MxvatvMachine::GolfCart, "golfcart.phy", "golfcart", 0x01b0_dac0),
    ("truck", MxvatvMachine::MonsterTruck, "monstertruck.phy", "monstertruck", 0x0159_8e10),
];

const BODY: u32 = 0xb8;
const RECORD: u32 = 0xc8;
const WORLD: u32 = 0xe8;
const MATRIX: u32 = 0x370;
const ENGINE: u32 = 0x1250;
const TIRES: u32 = 0x1254;
const MOUNT: u32 = 0x70;
const WIDTH: u32 = 0x1ec;
const LEVERS: [(Lever, u32); 4] = [(Lever::Brake, 0x1130), (Lever::BrakeForce, 0x1134), (Lever::Steer, 0x113c), (Lever::RearSteer, 0x1140)];

struct Image {
    ee: Ee,
    o: u32,
    tires: [u32; 4],
}

impl Image {
    fn f(&self, a: u32) -> F {
        F(self.ee.f32_at(a))
    }
    fn v(&self, a: u32) -> Vf {
        Vf(core::array::from_fn(|k| self.ee.f32_at(a + 4 * k as u32)))
    }
    fn b(&self, a: u32) -> bool {
        self.ee.u32_at(a) != 0
    }
    fn at(&self, o: u32) -> u32 {
        self.ee.u32_at(self.o + o)
    }
    fn layout(&self) -> mass::Layout {
        mass::Layout { object: self.o, body: self.at(BODY), record: self.at(RECORD), gravity: self.at(WORLD) }
    }
    /// The image's mounts less each tire's width: the nodes' points as the setup saw them (the float
    /// nearest the difference that adds back to the mount).
    fn points(&self) -> [Vf; 4] {
        self.tires.map(|t| {
            let (m, w) = (self.v(t + MOUNT), self.f(t + WIDTH));
            let x = (F(m.x()) - w).0;
            let x = [0, 1, -1, 2, -2]
                .map(|d: i32| f32::from_bits((x.to_bits() as i32 + d) as u32))
                .into_iter()
                .find(|&x| (F(x) + w).0.to_bits() == m.x().to_bits())
                .expect("a point that adds back to the mount");
            Vf::new(x, m.y(), m.z(), m.w())
        })
    }
}

struct Files {
    kit: Kit,
    curve: CurveTable,
}

impl Files {
    fn load() -> Option<Files> {
        let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
        for dir in ["mxvatv-ps2/md39a", "mxvatv-ps2/md39/phy", "mxvatv-ps2/md39/model"] {
            if !kit.ctx(dir).exists() {
                skip(&format!("{} is not there: MD39's files and MD39a's spawn images", kit.ctx(dir).display()));
                return None;
            }
        }
        Some(Files { kit, curve: CurveTable::build() })
    }
    fn image(&self, name: &str, o: u32) -> Image {
        let path = self.kit.ctx("mxvatv-ps2/md39a").join(format!("{name}.ee{}", ".bin"));
        let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let tires = core::array::from_fn(|k| ee.u32_at(o + TIRES + 4 * k as u32));
        Image { ee, o, tires }
    }
    fn ini(&self, phy: &str) -> game_sim::phy::Ini {
        game_sim::phy::Ini::parse_bytes(&std::fs::read(self.kit.ctx("mxvatv-ps2/md39/phy").join(phy)).unwrap())
    }
    fn model(&self, model: &str) -> quad::Model {
        let dir = self.kit.ctx("mxvatv-ps2/md39/model");
        for col in ["m.col", "f.col"] {
            let p = dir.join(format!("{model}{col}"));
            if !p.exists() {
                eprintln!("note: {} is not there", p.display());
            }
        }
        quad::Model::parse(&std::fs::read(dir.join(format!("{model}.psg"))).unwrap()).unwrap()
    }
}

/// The half box whose inertia is the record's (`+0x60`) and the body's (`+0x130`), searched ±24 floats a lane about the
/// closed form (MD122's).
fn load_box(part: &mass::Part, want: [u32; 3], body: [u32; 3]) -> Option<Vf> {
    let (m, (cm, _)) = (part.body.mass, part.record.mass?);
    let (s, k) = (part.body.inertia_scale, f64::from(cm.0) * f64::from(part.body.inv_mass.0));
    let m12 = f64::from(m.0) / 12.0;
    let w = want.map(|b| f64::from(f32::from_bits(b)) / k);
    let sums = [w[0] / (f64::from(s.x()) * m12), w[1] / (f64::from(s.y()) * m12), w[2] / (f64::from(s.z()) * m12)];
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
                let b = mass::block_inertia(m, s, half);
                if [i.x(), i.y(), i.z()].map(f32::to_bits) == want && [b.x(), b.y(), b.z()].map(f32::to_bits) == body {
                    return Some(half);
                }
            }
        }
    }
    None
}

fn record_want(img: &Image) -> [u32; 3] {
    let r = img.layout().record;
    core::array::from_fn(|k| img.ee.u32_at(r + 0x60 + 4 * k as u32))
}

fn lever(img: &Image, a: u32) -> Option<Lever> {
    LEVERS.iter().find(|(_, o)| img.o + o == a).map(|(l, _)| *l).or_else(|| {
        assert_eq!(a, 0, "a tire's lever pointer");
        None
    })
}

fn read_tire_keys(img: &Image, t: u32) -> TireKeys {
    let f = |o: u32| img.f(t + o);
    let mut k = TireKeys {
        brake: lever(img, img.ee.u32_at(t + 0x24)),
        steer: lever(img, img.ee.u32_at(t + 0x28)),
        max_compression: f(0x30),
        max_compression_rate: f(0x34),
        max_decompression_rate: f(0x38),
        spring: f(0x3c),
        bounce_damping: f(0x40),
        rebound_damping: f(0x44),
        inv_inertia: f(0x48),
        max_steer_angle: f(0x50),
        top_wheel_speed: f(0x90),
        max_tire_angle_rate: f(0x94),
        abs: img.b(t + 0x98),
        min_braking_slip: f(0x9c),
        max_braking_slip: f(0xa0),
        friction_scale: f(0x9a4),
        camber_roll_ratio: f(0x9b0),
        surfaces: [Surface::default(); 16],
    };
    for (i, s) in k.surfaces.iter_mut().enumerate() {
        let fields = [
            &mut s.max_braking_torque,
            &mut s.wheel_friction,
            &mut s.c1,
            &mut s.c2,
            &mut s.c3,
            &mut s.c4,
            &mut s.a0,
            &mut s.a1,
            &mut s.a3,
            &mut s.inv_a2,
            &mut s.inv_a4,
            &mut s.moment_scale,
            &mut s.cornering_stiffness,
            &mut s.max_normal_load,
            &mut s.friction_peak,
            &mut s.friction_end_point,
        ];
        for (j, x) in fields.into_iter().enumerate() {
            *x = f(0x5a4 + 0x40 * j as u32 + 4 * i as u32);
        }
    }
    k
}

fn read_engine_keys(img: &Image) -> EngineKeys {
    let e = img.at(ENGINE);
    let f = |o: u32| img.f(e + o);
    let slot = |i: u32| {
        let t = img.ee.u32_at(e + 0x24 + 4 * i);
        img.tires.iter().position(|&x| x == t)
    };
    let table = |at: u32, n: u32| -> Vec<F> { (0..n).map(|i| img.f(img.ee.u32_at(e + at) + 4 * i)).collect() };
    let point = |i: u32| f(0x84 + 4 * i);
    let rpms = img.ee.u32_at(e + 0xc4);
    EngineKeys {
        drive: [slot(0).expect("the engine's first slot"), slot(1).expect("the engine's second slot")],
        drive_more: slot(2).zip(slot(3)).map(|(a, b)| [a, b]),
        rpm: table(0xc8, rpms),
        torque: table(0xcc, rpms),
        redline: f(0x124),
        gear_ratios: table(0xd4, img.ee.u32_at(e + 0xd0)),
        final_gear_ratio: f(0x48),
        idle_throttle: f(0x40),
        engine_braking: f(0x44),
        transmission_loss: f(0x4c),
        inv_inertia: f(0x54),
        min_rpm: f(0x64),
        pump_torque_scale: f(0x58),
        max_pump_torque: f(0xe4),
        max_k: f(0x5c),
        tcs: img.b(e + 0x68),
        min_traction_slip: f(0x6c),
        max_traction_slip: f(0x70),
        nox_burn_rate: f(0x7c),
        nox_hp_boost: f(0x80),
        shift: Shift {
            low_load_rpm: point(0),
            high_load_rpm: Some(point(1)),
            low_load_up_shift_rpm: point(2),
            high_load_up_shift_rpm: point(3),
            low_load_down_shift_rpm: point(4),
            high_load_down_shift_rpm: point(5),
            shift_time: f(0x9c),
            shift_up_duration: f(0xa8),
            shift_up_throttle: f(0xac),
            shift_down_duration: f(0xb0),
            shift_down_throttle: f(0xb4),
        },
        sound: img.b(e + 0x130),
    }
}

fn read_keys(img: &Image) -> Keys {
    let c = |o: u32| img.o + o;
    let f = |o: u32| img.f(c(o));
    let table = |a: u32| -> [[F; 2]; 3] { core::array::from_fn(|i| [img.f(a + 8 * i as u32), img.f(a + 8 * i as u32 + 4)]) };
    let wings = img.ee.i32_at(c(0x11b4));
    let wing_ptr = img.at(0x11b8);
    let wing = |w: u32| Wing { lift_slope: img.f(w), angle_of_attack: img.f(w + 4), basic_drag: img.f(w + 8), induced_drag: img.f(w + 0xc), position: img.v(w + 0x10) };
    Keys {
        square_steering: img.b(c(0x11c0)),
        brake_rate_limit: f(0x11d4),
        gas_rate_limit: f(0x11d8),
        x_dpad_rate_limit_on: f(0x11dc),
        x_dpad_rate_limit_off: f(0x11e0),
        x_axis_rate_limit: f(0x11e4),
        steer: table(c(0x11e8)),
        wheel_steer: table(c(0x1200)),
        dpad_steer: table(c(0x1218)),
        rear_steer: {
            let [a, b, _] = table(c(0x1230));
            [a, b]
        },
        yaw_rate_limit: f(0x1244),
        hand_brake_yaw_rate_limit: f(0x1248),
        hand_brake_friction_scale: f(0x1158),
        hand_brake_brake_scale: f(0x115c),
        hand_brake_fade_time: f(0x1160),
        cofg: img.v(c(0x10f0)),
        cofg_ai: img.v(c(0x1100)),
        cofg_travel_forward: f(0x1110),
        cofg_travel_back: f(0x1114),
        preload_time: f(0xc5c),
        min_preload_time: f(0xc64),
        max_preload_time: f(0xc68),
        max_preload: f(0xc6c),
        clutch_torque_scale_time: f(0x1170),
        max_clutch_torque_scale: f(0x1174),
        max_clutch_torque_scale_speed: f(0x1178),
        aero_surface_area: f(0x117c),
        airborne_damping: f(0x1180),
        unroll_gain: f(0x1184),
        unroll_torque: f(0x1188),
        unyaw_gain: f(0x118c),
        unyaw_torque: f(0x1190),
        airborne_control_moment: img.v(c(0x11a0)),
        brake_gyro_torque: f(0x11b0),
        wings: (0..wings.max(0) as u32).map(|i| wing(wing_ptr + 0x20 * i)).collect(),
        tires: core::array::from_fn(|k| read_tire_keys(img, img.tires[k])),
        engine: Some(read_engine_keys(img)),
    }
}

fn read_input_keys(img: &Image) -> InputKeys {
    let f = |o: u32| img.f(img.o + o);
    InputKeys { max_analog_throttle: f(0x1144), digital_throttle_speed: f(0x1148), digital_side_slip_angle: f(0x114c) }
}

/// The car's loaded words: what [`Load::car`] sets, read off the image, the rest as built.
fn read_car(img: &Image, built: &Car) -> Car {
    let mut car = Car {
        state: img.at(0xc80),
        yaw_rate_limit: img.f(img.o + 0x124c),
        landing_sound: img.at(0x12b8) != 0,
        cofg: img.v(img.layout().body + 0x1a0),
        sort_rays: img.at(0xc4) != 0,
        ..built.clone()
    };
    for (t, &a) in car.tires.iter_mut().zip(&img.tires) {
        (t.mount, t.radius) = (img.v(a + MOUNT), img.f(a + 0x1e8));
    }
    car
}

/// The body's loaded words, its pose (as the ride sets it) taken from `built`.
fn read_body(img: &Image, built: &Body) -> Body {
    let b = seed::body(&|a| img.ee.u32_at(a), img.layout().body);
    Body {
        last_position: built.last_position,
        position: built.position,
        velocity: built.velocity,
        last_force: built.last_force,
        force: built.force,
        accel: built.accel,
        correction_peak: built.correction_peak,
        correction: built.correction,
        last_quat: built.last_quat,
        quat: built.quat,
        moment: built.moment,
        last_moment: built.last_moment,
        rates: built.rates,
        ang_accel: built.ang_accel,
        twist_peak: built.twist_peak,
        twist: built.twist,
        ..b
    }
}

/// MXVehicle's loaded words; the scene, bounds and what the ride has done since are `built`'s.
fn read_vehicle(img: &Image, built: &Vehicle) -> Vehicle {
    let c = |o: u32| img.o + o;
    let world = img.at(WORLD);
    Vehicle {
        wrecks_on: img.b(c(0x111c)),
        bounded: img.b(c(0x308)),
        clock: img.f(c(0xc84)),
        max_wreck_time: img.f(c(0xc88)),
        counting: img.b(c(0xc8c)),
        tipped_wreck: img.b(c(0x1118)),
        kind: img.at(0xbbc),
        wreck_damping: img.v(c(0xca0)),
        wreck_inertia: img.v(c(0xcb0)),
        inertia: img.v(c(0xcc0)),
        body_inertia: img.v(img.layout().body + 0x130),
        look: img.b(c(0x12b0)),
        record: img.at(RECORD) != 0,
        gravity: img.v(world + 0x40),
        drag: img.f(world + 4),
        flags: img.ee.u8_at(c(0x45)),
        ..built.clone()
    }
}

fn load(files: &Files, img: &Image, phy: &str, machine: MxvatvMachine) -> Load {
    let ini = files.ini(phy);
    let first = Load::read(&ini, machine, Vf::new(1.0, 1.0, 1.0, 1.0), img.points(), &files.curve);
    let b = img.layout().body;
    let body = core::array::from_fn(|k| img.ee.u32_at(b + 0x130 + 4 * k as u32));
    let half = load_box(&first.part, record_want(img), body).expect("a half box giving the record's inertia");
    Load::read(&ini, machine, half, img.points(), &files.curve)
}

fn diff<T: std::fmt::Debug + PartialEq>(name: &str, what: &str, got: &T, want: &T, bad: &mut Vec<String>) {
    if got != want {
        let (g, w) = (format!("{got:#?}"), format!("{want:#?}"));
        let lines: Vec<String> = g.lines().zip(w.lines()).filter(|(a, b)| a != b).map(|(a, b)| format!("    ours {a}\n    want {b}")).collect();
        bad.push(format!("{name} {what}:\n{}", lines.join("\n")));
    }
}

#[test]
fn each_car_from_its_files_is_its_spawn_image() {
    let Some(files) = Files::load() else { return };
    let mut bad = Vec::new();
    for (name, machine, phy, _, o) in CARS {
        let img = files.image(name, o);
        let l = load(&files, &img, phy, machine);
        diff(name, "keys", &l.keys(), &read_keys(&img), &mut bad);
        diff(name, "input keys", &l.input_keys(), &read_input_keys(&img), &mut bad);
        let car = l.car();
        diff(name, "car", &car, &read_car(&img, &car), &mut bad);
        let body = l.body();
        diff(name, "body", &body, &read_body(&img, &body), &mut bad);
        let vehicle = l.vehicle(img.f(img.at(WORLD) + 4));
        diff(name, "vehicle", &vehicle, &read_vehicle(&img, &vehicle), &mut bad);
        for (a, mask, v) in l.part.words(img.layout()) {
            let theirs = img.ee.u32_at(a);
            if theirs & mask != v & mask {
                bad.push(format!("{name}: part word {a:#x} ours {v:#010x} theirs {theirs:#010x} (mask {mask:#x})"));
            }
        }
    }
    assert!(bad.is_empty(), "{} differences:\n{}", bad.len(), bad.join("\n"));
}

/// The model's tire points at the origin: within 2e-3 ft of the image's, every lane (not 1:1: the
/// setup's frame is the car's placement then, which the image does not keep).
#[test]
fn the_models_tire_points_are_the_images_within_two_thousandths() {
    let Some(files) = Files::load() else { return };
    let origin = quad::spawn_frame(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 1.0));
    for (name, _, _, model, o) in CARS {
        let img = files.image(name, o);
        let got = atv_car::tire_points(&files.model(model), &origin).unwrap();
        let mat = game_sim::math::Mat4(core::array::from_fn(|r| img.v(o + MATRIX + 0x10 * r as u32)));
        let placed = atv_car::tire_points(&files.model(model), &mat).unwrap();
        for (k, want) in img.points().into_iter().enumerate() {
            for (frame, got) in [("origin", got[k]), ("the image's matrix", placed[k])] {
                for i in 0..4 {
                    let d = (got.0[i] - want.0[i]).abs();
                    assert!(d < 2e-3, "{name} tire {k} lane {i} at {frame}: {} against {}", got.0[i], want.0[i]);
                }
            }
        }
    }
}

/// The model's half box at the origin gives the record's inertia within 1%: the box is the model's
/// bounds, in a frame the image does not keep.
#[test]
fn the_models_box_is_the_records_within_a_percent() {
    let Some(files) = Files::load() else { return };
    let origin = quad::spawn_frame(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 1.0));
    for (name, machine, phy, model, o) in CARS {
        let img = files.image(name, o);
        let half = files.model(model).half_box(&origin);
        let l = Load::read(&files.ini(phy), machine, half, img.points(), &files.curve);
        let got = l.part.record.inertia.expect("the record's inertia");
        for (i, want) in record_want(&img).into_iter().enumerate() {
            let want = f32::from_bits(want);
            assert!(((got.0[i] - want) / want).abs() < 0.01, "{name} lane {i}: {} against {want}", got.0[i]);
        }
    }
}
