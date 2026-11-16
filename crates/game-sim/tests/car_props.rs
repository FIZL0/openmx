//! S335: the buggy and the monster truck among the knockable props. The car's collision object
//! searches the props' meshes as the plane's does (`Machine::step_props`), its resolve pushes the
//! props it strikes, and the props' own step answers with the car as its striker, as the shell's
//! `CarRide::step_traffic` runs them. free06's balls are the props; the terrain is free06's.
//!
//! Not proven against the original: no capture of a car striking a prop exists (see S335's journal).

use game_render::disc::Disc;
use game_sim::air::AcosTable;
use game_sim::archive::{Psr, Res};
use game_sim::camera::CurveTable;
use game_sim::car::{Car, CarProps, Kind, Machine};
use game_sim::flight_search::Rays;
use game_sim::phy::Ini;
use game_sim::prop_set::PropSet;
use game_sim::track::{self, Scene, Terrain};
use game_sim::world::{Ground, World};
use ps2_float::{F, Vf};
use ps2_oracle::{Kit, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// `name` out of `machine.res` in the extracted `MX.PSR`.
fn machine_file(kit: &Kit, name: &str) -> Option<Vec<u8>> {
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut f) = File::open(&at) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", at.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("machine.res").expect("machine.res is in MX.PSR");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    let res = Res::parse(&bytes).expect("machine.res");
    let e = res.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).unwrap_or_else(|| panic!("no {name} in machine.res"));
    Some(res.read(&bytes, e).expect("its bytes"))
}

/// free06's knockable props as the ride loads them, and its terrain, off the disc.
fn free06() -> Option<(PropSet, Ground<'static>)> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let member = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("Tracks\\free06.res")).expect("free06.res").name.clone();
    let res = disc.res(&member).expect("free06's archive");
    let res = &res;
    let toy = Ini::parse_bytes(&res.read_suffix("free06.toy").expect("free06.toy"));
    let scene = Scene::parse(&res.read_suffix("free06.scn").expect("free06.scn")).expect("free06.scn parses");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let bytes: &'static [u8] = Box::leak(res.read_suffix(&leaf).expect("free06's terrain").into_boxed_slice());
    let terrain: &'static Terrain = Box::leak(Box::new(Terrain::parse(bytes).expect("free06's terrain parses")));
    let ground = Ground::new(terrain, &scene.terrain);
    let area = ground.area();
    let (set, said) =
        PropSet::load(&toy, "Tracks\\Free06\\", &scene.models, &mut |p| res.read(p).ok(), &mut |p| res.read_suffix(p).ok(), area);
    assert!(said.is_empty(), "free06 loads whole: {said:?}");
    Some((set, ground))
}

/// **The buggy and the truck knock a ball** (S335): each car, set down 40 ft short of free06's
/// first ball and driven at it on full gas, strikes it with its body: the ball wakes and is pushed
/// along the car's way, and the car does not pass through it.
#[test]
fn a_car_knocks_a_ball_along_its_way() {
    let Some(kit) = Kit::load().map_err(|w| skip(&w)).ok() else { return };
    let (acos, curve) = (AcosTable::build(), CurveTable::build());
    for (kind, psg) in [(Kind::Buggy, "buggy\\anim\\buggy.psg"), (Kind::MonsterTruck, "monstertruck\\anim\\monstertruck.psg")] {
        let files = (machine_file(&kit, kind.phy()), machine_file(&kit, psg), machine_file(&kit, kind.rays()), machine_file(&kit, kind.hull()));
        let (Some(phy), Some(psg), Some(rays), Some(hull)) = files else { return };
        let Some((set, mut ground)) = free06() else { return };
        let car = Car::read_bytes(&phy, kind);
        let model = game_sim::car::CarModel::read(&psg).unwrap();
        let rays = track::Rays::parse(&rays).unwrap();
        let hull = track::Mesh::parse(&hull).unwrap();
        let mut sim = game_sim::Sim::new(0x5335);
        sim.traffic.props = set;
        let (c, o) = {
            let set = &sim.traffic.props;
            (0..set.collections.len())
                .flat_map(|c| (0..set.collections[c].objects.len()).map(move |o| (c, o)))
                .find(|&(c, o)| set.collections[c].objects[o].collision.is_some())
                .expect("free06 has a ball")
        };
        let ball = sim.traffic.props.collections[c].bones[o].body.position;
        let way = Vf::new(1.0, 0.0, 0.0, 0.0);
        let (spot, _) = ground.ground(Vf::new(ball.x() - 40.0, 0.0, ball.z(), 1.0));
        let start = Vf::new(spot.x(), spot.y() + 4.0, spot.z(), 1.0);
        let mut m = Machine::new(&car, &model, start, way, Vf::new(0.0, 1.0, 0.0, 0.0));
        m.drive(true);
        let mut col = game_sim::car::collision_of_model(&car, &model, &m.body, Rays::of(&rays));
        col.refresh(&m.body, &mut ground);
        col.record.search_ground = m.searches_terrain(&mut ground);
        let dt = F(1.0 / 30.0);
        let gas = |cmd| if cmd == game_sim::bindings::Command::GAS { F::ONE } else { F::ZERO };
        let mut woke = None;
        let mut nearest = f32::MAX;
        for frame in 0..150 {
            let movers = sim.traffic.frame_held(dt, &mut ground);
            let mut set = std::mem::take(&mut sim.traffic.props);
            m.controls.input(&car, (F::ZERO, F::ZERO), F::ZERO, m.ground_speed, m.side_slip, gas);
            let props = CarProps { set: &mut set, me: 0, hull: Some(&hull) };
            m.step_props(&car, &mut ground, &mut col, None, Some(props), dt, &acos, &curve, &mut |_| F::ZERO);
            let mut weighed = game_sim::contact::Weighed { body: &mut m.body, mass: col.mass };
            let mut strikers = [game_sim::contact::Other { collider: &mut col.collider, side: game_sim::contact::Side::Body(&mut weighed) }];
            set.step_with(dt, &movers, &mut strikers);
            sim.traffic.props = set;
            sim.traffic.frame_late(dt, &mut ground);
            let b = sim.traffic.props.collections[c].bones[o].body;
            if woke.is_none() && b.enabled {
                woke = Some(frame);
            }
            let d = b.position.sub(m.body.position);
            nearest = nearest.min((d.x() * d.x() + d.z() * d.z()).sqrt());
        }
        let b = sim.traffic.props.collections[c].bones[o].body;
        let moved = b.position.x() - ball.x();
        eprintln!("{kind:?}: ball woke on frame {woke:?}, moved {moved} ft along, nearest {nearest} ft; car at {:?}", m.body.position.0);
        assert!(woke.is_some(), "{kind:?}: the car never struck the ball");
        assert!(moved > 1.0, "{kind:?}: the ball was not pushed along the car's way ({moved} ft)");
        assert!(nearest > 1.0, "{kind:?}: the car's centre met the ball's ({nearest} ft): it drove through it");
    }
}
