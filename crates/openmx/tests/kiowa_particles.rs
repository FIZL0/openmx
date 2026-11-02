//! V114 — the Kiowa's particles: the rotor wash and the wreck's dust, off the disc's
//! `helicopterparticles.dat` through the plane's particle manager.
//!
//! `src/particles.rs` is compiled in by path, as the other tests take the shell's files. The disc's
//! half reads both copies of the list (a freeride track's and a `_ss` track's), their `.emt`s and
//! free09's particle sheet off the user's disc and skips without it, saying so; the rest steps a list
//! written here, so the rule is checked with no game data in the repository.
//!
//! The rule the Kiowa adds: a `CollisionModel` entry emits at the contacts of the collision model its
//! `AttachedToCollisionModel` names — 0 the body's, 1 to 4 the four thrown blades' — and at no other's.

#[allow(dead_code)]
#[path = "../src/particles.rs"]
mod particles;

use game_render::Disc;
use game_render::biplane;
use game_render::ini;
use game_render::mat::{V3, v3};
use particles::{Atlas, Disable, Kind, ModelFrame, PlaneFrame, PlaneFx};

/// The shell's frame, and the original's.
const DT: f32 = 1.0 / 30.0;

/// One group of one frame, for a 32 by 32 texture.
const SHEET: &str = "[General Info]\nNumFrames=1\nNumGroups=1\n\
[Group0]\nName=\"Grit\"\nFirstFrame=0\nLastFrame=0\n\
[Frame0]\nUVLeft=0\nUVRight=32\nUVTop=0\nUVBottom=32\n";

/// 30 a second, no gate, long-lived.
const DUST: &str = "[General Info]\nParticleGroupName=\"Grit\"\nMinimumEmitRate=30\nMaximumEmitRate=30\n\
MinimumLife=5000\nMaximumLife=5000\n[Properties]\nFades=T\n";

fn fx() -> PlaneFx {
    let list = biplane::emitters(
        "[Body]\nType = CollisionModel\nAttachedToCollisionModel = 0\nDisable = NoContact | ObjectContact\nEmitter = dust.emt\n\
         [Blade 2]\nType = CollisionModel\nAttachedToCollisionModel = 2\nDisable = NoContact | ObjectContact\nEmitter = dust.emt\n\
         [Unsaid]\nType = CollisionModel\nDisable = NoContact | ObjectContact\nEmitter = dust.emt\n",
    );
    let read = |name: &str| (name == "dust.emt").then(|| DUST.as_bytes().to_vec());
    PlaneFx::new(&list, Atlas::parse(SHEET, 32, 32), read)
}

/// `frames` frames of `frame`, and how many particles each entry made.
fn run(fx: &mut PlaneFx, frame: &PlaneFrame, frames: usize) -> Vec<(String, usize)> {
    for _ in 0..frames {
        fx.step(frame, DT);
    }
    fx.counts().into_iter().map(|(n, c)| (n.to_string(), c)).collect()
}

fn count(got: &[(String, usize)], name: &str) -> usize {
    got.iter().find(|(n, _)| n == name).map_or(0, |(_, c)| *c)
}

#[test]
fn the_list_keeps_each_entrys_collision_model() {
    let fx = fx();
    let models: Vec<(&str, usize)> = fx.entries.iter().map(|e| (e.name.as_str(), e.model)).collect();
    assert_eq!(models, [("Body", 0), ("Blade 2", 2), ("Unsaid", 0)], "an entry that does not say is the body's");
}

#[test]
fn a_blades_dust_comes_off_its_blade_alone() {
    let ground = vec![v3(0.0, 0.0, 0.0)];
    // The body on the ground, no blade anywhere: the body's entries, never the blade's.
    let body = PlaneFrame { ground_contacts: ground.clone(), vel: v3(10.0, 0.0, 0.0), ..PlaneFrame::default() };
    let got = run(&mut fx(), &body, 30);
    assert!(count(&got, "Body") > 0 && count(&got, "Unsaid") > 0, "{got:?}");
    assert_eq!(count(&got, "Blade 2"), 0, "a blade with no model touches nothing: {got:?}");

    // The second blade on the ground and the body in the air: that blade's entry alone.
    let blade = ModelFrame { ground_contacts: ground.clone(), vel: v3(0.0, -5.0, 0.0), ..ModelFrame::default() };
    let thrown = PlaneFrame { models: vec![ModelFrame::default(), blade.clone()], ..PlaneFrame::default() };
    let got = run(&mut fx(), &thrown, 30);
    assert_eq!((count(&got, "Body"), count(&got, "Unsaid")), (0, 0), "{got:?}");
    // The first frame on only takes the point: 29 frames of a 30th at 30 a second, which the carry
    // adds up in single precision to just under 29.
    assert_eq!(count(&got, "Blade 2"), 28, "{got:?}");

    // Its object contacts switch it off as the body's do.
    let struck = ModelFrame { object_contacts: ground, ..blade };
    let off = PlaneFrame { models: vec![ModelFrame::default(), struck], ..PlaneFrame::default() };
    assert_eq!(count(&run(&mut fx(), &off, 30), "Blade 2"), 0);
}

#[test]
fn the_discs_kiowa_particles() {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            eprintln!("skipped: {why}");
            return;
        }
    };
    let names = disc.entries().iter().map(|e| e.name.clone()).collect::<Vec<_>>();
    let member = |end: &str| names.iter().find(|n| n.to_ascii_lowercase().ends_with(end)).cloned().unwrap();
    let wash = |begin, end| Kind::HelicopterDust { begin, end };
    let nc_oc = Disable::NO_CONTACT | Disable::OBJECT_CONTACT;
    for (track, full) in [("free09", true), ("free09_ss", false)] {
        let archives = [disc.res(&member(&format!("{track}.res"))).unwrap(), disc.res(&member(&format!("{track}_2.res"))).unwrap()];
        let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
        // A `_ss` track draws from its freeride track's sheet.
        let sheet = |name: String| read(&name).or_else(|| read(&name.replace("_ss", "")));
        let texture = game_render::pst::Texture::parse(&sheet(particles::sheet_texture(track)).unwrap()).unwrap();
        let atlas = Atlas::parse(&ini::text(&sheet(particles::sheet(track)).unwrap()), texture.width, texture.height);
        let list = biplane::emitters(&ini::text(&read(particles::KIOWA_PARTICLES).unwrap()));
        let fx = PlaneFx::new(&list, atlas, read);
        assert!(fx.skipped.is_empty(), "{track}: {:?}", fx.skipped);

        let got: Vec<(&str, &Kind, usize, Option<&str>, u32)> =
            fx.entries.iter().map(|e| (e.name.as_str(), &e.kind, e.model, e.node.as_deref(), e.disable)).collect();
        let cm = &Kind::CollisionModel;
        let w = wash(15.0, 40.0);
        let mut want = vec![("MainBody Dust", cm, 0, None, nc_oc)];
        if full {
            want.extend([("MainBody Dirt", cm, 0, None, nc_oc), ("MainBody Chunks", cm, 0, None, nc_oc)]);
        }
        let dusts = ["Blade 1 Dust", "Blade 2 Dust", "Blade 3 Dust", "Blade 4 Dust"];
        let chunks = ["Blade 1 Chunks", "Blade 2 Chunks", "Blade 3 Chunks", "Blade 4 Chunks"];
        for k in 0..4 {
            want.push((dusts[k], cm, k + 1, None, nc_oc));
            if full {
                want.push((chunks[k], cm, k + 1, None, nc_oc));
            }
        }
        want.push(("Dust", &w, 0, Some(game_render::kiowa::SPINDLE), Disable::WRECKING));
        assert_eq!(got, want, "{track}");

        // The wash: 150 to 250 a second on a freeride track, 25 to 50 on a `_ss` one.
        let dust = fx.entries.last().unwrap();
        assert_eq!(dust.emt.rate, if full { (150.0, 250.0) } else { (25.0, 50.0) }, "{track}");
        assert!(dust.emt.matches_terrain && dust.emt.wind, "{track}");
        // The chunks collide, the dust does not.
        for e in &fx.entries {
            assert_eq!(e.emt.collides, e.name.contains("Chunks"), "{track}: {}", e.name);
        }
    }
}

/// The wash fades with the spindle's height over the ground: the file's rate to 15 ft, none past
/// 40, and between, the rate times what is left of the fade.
#[test]
fn the_wash_fades_from_fifteen_feet_to_forty() {
    let list = biplane::emitters(
        "[Dust]\nType = HelicopterDust\nAttachedToSoultree = RotorSpindle\nDisable = Wrecking\nEmitter = dust.emt\n\
         BeginFadeHeight = 15.0\nEndFadeHeight = 40.0\n",
    );
    let read = |name: &str| (name == "dust.emt").then(|| DUST.as_bytes().to_vec());
    let at = |h: f32, wrecked: bool| {
        let mut m = game_render::mat::M4::IDENTITY;
        m.0[3] = [0.0, h, 0.0, 1.0];
        PlaneFrame { nodes: vec![(game_render::kiowa::SPINDLE.to_string(), m)], wrecked, ..PlaneFrame::default() }
    };
    let made = |h: f32, wrecked: bool| {
        let mut fx = PlaneFx::new(&list, Atlas::parse(SHEET, 32, 32), read);
        let f = at(h, wrecked);
        for _ in 0..60 {
            fx.step_over(&f, DT, &mut |_: V3| Some((0.0, 0)));
        }
        fx.particles.len()
    };
    let low = made(10.0, false);
    assert!(low >= 55, "{low}");
    let half = made(27.5, false);
    assert!(half * 2 >= low - 4 && half * 2 <= low + 4, "half way up, half the rate: {half} of {low}");
    assert_eq!(made(41.0, false), 0);
    assert_eq!(made(10.0, true), 0, "off while wrecked");
}
