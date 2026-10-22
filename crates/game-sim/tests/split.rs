//! S93: the user's two split-screen rides, captured frame by frame off the original with
//! watch/split.toml, both riders in one row (the journal 2026-10-21-s93-split-screen-captures says how).
//!
//! - `splitscreen`: the long free ride, 2444 ticks from frame 3113. P1 rides into P2 at 3403 and
//!   goes over the bars, P2 is shoved off and stays up; at 3414 P2 slides back into the wreck; later
//!   P1 (4662) and twice P2 (5075, 5506) ride head-on into a parked biplane and go over the bars.
//! - `splitscreen2`: the short collision, 388 ticks from frame 5765. P2 rides into P1 standing at
//!   5998 and goes over the bars; P1 is shoved off and stays up.
//!
//! Each row holds both riders' collision records: eleven 0x80-byte entries with a kind (0: the
//! rider's own ray, 1: another actor's ray mirrored in) and the record on the other side (0 for
//! scenery). The count is not to be trusted to say a contact happened: a wreck clears it in the tick
//! the entries were written (4662, 5075, 5506 show count 0). So a contact here is an entry naming an
//! actor whose bytes differ from the row before, and a named contact is the first row of a run.
//!
//! The captures are game data, git-ignored: absent, the tests skip.

use ps2_oracle::{Kit, skip};
use std::collections::HashMap;
use std::path::PathBuf;

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

/// One split capture: its header's columns and its rows in order.
struct Split {
    cols: HashMap<String, usize>,
    rows: Vec<Vec<String>>,
}

/// The entries of a rider's collision record the spec reads (the record's capacity).
const ENTRIES: usize = 11;

impl Split {
    fn load(name: &str) -> Option<Split> {
        let text = std::fs::read_to_string(ctx(&format!("capture/split/{name}.csv"))?).expect("read the capture");
        let mut lines = text.lines();
        let cols = lines.next().expect("a header").split(',').enumerate().map(|(i, c)| (c.to_string(), i)).collect();
        let rows = lines.map(|l| l.split(',').map(str::to_string).collect()).collect();
        Some(Split { cols, rows })
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

    fn frame(&self, row: usize) -> u32 {
        self.u32(row, "frame_counter")
    }

    fn row_at(&self, frame: u32) -> usize {
        (0..self.rows.len()).find(|&r| self.frame(r) == frame).unwrap_or_else(|| panic!("no row for frame {frame}"))
    }

    fn speed(&self, row: usize, p: u32) -> f32 {
        let v = ["x", "y", "z"].map(|c| self.f32(row, &format!("p{p}_ch_vel.{c}")));
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }

    /// Rider `p`'s record entries in a row, as their captured text.
    fn entries(&self, row: usize, p: u32) -> Vec<Vec<&str>> {
        let fields = ["kind", "other", "end.x", "end.y", "end.z", "normal.x", "normal.y", "normal.z", "point.x", "point.y", "point.z"];
        (0..ENTRIES).map(|k| fields.iter().map(|f| self.text(row, &format!("p{p}_c{k}.{f}"))).collect()).collect()
    }

    /// The (kind, other record) of every entry rider `p`'s record wrote against an actor in this
    /// row: entries that differ from the row before and name something.
    fn actor_hits(&self, row: usize, p: u32) -> Vec<(u32, u32)> {
        if row == 0 {
            return vec![];
        }
        let (now, before) = (self.entries(row, p), self.entries(row - 1, p));
        (0..ENTRIES)
            .filter(|&k| now[k] != before[k] && now[k][1] != "0")
            .map(|k| (now[k][0].parse().expect("a kind"), now[k][1].parse().expect("a record")))
            .collect()
    }

    /// The frames that start a run of rows in which rider `p` hit an actor.
    fn contact_onsets(&self, p: u32) -> Vec<u32> {
        let hit: Vec<bool> = (0..self.rows.len()).map(|r| !self.actor_hits(r, p).is_empty()).collect();
        (0..self.rows.len()).filter(|&r| hit[r] && (r == 0 || !hit[r - 1])).map(|r| self.frame(r)).collect()
    }

    /// The (frame, reason) of every row where rider `p`'s wreck reason turns non-zero.
    fn wrecks(&self, p: u32) -> Vec<(u32, u32)> {
        let col = format!("p{p}_wreck_reason");
        (0..self.rows.len())
            .filter(|&r| self.u32(r, &col) != 0 && (r == 0 || self.u32(r, &col) != self.u32(r - 1, &col)))
            .map(|r| (self.frame(r), self.u32(r, &col)))
            .collect()
    }

    /// The distinct records rider `p`'s new entries name in a frame.
    fn named(&self, frame: u32, p: u32) -> Vec<u32> {
        let mut v: Vec<u32> = self.actor_hits(self.row_at(frame), p).into_iter().map(|(_, o)| o).collect();
        v.sort();
        v.dedup();
        v
    }
}

/// Both captures, or None (skipped) if either is not there.
fn both() -> Option<(Split, Split)> {
    Some((Split::load("splitscreen")?, Split::load("splitscreen2")?))
}

#[test]
fn both_rides_are_whole_with_both_riders_lanes() {
    let Some((long, short)) = both() else { return };
    for (cap, first, rows) in [(&long, 3113, 2444), (&short, 5765, 388)] {
        assert_eq!(cap.rows.len(), rows, "rows of the ride from {first}");
        for r in 0..rows {
            assert_eq!(cap.frame(r), first + r as u32, "a gap in the ride from {first}");
            assert_eq!(cap.text(r, "dt"), "0.033330000936985016", "dt at {}", cap.frame(r));
            // P1's and P2's ridden vehicles (actor type 2, player 0 and 1, both pads local) and bikes (type 4)
            for (p, index) in [(1, 0), (2, 1)] {
                assert_eq!(cap.u32(r, &format!("p{p}_type")), 2, "p{p} vehicle at {}", cap.frame(r));
                assert_eq!(cap.u32(r, &format!("p{p}_index")), index, "p{p} player at {}", cap.frame(r));
                assert_eq!(cap.u32(r, &format!("p{p}_ctl_src")), 1, "p{p} control at {}", cap.frame(r));
                assert_eq!(cap.u32(r, &format!("p{p}_bike_type")), 4, "p{p} bike at {}", cap.frame(r));
                assert_eq!(cap.u32(r, &format!("p{p}_bike_player")), index, "p{p} bike's player at {}", cap.frame(r));
            }
        }
        // every P1 column has its P2 twin, and P2's pad lane is there next to P1's
        for col in cap.cols.keys().filter(|c| c.starts_with("p1_")) {
            assert!(cap.cols.contains_key(&format!("p2_{}", &col[3..])), "{col} has no p2 twin");
        }
        for col in ["pad0_a", "pad0_b", "pad1_a", "pad1_b", "stamp1_a", "ctl115.held"] {
            assert!(cap.cols.contains_key(col), "no {col}");
        }
    }
    // P2's pad is live in both rides; P1's only in the long one (the short tape leaves port 0 idle)
    let moves = |cap: &Split, col: &str| (1..cap.rows.len()).filter(|&r| cap.text(r, col) != cap.text(0, col)).count();
    assert!(moves(&long, "pad0_a") > 1000 && moves(&long, "pad1_a") > 1000);
    assert_eq!(moves(&short, "pad0_a"), 0);
    assert!(moves(&short, "pad1_a") > 200);
}

#[test]
fn the_named_contacts_are_at_their_ticks() {
    let Some((long, short)) = both() else { return };

    // the long ride: rider meets rider at 3403 and again at 3414, then three biplane crashes
    assert_eq!(long.contact_onsets(1), [3403, 3414, 4662]);
    assert_eq!(long.contact_onsets(2), [3403, 3414, 5075, 5506]);
    // at 3403 each names the other's record: those are the riders' records
    let (r1, r2) = (long.named(3403, 2), long.named(3403, 1));
    assert!(r1.len() == 1 && r2.len() == 1 && r1 != r2, "3403: P2 names {r1:x?}, P1 names {r2:x?}");
    let (r1, r2) = (r1[0], r2[0]);
    // P1 strikes with its own rays (kind 0) and is mirrored into P2's (kind 1), both ways
    let kinds = |cap: &Split, f, p| cap.actor_hits(cap.row_at(f), p).iter().map(|&(k, _)| k).collect::<Vec<_>>();
    assert!(kinds(&long, 3403, 1).contains(&0) && kinds(&long, 3403, 2).contains(&1));
    // P1, at speed, goes over the bars a row later; P2, standing, is shoved off in the contact row and stays up
    assert!(long.speed(long.row_at(3402), 1) > 50.0 && long.speed(long.row_at(3402), 2) < 1.0);
    assert!(long.speed(long.row_at(3403), 2) > 20.0, "P2 is not pushed");
    // 3414: P2 slides into the wreck: P1's frozen record and one more (P1's wreck bike), no wreck
    let at3414 = long.named(3414, 2);
    assert!(at3414.contains(&r1) && at3414.len() == 2, "3414: P2 names {at3414:x?}");
    // the biplane: one record, neither rider's, met head-on by P1 once and by P2 twice
    let plane = long.named(4662, 1);
    assert!(plane.len() == 1 && plane[0] != r1 && plane[0] != r2, "4662: P1 names {plane:x?}");
    assert_eq!(long.named(5075, 2), plane);
    assert_eq!(long.named(5506, 2), plane);
    assert_eq!(long.wrecks(1), [(3404, 1), (4081, 8), (4662, 1)]);
    assert_eq!(long.wrecks(2), [(5075, 1), (5506, 1)]);

    // the short ride: P2 into P1 standing at 5998; the riders' records are the long ride's
    assert_eq!(short.contact_onsets(1), [5998]);
    assert_eq!(short.contact_onsets(2), [5998]);
    assert_eq!(short.named(5998, 1), [r2]);
    assert_eq!(short.named(5998, 2), [r1]);
    assert!(kinds(&short, 5998, 2).iter().all(|&k| k == 0) && kinds(&short, 5998, 1).iter().all(|&k| k == 1));
    assert!(short.speed(short.row_at(5997), 2) > 50.0 && short.speed(short.row_at(5997), 1) < 1.0);
    assert!(short.speed(short.row_at(5998), 1) > 20.0, "P1 is not pushed");
    assert!(short.wrecks(1).is_empty());
    assert_eq!(short.wrecks(2), [(5998, 1)]);
}
