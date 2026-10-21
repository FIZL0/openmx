//! P58 — the biplane's particles: the engine smoke, the exhaust and the wreck's dust, with the
//! original's timing.
//!
//! `src/particles.rs` is compiled in by path, as the other tests take the shell's files. The disc's
//! half reads `biplaneparticles.dat`, its `.emt`s and free06's particle sheet off the user's disc and
//! skips without it, saying so; the rest steps the emitters on lists and `.emt`s written here, so the
//! rules are checked with no game data in the repository.
//!
//! The rules (journal `biplane-particles`, `research/oracle.txt` P58): an emitter's speed is how far
//! its point moved over the frame's length, and below `MinimumEmissionSpeed` it emits nothing; it emits
//! a rate drawn between its two a second, carried from frame to frame; the smoke is off while the
//! plane is wrecked; an exhaust is off once `throttle × ½ × (MaxSpeed − airspeed)` is not positive; the
//! wreck's dust comes from the collision model's contacts, and from nothing while it touches nothing.
//!
//! P61 adds the ground and the wind: a particle whose `.emt` has a `GroundLevel`, or whose collision
//! model entry says `HasCollision`, meets the terrain under it on odd frames — under it and going
//! down, its velocity is scaled by `Restitution`, then within a tenth of a foot it lies there and
//! deeper it bounces; `Wind` adds the system's wind to a particle's start velocity.

#[allow(dead_code)]
#[path = "../src/particles.rs"]
mod particles;

use game_render::Disc;
use game_render::biplane::{self, Emitter};
use game_render::ini;
use game_render::mat::{M4, V3, v3};
use particles::{Atlas, Disable, Emt, Kind, Particle, PlaneFrame, PlaneFx};

/// The shell's frame, and the original's: the capture's rows are 30 Hz.
const DT: f32 = 1.0 / 30.0;

/// A sheet of four frames in two groups, for a 64 by 64 texture.
const SHEET: &str = "[General Info]\nNumFrames=4\nNumGroups=2\n\
[Group0]\nName=\"Puff\"\nFirstFrame=0\nLastFrame=1\n\
[Group1]\nName=\"Grit\"\nFirstFrame=2\nLastFrame=3\n\
[Frame0]\nUVLeft=0\nUVRight=32\nUVTop=0\nUVBottom=32\n\
[Frame1]\nUVLeft=32\nUVRight=64\nUVTop=0\nUVBottom=32\n\
[Frame2]\nUVLeft=0\nUVRight=32\nUVTop=32\nUVBottom=64\n\
[Frame3]\nUVLeft=32\nUVRight=64\nUVTop=32\nUVBottom=64\n";

/// A smoke-like emitter: 40 to 40 a second, gated at 20 ft/s, a second's life, no noise.
const SMOKE: &str = "[General Info]\nParticleGroupName=\"Puff\"\nMinimumEmitRate=40\nMaximumEmitRate=40\n\
MinimumLife=1000\nMaximumLife=1000\nMinimumInitialSize=1\nMaximumInitialSize=1\nMinimumGrowthRate=2\nMaximumGrowthRate=2\n\
Drag=2.0\nVelocity=0, -4, 0\nMinimumVelocityNoise=0,0,0\nMaximumVelocityNoise=0,0,0\nGravityMultiplier=0.5\n\
FadeOutFromPercentAge=0.5\nMinimumEmissionSpeed=20\n\
[Properties]\nHasVelocity=T\nGrows=T\nFades=T\nGravity=T\nHasDrag=T\nFadesIn=F\nApplyEmitterVelocity=F\n";

/// A dust-like emitter: 30 a second, gated at 2 ft/s, its points half a foot up, taking the body's
/// velocity.
const DUST: &str = "[General Info]\nParticleGroupName=\"Grit\"\nMinimumEmitRate=30\nMaximumEmitRate=30\n\
MinimumLife=1500\nMaximumLife=1500\nVelocity=0, 6, 0\nMinimumVelocityNoise=-1,-1,-1\nMaximumVelocityNoise=1,1,1\n\
MinimumEmitPointExtent=0, 0.5, 0\nMaximumEmitPointExtent=0, 0.5, 0\nMinimumEmissionSpeed=2\n\
[Properties]\nHasVelocity=T\nApplyEmitterVelocity=T\nFades=T\n";

fn list() -> Vec<Emitter> {
    biplane::emitters(
        "[Wreck Dust]\nType = CollisionModel\nDisable = NoContact | ObjectContact\nEmitter = dust.emt\n\
         [Smoke]\nType = Generic\nDisable = Wrecking\nAttachedToSoultree = Stack_L\nEmitter = smoke.emt\n\
         [Exhaust]\nType = Exhaust\nAttachedToSoultree = Stack_L\nDisable = Wrecking\nEmitter = smoke.emt\nMaxSpeed = 30.0\n\
         [Wheel Dust]\nType = Dust\nAttachedToSoultree = Wheel\nEmitter = dust.emt\n",
    )
}

fn fx() -> PlaneFx {
    let read = |name: &str| match name {
        "dust.emt" => Some(DUST.as_bytes().to_vec()),
        "smoke.emt" => Some(SMOKE.as_bytes().to_vec()),
        _ => None,
    };
    PlaneFx::new(&list(), Atlas::parse(SHEET, 64, 64), read)
}

/// Only `name`'s entry left in `fx`, so a count is that entry's alone.
fn only(mut fx: PlaneFx, name: &str) -> PlaneFx {
    fx.entries.retain(|e| e.name == name);
    fx
}

/// A plane frame with its stack at `at`, level.
fn frame(at: V3, throttle: f32, airspeed: f32, wrecked: bool) -> PlaneFrame {
    let mut m = M4::IDENTITY;
    m.0[3] = [at.x, at.y, at.z, 1.0];
    PlaneFrame { nodes: vec![("stack_l".to_string(), m)], throttle, airspeed, wrecked, ..PlaneFrame::default() }
}

/// Fly the stack along x at `speed` ft/s for `frames`, and count what is in the air after.
fn fly(fx: &mut PlaneFx, speed: f32, frames: usize, wrecked: bool) -> usize {
    for f in 0..frames {
        fx.step(&frame(v3(speed * DT * f as f32, 100.0, 0.0), 0.0, speed, wrecked), DT);
    }
    fx.particles.len()
}

#[test]
fn the_list_takes_what_the_port_emits() {
    let fx = fx();
    let names: Vec<&str> = fx.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["Wreck Dust", "Smoke", "Exhaust"]);
    assert_eq!(fx.skipped, [("Wheel Dust".to_string(), "Dust".to_string())], "the tire dust is not ported");
    assert_eq!(fx.entries[2].kind, Kind::Exhaust { max_speed: 30.0 });
    assert_eq!(fx.entries[0].disable, Disable::NO_CONTACT | Disable::OBJECT_CONTACT);
    assert_eq!(fx.entries[1].frames, (0, 1));
    assert_eq!(fx.entries[0].frames, (2, 3));
    assert_eq!(fx.atlas.frames[3], [0.5, 0.5, 1.0, 1.0], "the sheet's pixels over the texture's size");
}

#[test]
fn an_emt_leaves_the_constructors_defaults() {
    let e = Emt::parse("[General Info]\nParticleGroupName=\"Puff\"\n");
    assert_eq!(e.group, "Puff");
    assert_eq!((e.life, e.size, e.gravity, e.fade_in, e.fade_out), ((500.0, 3500.0), (0.0, 5.0), 3.0, 0.25, 0.7));
    let s = Emt::parse(SMOKE);
    assert_eq!((s.rate, s.min_speed, s.velocity, s.drag), ((40.0, 40.0), 20.0, v3(0.0, -4.0, 0.0), 2.0));
    assert!(s.has_velocity && s.grows && s.fades && s.falls && s.has_drag && !s.fades_in && !s.apply_emitter_velocity);
}

#[test]
fn the_smoke_needs_the_exhausts_speed() {
    // Under the gate: nothing, however long.
    assert_eq!(fly(&mut only(fx(), "Smoke"), 19.0, 60, false), 0);
    // Over it: the rate times the time, the first frame having nowhere it came from. 19 frames of a
    // 30th of a second at 40 a second is 25.33: 25 whole ones, the rest carried (all younger than
    // their second's life).
    let mut fx = only(fx(), "Smoke");
    assert_eq!(fly(&mut fx, 21.0, 20, false), 25);
    assert!((fx.entries[0].sources[0].carry - 0.333).abs() < 0.01, "{}", fx.entries[0].sources[0].carry);
    // A second later the first ones have died: what is up is a second's worth.
    let up = fly(&mut fx, 21.0, 40, false);
    assert!((39..=41).contains(&up), "{up}");
    // Wrecked, off.
    assert_eq!(fly(&mut only(self::fx(), "Smoke"), 40.0, 60, true), 0);
}

#[test]
fn smoke_spreads_along_the_path_and_starts_where_the_stack_is() {
    let mut fx = only(fx(), "Smoke");
    fx.step(&frame(v3(0.0, 100.0, 0.0), 0.0, 0.0, false), DT);
    assert!(fx.particles.is_empty(), "the first frame on has not moved");
    // Three feet in a frame is 90 ft/s; at 40 a second, 1.33 a frame: one now, a third carried.
    fx.step(&frame(v3(3.0, 100.0, 0.0), 0.0, 0.0, false), DT);
    let xs: Vec<f32> = fx.particles.iter().map(|p| p.at.x).collect();
    assert_eq!(fx.particles.len(), 1);
    assert!((xs[0] - 3.0).abs() < 1e-4, "at the end of the path: {xs:?}");
    // At 90 a second, three a frame: at a third, two thirds and all the way.
    fx.entries[0].emt.rate = (90.0, 90.0);
    fx.particles.clear();
    fx.entries[0].sources[0].carry = 0.0;
    fx.step(&frame(v3(6.0, 100.0, 0.0), 0.0, 0.0, false), DT);
    let xs: Vec<f32> = fx.particles.iter().map(|p| p.at.x).collect();
    assert_eq!(xs.len(), 3);
    assert!(xs.iter().zip([4.0, 5.0, 6.0]).all(|(x, want)| (x - want).abs() < 1e-4), "{xs:?}");
    // Turned with its node: the file's straight down is the node's down.
    let mut fx = only(self::fx(), "Smoke");
    let mut m = M4([[0.0, -1.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]);
    for f in 0..3 {
        m.0[3] = [0.0, 0.0, f as f32 * 3.0, 1.0];
        fx.step(&PlaneFrame { nodes: vec![("Stack_L".into(), m)], ..PlaneFrame::default() }, DT);
    }
    let v = fx.particles.last().unwrap().vel; // the newest, not stepped yet
    assert!((v.x + 4.0).abs() < 1e-4 && v.y.abs() < 1e-4, "the node's y is the world's -x: {v:?}");
}

#[test]
fn the_exhaust_runs_on_the_throttle_below_max_speed() {
    let count = |throttle: f32, airspeed: f32| {
        let mut fx = only(fx(), "Exhaust");
        // Standing still, as the plane on its wheels: the exhaust's file has the speed gate too.
        for _ in 0..30 {
            fx.step(&frame(v3(0.0, 0.0, 0.0), throttle, airspeed, false), DT);
        }
        fx
    };
    assert_eq!(count(1.0, 0.0).particles.len(), 0, "the stack must move 20 ft/s for this file's gate");
    // A file with no gate: the exhaust's own.
    let open = |throttle: f32, airspeed: f32, wrecked: bool| {
        let mut fx = only(fx(), "Exhaust");
        fx.entries[0].emt.min_speed = 0.0;
        for _ in 0..30 {
            fx.step(&frame(v3(0.0, 0.0, 0.0), throttle, airspeed, wrecked), DT);
        }
        fx
    };
    let full = open(1.0, 0.0, false);
    assert_eq!(full.particles.len(), 40, "30 frames at 40 a second: with no gate the first frame counts");
    // f = 1 × ½ × 30/30 = ½: the file's y times f + ½, so 4 ft/s down once the first frame's drag
    // and gravity are taken off: the newest particle has not been stepped yet.
    let newest = full.particles.last().unwrap();
    assert!((newest.vel.y + 4.0).abs() < 1e-4, "{:?}", newest.vel);
    let slow = open(0.5, 15.0, false);
    // f = 0.5 × ½ × 15/30 = 0.125: y × 0.625.
    assert!((slow.particles.last().unwrap().vel.y + 2.5).abs() < 1e-4);
    assert_eq!(open(1.0, 30.0, false).particles.len(), 0, "at MaxSpeed f is 0");
    assert_eq!(open(1.0, 45.0, false).particles.len(), 0, "past it, negative");
    assert_eq!(open(0.0, 0.0, false).particles.len(), 0, "off the throttle");
    assert_eq!(open(1.0, 0.0, true).particles.len(), 0, "wrecked");
}

#[test]
fn the_wreck_dust_comes_off_the_contacts() {
    let ground = |vel: V3, contacts: Vec<V3>, objects: Vec<V3>| PlaneFrame {
        wrecked: true,
        vel,
        ground_contacts: contacts,
        object_contacts: objects,
        ..PlaneFrame::default()
    };
    // The wreck tick: the hull on the ground at 40 ft/s. A puff at once, with no frame before it:
    // the contact's speed is the body's.
    let mut fx = only(fx(), "Wreck Dust");
    let hit = v3(10.0, 5.0, -3.0);
    fx.step(&ground(v3(40.0, -10.0, 0.0), vec![hit], vec![]), DT);
    assert_eq!(fx.particles.len(), 1, "30 a second for a 30th");
    let p = &fx.particles[0];
    assert!((p.at.y - 5.5).abs() < 1e-4, "half a foot up: {:?}", p.at);
    assert!(p.at.sub(v3(10.0, 5.5, -3.0)).len() < 1e-4, "{:?}", p.at);
    assert!((p.vel.x - 40.0).abs() <= 1.0 && (p.vel.y - (6.0 - 10.0)).abs() <= 1.0, "the body's velocity on top: {:?}", p.vel);
    // Two contacts, two emitters: each carries its own.
    let mut fx = only(self::fx(), "Wreck Dust");
    for _ in 0..30 {
        fx.step(&ground(v3(40.0, 0.0, 0.0), vec![hit, v3(0.0, 0.0, 0.0)], vec![]), DT);
    }
    assert_eq!(fx.particles.len(), 60);
    // Slower than its gate, touching nothing, or touching something that is not the ground: none.
    for frame in [
        ground(v3(1.0, 0.0, 0.0), vec![hit], vec![]),
        ground(v3(40.0, 0.0, 0.0), vec![], vec![]),
        ground(v3(40.0, 0.0, 0.0), vec![hit], vec![hit]),
    ] {
        let mut fx = only(self::fx(), "Wreck Dust");
        for _ in 0..30 {
            fx.step(&frame, DT);
        }
        assert_eq!(fx.particles.len(), 0, "{frame:?}");
    }
}

#[test]
fn a_particle_ages_grows_moves_drags_and_falls_in_that_order() {
    let emt = Emt::parse(SMOKE);
    let mut p = Particle {
        emitter: 0,
        at: v3(0.0, 0.0, 0.0),
        vel: v3(10.0, 0.0, 0.0),
        size: 1.0,
        growth: 2.0,
        age: 0.0,
        life: 100.0,
        frame: 0,
        mirror: (false, false),
        landed: false,
    };
    assert!(p.step(&emt, 0.05));
    assert_eq!(p.age, 50.0, "milliseconds");
    assert!((p.size - 1.1).abs() < 1e-6 && (p.at.y - 0.1).abs() < 1e-6, "grows and rises by the growth: {p:?}");
    assert!((p.at.x - 0.5).abs() < 1e-6, "moved by the velocity before the drag: {p:?}");
    assert!((p.vel.x - 9.0).abs() < 1e-5, "drag 2 a second for a 20th: {p:?}");
    assert!((p.vel.y + 0.5 * 32.2 * 0.05).abs() < 1e-5, "gravity × 32.2: {p:?}");
    assert_eq!(p.alpha(&emt), 1.0, "half its life: not fading yet");
    assert!(p.step(&emt, 0.025));
    assert!((p.alpha(&emt) - 0.5).abs() < 1e-5, "three quarters: halfway out");
    assert!(p.step(&emt, 0.025), "at its life it is still drawn");
    assert!(!p.step(&emt, 0.001), "past it, gone");
}

#[test]
fn the_particles_have_their_own_random_numbers() {
    // Two runs of the same frames make the same particles, and the drawn squares are cut from the
    // sheet: the simulation's rand is not an input.
    let run = || {
        let mut fx = only(fx(), "Smoke");
        fly(&mut fx, 50.0, 20, false);
        fx.drawn()
    };
    let (a, b) = (run(), run());
    assert_eq!(a, b);
    assert!(!a.is_empty());
    // Mirrored or not, each square is one of the "Puff" group's two frames.
    let sorted = |[l, t, r, b]: [f32; 4]| [l.min(r), t.min(b), l.max(r), t.max(b)];
    let puff = [sorted([0.0, 0.0, 0.5, 0.5]), sorted([0.5, 0.0, 1.0, 0.5])];
    assert!(a.iter().all(|d| puff.contains(&sorted(d.uv))), "{a:?}");
}

/// The disc's own files: the plane's list out of free06's second archive, each `.emt` it names
/// beside it, and free06's sheet.
#[test]
fn the_discs_biplane_particles() {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            eprintln!("skipped: {why}");
            return;
        }
    };
    let names = disc.entries().iter().map(|e| e.name.clone()).collect::<Vec<_>>();
    let member = |end: &str| names.iter().find(|n| n.to_ascii_lowercase().ends_with(end)).cloned().unwrap();
    let archives = [disc.res(&member("free06.res")).unwrap(), disc.res(&member("free06_2.res")).unwrap()];
    let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
    let texture = game_render::pst::Texture::parse(&read(&particles::sheet_texture("free06")).unwrap()).unwrap();
    let atlas = Atlas::parse(&ini::text(&read(&particles::sheet("free06")).unwrap()), texture.width, texture.height);
    assert_eq!(atlas.frames.len(), 208);
    let list = biplane::emitters(&ini::text(&read(biplane::PARTICLES).unwrap()));
    let fx = PlaneFx::new(&list, atlas, read);

    let got: Vec<(&str, &Kind, Option<&str>, &str)> =
        fx.entries.iter().map(|e| (e.name.as_str(), &e.kind, e.node.as_deref(), e.emt.group.as_str())).collect();
    let exhaust = Kind::Exhaust { max_speed: 35.0 };
    assert_eq!(
        got,
        [
            ("Vehicle Dust", &Kind::CollisionModel, None, "Dust 1"),
            ("Vehicle Chunks", &Kind::CollisionModel, None, "Chunks 1"),
            ("Smoke", &Kind::Generic, Some("Biplane_Exhaust_L"), "Smoke 2"),
            ("Left Exhaust", &exhaust, Some("Biplane_Exhaust_L"), "Smoke 1"),
            ("Right Exhaust", &exhaust, Some("Biplane_Exhaust_R"), "Smoke 1"),
        ]
    );
    let skipped: Vec<&str> = fx.skipped.iter().map(|s| s.0.as_str()).collect();
    assert_eq!(skipped, ["Front Left Tire Dust", "Front Right Tire Dust", "Rear Tire Dust", "Dust"]);

    // The smoke: gated at 35 ft/s, off while wrecked, white puffs that sink and grow.
    let smoke = &fx.entries[2];
    assert_eq!((smoke.emt.min_speed, smoke.disable), (35.0, Disable::WRECKING));
    assert_eq!((smoke.emt.rate, smoke.emt.life), ((50.0, 60.0), (1200.0, 2300.0)));
    assert_eq!(smoke.frames, (126, 141));
    // The wreck's dust: off with no contact and on anything but the ground, gated at 3 ft/s, with
    // the body's velocity.
    let dust = &fx.entries[0];
    assert_eq!((dust.emt.min_speed, dust.disable), (3.0, Disable::NO_CONTACT | Disable::OBJECT_CONTACT));
    assert!(dust.emt.apply_emitter_velocity && !smoke.emt.apply_emitter_velocity);
    assert_eq!(dust.frames, (49, 64));
    // The exhaust has no gate of its own.
    assert_eq!(fx.entries[3].emt.min_speed, 0.0);
    // P61: only the chunks meet the ground — by their entry's `HasCollision = TRUE`, as
    // `wreckchunks.emt` has no `GroundLevel` — keeping 0.6 of their velocity. The dust and the smoke
    // take the wind, which the track's system holds at nothing.
    let chunks = &fx.entries[1];
    assert_eq!((chunks.emt.collides, chunks.emt.restitution, chunks.emt.gravity), (true, 0.6, 2.0));
    assert!(list.iter().any(|e| e.name == "Vehicle Chunks" && e.has_collision));
    assert!(fx.entries.iter().filter(|e| e.name != "Vehicle Chunks").all(|e| !e.emt.collides));
    let winded: Vec<&str> = fx.entries.iter().filter(|e| e.emt.wind).map(|e| e.name.as_str()).collect();
    assert_eq!(winded, ["Vehicle Dust", "Smoke"]);
    assert_eq!(fx.wind, V3::default());

    // Flown through: the smoke runs over 35 ft/s and stops on the wreck; the dust needs a contact.
    let mut fx = fx;
    let mut at = |x: f32| {
        let mut m = M4::IDENTITY;
        m.0[3] = [x, 200.0, 0.0, 1.0];
        vec![("Biplane_Exhaust_L".to_string(), m), ("Biplane_Exhaust_R".to_string(), m)]
    };
    for f in 0..30 {
        fx.step(&PlaneFrame { nodes: at(f as f32 * 40.0 * DT), airspeed: 40.0, throttle: 1.0, ..PlaneFrame::default() }, DT);
    }
    let counts = fx.counts();
    let smoked = counts.iter().find(|c| c.0 == "Smoke").unwrap().1;
    assert!((50 * 29 / 30..=60 * 29 / 30 + 1).contains(&smoked), "{counts:?}");
    assert!(counts.iter().filter(|c| c.0 != "Smoke").all(|c| c.1 == 0), "past MaxSpeed, no exhaust: {counts:?}");
    let before = fx.particles.len();
    fx.step(&PlaneFrame { nodes: at(30.0 * 40.0 * DT), airspeed: 40.0, wrecked: true, ..PlaneFrame::default() }, DT);
    assert!(fx.particles.len() <= before, "the wreck tick makes no smoke");
    fx.step(&PlaneFrame { wrecked: true, vel: v3(0.0, -60.0, 50.0), ground_contacts: vec![v3(0.0, 0.0, 0.0)], ..PlaneFrame::default() }, DT);
    assert!(fx.counts().iter().any(|c| c.0 == "Vehicle Dust" && c.1 >= 1), "{:?}", fx.counts());

    // The wreck's chunks on flat ground: a third of a second of contacts at 15 ft/s and more, then
    // half a second; none is ever more than two frames' fall under the ground, and some lie on it.
    let hit = PlaneFrame { wrecked: true, vel: v3(0.0, -10.0, 30.0), ground_contacts: vec![v3(0.0, 0.0, 0.0)], ..PlaneFrame::default() };
    let mut lowest = f32::MAX;
    for f in 0..25 {
        let frame = if f < 10 { hit.clone() } else { PlaneFrame { wrecked: true, ..PlaneFrame::default() } };
        fx.step_over(&frame, DT, &mut |_| Some(0.0));
        lowest = fx.particles.iter().filter(|p| fx.entries[p.emitter].name == "Vehicle Chunks").map(|p| p.at.y).fold(lowest, f32::min);
    }
    let chunks: Vec<&Particle> = fx.particles.iter().filter(|p| fx.entries[p.emitter].name == "Vehicle Chunks").collect();
    let landed = chunks.iter().filter(|p| p.landed).count();
    assert!(landed > 0 && lowest > -2.0, "{landed} of {} landed, lowest {lowest}", chunks.len());
    assert!(chunks.iter().filter(|p| p.landed).all(|p| p.at.y == 0.0));
}

/// Chunks that fall onto the ground: 60 a second, a second's life, thrown up at 10 ft/s, twice the
/// gravity, gated at 1 ft/s. No `GroundLevel`: the entry's `HasCollision` is what makes them collide.
const CHUNKS: &str = "[General Info]\nParticleGroupName=\"Grit\"\nMinimumEmitRate=60\nMaximumEmitRate=60\n\
MinimumLife=1000\nMaximumLife=1000\nVelocity=0, 10, 0\nGravityMultiplier=2\nRestitution = 0.60\nMinimumEmissionSpeed=1\n\
[Properties]\nHasVelocity=T\nGravity=T\nWind=T\n";

fn chunks(has_collision: &str) -> PlaneFx {
    let list = biplane::emitters(&format!(
        "[Chunks]\nType = CollisionModel\nDisable = NoContact\nHasCollision = {has_collision}\nEmitter = chunks.emt\n\
         [Thrown]\nType = Generic\nAttachedToSoultree = Stack_L\nHasCollision = TRUE\nEmitter = chunks.emt\n"
    ));
    PlaneFx::new(&list, Atlas::parse(SHEET, 64, 64), |n| (n == "chunks.emt").then(|| CHUNKS.as_bytes().to_vec()))
}

/// A particle of `emt` at `y`, going `vy`.
fn falling(y: f32, vy: f32) -> Particle {
    Particle {
        emitter: 0,
        at: v3(1.0, y, 2.0),
        vel: v3(4.0, vy, -2.0),
        size: 1.0,
        growth: 0.0,
        age: 0.0,
        life: 10_000.0,
        frame: 0,
        mirror: (false, false),
        landed: false,
    }
}

#[test]
fn an_emt_says_whether_it_meets_the_ground_and_takes_the_wind() {
    let d = Emt::parse("[General Info]\n");
    assert_eq!((d.collides, d.restitution, d.wind), (false, 0.55, false), "the constructor's");
    let e = Emt::parse("[General Info]\nGroundLevel=1.000000\nRestitution=0.100000\n[Properties]\nWind=T\nGravity=TRUE\nHasDrag=0\n");
    assert_eq!((e.collides, e.restitution, e.wind), (true, 0.1, true));
    assert!(e.falls && !e.has_drag, "a switch reads like the original's: TRUE on, 0 off");
    // The chunks' entry collides by its HasCollision, and only a collision model's entry reads it.
    let fx = chunks("TRUE");
    assert_eq!(fx.entries.iter().map(|e| (e.name.as_str(), e.emt.collides)).collect::<Vec<_>>(), [("Chunks", true), ("Thrown", false)]);
    assert!(!chunks("FALSE").entries[0].emt.collides);
}

#[test]
fn a_particle_bounces_off_the_ground_then_lies_on_it() {
    let mut emt = Emt::parse(CHUNKS);
    emt.collides = true;
    // Deep under the ground and going down: 0.6 of its velocity, the y turned round, left where it is.
    let mut p = falling(-0.5, -10.0);
    p.land(&emt, 0.0);
    assert_eq!((p.at.y, p.vel), (-0.5, v3(4.0 * 0.6, 10.0 * 0.6, -2.0 * 0.6)));
    assert!(!p.landed);
    // Going up, or above the ground: nothing.
    let before = p.clone();
    p.land(&emt, 0.0);
    assert_eq!(p, before, "going up");
    let mut q = falling(0.5, -10.0);
    q.land(&emt, 0.0);
    assert_eq!(q, falling(0.5, -10.0), "above the ground");
    // Within a tenth of a foot: scaled, put on the ground, and it lies there.
    let mut r = falling(9.95, -3.0);
    r.land(&emt, 10.0);
    assert_eq!((r.at.y, r.vel, r.landed), (10.0, v3(4.0 * 0.6, -3.0 * 0.6, -2.0 * 0.6), true));
    let lying = r.at;
    assert!(r.step(&emt, DT));
    assert_eq!(r.at, lying, "no longer moves or falls");
    assert_eq!(r.age, DT * 1000.0, "but still ages");
    // A tenth exactly is a bounce.
    let mut t = falling(-0.1, -1.0);
    t.land(&emt, 0.0);
    assert!(!t.landed && t.vel.y > 0.0);
    // An emitter that does not collide never meets it.
    let mut u = falling(-5.0, -10.0);
    u.land(&Emt::parse(CHUNKS), 0.0);
    assert_eq!(u, falling(-5.0, -10.0));
}

#[test]
fn the_ground_is_met_on_odd_frames() {
    let mut fx = only(chunks("TRUE"), "Chunks");
    let none = PlaneFrame::default();
    let asked = std::cell::Cell::new(0);
    let mut ground = |_: V3| {
        asked.set(asked.get() + 1);
        Some(0.0)
    };
    fx.particles.push(falling(-1.0, -20.0));
    fx.step_over(&none, DT, &mut ground);
    assert_eq!(fx.tick, 1);
    assert_eq!(asked.get(), 1, "frame 1 is odd: asked about the one particle");
    assert!(fx.particles[0].vel.y > 0.0, "and it bounced: {:?}", fx.particles[0]);
    fx.particles[0].vel.y = -20.0;
    fx.step_over(&none, DT, &mut ground);
    assert_eq!(asked.get(), 1, "frame 2 is even: not asked");
    assert!(fx.particles[0].vel.y < -20.0, "and it fell on: {:?}", fx.particles[0]);
    fx.step_over(&none, DT, &mut ground);
    assert_eq!(asked.get(), 2);
    // No ground under it is not a collision.
    fx.particles = vec![falling(-1.0, -20.0)];
    fx.tick = 0;
    fx.step_over(&none, DT, &mut |_| None);
    assert!(fx.particles[0].vel.y < 0.0);
}

#[test]
fn wreck_chunks_stay_on_the_ground() {
    // Chunks off a contact on flat ground at 50 for a third of a second, then a second's wait — the
    // first ones' life: with the ground they bounce and come to lie on it;
    // over no ground they fall through it, as P58's did.
    let run = |ground: bool| {
        let mut fx = only(chunks("TRUE"), "Chunks");
        let wreck = PlaneFrame { wrecked: true, vel: v3(0.0, 0.0, 20.0), ground_contacts: vec![v3(0.0, 50.0, 0.0)], ..PlaneFrame::default() };
        let mut lowest = f32::MAX;
        for f in 0..30 {
            let frame = if f < 10 { wreck.clone() } else { PlaneFrame { wrecked: true, ..PlaneFrame::default() } };
            fx.step_over(&frame, DT, &mut |_| ground.then_some(50.0));
            lowest = fx.particles.iter().map(|p| p.at.y).fold(lowest, f32::min);
        }
        (fx.particles.iter().filter(|p| p.landed).count(), fx.particles.len(), lowest)
    };
    let (landed, alive, lowest) = run(true);
    assert!(alive > 0 && landed * 2 > alive, "most lie on the ground by the end: {landed} of {alive}");
    // Tested every other frame, a chunk can be up to two frames' fall under it before it turns round.
    assert!(lowest > 50.0 - 2.0, "{lowest}");
    let (landed, _, lowest) = run(false);
    assert_eq!(landed, 0);
    assert!(lowest < 40.0, "{lowest}");
}

#[test]
fn the_wind_is_added_to_the_start_velocity() {
    let first = |wind: V3| {
        let mut fx = only(chunks("TRUE"), "Chunks");
        fx.wind = wind;
        let f = PlaneFrame { wrecked: true, vel: v3(0.0, 0.0, 20.0), ground_contacts: vec![v3(0.0, 50.0, 0.0)], ..PlaneFrame::default() };
        fx.step(&f, DT);
        fx.particles[0].vel
    };
    assert_eq!(first(V3::default()), v3(0.0, 10.0, 0.0));
    assert_eq!(first(v3(3.0, 0.0, -1.0)), v3(3.0, 10.0, -1.0));
}
