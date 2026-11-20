//! V135 — a race's tyre tracks laid by the Sim: `src/tire_tracks.rs` builds each rider's tracks for
//! the Sim (`sim_rider`) off the shell's entries, and writes the rows the Sim's tracks lay
//! (`game_sim::tire_track`, S448) into its rings (`Tracks::lay_sim`), as `src/main.rs`'s
//! `step_tire_marks` does in a race.
//!
//! No disc: the entries are the disc's numbers for free09's two (the module header's), and the bike
//! is a made-up straight run on the dirt, then a bend. The Sim's half runs its manager's pass and its
//! render walk each frame, as `Sim::step_all` does; the shell's half steps its own tracks on the same
//! tyres. `src/tire_tracks.rs` is compiled in by path, as the other tests take the shell's files.

#[allow(dead_code)]
#[path = "../src/tire_tracks.rs"]
mod tire_tracks;

use game_render::mat::V3;
use game_sim::air::AcosTable;
use game_sim::rng::Rng;
use game_sim::tire_track::{self, Snapshot, Tyre};
use ps2_float::{F, Vf};
use tire_tracks::{Bike, Disable, Params, Tire, Tracks};

/// The disc's two entries (`mxparticles.dat`, the module header): front on tire 0, rear on 1.
fn disc_list() -> Vec<Params> {
    let entry = |name: &str, tire: usize, width: f32, min_compression: f32| {
        let mut p = Params::defaults(name);
        p.tire = tire;
        p.width = width;
        p.min_compression = min_compression;
        p.max_compression = 1.0;
        p.min_speed = 3.0;
        p.base_alpha = 0.4;
        p.fade_in = 1.0;
        p.fade_out = 10.0;
        p.disable = Disable::NO_CONTACT | Disable::OBJECT_CONTACT | Disable::WRECKING;
        p.zones[0] = false;
        p
    };
    vec![entry("Front Tire Track", 0, 0.26, 0.85), entry("Rear Tire Track", 1, 0.275, 0.75)]
}

/// The bike's tyres at frame `n`: 30 ft/s along +z for two seconds, then bending right, both pressed
/// past their thresholds on flat ground.
fn tyres(n: u32) -> [(V3, f32); 2] {
    let t = n as f32 / 30.0;
    let (x, z) = if t < 2.0 { (0.0, 30.0 * t) } else { (20.0 * (1.0 - ((t - 2.0) * 1.5).cos()), 60.0 + 20.0 * ((t - 2.0) * 1.5).sin()) };
    [(V3 { x, y: 0.0, z: z + 2.5 }, 0.9), (V3 { x, y: 0.0, z }, 0.9)]
}

fn sim_snapshot(n: u32, zone: u8) -> Snapshot {
    let tyre = |(at, c): (V3, f32)| Tyre {
        at: Vf([at.x, at.y, at.z, 1.0]),
        normal: Vf([0.0, 1.0, 0.0, 0.0]),
        ground: true,
        object: false,
        compression: F(c),
        spinning: false,
        zone,
    };
    let [f, r] = tyres(n);
    Snapshot { tyres: [tyre(f), tyre(r)], speed: F(30.0), wrecking: false }
}

fn shell_bike(n: u32, zone: u8) -> Bike {
    let tire = |(at, c): (V3, f32)| Tire { at, normal: V3 { x: 0.0, y: 1.0, z: 0.0 }, ground: true, object: false, compression: c, spinning: false, zone };
    let [f, r] = tyres(n);
    Bike { tires: vec![tire(f), tire(r)], speed: 30.0, wrecking: false }
}

/// `frames` of the Sim's tracks on surface `zone`, the rows laid into the shell's rings each frame:
/// the rings, and every row the Sim laid.
fn ride_sim(list: &[Params], frames: u32, zone: u8) -> (Tracks, Vec<tire_track::Row>) {
    let acos = AcosTable::build();
    let mut rng = Rng::from_seed(7);
    // Seeded on the bike as it stands, as the shell seeds a race's.
    let mut sim = tire_track::Tracks { riders: vec![tire_tracks::sim_rider(list, sim_snapshot(0, zone))] };
    let mut marks = Tracks::new(list, 1);
    let mut rows = Vec::new();
    for n in 0..frames {
        sim.riders[0].switch(sim_snapshot(n, zone));
        sim.render(&acos, &mut rng);
        rows.extend(sim.riders[0].tracks.iter().flat_map(|k| k.rows.iter().copied()));
        marks.lay_sim(&sim.riders[0], u64::from(n));
        // A frame drawn twice lays its rows once.
        marks.lay_sim(&sim.riders[0], u64::from(n));
    }
    (marks, rows)
}

fn triangles(t: &Tracks) -> usize {
    t.tracks.iter().map(|k| k.strip.ring.triangles().len()).sum()
}

/// The Sim's rows land in the rings as it laid them, and draw the strips the shell's own steps draw on
/// the same ride: the same triangles but for a frame's lag (the Sim's manager reads the frame before)
/// and the height's jitter.
#[test]
fn v135_the_sims_rows_lay_the_shells_strips() {
    let list = disc_list();
    let frames = 120;
    let (sim, rows) = ride_sim(&list, frames, 1);
    assert!(!rows.is_empty(), "the Sim laid rows");
    assert!(rows.iter().any(|r| r.restart) && rows.iter().any(|r| !r.restart), "restarts and carried rows");
    let mut shell = Tracks::new(&list, 1);
    for n in 0..frames {
        shell.step(&shell_bike(n, 1));
    }
    let (got, want) = (triangles(&sim), triangles(&shell));
    println!("triangles: Sim's rows {got}, shell's own {want}; {} rows", rows.len());
    assert!(got > 0 && got.abs_diff(want) <= 4, "the Sim's strips ({got} triangles) are the shell's ({want})");
    // Every point in the rings is an edge of a row the Sim laid, its u and alpha with it.
    for k in &sim.tracks {
        for b in &k.strip.ring.batches {
            for p in b.iter().flatten() {
                let laid = rows.iter().any(|r| {
                    let e = r.edges[usize::from(p.s != 0.0)];
                    (e.x(), e.y(), e.z()) == (p.at.x, p.at.y, p.at.z) && r.u.0 == p.t && r.alpha == p.alpha
                });
                assert!(laid, "{} point {:?} is a row the Sim laid", k.params.name, p);
            }
        }
    }
    // The heights are the track's 0.3 ft and the jitter's 0.005 at most.
    for r in &rows {
        for e in r.edges {
            assert!((0.3..=0.305).contains(&e.y()), "row height {}", e.y());
        }
    }
}

/// A surface the descriptor turns off (zone 0, `Zone1 = None`) lays nothing in the Sim either, and
/// draws nothing on its generator.
#[test]
fn v135_a_zone_turned_off_lays_no_sim_rows() {
    let (marks, rows) = ride_sim(&disc_list(), 90, 0);
    assert!(rows.is_empty(), "rows on a zone turned off: {}", rows.len());
    assert_eq!(triangles(&marks), 0);
}

/// A bike seeded on the grid before its wheels were swept (no contact on either tyre, as the shell's
/// race seeds it) still lays rows once it rides.
#[test]
fn v135_a_bike_seeded_unswept_lays_rows() {
    let list = disc_list();
    let mut seed = sim_snapshot(0, 1);
    for t in &mut seed.tyres {
        t.ground = false;
    }
    let acos = AcosTable::build();
    let mut rng = Rng::from_seed(7);
    let mut sim = tire_track::Tracks { riders: vec![tire_tracks::sim_rider(&list, seed)] };
    let mut rows = 0;
    for n in 0..60 {
        sim.riders[0].switch(sim_snapshot(n, 1));
        sim.render(&acos, &mut rng);
        rows += sim.riders[0].tracks.iter().map(|k| k.rows.len()).sum::<usize>();
    }
    assert!(rows > 0, "no rows from a bike seeded unswept");
}
