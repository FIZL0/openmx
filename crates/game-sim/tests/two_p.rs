//! C10: the user's 2p recording, the human recording of record for wrecks, collisions, the ragdoll,
//! rider-vs-rider, riding the trucks and the animations. Captured frame by frame off the original with
//! watch/2p.toml (research/journal/2026-10-26-c10-2p-capture says how, and indexes every event).
//!
//! `context/capture/2p/2p.csv`: 12,917 rows, one a tick, frames 127882..140798 (the state ticks at
//! 60 Hz and the playback reads one tape field a tick), both riders in every row (`p1_`/`p2_`): the
//! vehicle, its chassis and collision record, C6's air, backflip and trick words, each wheel's path
//! car, the bike actor, the rider body's mode, machine and clock, its 22 posed joints and 18 ragdoll
//! bodies; then the six path cars.
//!
//! The event tables below are research/c10/index.py's index of that capture (research/c10/consts.py
//! prints them); each test finds its events again in the rows, at the same ticks.
//!
//! The capture is game data, git-ignored: absent, the tests skip.

use ps2_oracle::{Kit, skip};
use std::collections::HashMap;
use std::io::BufRead;
use std::path::PathBuf;
use std::sync::OnceLock;

fn ctx(rel: &str) -> Option<PathBuf> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let local = kit.root.join("context").join(rel);
    let path = if local.exists() { local } else { kit.ctx(rel) };
    if !path.exists() {
        skip(&format!("{} is not there", path.display()));
        return None;
    }
    Some(path)
}

/// The capture, columns by name. The joints and bones are two thirds of the file: of those only the
/// first joint and the first ragdoll body are kept, to prove both lanes carry them.
struct TwoP {
    cols: HashMap<String, usize>,
    rows: Vec<Vec<String>>,
}

fn keep(col: &str) -> bool {
    let rest = col.strip_prefix("p1_").or_else(|| col.strip_prefix("p2_")).unwrap_or("");
    let numbered = |pre: &str| rest.strip_prefix(pre).is_some_and(|t| t.starts_with(|c: char| c.is_ascii_digit()));
    !(numbered("j") || numbered("bone")) || rest.starts_with("j0.") || rest.starts_with("bone0.")
}

/// Read once for every test in this file.
fn capture() -> Option<&'static TwoP> {
    static CAPTURE: OnceLock<Option<TwoP>> = OnceLock::new();
    CAPTURE.get_or_init(|| TwoP::load(keep)).as_ref()
}

impl TwoP {
    /// `keep` says which columns to hold on to.
    fn load(keep: impl Fn(&str) -> bool) -> Option<TwoP> {
        let file = std::fs::File::open(ctx("capture/2p/2p.csv")?).expect("open the capture");
        let mut lines = std::io::BufReader::new(file).lines().map(|l| l.expect("read the capture"));
        let head = lines.next().expect("a header");
        let header: Vec<&str> = head.split(',').collect();
        let wanted: Vec<usize> = (0..header.len()).filter(|&i| keep(header[i])).collect();
        let cols = wanted.iter().enumerate().map(|(k, &i)| (header[i].to_string(), k)).collect();
        let rows = lines
            .map(|l| {
                let all: Vec<&str> = l.split(',').collect();
                assert_eq!(all.len(), header.len(), "a short row");
                wanted.iter().map(|&i| all[i].to_string()).collect()
            })
            .collect();
        Some(TwoP { cols, rows })
    }

    fn text(&self, row: usize, col: &str) -> &str {
        &self.rows[row][*self.cols.get(col).unwrap_or_else(|| panic!("no column {col}"))]
    }

    fn u32(&self, row: usize, col: &str) -> u32 {
        let s = self.text(row, col);
        s.parse().unwrap_or_else(|_| panic!("{col} in row {row}: {s:?}"))
    }

    fn f32(&self, row: usize, col: &str) -> f32 {
        let s = self.text(row, col);
        s.parse::<f64>().unwrap_or_else(|_| panic!("{col} in row {row}: {s:?}")) as f32
    }

    /// A 32-bit word out of a `bytes:N` column, at a byte offset into it.
    fn word(&self, row: usize, col: &str, off: usize) -> u32 {
        let hex: String = self.text(row, col).chars().filter(|c| !c.is_whitespace()).collect();
        let b = |i: usize| u32::from_str_radix(&hex[2 * (off + i)..2 * (off + i) + 2], 16).expect("hex");
        b(0) | b(1) << 8 | b(2) << 16 | b(3) << 24
    }

    fn frame(&self, row: usize) -> u32 {
        self.u32(row, "frame_counter")
    }

    fn row_at(&self, frame: u32) -> usize {
        let r = (frame - self.frame(0)) as usize;
        assert_eq!(self.frame(r), frame, "the capture is not contiguous");
        r
    }

    fn pos(&self, row: usize, p: u32) -> [f32; 3] {
        [0, 1, 2].map(|k| self.f32(row, &format!("p{p}_bike_xform.m3{k}")))
    }

    /// Path car `c`'s position (row 3 of its world matrix).
    fn car(&self, row: usize, c: usize) -> [f32; 3] {
        [0, 1, 2].map(|k| self.f32(row, &format!("car{c}_xform.m3{k}")))
    }

    /// The tick's peak g on rider `p`'s vehicle (x side, y up, z fore-aft).
    fn g(&self, row: usize, p: u32) -> [f32; 3] {
        ["x", "y", "z"].map(|c| self.f32(row, &format!("p{p}_g_peak.{c}")))
    }

    /// Rider `p`'s chassis pitch and roll, degrees.
    fn pitch_roll(&self, row: usize, p: u32) -> (f32, f32) {
        let e = |c: &str| self.f32(row, &format!("p{p}_ch_euler.{c}")).to_degrees();
        (e("x"), e("z"))
    }

    /// Rider `p`'s animation machine state: the clip it plays (the body's machine word at +0x14).
    fn clip(&self, row: usize, p: u32) -> u32 {
        self.word(row, &format!("p{p}_d_cross"), 0x14)
    }

    /// The first rows of the stretches where `pred` holds, as frames, and the last row of each.
    fn stretches(&self, pred: impl Fn(usize) -> bool) -> Vec<(u32, u32)> {
        let mut out = vec![];
        let mut start = None;
        for r in 0..self.rows.len() {
            match (pred(r), start) {
                (true, None) => start = Some(r),
                (false, Some(s)) => {
                    out.push((self.frame(s), self.frame(r - 1)));
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(s) = start {
            out.push((self.frame(s), self.frame(self.rows.len() - 1)));
        }
        out
    }

    /// The (frame, reason) of every row where rider `p`'s wreck reason turns non-zero.
    fn wrecks(&self, p: u32) -> Vec<(u32, u32)> {
        let col = format!("p{p}_wreck_reason");
        (1..self.rows.len())
            .filter(|&r| self.u32(r, &col) != 0 && self.u32(r - 1, &col) == 0)
            .map(|r| (self.frame(r), self.u32(r, &col)))
            .collect()
    }

    /// Rider `p`'s record entries in a row, as their captured text.
    fn entries(&self, row: usize, p: u32) -> Vec<[&str; 4]> {
        (0..11)
            .map(|k| ["kind", "other", "end.y", "normal.x"].map(|f| self.text(row, &format!("p{p}_c{k}.{f}"))))
            .collect()
    }

    /// The other record of every entry of rider `p`'s record that changed since the row before
    /// (0 is scenery).
    fn new_others(&self, row: usize, p: u32) -> Vec<u32> {
        if row == 0 {
            return vec![];
        }
        let (now, before) = (self.entries(row, p), self.entries(row - 1, p));
        (0..11).filter(|&k| now[k] != before[k]).map(|k| now[k][1].parse().expect("a record")).collect()
    }

    /// Riding, as the near-miss index means it: no wreck reason within 10 ticks either side, and body
    /// mode 1 (riding) for the 10 ticks up to this one (a put-back teleports the bike).
    fn riding(&self, row: usize, p: u32) -> bool {
        let (wreck, mode) = (format!("p{p}_wreck_reason"), format!("p{p}_mode"));
        row >= 10
            && row + 10 < self.rows.len()
            && (row - 10..=row + 10).all(|r| self.u32(r, &wreck) == 0)
            && (row - 10..=row).all(|r| self.u32(r, &mode) == 1)
    }

    fn airborne(&self, row: usize, p: u32) -> bool {
        self.u32(row, &format!("p{p}_airborne")) != 0
    }
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The near-miss clauses, in research/c10/index.py's order: what each judges and its limit
/// (Race500P.phy's, both riders).
const NEAR_CLAUSES: [&str; 9] = [
    "fore-aft g (front)",
    "fore-aft g (back)",
    "side g",
    "vertical g (flat)",
    "ground pitch down (deg)",
    "ground pitch up (deg)",
    "touch pitch down (deg)",
    "touch pitch up (deg)",
    "touch |roll| (deg)",
];

/// The clip groups the animation index lists (machine states, `rider_anim`'s clip table order), and
/// last the put-backs (body mode 0).
const ANIM_GROUPS: [(&str, &[u32]); 7] = [
    ("foot down", &[24, 25, 26, 27, 28, 29]),
    ("reverse", &[33, 34]),
    ("backflip clip", &[38]),
    ("no-handed lander", &[39]),
    ("wreck flail", &[40]),
    ("hold (after a wreck)", &[41, 42, 43, 44, 45]),
    ("put-back", &[]),
];

// generated by research/c10/consts.py from the index of the capture of record
const ROWS: usize = 12917;
const FIRST: u32 = 127882;
/// (rider, frame, reason): the row the wreck reason turns non-zero.
const WRECKS: &[(u32, u32, u32)] = &[
    (1, 128995, 8), (1, 129945, 8), (1, 130966, 11), (1, 132997, 3), (1, 133313, 12), (1, 133953, 12),
    (1, 134381, 12), (1, 135196, 12), (1, 135855, 12), (1, 136559, 12), (1, 139659, 2), (2, 130418, 6),
    (2, 130897, 6), (2, 131595, 1), (2, 133800, 8), (2, 134315, 12), (2, 140363, 2),
];
/// (rider, first, last frame) of each body-mode-2 stretch.
const RAGDOLLS: &[(u32, u32, u32)] = &[
    (1, 128996, 129060), (1, 129946, 130033), (1, 130967, 131127), (1, 132998, 133152), (1, 133314, 133472),
    (1, 133954, 134108), (1, 134382, 134536), (1, 135197, 135351), (1, 135856, 136010), (1, 136560, 136714),
    (1, 139660, 139814), (2, 130419, 130620), (2, 130898, 131052), (2, 131597, 131754), (2, 133802, 134010),
    (2, 134316, 134471), (2, 140365, 140519),
];
/// (rider, frame): a record entry on scenery starts.
const CONTACTS: &[(u32, u32)] = &[
    (1, 131483), (2, 130413), (2, 130417), (2, 130896),
];
/// (rider, frame, clause in `NEAR_CLAUSES`, value): ridden out at half its limit or more.
const NEAR: &[(u32, u32, usize, f32)] = &[
    (1, 129486, 3, 55.874), (1, 131872, 3, 53.274), (1, 132463, 3, 34.328), (1, 130420, 4, 67.43),
    (1, 129485, 4, 60.252), (1, 129124, 4, 40.542), (1, 131628, 5, 48.143), (1, 128520, 5, 43.275),
    (1, 128892, 5, 43.028), (1, 130420, 6, 67.43), (1, 129485, 6, 60.252), (1, 129123, 6, 40.386),
    (1, 131484, 8, 85.949), (1, 132608, 8, 69.417), (1, 135076, 8, 50.019), (2, 133538, 3, 64.094),
    (2, 128851, 3, 48.348), (2, 128467, 3, 44.553), (2, 128849, 4, 50.332), (2, 133253, 4, 45.492),
    (2, 131576, 4, 41.341), (2, 136062, 5, 56.896), (2, 133744, 5, 52.783), (2, 136142, 5, 50.572),
    (2, 128849, 6, 50.332), (2, 131576, 6, 41.341), (2, 136062, 7, 56.896), (2, 135556, 8, 49.217),
    (2, 137982, 8, 43.005),
];
/// (rider, first, last frame, trick id) of each trick_running stretch.
const TRICKS: &[(u32, u32, u32, u32)] = &[
    (1, 128539, 128626, 4), (1, 130827, 130953, 1), (1, 133307, 133471, 12), (1, 133931, 134107, 5),
    (1, 134352, 134535, 3), (1, 135190, 135350, 6), (1, 135843, 136009, 0), (1, 136512, 136713, 3),
    (2, 128315, 128413, 7), (2, 128679, 128797, 7), (2, 128961, 129045, 2), (2, 130038, 130270, 7),
    (2, 134291, 134470, 7),
];
/// (rider, first, last frame) of each flip_on stretch.
const FLIPS: &[(u32, u32, u32)] = &[
    (2, 128653, 128798), (2, 128931, 128948), (2, 130016, 130269), (2, 138422, 138444),
];
/// (rider, group in `ANIM_GROUPS`, first, last frame) of each stretch.
const ANIMS: &[(u32, usize, u32, u32)] = &[
    (1, 1, 140354, 140396), (1, 4, 128996, 129060), (1, 4, 129946, 130033), (1, 4, 130967, 131084),
    (1, 4, 132998, 133043), (1, 4, 133314, 133419), (1, 4, 133954, 134045), (1, 4, 134382, 134463),
    (1, 4, 135197, 135247), (1, 4, 135856, 135952), (1, 4, 136560, 136618), (1, 4, 139660, 139706),
    (1, 5, 131085, 131127), (1, 5, 133044, 133152), (1, 5, 133420, 133472), (1, 5, 134046, 134108),
    (1, 5, 134464, 134536), (1, 5, 135248, 135351), (1, 5, 135953, 136010), (1, 5, 136619, 136714),
    (1, 5, 139707, 139814), (1, 6, 129061, 129063), (1, 6, 130034, 130035), (1, 6, 130435, 130436),
    (1, 6, 131128, 131129), (1, 6, 131972, 131973), (1, 6, 132729, 132730), (1, 6, 133153, 133154),
    (1, 6, 133473, 133474), (1, 6, 134109, 134110), (1, 6, 134537, 134538), (1, 6, 135352, 135353),
    (1, 6, 136011, 136012), (1, 6, 136134, 136135), (1, 6, 136715, 136716), (1, 6, 139815, 139816),
    (1, 6, 140142, 140143), (2, 1, 132128, 132152), (2, 1, 132646, 132688), (2, 1, 138871, 139066),
    (2, 2, 128664, 128678), (2, 2, 128799, 128799), (2, 2, 128943, 128949), (2, 2, 130019, 130037),
    (2, 2, 138440, 138445), (2, 4, 130419, 130537), (2, 4, 130898, 131017), (2, 4, 131597, 131671),
    (2, 4, 133802, 133927), (2, 4, 134316, 134388), (2, 4, 140365, 140433), (2, 5, 130538, 130620),
    (2, 5, 131018, 131052), (2, 5, 131672, 131754), (2, 5, 133928, 134010), (2, 5, 134389, 134471),
    (2, 5, 140434, 140519), (2, 6, 130621, 130622), (2, 6, 131053, 131054), (2, 6, 131755, 131756),
    (2, 6, 134011, 134012), (2, 6, 134472, 134473), (2, 6, 138118, 138119), (2, 6, 140520, 140521),
];
/// (rider, frame, roll in radians): the three deepest leans each way, riding on the ground.
const LEANS: &[(u32, u32, f32)] = &[
    (1, 128904, -0.9247387647628784), (1, 139116, -0.8302972316741943), (1, 128527, -0.8282827138900757),
    (1, 131484, 1.5000866651535034), (1, 131443, 1.3850473165512085), (1, 132590, 1.2144668102264404),
    (2, 135846, -1.1384403705596924), (2, 140057, -0.9986099004745483), (2, 130707, -0.8255370259284973),
    (2, 133177, 1.1458460092544556), (2, 135565, 1.082492470741272), (2, 132865, 0.976841151714325),
];
// end generated

/// Every row there, one a tick, both lanes whole: the guards hold, both pads are played, both riders
/// ride and both bodies are posed.
#[test]
fn the_whole_ride_both_lanes() {
    let Some(cap) = capture() else { return };
    assert_eq!(cap.rows.len(), ROWS, "rows");
    assert_eq!(cap.frame(0), FIRST);
    for r in 0..cap.rows.len() {
        assert_eq!(cap.frame(r), FIRST + r as u32, "a gap before row {r}");
        let dt = cap.f32(r, "dt");
        assert!((dt - 1.0 / 60.0).abs() < 1e-4, "dt {dt} in row {r}: the 2p state ticks at 60 Hz");
        for p in 1..=2 {
            let at = |c: &str| cap.u32(r, &format!("p{p}_{c}"));
            assert_eq!((at("type"), at("index"), at("bike_type"), at("bike_player")), (2, p - 1, 4, p - 1),
                       "P{p}'s guards in row {r}: the actor list moved");
            for c in ["j0.t.x", "bone0.pos.x"] {
                assert!(cap.f32(r, &format!("p{p}_{c}")).is_finite(), "P{p} {c} in row {r}");
            }
        }
    }
    for (p, pad) in [(1, "pad0_a"), (2, "pad1_a")] {
        // bytes 2..8: the buttons and sticks, idle ff ff 7f 7f 7f 7f
        let idle = |r: usize| {
            let hex: String = cap.text(r, pad).chars().filter(|c| !c.is_whitespace()).collect();
            hex[4..16].eq_ignore_ascii_case("ffff7f7f7f7f")
        };
        let live = (0..cap.rows.len()).filter(|&r| !idle(r)).count();
        assert!(live > 5000, "P{p}'s pad is live in {live} rows");
        let ridden: f32 = (1..cap.rows.len()).map(|r| dist(cap.pos(r, p), cap.pos(r - 1, p)).min(5.0)).sum();
        assert!(ridden > 5_000.0, "P{p} rides {ridden} ft");
    }
}

#[test]
fn every_wreck_at_its_tick() {
    let Some(cap) = capture() else { return };
    let got: Vec<_> = (1..=2).flat_map(|p| cap.wrecks(p).into_iter().map(move |(f, w)| (p, f, w))).collect();
    assert_eq!(got, WRECKS);
}

#[test]
fn every_ragdoll_and_put_back_at_its_tick() {
    let Some(cap) = capture() else { return };
    let mut got = vec![];
    for p in 1..=2 {
        let mode = format!("p{p}_mode");
        got.extend(cap.stretches(|r| cap.u32(r, &mode) == 2).into_iter().map(|(a, b)| (p, a, b)));
    }
    assert_eq!(got, RAGDOLLS);
    // every ragdoll is a wreck's, starting within two ticks of it
    for &(p, from, _) in RAGDOLLS {
        assert!(WRECKS.iter().any(|&(q, f, _)| q == p && from >= f && from <= f + 2), "P{p}'s ragdoll at {from}");
    }
}

/// The world contacts: a record entry on scenery starting (the wheels' ground is not in the record).
#[test]
fn every_world_contact_at_its_tick() {
    let Some(cap) = capture() else { return };
    let mut got = vec![];
    for p in 1..=2 {
        for r in 1..cap.rows.len() {
            if cap.new_others(r, p).contains(&0) && !cap.new_others(r - 1, p).contains(&0) {
                got.push((p, cap.frame(r)));
            }
        }
    }
    assert_eq!(got, CONTACTS);
}

/// What the user expected and this ride does not hold: no record entry ever names another record (a
/// rider, a biplane, a path car or a prop), no wheel stands on a path car, and the riders never come
/// within 100 ft of each other or 150 ft of a car. Both PCSX2 builds play it so (the journal).
#[test]
fn no_rider_vs_rider_nor_truck_contact_in_this_ride() {
    let Some(cap) = capture() else { return };
    let (mut riders, mut car) = (f32::MAX, f32::MAX);
    for r in 0..cap.rows.len() {
        for p in 1..=2 {
            for k in 0..11 {
                assert_eq!(cap.u32(r, &format!("p{p}_c{k}.other")), 0, "P{p}'s entry {k} in row {r}");
            }
            for w in ["wf", "wr"] {
                assert_eq!(cap.u32(r, &format!("p{p}_{w}_car")), 0, "P{p}'s {w} on a car in row {r}");
            }
            for c in 0..6 {
                car = car.min(dist(cap.pos(r, p), cap.car(r, c)));
            }
        }
        riders = riders.min(dist(cap.pos(r, 1), cap.pos(r, 2)));
    }
    assert!(riders > 100.0, "the riders come within {riders} ft");
    assert!(car > 150.0, "a rider comes within {car} ft of a path car");
}

/// Each near miss rides out at its tick with the value indexed, at half its clause's limit or more.
#[test]
fn every_near_miss_at_its_tick() {
    let Some(cap) = capture() else { return };
    assert!(!NEAR.is_empty());
    for &(p, frame, clause, value) in NEAR {
        let r = cap.row_at(frame);
        assert!(cap.riding(r, p), "P{p} not riding at {frame}");
        let (g, (pitch, roll)) = (cap.g(r, p), cap.pitch_roll(r, p));
        let (got, limit) = match clause {
            0 => (-g[2], 18.3333),
            1 => (g[2], 21.9999),
            2 => (g[0].abs(), 19.0666),
            3 => (g[1], 65.9997),
            4 => (-pitch, 70.0),
            5 => (pitch, 70.0),
            6 => (-pitch, 80.0),
            7 => (pitch, 85.0),
            _ => (roll.abs(), 85.0),
        };
        let name = NEAR_CLAUSES[clause];
        assert!((got - value).abs() < 1e-2, "P{p} {name} at {frame}: {got}, indexed {value}");
        assert!(got >= limit / 2.0, "P{p} {name} at {frame}: {got} is under half of {limit}");
        if clause >= 4 {
            assert!(!cap.airborne(r, p), "P{p} {name} at {frame}: airborne");
        }
        if clause >= 6 {
            assert!(cap.airborne(r - 1, p), "P{p} {name} at {frame}: not a touch");
        }
    }
}

#[test]
fn every_trick_and_backflip_at_its_tick() {
    let Some(cap) = capture() else { return };
    let (mut tricks, mut flips) = (vec![], vec![]);
    for p in 1..=2 {
        let (run, id, flip) = (format!("p{p}_trick_running"), format!("p{p}_trick_id"), format!("p{p}_flip_on"));
        for (a, b) in cap.stretches(|r| cap.u32(r, &run) != 0) {
            tricks.push((p, a, b, cap.u32(cap.row_at(a), &id)));
        }
        flips.extend(cap.stretches(|r| cap.u32(r, &flip) != 0).into_iter().map(|(a, b)| (p, a, b)));
    }
    assert_eq!(tricks, TRICKS);
    assert_eq!(flips, FLIPS);
}

/// Foot down, reverse, the backflip and lander clips, the wreck flail and holds, the put-backs, and
/// the deepest leans each way.
#[test]
fn every_animation_at_its_tick() {
    let Some(cap) = capture() else { return };
    let mut got = vec![];
    for p in 1..=2 {
        let mode = format!("p{p}_mode");
        for (g, (_, ids)) in ANIM_GROUPS.iter().enumerate() {
            let runs = if ids.is_empty() {
                cap.stretches(|r| cap.u32(r, &mode) == 0)
            } else {
                cap.stretches(|r| ids.contains(&cap.clip(r, p)))
            };
            got.extend(runs.into_iter().map(|(a, b)| (p, g, a, b)));
        }
    }
    assert_eq!(got, ANIMS);
    for p in 1..=2 {
        let ground = |r: usize| {
            cap.u32(r, &format!("p{p}_mode")) == 1
                && !cap.airborne(r, p)
                && cap.u32(r, &format!("p{p}_wreck_reason")) == 0
        };
        let roll = |r: usize| cap.f32(r, &format!("p{p}_ch_euler.z"));
        let rows: Vec<usize> = (0..cap.rows.len()).filter(|&r| ground(r)).collect();
        let lo = rows.iter().copied().min_by(|&a, &b| roll(a).total_cmp(&roll(b))).expect("riding rows");
        let hi = rows.iter().copied().max_by(|&a, &b| roll(a).total_cmp(&roll(b))).expect("riding rows");
        let mine: Vec<_> = LEANS.iter().filter(|l| l.0 == p).collect();
        assert_eq!(mine.len(), 6, "P{p}: three leans each way");
        for extreme in [lo, hi] {
            assert!(mine.iter().any(|l| l.1 == cap.frame(extreme)), "P{p}'s deepest lean at {}", cap.frame(extreme));
        }
        for &&(_, frame, value) in &mine {
            let r = cap.row_at(frame);
            assert!(ground(r), "P{p}'s lean at {frame} is not riding on the ground");
            assert!((roll(r) - value).abs() < 1e-5, "P{p}'s lean at {frame}: {}, indexed {value}", roll(r));
        }
    }
}
