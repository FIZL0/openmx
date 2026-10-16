//! The rider against the real disc: the three files that name each other, the one skeleton every
//! rider archive shares, the bone each vertex names, and the bolt onto the bike's chassis.
//!
//! Needs the user's own disc, so every test skips with the reason when it is not there — a fresh
//! checkout stays green.

use game_render::disc::{Disc, Res};
use game_render::mat::V3;
use game_render::psg::Model;
use game_render::rider::{FOOT_ANCHORS, HAND_ANCHORS, MotionSet, Rider, Sex, Skeleton, bolt_onto};
use game_render::scene::{Material, Vehicle};
use game_render::{ini, psg};

/// The stock gear: the rider every other one on the disc is a repaint of.
const A_RIDER: &str = "Rider\\thq1.res";
/// A licensed rider, whose `.sub` really does change the materials.
const A_LICENSED_RIDER: &str = "Rider\\reed1.res";
/// The bike the port is built against, for the chassis the rider bolts onto.
const A_BIKE: &str = "Bike\\250sx.res";

/// The 22 bones of the rider body, in the order its `.psg` lists them. Four of them carry no
/// geometry and exist only to be aimed at: the hands and the feet the bike has its own names for.
const BONES: [&str; 22] = [
    "Pelvis",
    "LwrTorso",
    "UprLegL",
    "UprLegR",
    "UprTorso",
    "Head",
    "ShoulderL",
    "ShoulderR",
    "UprArmL",
    "LwrArmL",
    "HandL",
    "Dummy_hand_L",
    "UprArmR",
    "LwrArmR",
    "HandR",
    "Dummy_hand_R",
    "LwrLegL",
    "FootL",
    "Dummy_foot_L",
    "LwrLegR",
    "FootR",
    "Dummy_foot_R",
];

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            println!("SKIPPED: {why}");
            None
        }
    }
}

/// Does the archive hold a file of that name, in whatever directory? By the file's own name: half
/// the clip names on the disc end in each other (`air_Mid.MOT`, `turn_still_Mid.MOT`).
fn holds(res: &Res, file: &str) -> bool {
    res.entries().iter().any(|e| {
        e.name
            .rsplit('\\')
            .next()
            .unwrap_or(&e.name)
            .eq_ignore_ascii_case(file)
    })
}

fn model(res: &Res, member: &str) -> Model {
    let bytes = res
        .read_suffix(member)
        .unwrap_or_else(|e| panic!("{member}: {e}"));
    Model::parse(&bytes).unwrap_or_else(|e| panic!("{member}: {e}"))
}

#[test]
fn the_three_files_of_a_rider_name_each_other() {
    let Some(mut disc) = disc() else { return };
    let res = disc.res(A_RIDER).expect(A_RIDER);
    let archives = [res];

    // `thq1.sub` says `MCF = rider.mcf`; the loader puts the body's sex in front of that.
    let rider = Rider::load(&archives, "thq1", Sex::Male).expect("thq1.sub");
    assert_eq!(rider.suit.mcf, "rider.mcf");
    assert_eq!(rider.mcf, "mrider.mcf", "the male body is the one on the disc");
    assert_eq!(rider.geometry, "mrider", "SLTFile=mrider.Slt is mrider.psg");
    assert_eq!(rider.set.rotations.as_deref(), Some("SHORTAXIS"));
    assert_eq!(rider.set.motions.len(), 40, "[General info] NumberOfMotions");
    // Three more are written out below the count and the clips they name are on no archive on the
    // disc, so the count at the top is what the loader walks.
    assert_eq!(rider.set.unlisted.len(), 3);
    for motion in &rider.set.unlisted {
        assert!(
            !holds(&archives[0], &motion.clip),
            "{} is past the count and yet shipped",
            motion.clip
        );
    }
    // There is no female body on this disc, so asking for one must fail rather than quietly serve
    // the male one — the fallback is for a `.sub` that names its own file, not for a missing sex.
    assert!(
        Rider::load(&archives, "thq1", Sex::Female).is_err(),
        "no frider.mcf is on the disc"
    );

    // The clips the set names are in the same archive, with one exception the whole disc shares:
    // `air_Bck_return.MOT` is named by all 324 rider `.sub`s and shipped by none of them, which is
    // the loader's "a motion whose clip the set does not have" path, not a misreading here.
    let mut clips: Vec<&str> = rider.set.motions.iter().map(|m| m.clip.as_str()).collect();
    let mut absent: Vec<&str> = Vec::new();
    for clip in &clips {
        if !holds(&archives[0], clip) {
            absent.push(clip);
        }
    }
    assert_eq!(absent, ["air_Bck_return.MOT"]);
    // The names and the files are not one to one: several poses share one clip.
    clips.sort_unstable();
    clips.dedup();
    assert!(
        clips.len() < rider.set.motions.len(),
        "{} clips for {} names",
        clips.len(),
        rider.set.motions.len()
    );

    // The same archive's second `.sub` is the rider as one of the pack: a different `.mcf`, naming a
    // different and cheaper body, and its name must survive the sex prefix.
    let ai = Rider::load(&archives, "thq1_ai", Sex::Male).expect("thq1_ai.sub");
    assert_eq!(ai.suit.mcf, "airider.mcf");
    assert_eq!(ai.mcf, "airider.mcf", "the prefix would make mairider.mcf");
    assert_eq!(ai.geometry, "airider");
    let (player, pack) = (
        model(&archives[0], "mrider.psg"),
        model(&archives[0], "airider.psg"),
    );
    assert_eq!(player.nodes.len(), pack.nodes.len(), "the same skeleton");
    assert!(
        pack.materials.len() < player.materials.len(),
        "the pack's body is one material, the player's is six and a name plate"
    );
}

#[test]
fn a_licensed_riders_gear_is_a_repaint_its_own_archive_carries() {
    let Some(mut disc) = disc() else { return };
    let res = disc.res(A_LICENSED_RIDER).expect(A_LICENSED_RIDER);
    let archives = [res];
    let rider = Rider::load(&archives, "reed1", Sex::Male).expect("reed1.sub");

    // Six pieces of gear, all six repainted — the same mechanism that paints the shared chassis in
    // another bike's colours, so the body's meshes ask for a material the archive does not have.
    assert_eq!(rider.suit.base_materials.len(), 6);
    assert_eq!(rider.suit.materials.len(), 6);
    let body = model(&archives[0], &format!("{}.psg", rider.geometry));
    for (from, to) in &rider.suit.materials {
        assert!(
            rider
                .suit
                .base_materials
                .iter()
                .any(|m| m.eq_ignore_ascii_case(from)),
            "{from} is repainted but is not a base material"
        );
        assert_ne!(from, to, "a change that changes nothing is commented out");
        assert!(
            body.materials.iter().any(|m| m.eq_ignore_ascii_case(from)),
            "no mesh asks for {from}"
        );
        // What it is repainted *to* is in this rider's own archive, texture and all.
        let psm = archives[0]
            .read_suffix(&format!("{to}.psm"))
            .unwrap_or_else(|e| panic!("{to}.psm: {e}"));
        let texture = Material::parse(&ini::text(&psm))
            .texture
            .unwrap_or_else(|| panic!("{to}.psm names no texture"));
        assert!(
            archives[0].find_suffix(&format!("{texture}.pst")).is_some(),
            "the archive has no {texture}.pst"
        );
    }
}

#[test]
fn every_rider_archive_holds_the_same_skeleton() {
    let Some(mut disc) = disc() else { return };
    let members: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let l = n.to_ascii_lowercase();
            l.starts_with("rider\\") && l.ends_with(".res")
        })
        .collect();
    assert_eq!(members.len(), 164, "the rider archives of this disc");

    let (mut players, mut pack) = (0, 0);
    for member in &members {
        let res = disc.res(member).unwrap_or_else(|e| panic!("{member}: {e}"));
        let subs: Vec<String> = res
            .entries()
            .iter()
            .filter(|e| e.name.to_ascii_lowercase().ends_with(".sub"))
            .map(|e| e.name.rsplit('\\').next().unwrap().to_string())
            .collect();
        assert!(!subs.is_empty(), "{member} holds no .sub");
        let archives = [res];
        for sub in &subs {
            let name = sub.trim_end_matches(".sub").trim_end_matches(".SUB");
            let rider = Rider::load(&archives, name, Sex::Male)
                .unwrap_or_else(|e| panic!("{member} {sub}: {e}"));
            // A rider's `.sub` carries a licensed rider's own bike, commented out everywhere here.
            assert_eq!(rider.suit.bike, None, "{member} {sub} names a bike");
            // A freestyle archive answers the same `MCF = rider.mcf` with its own copy of the
            // motion set, one mode prefix in front of the name the `.sub` asks for.
            let mode = if member.to_ascii_lowercase().starts_with("rider\\ff_") {
                "ff_"
            } else {
                ""
            };
            // And it is named after the body it names: `mrider.mcf` holds `mrider.Slt`.
            assert_eq!(
                rider.mcf,
                format!("{mode}{}.mcf", rider.geometry),
                "{member} {sub}"
            );
            // One clip is named by every `.sub` on the disc and shipped by none.
            let absent: Vec<&str> = rider
                .set
                .motions
                .iter()
                .map(|m| m.clip.as_str())
                .filter(|c| !holds(&archives[0], c))
                .collect();
            assert_eq!(absent, ["air_Bck_return.MOT"], "{member} {sub}");
            // The gear is in the archive: every material a mesh will ask for, after the repaint,
            // names a texture the same archive holds.
            for material in &rider.suit.base_materials {
                let painted = rider
                    .suit
                    .materials
                    .iter()
                    .find(|(from, _)| from.eq_ignore_ascii_case(material))
                    .map_or(material.as_str(), |(_, to)| to.as_str());
                let psm = archives[0]
                    .read_suffix(&format!("{painted}.psm"))
                    .unwrap_or_else(|e| panic!("{member} {sub}: {painted}.psm: {e}"));
                let texture = Material::parse(&ini::text(&psm))
                    .texture
                    .unwrap_or_else(|| panic!("{member} {sub}: {painted}.psm names no texture"));
                assert!(
                    holds(&archives[0], &format!("{texture}.pst")),
                    "{member} {sub}: no {texture}.pst"
                );
            }
            let body = model(&archives[0], &format!("{}.psg", rider.geometry));
            let skeleton = Skeleton::of(&body);
            let names: Vec<&str> = skeleton.bones.iter().map(|b| b.name.as_str()).collect();
            assert_eq!(names, BONES, "{member} {sub}");
            for bone in &skeleton.bones {
                // The bind pose is bone offsets and nothing else: every rotation is the identity,
                // which is why a clip can be nothing but rotations and still pose the whole body.
                for row in 0..3 {
                    for col in 0..3 {
                        let want = if row == col { 1.0 } else { 0.0 };
                        assert_eq!(
                            bone.rest.0[row][col], want,
                            "{member} {sub} {} is rotated in its rest pose",
                            bone.name
                        );
                    }
                }
            }
            bounds_hold(member, sub, &body);
            match rider.geometry.as_str() {
                "mrider" => players += 1,
                "airider" => pack += 1,
                other => panic!("{member} {sub} names a body called {other}"),
            }
        }
    }
    // Every archive but four has both: a player `.sub` naming the detailed body and a `_ai` one
    // naming the cheap one. The two create-a-rider archives carry only the player's (they are the
    // gear library), and the two ghost archives only the pack's.
    assert_eq!((players, pack), (162, 162));
}

/// Every vertex lies inside the bounds its own node stores — the check that the bone index in a
/// position's `w` is read right, over the whole body rather than one mesh of it.
fn bounds_hold(member: &str, sub: &str, body: &Model) {
    // The worst overrun over all 164 archives is 0.00005 of a foot, so this is as tight as the
    // quantised positions allow; a bone read one index out misses by tenths of a foot.
    const SLACK: f32 = 1e-4;
    for (i, node) in body.nodes.iter().enumerate() {
        for mesh in &body.meshes {
            for strip in &mesh.strips {
                for v in strip.verts.iter().filter(|v| v.node == i) {
                    for (axis, (p, (c, h))) in [v.pos.x, v.pos.y, v.pos.z]
                        .into_iter()
                        .zip(xyz(node.center).into_iter().zip(xyz(node.half_extents)))
                        .enumerate()
                    {
                        assert!(
                            p >= c - h - SLACK && p <= c + h + SLACK,
                            "{member} {sub}: a vertex of {} is {p} on axis {axis}, outside {}±{}",
                            node.name,
                            c,
                            h
                        );
                    }
                }
            }
        }
        assert!(
            node.parent.is_none_or(|p| p < body.nodes.len()),
            "{member} {sub}: {} hangs off a node that is not there",
            node.name
        );
    }
    for mesh in &body.meshes {
        for strip in &mesh.strips {
            for v in &strip.verts {
                assert!(
                    v.node < body.nodes.len(),
                    "{member} {sub}: a vertex names bone {}, and there are {}",
                    v.node,
                    body.nodes.len()
                );
            }
        }
    }
}

fn xyz(v: V3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

#[test]
fn the_rider_bolts_onto_the_chassis_at_its_pegs() {
    let Some(mut disc) = disc() else { return };
    let rider_res = disc.res(A_RIDER).expect(A_RIDER);
    let bike_res = disc.res(A_BIKE).expect(A_BIKE);
    let body = model(&rider_res, "mrider.psg");
    let chassis = model(&bike_res, "anim\\bike.psg");
    let rider = Skeleton::of(&body);
    let bike = Skeleton::of(&chassis);

    // The two skeletons share four names and no others: the grips and the foot pegs. Nobody wrote
    // that down anywhere — it is the only link between the two files, and it is what the game's own
    // inverse kinematics would close.
    let mut shared = rider.shared_with(&bike);
    shared.sort();
    assert_eq!(
        shared,
        ["Dummy_foot_L", "Dummy_foot_R", "Dummy_hand_L", "Dummy_hand_R"]
    );

    let at = bolt_onto(&rider, &bike, &FOOT_ANCHORS).expect("the four anchors");
    let moved = |name: &str| at.point(rider.at(name).expect(name));
    let peg = |name: &str| bike.at(name).expect(name);

    // The feet land on the pegs: the mean exactly, and each foot inside the peg it belongs to,
    // because the bind pose stands narrower than the bike's pegs are apart.
    let feet = moved("Dummy_foot_L").add(moved("Dummy_foot_R")).scale(0.5);
    let pegs = peg("Dummy_Foot_L").add(peg("Dummy_Foot_R")).scale(0.5);
    assert!(feet.sub(pegs).len() < 1e-5, "{feet:?} is not {pegs:?}");
    assert!(moved("Dummy_foot_L").x < 0.0 && moved("Dummy_foot_L").x > peg("Dummy_Foot_L").x);
    assert!(moved("Dummy_foot_R").x > 0.0 && moved("Dummy_foot_R").x < peg("Dummy_Foot_R").x);

    // And the body is on the bike rather than in it or under it: the pelvis above the seat, the
    // head above the bars, nothing below the pegs, and the whole of it inside the chassis's own
    // footprint fore and aft.
    let tris = body.triangles(0);
    assert!(!tris.is_empty(), "the body has geometry at its finest LOD");
    let (mut low, mut high, mut front, mut back) = (f32::MAX, f32::MIN, f32::MIN, f32::MAX);
    for t in &tris {
        for v in &t.v {
            let p = at.point(v.pos);
            low = low.min(p.y);
            high = high.max(p.y);
            front = front.max(p.z);
            back = back.min(p.z);
        }
    }
    let seat = chassis_top(&chassis);
    assert!(
        moved("Pelvis").y > seat,
        "the pelvis is at {:.2} and the bike's highest point is {seat:.2}",
        moved("Pelvis").y
    );
    assert!(high > peg("Dummy_Hand_L").y, "the head is above the bars");
    assert!(low > 1.0, "nothing of the rider is down at the axles: {low:.2}");
    assert!(front < 4.0 && back > -4.0, "front {front:.2} back {back:.2}");

    // The grips are the other pair of anchors, and they are the reason the feet are the pair used:
    // hanging the bind pose off the bars drops its feet through the engine and into the ground.
    let hung = bolt_onto(&rider, &bike, &HAND_ANCHORS).expect("the four anchors");
    assert!(hung.point(rider.at("Dummy_foot_L").unwrap()).y < 0.0);
}

/// How high the bike's own geometry reaches, the seat and the bars included.
fn chassis_top(chassis: &Model) -> f32 {
    chassis
        .triangles(0)
        .iter()
        .flat_map(|t: &psg::Tri| t.v.iter().map(|v| v.pos.y).collect::<Vec<f32>>())
        .fold(f32::MIN, f32::max)
}

#[test]
fn the_rider_is_built_the_way_a_bike_is() {
    let Some(mut disc) = disc() else { return };
    let rider_res = disc.res(A_RIDER).expect(A_RIDER);
    let bike_res = disc.res(A_BIKE).expect(A_BIKE);

    // The bike's `.sub` and the rider's are the same file format doing the same two jobs, so the
    // rider's reader must agree with the bike's on the half they share.
    let bike_sub = ini::text(&bike_res.read_suffix("250sx.sub").expect("250sx.sub"));
    let vehicle = Vehicle::parse(&bike_sub);
    let as_a_suit = game_render::rider::Suit::parse(&bike_sub);
    assert_eq!(as_a_suit.materials, vehicle.materials, "[Material Changes]");
    assert_eq!(as_a_suit.mcf, "bike.mcf", "[Objects] MCF");
    assert_eq!(vehicle.parts.len(), 7, "the bike has parts; a rider has none");

    // ...and the bike's own `.mcf` is the rider's format too, naming its geometry the same way.
    let set = MotionSet::parse(&ini::text(
        &bike_res.read_suffix("bike.mcf").expect("bike.mcf"),
    ));
    assert_eq!(set.geometry(), "bike", "SLTFile=bike.Slt is bike.psg");
    assert_eq!(set.motions.len(), 6);
    assert!(set.rotations.is_none(), "only a rider's .mcf packs rotations");
    for motion in &set.motions {
        assert!(
            holds(&bike_res, &motion.clip),
            "the archive has no {}",
            motion.clip
        );
    }

    // The rider has no sub-objects: its `.sub` is a repaint and a skeleton, and the body is one
    // mesh tree rather than a kit of parts.
    let rider_sub = ini::text(&rider_res.read_suffix("thq1.sub").expect("thq1.sub"));
    assert!(Vehicle::parse(&rider_sub).parts.is_empty());
}
