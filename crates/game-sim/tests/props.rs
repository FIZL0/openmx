//! S80 — free09's knockable props, against the collections the original built.
//!
//! The first recording's save state is free09 with its props loaded and none of them knocked yet.
//! The `.toy` actor's collection list holds 62 collections; each is checked against the port's loader
//! run on the disc's `free09.toy`, `free09.scn` and the `.pco` files: the order, the name, the `.pco`
//! path, the world's two numbers, the one-body flag, the member count, and every body's place, mass,
//! damping, inertia and gravity, bit for bit; the turn by value, one crate an ulp off (see below).
//! The inertia's box is the scene node's bounding box, which the node takes from its model's `.psg`;
//! it is read off the disc and checked against the state's node.
//!
//! Skipped with a line where the disc, the kit or the state is not there.

use std::path::PathBuf;

use game_render::disc::Disc;
use game_sim::math::Mat4;
use game_sim::phy::Ini;
use game_sim::props::{self, Kind, Member};
use game_sim::track::Scene;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};

const STATE: &str = "first_recording.p2m2_SaveState.eeMemory.bin";

/// The `.toy` actor's collection list in this state: `{count, head}`. The head cell is the first
/// collection made; each cell's `+8` is the next one made.
const LIST: u32 = 0x011b_43f4;
/// The collection's fields.
const PCO_PATH: u32 = 0x40;
const ONE_BODY: u32 = 0x38;
const NAME: u32 = 0x144;
const ITERATIONS: u32 = 0x250;
const RATE: u32 = 0x254;
const BODIES: u32 = 0x320; // count, then the array's pointer at +8
/// The physics object's: its place and axes, the gravity, the body, the node.
const AT: u32 = 0x90;
const GRAVITY: u32 = 0xf0;
const BODY: u32 = 0x110;
const NODE: u32 = 0x13c;
/// The node's name (a pointer to it), its box (centre and half extent, used when the flags byte's
/// bit 2 is set), its first child, and its world rows.
const NODE_NAME: u32 = 0x120;
const CENTRE: u32 = 0xc0;
const HALF: u32 = 0xd0;
const FLAGS: u32 = 0x110;
const HAS_BOX: u8 = 4;
const CHILDREN: u32 = 0x104;
const WORLD: u32 = 0x80;

fn image() -> Option<Ee> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/states").join(STATE);
    let path = if here.exists() { here } else { kit.ctx("states").join(STATE) };
    match Ee::from_ram(&path) {
        Ok(ee) => Some(ee),
        Err(why) => {
            skip(&format!("{why} — `./kit state ram context/recordings/first_recording.p2m2_SaveState.p2s` writes it"));
            None
        }
    }
}

fn text(ee: &Ee, at: u32) -> String {
    (0..256).map(|k| ee.u8_at(at + k)).take_while(|&b| b != 0).map(|b| b as char).collect()
}

fn quad(ee: &Ee, at: u32) -> [u32; 4] {
    [0, 4, 8, 12].map(|k| ee.u32_at(at + k))
}

fn bits(what: &str, port: Vf, game: [u32; 4]) {
    assert_eq!(port.to_bits(), game, "{what}: the original built {:?}, the port {:?}", game.map(f32::from_bits), port.0);
}

fn one(what: &str, port: F, game: u32) {
    assert_eq!(port.to_bits(), game, "{what}: the original built {:e}, the port {:e}", f32::from_bits(game), port.0);
}

#[test]
fn free09s_props_are_the_collections_the_original_built() {
    let Some(ee) = image() else { return };
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => return skip(&why),
    };
    let member = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("Tracks\\free09.res")).expect("free09.res").name.clone();
    let res = disc.res(&member).expect("free09's archive");
    let toy = Ini::parse_bytes(&res.read_suffix("free09.toy").expect("free09.toy"));
    let scene = Scene::parse(&res.read_suffix("free09.scn").expect("free09.scn")).expect("free09.scn parses");

    // The game's collections, in the order they were made.
    let mut game = Vec::new();
    let mut cell = ee.u32_at(LIST + 4);
    while cell != 0 {
        game.push(ee.u32_at(cell));
        cell = ee.u32_at(cell + 8);
    }
    assert_eq!(game.len() as u32, ee.u32_at(LIST), "the list's count");

    let mut read = |path: &str| res.read(path).ok();
    let (port, unported) = props::claim(&props::entries(&toy), "Tracks\\Free09\\", &scene.models, &mut read);
    assert!(unported.is_empty(), "free09 needs nothing unported: {unported:?}");
    assert_eq!(port.len(), 62, "free09's collections");
    assert_eq!(port.len(), game.len(), "the collection count");

    for (c, (p, &g)) in port.iter().zip(&game).enumerate() {
        let at = |what: &str| format!("collection {c} ({}): {what}", p.name);
        assert_eq!(text(&ee, g + NAME), p.name, "{}", at("name"));
        assert_eq!(text(&ee, g + PCO_PATH), p.pco_file, "{}", at(".pco"));
        assert_eq!(p.kind, Kind::Single, "{}", at("kind"));
        assert_eq!(ee.i32_at(g + ITERATIONS), p.pco.iterations, "{}", at("iterations"));
        one(&at("rate"), p.pco.rate, ee.u32_at(g + RATE));
        assert_eq!(ee.u32_at(g + ONE_BODY) != 0, p.one_body, "{}", at("one-body flag"));
        assert_eq!(ee.u32_at(g + BODIES) as usize, p.members.len(), "{}", at("bodies"));
    }

    // Every body, with the node's box handed in.
    let wrapper = |m: Member| -> u32 {
        let (c, k) = port
            .iter()
            .enumerate()
            .find_map(|(c, coll)| coll.members.iter().position(|&x| x == m).map(|k| (c, k)))
            .expect("a member of some collection");
        ee.u32_at(ee.u32_at(game[c] + BODIES + 8) + 4 * k as u32)
    };
    // Every node's box, read off its model's .psg (S131), is the one the original's node holds, bit
    // for bit and all four lanes; the bodies are then made from the model's box.
    let mut node_box = |m: Member| {
        let node = ee.u32_at(wrapper(m) + NODE);
        assert_eq!(ee.u32_at(node + CHILDREN), 0, "a prop node with children");
        assert_ne!(ee.u8_at(node + FLAGS) & HAS_BOX, 0, "a prop node with no box");
        let file = &scene.models[m.model].file;
        let psg = res.read_suffix(&props::model_file(file)).expect("the prop's .psg");
        let made = props::NodeBox::from_model(&psg).expect("a one-node model");
        bits(&format!("{file}'s box centre"), made.centre, quad(&ee, node + CENTRE));
        bits(&format!("{file}'s box half extent"), made.half_extent, quad(&ee, node + HALF));
        Some(made)
    };
    let bodies = props::bodies(&port, &scene.models, &mut node_box);
    assert_eq!(bodies.len(), 62, "one body a collection");
    let mut signs = 0;
    let mut ulp_off = Vec::new();
    for prop in &bodies {
        let coll = &port[prop.collection];
        let m = coll.members[prop.member];
        let w = wrapper(m);
        let node = ee.u32_at(w + NODE);
        let b = ee.u32_at(w + BODY);
        let at = |what: &str| format!("{} (model {}): {what}", coll.name, m.model);
        let model = &scene.models[m.model];
        assert_eq!(text(&ee, ee.u32_at(node + NODE_NAME)), model.file, "{}", at("node"));
        let (position, _) = props::pose(model);
        bits(&at("place"), position, quad(&ee, w + AT));
        bits(&at("gravity"), prop.gravity, quad(&ee, w + GRAVITY));
        bits(&at("body position"), prop.body.position, quad(&ee, b + 0x50));
        // The turn. The body starts at its node's world frame through the axes-to-quaternion, bit
        // for bit. But the nodes are not as the scene reader left them: their rows carry negative
        // zeros (a body at rest writes its pose back to its node), so the props in this state were
        // made off nodes their world had already posed, and the port, which makes them off the
        // scene's frame, matches by value. Twelve rotated props' nodes have drifted an ulp or two
        // off the scene's frame that way (for that crate, the original's look-at on its line gives the
        // port's rows, checked once by hand); eleven still round to the same turn, and one crate's
        // is an ulp off.
        let world = Mat4([0, 1, 2, 3].map(|k| Vf::from_bits(quad(&ee, node + WORLD + 16 * k))));
        let start = quad(&ee, b + 0x160);
        bits(&at("start turn from the node"), props::turn(&world), start);
        assert_eq!(quad(&ee, b + 0x150).map(f32::from_bits), start.map(f32::from_bits), "{}", at("turn at rest"));
        let port: [f32; 4] = prop.body.quat.0;
        if port != start.map(f32::from_bits) {
            ulp_off.push(m.model);
            let off = port.iter().zip(start).map(|(p, g)| (p.to_bits() as i64 - g as i64).abs()).max();
            assert_eq!(off, Some(1), "{}: the port's {port:?}, the original's {:?}", at("start turn"), start.map(f32::from_bits));
        }
        one(&at("inverse mass"), prop.body.inv_mass, ee.u32_at(b + 0x10));
        one(&at("mass"), coll.pco.parts[m.part].part.mass, ee.u32_at(b + 0xc));
        bits(&at("damping"), prop.body.damping, quad(&ee, b + 0x20));
        bits(&at("inverse inertia"), prop.body.inv_inertia, quad(&ee, b + 0x120));
        bits(&at("angular damping"), prop.body.ang_damping, quad(&ee, b + 0x130));
        if model.file.to_ascii_lowercase().starts_with("sign_") {
            signs += 1;
            assert!(matches!(coll.pco.constraints[..], [props::Constraint::Pin { .. }]), "{}", at("the pin"));
        }
    }
    assert_eq!(signs, 28, "free09's signs");
    assert_eq!(ulp_off, [529], "the one prop whose start turn is an ulp off (an SH_CrateC)");
}

#[test]
fn a_starred_entry_is_cut_at_its_star() {
    let toy = Ini::parse("[PhysicsCollections]\nNumberOfPhysicsCollections = 2\n[Collection1]\nName = BeachBall\n[Collection2]\nName = sign_*\n");
    let e = props::entries(&toy);
    assert_eq!(e[0].pco("Tracks\\Free09\\"), "Tracks\\Free09\\BeachBall.pco");
    assert_eq!(e[0].kind(), Some(Kind::Single));
    assert_eq!(e[1].pco("Tracks\\Free09\\"), "Tracks\\Free09\\sign_.pco");
    assert_eq!(e[1].kind(), Some(Kind::Group));
    assert_eq!(props::shape_file("Sign_SpeedLmt"), "Sign_SpeedLmtf.col");
    assert_eq!(props::rays_file("BeachBall"), "BeachBallm.col");
}
