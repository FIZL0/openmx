//! The track readers against the real disc: one freeride track, read out of the containers through the
//! same archive reader the app layer uses.
//!
//! Needs the game's own container, so it skips with the command that produces it when it is not there —
//! a fresh checkout stays green.

use game_sim::archive::{Entry, Psr, Res};
use game_sim::track::{
    BLOCK_SAMPLES, Child, Collision, Engine, FREERIDE_ENGINE, FREERIDE_PRO, Mesh, Points, Rays, Scene,
    Terrain, TerrainNode, Tuning, physics_file,
};
use ps2_float::F;
use ps2_oracle::{Kit, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// The freeride track the port is being built against. `_2` is the second half of the same track's
/// files: the vehicles' own collision and physics live there.
const TRACK: &str = "Tracks\\free06.res";
const TRACK_PART_2: &str = "Tracks\\free06_2.res";
const SCENE: &str = "tracks\\free06\\free06.scn";
/// The scene names its terrain `Free06.trn`; the disc stores the platform's copy of it.
const TERRAIN: &str = "tracks\\free06\\free06.ps2.trn";

/// Where the scene starts a rider, and which sample of the terrain that lands on.
const SPAWN: [f32; 3] = [2884.0, 338.0, 2750.0];

fn container() -> Option<File> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let at = kit.ctx("iso").join("MX.PSR");
    match File::open(&at) {
        Ok(f) => Some(f),
        Err(_) => {
            skip(&format!("no {} — `./kit extract MX.PSR` writes it", at.display()));
            None
        }
    }
}

fn member(f: &mut File, psr: &Psr, name: &str) -> Vec<u8> {
    let e = psr.find(name).unwrap_or_else(|| panic!("{name} is not in MX.PSR"));
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    bytes
}

fn directory(f: &mut File) -> Psr {
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    Psr::parse(&front).expect("MX.PSR's directory")
}

/// One archive of the track, ready to read members out of.
struct Part {
    file: Vec<u8>,
    res: Res,
}

impl Part {
    fn open(f: &mut File, psr: &Psr, name: &str) -> Part {
        let file = member(f, psr, name);
        let res = Res::parse(&file).unwrap_or_else(|e| panic!("{name}: {e}"));
        Part { file, res }
    }

    /// A member by the name the scene uses: no directory, and whatever case the author typed.
    fn by_leaf(&self, leaf: &str) -> Option<&Entry> {
        let want = leaf.to_ascii_lowercase();
        self.res.entries.iter().find(|e| {
            let name = e.name.to_ascii_lowercase();
            name == want || name.rsplit('\\').next() == Some(want.as_str())
        })
    }

    fn read(&self, e: &Entry) -> Vec<u8> {
        self.res.read(&self.file, e).unwrap_or_else(|why| panic!("{}: {why}", e.name))
    }
}

fn track(f: &mut File, psr: &Psr) -> Part {
    Part::open(f, psr, TRACK)
}

/// The scene: where a rider starts, how the terrain is laid out, and everything placed on it.
#[test]
fn the_scene_names_the_spawn_the_terrain_and_every_shape() {
    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);
    let part = track(&mut f, &psr);
    let bytes = part.read(part.by_leaf(SCENE).expect("the scene"));
    let scene = Scene::parse(&bytes).expect("the scene parses");

    assert_eq!(scene.start.map(|x| x.to_f32()), SPAWN, "the spawn");
    assert_eq!(scene.start_direction.map(|x| x.to_f32()), [0.0, 0.0, 1.0], "facing");
    assert_eq!(scene.terrain_file.to_ascii_lowercase(), "free06.trn");
    assert_eq!((scene.terrain.width, scene.terrain.breadth), (3, 3));
    assert_eq!(scene.terrain.width_scale, scene.terrain.breadth_scale);
    assert_eq!(scene.terrain.width_scale.to_f32(), 9.0);

    // Every section of the file is accounted for: the scene's own counts are the oracle.
    let text = String::from_utf8(bytes).expect("the scene is text");
    let counted = |key: &str| -> usize {
        text.lines()
            .find_map(|l| l.trim().strip_prefix(key)?.trim().strip_prefix('=').map(str::trim).map(str::to_owned))
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| panic!("the scene has no {key}"))
    };
    assert_eq!(scene.models.len(), counted("NumberOfStaticModels"), "static models");
    assert_eq!(scene.collision.len(), counted("NumberOfCollisionObjects"), "collision objects");
    assert!(scene.collision.len() > 300, "{} collision objects", scene.collision.len());

    // Each placement carries a file and a position; the direction vectors are unit length where given.
    for p in scene.collision.iter().chain(&scene.models) {
        assert!(!p.file.is_empty());
        let length = F::dot(p.look, p.look);
        assert!((length - F(1.0)).abs() < F(0.001), "{} looks along {:?}", p.file, p.look);
    }
}

/// Every collision mesh the scene places, read whole: the layout accounts for the file to the byte, each
/// triangle's plane really is the plane of its three vertices, and every tree child resolves.
#[test]
fn every_collision_shape_the_scene_places_reads_whole() {
    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);
    let part = track(&mut f, &psr);
    let scene = Scene::parse(&part.read(part.by_leaf(SCENE).expect("the scene"))).expect("the scene");

    let mut names: Vec<String> = scene.collision.iter().map(|p| p.file.to_ascii_lowercase()).collect();
    names.sort();
    names.dedup();
    assert!(names.len() > 80, "{} distinct shapes", names.len());

    let mut triangles = 0usize;
    let mut worst = F::ZERO;
    for name in &names {
        let e = part.by_leaf(name).unwrap_or_else(|| panic!("{name} is not in {TRACK}"));
        let bytes = part.read(e);
        let mesh = Mesh::parse(&bytes).unwrap_or_else(|why| panic!("{name}: {why}"));
        // The class word in the header is enough to read a file without being told what it is.
        assert_eq!(Collision::parse(&bytes), Ok(Collision::Mesh(mesh.clone())), "{name}");
        assert!(Rays::parse(&bytes).is_err(), "{name} read as the wrong class");

        assert!(mesh.version == 1 || mesh.version == 2, "{name} version {}", mesh.version);
        assert!(!mesh.triangles.is_empty(), "{name} has no triangles");
        if let Some(normals) = &mesh.normals {
            assert_eq!(normals.len(), mesh.vertices.len(), "{name}");
        }

        for (i, node) in mesh.nodes.iter().enumerate() {
            for which in 0..2 {
                match node.child(which) {
                    Child::Node(n) => assert!(
                        (n as usize) < mesh.nodes.len() && n as usize != i,
                        "{name} node {i} points at node {n} of {}",
                        mesh.nodes.len()
                    ),
                    Child::Leaf(t) => assert!(
                        (t as usize) < mesh.triangles.len(),
                        "{name} node {i} points at triangle {t} of {}",
                        mesh.triangles.len()
                    ),
                }
            }
        }

        let Points::Wide(verts) = &mesh.vertices else { panic!("{name} has packed vertices") };
        for t in &mesh.triangles {
            triangles += 1;
            for &v in &t.vertices {
                let v = verts[v as usize];
                let residual = (F::dot(t.normal, v) + t.distance).abs();
                if residual > worst {
                    worst = residual;
                }
                assert!(residual < F(0.01), "{name}: a vertex is {residual:?} off its own plane");
            }
        }
    }
    assert!(triangles > 3000, "{triangles} triangles");
    assert!(worst > F::ZERO, "no plane was actually checked");
}

/// The other collision class: the vehicles' swept line segments, in the same tree in the same header.
#[test]
fn the_vehicles_collision_files_are_segments_in_a_tree() {
    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);
    let mut found = 0;
    for archive in [TRACK, TRACK_PART_2] {
        let part = Part::open(&mut f, &psr, archive);
        let rays: Vec<String> = part
            .res
            .entries
            .iter()
            .map(|e| e.name.to_ascii_lowercase())
            .filter(|n| n.ends_with("m.col"))
            .collect();
        for name in rays {
            let e = part.by_leaf(&name).expect("the member we just listed");
            let bytes = part.read(e);
            let object = Rays::parse(&bytes).unwrap_or_else(|why| panic!("{name}: {why}"));
            assert_eq!(Collision::parse(&bytes), Ok(Collision::Rays(object.clone())), "{name}");
            assert!(Mesh::parse(&bytes).is_err(), "{name} read as the wrong class");
            // A binary tree over n segments has n - 1 nodes, which is the shape every one of these has.
            assert_eq!(
                object.nodes.len() + 1,
                object.segments.len(),
                "{name}: {} nodes over {} segments",
                object.nodes.len(),
                object.segments.len()
            );
            for (i, node) in object.nodes.iter().enumerate() {
                for which in 0..2 {
                    match node.child(which) {
                        Child::Node(n) => assert!((n as usize) < object.nodes.len(), "{name} {i}"),
                        Child::Leaf(s) => assert!((s as usize) < object.segments.len(), "{name} {i}"),
                    }
                }
            }
            found += 1;
        }
    }
    assert!(found >= 4, "only {found} ray collision objects");
}

fn terrain_bytes(f: &mut File, psr: &Psr) -> Vec<u8> {
    let part = track(f, psr);
    part.read(part.by_leaf(TERRAIN).expect("the terrain"))
}

/// Walk the whole tree, so every node and every child offset in the file is touched once.
fn leaves(terrain: &Terrain) -> Vec<TerrainNode> {
    let mut out = Vec::new();
    let mut stack = vec![terrain.root().expect("the root node")];
    while let Some(node) = stack.pop() {
        if node.children_at == 0 {
            out.push(node);
            continue;
        }
        for cz in 0..16 {
            for cx in 0..16 {
                if let Some(at) = terrain.child(&node, cx, cz) {
                    stack.push(terrain.node(at).expect("a child node"));
                }
            }
        }
    }
    out
}

/// The terrain's header: its blocks fill the space in front of its tree exactly, and its tree is as wide
/// as the scene says the terrain is.
#[test]
fn the_terrains_blocks_account_for_the_space_in_front_of_its_tree() {
    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);
    let bytes = terrain_bytes(&mut f, &psr);
    // Parsing is the accounting: the blocks have to end where the tree begins.
    let terrain = Terrain::parse(&bytes).expect("the terrain");
    assert_eq!(terrain.platform, *b"ps2\0");
    assert_eq!(terrain.scale.to_f32(), 9.0, "world units per sample");
    assert!(terrain.levels >= 1 && terrain.levels <= 4, "{} levels", terrain.levels);
    assert!(terrain.palette().is_some(), "this terrain is palettised");

    let root = terrain.root().expect("the root");
    assert_eq!(root.children_at, terrain.root_at() + 97, "the children follow the root node");

    let leaves = leaves(&terrain);
    // One leaf per 16x16 samples: three root cells of sixteen cells of sixteen samples, each way.
    let across = 3 * 16;
    assert_eq!(leaves.len(), across * across, "{} leaves", leaves.len());
    for leaf in &leaves {
        assert!(u32::from(leaf.block) < terrain.blocks, "block {} of {}", leaf.block, terrain.blocks);
        assert_eq!(leaf.height_scale, root.height_scale, "one quantisation step for the whole terrain");
        terrain.block(leaf.block).expect("the leaf's block");
    }
}

/// Every track's terrain on the disc reads, the nationals' too (S98): theirs carry 32 or 48 pictures of
/// 256×256 bytes between the blocks and the tree, each where the table in front of the blocks says, and
/// with them the space in front of the tree is accounted for to the byte. The free tracks carry none.
#[test]
fn every_tracks_terrain_accounts_for_its_blocks_and_textures() {
    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);
    let tracks: Vec<String> = psr
        .entries
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("tracks\\") && n.ends_with(".res") && !n.ends_with("_2.res")
        })
        .collect();
    let mut nationals = 0;
    for name in &tracks {
        let file = member(&mut f, &psr, name);
        let Ok(res) = Res::parse(&file) else { continue };
        let Some(e) = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(".ps2.trn")) else { continue };
        let bytes = res.read(&file, e).unwrap_or_else(|why| panic!("{name}: {why}"));
        let terrain = Terrain::parse(&bytes).unwrap_or_else(|why| panic!("{name}: {why}"));
        let stem = name.to_ascii_lowercase();
        if stem.contains("\\nat") {
            nationals += 1;
            assert!(matches!(terrain.textures.len(), 32 | 48), "{name}: {} textures", terrain.textures.len());
            for t in &terrain.textures {
                assert_eq!((t.format, t.levels.len()), (39, 1), "{name}");
                assert_eq!((t.levels[0].width, t.levels[0].height, t.levels[0].pixels.len()), (256, 256, 65536), "{name}");
            }
        } else if stem.contains("\\free") {
            assert!(terrain.textures.is_empty(), "{name}: {} textures", terrain.textures.len());
        }
        for leaf in leaves(&terrain) {
            terrain.block(leaf.block).unwrap_or_else(|| panic!("{name}: block {} of {}", leaf.block, terrain.blocks));
        }
    }
    assert_eq!(nationals, 22, "nat01..nat22");
}

/// The height formula, against the only oracle the file itself offers: every leaf declares the lowest and
/// highest point of what it samples, and those have to come out of the heights it points at.
#[test]
fn every_leaf_agrees_with_the_bounds_it_declares() {
    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);
    let bytes = terrain_bytes(&mut f, &psr);
    let terrain = Terrain::parse(&bytes).expect("the terrain");

    let leaves = leaves(&terrain);
    let mut flattened = 0;
    for leaf in &leaves {
        let block = terrain.block(leaf.block).expect("the block");
        let mut low = F(f32::MAX);
        let mut high = F(-f32::MAX);
        for z in 0..BLOCK_SAMPLES {
            for x in 0..BLOCK_SAMPLES {
                let y = leaf.height(block.raw_height(x, z).expect("a sample"), block.base);
                low = low.min(y);
                high = high.max(y);
            }
        }
        assert!(
            (low - leaf.min_y).abs() < F(0.01) && (high - leaf.max_y).abs() < F(0.01),
            "block {} spans {low:?}..{high:?} but its leaf declares {:?}..{:?}",
            leaf.block,
            leaf.min_y,
            leaf.max_y
        );
        if leaf.vertical_scale != F(1.0) {
            flattened += 1;
        }
    }
    assert!(flattened > 0, "no leaf reuses a block at reduced relief, so that path went unchecked");
    assert!(flattened < leaves.len(), "every leaf is flattened, which cannot be right");
}

/// End to end: the ground under the point the scene starts a rider at is where a rider would stand.
#[test]
fn the_ground_under_the_spawn_is_where_the_scene_starts_a_rider() {
    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);
    let part = track(&mut f, &psr);
    let scene = Scene::parse(&part.read(part.by_leaf(SCENE).expect("the scene"))).expect("the scene");
    let bytes = part.read(part.by_leaf(TERRAIN).expect("the terrain"));
    let terrain = Terrain::parse(&bytes).expect("the terrain");

    let scale = terrain.scale;
    assert_eq!(scale, scene.terrain.width_scale, "the two files agree on the scale");

    let sample = |p: F| (p / scale).to_i32() as u32;
    let (sx, sz) = (sample(scene.start[0]), sample(scene.start[2]));
    let quad = terrain.cell(sx, sz, 1).expect("the cell under the spawn");

    let ground = scale * quad.height[0];
    let drop = scene.start[1] - ground;
    assert!(
        drop > F::ZERO && drop < F(4.0),
        "the spawn is {drop:?} above the ground at sample ({sx}, {sz})"
    );
    // Surface ids index the friction surfaces a .phy lists, of which there are sixteen.
    for id in quad.surface {
        assert!(id < 16, "surface id {id}");
    }
    // The normals are the stored bytes, scaled but not normalised; the fourth lane is always one.
    for n in quad.normal {
        assert_eq!(n[3], F(1.0));
        assert!(n[1] > F::ZERO, "the ground faces up");
    }
}

/// End to end the other way: the shapes the scene places sit on the ground the terrain describes. Half of
/// them are within a unit of it, which is what pins the terrain's vertical scale — getting that wrong
/// moves the whole heightfield by hundreds of units.
#[test]
fn the_shapes_the_scene_places_sit_on_the_ground() {
    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);
    let part = track(&mut f, &psr);
    let scene = Scene::parse(&part.read(part.by_leaf(SCENE).expect("the scene"))).expect("the scene");
    let bytes = part.read(part.by_leaf(TERRAIN).expect("the terrain"));
    let terrain = Terrain::parse(&bytes).expect("the terrain");

    let scale = terrain.scale;
    let mut errors = Vec::new();
    for p in &scene.collision {
        let sx = (p.position[0] / scale).to_i32();
        let sz = (p.position[2] / scale).to_i32();
        if sx < 0 || sz < 0 {
            continue;
        }
        let Some(quad) = terrain.cell(sx as u32, sz as u32, 1) else { continue };
        errors.push((p.position[1] - scale * quad.height[0]).to_f32());
    }
    assert!(errors.len() > 300, "only {} placements landed on the terrain", errors.len());
    errors.sort_by(|a, b| a.partial_cmp(b).expect("no nans"));
    let median = errors[errors.len() / 2];
    assert!(
        median.abs() < 1.0,
        "the median shape sits {median} from the ground, over {} placements",
        errors.len()
    );
}

/// Which bike a freeride area starts on is the loader's business, not the archive's.
///
/// The track's own second archive carries the 250 pair of each tier, which is the directory listing that
/// used to make the port call a freeride bike a 250 — but the loader asks for the pair the event names,
/// and on a freeride boot that is the 125 Race pair (D7). All sixteen names it can build are on the disc,
/// inside the bike *model* archives, so nothing about the disc narrows the choice.
#[test]
fn every_physics_name_the_loader_can_build_is_on_the_disc() {
    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);

    // The bike's share of the track's own physics files — it carries every other vehicle's too.
    let part = Part::open(&mut f, &psr, TRACK_PART_2);
    let mut here: Vec<String> = part
        .res
        .entries
        .iter()
        .map(|e| e.name.to_ascii_lowercase())
        .filter(|n| n.starts_with("bike\\") && n.ends_with(".phy"))
        .collect();
    here.sort();
    assert_eq!(
        here,
        [
            "bike\\pro250a.phy",
            "bike\\pro250p.phy",
            "bike\\race250a.phy",
            "bike\\race250p.phy",
            "bike\\wreckbike.phy",
        ],
        "{TRACK_PART_2}"
    );

    // Every bike model the disc ships, and the physics files each one carries.
    let models: Vec<String> = psr
        .entries
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("bike\\") && n.ends_with(".res")
        })
        .collect();
    assert!(models.len() > 40, "only {} bike archives", models.len());
    let mut on_disc: Vec<String> = Vec::new();
    for name in &models {
        let part = Part::open(&mut f, &psr, name);
        for e in &part.res.entries {
            let member = e.name.to_ascii_lowercase();
            if member.ends_with(".phy") {
                on_disc.push(member);
            }
        }
    }
    on_disc.sort();
    on_disc.dedup();

    for engine in [Engine::Cc50, Engine::Cc125, Engine::Cc250, Engine::Cc500] {
        for pro in [false, true] {
            for tuning in [Tuning::P, Tuning::A] {
                let want = physics_file(engine, pro, tuning).to_ascii_lowercase();
                assert!(on_disc.contains(&want), "no bike archive carries {want}: {on_disc:?}");
            }
        }
    }

    // The pair a freeride boot loads, by name, in the order the loader builds them.
    assert_eq!(physics_file(FREERIDE_ENGINE, FREERIDE_PRO, Tuning::P), "Bike\\Race125P.phy");
    assert_eq!(physics_file(FREERIDE_ENGINE, FREERIDE_PRO, Tuning::A), "Bike\\Race125A.phy");
    for tuning in [Tuning::P, Tuning::A] {
        let want = physics_file(FREERIDE_ENGINE, FREERIDE_PRO, tuning).to_ascii_lowercase();
        assert!(on_disc.contains(&want), "the freeride pair is not on the disc: {want}");
    }
}

/// MD13: MX Unleashed's own scenes have none of MX vs. ATV's activity models, so the free-ride filter
/// leaves every one of them as it was: each `[Model<n>]` with a model and each `[Object<n>]` with a
/// shape that the file has, counted off the text, is in the parse.
#[test]
fn every_disc_scene_parses_with_no_activity_prop() {
    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);
    let tracks: Vec<String> = psr
        .entries
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("tracks\\") && n.ends_with(".res") && !n.ends_with("_2.res")
        })
        .collect();
    let mut scenes = 0;
    for track in &tracks {
        let part = Part::open(&mut f, &psr, track);
        for e in part.res.entries.iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".scn")) {
            let bytes = part.read(e);
            let Ok(scene) = Scene::parse(&bytes) else { continue };
            let text = String::from_utf8_lossy(&bytes);
            let (mut models, mut objects, mut section) = (0, 0, String::new());
            for line in text.lines().map(str::trim) {
                if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                    section = name.to_ascii_lowercase();
                    continue;
                }
                let Some((key, value)) = line.split_once('=') else { continue };
                let (key, value) = (key.trim().to_ascii_lowercase(), value.trim());
                let numbered = |p: &str| {
                    section.strip_prefix(p).is_some_and(|r| !r.is_empty() && r.bytes().all(|b| b.is_ascii_digit()))
                };
                match key.as_str() {
                    "slt" if numbered("model") && !value.is_empty() => models += 1,
                    "col" if numbered("object") && !value.is_empty() => objects += 1,
                    "type" if numbered("model") => assert!(
                        game_sim::track::ActivityModel::of(Some(value)).is_none(),
                        "{track}: {}: [{section}] Type={value}",
                        e.name
                    ),
                    _ => {}
                }
            }
            assert_eq!((scene.models.len(), scene.collision.len()), (models, objects), "{track}: {}", e.name);
            scenes += 1;
        }
    }
    assert!(scenes >= 57, "{scenes} scenes in {} track archives", tracks.len());
}

/// S82: a scene's `COL=` names reach the archive the way the game's ini reader leaves them, with the
/// spaces squeezed out — free09's `Large Egg.col` is the archive's `LargeEgg.col`, nat20's ice fences
/// and Indian ruins the same — so every shape a disc scene spells with a space is a member of its
/// track's two archives.
#[test]
fn every_disc_scenes_shapes_are_in_its_archives() {
    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);
    let tracks: Vec<String> = psr
        .entries
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("tracks\\") && n.ends_with(".res") && !n.ends_with("_2.res")
        })
        .collect();
    let (mut shapes, mut spaced, mut missing, mut by_track) = (0, 0, Vec::new(), Vec::<String>::new());
    for track in &tracks {
        let part = Part::open(&mut f, &psr, track);
        let second = track[..track.len() - 4].to_owned() + "_2.res";
        let parts: Vec<Part> = std::iter::once(part)
            .chain(psr.find(&second).map(|_| Part::open(&mut f, &psr, &second)))
            .collect();
        for e in parts[0].res.entries.iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".scn")) {
            let bytes = parts[0].read(e);
            let Ok(scene) = Scene::parse(&bytes) else { continue };
            let text = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
            for at in &scene.collision {
                shapes += 1;
                assert!(!at.file.contains(char::is_whitespace), "{track}: {}", at.file);
                // Other tracks place shapes their archives do not have at all; those are not this.
                if text.contains(&format!("={}", at.file.to_ascii_lowercase())) {
                    continue;
                }
                spaced += 1;
                by_track.push(track.clone());
                match parts.iter().find_map(|p| p.by_leaf(&at.file).map(|e| p.read(e))) {
                    Some(bytes) => {
                        Mesh::parse(&bytes).unwrap_or_else(|why| panic!("{track}: {}: {why}", at.file));
                    }
                    None => missing.push(format!("{track}: {}", at.file)),
                }
            }
        }
    }
    by_track.dedup();
    eprintln!("{shapes} shapes, {spaced} spelled with a space, in {by_track:?}");
    assert!(missing.is_empty(), "{} squeezed shapes not in their archives: {missing:?}", missing.len());
    assert!(spaced >= 62, "{spaced} names had a space in the scene");
}
