//! **V64:** a trick started while the bike is whipped keeps the rider on the bike.
//!
//! The whip is the airborne row's crossup: the bike's `bike_air_*` clips at the phase axis turn the
//! chassis' root, and the rider's `air_*` clips at the same axis turn its pelvis with it. A trick
//! switches the rider to the trick's clip; these rides start one of each kind while whipped — a
//! gesture slot, a trick pack's extra, a 360 — with the body and the chassis stepped the way the
//! shell steps them, and measure the rider against the bike every frame.
//!
//! **V69:** a 360's fade back to the airborne row keeps the drawn hands on the bars.

use game_render::anim::PackClips;
use game_render::chassis::grips;
use game_render::disc::Disc;
use game_render::mat::{M4, V3, v3};
use game_render::rider::{Body, HAND_ANCHORS, Sex};
use game_render::scene::Vehicle;
use game_render::vehicle::{Chassis, Drive, FRONT, REAR, params};
use game_render::ini;
use game_sim::ride::Controls;
use game_sim::trick::{Set as Stunts24, Stunts};
use game_sim::collide::{Bundle, Record};
use game_sim::world::World;
use game_sim::{phy, phy::Phy, rider, trick_pack};
use ps2_float::{F, Vf};
use std::path::PathBuf;

const A_RIDER: &str = "thq1";
const A_BIKE: &str = "250sx";
const STUNT_ARCHIVE: &str = "appstart.res";
const STUNT_TABLE: &str = "rider\\anim\\stunts.ini";
const FRAME: F = F(1.0 / 60.0);

fn pack() -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../mods/tricks");
    let mut folders: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    folders.sort();
    let found = folders.into_iter().find(|f| {
        std::fs::read_to_string(f.join("mod.json")).is_ok_and(|t| t.contains("\"kind\": \"tricks\""))
    });
    if found.is_none() {
        println!("SKIPPED: no trick pack in {} (openmx-mods' tools/import-tricks makes one)", dir.display());
    }
    found
}

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            println!("SKIPPED: {why}");
            None
        }
    }
}

fn member(disc: &Disc, ends: &str) -> String {
    let want = ends.to_ascii_lowercase();
    disc.entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(&want))
        .min_by_key(|e| e.name.len())
        .map(|e| e.name.clone())
        .unwrap_or_else(|| panic!("the disc has no {ends}"))
}

/// Flat ground with `tests/ride.rs`'s flip ramp across +z: 108 feet of run-up, then 25 feet at a
/// slope of a half.
struct Ramp;

impl World for Ramp {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let (start, slope, length) = (108.0, 0.5, 25.0);
        let t = at.z() - start;
        if !(0.0..=length).contains(&t) {
            return (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0));
        }
        let n = 1.0 / (1.0f32 + slope * slope).sqrt();
        (Vf::new(at.x(), t * slope, at.z(), 1.0), Vf::new(0.0, n, -slope * n, 0.0))
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// Everything a ride on the pack needs: the bike with the pack laid over it, the set with its
/// extras, and the body and chassis with the pack's clips.
struct Kit {
    tables: rider::Tables,
    phy: Phy,
    set: Stunts24,
    body: Body,
    chassis: Chassis,
    /// The stunts the pack has no bike clip for.
    bike_lacks: Vec<String>,
}

fn kit() -> Option<Kit> {
    let folder = pack()?;
    let mut disc = disc()?;
    let tables = rider::Tables::build();
    let res = disc.res(&member(&disc, STUNT_ARCHIVE)).expect("the app archive");
    let set = Stunts24::new(Stunts::parse(&res.read_suffix(STUNT_TABLE).unwrap()).unwrap().selection(0));
    let rows = trick_pack::table(&std::fs::read(folder.join("Rider/Anim/MX_Rider_Stunts.ini")).unwrap()).unwrap();
    let extras = trick_pack::extras(&rows, &set);
    let set = set.with_extras(extras);

    let bike = disc.res(&member(&disc, &format!("{A_BIKE}.res"))).expect("the bike archive");
    let phy_entry = bike.find_suffix(".phy").expect("a .phy").name.clone();
    let mut p = Phy::read_bytes(&bike.read(&phy_entry).unwrap(), &tables.ride.curve);
    let pack_phy = std::fs::read_dir(folder.join("Veh"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|f| f.file_name().unwrap().to_string_lossy().to_ascii_lowercase() == trick_pack::phy_name_for(&phy_entry))
        .unwrap_or_else(|| panic!("the pack has no {}", trick_pack::phy_name_for(&phy_entry)));
    let ini = phy::Ini::parse(&std::fs::read_to_string(pack_phy).unwrap());
    let moves = trick_pack::moves(&ini, &rows).expect("the pack's moves");
    trick_pack::apply(&mut p, &ini, moves);

    let clips = PackClips::open(&folder, "MX").unwrap();
    let rider_res = disc.res(&member(&disc, &format!("{A_RIDER}.res"))).expect("the rider archive");
    let mut body = Body::load(vec![rider_res], A_RIDER, Sex::Male, "", &set.stunts, p.motion()).expect("the body");
    body.add_pack(&clips, &set.stunts);
    let sub = Vehicle::parse(&ini::text(&bike.read_suffix(&format!("{A_BIKE}.sub")).unwrap()));
    let mut chassis = Chassis::load(vec![bike], &sub.mcf, "bike_", &set.stunts, params()).expect("the chassis");
    let bike_lacks = chassis.add_pack(&clips, &set.stunts);
    Some(Kit { tables, phy: p, set, body, chassis, bike_lacks })
}

/// One frame as drawn: the body's state, the chassis', and the rider against the bike, in the
/// chassis root's own frame — which is the bike as drawn, whip and all.
#[derive(Debug, Clone, Copy)]
struct Seen {
    on_ground: bool,
    body: i32,
    bike: i32,
    /// The rider's two hand anchors as the clips posed them, and the chassis' two grips.
    hands: [V3; 2],
    grips: [V3; 2],
    /// The two hand anchors as the shell draws them: the clip pose held on to the grips
    /// ([`Body::palette`], which both renderers draw with) at the body's own hold.
    drawn: [V3; 2],
    /// How hard the drawn rider holds on ([`Body::holds`]' hands): 1 while riding.
    hold: f32,
    /// The rider's pelvis.
    pelvis: V3,
}

impl Seen {
    /// How far each hand is off its grip, in feet.
    fn off(&self) -> f32 {
        (0..2).map(|i| self.hands[i].sub(self.grips[i]).len()).fold(0.0, f32::max)
    }

    /// How far each drawn hand is off its grip, in feet.
    fn drawn_off(&self) -> f32 {
        (0..2).map(|i| self.drawn[i].sub(self.grips[i]).len()).fold(0.0, f32::max)
    }

    /// How far this frame's rider is from `other`'s on the bike, in feet: the pelvis against the
    /// chassis' root and each hand against its own grip — the bars move with the whip — the
    /// furthest of the three.
    fn apart(&self, other: &Seen) -> f32 {
        let off = |s: &Seen, i: usize| s.hands[i].sub(s.grips[i]);
        let hands = (0..2).map(|i| off(self, i).sub(off(other, i)).len()).fold(0.0, f32::max);
        hands.max(self.pelvis.sub(other.pelvis).len())
    }
}

fn at(m: &M4) -> V3 {
    v3(m.0[3][0], m.0[3][1], m.0[3][2])
}

/// A point out of a rigid transform's frame.
fn into(m: &M4, p: V3) -> V3 {
    let q = p.sub(at(m));
    v3(
        q.x * m.0[0][0] + q.y * m.0[0][1] + q.z * m.0[0][2],
        q.x * m.0[1][0] + q.y * m.0[1][1] + q.z * m.0[1][2],
        q.x * m.0[2][0] + q.y * m.0[2][1] + q.z * m.0[2][2],
    )
}

/// The shell's frame, call for call (`main.rs`): the ride, the body ([`Body::ride`]), the chassis
/// ([`Chassis::want_beside`] the body's state, at [`Chassis::riding`]'s request), with `script`'s
/// hands from each tick on and the gas held. Wrecks off: what is measured is the rider on the bike.
fn ride(kit: &mut Kit, script: &[(usize, Controls)], frames: usize) -> Vec<Seen> {
    let p = &kit.phy;
    // The chassis model's own wheel nodes, which is what the shell hands the simulation (S22).
    let node = |name: &str| kit.chassis.node_rest(name).map(|v| Vf::new(v.x, v.y, v.z, 1.0));
    let mounts = (node(FRONT).expect("a TireFront"), node(REAR).expect("a TireRear"));
    let mut r = rider::Rider::new(p, mounts, Vf::new(0.0, 2.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), kit.set.clone());
    r.sense.wrecks_enabled = false;
    r.body_rates = params();
    let mut world = Ramp;
    let mut now = Controls::default();
    let skel = kit.body.rig.set.skeleton.clone();
    let pelvis = skel.find("Pelvis").expect("a pelvis");
    let hands = HAND_ANCHORS.map(|n| skel.find(n).expect("a hand anchor"));
    let root = kit.chassis.nodes.root.expect("a chassis root");
    let mut out = Vec::new();
    for k in 0..frames {
        if let Some((_, c)) = script.iter().find(|(at, _)| *at == k) {
            now = c.clone();
        }
        let c = Controls { gas: F::ONE, ..now.clone() };
        r.frame(p, &kit.tables, &c, FRAME, &mut world);
        kit.body.ride(FRAME, &mut r, &c, rider::Gate::default());
        let want = Chassis::want_beside(&r.ride, &r.tricks, r.body.veh.want);
        kit.chassis.frame(FRAME, want, &Chassis::riding(&r), Drive::of(&r.ride));
        let world = kit.body.rig.world();
        let g = grips(&kit.chassis);
        let (drawn, _) = kit.body.palette(Some(&g));
        let bike = kit.chassis.world[root];
        let grip = |i: usize| into(&bike, g[2 + i].expect("a grip"));
        out.push(Seen {
            on_ground: r.ride.air.on_ground,
            body: r.body.veh.state,
            bike: kit.chassis.stance.driver.state,
            hands: hands.map(|j| into(&bike, at(&world[j]))),
            grips: [grip(0), grip(1)],
            drawn: hands.map(|j| into(&bike, at(&drawn[j]))),
            hold: kit.body.holds()[2].min(kit.body.holds()[3]),
            pelvis: into(&bike, at(&world[pelvis])),
        });
    }
    out
}

/// When the tape whips: the stick hard right from just after take-off (the ramp's lip is left at
/// about frame 178) until the trick, which crosses the bike up a quarter of the sweep.
const WHIP: usize = 190;
/// When the trick is asked for, half a second into the flight and well before the landing (~305).
const TRICK: usize = 215;
/// The ride's length: past the landing, which ends what is measured.
const FRAMES: usize = 320;

/// How far the rider of a trick started whipped may be from the same trick's started square, on
/// the bike, once the trick's fade in is over: an inch and a half. Measured (V64): 0.000 ft for a
/// gesture slot and for the pack's extras, both the one the bike has a clip for and the one it has
/// not; the frame before the trick the two are 0.43 ft apart, which is the whip, and the fade closes
/// it. Before the fix the clipless extra held the bike crossed up under its clip and came 1.9 ft
/// apart.
const APART: f32 = 0.125;

/// How many frames a trick's fade in takes to have the whip out of the pose: a quarter second.
/// Measured (V64): the gesture slot's gap closes from 0.43 ft to 0 by its fourteenth frame.
const FADE: usize = 15;

/// How much the gap between the two riders may grow in one frame while the fade closes it: none
/// beyond rounding. Before the fix the clipless extra's bike snapped square on the trick's first
/// frame and the gap jumped by 0.55 ft.
const STEP: f32 = 0.01;

/// How far a drawn hand may be off its grip through a 360 and its exit, any frame: a hundredth of
/// a foot, which is the hold's own rounding. Measured (V69): 0.000 ft on every frame, where the
/// clips alone take a hand up to 0.82 ft off on the exit's fade. The original's rider keeps both
/// hands on the bar ends through the same exit while its body swings back onto the seat (PCSX2,
/// MX vs. ATV Unleashed, M21's state and gesture, by eye).
const SPIN_HELD: f32 = 0.01;

fn hands(trick: [bool; 3], extra: [bool; 2], stick: (f32, f32)) -> Controls {
    Controls { trick, stunt: trick.iter().any(|&b| b), extra, stick: (F(stick.0), F(stick.1)), ..Controls::default() }
}

/// **The brief's ride:** off the flip ramp, whipped, then a gesture slot, a pack extra the bike has
/// its own clip for, one it has not, and a 360. Through every one of them the bike runs the state
/// the body runs, and the rider sits on the bike where the same trick started square puts it, every
/// frame, within [`APART`] — the pelvis over the seat and the hands where that trick has them,
/// which for a clip that keeps them on is the grips. The 360 is [`a_360_keeps_the_hands_on_the_bars`].
#[test]
fn a_whipped_trick_keeps_the_rider_on_the_bike() {
    let Some(first) = kit() else { return };
    let whip = hands([false; 3], [false; 2], (1.0, 0.0));
    let tricks = [
        ("a gesture slot (Up on the first button)", hands([true, false, false], [false; 2], (0.0, -1.0))),
        ("9oclockindianair, Cross Up", hands([false; 3], [true, false], (0.0, -1.0))),
        ("cliffhanger, Cross Left", hands([false; 3], [true, false], (-1.0, 0.0))),
    ];
    assert_eq!(first.set.stunts[game_sim::trick::SLOTS + 2].name, "cliffhanger", "Cross Left is cliffhanger");
    assert!(first.bike_lacks.iter().any(|n| n == "cliffhanger"), "the bike has a cliffhanger clip: {:?}", first.bike_lacks);
    assert!(!first.bike_lacks.iter().any(|n| n == "9oclockindianair"), "the bike has no 9oclockindianair clip");

    let fresh = || kit().expect("the kit loaded once");
    for (name, trick) in tricks {
        let rest = Controls::default();
        let whipped = ride(&mut fresh(), &[(WHIP, whip.clone()), (TRICK, trick.clone()), (TRICK + 25, rest.clone())], FRAMES);
        let square = ride(&mut fresh(), &[(TRICK, trick), (TRICK + 25, rest)], FRAMES);
        let started = whipped[TRICK].body;
        assert!(started != whipped[TRICK - 1].body && started != game_sim::vehicle_anim::AIR,
                "{name}: no trick started at {TRICK}: {} then {}", whipped[TRICK - 1].body, started);
        // Up to the landing: wrecks are off, and a trick still running there would have put the
        // rider down.
        let down = TRICK + whipped[TRICK..].iter().position(|s| s.on_ground).expect("a landing");
        // The frame before the trick the two riders differ by the whip itself; the trick's fade
        // takes that away, so through it the gap only closes, and past it the two are together.
        let whip_gap = whipped[TRICK - 1].apart(&square[TRICK - 1]);
        let (mut worst, mut last) = ((0.0f32, 0usize), whip_gap);
        for k in TRICK..down {
            let (w, s) = (&whipped[k], &square[k]);
            assert_eq!(w.bike, w.body, "{name} at {k}: the bike runs {} under a body in {}", w.bike, w.body);
            assert_eq!(w.body, s.body, "{name} at {k}: the two rides are in different states");
            let apart = w.apart(s);
            assert!(apart <= last + STEP, "{name} at {k}: the whipped rider jumped from {last:.3} to {apart:.3} ft off the square one's place");
            last = apart;
            if k >= TRICK + FADE && apart > worst.0 {
                worst = (apart, k);
            }
        }
        println!("{name}: whipped against square {whip_gap:.3} ft apart before the trick, at worst {:.3} past its fade, at {}",
                 worst.0, worst.1);
        assert!(worst.0 <= APART, "{name}: the whipped rider came {:.3} ft off the square one's place, at {}", worst.0, worst.1);
    }
}

/// **V69:** a 360 and its fade back to the airborne row, with MX vs. ATV's gesture (MD30: R1 held
/// into the lip, the stick left then right on it, centred). The clips alone part the hands from the
/// grips — 0.3 to 0.4 ft while the 360's clips play and up to 0.82 on the exit's fade, because each
/// skeleton blends its own two poses and a blend of two poses with the hands on the bars is not a
/// pose with the hands on the bars — and the drawn rider is held on: the body is riding through
/// all of it ([`game_sim::rider_anim::Mode::Riding`], the 360 is no pose off the clip table), so
/// the hold is full and the hands are drawn on the grips every frame, within [`SPIN_HELD`].
#[test]
fn a_360_keeps_the_hands_on_the_bars() {
    let Some(mut kit) = kit() else { return };
    let r1 = hands([false, false, true], [false; 2], (0.0, 0.0));
    let left = hands([false, false, true], [false; 2], (-1.0, 0.0));
    let right = hands([false, false, true], [false; 2], (1.0, 0.0));
    let spin = ride(&mut kit, &[(130, r1.clone()), (150, left), (158, right), (166, r1), (180, Controls::default())], FRAMES);
    let turning: Vec<usize> = (0..FRAMES).filter(|&k| spin[k].body == game_sim::vehicle_anim::SPIN_RIGHT).collect();
    assert!(turning.len() > 30, "the body played threesixtyR for {} frames", turning.len());
    let from = turning[0];
    let down = from + spin[from..].iter().position(|s| s.on_ground).expect("a landing");
    // The exit: the 360's clips over, the airborne row fading back in, still in the air.
    let exit: Vec<usize> = (from..down)
        .filter(|&k| spin[k].body == game_sim::vehicle_anim::AIR && spin[k - 1].body != game_sim::vehicle_anim::AIR)
        .collect();
    assert!(!exit.is_empty(), "the 360's clip never faded back to the airborne row before the landing at {down}");
    let (mut clips, mut drawn) = ((0.0f32, 0usize), (0.0f32, 0usize));
    for (k, s) in spin.iter().enumerate().take(down).skip(from) {
        assert_eq!(s.bike, s.body, "the 360 at {k}: the bike runs {} under a body in {}", s.bike, s.body);
        assert_eq!(s.hold, 1.0, "the 360 at {k}: the drawn rider lets go of the bars (state {})", s.body);
        if s.off() > clips.0 {
            clips = (s.off(), k);
        }
        if s.drawn_off() > drawn.0 {
            drawn = (s.drawn_off(), k);
        }
    }
    println!("the 360: {} frames of threesixtyR from {from}, the exit's fade from {:?}, landing at {down}; the clips take a hand {:.2} ft off its grip at {}, the drawn rider {:.3} at {}",
             turning.len(), exit, clips.0, clips.1, drawn.0, drawn.1);
    // What the hold is holding against: the exit's blend parts the clips' hands from the bars.
    assert!(clips.0 > 0.5 && exit.iter().any(|&e| (e..e + 15).contains(&clips.1)),
            "the clips' worst is no longer the exit's fade: {:.2} ft at {}", clips.0, clips.1);
    assert!(drawn.0 <= SPIN_HELD, "the 360 drew a hand {:.3} ft off its grip, at {}", drawn.0, drawn.1);
}
