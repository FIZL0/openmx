//! The biplane's crash against the original: S36's capture of the human's biplane recording played
//! back (`context/capture/s36/biplane-crash.csv`, one row per tick, made by
//! research/journal/2026-10-20-s36-s36-the-biplane-crashes/play_crash.sh) and the disc's `biplane.phy`.

use game_sim::biplane::{Contact, Placement, Plane, Prop, Wreck, Wrecks, map_extent, near_edge};
use game_sim::rng::Rng;
use ps2_float::{F, Vf};

/// The capture: its header and its rows, every cell as the `f32` it was read as.
struct Capture {
    names: Vec<String>,
    rows: Vec<Vec<f32>>,
}

impl Capture {
    fn col(&self, name: &str) -> usize {
        self.names.iter().position(|n| n == name).unwrap_or_else(|| panic!("no column {name}"))
    }
    fn get(&self, row: usize, name: &str) -> f32 {
        self.rows[row][self.col(name)]
    }
    fn tick(&self, row: usize) -> u32 {
        self.get(row, "frame_counter") as u32
    }
    fn v3(&self, row: usize, name: &str) -> Vf {
        let g = |axis: &str| self.get(row, &format!("{name}.{axis}"));
        Vf::new(g("x"), g("y"), g("z"), 1.0)
    }
}

/// The two files, or `None` (and a SKIPPED line) when this checkout has not got them.
fn inputs() -> Option<(Wrecks, F, Capture)> {
    use ps2_oracle::{Kit, skip};
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let phy = kit.ctx("res/machine/biplane/biplane.phy");
    let csv = kit.ctx("capture/s36/biplane-crash.csv");
    let phy = std::fs::read(&phy).map_err(|e| skip(&format!("{}: {e}", phy.display()))).ok()?;
    let csv = std::fs::read_to_string(&csv).map_err(|e| skip(&format!("{}: {e}", csv.display()))).ok()?;
    let ini = game_sim::phy::Ini::parse_bytes(&phy);
    let weight = ini.section("Mass Properties").float_or("Weight", F::ZERO);
    let mut lines = csv.lines();
    let names = lines.next()?.split(',').map(str::to_owned).collect();
    let rows = lines
        .map(|l| l.split(',').map(|c| c.parse::<f64>().expect("a number") as f32).collect())
        .collect();
    Some((Wrecks::read(&ini), weight, Capture { names, rows }))
}

fn bits(v: Vf) -> [u32; 3] {
    [v.x().to_bits(), v.y().to_bits(), v.z().to_bits()]
}

/// The recording crashes twice, both times into the ground above `MinWreckSpeed` (reason 1). The
/// second wreck is waited out and the first is cut short by the reset chord. The port is given what
/// the original's test reads on each tick — the body contact of that tick, the airspeed and up.y the
/// previous tick's measure left — and has to wreck on the same two ticks and nowhere else, count the
/// same wreck clock, unwreck the waited-out one on the same tick (151 ticks in), and put the plane
/// back where the original did: the spawn, the spawn's facing, standing still, under its own gravity.
#[test]
fn the_biplane_crashes_and_respawns() {
    let Some((w, weight, cap)) = inputs() else { return };
    assert_eq!((w.wreck_time, w.min_wreck_speed, w.wreck_gravity), (F(5.0), F(50.0) * F(1.46666), F(1.5)));
    let spawn = Placement { at: cap.v3(0, "at"), forward: cap.v3(0, "fwd"), up: cap.v3(0, "up") };
    let inertia = Vf::new(1000.0, 2000.0, 3000.0, 1.0);
    let mut plane = Plane::new(spawn, weight, inertia, F::ONE);
    let mut rng = Rng::default();

    // The plane's own gravity, the bits the capture has before any crash.
    assert_eq!(plane.gravity.y().to_bits(), cap.get(0, "gravity_y").to_bits(), "the flying gravity");

    let mut entries = Vec::new();
    let mut exits = Vec::new();
    for r in 1..cap.rows.len() {
        let tick = cap.tick(r);
        assert_eq!(tick, cap.tick(r - 1) + 1, "the capture has a gap before {tick}");
        let dt = F(cap.get(r, "dt"));
        plane.airspeed = F(cap.get(r - 1, "airspeed"));
        plane.up_y = F(cap.get(r - 1, "up_y"));
        plane.grounded = cap.get(r - 1, "grounded") != 0.0;
        plane.throttle = F(cap.get(r - 1, "throttle"));
        let was = plane.wreck;

        // The chord: the one exit the clock cannot account for is the reset chord's (L1+R1 held in the
        // recording just there), and the input slot runs before the measure.
        let theirs = cap.get(r, "wreck") as u32;
        let chord = was.is_some() && theirs == 0 && !(w.wreck_time < plane.clock + dt);
        plane.reset_chord(chord);
        plane.frame(dt);

        let contact = Contact { touching: cap.get(r, "contact") != 0.0, vehicle: false };
        plane.wreck_check(dt, &w, contact, None, &mut rng);

        assert_eq!(plane.wreck_word(), theirs, "the wreck word on tick {tick}");
        if was.is_none() && plane.wreck.is_some() {
            entries.push(tick);
            assert_eq!(plane.throttle, F::ZERO, "a wreck cuts the throttle");
            assert_eq!(plane.prop(), Prop::Bent);
        }
        if was.is_some() && plane.wreck.is_none() {
            exits.push((tick, chord));
            assert_eq!(plane.prop(), Prop::Blur);
        }
        if plane.wreck.is_some() {
            assert_eq!(plane.clock.0.to_bits(), cap.get(r, "wreck_clock").to_bits(), "the clock on {tick}");
        }
        assert_eq!(plane.gravity.y().to_bits(), cap.get(r, "gravity_y").to_bits(), "the gravity on {tick}");
        if was == Some(Wreck::Contact) && plane.wreck.is_none() && !chord {
            // Waited out: the body is exactly where the next tick's integration starts from.
            assert_eq!(bits(plane.body.at), bits(cap.v3(r, "pos")), "respawned at the spawn");
            assert_eq!(bits(plane.body.vel), bits(cap.v3(r, "vel")), "respawned standing still");
            assert_eq!(bits(plane.body.spin), bits(cap.v3(r, "spin")), "respawned not turning");
            assert_eq!((plane.body.forward, plane.body.up), (spawn.forward, spawn.up), "facing the spawn's way");
            assert_eq!(plane.body.inertia, inertia, "the inertia given back");
        }
    }
    assert_eq!(entries, [11664, 12498], "the two crashes' entry ticks");
    assert_eq!(exits, [(11747, true), (12649, false)], "the chord's exit, then the waited-out one");
    assert_eq!(exits[1].0 - entries[1], 151, "a waited-out wreck lasts 151 ticks at the capture's dt");
    assert_eq!(exits[0].0 - entries[0], 83, "the chord cut the first one short");
}

/// What the capture cannot show: the map edge's throw and its longer wreck, the chord's gating, the
/// speed rule's `VEHICLE` exception, and the upside-down rule.
#[test]
fn the_biplane_crash_rules_the_capture_does_not_reach() {
    let w = Wrecks {
        wreck_time: F(5.0),
        min_wreck_speed: F(50.0) * F(1.46666),
        wreck_gravity: F(1.5),
        wreck_damping: Vf::new(0.999, 0.999, 0.999, 1.0),
        wreck_inertia: Vf::new(0.5, 0.5, 0.5, 1.0),
    };
    let spawn = Placement {
        at: Vf::new(2304.0, 138.614_06, 2281.0, 1.0),
        forward: Vf::new(0.0, 0.0, -1.0, 1.0),
        up: Vf::new(0.0, 1.0, 0.0, 1.0),
    };
    let new = || Plane::new(spawn, F(1500.0), Vf::new(10.0, 20.0, 30.0, 1.0), F::ONE);
    let dt = F(0.033_33);
    let mut rng = Rng::default();
    let touch = Contact { touching: true, vehicle: false };

    // Slow, upright and touching: no wreck. Fast on a VEHICLE surface: none either. Upside down: one.
    let mut p = new();
    p.airspeed = w.min_wreck_speed;
    p.wreck_check(dt, &w, touch, None, &mut rng);
    assert_eq!(p.wreck, None, "at MinWreckSpeed exactly it is not a crash");
    p.airspeed = F(200.0);
    p.wreck_check(dt, &w, Contact { touching: true, vehicle: true }, None, &mut rng);
    assert_eq!(p.wreck, None, "a VEHICLE surface is spared the speed rule");
    p.airspeed = F::ZERO;
    p.up_y = F(-0.1);
    p.wreck_check(dt, &w, touch, None, &mut rng);
    assert_eq!(p.wreck, Some(Wreck::Contact), "upside down on anything is a crash");
    assert_eq!(p.body.inertia, Vf::new(5.0, 10.0, 15.0, 1.0));
    assert_eq!(p.body.damping, w.wreck_damping);

    // The chord: not in the air, once a frame, and the frame slot opens it again.
    let mut p = new();
    p.body.vel = Vf::new(1.0, 2.0, 3.0, 1.0);
    p.reset_chord(true);
    assert_eq!(p.body.vel, Vf::new(1.0, 2.0, 3.0, 1.0), "in the air the chord does nothing");
    p.grounded = true;
    p.reset_chord(true);
    assert_eq!(p.body.vel, Vf::new(0.0, 0.0, 0.0, 1.0), "on the wheels it resets");
    p.body.vel = Vf::new(1.0, 2.0, 3.0, 1.0);
    p.reset_chord(true);
    assert_eq!(p.body.vel, Vf::new(1.0, 2.0, 3.0, 1.0), "only once a frame");
    p.frame(dt);
    p.reset_chord(true);
    assert_eq!(p.body.vel, Vf::new(0.0, 0.0, 0.0, 1.0), "the next frame it can again");

    // The edge: a 4096 ft square map, the plane 100 ft in from its west edge.
    let size = map_extent(F(16.0), F(1.0), F(16.0), F(1.0));
    assert_eq!(size, (F(4096.0), F(4096.0)));
    assert!(near_edge(Vf::new(100.0, 0.0, 2048.0, 1.0), size));
    assert!(!near_edge(Vf::new(150.0, 0.0, 3946.0, 1.0), size), "150 ft in is still inside");
    let mut p = new();
    p.body.at = Vf::new(100.0, 500.0, 2048.0, 1.0);
    p.edge_scale = F(2.0);
    p.wreck_check(dt, &w, Contact::default(), Some(size), &mut rng);
    assert_eq!(p.wreck, Some(Wreck::MapEdge));
    assert_eq!(p.clock, dt, "the edge's wreck counts its first tick");
    assert_eq!(p.edge_scale, F(1.0));
    let v = p.body.vel;
    assert!(v.x() > 0.0 && v.y() > 0.0 && v.z() == 0.0, "thrown east and up: {v:?}");
    let s = p.body.spin;
    assert!((0.0..2.0).contains(&s.x()) && (0.0..1.6).contains(&s.y()) && (0.0..2.8).contains(&s.z()));

    // In the air it never comes back; down for 2.5 s then WreckTime, it does.
    for _ in 0..400 {
        p.wreck_check(dt, &w, Contact::default(), Some(size), &mut rng);
    }
    assert_eq!(p.wreck, Some(Wreck::MapEdge), "still flying, still wrecked");
    p.grounded = true;
    let mut ticks = 0;
    while p.wreck.is_some() {
        p.wreck_check(dt, &w, Contact::default(), Some(size), &mut rng);
        ticks += 1;
    }
    assert_eq!(ticks, 152, "one tick to see it is down, then WreckTime");
    assert_eq!(p.edge_scale, F(2.0), "the halving undone");
    assert_eq!(p.body.at, spawn.at);
}
