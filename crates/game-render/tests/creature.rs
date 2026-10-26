//! A track's birds and sequence-animated toys (V44), read off the disc and stepped by ticks.

use game_render::creature::{self, Creatures, FLY, IDLE, TAKE_OFF};
use game_render::disc::{Disc, Res};
use game_render::mat::{V3, v3};

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            println!("SKIPPED: {why}");
            None
        }
    }
}

/// A track's archives as the shell opens them for creatures: the track's own, then its `_2`.
fn archives(disc: &mut Disc, name: &str) -> Vec<Res> {
    let mut out = Vec::new();
    for want in [format!("\\{name}.res"), format!("\\{name}_2.res")] {
        let member = disc.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with(&want)).map(|e| e.name.clone());
        if let Some(m) = member {
            out.push(disc.res(&m).expect("the archive"));
        }
    }
    assert!(!out.is_empty(), "no {name}.res on the disc");
    out
}

const TICK: f32 = 1.0 / 30.0;

#[test]
fn a_vue_is_its_transforms_turned_y_up() {
    let text = "FRAME 0\ntransform \"birds\" 1 0 0 0 1 0 0.1 0.2 0.9 10 20 30\nFRAME 1\njunk\n";
    let nodes = creature::vue(text);
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].pos, v3(10.0, 30.0, 20.0));
    assert_eq!(nodes[0].up, v3(0.1, 0.9, 0.2));
}

#[test]
fn flock_offsets_ring_out_by_the_step() {
    let o = creature::flock_offset(10, 5);
    assert_eq!(
        o,
        vec![(0.0, 0.0), (0.0, 5.0), (0.0, -5.0), (5.0, 0.0), (-5.0, 0.0), (5.0, 5.0), (-5.0, 5.0), (5.0, -5.0), (-5.0, -5.0), (0.0, 10.0)]
    );
}

#[test]
fn free06_flies_fifty_crows_round_its_loop() {
    let Some(mut disc) = disc() else { return };
    let arch = archives(&mut disc, "free06");
    let c = Creatures::load(&arch, "free06");
    println!("skipped: {:?}", c.skipped);
    assert_eq!(c.birds.len(), 50, "Flock01's NumFlyingBirds");
    assert_eq!(c.models(), vec!["crow"]);
    assert!(c.birds.iter().all(|b| b.flying && b.playing.motion == Some(FLY)));
    let body = &c.bodies[0];
    assert!(body.motions.iter().all(|m| m.clip.is_some() && m.looping), "{:?}", body.motions.iter().map(|m| &m.name).collect::<Vec<_>>());
    // Groups of three or four on one node each, the nodes a third of the flock apart.
    let mut nodes: Vec<i32> = c.birds.iter().map(|b| b.node).collect();
    nodes.dedup();
    assert!((13..=17).contains(&nodes.len()), "{nodes:?}");

    // The same layout every load, and the birds go round.
    let again = Creatures::load(&arch, "free06");
    assert!(c.birds.iter().zip(&again.birds).all(|(a, b)| a.pos == b.pos && a.node == b.node));
    let mut c = c;
    let start: Vec<V3> = c.birds.iter().map(|b| b.pos).collect();
    for _ in 0..300 {
        c.step(TICK, &[]);
    }
    let moved = c.birds.iter().zip(&start).filter(|(b, s)| b.pos.sub(**s).len() > 50.0).count();
    assert_eq!(moved, 50, "ten seconds at 60 ft/s");
    let placed = c.placed(0.5);
    assert_eq!(placed.len(), 50);
    assert!(placed.iter().all(|p| p.palette.len() == c.bodies[0].skeleton.len() && p.at.0[3][0].is_finite()));
}

#[test]
fn nat02s_vultures_sit_until_the_rider_comes() {
    let Some(mut disc) = disc() else { return };
    let arch = archives(&mut disc, "nat02");
    let mut c = Creatures::load(&arch, "nat02");
    println!("skipped: {:?}", c.skipped);
    let sitting: Vec<usize> = (0..c.birds.len()).filter(|&b| !c.birds[b].flying).collect();
    assert_eq!(sitting.len(), 3, "three perched vultures");
    assert!(sitting.iter().all(|&b| c.birds[b].playing.motion == Some(IDLE)));
    assert_eq!(c.triggers.len(), 3);
    let at: Vec<V3> = sitting.iter().map(|&b| c.birds[b].pos).collect();
    for _ in 0..60 {
        c.step(TICK, &[v3(1e6, 0.0, 1e6)]);
    }
    assert!(sitting.iter().zip(&at).all(|(&b, a)| !c.birds[b].flying && c.birds[b].pos == *a), "nobody near: they stay");

    // A rider in the first trigger scares whoever sits within its ScareDistance.
    let t = &c.triggers[0];
    let rider = t.at;
    assert!(t.holds(rider));
    let near: Vec<usize> =
        sitting.iter().copied().filter(|&b| t.scares.iter().any(|s| c.birds[b].pos.sub(s.at).len() < s.distance)).collect();
    println!("trigger {} at {:?}, scares {:?}, near {near:?}", t.name, t.at, t.scares);
    assert!(!near.is_empty(), "the trigger's scare reaches a vulture");
    c.step(TICK, &[rider]);
    for &b in &near {
        assert!(c.birds[b].flying);
        assert!(matches!(c.birds[b].playing.motion, Some(TAKE_OFF) | Some(FLY)), "{:?}", c.birds[b].playing);
    }
    for _ in 0..90 {
        c.step(TICK, &[]);
    }
    assert!(near.iter().zip(&at).all(|(&b, a)| c.birds[b].pos.sub(*a).len() > 10.0), "three seconds later they are away");
}

#[test]
fn nat04s_camels_play_their_sequences() {
    let Some(mut disc) = disc() else { return };
    let arch = archives(&mut disc, "nat04");
    let mut c = Creatures::load(&arch, "nat04");
    println!("skipped: {:?}", c.skipped);
    assert_eq!(c.toys.len(), 7, "seven camels");
    assert!(c.birds.is_empty());
    let first: Vec<_> = c.toys.iter().map(|t| t.playing.motion).collect();
    assert!(first.iter().all(Option::is_some), "{first:?}");
    let mut changed = 0;
    for _ in 0..1800 {
        let before: Vec<_> = c.toys.iter().map(|t| t.playing.motion).collect();
        c.step(TICK, &[]);
        changed += c.toys.iter().zip(&before).filter(|(t, b)| t.playing.time < TICK * 1.5 && t.playing.motion.is_some() && **b != None).count();
    }
    assert!(changed >= 7, "every camel moves on through its sequence in a minute: {changed}");
}

#[test]
fn nat07_has_mills_and_elk_and_nat01_draws_nothing() {
    let Some(mut disc) = disc() else { return };
    let arch = archives(&mut disc, "nat07");
    let c = Creatures::load(&arch, "nat07");
    println!("nat07 skipped: {:?}, models {:?}", c.skipped, c.models());
    assert!(c.toys.len() >= 4, "{}", c.toys.len());
    let arch = archives(&mut disc, "nat01");
    let c = Creatures::load(&arch, "nat01");
    println!("nat01 skipped: {:?}", c.skipped);
    assert!(c.birds.is_empty() && c.toys.is_empty());
}
