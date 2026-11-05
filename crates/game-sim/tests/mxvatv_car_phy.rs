//! MX vs. ATV's car files (MD39) through MX Unleashed's car reader. SLUS-21104's car loader (`atv_car_phy_load`,
//! research/mxvatv-pc.md "The cars") reads the same keys, defaults and order as MX
//! Unleashed's, so the reader takes the five files as they are; `Car::read_mxvatv` adds the wreck and
//! camera keys MXVehicle's loaders read around it (MD39b). The files come out of MXVSATV.PSR's
//! `Veh\vehicletype_{4..8}.res` into `context/mxvatv-ps2/md39/phy` (research/journal/2026-11-03-md39-mxvatv-machines);
//! without them every test here skips.

use game_sim::car::{Car, Kind, MxvatvMachine};
use game_sim::phy::Ini;
use ps2_float::{F, Vf};
use ps2_oracle::{Kit, skip};

/// Each file and the reader's kind for it: the PC factory builds the buggy, the baja and the
/// sandrail with MXVehicleBuggy, the golf cart with MXVehicleGolfCart (the buggy's loader plus four
/// rider boxes) and the truck with MXVehicleMonsterTruck (the only one with RearTireSteerScale).
const FILES: [(&str, Kind); 5] = [
    ("buggy.phy", Kind::Buggy),
    ("baja.phy", Kind::Baja),
    ("sandrail.phy", Kind::Buggy),
    ("golfcart.phy", Kind::Buggy),
    ("monstertruck.phy", Kind::MonsterTruck),
];

fn cars() -> Option<Vec<(&'static str, Car)>> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let dir = kit.ctx("mxvatv-ps2/md39/phy");
    if !dir.exists() {
        skip(&format!(
            "{} is not there: it is MX vs. ATV's Veh\\<car>.phy out of MXVSATV.PSR's vehicletype_4..8.res",
            dir.display()
        ));
        return None;
    }
    Some(
        FILES
            .iter()
            .map(|&(name, kind)| {
                let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
                (name, Car::read_bytes(&bytes, kind))
            })
            .collect(),
    )
}

/// The five read, with MX vs. ATV's retuning showing through: the buggy's tires are 1.1 ft (MX
/// Unleashed's are 1.2) and the truck's rear tires steer by its own scale.
#[test]
fn mxvatvs_cars_read() {
    let Some(cars) = cars() else { return };
    for (name, car) in &cars {
        for t in &car.tires {
            assert!(t.radius > F::ZERO, "{name}: a tire's radius");
            assert_eq!(t.number_of_surfaces as usize, t.surfaces.len(), "{name}: surfaces");
        }
        assert!(!car.engine.rpm.is_empty(), "{name}: the RPM table");
        assert_eq!(
            car.animation.rear_tire_steer_scale.is_some(),
            *name == "monstertruck.phy",
            "{name}: RearTireSteerScale"
        );
    }
    let buggy = &cars[0].1;
    // The game's own strtod puts "1.1" a bit under f32's nearest.
    let r = buggy.tires[0].radius;
    assert!(r > F::from(1.09f32) && r < F::from(1.11f32), "buggy.phy TireRadius {r:?}");
}

/// The five through SLUS-21104's own reading (MD39b): MXVehicle's `[Wrecks]` keys and the
/// machine's `[Camera]`, everything else as MX Unleashed's reader takes it. Every file writes the
/// same wreck keys, three lanes each, so the quadwords keep their default's `w`; each has its own
/// `FPCameraOffset` and none an `FCamFileName`.
#[test]
fn mxvatvs_cars_read_their_wreck_and_camera_keys() {
    let Some(cars) = cars() else { return };
    let dir = Kit::load().unwrap().ctx("mxvatv-ps2/md39/phy");
    let machines = [
        MxvatvMachine::Buggy,
        MxvatvMachine::Baja,
        MxvatvMachine::Sandrail,
        MxvatvMachine::GolfCart,
        MxvatvMachine::MonsterTruck,
    ];
    let cameras = ["0.0, 4.5, 0.86", "0.0, 6.8, 2.5", "0.0, 4.6, 1.6", "-0.5, 6.2, 0.05", "-1.0, 12.4, 2.2"];
    // The game's own strtod, which is not always f32's nearest ("0.999" reads a bit under it).
    let v = |text: &str| Ini::parse(&format!("[a]\nk = {text}\n")).section("a").vector("k", Vf::new(0.0, 0.0, 0.0, 1.0));
    for (((name, car), machine), cam) in cars.iter().zip(machines).zip(cameras) {
        assert!(machine.phy().ends_with(name), "{name}: {}", machine.phy());
        let bytes = std::fs::read(dir.join(name)).unwrap();
        let m = Car::read_mxvatv_bytes(&bytes, machine);
        assert_eq!(&m.car, car, "{name}: the car's own keys as MX Unleashed's reader takes them");
        assert_eq!(m.car.max_wreck_time, F(5.0), "{name}: MaxWreckTime");
        assert_eq!(m.wrecks.wreck_gravity, F(3.0), "{name}: WreckGravity");
        assert_eq!(m.wrecks.wreck_damping, v("0.999, 0.999, 0.999"), "{name}: WreckDamping");
        assert_eq!(m.wrecks.wreck_inertia, v("0.2, 0.2, 0.2"), "{name}: WreckInertia");
        assert_eq!(m.camera.fp_camera_offset, v(cam), "{name}: FPCameraOffset");
        assert_eq!(m.camera.left_shoulder_offset, Vf::new(0.0, 0.0, 0.0, 1.0), "{name}");
        assert_eq!(m.camera.right_shoulder_offset, Vf::new(0.0, 0.0, 0.0, 1.0), "{name}");
        assert_eq!(m.camera.camera_file, machine.default_camera_file(), "{name}: FCamFileName");
    }
}

/// The two loaders' defaults and the camera name's buffer, on a file that writes none of their keys
/// and on one that writes them all. Needs no disc.
#[test]
fn the_wreck_and_camera_keys_default_as_slus_21104s() {
    let bare = Car::read_mxvatv_bytes(b"[Mass Properties]\nMass = 100\n", MxvatvMachine::GolfCart);
    assert_eq!(bare.car.max_wreck_time, F(5.0));
    assert_eq!(bare.wrecks.wreck_gravity, F(3.0));
    assert_eq!(bare.wrecks.wreck_damping, Vf::new(1.0, 1.0, 1.0, 1.0));
    assert_eq!(bare.wrecks.wreck_inertia, Vf::new(0.0, 0.0, 0.0, 1.0));
    assert_eq!(bare.camera.fp_camera_offset.0.map(f32::to_bits), [0, 0x40a3_3333, 0xbf19_999a, 0x3f80_0000]);
    assert_eq!(bare.camera.left_shoulder_offset, Vf::new(0.0, 0.0, 0.0, 1.0));
    assert_eq!(bare.camera.camera_file, "FC_GolfCartDefault.pb");
    for m in MxvatvMachine::ALL {
        let k = m.kind_word();
        assert!(m.phy().contains("Veh\\"), "{k}");
        assert!(m.default_camera_file().starts_with("FC_") && m.default_camera_file().ends_with("Default.pb"));
    }

    let text = b"[Wrecks]\nMaxWreckTime = 2.5\nWreckGravity = 1\nWreckDamping = 0.5, 0.5, 0.5, 0.25\n\
        [Camera]\nLeft1stPerShoulderOffset = 1, 2, 3\nRight1stPerShoulderOffset = -1, 2, 3\n\
        FPCameraOffset = 0, 1, 2\nFCamFileName = FC_AVeryLongCameraPresetsName.pb\n";
    let full = Car::read_mxvatv_bytes(text, MxvatvMachine::Sandrail);
    assert_eq!(full.car.max_wreck_time, F(2.5));
    assert_eq!(full.wrecks.wreck_gravity, F::ONE);
    assert_eq!(full.wrecks.wreck_damping, Vf::new(0.5, 0.5, 0.5, 0.25));
    assert_eq!(full.camera.left_shoulder_offset, Vf::new(1.0, 2.0, 3.0, 1.0));
    assert_eq!(full.camera.right_shoulder_offset, Vf::new(-1.0, 2.0, 3.0, 1.0));
    assert_eq!(full.camera.fp_camera_offset, Vf::new(0.0, 1.0, 2.0, 1.0));
    // The getter prints into a 0x20-byte buffer: 31 bytes and the terminator.
    assert_eq!(full.camera.camera_file, "FC_AVeryLongCameraPresetsName.p");
}
