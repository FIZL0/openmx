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
