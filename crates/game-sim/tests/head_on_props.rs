//! **A static prop meets the bike's body rays, not a stand-in** (S152).
//!
//! Since S138 the riding bike's eleven body rays search every prop and plant the world hands over,
//! once a tick from where the last tick left the bike ([`rider_contact::scenery`]), and the wreck test
//! reads what they struck. Before S152 it also swept the port's head ray through the same props at the
//! frame's *end* (`ride::hits`), so a head that reached a prop wrecked a tick before the body rays,
//! which look from the tick's start, could have listed it, and a head ray both listed counted twice.
//!
//! The yard here is a flat floor with a ceiling over the rider's path at head height, above every ray
//! but the head's: the rider thrown at it still wrecks at reason 6, and on the tick whose search, from
//! the tick's start, first lists the head ray on it — never on the tick before.

use game_render::disc::Disc;
use game_sim::bike_col::{self, BikeCol};
use game_sim::collide::{Bundle, Record};
use game_sim::contact_search::Heightfield;
use game_sim::phy::Phy;
use game_sim::ride::Controls;
use game_sim::rider::{Rider, Tables};
use game_sim::trick::Set;
use game_sim::track::{Mesh, Node, Placement, Points, Triangle};
use game_sim::world::{Flat, Prop, Props, World};
use game_sim::{Sim, mover_contact, rider_contact};
use ps2_float::{F, Vf};
use ps2_oracle::skip;
use std::sync::Arc;

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);

/// The disc's `race125p.phy` and the bike's two collision files (`tests/rider_contact.rs`'s).
fn disc(tables: &Tables) -> Option<(Phy, BikeCol)> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let res = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
    let read = |name: String| res.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
    let col = BikeCol::load(&read(bike_col::rays_file(bike_col::MODEL)), &read(bike_col::shape_file(bike_col::MODEL)))
        .expect("the bike's collision files");
    let bikes: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("bike\\") && n.ends_with(".res")
        })
        .collect();
    let phy = bikes
        .iter()
        .find_map(|b| disc.res(b).ok()?.read_suffix("race125p.phy").ok())
        .expect("race125p.phy in a bike archive");
    Some((Phy::read_bytes(&phy, &tables.ride.curve), col))
}

/// A flat floor with props standing on it, handed over as the shell's world hands its own.
struct Yard {
    floor: Flat,
    props: Props,
}

impl World for Yard {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.floor.ground(at)
    }
    fn solid(&mut self, bundle: &Bundle, record: &mut Record) -> bool {
        self.props.sweep(bundle, record)
    }
    fn heightfield(&mut self) -> Option<Box<dyn Heightfield + '_>> {
        Some(Box::new(self.floor))
    }
    fn collision(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&Props>) {
        (Some(Box::new(self.floor)), Some(&self.props))
    }
}

/// A ceiling over the path from `z0` to `z1`, twenty feet wide, `y` up, facing down at the rider who
/// rides under it. The head ray runs almost straight up out of the hub (`air::HEAD`), so a ceiling is
/// what it crosses.
fn ceiling(y: f32, z0: f32, z1: f32) -> Prop {
    let v = |x: f32, z: f32| [F(x), F(y), F(z)];
    let mesh = Mesh {
        version: 0,
        surface: None,
        transform: [
            [F::ONE, F::ZERO, F::ZERO],
            [F::ZERO, F::ONE, F::ZERO],
            [F::ZERO, F::ZERO, F::ONE],
            [F::ZERO, F::ZERO, F::ZERO],
        ],
        vertices: Points::Wide(vec![v(-10.0, z0), v(10.0, z0), v(10.0, z1), v(-10.0, z1)]),
        normals: None,
        nodes: vec![Node {
            volume: F::ZERO,
            centre: [F::ZERO, F(y), F((z0 + z1) * 0.5)],
            half_extent: [F(10.0), F(0.5), F((z1 - z0) * 0.5)],
            children: [0x8000, 0x8001],
        }],
        triangles: vec![
            // Wound to face down, at the rays coming up from under it (`tests/ride.rs`'s wall).
            Triangle { distance: F::ZERO, normal: [F::ZERO; 3], vertices: [0, 1, 2] },
            Triangle { distance: F::ZERO, normal: [F::ZERO; 3], vertices: [0, 2, 3] },
        ],
    };
    Prop::place(mesh, &Placement::default())
}

/// The ceiling's height: halfway between the highest end of every ray but the head's and the head's
/// own end, so only the head ray reaches it.
fn between(col: &BikeCol) -> f32 {
    let mut ends: Vec<f32> = col.rays.segments.iter().map(|s| s.to.y()).collect();
    ends.sort_by(f32::total_cmp);
    let (head, next) = (ends[ends.len() - 1], ends[ends.len() - 2]);
    assert!(next + 0.5 < head, "the head ray reaches highest by a clear margin: {ends:?}");
    (next + head) * 0.5
}

/// The rider on the disc's bike and body rays at the origin facing +z, thrown at `speed` along it.
fn thrown(tables: &Arc<Tables>, phy: &Phy, col: &Arc<BikeCol>, speed: f32) -> Sim {
    let mounts = game_sim::stock::mounts(phy);
    let mut r = Rider::new(phy, mounts, Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    r.carries_bike_col(Arc::clone(col));
    let mut sim = Sim::with_rider(0x5152, Arc::clone(tables), phy.clone(), r);
    sim.rider.ride.chassis.vel = Vf::new(0.0, 0.0, speed, 0.0);
    sim
}

#[test]
fn a_head_high_ceiling_wrecks_on_the_tick_the_body_rays_list_it() {
    let tables = Arc::new(Tables::build());
    let Some((phy, col)) = disc(&tables) else { return };
    let col = Arc::new(col);
    let y = between(&col);
    let head_reach = F(col.rays.segments.iter().map(|s| s.to.y()).fold(f32::MIN, f32::max));
    let mut compared = 0;
    for speed in [20.0, 35.0, 50.0] {
        let mut sim = thrown(&tables, &phy, &col, speed);
        let mut yard = Yard { floor: Flat::default(), props: Props { props: vec![ceiling(y, 15.0, 30.0)] } };
        let (mut listed, mut wrecked) = (None, None);
        for tick in 0..90 {
            if sim.rider.state.wreck.is_none() && listed.is_none() {
                let at = sim.rider.ride.transform();
                let found = rider_contact::scenery(&col, &at, &mut yard, DT).expect("the yard hands its props over").0;
                if mover_contact::wreck_contacts(&found).iter().any(|c| c.reach == head_reach) {
                    listed = Some(tick);
                }
            }
            sim.step(DT, &Controls::default(), &mut yard);
            if let Some(w) = sim.rider.state.wreck {
                wrecked = Some((tick, w.0));
                break;
            }
        }
        println!("thrown at {speed}: the body rays list the head on tick {listed:?}, the rider wrecks {wrecked:?}");
        let listed = listed.unwrap_or_else(|| panic!("at {speed}: the head ray never reached the ceiling"));
        let (tick, reason) = wrecked.unwrap_or_else(|| panic!("at {speed}: a head into a ceiling never wrecked"));
        assert_eq!(reason, 6, "at {speed}: a head on scenery is reason 6");
        assert_eq!(tick, listed, "at {speed}: the wreck is the body rays' tick, not the frame-end head sweep's");
        compared += 1;
    }
    assert_eq!(compared, 3);
}
