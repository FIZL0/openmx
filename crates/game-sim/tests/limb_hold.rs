//! V61 — how the rider holds the bike, against the user's recording2 ride played back on the
//! original.
//!
//! What V10 took for four limb controllers aimed at the bike's grips and pegs are the riding
//! ragdoll's four hold pins, and the weight it found read off each one's bone is that bone's inverse
//! mass, which the body's mass setter writes beside the mass. The pin's solve is ported and proved
//! over ten thousand poses in `tests/ragdoll.rs` (`the_pins_one_bone_solve_matches_the_original`).
//! What is proved here, off two captures of the same playback:
//!
//! - `holds.csv`: each hold pin's bone, its mass and inverse mass, the pin's anchor and gains;
//! - `gate.csv`: when the ragdoll is placed, which is when anything holds at all.
//!
//! Both are in `context/capture/v61/` (git-ignored), made by
//! `research/journal/2026-10-23-v61-limb-hold/capture/play.sh` from recording2's own state, frame
//! counter 2374 to 4023, one row a tick. A slot without them skips.

use game_sim::ragdoll::HOLD_BONES;
use ps2_oracle::{Kit, skip};
use std::collections::HashMap;
use std::path::PathBuf;

fn ctx(rel: &str) -> Option<PathBuf> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let local = kit.root.join("context").join(rel);
    let path = if local.exists() { local } else { kit.ctx(rel) };
    if !path.exists() {
        skip(&format!("{} is not there (V61's capture: research/journal/2026-10-23-v61-limb-hold)", path.display()));
        return None;
    }
    Some(path)
}

/// A capture's rows in order, each by column name.
fn rows(name: &str) -> Option<Vec<HashMap<String, String>>> {
    let text = std::fs::read_to_string(ctx(&format!("capture/v61/{name}.csv"))?).expect("read the capture");
    let mut lines = text.lines();
    let cols: Vec<String> = lines.next().expect("a header").split(',').map(str::to_string).collect();
    Some(lines.map(|l| cols.iter().cloned().zip(l.split(',').map(str::to_string)).collect()).collect())
}

fn f(row: &HashMap<String, String>, col: &str) -> f32 {
    let s = &row[col];
    s.parse::<f64>().unwrap_or_else(|_| panic!("{col}: {s:?}")) as f32
}

fn on(row: &HashMap<String, String>, col: &str) -> bool {
    row[col] != "0"
}

/// The ride kit off the disc, as the shell builds it (the same build as `tests/ragdoll_riding.rs`).
fn ride_kit() -> Option<game_sim::ragdoll_ride::Kit> {
    use game_render::disc::Disc;
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let names: Vec<String> =
        disc.entries().iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".res")).map(|e| e.name.clone()).collect();
    let rider_name = names.iter().filter(|n| n.to_ascii_lowercase().ends_with("thq1.res")).min_by_key(|n| n.len())?.clone();
    let res = disc.res(&rider_name).expect("the rider archive opens");
    let rider_file = res.read_suffix("anim\\mrider.phy").expect("the rider's archive carries mrider.phy");
    let mut body = game_render::rider::Body::load(
        vec![disc.res(&rider_name).expect("the rider archive opens")],
        "thq1",
        game_render::rider::Sex::Male,
        "",
        &[],
        game_sim::motion::Params::default(),
    )
    .expect("the rider's body loads");
    let model = game_render::psg::Model::parse(&res.read_suffix(&format!("{}.psg", body.geometry())).expect("its model"))
        .expect("the model parses");
    let doll = body.ragdoll_rig(&model, &rider_file).expect("every part names a node");
    Some(game_sim::ragdoll_ride::Kit::new(&rider_file, doll.rig))
}

/// **The weight each hold solves with is its bone's inverse mass, the riding one**: on every tick
/// of the capture the riding ragdoll is placed, each of the four hold pins' bone carries the mass and the inverse
/// mass the port's riding entry sets off `mrider.phy`, to the bit, and the pin's first gain is that
/// inverse mass. The hands' third gain is 1 and the last gain of all four 0.
#[test]
fn a_holds_weight_is_its_bones_riding_inverse_mass() {
    let Some(cap) = rows("holds") else { return };
    let Some(gate) = rows("gate") else { return };
    let Some(kit) = ride_kit() else { return };
    // The ticks the pins act on: riding, and the ragdoll placed. Off the bike the bones carry the
    // wreck's masses (`RagdollMass`), and they keep them for the two or three ticks after the wreck
    // until the riding entry runs again.
    let acting: std::collections::HashSet<&str> = gate
        .iter()
        .filter(|r| on(r, "wreck_mode") && on(r, "placed"))
        .map(|r| r["frame_counter"].as_str())
        .collect();
    let mut riding = game_sim::ragdoll_riding::Riding::new(&kit);
    riding.actor.ragdoll.ride_enter(true);
    let doll = &riding.actor.ragdoll;
    let (mut ticks, mut held, mut bad) = (0usize, [0usize; 4], Vec::new());
    for row in cap.iter().filter(|r| r["wreck_reason"] == "0" && acting.contains(r["frame_counter"].as_str())) {
        ticks += 1;
        for (h, &bone) in HOLD_BONES.iter().enumerate() {
            let p = |c: &str| f(row, &format!("pin{h}_{c}"));
            let b = &doll.bones[bone];
            let want = (b.mass.0.to_bits(), b.body.inv_mass.0.to_bits());
            let got = (p("mass").to_bits(), p("inv_mass").to_bits());
            if got != want && bad.len() < 6 {
                bad.push(format!("{} pin {h} (bone {bone}): mass {} inv {} want {} {}", row["frame_counter"], p("mass"), p("inv_mass"), b.mass.0, b.body.inv_mass.0));
            }
            if !on(row, &format!("pin{h}_on")) {
                continue;
            }
            held[h] += 1;
            assert_eq!(p("gains.x").to_bits(), p("inv_mass").to_bits(), "{} pin {h}: the first gain", row["frame_counter"]);
            assert_eq!(p("gains.w"), 0.0, "{} pin {h}: the last gain", row["frame_counter"]);
            if h < 2 {
                assert_eq!(p("gains.z"), 1.0, "{} pin {h}: a hand's third gain", row["frame_counter"]);
            }
        }
    }
    println!("{ticks} riding ticks, pins on {held:?}");
    assert!(ticks > 1000, "only {ticks} riding ticks in the capture");
    assert!(held[0] > 1000 && held[1] > 1000, "the hands were hardly held: {held:?}");
    assert!(bad.is_empty(), "a hold's bone weighs something else:\n{}", bad.join("\n"));
}

/// **Each hold's anchor is where the port's kit puts it** on its bone, to the bit: the hands' off the
/// rig's hand points, the feet's at the bone's own centre.
#[test]
fn a_holds_anchor_is_the_kits() {
    let Some(cap) = rows("holds") else { return };
    let Some(kit) = ride_kit() else { return };
    for row in &cap {
        for h in 0..4 {
            let a = kit.ragdoll.pins[h].anchors[0];
            let got = ["x", "y", "z"].map(|c| f(row, &format!("pin{h}_anchor.{c}")).to_bits());
            assert_eq!(got, [a[0], a[1], a[2]].map(f32::to_bits), "{} pin {h}: the anchor", row["frame_counter"]);
        }
    }
}

/// **Nothing holds while a pose off the clip table is in charge**: on every riding tick of the
/// capture the ragdoll is placed, which is what steps its pins and draws it, exactly when neither
/// `from_table` (as the tick before left it) nor `holding` (as this tick's body mode left it) is
/// up. Those are the two words [`game_sim::rider_anim::Body::mode`] chooses `Posed` on, read at
/// the same point, which is what the shell hands the riding ragdoll
/// (`Riding::posed_off_the_table`).
#[test]
fn the_ragdoll_lets_go_while_a_pose_is_off_the_table() {
    let Some(cap) = rows("gate") else { return };
    let (mut ticks, mut posed, mut bad) = (0usize, 0usize, Vec::new());
    for (before, row) in cap.iter().zip(&cap[1..]) {
        // Riding, not reposing, and the physics on: the other two of the placement's three flags.
        if !on(row, "wreck_mode") || on(row, "repose") || !on(row, "active") {
            continue;
        }
        ticks += 1;
        let off_the_table = on(before, "from_table") || on(row, "holding");
        posed += usize::from(off_the_table);
        if on(row, "placed") == off_the_table && bad.len() < 6 {
            bad.push(format!(
                "{}: placed {} with from_table {} holding {}",
                row["frame_counter"], row["placed"], before["from_table"], row["holding"]
            ));
        }
    }
    println!("{ticks} riding ticks, {posed} of them posed off the clip table");
    assert!(ticks > 1400, "only {ticks} riding ticks in the capture");
    assert!(posed > 300, "only {posed} posed ticks: the capture has hardly any trick in it");
    assert!(bad.is_empty(), "the ragdoll's placement is not the rule:\n{}", bad.join("\n"));
}
