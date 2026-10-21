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

#[allow(dead_code)]
#[path = "../src/particles.rs"]
mod particles;

use game_render::Disc;
use game_render::biplane::{self, Emitter};
use game_render::ini;
use game_render::mat::{M4, V3, v3};
use particles::{Atlas, Disable, DustKeys, Emt, Kind, Particle, PlaneFrame, PlaneFx, TireFrame, Zone};

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

/// A prop-wash-like emitter: 30 a second, no gate, still particles started two feet over the ground.
const WASH: &str = "[General Info]\nParticleGroupName=\"Grit\"\nMinimumEmitRate=30\nMaximumEmitRate=30\n\
MinimumLife=2000\nMaximumLife=2000\nMinimumEmitPointExtent=0, 2, 0\nMaximumEmitPointExtent=0, 2, 0\n\
[Properties]\nMatchTerrainContour=T\n";

/// A zones file: the second surface's dust is the puff, the third's nothing, the rest the `.emt`'s.
const ZONE_FILE: &str = "[Default]\n[Grit]\nZone2=\"Puff\"\nZone3=none\n";

fn list() -> Vec<Emitter> {
    biplane::emitters(
        "[Wreck Dust]\nType = CollisionModel\nDisable = NoContact | ObjectContact\nEmitter = dust.emt\n\
         [Smoke]\nType = Generic\nDisable = Wrecking\nAttachedToSoultree = Stack_L\nEmitter = smoke.emt\n\
         [Exhaust]\nType = Exhaust\nAttachedToSoultree = Stack_L\nDisable = Wrecking\nEmitter = smoke.emt\nMaxSpeed = 30.0\n\
         [Wheel Dust]\nType = Dust\nAttachedToSoultree = Wheel\nAttachedToTire = 0\nDisable = ObjectContact | Wrecking\n\
         ZoneDescriptor = Grit\nEmitter = dust.emt\nMinCompressionThreshold = 0.1\n\
         [Wash]\nType = HelicopterDust\nZoneDescriptor = Grit\nAttachedToSoultree = Prop\nDisable = Wrecking\nEmitter = wash.emt\n\
         BeginFadeHeight = 20.0\nEndFadeHeight = 25.0\n\
         [Ruts]\nType = TireTrack\nAttachedToTire = 0\nEmitter = dust.emt\n",
    )
}

fn fx() -> PlaneFx {
    let read = |name: &str| match name {
        "dust.emt" => Some(DUST.as_bytes().to_vec()),
        "smoke.emt" => Some(SMOKE.as_bytes().to_vec()),
        "wash.emt" => Some(WASH.as_bytes().to_vec()),
        "mxparticlezones.dat" => Some(ZONE_FILE.as_bytes().to_vec()),
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
    assert_eq!(names, ["Wreck Dust", "Smoke", "Exhaust", "Wheel Dust", "Wash"]);
    assert_eq!(fx.skipped, [("Ruts".to_string(), "TireTrack".to_string())], "the tire tracks are not the plane's");
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
    // The tire dusts set MinCompressionThreshold to 0 and leave the rest at the constructor's.
    let tire_dust = Kind::Dust(DustKeys { min_compression: 0.0, ..DustKeys::default() });
    let wash = Kind::HelicopterDust { begin: 20.0, end: 25.0 };
    assert_eq!(
        got,
        [
            ("Vehicle Dust", &Kind::CollisionModel, None, "Dust 1"),
            ("Vehicle Chunks", &Kind::CollisionModel, None, "Chunks 1"),
            ("Smoke", &Kind::Generic, Some("Biplane_Exhaust_L"), "Smoke 2"),
            ("Left Exhaust", &exhaust, Some("Biplane_Exhaust_L"), "Smoke 1"),
            ("Right Exhaust", &exhaust, Some("Biplane_Exhaust_R"), "Smoke 1"),
            ("Front Left Tire Dust", &tire_dust, Some("biplane_FtireL_particles"), "Dust 1"),
            ("Front Right Tire Dust", &tire_dust, Some("biplane_FtireR_particles"), "Dust 1"),
            ("Rear Tire Dust", &tire_dust, Some("biplane_Rtire_particles"), "Dust 1"),
            ("Dust", &wash, Some("PropDummy"), "Dust 1"),
        ]
    );
    assert!(fx.skipped.is_empty(), "{:?}", fx.skipped);
    let tires: Vec<Option<usize>> = fx.entries.iter().map(|e| e.tire).collect();
    assert_eq!(tires, [None, None, None, Some(0), Some(1), Some(0), Some(1), Some(2), None]);
    for e in &fx.entries[5..] {
        // The disc's zones file names no zone of `Dust`: every surface raises the .emt's own dust.
        assert_eq!(e.zones, [particles::Zone::Group(49, 64); 16], "{}", e.name);
        assert_eq!(e.disable, Disable::WRECKING | if e.tire.is_some() { Disable::OBJECT_CONTACT } else { 0 });
    }
    // The tire dust: gated at 15 ft/s, 100 to 250 a second before the compression's factor.
    let td = &fx.entries[5];
    assert_eq!((td.emt.min_speed, td.emt.rate, td.emt.life), (15.0, (100.0, 250.0), (600.0, 900.0)));
    assert!(!td.emt.matches_terrain && !td.emt.has_drag);
    // The prop wash: 35 to 45 a second, no gate, on the ground under it.
    let pw = &fx.entries[8];
    assert_eq!((pw.emt.min_speed, pw.emt.rate, pw.emt.extent), (0.0, (35.0, 45.0), (v3(-5.0, 1.0, -5.0), v3(5.0, 3.0, 1.0))));
    assert!(pw.emt.matches_terrain && pw.emt.apply_emitter_velocity && pw.emt.fades_in);

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

    // Rolled along the ground at 20 ft/s on all three tires, the prop 8 ft up: every tire raises its
    // dust past the 15 ft/s gate, and the prop its wash, on the ground.
    let mut fx = PlaneFx::new(&list, fx.atlas.clone(), read);
    let nodes = ["biplane_FtireL_particles", "biplane_FtireR_particles", "biplane_Rtire_particles", "PropDummy"];
    for f in 0..15 {
        let x = 20.0 * DT * f as f32;
        let nodes = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let mut m = M4::IDENTITY;
                m.0[3] = [x, if i == 3 { 8.0 } else { 0.0 }, i as f32 * 4.0, 1.0];
                (n.to_string(), m)
            })
            .collect();
        let frame = PlaneFrame { nodes, tires: vec![TireFrame { ground: true, compression: 0.1, ..TireFrame::default() }; 3], ..PlaneFrame::default() };
        fx.step_over(&frame, DT, &mut |_| Some((0.0, 0)));
    }
    let counts = fx.counts();
    for name in ["Front Left Tire Dust", "Front Right Tire Dust", "Rear Tire Dust", "Dust"] {
        assert!(counts.iter().any(|c| c.0 == name && c.1 > 0), "{name}: {counts:?}");
    }
    let wash_index = fx.entries.iter().position(|e| e.name == "Dust").unwrap();
    assert!(fx.particles.iter().filter(|p| p.emitter == wash_index).all(|p| p.at.y < 8.0), "the wash is on the ground");
}


/// A tire for the tire dust.
fn tire(ground: bool, object: bool, compression: f32, zone: u8) -> TireFrame {
    TireFrame { ground, object, compression, zone }
}

/// The wheel node rolling along x at 30 ft/s, frame `f` of it, on `t`.
fn rolling(f: usize, t: TireFrame, wrecked: bool) -> PlaneFrame {
    let mut m = M4::IDENTITY;
    m.0[3] = [30.0 * DT * f as f32, 0.0, 0.0, 1.0];
    PlaneFrame { nodes: vec![("wheel".to_string(), m)], tires: vec![t], wrecked, ..PlaneFrame::default() }
}

/// What the wheel dust alone has in the air after riding `tires`, a frame each (all of it: its
/// particles live 1.5 s).
fn dust_after(tires: &[TireFrame], wrecked: bool) -> PlaneFx {
    let mut fx = only(fx(), "Wheel Dust");
    for (f, &t) in tires.iter().enumerate() {
        fx.step(&rolling(f, t, wrecked), DT);
    }
    fx
}

#[test]
fn the_tire_dust_rides_its_tires_contact_and_compression() {
    let n = |tires: &[TireFrame]| dust_after(tires, false).particles.len();
    // In the air, or on the ground with the strut not past MinCompressionThreshold (0.1): nothing.
    assert_eq!(n(&[tire(false, false, 0.5, 0); 30]), 0);
    assert_eq!(n(&[tire(true, false, 0.1, 0); 30]), 0, "the threshold is a strict 'past'");
    // On it: the file's 30 a second times ModifierBase + ModifierScale x c, 0.5 + 5 c. The first
    // frame on has not moved, which the gate (2 ft/s) reads as standing still.
    let at_02 = n(&[tire(true, false, 0.2, 0); 30]);
    assert!((42..=45).contains(&at_02), "x1.5: {at_02}");
    let at_03 = n(&[tire(true, false, 0.3, 0); 30]);
    assert!((56..=59).contains(&at_03), "x2: {at_03}");
    // A ground contact latches it: it keeps raising dust with the strut compressed and no contact,
    // until an object contact unlatches it.
    let mut latched = vec![tire(true, false, 0.3, 0); 10];
    latched.extend([tire(false, false, 0.3, 0); 20]);
    assert!((56..=59).contains(&n(&latched)), "{}", n(&latched));
    let mut unlatched = vec![tire(true, false, 0.3, 0); 10];
    unlatched.push(tire(false, true, 0.3, 0));
    unlatched.extend([tire(false, false, 0.3, 0); 19]);
    // The object frame is also its tire's ObjectContact, which the entry's Disable switches off.
    assert!((17..=19).contains(&n(&unlatched)), "{}", n(&unlatched));
    // ObjectContact on the tire and the plane's wreck are in its Disable line.
    assert_eq!(n(&[tire(true, true, 0.3, 0); 30]), 0);
    assert_eq!(dust_after(&[tire(true, false, 0.3, 0); 30], true).particles.len(), 0);
    // A tire the frame does not have is no dust.
    let mut fx = only(fx(), "Wheel Dust");
    for f in 0..30 {
        fx.step(&PlaneFrame { tires: Vec::new(), ..rolling(f, tire(true, false, 0.3, 0), false) }, DT);
    }
    assert!(fx.particles.is_empty());
}

#[test]
fn the_tire_dust_bursts_past_the_burst_compression() {
    let keys = DustKeys::default();
    assert_eq!((keys.min_compression, keys.burst_compression, keys.burst), (0.3, 0.85, true));
    assert_eq!((keys.growth, keys.velocity, keys.base, keys.scale, keys.burst_scale), (2.0, 5.0, 0.5, 5.0, 5.0));
    let file = Emt {
        rate: (100.0, 250.0),
        growth: (1.8, 3.5),
        noise: (v3(-1.0, -0.5, 0.25), v3(1.0, 1.0, 0.5)),
        ..Emt::default()
    };
    let mut emt = file.clone();
    keys.apply(&file, 0.5, &mut emt);
    let k = 0.5f32 + 5.0 * 0.5;
    assert_eq!(emt.rate, (100.0 * k, 250.0 * k));
    assert_eq!((emt.growth, emt.noise), (file.growth, file.noise));
    keys.apply(&file, 1.0, &mut emt);
    let k = 0.5f32 + 5.0 * 1.0 + 5.0 * 1.0;
    assert_eq!(emt.rate, (100.0 * k, 250.0 * k));
    let low = 1.8f32 * 2.0;
    assert_eq!(emt.growth, (low, low + (3.5 - 1.8)), "the low end doubled, the span the file's");
    // The x bounds five times over, and the top y the file's top z five times over.
    assert_eq!(emt.noise, (v3(-5.0, -0.5, 0.25), v3(5.0, 2.5, 0.5)));
    // Back under it, the file's again.
    keys.apply(&file, 0.5, &mut emt);
    assert_eq!((emt.growth, emt.noise), (file.growth, file.noise));
    // With UseBurstDust off the growth and the noise are never touched.
    let plain = DustKeys { burst: false, ..DustKeys::default() };
    let mut emt = Emt { growth: (9.0, 9.0), ..file.clone() };
    plain.apply(&file, 1.0, &mut emt);
    assert_eq!((emt.growth, emt.rate), ((9.0, 9.0), (100.0 * 5.5, 250.0 * 5.5)));
}

#[test]
fn a_zone_picks_the_dusts_group() {
    let fx = fx();
    let wheel = fx.entries.iter().find(|e| e.name == "Wheel Dust").unwrap();
    assert_eq!(wheel.zones.len(), 16);
    assert_eq!(&wheel.zones[..4], [Zone::Group(2, 3), Zone::Group(0, 1), Zone::Off, Zone::Group(2, 3)]);
    // An entry naming no descriptor reads [Default], which names nothing.
    assert!(fx.entries[1].zones.iter().all(|z| *z == Zone::Group(0, 1)));
    // A group the sheet does not have is its first; no file at all leaves every zone the .emt's.
    let atlas = Atlas::parse(SHEET, 64, 64);
    assert_eq!(particles::zones(Some("[G]\nZone1=\"Nope\"\n"), "g", &atlas, (2, 3))[0], Zone::Group(0, 1));
    assert!(particles::zones(None, "G", &atlas, (2, 3)).iter().all(|z| *z == Zone::Group(2, 3)));

    // Over the second surface the tire dust draws the puff's frames; over the third ("none") its
    // own frame turns it on regardless, with the group it had.
    let fx = dust_after(&[tire(true, false, 0.3, 1); 30], false);
    assert!(!fx.particles.is_empty() && fx.particles.iter().all(|p| p.frame <= 1));
    let fx = dust_after(&[tire(true, false, 0.3, 2); 30], false);
    assert!(!fx.particles.is_empty() && fx.particles.iter().all(|p| (2..=3).contains(&p.frame)));
}

/// The prop at `height` over flat ground at `floor`, frame `f` of flying along x at 30 ft/s, on
/// surface `zone`.
fn wash_after(height: f32, floor: f32, zone: u8, frames: usize, wrecked: bool) -> PlaneFx {
    let mut fx = only(fx(), "Wash");
    for f in 0..frames {
        let mut m = M4::IDENTITY;
        m.0[3] = [30.0 * DT * f as f32, floor + height, 0.0, 1.0];
        let frame = PlaneFrame { nodes: vec![("prop".to_string(), m)], wrecked, ..PlaneFrame::default() };
        fx.step_over(&frame, DT, &mut |_| Some((floor, zone)));
    }
    fx
}

#[test]
fn the_prop_wash_fades_with_height() {
    // Under BeginFadeHeight (20 ft), the file's 30 a second: 30 frames, a second.
    let low = wash_after(10.0, 5.0, 0, 30, false).particles.len();
    assert!((29..=30).contains(&low), "{low}");
    // Half way to EndFadeHeight (25 ft), half the rate.
    let half = wash_after(22.5, 5.0, 0, 30, false).particles.len();
    assert!((14..=15).contains(&half), "{half}");
    // At or over it, nothing; nothing while wrecked; nothing with no ground under it.
    assert_eq!(wash_after(25.0, 5.0, 0, 30, false).particles.len(), 0);
    assert_eq!(wash_after(10.0, 5.0, 0, 30, true).particles.len(), 0);
    let mut fx = only(fx(), "Wash");
    for f in 0..30 {
        let mut m = M4::IDENTITY;
        m.0[3] = [f as f32, 10.0, 0.0, 1.0];
        fx.step(&PlaneFrame { nodes: vec![("prop".to_string(), m)], ..PlaneFrame::default() }, DT);
    }
    assert!(fx.particles.is_empty(), "step asks no ground");
    // MatchTerrainContour: every particle starts on the ground, raised by its extent's two feet,
    // however high the prop is.
    let fx = wash_after(15.0, 5.0, 0, 30, false);
    assert!(fx.particles.iter().all(|p| p.at.y == 7.0), "{:?}", fx.particles.first());
    // The zones: the second surface's group, and nothing over the third.
    assert!(wash_after(10.0, 5.0, 1, 30, false).particles.iter().all(|p| p.frame <= 1));
    assert_eq!(wash_after(10.0, 5.0, 2, 30, false).particles.len(), 0);
}
