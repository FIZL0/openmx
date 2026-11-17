//! S245: the cars off the user's disc, as the shell draws them — each car's two liveries with their
//! textures, each vertex on the node its mesh's node list names, posed by their tires: at rest exactly the model (the baja and the buggy to within the turns the original drops, S264), each tire raised by its own
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

        // At rest it is the model; the baja's and the buggy's rig sets its turns afresh as the
        // original's does (S264), dropping the model's own: the baja's left hub's 11°, and its
        // unturned swing arm pitched 7.4° to look at its rear tires, which then sit 0.68 ft low.
        let rest = drawn.pose(&assets, &[Tire::REST; 4]);
        let near = if car::has_buggy_rig(&model) { 0.7 } else { 1e-4 };
        assert_eq!(car::has_buggy_rig(&model), !name.starts_with("monstertruck"), "{name}");
        for (i, p) in rest.iter().enumerate() {
            let w = model.world(i);
            let d = at(*p).sub(at(w)).len();
            assert!(d < near, "{name}: {} moved {d} at rest", model.nodes[i].name);
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

/// S264: the buggy's pose against the original's own node matrices, in C34's switched image of the
/// driven buggy on free06 (`context/states/c34-buggy-switched.ee.bin`, the car at [`BUGGY`]). The four tires' spin, steer and compression are read off the car's tire
/// objects and posed by the port; every node of the original's tree is brought into the car's own
/// space (its root's world undone) and compared with the port's node of the same name.
mod against_the_original {
    use super::*;
    use game_render::mat::M4;
    use ps2_oracle::{Ee, Kit, skip};

    /// The driven buggy's car object in the image, where S149a's captures found it.
    const BUGGY: u32 = 0x0060_5270;
    /// The car's root node, its four tires (front left, front right, rear left, rear right), and a
    /// tire's spin, steer and compression.
    const ROOT: u32 = 0x18c;
    const TIRE_OBJECTS: [u32; 4] = [0x548, 0x54c, 0x550, 0x554];
    const TIRE_SPIN: u32 = 0x1a0;
    const TIRE_STEER: u32 = 0x260;
    const TIRE_COMPRESSION: u32 = 0xdc;
    /// A scene node: its local and world matrices, first child, next sibling, name.
    const NODE_LOCAL: u32 = 0x40;
    const NODE_WORLD: u32 = 0x80;
    const NODE_CHILD: u32 = 0x104;
    const NODE_NEXT: u32 = 0x108;
    const NODE_NAME: u32 = 0x120;

    pub(super) fn mat(ee: &Ee, a: u32) -> M4 {
        M4(std::array::from_fn(|r| std::array::from_fn(|c| ee.f32_at(a + 16 * r as u32 + 4 * c as u32))))
    }

    fn name(ee: &Ee, a: u32) -> String {
        let mut s = String::new();
        for i in 0..64 {
            let b = ee.u8_at(a + i);
            if b == 0 {
                break;
            }
            s.push(b as char);
        }
        s
    }

    /// Every node under `first` and its siblings, with its name.
    fn nodes(ee: &Ee, first: u32, out: &mut Vec<(String, u32)>) {
        let mut n = first;
        while n != 0 && out.len() < 256 {
            out.push((name(ee, ee.u32_at(n + NODE_NAME)), n));
            nodes(ee, ee.u32_at(n + NODE_CHILD), out);
            n = ee.u32_at(n + NODE_NEXT);
        }
    }

    /// `m` (a world matrix) in the space `root` (a rigid world matrix) places.
    pub(super) fn into_root(root: M4, m: M4) -> M4 {
        let row = |a: [f32; 4]| v3(a[0], a[1], a[2]);
        let r = [row(root.0[0]), row(root.0[1]), row(root.0[2])];
        let o = row(root.0[3]);
        M4(std::array::from_fn(|i| {
            let v = if i == 3 { row(m.0[3]).sub(o) } else { row(m.0[i]) };
            [v.dot(r[0]), v.dot(r[1]), v.dot(r[2]), if i == 3 { 1.0 } else { 0.0 }]
        }))
    }

    pub(super) fn image() -> Option<(Ee, u32)> {
        let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
        let path = kit.ctx("states").join("c34-buggy-switched.ee.bin");
        if !path.exists() {
            skip(&format!("{} is not there: C34's switched image of the driven buggy", path.display()));
            return None;
        }
        Some((Ee::from_ram(&path).map_err(|w| skip(&w)).ok()?, BUGGY))
    }

    /// The tire's `MaxCompression` and `MaxTireScale`, as the car's tire object holds them.
    const TIRE_MAX_COMPRESSION: u32 = 0x3c;
    const TIRE_MAX_SCALE: u32 = 0x60;
    /// A node's parent.
    const NODE_PARENT: u32 = 0x100;

    /// The largest difference between two matrices' turns, and between their places.
    fn off(a: M4, b: M4) -> (f32, f32) {
        let turn = (0..3).flat_map(|r| (0..3).map(move |c| (r, c))).map(|(r, c)| (a.0[r][c] - b.0[r][c]).abs()).fold(0.0f32, f32::max);
        (turn, at(a).sub(at(b)).len())
    }

    #[test]
    fn the_buggys_pose_is_the_originals() {
        let Some((ee, car)) = image() else { return };
        let Some(mut disc) = disc() else { return };
        let archives = [disc.res(car::ARCHIVE).expect("the machines' archive")];
        let mut assets = Assets::default();
        let drawn = Car::load(&mut assets, &archives, "buggy").expect("the buggy");
        let model = assets.models[&drawn.model].model.clone();
        assert!(car::has_buggy_rig(&model), "the buggy lacks its rig's nodes");

        let tires: [Tire; 4] = std::array::from_fn(|k| {
            let t = ee.u32_at(car + TIRE_OBJECTS[k]);
            let compression = ee.f32_at(t + TIRE_COMPRESSION);
            let scale = car::squash(compression, ee.f32_at(t + TIRE_MAX_COMPRESSION), ee.f32_at(t + TIRE_MAX_SCALE));
            Tire { spin: ee.f32_at(t + TIRE_SPIN), steer: ee.f32_at(t + TIRE_STEER), compression, scale }
        });
        // A ridden frame: every spring is working.
        assert!(tires.iter().all(|t| t.compression > 0.1), "{tires:?}");
        let posed = drawn.pose(&assets, &tires);
        let root = ee.u32_at(car + ROOT);
        assert_eq!(name(&ee, ee.u32_at(root + NODE_NAME)), "Buggy");
        let root_world = mat(&ee, root + NODE_WORLD);
        let in_car = |node: u32| into_root(root_world, mat(&ee, node + NODE_WORLD));
        let mut theirs = Vec::new();
        nodes(&ee, ee.u32_at(root + NODE_CHILD), &mut theirs);
        assert_eq!(theirs.len() + 1, model.nodes.len(), "the original's tree and the model's differ in size");

        let (mut wrong, mut moved) = (Vec::new(), 0);
        for (n, a) in &theirs {
            let i = model.nodes.iter().position(|m| m.name.eq_ignore_ascii_case(n)).unwrap_or_else(|| panic!("{n} is not in the model"));
            let mut want = in_car(*a);
            // The marks under a front hub keep the world the original measured them in, before the
            // hub's last turn about z: nothing writes them again. Their local on the hub unturned
            // (steer 0 in this frame) on its upper arm is that world; nothing is drawn on them.
            if n.ends_with("BottomHubNODE") || n.ends_with("TopSwingarmNODE") {
                assert!(tires[0].steer == 0.0 && tires[1].steer == 0.0, "the image is steered: {tires:?}");
                let hub = ee.u32_at(a + NODE_PARENT);
                let mut unturned = M4::IDENTITY;
                unturned.0[3] = mat(&ee, hub + NODE_LOCAL).0[3];
                let stale = model.nodes[i].local.mul(unturned).mul(in_car(ee.u32_at(hub + NODE_PARENT)));
                let (turn, place) = off(stale, want);
                assert!(turn < 2e-3 && place < 3e-3, "{n}: not the world before the hub's turn: {turn} {place}");
                let drawn_on = model.meshes.iter().flat_map(|m| m.strips.iter().flat_map(|s| s.verts.iter())).any(|v| v.node == i);
                assert!(!drawn_on, "{n} carries vertices");
                want = model.nodes[i].local.mul(in_car(hub));
            }
            let (turn, place) = off(posed[i], want);
            // The original's worlds are floats a frame of driving from the car's own; the fixed
            // nodes are as far off.
            if turn > 2e-3 || place > 3e-3 {
                wrong.push(format!("{n}: turn {turn}, place {place}"));
            }
            if off(model.world(i), want).1 > 0.2 {
                moved += 1;
            }
        }
        assert!(wrong.is_empty(), "nodes not as the original posed them:\n{}", wrong.join("\n"));
        assert!(moved >= 16, "only {moved} nodes moved from the model: the image is hardly posed");
    }
}
