//! V45: the track's own particle emitters — the `.toy`'s `[ParticleEmitterN]` — read off the disc the
//! way the original's emitter toy reads them, and run by particles.rs's manager.
//!
//! What each track runs is checked against the list read by hand out of the disc's `.toy`s (journal
//! `track-particles`, V45): the emitters that stand at a `Position` and start on, each with its `.emt`
//! and its sheet's group found; the geysers that start off, every sx track's fireworks on the finish
//! gate, and free07's 27 under a count of 0 left out. Then nat08's run for ten seconds: every emitter
//! makes its smoke or fire where it stands, from the frame after the track loads.
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
    let list = toy::emitters(&ini::text(&read(&format!("{track}.toy")).unwrap()));
    let texture = game_render::pst::Texture::parse(&read(&particles::sheet_texture(track)).unwrap()).unwrap();
    let atlas = Atlas::parse(&ini::text(&read(&particles::sheet(track)).unwrap()), texture.width, texture.height);
    let fx = TrackFx::new(&list, atlas, read);
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

/// How many emitters each track lists and runs, and the `.emt`s it runs them on.
#[test]
fn every_tracks_emitters() {
    let Some(mut disc) = disc() else { return };
    let want: [(&str, usize, &[(&str, usize)]); 13] = [
        // free07's sections are under NumberOfParticleEmitters = 0: none are read.
        ("free07", 0, &[]),
        ("nat08", 8, &[("nat08_campfire.emt", 3), ("nat08_campfiresmoke.emt", 4), ("nat08_chimneysmoke.emt", 1)]),
        ("nat09", 2, &[("blacksmoke.emt", 1), ("bonfire.emt", 1)]),
        ("nat11", 3, &[("nat11_watermist.emt", 3)]),
        // The two geysers' blast, mist and spray start off, for their triggers.
        ("nat12", 8, &[("puddlefog.emt", 1), ("puddlemist.emt", 1)]),
        ("nat14", 4, &[("chimneysmoke.emt", 2), ("torchsmoke.emt", 2)]),
        ("nat15", 1, &[("blacksmoke.emt", 1)]),
        ("nat17", 39, &[("torch.emt", 15), ("torchsmoke.emt", 15), ("watermist.emt", 2), ("watermist01.emt", 4), ("watermist02.emt", 3)]),
        ("nat18", 6, &[("chimneysmoke.emt", 6)]),
        ("nat01", 0, &[]),
        // The fireworks: off until the finish, and on the finish gate.
        ("sx01", 30, &[]),
        // sx04 and sx20 have thirteen water mists beside theirs.
        ("sx04", 43, &[("sx_watermist.emt", 3), ("watermist03.emt", 5), ("watermist04.emt", 5)]),
        ("sx20", 43, &[("sx_watermist.emt", 3), ("watermist03.emt", 5), ("watermist04.emt", 5)]),
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
        // Every emitter left out is left out for starting off or riding an object, never for a
        // missing file or group.
        for (name, why) in &fx.skipped {
            assert!(why == "starts off" || why == "on an object", "{track}: {name} skipped: {why}");
        }
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
