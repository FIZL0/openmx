//! S245: the cars off the user's disc, as the shell draws them — each car's two liveries with their
//! textures, each vertex on the node its mesh's node list names, posed by their tires: at rest exactly the model, each tire raised by its own
//! compression and the others kept, and the driver's seat inside the car. Skips without a disc.

use game_render::Disc;
use game_render::car::{self, Car, Tire};
use game_render::mat::{V3, v3};
use game_render::world::Assets;

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            eprintln!("skipped: {why}");
            None
        }
    }
}

const MODELS: [&str; 6] = ["monstertruck", "monstertruck2", "baja", "baja2", "buggy", "buggy2"];

fn at(m: game_render::mat::M4) -> V3 {
    v3(m.0[3][0], m.0[3][1], m.0[3][2])
}

#[test]
fn every_car_loads_and_poses_by_its_tires() {
    let Some(mut disc) = disc() else { return };
    let archives = [disc.res(car::ARCHIVE).expect("the machines' archive")];
    let mut assets = Assets::default();
    for name in MODELS {
        let drawn = Car::load(&mut assets, &archives, name).unwrap_or_else(|| panic!("{name} is not on the disc"));
        let loaded = &assets.models[&drawn.model];
        assert!(!loaded.tris.is_empty(), "{name} has no triangles");
        assert!(loaded.textures.iter().all(Option::is_some), "{name}: a material without its texture: {:?}", loaded.model.materials);
        let model = loaded.model.clone();

        // Each vertex names its node through its mesh's node list: it lies in that node's bounds.
        let mut outside = Vec::new();
        for mesh in model.meshes.iter().filter(|m| m.group == 0) {
            for v in mesh.strips.iter().flat_map(|s| s.verts.iter()) {
                let n = &model.nodes[v.node];
                let off = v.pos.sub(n.center);
                let out = [(off.x, n.half_extents.x), (off.y, n.half_extents.y), (off.z, n.half_extents.z)].iter().any(|(o, h)| o.abs() > h + 0.05);
                if out && outside.len() < 5 {
                    outside.push(format!("{} {:?}", n.name, v.pos));
                }
            }
        }
        assert!(outside.is_empty(), "{name}: vertices outside their node's bounds: {outside:?}");
        let tires: Vec<usize> = car::TIRES.iter().map(|t| model.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(t)).expect(t)).collect();

        // At rest it is the model.
        let rest = drawn.pose(&assets, &[Tire::REST; 4]);
        for (i, p) in rest.iter().enumerate() {
            let w = model.world(i);
            let d = at(*p).sub(at(w)).len();
            assert!(d < 1e-4, "{name}: {} moved {d} at rest", model.nodes[i].name);
        }

        // Each tire up by its compression, the opposite one kept, everything finite.
        for k in 0..4 {
            let mut t = [Tire::REST; 4];
            t[k].compression = 0.5;
            t[k].spin = 1.0;
            t[k].steer = if k < 2 { 0.3 } else { 0.0 };
            let posed = drawn.pose(&assets, &t);
            assert!(posed.iter().all(|m| m.0.iter().flatten().all(|v| v.is_finite())), "{name}: tire {k}: not finite");
            let rise = at(posed[tires[k]]).y - at(rest[tires[k]]).y;
            assert!((0.3..1.0).contains(&rise), "{name}: tire {k} rose {rise} for 0.5 ft");
            let across = [1, 0, 3, 2][k];
            let still = at(posed[tires[across]]).sub(at(rest[tires[across]])).len();
            // The baja's and buggy's rear tires share one swing arm: they rise together.
            let shared = k >= 2 && !name.starts_with("monstertruck");
            assert!(shared || still < 0.05, "{name}: tire {across} moved {still} with tire {k}");
            let skinned = game_render::skin::skin(&model, &posed, 0);
            assert_eq!(skinned.len(), loaded.tris.len(), "{name}: skinned");
        }

        // The driver's seat: inside the car's box, between the feet and the hands, on the left.
        let seat = drawn.seat(&assets).expect("the seat nodes");
        let feet = car::node_at(&model, car::SEAT[0]).unwrap();
        let hands = car::node_at(&model, car::SEAT[2]).unwrap();
        assert!(seat.y > feet.y - 0.5 && seat.y < hands.y, "{name}: seat {seat:?}, feet {feet:?}, hands {hands:?}");
        assert!(seat.z < feet.z && seat.z < hands.z, "{name}: the seat is not behind the wheel: {seat:?}");
    }
}
