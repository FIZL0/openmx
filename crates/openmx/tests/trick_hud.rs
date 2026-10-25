//! P68 — the original HUD's trick pop-up, its camera text and its score.
//!
//! The original's score card, captured off PCSX2 riding the human's recording2 (S79's
//! `recording2-score.csv`), is replayed into the HUD's tracker: what it pops up is held to what the
//! original's text setter writes and to the frames filmed of that ride, a landed chain's total to
//! the original's second, and a bail plays out with nothing banked. A camera press shows the
//! preset's name for the original's three seconds. With the disc, the Stunt movie and the info text
//! are drawn where `stuntscoreoverlays.lyt` and `infotextoverlay1.lyt` put them, in a whole screen
//! and in each split, at the size the original's are on a PCSX2 frame.

#[path = "../src/menu.rs"]
#[allow(dead_code)]
mod menu;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;
#[path = "../src/roster.rs"]
#[allow(dead_code)]
mod roster;
#[path = "../src/chase.rs"]
#[allow(dead_code)]
mod chase;
#[path = "../src/join.rs"]
#[allow(dead_code)]
mod join;
#[path = "../src/split.rs"]
#[allow(dead_code)]
mod split;
#[path = "../src/overlay.rs"]
#[allow(dead_code)]
mod overlay;
#[path = "../src/ride_hud.rs"]
#[allow(dead_code)]
mod ride_hud;

use game_render::{Disc, Frame};
use game_sim::score::{Node, Score};
use overlay::{Info, Pops, Stunt};
use ps2_float::F;
use ps2_oracle::{Kit, skip};
use ride_hud::Tracker;
use std::time::{Duration, Instant};

/// The ids recording2's tricks score as, named as the film shows them.
fn name(id: u8) -> String {
    match id {
        0 => "Taunt".into(),
        2 => "CanCan".into(),
        24 => "BackFlip".into(),
        _ => format!("Trick {id}"),
    }
}

/// The capture's card, a row a tick: the frame, then the card as a [`Score`] (its older nodes as
/// they last stood, since the capture holds only the newest).
fn recording2() -> Option<Vec<(u32, Score)>> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let path = kit.ctx("capture").join("s79").join("recording2-score.csv");
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!("no {} — S79's journal has the recipe", path.display()));
        return None;
    };
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let col = |n: &str| head.iter().position(|c| *c == n).unwrap_or_else(|| panic!("no column {n}"));
    let [frame, count, id, points, flip, total, flags] =
        ["frame", "card_count", "node_id", "node_points", "flip_stage", "card_total", "card_flags"].map(col);
    let mut nodes: Vec<Node> = Vec::new();
    let mut rows = Vec::new();
    for line in lines {
        let c: Vec<&str> = line.split(',').collect();
        let num = |k: usize| -> f32 { c[k].parse().unwrap_or(0.0) };
        let n = num(count) as usize;
        if n == 0 {
            nodes.clear();
        } else {
            while nodes.len() < n {
                nodes.insert(0, Node::default());
            }
            nodes[0].id = num(id) as u8;
            nodes[0].points = F(num(points));
        }
        // The card's 0x20 flag: a combo is open.
        let open = n > 0 && (num(flags) as u32) & 0x20 != 0;
        let score = Score { nodes: nodes.clone(), combo: open, total: F(num(total)), flip: num(flip) as u8, ..Score::default() };
        rows.push((num(frame) as u32, score));
    }
    Some(rows)
}

/// A tick of the capture is a thirtieth of a second.
fn at(t0: Instant, frame: u32) -> Instant {
    t0 + Duration::from_secs_f64(frame as f64 / 30.0)
}

fn texts(p: &Pops) -> Option<(&str, &str)> {
    p.stunt.as_ref().map(|s| (s.name.as_str(), s.points.as_str()))
}

#[test]
fn the_stunt_lines_are_the_setters_formats() {
    assert_eq!(overlay::stunt_name("Taunt", true), "BackFlip + Taunt");
    assert_eq!(overlay::stunt_name("CanCan", false), "CanCan");
    assert_eq!(overlay::stunt_points(3740.0, 1), "3740");
    assert_eq!(overlay::stunt_points(20000.0, 2), "10000  X 2");
    assert_eq!(overlay::stunt_points(9001.5, 3), "3000  X 3");
    assert_eq!(overlay::combo_total(2), "2 Stunt Combo Total");
}

/// recording2's tricks through the tracker, held to the film of the same ride (P140's frames of
/// P121's Video Capture): `BackFlip + Taunt` counting 3240…3990, `BackFlip` / `10000 X 2`,
/// `2 Stunt Combo Total` / `24160` for a second, `CanCan` / `3740`; and the Taunt it bails on at
/// 3330 plays out banking nothing.
#[test]
fn a_replayed_trick_shows_its_name_and_points_for_the_originals_time() {
    let Some(rows) = recording2() else { return };
    let t0 = Instant::now();
    let mut t = Tracker::default();
    let mut seen = std::collections::BTreeMap::new();
    let watch = [2550, 2581, 2591, 2594, 2595, 2600, 2624, 2700, 2716, 3561, 3562];
    for (frame, score) in &rows {
        t.update(at(t0, *frame), score, name);
        if watch.contains(frame) {
            seen.insert(*frame, (t.pops(at(t0, *frame)), score.total.0));
        }
        // After the bank the total holds until a second has gone, then plays out.
        if (2596..2595 + 30).contains(frame) {
            let p = t.pops(at(t0, *frame));
            assert_eq!(texts(&p), Some(("2 Stunt Combo Total", "24160")), "frame {frame}");
            assert_eq!(p.stunt.as_ref().unwrap().alpha, 1.0, "frame {frame}");
        }
        if *frame == 2595 + 30 + 8 {
            assert!(texts(&t.pops(at(t0, *frame))).is_none(), "the total is gone 1.25 s after the bank");
        }
        if *frame == 3562 + 8 {
            assert!(texts(&t.pops(at(t0, *frame))).is_none(), "the bailed trick has played out");
        }
    }
    assert!(seen.len() == watch.len(), "the capture runs past the tricks: {:?}", seen.keys());
    let shown = |f: u32| texts(&seen[&f].0).unwrap_or_else(|| panic!("nothing up at {f}"));
    // A flip coming round under the Taunt (stage 2, then 3 once it is round): the setter's
    // `BackFlip + %s`. The film shows it at stage 3; stage 2 is the same flag as far as is known.
    assert_eq!(shown(2550), ("BackFlip + Taunt", "660"));
    assert_eq!(shown(2581), ("BackFlip + Taunt", "3240"));
    assert_eq!(shown(2591), ("BackFlip + Taunt", "3990"));
    // The flip scores: the newest node over the combo's length.
    assert_eq!(shown(2594), ("BackFlip", "10000  X 2"));
    // Landed: (10000 + 2080) × 2 banked.
    assert_eq!(seen[&2595].1, 24160.0);
    assert_eq!(shown(2595), ("2 Stunt Combo Total", "24160"));
    assert_eq!(shown(2700), ("CanCan", "3740"));
    // A single trick landed plays out over the original's quarter second, its points as they were.
    assert_eq!(shown(2716), ("CanCan", "3740"));
    assert_eq!(seen[&2716].1, 27900.0);
    // The bail: the Taunt stays to play out, the total does not move.
    assert_eq!(shown(3561), ("Taunt", "3330"));
    let bail = &seen[&3562];
    assert_eq!(texts(&bail.0), Some(("Taunt", "3330")));
    assert_eq!(bail.1, 55190.0, "nothing banked on a bail");
}

/// A camera press shows the new preset's name, as the original's info text does, for its three
/// seconds; the ride's first camera is not a press.
#[test]
fn a_camera_press_shows_the_preset_name() {
    let t0 = Instant::now();
    let mut t = Tracker::default();
    t.note_camera(t0, "Chase Cam");
    assert!(t.pops(t0).info.is_none(), "the ride's first camera shows nothing");
    let pressed = t0 + Duration::from_secs(5);
    t.note_camera(pressed, "Helmet Cam");
    let info = |d: f32| t.pops(pressed + Duration::from_secs_f32(d)).info.map(|i| (i.text, i.alpha));
    assert_eq!(info(0.0), Some(("Helmet Cam".to_string(), 1.0)));
    assert_eq!(info(1.5).map(|i| i.1), Some(1.0), "it stands");
    assert!(info(2.95).is_some());
    assert!(info(overlay::INFO_SECONDS).is_none(), "gone after the original's {} s", overlay::INFO_SECONDS);
    assert_eq!(overlay::INFO_SECONDS, 3.0);
    assert!(t.pops(pressed).stunt.is_none(), "no trick, no Stunt movie");
}

fn art() -> Option<overlay::Art> {
    match Disc::open_configured() {
        Ok(mut disc) => Some(overlay::Art::load(&mut disc).expect("fstyleui.res's HUD art")),
        Err(e) => {
            skip(&format!("no disc: {e}"));
            None
        }
    }
}

/// The ink of `frame` whose pixels pass `is`, between rows `rows`: left, top, right, bottom.
fn ink(frame: &Frame, rows: std::ops::Range<usize>, is: impl Fn(&[u8]) -> bool) -> Option<[usize; 4]> {
    let mut b: Option<[usize; 4]> = None;
    for y in rows.start..rows.end.min(frame.height) {
        for x in 0..frame.width {
            let at = 3 * (y * frame.width + x);
            if is(&frame.color[at..at + 3]) {
                let o = b.get_or_insert([x, y, x, y]);
                *o = [o[0].min(x), o[1].min(y), o[2].max(x), o[3].max(y)];
            }
        }
    }
    b
}

fn orange(c: &[u8]) -> bool {
    c[0] > 120 && c[1] > 50 && c[1] < c[0] && c[2] < 40
}

fn white(c: &[u8]) -> bool {
    c.iter().all(|&v| v > 150)
}

fn pops(name: &str, points: &str, info: Option<&str>) -> Pops {
    Pops {
        stunt: Some(Stunt { name: name.into(), points: points.into(), scale: 1.0, alpha: 1.0, newest: (1.0, 1.0) }),
        info: info.map(|i| Info { text: i.into(), alpha: 1.0 }),
    }
}

/// On a whole 640x448 screen the Stunt movie's two lines sit and measure as the original's do on a
/// PCSX2 frame of recording2 (P121's `pcsx2-combo.png`, its 448 lines stretched to 480): `BackFlip +
/// Taunt` inked over x 244–400, 11 lines tall from 355; `3160` 74 wide and 21 tall from 374, both
/// centred near x 320 (the points 321.5) and orange. The info text's top is on line 399. In a split each view puts them
/// at its own layout's place: 182 down a half-height view, 360 down a half-width one.
#[test]
fn the_original_overlay_draws_them_where_its_layouts_do() {
    let Some(art) = art() else { return };
    let mut frame = Frame::new(640, 448, [0, 0, 0]);
    let drawn = art.draw_pops(&mut frame, (640, 448), 0, &pops("BackFlip + Taunt", "3160", None));
    assert_eq!(drawn.stunt.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(), ["BackFlip + Taunt", "3160"]);
    assert!(drawn.info.is_none());
    // The two lines' glyph boxes overlap; their ink does not, the original's parting at line 370.
    let split = 370;
    let name = ink(&frame, 300..split, orange).expect("the name is orange");
    let points = ink(&frame, split..448, orange).expect("the points are orange");
    eprintln!("name ink {name:?}, points ink {points:?}, boxes {:?}", drawn.stunt);
    let near = |got: usize, want: f32, by: f32, what: &str| assert!((got as f32 - want).abs() <= by, "{what}: {got}, the original's {want}");
    near(name[0], 242.0, 3.0, "name left");
    near(name[2], 399.0, 3.0, "name right");
    near(name[1], 355.0, 2.0, "name top");
    near(name[3] - name[1] + 1, 11.0, 2.0, "name height");
    near(points[2] - points[0] + 1, 80.0, 4.0, "points width");
    near(points[1], 375.0, 2.0, "points top");
    near(points[3] - points[1] + 1, 20.0, 2.0, "points height");
    near((points[0] + points[2]) / 2, 321.5, 2.0, "points centred");

    let mut frame = Frame::new(640, 448, [0, 0, 0]);
    let drawn = art.draw_pops(&mut frame, (640, 448), 0, &Pops { stunt: None, info: Some(Info { text: "Helmet Cam".into(), alpha: 1.0 }) });
    assert_eq!(drawn.info.as_ref().map(|i| i.0.as_str()), Some("Helmet Cam"));
    let info = ink(&frame, 0..448, white).expect("the info text is white");
    eprintln!("info ink {info:?}");
    near(info[1], 399.0, 4.0, "info top");
    near((info[0] + info[2]) / 2, 320.0, 3.0, "info centred");

    // Two players cut across, then side by side: each view its own layout's place.
    for (view, window, stunt, top) in [((640, 224), (640, 448), 182.0, 50.0), ((320, 448), (640, 448), 360.0, 224.0)] {
        for v in 0..2 {
            let mut frame = Frame::new(view.0, view.1, [0, 0, 0]);
            art.draw_pops(&mut frame, window, v, &pops("CanCan", "3740", Some("Helmet Cam")));
            let name = ink(&frame, 0..view.1, orange).expect("the Stunt movie is up");
            let info = ink(&frame, 0..view.1, white).expect("the info text is up");
            eprintln!("{view:?} view {v}: stunt {name:?} info {info:?}");
            let mid = view.0 as f32 / 2.0;
            near((name[0] + name[2]) / 2, mid, 3.0, "stunt centred");
            assert!(name[1] as f32 > stunt - 25.0 && (name[1] as f32) < stunt, "{view:?} view {v}: stunt top {}", name[1]);
            near(info[1], top, 4.0, "info top");
            near((info[0] + info[2]) / 2, mid, 3.0, "info centred");
        }
    }
}
