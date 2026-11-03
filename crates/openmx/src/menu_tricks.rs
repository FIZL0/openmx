//! The pause's Tricks page (P259): every trick the ride has now, each with the buttons and stick that
//! do it, where it is done and what it scores, and how the page's live view shows it performed.
//!
//! The list is the ride's own, made once at its start from the stunt set it rides with
//! ([`game_sim::trick::Set`]) and the moves a trick pack laid over the bike, if any
//! ([`game_sim::trick_pack::Moves`]): the original's 24 gesture slots and its three moves of no
//! slot, the backflip, the wheelie and the stoppie; then the trick packs' extras, on Cross and
//! Square (MD16, MD77); then R1's front flip, 360s and whip, which ride only with a pack's moves on
//! the bike. Nothing here is read by the simulation.

use game_sim::trick::{self, Dir, EXTRA_SLOTS, SLOTS, Set};
use game_sim::trick_pack::Moves;
use game_sim::vehicle_anim;
use ps2_float::F;

/// What a row's gesture is drawn as: a pad button, a stick direction, or the step between two
/// pushes of the stick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Circle,
    Triangle,
    R1,
    Cross,
    Square,
    Up,
    Down,
    Left,
    Right,
    /// "then": the stick one way, then the other.
    Then,
}

/// How the live view shows a trick (P259): the clip it plays, and how the whole bike turns under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Show {
    /// A stunt's own clip, by its animation state: a gesture slot `0..24`, or a pack extra's
    /// [`vehicle_anim::PACK_EXTRA`]` + k`. The bike holds still in the air under it.
    Clip(i32),
    /// The backflip's clip, the bike pitching nose up once round.
    Backflip,
    /// The front flip: the bike pitching nose down once round, the rider leant forward.
    FrontFlip,
    /// A 360, the bike yawing once round: right is clockwise seen from above.
    Spin { right: bool },
    /// The whip: the bike thrown on its side and back, the rider in the airborne pose.
    Whip,
    /// On the back wheel, the rider leaning back.
    Wheelie,
    /// On the front wheel, the rider leaning forward.
    Stoppie,
}

/// Where a trick is started.
pub const IN_THE_AIR: &str = "In the air";
pub const ON_A_LIP: &str = "Off a lip";
pub const ON_THE_GROUND: &str = "On the ground";

/// One of the page's rows.
#[derive(Clone, Debug, PartialEq)]
pub struct TrickRow {
    /// As the HUD names it when it is done.
    pub name: String,
    /// The buttons held, then the stick, in the order drawn.
    pub how: Vec<Glyph>,
    /// Where it is started: [`IN_THE_AIR`], [`ON_A_LIP`] or [`ON_THE_GROUND`].
    pub when: &'static str,
    /// What it scores, as the page writes it.
    pub points: String,
    /// Which list it is of: the original's, a trick pack's, or R1's moves.
    pub from: &'static str,
    pub show: Show,
    /// How long the live view's loop of it is, in seconds, before its rest.
    pub length: f32,
}

/// The lists the rows are of.
pub const ORIGINAL: &str = "MX Unleashed";
pub const PACKS: &str = "Trick packs";
pub const MOVES: &str = "R1 moves";

/// The four stick directions in [`trick::slot_of`]'s order within a group.
const DIRS: [Dir; 4] = [Dir::Up, Dir::Down, Dir::Left, Dir::Right];

/// How long the front flip's, the backflip's and a 360's loop is, by eye: about a jump's air.
pub const TURN_LENGTH: f32 = 1.4;
/// The whip's loop.
pub const WHIP_LENGTH: f32 = 1.2;
/// The wheelie's and the stoppie's loop.
pub const WHEEL_LENGTH: f32 = 2.0;

/// The rows for a ride on `set`, with `moves` the trick pack's laid over the bike (`None` for the
/// original's tricks).
pub fn rows(set: &Set, moves: Option<&Moves>) -> Vec<TrickRow> {
    let mut out = Vec::new();
    let named = |s: &trick::Stunt| -> Option<String> {
        let n = if s.display_name.is_empty() { &s.name } else { &s.display_name };
        (!n.is_empty()).then(|| n.clone())
    };
    for (slot, s) in set.stunts.iter().enumerate().take(SLOTS) {
        let Some(name) = named(s) else { continue };
        let group = (slot / 4 + 1) as u8;
        let mut how = buttons(group);
        how.push(dir_glyph(DIRS[slot % 4]));
        out.push(TrickRow {
            name,
            how,
            when: IN_THE_AIR,
            points: stunt_points(s),
            from: ORIGINAL,
            show: Show::Clip(slot as i32),
            length: s.duration.0.max(0.2),
        });
    }
    out.push(TrickRow {
        name: "BackFlip".into(),
        how: vec![Glyph::Up, Glyph::Then, Glyph::Down],
        when: ON_A_LIP,
        points: thousands(game_sim::score::BACKFLIP_POINTS.0),
        from: ORIGINAL,
        show: Show::Backflip,
        length: TURN_LENGTH,
    });
    let per_second = format!("{}/s", thousands(game_sim::score::WHEEL_RATE.0));
    out.push(TrickRow {
        name: "Wheelie".into(),
        how: vec![Glyph::Down],
        when: ON_THE_GROUND,
        points: per_second.clone(),
        from: ORIGINAL,
        show: Show::Wheelie,
        length: WHEEL_LENGTH,
    });
    out.push(TrickRow {
        name: "Stoppie".into(),
        how: vec![Glyph::Up],
        when: ON_THE_GROUND,
        points: per_second,
        from: ORIGINAL,
        show: Show::Stoppie,
        length: WHEEL_LENGTH,
    });
    for (k, s) in set.stunts.iter().enumerate().skip(SLOTS).take(EXTRA_SLOTS) {
        let k = k - SLOTS;
        let Some(name) = named(s) else { continue };
        let group = 7 + (k / 4) as u8;
        let mut how = buttons(group);
        how.push(dir_glyph(DIRS[k % 4]));
        out.push(TrickRow {
            name,
            how,
            when: IN_THE_AIR,
            points: stunt_points(s),
            from: PACKS,
            show: Show::Clip(vehicle_anim::PACK_EXTRA + k as i32),
            length: s.duration.0.max(0.2),
        });
    }
    if let Some(m) = moves {
        out.push(TrickRow {
            name: "Frontflip".into(),
            how: vec![Glyph::R1, Glyph::Down, Glyph::Then, Glyph::Up],
            when: ON_A_LIP,
            points: thousands(m.front_flip_points.0),
            from: MOVES,
            show: Show::FrontFlip,
            length: TURN_LENGTH,
        });
        for (name, right, points) in [("360 Left", false, m.three_sixty_points.0), ("360 Right", true, m.three_sixty_points.1)] {
            // The 360 arms on the stick one way and fires on the other: right is left, then right.
            let (a, b) = if right { (Glyph::Left, Glyph::Right) } else { (Glyph::Right, Glyph::Left) };
            out.push(TrickRow {
                name: name.into(),
                how: vec![Glyph::R1, a, Glyph::Then, b],
                when: ON_A_LIP,
                points: thousands(points.0),
                from: MOVES,
                show: Show::Spin { right },
                length: TURN_LENGTH,
            });
        }
        out.push(TrickRow {
            name: "Whip".into(),
            how: vec![Glyph::Left, Glyph::Then, Glyph::Right],
            when: IN_THE_AIR,
            points: "—".into(),
            from: MOVES,
            show: Show::Whip,
            length: WHIP_LENGTH,
        });
    }
    out
}

/// The buttons a gesture group is held on, in the order [`trick::extra_group_name`] names them:
/// Cross and Square first, then Circle, Triangle and R1.
pub fn buttons(group: u8) -> Vec<Glyph> {
    let (extra, under) = match group {
        1..=6 => (None, group),
        7..=9 => (Some(group - 7), 0),
        g if g <= trick::LAST_GROUP => (Some((g - 10) / 7), (g - 10) % 7 + 1),
        _ => return Vec::new(),
    };
    let mut out = match extra {
        Some(0) => vec![Glyph::Cross],
        Some(1) => vec![Glyph::Square],
        Some(_) => vec![Glyph::Cross, Glyph::Square],
        None => Vec::new(),
    };
    let held: [bool; 3] = match under {
        1 => [true, false, false],
        2 => [false, true, false],
        3 => [true, true, false],
        4 => [true, false, true],
        5 => [false, true, true],
        6 => [true, true, true],
        7 => [false, false, true],
        _ => [false; 3],
    };
    out.extend([Glyph::Circle, Glyph::Triangle, Glyph::R1].into_iter().zip(held).filter(|(_, h)| *h).map(|(g, _)| g));
    out
}

fn dir_glyph(d: Dir) -> Glyph {
    match d {
        Dir::Up => Glyph::Up,
        Dir::Down => Glyph::Down,
        Dir::Left => Glyph::Left,
        _ => Glyph::Right,
    }
}

/// What a stunt of the table scores done once through, before the combo's multiplier: it grows at
/// [`game_sim::score::TRICK_RATE`] a second over its whole length, floored to ten. A `+` where the
/// buttons held past its hold point keep it growing.
pub fn stunt_points(s: &trick::Stunt) -> String {
    let grown = game_sim::score::TRICK_RATE.0 * s.duration.0;
    let n = (grown / 10.0).floor() * 10.0;
    let held = F::ZERO < s.hold_time && s.hold_time < s.duration;
    format!("{}{}", thousands(n), if held { "+" } else { "" })
}

/// A whole number of points with its thousands marked: `12,500`.
pub fn thousands(n: f32) -> String {
    let n = n.max(0.0) as u64;
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A gesture as words, for the page's line under the view and the logs: `Hold Circle + R1, push
/// Up`; `Up, then Down`.
pub fn how_words(how: &[Glyph]) -> String {
    let word = |g: &Glyph| match g {
        Glyph::Circle => "Circle",
        Glyph::Triangle => "Triangle",
        Glyph::R1 => "R1",
        Glyph::Cross => "Cross",
        Glyph::Square => "Square",
        Glyph::Up => "Up",
        Glyph::Down => "Down",
        Glyph::Left => "Left",
        Glyph::Right => "Right",
        Glyph::Then => "then",
    };
    let held: Vec<&str> = how.iter().filter(|g| is_button(**g)).map(word).collect();
    let stick: Vec<&str> = how.iter().filter(|g| !is_button(**g)).map(word).collect();
    let stick = stick.join(" ").replace(" then", ", then");
    match held.is_empty() {
        true => format!("Stick {stick}"),
        false => format!("Hold {}, stick {stick}", held.join(" + ")),
    }
}

/// Whether a glyph is a button held, not the stick.
pub fn is_button(g: Glyph) -> bool {
    matches!(g, Glyph::Circle | Glyph::Triangle | Glyph::R1 | Glyph::Cross | Glyph::Square)
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_sim::trick::{Stunt, Trick};

    fn stunt(name: &str, duration: f32, hold: f32) -> Stunt {
        Stunt { name: name.to_lowercase(), display_name: name.into(), duration: F(duration), hold_time: F(hold), ..Stunt::default() }
    }

    /// The glyphs of every gesture group name the buttons [`trick::extra_group_name`] spells, so the
    /// page draws the gesture the trick machine reads.
    #[test]
    fn group_glyphs_match_the_machines_names() {
        let word = |g: Glyph| format!("{g:?}");
        for group in 7..=trick::LAST_GROUP {
            let drawn: Vec<String> = buttons(group).into_iter().map(word).collect();
            assert_eq!(drawn.join(" + "), trick::extra_group_name(group).unwrap(), "group {group}");
        }
        assert_eq!(buttons(1), vec![Glyph::Circle]);
        assert_eq!(buttons(5), vec![Glyph::Triangle, Glyph::R1]);
        assert_eq!(buttons(6), vec![Glyph::Circle, Glyph::Triangle, Glyph::R1]);
    }

    /// The original's 24 in slot order with each slot's gesture, then its three moves; no pack rows
    /// and no R1 moves without a pack's.
    #[test]
    fn original_rows() {
        let stunts: Vec<Stunt> = (0..SLOTS).map(|i| stunt(&format!("Trick{i}"), 1.0 + i as f32 * 0.1, 0.5)).collect();
        let rows = rows(&Set::new(stunts), None);
        assert_eq!(rows.len(), SLOTS + 3);
        for (slot, r) in rows.iter().enumerate().take(SLOTS) {
            let (group, dir) = ((slot / 4 + 1) as u8, DIRS[slot % 4]);
            assert_eq!(trick::slot_of(group, dir), Some(slot));
            assert_eq!(r.show, Show::Clip(slot as i32));
            assert_eq!(*r.how.last().unwrap(), dir_glyph(dir));
        }
        // Circle Up: 2500 a second over a second.
        assert_eq!(rows[0].how, vec![Glyph::Circle, Glyph::Up]);
        assert_eq!(rows[0].points, "2,500+");
        assert_eq!(rows[23].how, vec![Glyph::Circle, Glyph::Triangle, Glyph::R1, Glyph::Right]);
        let names: Vec<&str> = rows[SLOTS..].iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["BackFlip", "Wheelie", "Stoppie"]);
        assert_eq!(rows[SLOTS].points, "10,000");
        assert!(rows.iter().all(|r| r.from == ORIGINAL));
    }

    /// A pack's extras follow on their gestures, and its moves bring the front flip, the 360s and
    /// the whip with the pack's points.
    #[test]
    fn pack_rows() {
        let mut stunts: Vec<Stunt> = (0..SLOTS).map(|i| stunt(&format!("Trick{i}"), 1.0, 0.0)).collect();
        stunts[3] = Stunt::default();
        let set = Set::new(stunts).with_extras(vec![stunt("Superman", 1.5, 0.0), stunt("Lazyboy", 0.8, 0.0)]);
        let moves = Moves {
            spin: game_sim::air::SpinTuning {
                min_time: F::ZERO,
                max_time: F::ZERO,
                min_torque_x: F::ZERO,
                max_torque_x: F::ZERO,
                min_torque_y: F::ZERO,
                max_torque_y: F::ZERO,
                rate_increase: F::ZERO,
                rate_decrease: F::ZERO,
            },
            front_flip_points: F(10000.0),
            three_sixty_points: (F(4000.0), F(4500.0)),
        };
        let rows = rows(&set, Some(&moves));
        // The empty slot is left out.
        assert_eq!(rows.len(), SLOTS - 1 + 3 + 2 + 4);
        let pack: Vec<&TrickRow> = rows.iter().filter(|r| r.from == PACKS).collect();
        assert_eq!(pack[0].how, vec![Glyph::Cross, Glyph::Up]);
        assert_eq!(pack[1].how, vec![Glyph::Cross, Glyph::Down]);
        assert_eq!(pack[0].points, "3,750");
        assert_eq!(pack[1].show, Show::Clip(vehicle_anim::PACK_EXTRA + 1));
        assert_eq!(set.stunt(Trick(Trick::EXTRA + 1)).unwrap().display_name, pack[1].name);
        let r1: Vec<(&str, &str)> = rows.iter().filter(|r| r.from == MOVES).map(|r| (r.name.as_str(), r.points.as_str())).collect();
        assert_eq!(r1, [("Frontflip", "10,000"), ("360 Left", "4,000"), ("360 Right", "4,500"), ("Whip", "—")]);
        let right = rows.iter().find(|r| r.name == "360 Right").unwrap();
        assert_eq!(how_words(&right.how), "Hold R1, stick Left, then Right");
    }

    #[test]
    fn thousands_are_marked() {
        assert_eq!(thousands(0.0), "0");
        assert_eq!(thousands(999.0), "999");
        assert_eq!(thousands(1000.0), "1,000");
        assert_eq!(thousands(1234567.0), "1,234,567");
    }
}
