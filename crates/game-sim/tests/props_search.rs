//! S132 — the props' part search, against the original's own loader, on every track.
//!
//! No save state has a nationals track loaded, so this runs the original's `.toy` reader, its
//! create, its part search and its attach in the oracle, over each track's `.toy`, `.pco`s and scene
//! as the disc has them: the scene's models are laid out as the scene reader leaves them (an object
//! per `[ModelN]` with its node and the node's name, and the `RefName` table), the track's directory
//! is set, and the original walks the lot. Its own ini reader reads the files (handed in as
//! in-memory blocks), its own heap is stood in for by a bump allocator, and three calls are recorded
//! or dropped rather than run: the part's body (which needs the physics world), the constraints
//! (built on those bodies), and the collection's place in the scene graph and in the toy's list.
//! What comes out — the collections in order, each one's name, `.pco` and one-body flag, the model
//! every body stands on, and which models end up claimed — is checked against [`props::claim`] on
//! the same files. nat07's snowmen are the part search's; its test checks they came out of it.
//!
//! Skipped with a line where the disc, the kit or the state is not there.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use game_render::disc::Disc;
use game_sim::phy::Ini;
use game_sim::props;
use game_sim::track::Scene;
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "first_recording.p2m2_SaveState.eeMemory.bin";
const BUDGET: u64 = 2_000_000_000;

/// Where the test's own objects go: high RAM, clear of the code and of the stack the calls use.
const ARENA: u32 = 0x01c0_0000;
const ARENA_END: u32 = 0x01f0_0000;

/// The scene object's model array and `RefName` table, as the scene reader fills them.
const SCENE_MODELS: u32 = 0x3d0; // count, then the array at +4
const SCENE_REFS: u32 = 0x3dc; // the table, then its count at +4; {model, name[32]} each
const REF_STRIDE: u32 = 0x24;
/// A scene model's node and its claimed flag; a node's name pointer.
const MODEL_NODE: u32 = 0x34;
const MODEL_CLAIMED: u32 = 0x3c;
const NODE_NAME: u32 = 0x120;
/// The toy actor: its reader, its scene, the object whose `+0x3c` is the collections' parent, and
/// its collection list.
const ACTOR_READER: u32 = 0x0;
const ACTOR_SCENE: u32 = 0x8;
const ACTOR_WORLD: u32 = 0xc;
const ACTOR_LIST: u32 = 0x14;
/// The collection's fields.
const PCO_PATH: u32 = 0x40;
const NAME: u32 = 0x144;
const ONE_BODY: u32 = 0x38;

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

/// A bump allocator over [`ARENA`], standing in for the game's heap. Freed blocks are not reused.
#[derive(Default)]
struct Bump {
    next: u32,
    sizes: HashMap<u32, u32>,
}

impl Bump {
    fn take(&mut self, ee: &mut Ee, size: u32) -> u32 {
        if self.next == 0 {
            self.next = ARENA;
        }
        let at = (self.next + 15) & !15;
        self.next = at + size.max(1);
        assert!(self.next < ARENA_END, "the test's arena is full");
        ee.load(at, &vec![0; size as usize]);
        self.sizes.insert(at, size);
        at
    }

    fn string(&mut self, ee: &mut Ee, s: &str) -> u32 {
        let at = self.take(ee, s.len() as u32 + 1);
        ee.load(at, s.as_bytes());
        at
    }
}

fn key(path: &str) -> String {
    path.replace('/', "\\").to_ascii_lowercase()
}

/// The functions and the global the run needs, by name.
const NAMES: [&str; 15] = [
    "toy_physics_collections",
    "toy_collection_list_add",
    "physics_collection_part",
    "physics_collection_constraint",
    "node_add_child",
    "ini_new",
    "ini_open",
    "operator_new",
    "operator_new_array",
    "operator_delete",
    "operator_delete_array",
    "malloc",
    "realloc",
    "free",
    "track_dir",
];

/// Everything a run needs, or `None` and a line saying what is missing.
fn setup() -> Option<(HashMap<&'static str, u32>, Disc)> {
    image()?;
    let mut at = HashMap::new();
    for n in NAMES {
        match addr(n) {
            Ok(a) => {
                at.insert(n, a);
            }
            Err(why) => {
                skip(&why);
                return None;
            }
        }
    }
    match Disc::open_configured() {
        Ok(d) => Some((at, d)),
        Err(why) => {
            skip(&why);
            None
        }
    }
}

/// The tracks whose archive has a `.toy`: `(archive member, track name)`.
fn tracks(disc: &Disc) -> Vec<(String, String)> {
    disc.entries()
        .iter()
        .filter_map(|e| {
            let stem = e.name.strip_prefix("Tracks\\").or_else(|| e.name.strip_prefix("TRACKS\\"))?;
            let stem = stem.strip_suffix(".res").or_else(|| stem.strip_suffix(".RES"))?;
            (!stem.contains('\\')).then(|| (e.name.clone(), stem.to_string()))
        })
        .collect()
}

/// One track's `.toy` through the original's loader and through [`props::claim`], checked equal.
/// Hands back the port's collections and the scene, or `None` for a track with no `.toy`.
fn run(at: &HashMap<&'static str, u32>, disc: &mut Disc, member: &str, track: &str) -> Option<(Vec<props::Collection>, Scene)> {
    let res = disc.res(member).expect("a track's archive");
    let toy_bytes = res.read_suffix(&format!("{track}.toy")).ok()?;
    let scene = Scene::parse(&res.read_suffix(&format!("{track}.scn")).expect("the scene")).expect("the scene parses");
    let mut files: HashMap<String, Vec<u8>> = res
        .entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(".pco"))
        .map(|e| (key(&e.name), res.read(&e.name).expect("a .pco")))
        .collect();
    let dir = format!("Tracks\\{track}\\");
    let dir = dir.as_str();
    let toy_file = format!("{track}.toy");
    files.insert(key(&toy_file), toy_bytes.clone());
    let mut ee = image().expect("the state");

    // The port.
    let mut read = |path: &str| res.read(path).ok();
    let toy = Ini::parse_bytes(&toy_bytes);
    let (port, unported) = props::claim(&props::entries(&toy), dir, &scene.models, &mut read);
    assert!(unported.is_empty(), "{track} needs nothing unported: {unported:?}");

    // The heap.
    let bump = Rc::new(RefCell::new(Bump::default()));
    for n in ["operator_new", "operator_new_array", "malloc"] {
        let b = Rc::clone(&bump);
        ee.hooks.insert(at[n], Box::new(move |ee| {
            let size = ee.r(4) as u32;
            let p = b.borrow_mut().take(ee, size);
            ee.set(2, p as u64);
        }));
    }
    let b = Rc::clone(&bump);
    ee.hooks.insert(at["realloc"], Box::new(move |ee| {
        let (old, size) = (ee.r(4) as u32, ee.r(5) as u32);
        let mut b = b.borrow_mut();
        let p = b.take(ee, size);
        if old != 0 {
            let keep = b.sizes.get(&old).copied().unwrap_or(0).min(size);
            let bytes: Vec<u8> = (0..keep).map(|k| ee.u8_at(old + k)).collect();
            ee.load(p, &bytes);
        }
        ee.set(2, p as u64);
    }));
    for n in ["operator_delete", "operator_delete_array", "free"] {
        ee.hooks.insert(at[n], Box::new(|_| {}));
    }

    // The files: the original's own reader opens each one as an in-memory block.
    let open = at["ini_open"];
    let block = |ee: &mut Ee, b: &mut Bump, bytes: &[u8]| -> u32 {
        let p = b.take(ee, 8 + bytes.len() as u32);
        ee.load(p, &(bytes.len() as u64).to_le_bytes());
        ee.load(p + 8, bytes);
        p
    };
    let b = Rc::clone(&bump);
    let opened = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&opened);
    ee.hooks.insert(open, Box::new(move |ee| {
        let name = text(ee, ee.r(5) as u32);
        seen.borrow_mut().push(name.clone());
        let Some(bytes) = files.get(&key(&name)) else {
            ee.set(2, 0);
            return;
        };
        let p = block(ee, &mut b.borrow_mut(), bytes);
        let (reader, path, sp, ra, stack) = (ee.r(4), ee.r(5), ee.r(29), ee.r(31), ee.stack);
        ee.stack = (sp as u32 - 0x100) & !15;
        ee.call(open, &[reader, path, 0, u64::MAX, p as u64], &[], BUDGET);
        let v = ee.v0();
        ee.stack = stack;
        ee.set(29, sp);
        ee.set(31, ra);
        ee.set(2, v);
        ee.pc = open;
    }));

    // What is recorded rather than run.
    let listed = Rc::new(RefCell::new(Vec::new()));
    let l = Rc::clone(&listed);
    ee.hooks.insert(at["toy_collection_list_add"], Box::new(move |ee| {
        l.borrow_mut().push(ee.r(5) as u32);
        ee.set(2, 0);
    }));
    let parts = Rc::new(RefCell::new(Vec::new()));
    let p = Rc::clone(&parts);
    ee.hooks.insert(at["physics_collection_part"], Box::new(move |ee| {
        let kind = text(ee, ee.r(6) as u32);
        let rigid = kind.eq_ignore_ascii_case("RigidBody");
        p.borrow_mut().push((ee.r(4) as u32, ee.r(7) as u32, rigid));
        ee.set(2, rigid as u64);
    }));
    ee.hooks.insert(at["physics_collection_constraint"], Box::new(|_| {}));
    ee.hooks.insert(at["node_add_child"], Box::new(|ee| ee.set(2, 0)));

    // The scene, as the scene reader leaves it.
    let mut b = bump.borrow_mut();
    let n = scene.models.len() as u32;
    let scn = b.take(&mut ee, 0x400);
    let array = b.take(&mut ee, 4 * n);
    let refs: Vec<(usize, &str)> =
        scene.models.iter().enumerate().filter_map(|(i, m)| m.ref_name.as_deref().map(|r| (i, r))).collect();
    let table = b.take(&mut ee, REF_STRIDE * refs.len() as u32);
    let mut objects = Vec::new();
    let mut node_model = HashMap::new();
    for (i, m) in scene.models.iter().enumerate() {
        let obj = b.take(&mut ee, 0x40);
        let node = b.take(&mut ee, 0x130);
        let name = b.string(&mut ee, &m.file);
        ee.set_u32(node + NODE_NAME, name);
        ee.set_u32(obj + MODEL_NODE, node);
        ee.set_u32(array + 4 * i as u32, obj);
        objects.push(obj);
        node_model.insert(node, i);
    }
    for (k, &(i, r)) in refs.iter().enumerate() {
        let e = table + REF_STRIDE * k as u32;
        ee.set_u32(e, objects[i]);
        ee.load(e + 4, r.as_bytes());
    }
    ee.set_u32(scn + SCENE_MODELS, n);
    ee.set_u32(scn + SCENE_MODELS + 4, array);
    ee.set_u32(scn + SCENE_REFS, table);
    ee.set_u32(scn + SCENE_REFS + 4, refs.len() as u32);

    // The track's directory, and the actor with the `.toy` open in its reader.
    let mut d = dir.as_bytes().to_vec();
    d.push(0);
    ee.load(at["track_dir"], &d);
    let actor = b.take(&mut ee, 0x40);
    let world = b.take(&mut ee, 0x40);
    let reader = b.take(&mut ee, 0x2000);
    let toy_name = b.string(&mut ee, &toy_file);
    drop(b);
    ee.call(at["ini_new"], &[reader as u64], &[], BUDGET);
    // Through the hook, which hands the reader the file as a block.
    ee.call(open, &[reader as u64, toy_name as u64, 0, u64::MAX, 0], &[], BUDGET);
    assert_eq!(ee.v0(), 1, "the original's reader takes {toy_file}");
    ee.set_u32(actor + ACTOR_READER, reader);
    ee.set_u32(actor + ACTOR_SCENE, scn);
    ee.set_u32(actor + ACTOR_WORLD, world);
    let _ = ACTOR_LIST;

    ee.call(at["toy_physics_collections"], &[actor as u64], &[], BUDGET);

    // The original's collections, in the order its list has them.
    let listed = listed.borrow();
    let parts = parts.borrow();
    let _ = opened;
    let game: Vec<(String, String, bool, Vec<usize>)> = listed
        .iter()
        .map(|&c| {
            let members = parts
                .iter()
                .filter(|&&(coll, _, rigid)| coll == c && rigid)
                .map(|&(_, node, _)| *node_model.get(&node).expect("a body on a scene model's node"))
                .collect();
            (text(&ee, c + NAME), text(&ee, c + PCO_PATH), ee.u32_at(c + ONE_BODY) != 0, members)
        })
        .collect();
    let ours: Vec<(String, String, bool, Vec<usize>)> = port
        .iter()
        .map(|c| (c.name.clone(), c.pco_file.clone(), c.one_body, c.members.iter().map(|m| m.model).collect()))
        .collect();
    for (k, (g, o)) in game.iter().zip(&ours).enumerate() {
        assert_eq!(g, o, "{track} collection {k}: the original (left) and the port (right)");
    }
    assert_eq!(game.len(), ours.len(), "{track}: the collection count");
    // Every model the original claimed, the port has a body on.
    let claimed: Vec<usize> = (0..objects.len()).filter(|&i| ee.u32_at(objects[i] + MODEL_CLAIMED) != 0).collect();
    let mut bodies: Vec<usize> = port.iter().flat_map(|c| c.members.iter().map(|m| m.model)).collect();
    bodies.sort_unstable();
    bodies.dedup();
    assert_eq!(claimed, bodies, "{track}: the claimed models");
    println!("{track}: {} collections, {} bodies", port.len(), bodies.len());
    Some((port, scene))
}

#[test]
fn every_tracks_props_are_the_collections_the_original_builds() {
    let Some((at, mut disc)) = setup() else { return };
    let mut ran = 0;
    for (member, track) in tracks(&disc) {
        if run(&at, &mut disc, &member, &track).is_some() {
            ran += 1;
        }
    }
    assert!(ran >= 30, "the tracks with a .toy: {ran}");
}

#[test]
fn nat07s_snowmen_are_the_collections_the_original_builds() {
    let Some((at, mut disc)) = setup() else { return };
    let member = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("Tracks\\nat07.res")).expect("nat07.res").name.clone();
    let (port, scene) = run(&at, &mut disc, &member, "nat07").expect("nat07.toy");

    // The snowmen came out of the part search: two of each kind, six bodies a snowman, each part on
    // the model its RefName suffix names.
    for kind in ["SnowmanA", "SnowmanB"] {
        let men: Vec<_> = port.iter().filter(|c| c.name == kind).collect();
        assert_eq!(men.len(), 2, "{kind}: two snowmen");
        for (s, man) in men.iter().enumerate() {
            assert_eq!(man.members.len(), 6, "{kind} {s}: six parts");
            let tails: Vec<&str> = man
                .members
                .iter()
                .map(|m| {
                    let r = scene.models[m.model].ref_name.as_deref().expect("a part's model has a RefName");
                    &r[man.pco.parts[m.part].name.len()..]
                })
                .collect();
            assert!(tails.iter().all(|t| *t == tails[0]), "{kind} {s}: one suffix for every part: {tails:?}");
        }
    }
}

/// The search's rules on made-up files, no disc: a suffix found off part 0, a set whose suffixed
/// part is missing ending the walk, and an entry with no file reported rather than looped on.
#[test]
fn the_part_search_follows_the_ref_name_suffix() {
    use game_sim::track::Placement;
    let model = |file: &str, r: Option<&str>| Placement { file: file.into(), ref_name: r.map(Into::into), ..Placement::default() };
    let models = vec![
        model("Hat", Some("Hat_b")),
        model("Head", Some("Head_a")),
        model("Head", Some("Head_b")),
        model("Hat", Some("Hat_a")),
        model("Head", Some("Head_c")),
    ];
    let pco = "[PhysicsParts]\nNumberOfPhysicsParts = 2\n[PhysicsPart1]\nType = RigidBody\nName = Head\n\
               [PhysicsPart2]\nType = RigidBody\nName = Hat\n";
    let toy = Ini::parse("[PhysicsCollections]\nNumberOfPhysicsCollections = 2\n[Collection1]\nName = Man\n[Collection2]\nName = Gone\n");
    let mut read = |path: &str| (path == "T\\Man.pco").then(|| pco.as_bytes().to_vec());
    let (out, unported) = props::claim(&props::entries(&toy), "T\\", &models, &mut read);
    // Head_a takes Hat_a, Head_b takes Hat_b; Head_c has no Hat_c, so the walk stops there.
    let men: Vec<Vec<usize>> = out.iter().map(|c| c.members.iter().map(|m| m.model).collect()).collect();
    assert_eq!(men, vec![vec![1, 3], vec![2, 0]]);
    assert!(out.iter().all(|c| c.name == "Man" && !c.one_body));
    assert_eq!(unported, vec![props::Unported::EndlessSearch { entry: "Gone".into() }]);
}
