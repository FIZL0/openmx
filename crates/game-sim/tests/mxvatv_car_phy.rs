//! MX vs. ATV's car files (MD39) through MX Unleashed's car reader. SLUS-21104's car loader (`atv_car_phy_load`,
//! research/mxvatv-pc.md "The cars") reads the same keys, defaults and order as MX
//! Unleashed's, so the reader takes the five files as they are; what it does not yet read are the
//! wreck keys MXVehicle's base loader adds (MD39b). The files come out of MXVSATV.PSR's
//! `Veh\vehicletype_{4..8}.res` into `context/mxvatv-ps2/md39/phy` (research/journal/2026-11-03-md39-mxvatv-machines);
//! without them every test here skips.

use game_sim::car::{Car, Kind};
use ps2_float::F;
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
