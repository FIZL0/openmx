//! **MX vs. ATV's tricks, as a pack laid over the original's (MD16).** None of this is MX Unleashed's:
//! a trick pack (`mods/tricks/<id>/`, openmx-mods' `tools/import-tricks`) carries another game's stunt
//! table and its bikes' `.phy` files, and this module reads the three things the port takes from it —
//!
//! - **the extra stunts**: up to [`EXTRA_SLOTS`] rows of the pack's table that the rider's own 24 do
//!   not have, started on Cross and Square ([`crate::trick::group_with`]) — rows MX vs. ATV maps to a
//!   gesture first (`DefaultKeyMapping`), then the rest in file order;
//! - **the 360** (`[Three Sixty]`, [`crate::air::three_sixty`]) and the port's own front flip
//!   ([`crate::air::FRONT_FLIP`]), with what each scores;
//! - **the whip**: MX vs. ATV's `UnrollGain` and `UnyawGain`, the air-control auto-level, laid over the
//!   bike's — the lean itself is the original's airborne control, unchanged.
//!
//! A bike with no pack laid over it ([`Phy::tricks`] `None`) is the original, to the bit.

use crate::air::SpinTuning;
use crate::phy::{Ini, Phy};
use crate::trick::{EXTRA_SLOTS, Set, Stunt};
use ps2_float::F;

/// One row of a pack's stunt table: MX Unleashed's six keys and MX vs. ATV's four more.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub stunt: Stunt,
    /// The gesture MX vs. ATV maps it to by default (`"1CR"`), empty for none.
    pub mapping: String,
    pub has_animation: bool,
    pub point_value: F,
    pub hold_point_value: F,
}

/// What a pack gives the air code and the points: the 360's numbers and the two flat scores.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moves {
    pub spin: SpinTuning,
    /// How the 360 is asked for and turns (MD25): the port's easy one unless the shell says
    /// MX vs. ATV's.
    pub spin_style: crate::air::SpinStyle,
    /// MX vs. ATV's backflip `HoldPointValue` — the flat score its backflip is worth (MX Unleashed's
    /// backflip is a flat 10000, [`crate::score::BACKFLIP_POINTS`], and so is MX vs. ATV's row) — which
    /// the front flip, scored like a backflip, is worth too.
    pub front_flip_points: F,
    /// The 360 rows' `HoldPointValue`, scored flat when the 360 comes round: one each way.
    pub three_sixty_points: (F, F),
}

/// The report's controls the two extra trick buttons ride, Cross's then Square's (MD16): R2 and L2,
/// which the original binds only to camera commands its ride never reads, and which the shell's new
/// layout leaves free when it moves the triggers onto the gas and the brake.
pub const EXTRA_CONTROLS: [crate::input::Control; crate::trick::EXTRA_BUTTONS] =
    [crate::input::Control::R2, crate::input::Control::L2];

/// The rows the extras never take: the moves the port runs as moves, and the two that are not tricks.
const NOT_EXTRAS: &[&str] = &["backflip", "wheelie", "stoppie", "threesixtyl", "threesixtyr", "taunt", "nothing"];

/// Read a pack's stunt table: every `[StuntNN]` section from 1 up, as many as the file has.
pub fn table(bytes: &[u8]) -> Result<Vec<Row>, String> {
    let ini = Ini::parse_bytes(bytes);
    let mut rows = Vec::new();
    for i in 1.. {
        let want = format!("Stunt{i:02}");
        if !ini.names().iter().any(|n| n.eq_ignore_ascii_case(&want)) {
            break;
        }
        let s = ini.section(&want);
        let text = |k: &str| s.text(k).unwrap_or("").to_owned();
        let clock = |k: &str| s.float_or(k, F::ZERO);
        let yes = |k: &str, default: bool| {
            s.text(k).map_or(default, |v| !matches!(v.to_ascii_lowercase().as_str(), "f" | "false" | "0" | "n" | "no"))
        };
        rows.push(Row {
            stunt: Stunt {
                name: text("Name"),
                display_name: text("DisplayName"),
                safe_time: clock("SafeTime"),
                hold_time: clock("HoldTime"),
                duration: clock("Duration"),
                for_bike: yes("UsedForBike", true),
                index: i - 1,
            },
            mapping: text("DefaultKeyMapping"),
            has_animation: yes("HasAnimation", true),
            point_value: clock("PointValue"),
            hold_point_value: clock("HoldPointValue"),
        });
    }
    if rows.is_empty() {
        return Err("the trick pack's table has no [Stunt01]".to_owned());
    }
    Ok(rows)
}

/// The extra stunts for a rider: the pack's rows the rider's 24 do not already have, gesture-mapped
/// ones first, at most [`EXTRA_SLOTS`]. The same pack and the same set always give the same list.
pub fn extras(rows: &[Row], set: &Set) -> Vec<Stunt> {
    let has = |name: &str| set.stunts.iter().take(crate::trick::SLOTS).any(|s| s.name.eq_ignore_ascii_case(name));
    let free = |r: &&Row| {
        !r.stunt.name.is_empty()
            && !NOT_EXTRAS.iter().any(|n| r.stunt.name.eq_ignore_ascii_case(n))
            && !has(&r.stunt.name)
            && r.stunt.duration > F::ZERO
    };
    let mapped = rows.iter().filter(free).filter(|r| !r.mapping.is_empty());
    let unmapped = rows.iter().filter(free).filter(|r| r.mapping.is_empty());
    mapped.chain(unmapped).take(EXTRA_SLOTS).map(|r| r.stunt.clone()).collect()
}

/// The row of that name, case-insensitively.
pub fn row<'a>(rows: &'a [Row], name: &str) -> Option<&'a Row> {
    rows.iter().find(|r| r.stunt.name.eq_ignore_ascii_case(name))
}

/// What one of the pack's `.phy` files and its table give the moves, or `None` when the file has no
/// `[Three Sixty]` (an MX Unleashed file, or a pack without one).
pub fn moves(phy: &Ini, rows: &[Row]) -> Option<Moves> {
    if !phy.names().iter().any(|n| n.eq_ignore_ascii_case("Three Sixty")) {
        return None;
    }
    let s = phy.section("Three Sixty");
    let f = |k: &str| s.float_or(k, F::ZERO);
    let spin = SpinTuning {
        min_time: f("MinThreeSixtyTime"),
        max_time: f("MaxThreeSixtyTime"),
        min_torque_x: f("MinThreeSixtyTorqueX"),
        max_torque_x: f("MaxThreeSixtyTorqueX"),
        min_torque_y: f("MinThreeSixtyTorqueY"),
        max_torque_y: f("MaxThreeSixtyTorqueY"),
        rate_increase: f("ThreeSixtyTorqueRateIncrease"),
        rate_decrease: f("ThreeSixtyTorqueRateDecrease"),
    };
    let points = |name: &str, default: F| row(rows, name).map_or(default, |r| r.hold_point_value);
    Some(Moves {
        spin,
        spin_style: crate::air::SpinStyle::default(),
        front_flip_points: points("backflip", crate::score::BACKFLIP_POINTS),
        three_sixty_points: (points("threesixtyL", F::ZERO), points("threesixtyR", F::ZERO)),
    })
}

/// Lay a pack's moves and its whip over a bike: the 360 and the front flip switched on, and the
/// auto-level gains replaced by the pack file's `[Airborne Control]` ones where it has them.
pub fn apply(bike: &mut Phy, pack_phy: &Ini, moves: Moves) {
    bike.tricks = Some(moves);
    let s = pack_phy.section("Airborne Control");
    if let Some(g) = s.float("UnrollGain") {
        bike.air.unroll_gain = g;
    }
    if let Some(g) = s.float("UnyawGain") {
        bike.air.unyaw_gain = g;
    }
}

/// The pack file for a bike: MX vs. ATV names its bikes `mx_` + MX Unleashed's name (`race125p.phy`
/// is `mx_Race125P.phy`), matched case-insensitively.
pub fn phy_name_for(bike_file: &str) -> String {
    let base = bike_file.rsplit(['/', '\\']).next().unwrap_or(bike_file);
    format!("mx_{base}").to_ascii_lowercase()
}

/// The per-ride state MX vs. ATV's moves keep, on [`crate::ride::Ride::tricks`] — `None` on every ride
/// on the original's tricks, which is why their hashes do not move.
#[derive(Clone, Copy, Debug, Default)]
pub struct State {
    /// The 360's own state.
    pub spin: crate::air::Spin,
    /// Which of [`crate::air::UNLEASHED_ROTATIONS`] the flip state belongs to.
    pub flip_row: usize,
    /// The sideways flick, measured as the fore-aft one is (`ride::Flick`) on the stick's `x`.
    pub side: crate::ride::Flick,
    pub side_flick: F,
    /// The front flip watch, the backflip's mirrored ([`crate::score::Score::front_flip_watch`]).
    pub front_watch: u8,
}

impl crate::hash::State for State {
    fn walk(&self, w: &mut crate::hash::Walk<'_>) {
        let s = &self.spin;
        w.bool("spin.applied", s.applied);
        w.bool("spin.ramp", s.ramp);
        w.bool("spin.active", s.active);
        w.bool("spin.armed", s.armed);
        w.f("spin.grace", s.grace);
        w.i32("spin.dir", s.dir);
        w.f("spin.start", s.start);
        w.vf("spin.torque", s.torque);
        w.u8("spin.clip", s.clip);
        w.bool("spin.came_round", s.came_round);
        w.bool("spin.wrecked", s.wrecked);
        w.bool("spin.forward", s.forward);
        // MD25's two, walked only once an easy 360 has fired, so a ride on MX vs. ATV's step hashes
        // as it did before them.
        if s.easy || s.progress != F::ZERO {
            w.bool("spin.easy", s.easy);
            w.f("spin.progress", s.progress);
        }
        w.u8("flip_row", self.flip_row as u8);
        w.f("side.now", self.side.now);
        w.u8("side.head", self.side.head as u8);
        for k in 0..crate::ride::FLICK_SAMPLES {
            w.f("side.at", self.side.at[k]);
            w.f("side.value", self.side.value[k]);
        }
        w.f("side_flick", self.side_flick);
        w.u8("front_watch", self.front_watch);
    }
}
