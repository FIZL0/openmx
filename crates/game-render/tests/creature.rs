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
        c.step(TICK, &[creature::Rays::at(v3(1e6, 0.0, 1e6))]);
    }
    assert!(sitting.iter().zip(&at).all(|(&b, a)| !c.birds[b].flying && c.birds[b].pos == *a), "nobody near: they stay");

    // A rider in the first trigger scares whoever sits within its ScareDistance.
    let t = &c.triggers[0];
    let rider = t.at;
    assert!(t.holds(rider));
    let near: Vec<usize> =
        sitting.iter().copied().filter(|&b| t.scares().any(|s| c.birds[b].pos.sub(s.at).len() < s.distance)).collect();
    println!("trigger {} at {:?}, scares {:?}, near {near:?}", t.name, t.at, t.scares().collect::<Vec<_>>());
    assert!(!near.is_empty(), "the trigger's scare reaches a vulture");
    c.step(TICK, &[creature::Rays::at(rider)]);
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

/// V75: every bird trigger on the disc is a box (no `Radius`), its look and up square to each other,
/// so the frame the reader builds with its squaring off is a rotation and the port's transpose takes a
/// point into the box. nat02's vulture_10_trigger is printed: Box is half its size along each axis.
#[test]
fn every_bird_trigger_is_a_square_box() {
    let Some(mut disc) = disc() else { return };
    let mut tracks: Vec<String> = disc
        .entries()
        .iter()
        .filter_map(|e| {
            let n = e.name.to_ascii_lowercase();
            let stem = n.rsplit('\\').next()?.strip_suffix(".res")?.to_string();
            (stem.len() == 5 && (stem.starts_with("nat") || stem.starts_with("free"))).then_some(stem)
        })
        .collect();
    tracks.sort();
    tracks.dedup();
    let mut seen = 0;
    for t in &tracks {
        let arch = archives(&mut disc, t);
        let c = Creatures::load(&arch, t);
        for tr in c.triggers.iter().filter(|tr| !tr.actions.is_empty()) {
            seen += 1;
            println!("{t} {}: at {:?} box {:?} look {:?} up {:?} percent {} one_time {}", tr.name, tr.at, tr.half, tr.look, tr.up, tr.percent, tr.one_time);
            assert!(tr.radius.is_none(), "{t} {} is a sphere", tr.name);
            assert!(tr.look.dot(tr.up).abs() < 1e-6, "{t} {}: look·up {}", tr.name, tr.look.dot(tr.up));
            assert!(tr.half.x > 0.0 && tr.half.y > 0.0 && tr.half.z > 0.0, "{t} {}", tr.name);
        }
    }
    assert!(seen >= 3, "nat02's three at least: {seen} on {tracks:?}");
    let arch = archives(&mut disc, "nat02");
    let c = Creatures::load(&arch, "nat02");
    let v = c.triggers.iter().find(|t| t.name.eq_ignore_ascii_case("vulture_10_trigger")).expect("vulture_10_trigger");
    // Inside by just under its half on each axis, out by just over.
    let along = |s: f32| v.at.add(v.right.scale(s * v.half.x)).add(v.up.scale(s * v.half.y)).add(v.look.scale(s * v.half.z));
    assert!(v.holds(along(0.99)) && v.holds(along(-0.99)));
    assert!(!v.holds(along(1.01)) && !v.holds(along(-1.01)));
}

/// A box 20 feet on a side at the origin, square to the world.
fn cube(percent: i32) -> creature::Trigger {
    creature::Trigger {
        name: "t".into(),
        half: v3(10.0, 10.0, 10.0),
        right: v3(1.0, 0.0, 0.0),
        up: v3(0.0, 1.0, 0.0),
        look: v3(0.0, 0.0, 1.0),
        enabled: true,
        percent,
        ..Default::default()
    }
}

/// V102: a trigger under 100 percent rolls only on a tick nobody was in it the tick before, as the
/// toy trigger's contact does; a rider held in it rolls once, however long it stays.
#[test]
fn a_chance_trigger_rolls_on_entry_only() {
    let mut c = Creatures::default();
    c.triggers.push(cube(50));
    let (inside, outside) = (creature::Rays::at(V3::default()), creature::Rays::at(v3(1e6, 0.0, 0.0)));
    let rolled = |c: &mut Creatures, riders: &[creature::Rays]| {
        let before = c.rng;
        c.step(TICK, riders);
        let mut r = before;
        if r == c.rng {
            return 0;
        }
        (1..10).find(|_| {
            r.rand();
            r == c.rng
        })
        .expect("the trigger draws a few numbers at most")
    };
    assert_eq!(rolled(&mut c, &[outside.clone()]), 0);
    assert_eq!(rolled(&mut c, &[inside.clone()]), 1, "entry rolls");
    for _ in 0..20 {
        assert_eq!(rolled(&mut c, &[inside.clone()]), 0, "staying in rolls nothing");
    }
    // A second rider coming in while the first stays: still in contact, no roll.
    assert_eq!(rolled(&mut c, &[inside.clone(), inside.clone()]), 0);
    assert_eq!(rolled(&mut c, &[outside.clone(), outside.clone()]), 0);
    // Two coming in on one tick roll once each.
    assert_eq!(rolled(&mut c, &[inside.clone(), inside.clone()]), 2);
}

/// V104: a trigger's keys as the reader takes them. `IgnoreTrigger` is searched for its three words
/// as written (so `AIANIMATION` is AI only); the tests are kept by name; the counter, delay and
/// `UponExit` keys go on each action.
#[test]
fn trigger_keys_read_as_the_original_reads_them() {
    let text = "[Triggers]\nNumberOfTriggers=1\n[Trigger1]\nName=t\nPosition=1,2,3\nBox=5,5,5\n\
                IgnoreTrigger=AIANIMATION\nEnterTest=Nobody\nNumberOfActions=2\n\
                [Trigger1Action1]\nActionType=ScareBirds\nScarePosition=1,2,3\nScareDistance=40\n\
                TriggerCounterEquals=2\nTriggerActionDelayTimer=0.5\n\
                [Trigger1Action2]\nActionType=ScareBirds\nTriggerActionUponExit=TRUE\nActionTest=Nobody\n";
    let c = Creatures::parse(&[], text);
    let t = &c.triggers[0];
    assert_eq!(t.ignore, creature::IGNORE_AI);
    assert_eq!((t.enter_test.as_deref(), t.exit_test.as_deref()), (Some("Nobody"), None));
    let [a, b] = &t.actions[..] else { panic!("{:?}", t.actions) };
    assert_eq!((a.equals, a.greater, a.delay, a.upon_exit, a.scare.distance), (2, -1, 0.5, false, 40.0));
    assert_eq!((b.equals, b.greater, b.delay, b.upon_exit, b.test.as_deref()), (-1, -1, 0.0, true, Some("Nobody")));
    let all = "[Triggers]\nNumberOfTriggers=1\n[Trigger1]\nIgnoreTrigger=PLAYER ANIMATIONS AI\nNumberOfActions=1\n\
               [Trigger1Action1]\nActionType=ScareBirds\n";
    assert_eq!(Creatures::parse(&[], all).triggers[0].ignore, 7);
}

/// V104: an actor stays in a trigger until its box is out of the trigger's, however its segments
/// strike: back in its segments' reach before that, the trigger fires again (it is touched) but runs
/// no actions for it. Out and back, it runs them again.
#[test]
fn an_actor_is_in_until_its_box_leaves() {
    let mut c = Creatures::default();
    let mut t = cube(100);
    t.actions.push(creature::Action { equals: -1, greater: -1, ..Default::default() });
    c.triggers.push(t);
    let inside = creature::Rays::at(v3(1.0, 2.0, 3.0));
    // Past the cube's corner: the segment's box overlaps it, the segment misses.
    let grazing = creature::Rays(vec![[v3(-15.0, 6.0, 0.0), v3(-5.0, 16.0, 0.0)]]);
    let out = creature::Rays::at(v3(500.0, 0.0, 0.0));
    let runs = |c: &Creatures| c.triggers[0].actions[0].ran;
    c.step(TICK, &[inside.clone()]);
    assert_eq!((c.triggers[0].count, runs(&c)), (1, [0, 1]));
    c.step(TICK, &[grazing.clone()]);
    assert_eq!(c.triggers[0].inside, vec![Some(0)], "kept by its box");
    c.step(TICK, &[inside.clone()]);
    assert_eq!((c.triggers[0].count, runs(&c)), (2, [0, 1]), "fired, and no second run");
    c.step(TICK, &[out.clone()]);
    assert_eq!(c.triggers[0].inside, vec![None], "the slot is kept, empty");
    c.step(TICK, &[inside]);
    assert_eq!((c.triggers[0].count, runs(&c), c.triggers[0].inside.clone()), (3, [0, 2], vec![Some(0)]));
}
