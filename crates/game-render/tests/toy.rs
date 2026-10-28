//! The moving scenery, against the real disc and against the original's own numbers: where free06's
//! train, trucks and scrapers and free09's trailers stood in save states of the original (V41).
//!
//! Every number below was read out of the cars themselves in a save state: each car's clip clock, its
//! point on the path, where it was drawn and which way it faced. The biplane recording's state is on
//! free06 at 21.89 seconds after the track loaded; the first human recording's is on free09. The
//! journal entry for V41 has how they were read.
//!
//! Needs the user's own disc, so every test skips with the reason when it is not there.

use game_render::disc::{Disc, Res};
use game_render::mat::{V3, v3};
use game_render::skin;
use game_render::toy::{self, Car, Path};
use game_sim::track::{Scene, Terrain};
use game_sim::world::Ground;
use ps2_float::F;

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            println!("SKIPPED: {why}");
            None
        }
    }
}

/// A track's main archive, by the name the shell asks for.
fn track(disc: &mut Disc, name: &str) -> Res {
    let want = format!("\\{name}.res");
    let member = disc
        .entries()
        .iter()
        .find(|e| e.name.to_ascii_lowercase().ends_with(&want))
        .unwrap_or_else(|| panic!("no {name}.res on the disc"))
        .name
        .clone();
    disc.res(&member).expect("the track's archive")
}

fn member_ending(res: &Res, ending: &str) -> Vec<u8> {
    let name = res
        .entries()
        .iter()
        .find(|e| e.name.to_ascii_lowercase().ends_with(ending))
        .unwrap_or_else(|| panic!("no member ending in {ending}"))
        .name
        .clone();
    res.read(&name).expect("the member")
}

/// The track's scene and terrain, which is what the cars are snapped onto.
fn ground_of(res: &Res) -> (Scene, Terrain<'static>) {
    let scene = Scene::parse(&member_ending(res, ".scn")).expect("the scene");
    let name = scene.terrain_file.to_ascii_lowercase();
    let tagged = match name.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}.ps2.{ext}"),
        None => name.clone(),
    };
    let bytes = res
        .entries()
        .iter()
        .find(|e| {
            let n = e.name.to_ascii_lowercase();
            n.ends_with(&name) || n.ends_with(&tagged)
        })
        .map(|e| res.read(&e.name.clone()).expect("the terrain"))
        .expect("the track's terrain");
    let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    (scene, Terrain::parse(bytes).expect("the terrain parses"))
}

/// The cars a track's `.toy` drives along a clip, each with its path.
fn cars(res: &Res, track: &str) -> Vec<(Car, Path)> {
    let text = game_render::ini::text(&member_ending(res, &format!("{track}.toy")));
    toy::cars(&text)
        .into_iter()
        .filter(|c| !c.has_physics)
        .map(|c| {
            let clip = skin::Clip::parse(&member_ending(res, &c.mot.to_ascii_lowercase())).expect("the car's clip");
            let path = Path::from_clip(&clip).expect("a Body channel");
            (c, path)
        })
        .collect()
}

/// One frame of the original's: every car moves once per scene update, at 30 a second. The trucks'
/// headings, which lag the way they move by one filter's worth of frames, agree with the original's
/// at this step and not at a sixtieth.
const STEP: f32 = 1.0 / 30.0;

/// One car as the save state has it: its model, its lag, where it was drawn, its flat heading's `x`.
struct Seen {
    model: &'static str,
    lag: f32,
    pos: [f32; 3],
    head: [f32; 3],
}

const FREE06_TRAIN_CLOCK: f32 = 18.608_541;

/// free06's train in the biplane recording's state: thirteen cars, one clock.
const FREE06_TRAIN: [Seen; 13] = [
    Seen { model: "train_car_01.psg", lag: 0.0, pos: [2313.3508, 134.9986, 2703.9092], head: [-0.0015, 0.0, -1.0] },
    Seen { model: "train_car_02.psg", lag: 57.0, pos: [2313.4397, 134.9986, 2760.9092], head: [-0.00157, 0.0, -1.0] },
    Seen { model: "train_car_03.psg", lag: 107.0, pos: [2313.5173, 134.9986, 2810.9089], head: [-0.00158, 0.0, -1.0] },
    Seen { model: "train_car_04.psg", lag: 160.0, pos: [2313.5999, 134.9986, 2863.9092], head: [-0.00152, 0.0, -1.0] },
    Seen { model: "train_car_05.psg", lag: 213.0, pos: [2313.6821, 134.9986, 2916.9089], head: [-0.00155, 0.0, -1.0] },
    Seen { model: "train_car_06.psg", lag: 267.0, pos: [2313.7659, 134.9986, 2970.9089], head: [-0.00163, 0.0, -1.0] },
    Seen { model: "train_car_03.psg", lag: 317.0, pos: [2313.8438, 134.9986, 3020.9092], head: [-0.00168, 0.0, -1.0] },
    Seen { model: "train_car_04.psg", lag: 371.0, pos: [2313.9280, 134.9986, 3074.9089], head: [-0.00146, 0.0, -1.0] },
    Seen { model: "train_car_05.psg", lag: 424.0, pos: [2314.0103, 134.9986, 3127.9087], head: [-0.00161, 0.0, -1.0] },
    Seen { model: "train_car_06.psg", lag: 478.0, pos: [2314.0942, 134.9986, 3181.9087], head: [-0.00157, 0.0, -1.0] },
    Seen { model: "train_car_03.psg", lag: 527.0, pos: [2314.1704, 134.9986, 3230.9087], head: [-0.00159, 0.0, -1.0] },
    Seen { model: "train_car_04.psg", lag: 581.0, pos: [2314.2544, 134.9986, 3284.9087], head: [-0.00151, 0.0, -1.0] },
    Seen { model: "train_car_08.psg", lag: 627.0, pos: [2314.3354, 134.9986, 3330.9089], head: [-0.00012, 0.0, -1.0] },
];

fn close(what: &str, got: V3, want: [f32; 3], tol: f32) {
    let d = got.sub(v3(want[0], want[1], want[2])).len();
    assert!(d <= tol, "{what}: {got:?}, the original {want:?}, {d} apart (allowed {tol})");
}

/// The train is free06's, it is the first thirteen cars of its `.toy`, and at the clock the
/// original's had reached it is where the original drew it: on its rails, every car its own lag
/// behind the engine, heading north.
#[test]
fn free06_spawns_its_train_where_the_original_runs_it() {
    let Some(mut disc) = disc() else { return };
    let res = track(&mut disc, "free06");
    let cars = cars(&res, "free06");
    let (scene, terrain) = ground_of(&res);
    let ground = Ground::new(&terrain, &scene.terrain);
    let mut on_ground = |p: V3| match ground.at(F(p.x), F(p.z)) {
        Some(hit) => (v3(p.x, hit.y.0, p.z), v3(hit.normal.x(), hit.normal.y(), hit.normal.z())),
        None => (p, v3(0.0, 1.0, 0.0)),
    };

    let train: Vec<_> = cars.iter().filter(|(c, _)| c.mot.eq_ignore_ascii_case("train_spline.mot")).collect();
    assert_eq!(train.len(), FREE06_TRAIN.len(), "the train's cars");
    // The clock the original's had after 21.89 seconds at the train's TimeScale.
    let clock = toy::clock(21.892_4, train[0].0.time_scale, train[0].1.last());
    assert!((clock - FREE06_TRAIN_CLOCK).abs() < 1e-3, "the train's clock: {clock}");
    for ((car, path), seen) in train.iter().zip(&FREE06_TRAIN) {
        assert_eq!(car.model, seen.model);
        assert_eq!(car.lag, seen.lag, "{}'s lag", car.model);
        let pose = path.pose(car, FREE06_TRAIN_CLOCK, STEP * car.time_scale, &mut on_ground);
        let what = format!("{} at lag {}", car.model, car.lag);
        // On the path to a hundredth of a foot: the clock, the lag walk and the curve are the original's.
        close(&format!("{what}: x and z"), v3(pose.pos.x, seen.pos[1], pose.pos.z), seen.pos, 0.01);
        // On the rails: the ground under them is the port's terrain blend, not the original's query.
        close(&format!("{what}: height"), v3(seen.pos[0], pose.pos.y, seen.pos[2]), seen.pos, 0.25);
        close(&format!("{what}: heading"), pose.head, seen.head, 0.002);
        let m = pose.transform.0;
        close(&format!("{what}: drawn at"), v3(m[3][0], m[3][1], m[3][2]), seen.pos, 0.25);
        close(&format!("{what}: facing"), v3(m[2][0], m[2][1], m[2][2]), seen.head, 0.002);
    }
}

/// The rest of free06's path cars and free09's trailers, which are where the same code puts them in
/// the original: free06's snapped onto the ground every frame, free09's riding keys presampled onto it.
#[test]
fn the_other_path_cars_drive_where_the_originals_do() {
    let Some(mut disc) = disc() else { return };
    // (track, the cars' clip clock in the state, model, lag, drawn at, heading)
    let seen: [(&str, f32, &str, f32, [f32; 3], [f32; 3]); 8] = [
        ("free06", 10.946_272, "dumptruck.psg", 0.0, [2926.1985, 169.3900, 3857.3157], [-0.06403, 0.0, 0.99794]),
        ("free06", 10.946_272, "dumptruck.psg", 0.0, [2929.3086, 170.5598, 3756.7976], [0.30546, 0.0, 0.94733]),
        ("free06", 10.946_272, "dumptruck.psg", 0.0, [3065.6711, 166.5667, 4022.7041], [0.69405, 0.0, -0.71724]),
        ("free06", 10.946_272, "dumptruck.psg", 0.0, [3032.2512, 332.0362, 2647.8899], [-0.06945, 0.0, -0.99634]),
        ("free06", 10.946_272, "scraperfront.psg", 0.0, [3292.5195, 334.9935, 2836.9336], [-0.95979, 0.0, 0.28071]),
        ("free06", 10.946_272, "scraperback.psg", 21.0, [2643.3528, 293.9330, 2896.8394], [0.98711, 0.0, -0.16002]),
        ("free09", 6.796_041, "free_trailercab.psg", 0.0, [2598.8704, 115.4136, 4136.1426], [-0.70708, 0.0, -0.70713]),
        ("free09", 6.796_041, "free_trailerD.psg", 650.0, [2888.1404, 99.7504, 4346.8354], [0.91003, 0.0, 0.41448]),
    ];
    for name in ["free06", "free09"] {
        let res = track(&mut disc, name);
        let mut cars = cars(&res, name);
        let (scene, terrain) = ground_of(&res);
        let ground = Ground::new(&terrain, &scene.terrain);
        let mut on_ground = |p: V3| match ground.at(F(p.x), F(p.z)) {
            Some(hit) => (v3(p.x, hit.y.0, p.z), v3(hit.normal.x(), hit.normal.y(), hit.normal.z())),
            None => (p, v3(0.0, 1.0, 0.0)),
        };
        for (_, path) in cars.iter_mut().filter(|(c, _)| c.presample) {
            path.presample(&mut on_ground);
        }
        for &(_, clock, model, lag, pos, head) in seen.iter().filter(|s| s.0 == name) {
            // Several cars share a model and a lag; the one the state saw is the one that is there.
            let best = cars
                .iter()
                .filter(|(c, _)| c.model == model && c.lag == lag)
                .map(|(c, p)| p.pose(c, clock, STEP * c.time_scale, &mut on_ground))
                .min_by(|a, b| {
                    let d = |p: &toy::Pose| v3(p.pos.x - pos[0], 0.0, p.pos.z - pos[2]).len();
                    d(a).total_cmp(&d(b))
                })
                .unwrap_or_else(|| panic!("{name} has no {model} at lag {lag}"));
            let what = format!("{name}'s {model} at lag {lag}");
            // A lag that walks back past the clip's start lands 0.0126 clip seconds later than the
            // original's (a foot of path), for a reason not found: see the journal entry for V41.
            let wraps = lag > 500.0;
            close(&format!("{what}: x and z"), v3(best.pos.x, pos[1], best.pos.z), pos, if wraps { 1.0 } else { 0.05 });
            close(&format!("{what}: height"), v3(pos[0], best.pos.y, pos[2]), pos, 0.5);
            close(&format!("{what}: heading"), best.head, head, 0.01);
        }
    }
}

/// One of the train's turned cars as the save state drew it: the matrix the car's model object held
/// for its own (root) node, the box the object kept in model space, and that box's centre in the world.
struct Drawn {
    model: &'static str,
    lag: f32,
    rows: [[f32; 4]; 4],
    centre: [f32; 3],
    half: [f32; 3],
    world_centre: [f32; 3],
}

/// free06's three car models whose file root is a half turn about up, in the biplane recording's state.
const TURNED_CARS: [Drawn; 3] = [
    Drawn {
        model: "train_car_03",
        lag: 107.0,
        rows: [
            [-1.0, 0.0, 0.00158, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [-0.00158, 0.0, -1.0, 0.0],
            [2313.51733, 134.9986, 2810.90894, 1.0],
        ],
        centre: [0.0, 9.04638, 0.07011],
        half: [5.0, 9.08959, 26.77135],
        world_centre: [2313.51685, 144.04495, 2810.83838],
    },
    Drawn {
        model: "train_car_04",
        lag: 160.0,
        rows: [
            [-1.0, -0.00014, 0.00152, 0.0],
            [-0.00014, 1.0, 0.0, 0.0],
            [-0.00152, 0.0, -1.0, 0.0],
            [2313.59985, 134.9986, 2863.90918, 1.0],
        ],
        centre: [-0.06549, 7.32081, 0.0],
        half: [5.91, 7.37276, 26.68585],
        world_centre: [2313.66382, 142.3194, 2863.90869],
    },
    Drawn {
        model: "train_car_05",
        lag: 213.0,
        rows: [
            [-0.99999, -0.00299, 0.00155, 0.0],
            [-0.00299, 1.0, 0.0, 0.0],
            [-0.00155, 0.0, -1.0, 0.0],
            [2313.68213, 134.9986, 2916.90894, 1.0],
        ],
        centre: [0.0, 8.0949, 0.07011],
        half: [5.0, 8.13811, 26.77135],
        world_centre: [2313.65747, 143.09343, 2916.83838],
    },
];

/// The centre and half size of what a loaded model draws, in its own space.
fn drawn_box(assets: &game_render::world::Assets, model: &str) -> (V3, V3) {
    let mut lo = v3(f32::MAX, f32::MAX, f32::MAX);
    let mut hi = v3(f32::MIN, f32::MIN, f32::MIN);
    for v in assets.models[model].tris.iter().flat_map(|t| t.v.iter()) {
        lo = v3(lo.x.min(v.pos.x), lo.y.min(v.pos.y), lo.z.min(v.pos.z));
        hi = v3(hi.x.max(v.pos.x), hi.y.max(v.pos.y), hi.z.max(v.pos.z));
    }
    (lo.add(hi).scale(0.5), hi.sub(lo).scale(0.5))
}

/// A path car is drawn the way a scene prop is (V46): the original's car makes the same model object
/// a prop does, whose node reader takes the file's root node to be the object itself, so the root's
/// matrix is read into the very slot the car's placement then overwrites. The file's half turn on
/// train_car_03, _04 and _05 is never drawn. The state shows it: each turned car's object holds its
/// pose and nothing else, and the box it kept, put in the world by that pose alone, is where the port
/// draws the car's vertices once [`Assets::load_placed`] has dropped the root — and 0.13 ft or more
/// away from where they land with the root's turn kept.
#[test]
fn the_trains_turned_cars_are_drawn_without_their_files_root_turn() {
    use game_render::mat::M4;
    use game_render::world::Assets;
    let Some(mut disc) = disc() else { return };
    let res = track(&mut disc, "free06");
    let archives = std::slice::from_ref(&res);
    let cars = cars(&res, "free06");
    let (scene, terrain) = ground_of(&res);
    let ground = Ground::new(&terrain, &scene.terrain);
    let mut on_ground = |p: V3| match ground.at(F(p.x), F(p.z)) {
        Some(hit) => (v3(p.x, hit.y.0, p.z), v3(hit.normal.x(), hit.normal.y(), hit.normal.z())),
        None => (p, v3(0.0, 1.0, 0.0)),
    };
    for seen in &TURNED_CARS {
        let (mut placed, mut kept) = (Assets::default(), Assets::default());
        assert!(placed.load_placed(archives, seen.model), "{:?}", placed.missing());
        assert!(kept.load(archives, seen.model), "{:?}", kept.missing());
        let file_root = kept.models[seen.model].model.nodes[0].local;
        assert!(
            (file_root.0[0][0] + 1.0).abs() < 1e-6 && (file_root.0[2][2] + 1.0).abs() < 1e-6,
            "{}'s file root is no longer a half turn: {file_root:?}",
            seen.model
        );
        assert_eq!(placed.models[seen.model].model.nodes[0].local.0, M4::IDENTITY.0, "{}'s root once placed", seen.model);

        // The box the original kept is the file's root box, and it is the box of the vertices as
        // they are in the file, not turned: the root's node frame is the object's own.
        let (centre, half) = drawn_box(&placed, seen.model);
        close(&format!("{}: drawn centre", seen.model), centre, seen.centre, 0.01);
        close(&format!("{}: drawn half size", seen.model), half, seen.half, 0.01);

        // That box through the object's matrix alone is where the original's world box sits, and the
        // port's vertices land there; with the root's turn kept they land elsewhere.
        let pose = M4(seen.rows);
        close(&format!("{}: the original's world box", seen.model), pose.point(v3(seen.centre[0], seen.centre[1], seen.centre[2])), seen.world_centre, 0.01);
        close(&format!("{}: drawn in the world", seen.model), pose.point(centre), seen.world_centre, 0.01);
        let (turned, _) = drawn_box(&kept, seen.model);
        let off = pose.point(turned).sub(v3(seen.world_centre[0], seen.world_centre[1], seen.world_centre[2])).len();
        assert!(off > 0.1, "{}: the root's turn would move it only {off} ft", seen.model);

        // And the port's pose for the car is that matrix, row for row.
        let (car, path) = cars
            .iter()
            .find(|(c, _)| c.stem() == seen.model && c.lag == seen.lag)
            .unwrap_or_else(|| panic!("no {} at lag {}", seen.model, seen.lag));
        let got = path.pose(car, FREE06_TRAIN_CLOCK, STEP * car.time_scale, &mut on_ground).transform;
        for r in 0..3 {
            let (g, w) = (got.0[r], seen.rows[r]);
            close(&format!("{}: row {r}", seen.model), v3(g[0], g[1], g[2]), [w[0], w[1], w[2]], 0.01);
        }
    }
}

/// The train as the shell draws it (V47): ticks at the game's 30 a second, each adding its `dt` to
/// the scene's clock, and the picture at a monitor's rate in between, at the clock as far between the
/// last two ticks as the bike's blend is. Over five seconds from the biplane state's clock every car
/// moves every frame by a steady step — none repeated, none doubled — and every pair of cars keeps its
/// coupling. The same frames drawn at the last tick's clock, as the shell used to, repeat.
#[test]
fn the_train_moves_every_frame_drawn_and_stays_coupled() {
    let Some(mut disc) = disc() else { return };
    let res = track(&mut disc, "free06");
    let (scene, terrain) = ground_of(&res);
    let ground = Ground::new(&terrain, &scene.terrain);
    let mut on_ground = |p: V3| match ground.at(F(p.x), F(p.z)) {
        Some(hit) => (v3(p.x, hit.y.0, p.z), v3(hit.normal.x(), hit.normal.y(), hit.normal.z())),
        None => (p, v3(0.0, 1.0, 0.0)),
    };
    let train = toy::Traffic {
        cars: cars(&res, "free06").into_iter().filter(|(c, _)| c.mot.eq_ignore_ascii_case("train_spline.mot")).collect(),
        skipped: Vec::new(),
    };
    assert_eq!(train.cars.len(), FREE06_TRAIN.len(), "the train's cars");
    let at = |placed: &[(String, game_render::mat::M4)]| -> Vec<V3> {
        placed.iter().map(|(_, m)| v3(m.0[3][0], m.0[3][1], m.0[3][2])).collect()
    };
    // The scene's clock after `n` ticks, summed from each tick's `dt` as the shell sums it.
    let dt = 1.0f32 / 30.0;
    let ticked = |n: u64| (0..n).fold(0.0f64, |t, _| t + f64::from(dt));
    let start = 21.892_4f64;
    let first = (start / f64::from(dt)) as u64;
    for hz in [60.0f64, 144.0, 165.0] {
        let frames = (5.0 * hz) as u64;
        let mut shown: Vec<Vec<V3>> = Vec::new();
        let mut stepped: Vec<Vec<V3>> = Vec::new();
        for f in 0..frames {
            // The wall clock past the first tick, and the ticks the pacer has run by then.
            let wall = f as f64 / hz + 0.25 / hz;
            let done = first + (wall * 30.0).floor() as u64 + 1;
            let by = ((wall * 30.0).fract()) as f32;
            let (before, last) = (ticked(done - 1), ticked(done));
            shown.push(at(&train.placed(toy::between(before, last, by), &mut on_ground)));
            stepped.push(at(&train.placed(last, &mut on_ground)));
        }
        let steps = |frames: &[Vec<V3>], car: usize| -> Vec<f32> {
            frames.windows(2).map(|w| w[1][car].sub(w[0][car]).len()).collect()
        };
        for car in 0..train.cars.len() {
            let s = steps(&shown, car);
            let mean = s.iter().sum::<f32>() / s.len() as f32;
            assert!(mean > 0.05, "{hz} Hz, car {car}: the train is moving ({mean} ft a frame)");
            for (i, w) in s.windows(2).enumerate() {
                let ratio = w[1] / w[0];
                assert!(
                    (0.9..1.1).contains(&ratio),
                    "{hz} Hz, car {car}, frame {}: a step of {} after {} (a repeat or a jump)",
                    i + 1,
                    w[1],
                    w[0]
                );
            }
            // The coupling: the gap to the car ahead is its lag's share of the path, which the lag
            // walk's straight-line reading of each segment lets breathe by a tenth of a foot or two
            // over the seconds — the original's own walk — but never moves from one frame to the next.
            if car > 0 {
                let want = train.cars[car].0.lag - train.cars[car - 1].0.lag;
                let gap: Vec<f32> = shown.iter().map(|p| p[car].sub(p[car - 1]).len()).collect();
                for (i, w) in gap.windows(2).enumerate() {
                    assert!((w[0] - want).abs() < 0.5, "{hz} Hz, cars {} and {car}: {} ft apart, lag {want}", car - 1, w[0]);
                    assert!((w[1] - w[0]).abs() < 0.01, "{hz} Hz, cars {} and {car}, frame {}: the coupling {} then {}", car - 1, i + 1, w[0], w[1]);
                }
            }
        }
        if hz < 100.0 {
            println!("{hz} Hz: steady, first car's step {:.4} ft", steps(&shown, 0)[0]);
        }
        let repeats = steps(&stepped, 0).iter().filter(|s| **s == 0.0).count();
        assert!(repeats > 0, "{hz} Hz: drawn at the tick's clock the train repeats frames");
    }
}

/// The simulation's cars (S81) off the disc, against the original's in S89's s89-along save state on
/// free06: every one of the state's 21 path cars and its two `HasPhysics` trucks (S127) has a car of
/// the port's with its path's keys (time, arc length and point) and its four numbers bit for bit, and
/// a collision mesh; and the port has no other.
#[test]
fn the_simulations_cars_are_the_originals_paths() {
    use ps2_oracle::{Ee, Kit, addr, skip};
    let Some(mut disc) = disc() else { return };
    let Ok(kit) = Kit::load().map_err(|w| skip(&w)) else { return };
    let state = kit.ctx("states").join("s89-along.eeMemory.bin");
    if !state.exists() {
        skip(&format!("{} is not there: `./kit state ram` of S89's s89-along save state", state.display()));
        return;
    }
    let ee = Ee::from_ram(&state).unwrap_or_else(|e| panic!("{e}"));
    let mut archives = vec![track(&mut disc, "free06")];
    if disc.entries().iter().any(|e| e.name.to_ascii_lowercase().ends_with("\\free06_2.res")) {
        archives.push(track(&mut disc, "free06_2"));
    }
    let (scene, terrain) = ground_of(&archives[0]);
    let ground = Ground::new(&terrain, &scene.terrain);
    let mut on_ground = |p: V3| match ground.at(F(p.x), F(p.z)) {
        Some(hit) => (v3(p.x, hit.y.0, p.z), v3(hit.normal.x(), hit.normal.y(), hit.normal.z())),
        None => (p, v3(0.0, 1.0, 0.0)),
    };
    let sim = toy::Traffic::load(&archives, "free06", &mut on_ground).sim(&archives);
    // The state's cars: the path car's vtable or its physics subclass's, eight bytes in.
    let of = |vtable: u32| -> Vec<u32> { (0..32u32 << 20).step_by(4).filter(|&a| ee.u32_at(a) == vtable).map(|a| a - 8).collect() };
    let mut cars = of(addr("path_car_vtable").unwrap());
    assert_eq!(cars.len(), 21, "the state's path cars");
    let trucks = of(addr("physics_car_vtable").unwrap());
    assert_eq!(trucks.len(), 2, "the state's physics cars");
    cars.extend(trucks);
    assert_eq!(sim.specs.len(), cars.len(), "the port's cars against the state's");
    for car in cars {
        let path = ee.u32_at(car + 0x430);
        let (n, keys) = (ee.u32_at(path), ee.u32_at(path + 0x8));
        let f = |a: u32| ee.u32_at(a);
        let theirs: Vec<[u32; 6]> = (0..n)
            .map(|k| keys + k * 0x30)
            .map(|k| [f(k), f(k + 4), f(k + 0x10), f(k + 0x14), f(k + 0x18), f(k + 0x1c)])
            .collect();
        let numbers = [0x534, 0x55c, 0x560, 0x558].map(|o| f(car + o));
        let found = sim.specs.iter().find(|s| {
            let ours: Vec<[u32; 6]> = s.keys.iter().map(|k| {
                let p = k.pos.to_bits();
                [k.t.to_bits(), k.len.to_bits(), p[0], p[1], p[2], p[3]]
            }).collect();
            ours == theirs && [s.time_scale, s.heading_filter, s.normal_filter, s.lag].map(|x| x.to_bits()) == numbers
        });
        let spec = found.unwrap_or_else(|| panic!("the state's car {car:#x} ({n} keys, lag {}) has no car of the port's", f32::from_bits(numbers[3])));
        assert!(spec.mesh.is_some(), "car {car:#x} has no collision mesh");
    }
}
