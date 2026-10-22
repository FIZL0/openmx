//! S89: the port rides free06's railway bed on its wheels, not on its belly.
//!
//! The bed is a prop (`Free06_RR_COL_*`, see `dallas_rails.rs`), so the wheels meet it through the
//! ride's collision bundle as swept contacts. The bundle's straight-down probe from the centre of
//! mass used to reach deeper than either wheel's suspension line: it found the bed first and held the
//! bike up where no wheel touched it — no tire force, gas did nothing, and the bike slid sideways off.

use game_sim::archive::{Entry, Psr, Res};
use game_sim::phy::Phy;
use game_sim::ride::{Controls, Ride, Tables};
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Ground, Place, Props};
use ps2_float::{F, Vf};
use ps2_oracle::{Kit, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

fn member(f: &mut File, psr: &Psr, name: &str) -> Vec<u8> {
    let e = psr.find(name).unwrap();
    f.seek(SeekFrom::Start(e.at)).unwrap();
    let mut b = vec![0u8; e.size as usize];
    f.read_exact(&mut b).unwrap();
    b
}

fn by_leaf<'a>(res: &'a Res, leaf: &str) -> Option<&'a Entry> {
    let want = leaf.to_ascii_lowercase();
    res.entries.iter().find(|e| {
        let n = e.name.to_ascii_lowercase();
        n == want || n.rsplit('\\').next() == Some(want.as_str())
    })
}

/// What a ride along the bed did: the forward speed it reached, and the frames past the first second
/// where a wheel had no contact, where the body probe was holding the bike up, and the worst
/// sideways speed.
struct Rode {
    speed: f32,
    wheel_off: usize,
    belly: usize,
    side: f32,
}

#[test]
fn the_wheels_carry_the_bike_along_the_railway_bed() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => {
            skip(&w);
            return;
        }
    };
    let Ok(mut f) = File::open(kit.ctx("iso").join("MX.PSR")) else {
        skip("no disc container");
        return;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).unwrap();
    let psr = Psr::parse(&front).unwrap();
    let file = member(&mut f, &psr, "Tracks\\free06.res");
    let res = Res::parse(&file).unwrap();
    let scene = Scene::parse(&res.read(&file, by_leaf(&res, "tracks\\free06\\free06.scn").unwrap()).unwrap()).unwrap();
    let trn = res.read(&file, by_leaf(&res, "tracks\\free06\\free06.ps2.trn").unwrap()).unwrap();
    let terrain = Terrain::parse(&trn).unwrap();
    let file2 = member(&mut f, &psr, "Tracks\\free06_2.res");
    let res2 = Res::parse(&file2).unwrap();
    let tables = Tables::build();

    // The bike the human rode: race125p.phy, on 125_faisst's wheel mounts.
    let (mut phy, mut mounts) = (None, None);
    let names: Vec<String> = psr.entries.iter().map(|e| e.name.clone()).filter(|n| {
        let n = n.to_ascii_lowercase();
        n.starts_with("bike\\") && n.ends_with(".res")
    }).collect();
    for a in names {
        let b = member(&mut f, &psr, &a);
        let r = Res::parse(&b).unwrap();
        if phy.is_none() && let Some(e) = by_leaf(&r, "race125p.phy") {
            phy = Some(Phy::read_bytes(&r.read(&b, e).unwrap(), &tables.curve));
        }
        if mounts.is_none() && a.to_ascii_lowercase().contains("125_faisst") && let Some(e) = by_leaf(&r, "bike.psg") {
            let model = game_render::psg::Model::parse(&r.read(&b, e).unwrap()).unwrap();
            let node = |name: &str| {
                let i = model.nodes.iter().position(|n| n.name == name)?;
                let mut root = i;
                while let Some(up) = model.nodes[root].parent {
                    root = up;
                }
                Some(Vf(model.place_in(i, root)))
            };
            mounts = Some((node("TireFront").unwrap(), node("TireRear").unwrap()));
        }
    }
    let (phy, mounts) = (phy.unwrap(), mounts.unwrap());

    let ride = |at: [f32; 3], props: bool| {
        let props = if props {
            Props::load(&scene, |w| {
                by_leaf(&res, w).map(|e| res.read(&file, e).unwrap())
                    .or_else(|| by_leaf(&res2, w).map(|e| res2.read(&file2, e).unwrap()))
                    .ok_or_else(|| format!("{w}: not in free06"))
            }).0
        } else {
            Props::default()
        };
        let mut world = Place { ground: Ground::new(&terrain, &scene.terrain), props };
        // Dropped onto the bed facing along it (+z), gas from the second second.
        let mut ride = Ride::new(&phy, mounts, Vf::new(at[0], at[1], at[2], 1.0), Vf::new(0.0, 0.0, 1.0, 0.0));
        let mut rode = Rode { speed: 0.0, wheel_off: 0, belly: 0, side: 0.0 };
        for k in 0..135 {
            let c = Controls { gas: F(if k > 30 { 1.0 } else { 0.0 }), ..Controls::default() };
            ride.frame(&phy, &tables, &c, F(1.0 / 30.0), &mut world);
            let m = ride.transform();
            let v = ride.chassis.vel;
            let along = |a: Vf| v.x() * a.x() + v.y() * a.y() + v.z() * a.z();
            rode.speed = along(m.0[2]);
            if k >= 30 {
                for w in [&ride.front, &ride.rear] {
                    rode.wheel_off += usize::from(w.w.ray_hit == 0 && w.w.swept_contact == 0);
                }
                rode.belly += usize::from(ride.touching);
                rode.side = rode.side.max(along(m.0[0]).abs());
            }
        }
        rode
    };

    // The middle of the bed's flat top, and the bottom of its shoulder.
    for (name, at) in [("top", [4591.4, 137.5, 3860.0]), ("shoulder", [4582.0, 137.0, 3860.0])] {
        let bed = ride(at, true);
        let bare = ride(at, false);
        assert_eq!(bed.belly, 0, "{name}: the bike rested on its belly");
        assert!(bed.wheel_off <= 6, "{name}: a wheel was off the bed {} times", bed.wheel_off);
        assert!(bed.side < 1.0, "{name}: slid sideways at {}", bed.side);
        assert!(bed.speed > 0.95 * bare.speed, "{name}: {} on the bed, {} on the terrain", bed.speed, bare.speed);
    }
}
