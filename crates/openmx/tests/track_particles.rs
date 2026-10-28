//! V45: the track's own particle emitters — the `.toy`'s `[ParticleEmitterN]` — read off the disc the
//! way the original's emitter toy reads them, and run by particles.rs's manager.
//!
//! What each track runs is checked against the list read by hand out of the disc's `.toy`s (journal
//! `track-particles`, V45): every emitter, each with its `.emt` and its sheet's group found, and
//! free07's 27 under a count of 0 left out. Then nat08's run for ten seconds: every emitter makes its
//! smoke or fire where it stands, from the frame after the track loads.
//!
//! V71: the ones that start off — nat12's geysers, which a rider in their triggers sets off, and every
//! sx track's fireworks, which the finish does, on the finish gate's nodes.
//!
//! The disc is not in the repo: without it the tests say so and pass.

#[allow(dead_code)]
#[path = "../src/particles.rs"]
mod particles;

use game_render::Disc;
use game_render::ini;
use game_render::toy;
use particles::{Atlas, TrackFx};

/// A track's emitters off the disc, and the manager built from them.
fn track_fx(disc: &mut Disc, track: &str) -> (Vec<toy::Emitter>, TrackFx) {
    let names = disc.entries().iter().map(|e| e.name.clone()).collect::<Vec<_>>();
    let member = names.iter().find(|n| n.eq_ignore_ascii_case(&format!("Tracks\\{track}.res"))).cloned().unwrap();
    let archives = [disc.res(&member).unwrap()];
    let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
    let text = ini::text(&read(&format!("{track}.toy")).unwrap());
    let list = toy::emitters(&text);
    let texture = game_render::pst::Texture::parse(&read(&particles::sheet_texture(track)).unwrap()).unwrap();
    let atlas = Atlas::parse(&ini::text(&read(&particles::sheet(track)).unwrap()), texture.width, texture.height);
    let fx = TrackFx::new(&list, atlas, read).with_triggers(toy::particle_triggers(&text));
    (list, fx)
}

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            eprintln!("skipped: {why}");
            None
        }
    }
}

/// Every sx track's fireworks: six of each colour's main, flash and smoke on the finish gate's six
/// nodes, six still smokes, and six sparks at their own places.
const FIREWORKS: &[(&str, usize)] = &[
    ("fireworks_blue.emt", 2),
    ("fireworks_blue_flash.emt", 2),
    ("fireworks_red.emt", 2),
    ("fireworks_red_flash.emt", 2),
    ("fireworks_smoke.emt", 6),
    ("fireworks_smoke_blue.emt", 2),
    ("fireworks_smoke_red.emt", 2),
    ("fireworks_smoke_white.emt", 2),
    ("fireworks_sparks.emt", 6),
    ("fireworks_white.emt", 2),
    ("fireworks_white_flash.emt", 2),
];

/// How many emitters each track lists and runs, and the `.emt`s it runs them on.
#[test]
fn every_tracks_emitters() {
    let Some(mut disc) = disc() else { return };
    let want: [(&str, usize, Vec<(&str, usize)>); 13] = [
        // free07's sections are under NumberOfParticleEmitters = 0: none are read.
        ("free07", 0, vec![]),
        ("nat08", 8, vec![("nat08_campfire.emt", 3), ("nat08_campfiresmoke.emt", 4), ("nat08_chimneysmoke.emt", 1)]),
        ("nat09", 2, vec![("blacksmoke.emt", 1), ("bonfire.emt", 1)]),
        ("nat11", 3, vec![("nat11_watermist.emt", 3)]),
        // The two geysers' blast, mist and spray start off, for their triggers.
        ("nat12", 8, vec![("geyserblast.emt", 2), ("geyserspray.emt", 2), ("mistpuff.emt", 2), ("puddlefog.emt", 1), ("puddlemist.emt", 1)]),
        ("nat14", 4, vec![("chimneysmoke.emt", 2), ("torchsmoke.emt", 2)]),
        ("nat15", 1, vec![("blacksmoke.emt", 1)]),
        ("nat17", 39, vec![("torch.emt", 15), ("torchsmoke.emt", 15), ("watermist.emt", 2), ("watermist01.emt", 4), ("watermist02.emt", 3)]),
        ("nat18", 6, vec![("chimneysmoke.emt", 6)]),
        ("nat01", 0, vec![]),
        // The fireworks: off until the finish, 24 on the finish gate.
        ("sx01", 30, FIREWORKS.to_vec()),
        // sx04 and sx20 have thirteen water mists beside theirs.
        ("sx04", 43, [FIREWORKS, &[("sx_watermist.emt", 3), ("watermist03.emt", 5), ("watermist04.emt", 5)]].concat()),
        ("sx20", 43, [FIREWORKS, &[("sx_watermist.emt", 3), ("watermist03.emt", 5), ("watermist04.emt", 5)]].concat()),
    ];
    for (track, listed, runs) in want {
        let (list, fx) = track_fx(&mut disc, track);
        assert_eq!(list.len(), listed, "{track}: emitters listed");
        assert!(list.iter().all(|e| e.kind == 0), "{track}: every emitter is Generic or Smoke, class 0");
        let mut got: Vec<(String, usize)> = Vec::new();
        for e in &fx.fx.entries {
            // An entry's node is its place in the list.
            let k: usize = e.node.as_deref().unwrap().trim_start_matches("ParticleEmitter").parse().unwrap();
            let file = list[k - 1].file.to_ascii_lowercase();
            match got.iter_mut().find(|(f, _)| *f == file) {
                Some((_, n)) => *n += 1,
                None => got.push((file, 1)),
            }
        }
        got.sort();
        let want: Vec<(String, usize)> = runs.iter().map(|(f, n)| (f.to_string(), *n)).collect();
        assert_eq!(got, want, "{track}: what runs");
        // None is left out: no track has one on a scene object, and every file and group is there.
        assert!(fx.skipped.is_empty(), "{track}: left out {:?}", fx.skipped);
        assert_eq!(fx.fx.entries.len() + fx.skipped.len(), listed, "{track}: each emitter run or left out");
    }
}

/// nat08's fires and smokes over ten seconds at the game's 30 a second: nothing the first frame, as
/// an emitter switched on emits from the frame after; then every emitter makes particles, and each
/// starts where its emitter stands.
#[test]
fn nat08_smokes_where_its_fires_are() {
    let Some(mut disc) = disc() else { return };
    let (list, mut fx) = track_fx(&mut disc, "nat08");
    let dt = 1.0 / 30.0;
    fx.step_over(dt, &mut |_| None);
    assert!(fx.fx.particles.is_empty(), "nothing on the first frame");
    fx.step_over(dt, &mut |_| None);
    for p in &fx.fx.particles {
        let node = fx.fx.entries[p.emitter].node.as_deref().unwrap();
        let at = fx.frame.nodes.iter().find(|(n, _)| n == node).unwrap().1.point(game_render::mat::v3(0.0, 0.0, 0.0));
        // Born this frame and moved one frame's worth: within its noise and its first step of it.
        assert!(p.at.sub(at).len() < 2.0, "{} born {:?} from {:?}", fx.fx.entries[p.emitter].name, p.at, at);
    }
    for _ in 0..300 {
        fx.step_over(dt, &mut |_| None);
    }
    for (name, n) in fx.fx.counts() {
        assert!(n > 0, "{name} has nothing in the air");
    }
    let positions: Vec<_> = list.iter().map(|e| e.position).collect();
    for p in &fx.fx.particles {
        // Smoke rises and drifts, but none of it far from some fire on the track.
        assert!(positions.iter().any(|e| p.at.sub(*e).len() < 150.0), "a particle at {:?}", p.at);
    }
    assert_eq!(fx.fx.drawn().len(), fx.fx.particles.len());
}

/// The particles in the air of the entries named so.
fn made(fx: &TrackFx, names: &[&str]) -> usize {
    fx.fx.particles.iter().filter(|p| names.iter().any(|n| fx.fx.entries[p.emitter].name.eq_ignore_ascii_case(n))).count()
}

/// nat12's geysers: two triggers, each with a coin toss as a rider comes in, set off their geyser's
/// blast, spray and mist for the one-shot's 15 seconds; the other geyser stays still.
#[test]
fn nat12_geysers_blow_when_a_rider_comes_by() {
    let Some(mut disc) = disc() else { return };
    let (_, mut fx) = track_fx(&mut disc, "nat12");
    let names: Vec<&str> = fx.triggers.iter().map(|t| t.trigger.name.as_str()).collect();
    assert_eq!(names, ["Geyser_b_trigger", "Geyser_a_trigger"]);
    for t in &fx.triggers {
        assert_eq!((t.trigger.percent, t.trigger.one_time, t.trigger.enabled), (50, false, true), "{}", t.trigger.name);
        let emitters: Vec<&str> = t.trigger.actions.iter().map(|a| a.emitter.as_str()).collect();
        let g = &t.trigger.name[..8].to_ascii_lowercase();
        assert_eq!(emitters, [format!("{g}_blast"), format!("{g}_spray"), format!("{g}_mist")], "the sound action is not a particles one");
        assert!(t.trigger.actions.iter().all(|a| a.on && a.delay == 0.0));
    }
    let a = ["Geyser_a_blast", "Geyser_a_spray", "Geyser_a_mist"];
    let b = ["Geyser_b_blast", "Geyser_b_spray", "Geyser_b_mist"];
    let dt = 1.0 / 30.0;
    let away = game_render::mat::v3(0.0, 0.0, 0.0);
    let gate = fx.triggers[1].trigger.at;
    for _ in 0..60 {
        fx.touch(&[away]);
        fx.step_over(dt, &mut |_| None);
    }
    assert_eq!(made(&fx, &a) + made(&fx, &b), 0, "still until a rider comes by");
    // In for a frame and out again until the coin comes up: one toss each time in.
    let mut tries = 0;
    while !fx.fx.entries.iter().any(|e| e.name == a[0] && e.gate.is_on()) {
        tries += 1;
        assert!(tries < 40, "forty tosses and no blast");
        fx.touch(&[gate]);
        fx.step_over(dt, &mut |_| None);
        fx.touch(&[away]);
        fx.step_over(dt, &mut |_| None);
    }
    let mut first = None;
    let mut last = 0;
    for f in 0..600 {
        fx.touch(&[away]);
        fx.step_over(dt, &mut |_| None);
        let lit = fx.fx.entries.iter().filter(|e| a.contains(&e.name.as_str())).all(|e| e.gate.is_on());
        if lit {
            last = f;
        }
        if first.is_none() && made(&fx, &a) > 0 {
            first = Some(f);
        }
    }
    let first = first.expect("the blast made nothing");
    assert!(first <= 2, "first made {first} frames on");
    // 15 s of 1/30 s frames and the one it runs out on, two of them before this loop: the rest frame
    // and the first shot's.
    assert!((447..=450).contains(&last), "on till {last}");
    assert_eq!(made(&fx, &b), 0, "the other geyser");
    assert!(fx.fx.entries.iter().filter(|e| a.contains(&e.name.as_str()) || b.contains(&e.name.as_str())).all(|e| !e.gate.is_on()));
}

/// Every sx track's fireworks: nothing until the finish; then each, on the finish gate's node or at
/// its place, waits its `.emt`'s delay and makes its shot, and the still smokes make nothing while
/// the gate stands still.
#[test]
fn the_finish_sets_the_fireworks_off() {
    let Some(mut disc) = disc() else { return };
    let (_, mut fx) = track_fx(&mut disc, "sx01");
    assert!(fx.on_toys());
    let node = |sub: &str| {
        let k: f32 = sub.trim_start_matches("FireworksDummy").parse().unwrap();
        let mut m = game_render::mat::M4::IDENTITY;
        m.0[3] = [100.0 + 10.0 * k, 160.0, 250.0, 1.0];
        m
    };
    let mut asked = Vec::new();
    fx.place(&mut |toy, sub| {
        asked.push((toy.to_string(), sub.to_string()));
        Some(node(sub))
    });
    assert_eq!(asked.len(), 24);
    assert!(asked.iter().all(|(t, s)| t == "FinishGate" && s.starts_with("FireworksDummy0")));
    let dt = 1.0 / 30.0;
    for _ in 0..90 {
        fx.step_over(dt, &mut |_| None);
    }
    assert!(fx.fx.particles.is_empty(), "nothing before the finish");
    fx.finish();
    let n = fx.fx.entries.len();
    let mut first = vec![None; n];
    let mut started = vec![None; n];
    let mut on_till = vec![0usize; n];
    for f in 0..300 {
        fx.step_over(dt, &mut |_| None);
        for p in &fx.fx.particles {
            first[p.emitter].get_or_insert(f);
        }
        for (k, e) in fx.fx.entries.iter().enumerate() {
            if e.gate.is_on() {
                on_till[k] = f;
            }
            // Its shot has started once its time left runs down.
            if e.gate.left != e.emt.times[1] {
                started[k].get_or_insert(f);
            }
        }
    }
    for (k, e) in fx.fx.entries.iter().enumerate() {
        let emt = &e.emt;
        if emt.mode == 0 {
            // fireworks_smoke: on for good, but it emits only moving a foot a second.
            assert!(emt.min_speed >= 1.0 && first[k].is_none(), "{} made something standing still", e.name);
            assert!(e.gate.is_on());
            continue;
        }
        assert_eq!(emt.mode, 1, "{}", e.name);
        // The rest frame, then the delay's frames.
        let wait = (emt.delay / dt).ceil() as usize;
        let f = started[k].unwrap_or_else(|| panic!("{} never started", e.name));
        assert!((wait..=wait + 2).contains(&f), "{}: delay {} started on {f}", e.name, emt.delay);
        let made = first[k].unwrap_or_else(|| panic!("{} made nothing", e.name));
        assert!(made >= f, "{}: made on {made} before its shot on {f}", e.name);
        let shot = (emt.times[1] / dt).ceil() as usize;
        assert!(on_till[k] >= wait + shot - 1 && on_till[k] <= wait + shot + 3, "{}: shot {} on till {}", e.name, emt.times[1], on_till[k]);
    }
    // The still smokes, carried along: now they make smoke.
    for f in 0..30 {
        fx.place(&mut |_, sub| {
            let mut m = node(sub);
            m.0[3][1] += f as f32 * 0.2;
            Some(m)
        });
        fx.step_over(dt, &mut |_| None);
    }
    for (k, e) in fx.fx.entries.iter().enumerate() {
        if e.emt.mode == 0 && fx.toys[k].toy.is_some() {
            assert!(fx.fx.particles.iter().any(|p| p.emitter == k), "{} moving made nothing", e.name);
        }
    }
}

/// The finish gate's six fireworks nodes are its skeleton's, which the shell places them on.
#[test]
fn the_finish_gate_has_the_fireworks_nodes() {
    let Some(mut disc) = disc() else { return };
    let names = disc.entries().iter().map(|e| e.name.clone()).collect::<Vec<_>>();
    let member = names.iter().find(|n| n.eq_ignore_ascii_case("Tracks\\sx01.res")).cloned().unwrap();
    let mut archives = vec![disc.res(&member).unwrap()];
    let animated = game_render::animated_toy::Animated::load(&mut archives, "sx01");
    let k = animated.names.iter().position(|n| n.eq_ignore_ascii_case("FinishGate")).unwrap_or_else(|| panic!("no FinishGate in {:?}, left out {:?}", animated.names, animated.skipped));
    for n in 1..=6 {
        let name = format!("FireworksDummy{n:02}");
        assert!(animated.bones[k].iter().any(|b| b.eq_ignore_ascii_case(&name)), "{name} not in {:?}", animated.bones[k]);
    }
}
