//! The animation seam: a clip read off the user's own disc, bound to the skeleton it names, and
//! posing it under the simulation's own driving.
//!
//! The three halves this joins are each proved elsewhere — the driving and the clock against the
//! original (S6, S20), the `.mot` channels and the quaternion blend against the original's samplers
//! (V5) — and what is left is the join itself, which has no oracle: the original builds the joint
//! map inside an allocating loader that reads the disc, which is the one thing `ps2-oracle` cannot
//! run. So the proofs here are against the disc.
//!
//! 1. **the seam, end to end** — a bike archive's own animation set, every clip it ships bound to
//!    the `.psg` the set names, a stunt driven through the simulation's frame, and the bones it
//!    moved.
//! 2. **which clips a bike has** — `stunts.ini`'s `UsedForBike` against the clips the archives
//!    actually ship, slot for slot, over every bike on the disc. Two tables written by different
//!    people that have to agree, and do.
//! 3. **the name rules**, **binding** and **the blend modes** — port only, on fixtures — and the
//!    two blends nothing in the port could draw until V11: the four-clip pose grid and the pair at
//!    two phases, both taken out of the machine that asks for them rather than written by hand.
//!
//! Needs the user's own disc; the disc tests skip with the reason when it is not there.

use game_render::anim::{Bound, Missing, Set};
use game_render::disc::Disc;
use game_render::mat::M4;
use game_render::psg::{Model, Node};
use game_render::rider::{Motion, MotionSet};
use game_render::skin::{Clip, PosChannel, PosKey, Pose, RotChannel, RotKey, SKIP, Skeleton};
use game_sim::motion::{
    Animations, Blend, ClipId, Joints, Library, Params, Player, Request, Slot, Snapshot,
};
use game_sim::trick::{Stunts, Trick};
use game_sim::vehicle_anim;
use ps2_float::F;

/// The bike the port is built against, and the animation set every bike archive carries.
const A_BIKE: &str = "Bike\\250sx.res";
const BIKE_MCF: &str = "bike\\anim\\bike.mcf";
/// `stunts.ini` lives in the container everything else is loaded out of.
const APPSTART: &str = "appstart.res";
const STUNTS: &str = "rider\\anim\\stunts.ini";

/// Thirty frames a second, which is what the clocks are in.
const DT: f32 = 1.0 / 30.0;

fn skip(why: &str) {
    println!("SKIPPED: {why}");
}

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            skip(&why);
            None
        }
    }
}

/// `stunts.ini`'s table, which is where the names of the 24 gesture slots come from.
fn stunts(disc: &mut Disc) -> Stunts {
    let res = disc.res(APPSTART).unwrap_or_else(|e| panic!("{APPSTART}: {e}"));
    let bytes = res.read(STUNTS).unwrap_or_else(|e| panic!("{STUNTS}: {e}"));
    Stunts::parse(&bytes).unwrap_or_else(|e| panic!("{STUNTS}: {e}"))
}

// ---------------------------------------------------------------------------------------------------
// 1 — the seam, end to end
// ---------------------------------------------------------------------------------------------------

/// The brief's own test: a clip read off the disc binds to the skeleton it names and poses it.
///
/// Everything in the chain is the real thing — the archive, the `.mcf`, the `.psg`, the `.mot`, the
/// stunt table, the simulation's selection, clock and clip table — and the only things asserted are
/// the ones the join is responsible for: that the names the simulation asks for find files, that the
/// files' channels find joints of *this* model, and that driving the frame moves those joints and
/// nothing else.
#[test]
fn a_clip_off_the_disc_binds_to_its_skeleton_and_poses_it() {
    let Some(mut disc) = disc() else { return };
    let table = stunts(&mut disc);
    let res = disc.res(A_BIKE).unwrap_or_else(|e| panic!("{A_BIKE}: {e}"));
    let set = Set::load(vec![res], BIKE_MCF).unwrap_or_else(|e| panic!("{e}"));

    // The set's own geometry is the skeleton the clips are bound against: a bike, not a rider.
    assert!(set.skeleton.find("Bike").is_some(), "the bike's own root joint");
    assert!(set.skeleton.len() > 8, "{} joints is not a bike", set.skeleton.len());
    let joints = set.skeleton.len();
    let rest = set.skeleton.world(&Pose::new(joints));

    // The clip table, filled the way the rider's loader fills it: the set's own motions first, then
    // one `bike_<stunt name>` per gesture slot marked for the bike.
    let mut rig = game_render::anim::Rig::new(set, "", &table.stunts, Params::default());

    // The three airborne poses come from the `.mcf`, and all three of this set's point at one file:
    // `bike_air_Mid.MOT` is three motions, which is what the name lookup being by *name* is for.
    let air = [rig.lib.air_fwd, rig.lib.air_mid, rig.lib.air_bck];
    assert!(air.iter().all(Option::is_some), "the airborne poses: {air:?}");
    let members: Vec<&str> = air
        .iter()
        .flatten()
        .map(|&id| rig.set.get(id).map_or("", |b| b.member.as_str()))
        .collect();
    assert_eq!(members[0], members[1], "three names, one clip");
    assert_eq!(members[1], members[2]);
    assert_ne!(air[0], air[1], "and three handles, because the simulation asks by name");

    // Every gesture slot marked for the bike found a clip, and every clip found its joints.
    let (mut bound, mut empty) = (0u32, 0u32);
    for (slot, stunt) in table.stunts.iter().take(game_sim::trick::SLOTS).enumerate() {
        let id = rig.lib.stunt[slot];
        if !stunt.for_bike {
            assert_eq!(id, None, "slot {slot} ({}) is not a bike clip", stunt.name);
            empty += 1;
            continue;
        }
        let id = id.unwrap_or_else(|| panic!("slot {slot} ({}) found no clip", stunt.name));
        let clip = rig.set.get(id).expect("a handle the set gave out");
        assert!(clip.clip.duration() > 0.0, "{}: a clip of no length", clip.member);
        assert_eq!(
            clip.named(),
            clip.joints.len(),
            "{}: {} of its {} channels are not joints of the bike it was bound to",
            clip.member,
            clip.joints.len() - clip.named(),
            clip.joints.len()
        );
        assert!(clip.joints.iter().all(|&j| j == SKIP || (j as usize) < joints));
        bound += 1;
    }
    assert!(bound >= 5, "only {bound} of the gesture slots have a bike clip");
    assert!(empty > 0, "every gesture slot had a clip, so the empty ones prove nothing");
    assert!(rig.set.missing.iter().all(|m| matches!(m, Missing::NoMember(_))), "{:?}", rig.set.missing);

    // Drive a stunt the way the game does: the trick's progress runs 0 to 1 over its duration, and
    // the selection plays that slot's clip at exactly that phase.
    let slot = table
        .stunts
        .iter()
        .take(game_sim::trick::SLOTS)
        .position(|s| s.for_bike)
        .expect("a bike stunt");
    let mut moved = vec![false; joints];
    let (mut frames, mut off_rest) = (0u32, 0u32);
    for step in 0..=10u32 {
        let progress = F(step as f32 / 10.0);
        let r = Request {
            action: Trick(slot as u8),
            seq: 1,
            prev_seq: if step == 0 { 0 } else { 1 },
            progress,
            ..Request::default()
        };
        let (frame, _) = rig.frame(F(DT), r, Default::default());
        assert_eq!(frame.blend.mode(), 1, "a gesture slot plays one clip");
        let want = rig.set.timing(rig.lib.stunt[slot]).duration * progress;
        assert_eq!(rig.player.sample_time(frame.blend), want, "the pose is sampled at the clock");

        let world = rig.world();
        assert_eq!(world.len(), joints);
        for (j, m) in world.iter().enumerate() {
            assert!(
                m.0.iter().flatten().all(|v| v.is_finite()),
                "slot {slot} at {progress:?}: joint {j}'s matrix is {m:?}"
            );
            if *m != rest[j] {
                moved[j] = true;
                off_rest += 1;
            }
        }
        frames += 1;
    }
    let moved_count = moved.iter().filter(|&&m| m).count();
    assert!(moved_count > 0, "the stunt moved no joint off the bike's rest pose");
    assert!(off_rest > 10, "only {off_rest} joint-frames differ from the rest pose");

    // Only the joints the clip names were written, which is what the joint map is for.
    let written = rig.set.pose.turned.iter().filter(|&&t| t).count();
    let names = rig.set.get(rig.lib.stunt[slot].unwrap()).unwrap().joints.len();
    assert_eq!(written, names, "the clip wrote joints it has no channel for");
    assert!(written < joints, "the clip writes every joint, so the map proves nothing");

    // The airborne idle: two clips mixed, which is the other blend the rider ever asks for.
    let lean = game_sim::motion::Pose { fore_aft: F(0.75), ..Default::default() };
    let r = Request { action: Trick::WHEELIE, seq: 2, prev_seq: 1, ..Request::default() };
    let (frame, _) = rig.frame(F(DT), r, lean);
    assert_eq!(frame.blend.mode(), 2, "the airborne idle mixes two clips");
    let world = rig.world();
    assert!(world.iter().flat_map(|m| m.0.iter().flatten()).all(|v| v.is_finite()));

    // And what the simulation sees of all this is the one pose, through `Joints`.
    let mut snapshot = Snapshot::default();
    snapshot.take(&rig.set.pose, None, 0);
    assert_eq!(snapshot.rotation.len(), joints);
    for (j, q) in snapshot.rotation.iter().enumerate() {
        assert_eq!(q.map(|v| v.0), rig.set.pose.rot[j], "joint {j} reads back differently");
    }

    println!(
        "the seam: {A_BIKE}'s {} clips bound to {joints} bike joints, {bound} gesture slots with a \
         clip and {empty} without, {frames} driven frames, {moved_count} joints moved",
        rig.set.clips.len()
    );
}

// ---------------------------------------------------------------------------------------------------
// 2 — which clips a bike has
// ---------------------------------------------------------------------------------------------------

/// `stunts.ini` says which stunts the *bike* animates; the bike archives ship a clip for exactly
/// those and no others.
///
/// This is the rule the loader leans on written down twice by two different people, and it holds on
/// all 24 gesture slots of every bike on the disc. It is also the proof that the second name rule —
/// `<ContentDirectory><name>.mot`, no `.mcf` entry anywhere — is real: not one of these clips is
/// listed in a bike's `.mcf`, and the flag that says they exist is in a different file again.
///
/// The disc has two families of bike. The race bikes carry `bike\anim\bike.mcf` and clips called
/// `bike_<stunt>`; the freestyle minibikes carry `bike50.mcf` and the same clips called
/// `ff_bike_<stunt>`, which is the rider prefix [`Library::stunt_name`] puts in front of every name
/// it asks for. Both families are walked here, each with its own prefix, and the same flag decides
/// both.
#[test]
fn the_bike_clips_on_the_disc_are_the_ones_the_table_marks() {
    let Some(mut disc) = disc() else { return };
    let table = stunts(&mut disc);
    let archives: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let l = n.to_ascii_lowercase();
            l.starts_with("bike\\") && l.ends_with(".res")
        })
        .collect();
    assert!(archives.len() > 20, "only {} bike archives", archives.len());

    let (mut checked, mut marked, mut loaded, mut freestyle) = (0u32, 0u32, 0u32, 0u32);
    let mut no_geometry: Vec<String> = Vec::new();
    for name in &archives {
        let res = disc.res(name).unwrap_or_else(|e| panic!("{name}: {e}"));
        let held: Vec<String> = res.entries().iter().map(|e| e.name.to_ascii_lowercase()).collect();
        let mcf = held
            .iter()
            .find(|m| m.ends_with(".mcf"))
            .unwrap_or_else(|| panic!("{name}: a bike with no animation set"))
            .clone();
        // Which family: the minibike's own set, and the rider prefix that goes with it.
        let prefix = if held.iter().any(|m| m.contains("\\ff_bike_")) { "ff_" } else { "" };
        assert_eq!(
            mcf.ends_with("bike50.mcf"),
            prefix == "ff_",
            "{name}: {mcf} and the {prefix:?} clips should go together"
        );
        if prefix == "ff_" {
            freestyle += 1;
        }

        // The set is only needed for the loader half: a bike whose geometry is not in its own
        // archive (the ghost bikes borrow one) still has to agree with the table.
        let mut set = match Set::load(vec![res], &mcf) {
            Ok(set) => {
                loaded += 1;
                Some(set)
            }
            Err(why) => {
                no_geometry.push(format!("{name}: {why}"));
                None
            }
        };
        for stunt in table.stunts.iter().take(game_sim::trick::SLOTS) {
            let asked = Library::stunt_name(prefix, &stunt.name);
            let want = format!("bike\\anim\\{}.mot", asked.to_ascii_lowercase());
            let ships = held.iter().any(|m| *m == want);
            assert_eq!(
                ships,
                stunt.for_bike,
                "{name}: the archive {} a clip for {}, and stunts.ini says UsedForBike={}",
                if ships { "ships" } else { "has no" },
                stunt.name,
                stunt.for_bike
            );
            // And the loader finds exactly the ones that are there, by the second rule.
            if let Some(set) = set.as_mut() {
                set.load(&asked);
                assert_eq!(set.bound(&asked).is_some(), ships, "{name}: {asked}");
                assert!(set.set.clip(&asked).is_none(), "{asked} is in no .mcf: it is found by name");
            }
            if ships {
                marked += 1;
            }
            checked += 1;
        }
    }
    assert!(loaded * 2 > archives.len() as u32, "only {loaded} archives carry their own geometry");
    assert!(freestyle > 0, "no freestyle minibike, so the rider prefix was never exercised");
    println!(
        "the bike's clips: {checked} slots over {} bike archives ({freestyle} freestyle, {loaded} \
         with their own geometry), {marked} shipped and marked UsedForBike, none of them listed in \
         any .mcf{}",
        archives.len(),
        if no_geometry.is_empty() { String::new() } else { format!("; no geometry: {no_geometry:?}") }
    );
}

// ---------------------------------------------------------------------------------------------------
// 3 — the plumbing, port only
// ---------------------------------------------------------------------------------------------------

/// The three rules that turn a name into an archive member, in order.
#[test]
fn a_name_becomes_a_member_by_the_set_then_by_itself() {
    let set = Set::new(
        MotionSet {
            directory: "Bike\\Anim\\".to_string(),
            motions: vec![
                Motion {
                    name: "bike_air_Fwd".to_string(),
                    clip: "bike_air_Mid.MOT".to_string(),
                    looping: false,
                },
                // The UI's own set writes the editor's extension, which is not the member's.
                Motion { name: "menu_idle".to_string(), clip: "menu_idle.vue".to_string(), looping: true },
                Motion { name: "bare".to_string(), clip: "bare".to_string(), looping: false },
            ],
            ..MotionSet::default()
        },
        bones(&["root"]),
        Vec::new(),
    );

    // The set's entry wins, and whatever extension it was written with becomes `.mot`.
    assert_eq!(set.member("bike_air_Fwd"), ["Bike\\Anim\\bike_air_Mid.mot", "Bike\\Anim\\bike_air_Fwd.mot"]);
    assert_eq!(set.member("MENU_IDLE"), ["Bike\\Anim\\menu_idle.mot", "Bike\\Anim\\MENU_IDLE.mot"]);
    assert_eq!(set.member("bare"), ["Bike\\Anim\\bare.mot"], "a name already without one");
    // A name the set has never heard of is still looked for: this is how a stunt clip is found.
    assert_eq!(set.member("bike_nacnac"), ["Bike\\Anim\\bike_nacnac.mot"]);
}

/// Binding is where a clip's channel names become this skeleton's joint indices — and where a clip
/// that does not belong to the skeleton says so.
#[test]
fn binding_maps_a_clips_channels_onto_the_skeletons_joints() {
    let skel = bones(&["Pelvis", "UprArmL", "HandL"]);
    let clip = clip_of(&["handl", "elbow", "PELVIS"]);
    let map = clip.joints(&skel);
    assert_eq!(map, vec![2, SKIP, 0], "case-insensitive, and 0xff for a joint this model has not");

    // A clip bound to the wrong skeleton reaches nothing, which is what `named` is for.
    let other = bones(&["Bike", "Handlebars"]);
    let bound = Bound {
        name: "x".to_string(),
        member: "x.mot".to_string(),
        joints: clip.joints(&other),
        clip,
    };
    assert_eq!(bound.named(), 0, "a rider's clip on a bike's skeleton drives nothing");
}

/// Which sampler each of the four blend modes means, and the two that draw nothing.
#[test]
fn each_blend_mode_poses_what_its_entry_point_takes() {
    let skel = bones(&["a", "b", "c"]);
    let mut set = Set::new(MotionSet::default(), skel, Vec::new());
    for (i, name) in ["one", "two"].iter().enumerate() {
        let clip = clip_of(&["a", "c"]);
        let joints = clip.joints(&set.skeleton);
        set.clips.push(Bound {
            name: (*name).to_string(),
            member: format!("{i}.mot"),
            clip,
            joints,
        });
    }
    let mut lib = Library::default();
    lib.stunt[0] = Some(ClipId(0));
    lib.stunt[1] = Some(ClipId(1));
    let (a, b) = (Slot::Stunt(0), Slot::Stunt(1));

    // One clip, half way through, with no cross-fade left: the pose is the clip's own.
    let mut player = Player::default();
    let blend = Blend::One { phase: F(0.5), ramp: F::ZERO, clip: a };
    player.run(F(DT), blend, &lib, &set.timing(Some(ClipId(0))));
    assert!(set.apply(blend, &lib, &player), "mode 1 should have posed");
    assert_eq!(set.pose.turned, vec![true, false, true], "only the channels' own joints");
    let half = set.pose.pos[0];
    assert_eq!(half, [1.0, 2.0, 3.0, 1.0], "the fixture's midpoint key");

    // Two clips of the same shape mixed any which way are still that shape.
    let blend = Blend::Two { phase: F(0.5), mix: F(0.25), ramp: F::ZERO, a, b };
    player.run(F(DT), blend, &lib, &set.timing(Some(ClipId(0))));
    assert!(set.apply(blend, &lib, &player), "mode 2 should have posed");
    for (got, want) in set.pose.pos[0].iter().zip(half) {
        assert!((got - want).abs() < 1e-6, "{:?} is not the mix of a clip with itself", set.pose.pos[0]);
    }

    // Four clips and three weights, all of them the same shape: still that shape, whatever the
    // three weights are. What the weights do is the next proof down.
    let blend = Blend::Three {
        phase: F(0.5),
        mix: F(0.5),
        cross: F(0.5),
        ramp: F::ZERO,
        a,
        b,
        c: a,
        d: b,
    };
    player.run(F(DT), blend, &lib, &set.timing(Some(ClipId(0))));
    assert!(set.apply(blend, &lib, &player), "mode 4 should have posed");
    for (got, want) in set.pose.pos[0].iter().zip(half) {
        assert!((got - want).abs() < 1e-6, "{:?} is not the mix of a clip with itself", set.pose.pos[0]);
    }

    // The free-running advance poses the clip that is on, and holds it once it has finished.
    let mut player = Player { clip: Some(ClipId(0)), ..Player::default() };
    let clip = set.timing(Some(ClipId(0)));
    let mut frames = 0;
    while !player.finished {
        player.run(F(DT), Blend::Free, &lib, &clip);
        let posed = set.apply(Blend::Free, &lib, &player);
        assert_eq!(posed, !player.finished, "a finished clip holds instead of snapping to its start");
        frames += 1;
        assert!(frames < 100, "the fixture clip never finished");
    }
    let held = set.pose.clone();
    for _ in 0..3 {
        player.run(F(DT), Blend::Free, &lib, &clip);
        assert!(!set.apply(Blend::Free, &lib, &player));
    }
    assert_eq!(set.pose.pos, held.pos, "and goes on holding it");

    // A blend naming a slot with no clip draws nothing at all, which is the frame the clock skips.
    let empty = Library::default();
    let blend = Blend::One { phase: F(0.5), ramp: F::ZERO, clip: a };
    assert!(!set.apply(blend, &empty, &player));
}

/// The cross-fade's two poses: what the simulation snapshots and what the renderer fades from are
/// the same joints, taken in one call so they cannot drift.
#[test]
fn a_rewind_takes_the_same_pose_both_sides_read() {
    let skel = bones(&["a", "b", "c"]);
    let mut set = Set::new(MotionSet::default(), skel, Vec::new());
    set.pose.rot[1] = [0.0, 0.0, 0.707_106_77, 0.707_106_77];
    set.pose.pos[1] = [4.0, 5.0, 6.0, 1.0];
    set.pose.turned[1] = true;
    set.pose.moved[1] = true;

    let mut snapshot = Snapshot::default();
    set.rewind(&mut snapshot);
    assert_eq!(snapshot.rotation.len(), 3);
    assert_eq!(snapshot.rotation[1].map(|v| v.0), set.pose.rot[1]);
    assert_eq!(snapshot.translation[1].map(|v| v.0), set.pose.pos[1]);
    assert_eq!(set.under.pos[1], set.pose.pos[1], "the renderer fades from the same pose");
    assert!(set.under.turned.iter().all(|&t| t), "all of it, not just the joints the clip wrote");

    // And a joint no clip has written answers with the identity, not with the bone's rest transform
    // — see the journal: the original reads the engine's joint array, which the rest pose is in.
    assert_eq!(set.pose.rotation(0).map(|v| v.0), [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(set.pose.translation(0).map(|v| v.0), [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(set.pose.count(), 3);

    // A pose can also be made from the simulation's own record of one, which is what a fade needs.
    let from = Pose::of_snapshot(&snapshot);
    assert_eq!(from.pos[1], set.pose.pos[1]);
    assert_eq!(from.rot[1], set.pose.rot[1]);
}

/// A `.mcf` that names geometry no archive holds is an error, not a panic.
#[test]
fn a_set_whose_geometry_is_missing_says_so() {
    let Err(why) = Set::load(Vec::new(), BIKE_MCF) else { panic!("an empty archive list loaded") };
    assert!(why.contains(BIKE_MCF), "{why}");
}

// ---------------------------------------------------------------------------------------------------
// 4 — the two blends nothing could draw (V11)
// ---------------------------------------------------------------------------------------------------

/// The four-clip blend: the pose grid's low band, through the machine that asks for it, and the pose
/// it writes is between the four clips it names.
///
/// Nothing here is a handmade blend value. The state is the grid (`vehicle_anim::TURN_RIDING`, which
/// is what the state chooser asks for on a ride — `game-sim/tests/motion.rs`), the band value is
/// under the grid's first threshold, and the selection picks the four clips, the two pair mixes and
/// the cross itself. The nine fixture clips stand in nine different places and turn nine different
/// ways, so a pose between four of them cannot have come from any other four.
#[test]
fn the_four_clip_blend_draws_between_the_four_rows_it_names() {
    let mut set = Set::new(MotionSet::default(), bones(&["x", "y"]), Vec::new());
    let mut lib = vehicle_anim::Library::default();
    for i in 0..9 {
        let turn = 128 + 64 * i as u16;
        let clip = clip_turning(&["x"], turn, [i as f32, 10.0 * i as f32, -1.0], 1.0 + 0.25 * i as f32);
        let joints = clip.joints(&set.skeleton);
        set.clips.push(Bound { name: format!("row{i}"), member: format!("{i}.mot"), clip, joints });
        lib.clip[vehicle_anim::TURN_STOP as usize + i] = Some(ClipId(i as u32));
    }

    // The grid's own thresholds, which are 5, 10 and 15 mph of chassis speed — see the journal.
    let params = vehicle_anim::Params {
        rate_default: F(3.0),
        band: [F(7.333_333), F(14.666_667), F(22.0)],
        ..vehicle_anim::Params::default()
    };
    let request = vehicle_anim::Request {
        phase_axis: F(0.5),
        mix_axis: F(0.25),
        band: F(3.0),
        seq: 1,
        ..vehicle_anim::Request::default()
    };
    let mut driver = vehicle_anim::Driver {
        want: vehicle_anim::TURN_RIDING,
        extra: -1,
        mix: vehicle_anim::Lag::new(F(0.2)),
        phase: vehicle_anim::Lag::new(F(0.2)),
        ..vehicle_anim::Driver::default()
    };
    let mut player = Player::default();

    // Half a second of standing nearly still, which is the low band: the two axes ease towards their
    // targets, so the weights the last frame draws with are the machine's and not a chosen pair.
    let mut frame = driver.frame(F(DT), &params, &request, &lib, true, |_| F::ONE);
    for _ in 0..15 {
        frame = driver.frame(F(DT), &params, &request, &lib, true, |id| {
            set.timing(Some(id)).duration
        });
        let clip = set.timing(frame.blend.clock_clip(&player));
        frame.blend.run(&mut player, F(DT), &clip);
        assert!(set.apply_vehicle(frame.blend, &player), "the grid's low band should have posed");
    }

    let vehicle_anim::Blend::Four { mix, cross, a, b, c, d, .. } = frame.blend else {
        panic!("the grid's low band is not a four-clip blend: {:?}", frame.blend)
    };
    // The clips it named: the stopped row's middle and low clips, and the still row's — the mix axis
    // is under a half, so each row's pair is the one below its middle.
    let row = |state: i32| Some(ClipId((state - vehicle_anim::TURN_STOP) as u32));
    assert_eq!(
        [a, b, c, d],
        [
            row(vehicle_anim::TURN_STOP + 1),
            row(vehicle_anim::TURN_STOP),
            row(vehicle_anim::TURN_STILL + 1),
            row(vehicle_anim::TURN_STILL),
        ],
        "the four clips are not the grid's own rows"
    );
    assert!(mix.0 > 0.0 && mix.0 < 1.0 && cross.0 > 0.0 && cross.0 < 1.0, "{mix:?} {cross:?}");

    // What the four clips themselves say at the one second the blend samples them all at — which is
    // the *first* clip's duration times the phase, so the four are read at one time and not four.
    let time = player.prev.0;
    let sample = |id: Option<ClipId>| {
        let bound = &set.clips[id.expect("a named clip").0 as usize];
        (bound.clip.rotation[0].sample(time).unwrap(), bound.clip.position[0].sample(time).unwrap())
    };
    let (qa, pa) = sample(a);
    let (qb, pb) = sample(b);
    let (qc, pc) = sample(c);
    let (qd, pd) = sample(d);

    // Rotation: every clip turns about the same axis, so the two pair mixes and the cross between
    // them come out as one angle, and the angle the pose holds has to be that nesting exactly.
    let (near, far) = (lerp(mix.0, angle(qb), angle(qa)), lerp(mix.0, angle(qd), angle(qc)));
    let want = lerp(cross.0, far, near);
    let got = angle(set.pose.rot[0]);
    assert!((got - want).abs() < 1e-4, "the pose turns {got}, not the {want} the four clips mix to");
    let (lo, hi) = bounds(&[angle(qa), angle(qb), angle(qc), angle(qd)]);
    assert!(lo < got && got < hi, "{got} is not between the four clips' {lo}..{hi}");

    // Position: the same nesting, on each axis, and strictly inside the four.
    for k in 0..3 {
        let (near, far) = (lerp(mix.0, pb[k], pa[k]), lerp(mix.0, pd[k], pc[k]));
        let want = lerp(cross.0, far, near);
        let got = set.pose.pos[0][k];
        assert!((got - want).abs() < 1e-4, "axis {k} is at {got}, not the {want} the four mix to");
        let (lo, hi) = bounds(&[pa[k], pb[k], pc[k], pd[k]]);
        assert!(lo <= got && got <= hi, "axis {k}: {got} is not between {lo}..{hi}");
    }

    // And nothing but the joint the clips name was touched.
    assert_eq!(set.pose.turned, vec![true, false], "only the channels' own joints");
    assert_eq!(set.pose.moved, vec![true, false]);
}

/// The pair at two phases: a gesture crossing out of the gesture that was running, each clip read at
/// its own point, and the joint map asymmetry the original has and nothing else in the port shows.
///
/// The two fixture clips name *different* joints — the gesture being asked for turns `y` and the one
/// it is coming out of turns `x` — which is the sharpest form of the asymmetry: the rotation channels
/// write the joint the **second** clip's map names and the position channels the one the first's does,
/// so one blend writes the rotation of one joint and the position of another. It is the original's
/// sixth blend entry that does that, not the port's reading of it.
#[test]
fn the_pair_blend_reads_each_clip_at_its_own_phase() {
    let mut set = Set::new(MotionSet::default(), bones(&["x", "y"]), Vec::new());
    let mut lib = vehicle_anim::Library::default();
    // two gesture slots: the one being asked for, two seconds long, and the one it comes out of
    for (slot, (name, turn, p, len)) in
        [("x", 128u16, [1.0, 2.0, 3.0], 2.0), ("y", 1024, [-5.0, -6.0, -7.0], 0.5)]
            .into_iter()
            .enumerate()
    {
        let clip = clip_turning(&[name], turn, p, len);
        let joints = clip.joints(&set.skeleton);
        set.clips.push(Bound { name: name.to_string(), member: format!("{slot}.mot"), clip, joints });
        lib.clip[slot] = Some(ClipId(slot as u32));
    }

    let params = vehicle_anim::Params {
        rate_stunt: F(3.0),
        cross_cap: F(0.25),
        start_phase: F(0.5),
        ..vehicle_anim::Params::default()
    };
    // A gesture a third of the way through, asked for while slot 0 is the state that was running:
    // the machine's own route to the pair is a repeat of a gesture, which restarts on the count.
    let request = vehicle_anim::Request {
        phase_axis: F(0.5),
        mix_axis: F(0.5),
        progress: F(1.0 / 3.0),
        seq: 1,
        ..vehicle_anim::Request::default()
    };
    let mut driver = vehicle_anim::Driver {
        want: 1,
        extra: -1,
        mix: vehicle_anim::Lag::new(F(0.2)),
        phase: vehicle_anim::Lag::new(F(0.2)),
        ..vehicle_anim::Driver::default()
    };
    let mut player = Player::default();
    let length = |id: ClipId| set.timing(Some(id)).duration;

    // The restart frame zeroes the new clip's phase and starts the old one at `start_phase`; the
    // frame after it is the one that reads both clips at their own point.
    driver.frame(F(DT), &params, &request, &lib, true, length);
    let frame = driver.frame(F(DT), &params, &request, &lib, true, length);
    let vehicle_anim::Blend::Pair { phase_a, phase_b, mix, a, b, .. } = frame.blend else {
        panic!("a gesture out of a gesture is not a pair blend: {:?}", frame.blend)
    };
    assert_eq!((a, b), (Some(ClipId(1)), Some(ClipId(0))), "the new gesture, out of the old one");
    assert!(phase_a.0 > 0.0 && phase_b.0 > phase_a.0, "{phase_a:?} {phase_b:?}");

    let clip = set.timing(frame.blend.clock_clip(&player));
    frame.blend.run(&mut player, F(DT), &clip);
    assert!(set.apply_vehicle(frame.blend, &player), "the pair should have posed");

    // The two times: the first clip's own duration times its phase, which the clock left behind, and
    // the second clip's duration times *its* phase, which is the whole point of this entry point.
    let (first, second) = (&set.clips[1], &set.clips[0]);
    let ta = player.prev.0;
    let tb = second.clip.duration() * phase_b.0;
    assert!((ta - first.clip.duration() * phase_a.0).abs() < 1e-6, "the clock is not a's own time");
    assert!((tb - ta).abs() > 0.1, "the two clips are being read at the same time after all");

    let (qa, pa) = (
        first.clip.rotation[0].sample(ta).unwrap(),
        first.clip.position[0].sample(ta).unwrap(),
    );
    let (qb, pb) = (
        second.clip.rotation[0].sample(tb).unwrap(),
        second.clip.position[0].sample(tb).unwrap(),
    );

    // The asymmetry: the rotation landed on the joint the *second* clip names, the position on the
    // joint the first one names, out of the one blend.
    assert_eq!(set.pose.turned, vec![true, false], "the rotation writes the second clip's joint, x");
    assert_eq!(set.pose.moved, vec![false, true], "the position writes the first clip's joint, y");

    let got = angle(set.pose.rot[0]);
    let want = lerp(mix.0, angle(qb), angle(qa));
    assert!((got - want).abs() < 1e-4, "the pose turns {got}, not the {want} the two clips mix to");
    let (lo, hi) = bounds(&[angle(qa), angle(qb)]);
    assert!(lo < got && got < hi, "{got} is not between the two clips' {lo}..{hi}");
    for k in 0..3 {
        let got = set.pose.pos[1][k];
        let want = lerp(mix.0, pb[k], pa[k]);
        assert!((got - want).abs() < 1e-4, "axis {k} is at {got}, not {want}");
        let (lo, hi) = bounds(&[pa[k], pb[k]]);
        assert!(lo < got && got < hi, "axis {k}: {got} is not between {lo}..{hi}");
    }
}

// --- small fixtures ---------------------------------------------------------------------------------

/// A skeleton of named joints, all at the origin and all roots.
fn bones(names: &[&str]) -> Skeleton {
    let nodes: Vec<Node> = names
        .iter()
        .map(|n| Node {
            name: (*n).to_string(),
            local: M4::IDENTITY,
            parent: None,
            center: Default::default(),
            half_extents: Default::default(),
            radius: 0.0,
        })
        .collect();
    Skeleton::of(&Model { nodes, materials: vec![], meshes: vec![], groups: 1 })
}

/// A one-second clip with one rotation and one position channel per name, each of three keys.
fn clip_of(names: &[&str]) -> Clip {
    let (mut rotation, mut position) = (Vec::new(), Vec::new());
    for k in 0..names.len() {
        rotation.push(RotChannel::new(k as i32, vec![
            RotKey { t: 0.0, turn: 0, axis: [32767, 0, 0] },
            RotKey { t: 0.5, turn: 512, axis: [32767, 0, 0] },
            RotKey { t: 1.0, turn: 1024, axis: [32767, 0, 0] },
        ]));
        position.push(PosChannel::new(k as i32, vec![
            PosKey { t: 0.0, p: [0.0, 0.0, 0.0] },
            PosKey { t: 0.5, p: [1.0, 2.0, 3.0] },
            PosKey { t: 1.0, p: [2.0, 4.0, 6.0] },
        ]));
    }
    Clip {
        version: 3,
        frames: 30,
        names: names.iter().map(|n| (*n).to_string()).collect(),
        timing: game_sim::motion::Clip::new(F::ONE, false),
        position,
        rotation,
    }
}

/// A clip whose one rotation channel per name turns about `x` from `turn` to a quarter-turn past it
/// over `duration` seconds, and whose position channel stands still at `p`.
///
/// The turn is what proves *when* a blend read the clip, and the position what it mixed: the two
/// halves of "the pose is between the clips it names".
fn clip_turning(names: &[&str], turn: u16, p: [f32; 3], duration: f32) -> Clip {
    let (mut rotation, mut position) = (Vec::new(), Vec::new());
    for k in 0..names.len() {
        rotation.push(RotChannel::new(k as i32, vec![
            RotKey { t: 0.0, turn, axis: [32767, 0, 0] },
            RotKey { t: duration * 0.5, turn: turn + 256, axis: [32767, 0, 0] },
            RotKey { t: duration, turn: turn + 512, axis: [32767, 0, 0] },
        ]));
        position.push(PosChannel::new(k as i32, vec![
            PosKey { t: 0.0, p },
            PosKey { t: duration * 0.5, p },
            PosKey { t: duration, p },
        ]));
    }
    Clip {
        version: 3,
        frames: 30,
        names: names.iter().map(|n| (*n).to_string()).collect(),
        timing: game_sim::motion::Clip::new(F(duration), false),
        position,
        rotation,
    }
}

/// The angle a quaternion about `x` holds, in radians, which is the one number the fixture clips
/// differ by.
fn angle(q: [f32; 4]) -> f32 {
    2.0 * q[0].atan2(q[3])
}

/// `from` at 0 and `to` at 1 — the weight convention every one of the original's blend entries uses.
fn lerp(t: f32, from: f32, to: f32) -> f32 {
    from + t * (to - from)
}

/// The lowest and highest of a blend's inputs: what "between the clips it names" is measured against.
fn bounds(of: &[f32]) -> (f32, f32) {
    let lo = of.iter().copied().fold(f32::INFINITY, f32::min);
    (lo, of.iter().copied().fold(f32::NEG_INFINITY, f32::max))
}
