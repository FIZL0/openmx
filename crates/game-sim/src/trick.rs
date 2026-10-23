//! Tricks: the table of them, the stick-and-button gesture that starts one, the clocks that decide
//! whether it counts, and the part of the landing test a trick is responsible for.
//!
//! Freeroam is tricks, so all of this is ported rather than approximated. Four pieces, in the order the
//! original runs them:
//!
//! 1. **The table.** 57 stunts, read from `rider\anim\stunts.ini` inside the big container — a name, a
//!    display name and three clocks each (see [`Stunt`]). A player keeps a 24-entry map from gesture
//!    slot to stunt ([`Stunts::map`]), identity by default, and a rider is handed its own copy of the
//!    24 records that map selects ([`Set`]). Everything after this indexes *that* copy, so a remapped
//!    player and a default one run the same code.
//! 2. **The gesture.** Three trick buttons make a group of 1..=6, the left stick (or the d-pad, as a
//!    fallback) makes a direction of 1..=4, and the two together name one of the 24 slots
//!    ([`Gesture::read`], [`Tricks::request`]). The buttons must be released between tricks; a gesture
//!    made while one is already running is queued instead.
//! 3. **The clocks.** `HoldTime`, `SafeTime` and `Duration`, run by [`Tricks::update`]: how long the
//!    rider holds the pose, when the trick becomes safe to land, and when it is over.
//! 4. **The landing.** [`Tricks::landing`] is the trick's own clause of the wreck test: touching down
//!    mid-trick before it is safe, a no-hander landed too slowly, or a throw-away taken to the end.
//!
//! What is *not* here, because it is not a trick: none of the 24 stunts applies any rotation. Each one
//! is a pose the rider animation plays, and nothing in the original reads the running trick to torque
//! the bike. The three rotating moves are separate trick ids — [`Trick::BACKFLIP`], [`Trick::WHEELIE`]
//! and [`Trick::STOPPIE`] — driven by the airborne control's own torque parameters, which belong to the
//! bike-in-the-air task. The animation blending the original does in the same function, and the
//! attitude limits in front of the landing clause, likewise belong elsewhere.
//!
//! `research/journal/2026-10-15-s11-s11-tricks/` has the addresses, the key layout, the measurements
//! and the proofs.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::hash;
use crate::rng::Rng;
use ps2_float::{F, fpu, libm};
use std::collections::VecDeque;

/// How many stunts the table holds. The loader reads exactly this many sections, numbered from 1.
pub const STUNTS: usize = 57;
/// How many gesture slots a player has: six button groups by four stick directions.
pub const SLOTS: usize = 24;
/// How many players the table keeps a map for.
pub const PLAYERS: usize = 2;
/// The loader's name buffers. 32 bytes with the terminator, so 31 usable characters.
const NAME: usize = 0x20;

// ---------------------------------------------------------------------------------------------------
// the table
// ---------------------------------------------------------------------------------------------------

/// One stunt, as the table holds it.
///
/// The three clocks are seconds and all of them are read against the same elapsed time, so they are
/// listed here in the order they fire rather than the order the file prints them:
///
/// - `hold_time` — once the trick is this old, the rider is holding it: the hand-hold pose goes on, and
///   while the trick buttons stay down the clock pauses (and wobbles) instead of running on.
/// - `safe_time` — from here the trick can be landed. Before it, touching down is a wreck.
/// - `duration` — the whole trick. Progress is elapsed over this, and past it the trick is finished.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stunt {
    /// The animation name, lower-case in the file (`"nacnac"`); what the two special stunts are found
    /// by, case-insensitively.
    pub name: String,
    /// What the HUD shows (`"NacNac"`). Empty for the stunts the file leaves unnamed.
    pub display_name: String,
    /// When the trick becomes safe to land.
    pub safe_time: F,
    /// When the rider is holding the pose.
    pub hold_time: F,
    /// How long the whole trick takes.
    pub duration: F,
    /// False for the stunts that are rider-only — the loader defaults it to true when the key is
    /// missing, and the animation loader skips the ones that are false.
    pub for_bike: bool,
    /// Which section this came from, zero-based. Copied along with the record, so a rider's own copy
    /// still says which stunt of the 57 each of its 24 slots is.
    pub index: i32,
}

/// The whole table plus each player's gesture map.
#[derive(Clone, Debug)]
pub struct Stunts {
    /// The 57 stunts, in file order.
    pub stunts: Vec<Stunt>,
    map: [[i32; SLOTS]; PLAYERS],
}

impl Default for Stunts {
    fn default() -> Stunts {
        Stunts { stunts: Vec::new(), map: [Stunts::IDENTITY; PLAYERS] }
    }
}

impl Stunts {
    /// The map the loader writes: slot *n* is stunt *n*, which is why the file's first 24 sections are
    /// in gesture order.
    pub const IDENTITY: [i32; SLOTS] = {
        let mut m = [0; SLOTS];
        let mut i = 0;
        while i < SLOTS {
            m[i] = i as i32;
            i += 1;
        }
        m
    };

    /// Read `stunts.ini`.
    ///
    /// The original asks its INI reader for one section per stunt and for six keys per section, and
    /// takes a default when a key is absent: an empty string, zero for a clock, and *true* for
    /// `UsedForBike`. Sections and keys it does not ask for are ignored, so a file with extra ones
    /// reads the same.
    pub fn parse(bytes: &[u8]) -> Result<Stunts, String> {
        let text = core::str::from_utf8(bytes).map_err(|e| format!("stunts.ini is not text: {e}"))?;
        let mut sections: Vec<(String, Vec<(String, String)>)> = Vec::new();
        for raw in text.lines() {
            let line = raw.trim();
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                sections.push((name.trim().to_ascii_lowercase(), Vec::new()));
                continue;
            }
            let Some((key, value)) = line.split_once('=') else { continue };
            if let Some((_, keys)) = sections.last_mut() {
                keys.push((key.trim().to_ascii_lowercase(), value.trim().to_owned()));
            }
        }

        let mut stunts = Vec::with_capacity(STUNTS);
        for i in 0..STUNTS {
            // The loader builds the section name with two formats: one zero-padded for the first nine.
            let want = if i < 9 { format!("stunt0{}", i + 1) } else { format!("stunt{}", i + 1) };
            let keys = sections
                .iter()
                .find(|(name, _)| *name == want)
                .map(|(_, keys)| keys.as_slice())
                .ok_or_else(|| format!("stunts.ini has no [{want}]"))?;
            let text = |k: &str| {
                keys.iter()
                    .find(|(key, _)| key == k)
                    .map(|(_, v)| truncate(unquote(v)))
                    .unwrap_or_default()
            };
            let clock = |k: &str| -> Result<F, String> {
                match keys.iter().find(|(key, _)| key == k) {
                    Some((_, v)) => single(v),
                    None => Ok(F::ZERO),
                }
            };
            stunts.push(Stunt {
                name: text("name"),
                display_name: text("displayname"),
                safe_time: clock("safetime")?,
                hold_time: clock("holdtime")?,
                duration: clock("duration")?,
                for_bike: match keys.iter().find(|(key, _)| key == "usedforbike") {
                    Some((_, v)) => !matches!(v.trim().to_ascii_lowercase().as_str(), "f" | "false" | "0" | "n" | "no"),
                    None => true,
                },
                index: i as i32,
            });
        }
        Ok(Stunts { stunts, map: [Stunts::IDENTITY; PLAYERS] })
    }

    /// One stunt by table index.
    pub fn get(&self, index: usize) -> Option<&Stunt> {
        self.stunts.get(index)
    }

    /// A player's gesture map: `map[slot]` is the table index that slot starts.
    pub fn map(&self, player: usize) -> &[i32; SLOTS] {
        &self.map[player]
    }

    /// Remap a player's gestures. The front end writes all 24 at once, as the original does.
    pub fn set_map(&mut self, player: usize, map: [i32; SLOTS]) {
        self.map[player] = map;
    }

    /// The 24 records a rider is handed: one copy per gesture slot, through the player's map.
    ///
    /// The original allocates this array and `memcpy`s the records into it when the rider is built, so
    /// a later remap does not reach a rider already on the track. A slot whose map entry is out of
    /// range gets a default record, which is what reading past the table would have produced from a
    /// freshly allocated table.
    pub fn selection(&self, player: usize) -> Vec<Stunt> {
        self.map[player]
            .iter()
            .map(|&i| usize::try_from(i).ok().and_then(|i| self.get(i)).cloned().unwrap_or_default())
            .collect()
    }

    /// What the HUD calls a trick id, for a given player.
    ///
    /// The three ids that are not table slots have their own names here, spelled as the image spells
    /// them; everything else is the mapped stunt's display name.
    pub fn display_name(&self, player: usize, trick: Trick) -> &str {
        match trick {
            Trick::STOPPIE => "Stoppie",
            Trick::WHEELIE => "Wheelie",
            Trick::BACKFLIP => "BackFlip",
            Trick::FRONTFLIP => "Frontflip",
            Trick::THREE_SIXTY_LEFT => "Three Sixty Left",
            Trick::THREE_SIXTY_RIGHT => "Three Sixty Right",
            _ => match trick.slot() {
                Some(slot) => match usize::try_from(self.map[player][slot]).ok().and_then(|i| self.get(i)) {
                    Some(stunt) => &stunt.display_name,
                    None => "",
                },
                None => "",
            },
        }
    }
}

/// `strncpy` into a 32-byte buffer: at most 31 characters, and nothing is appended.
/// Drop the quotes the file wraps its display names in, the way the original's INI reader does: a
/// leading quote and a trailing quote, each independently of the other, before the length cap.
fn unquote(value: &str) -> &str {
    let inner = value.strip_prefix('"').unwrap_or(value);
    inner.strip_suffix('"').unwrap_or(inner)
}

fn truncate(value: &str) -> String {
    let mut end = NAME - 1;
    while end < value.len() && !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end.min(value.len())].to_owned()
}

fn single(value: &str) -> Result<F, String> {
    // The loader reads its clocks through the INI reader's float-with-default getter, which is the
    // C library's `strtod` and then the EE's double-to-float routine — the same two steps as a `.phy`
    // value, and the second one truncates. A round-to-nearest parse puts `1.33` one bit above the
    // original's, which the human's recording shows as a trick that ends one bit late (S30). Text
    // that is not a number stays an error here, where the original would read a zero.
    value.trim().parse::<f32>().map_err(|e| format!("{value:?} is not a number: {e}"))?;
    Ok(crate::phy::number(value))
}

/// The 24 stunts one rider can do, and the two of them the simulation treats specially.
///
/// The original searches the rider's own copy of the table for two names and remembers where they
/// landed; when a name is absent the index it stores is 24, which is no slot at all, so the special
/// case can never fire. Both searches are case-insensitive.
#[derive(Clone, Debug, Default)]
pub struct Set {
    /// The 24 records, one per gesture slot.
    pub stunts: Vec<Stunt>,
    /// Where the no-hander is. Landing one is only allowed above a speed, and it is the one trick that
    /// becomes safe at `hold_time` rather than at `safe_time`.
    pub no_hander: usize,
    /// Where the throw-away is: taken past 95% of its duration, it throws the rider off deliberately.
    pub throwaway: usize,
}

impl Set {
    /// The sentinel the original stores when a name is not in the rider's 24: one past the last slot.
    pub const ABSENT: usize = SLOTS;

    /// Build a rider's set from the 24 records it was handed.
    pub fn new(stunts: Vec<Stunt>) -> Set {
        let find = |want: &str| {
            stunts
                .iter()
                .position(|s| s.name.eq_ignore_ascii_case(want))
                .unwrap_or(Set::ABSENT)
        };
        Set { no_hander: find("nohandedlander"), throwaway: find("throwaway"), stunts }
    }

    /// The record a trick id runs, or `None` for the ids that are not table slots.
    pub fn stunt(&self, trick: Trick) -> Option<&Stunt> {
        self.stunts.get(trick.slot().or_else(|| trick.extra().map(|k| SLOTS + k))?)
    }

    /// The 24 with the extra slots' records after them (MD16), in [`extra_of`]'s order. A set with
    /// none is the original's.
    pub fn with_extras(mut self, extras: Vec<Stunt>) -> Set {
        self.stunts.truncate(SLOTS);
        self.stunts.resize_with(SLOTS, Stunt::default);
        self.stunts.extend(extras.into_iter().take(EXTRA_SLOTS));
        self
    }

    /// Whether this rider reads Cross and Square as trick buttons (MD16).
    pub fn has_extras(&self) -> bool {
        self.stunts.len() > SLOTS
    }

    /// The AI's trick choice for an air time it expects to have.
    ///
    /// It collects every slot whose whole duration fits in the air time and is not the throw-away, then
    /// picks one at random. Two quirks are the original's and are kept: the name test is a `strcmp`
    /// against `"throwaway"` rather than the slot the rider already found, and the pick is over
    /// *count − 1*, so the last trick collected is only ever chosen when the random float is exactly
    /// zero after scaling — in practice never.
    pub fn ai_choice(&self, airtime: F, rng: &mut Rng) -> Option<usize> {
        let mut fit: Vec<usize> = Vec::with_capacity(SLOTS);
        for (slot, stunt) in self.stunts.iter().enumerate().take(SLOTS) {
            if stunt.duration <= airtime && stunt.name != "throwaway" {
                fit.push(slot);
            }
        }
        let count = i32::try_from(fit.len()).unwrap_or(i32::MAX);
        if count <= 0 {
            return None;
        }
        let pick = rng.index(count - 1);
        fit.get(usize::try_from(pick).unwrap_or(0)).copied()
    }
}

// ---------------------------------------------------------------------------------------------------
// trick ids
// ---------------------------------------------------------------------------------------------------

/// What the rider is doing: one of the 24 gesture slots, or one of the moves that is not a stunt.
///
/// The ids are the original's, because they are stored in the state packet a networked game sends and
/// read back by index.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Trick(pub u8);

impl Trick {
    /// A backflip: the only trick that turns the bike over, and it is not in the table.
    pub const BACKFLIP: Trick = Trick(24);
    /// A wheelie. The rider update blends the three airborne poses for this one instead of playing a
    /// stunt animation.
    pub const WHEELIE: Trick = Trick(25);
    /// A stoppie.
    pub const STOPPIE: Trick = Trick(26);
    /// Nothing: what the rider is set to on the ground, and what it starts on.
    pub const NONE: Trick = Trick(28);
    /// **The port's own (MD16)**, past every id of the original's: the front flip, and MX vs. ATV's
    /// two 360s. Only a ride on MX vs. ATV's tricks ever scores one.
    pub const FRONTFLIP: Trick = Trick(29);
    pub const THREE_SIXTY_LEFT: Trick = Trick(30);
    pub const THREE_SIXTY_RIGHT: Trick = Trick(31);
    /// The first of the [`EXTRA_SLOTS`] ids Cross and Square name (MD16).
    pub const EXTRA: u8 = 32;

    /// Which extra slot this is, if it is one (MD16). Never a gesture slot: [`Trick::slot`] stays
    /// the original's 24, so the clip tables keyed on it are not reached past their end.
    pub fn extra(self) -> Option<usize> {
        let k = usize::from(self.0.checked_sub(Trick::EXTRA)?);
        (k < EXTRA_SLOTS).then_some(k)
    }

    /// Which gesture slot this is, if it is one.
    pub fn slot(self) -> Option<usize> {
        let slot = self.0 as usize;
        (slot < SLOTS).then_some(slot)
    }
}

/// The gesture slot a (group, direction) pair names.
///
/// Groups run 1..=6 and directions 1..=4, in the order the original tests them, so the slots come out
/// in the order the file lists its first 24 stunts: circle up, circle down, circle left, circle right,
/// triangle up, and so on.
pub fn slot_of(group: u8, dir: Dir) -> Option<usize> {
    if !(1..=6).contains(&group) || dir == Dir::None {
        return None;
    }
    Some((group as usize - 1) * 4 + (dir as usize - 1))
}

// ---------------------------------------------------------------------------------------------------
// the stick
// ---------------------------------------------------------------------------------------------------

/// The engine's arc cosine: a 30,001-entry table over `x ∈ [-1, 1]`, read with a linear interpolation.
///
/// It is built once at start-up and shared by everything in the game that needs an angle — tricks are
/// only one caller, so when the next subsystem needs it this is the thing to lift out of here. The
/// table is what makes the trick direction reproducible: a port that called a correctly-rounded
/// `acos` would disagree with the original by far more than a last bit, because two neighbouring
/// entries 6.7e-5 apart are blended with a plain `lerp`.
///
/// The grid is `x = i·step − 1` for `i` in `0..30000`, clamped at 1, and entry 30,000 repeats entry
/// 29,999 so that a lookup landing exactly on the last entry can still interpolate towards something.
#[derive(Clone, Debug)]
pub struct AngleTable {
    values: Box<[F]>,
}

impl AngleTable {
    /// Entries, including the repeated last one.
    pub const LEN: usize = 30_001;
    /// Entries the builder computes; the one after them is a copy of the last.
    pub const GRID: usize = 30_000;
    /// What a value in `[-1, 1]` is multiplied by to become an index, after `+1`: `(30000 − 1) / 2`.
    pub const SCALE: f32 = 14999.5;
    /// The step between grid points: `2 / 29999`, as the image's literal holds it.
    pub const STEP: f32 = 6.666_888_657_491_654e-5;

    /// Build the table the way the original does.
    ///
    /// Each entry is a *double* arc cosine chopped to a single, which is the one place in the
    /// simulation where double precision is the right answer: the original calls its C library's
    /// `acos`, which the EE runs in software because it has no hardware doubles, and then converts
    /// the result with the EE's chopping float conversion. The single-precision `acosf` is a
    /// different function and disagrees with the table on 11,432 of its 30,000 entries, which is why
    /// [`ps2_float::libm::acos`] exists. `tests/trick.rs` compares every entry against the original's
    /// own chain.
    pub fn build() -> AngleTable {
        let mut values = Vec::with_capacity(AngleTable::LEN);
        for i in 0..AngleTable::GRID {
            let mut x = F(-1.0) + F(fpu::from_i32(i as i32)) * F(AngleTable::STEP);
            if !(x <= F::ONE) {
                x = F::ONE;
            }
            values.push(F(fpu::chop(libm::acos(f64::from(x.to_f32())))));
        }
        let last = values[AngleTable::GRID - 1];
        values.push(last);
        AngleTable { values: values.into_boxed_slice() }
    }

    /// A table measured somewhere else — the original's own, in a test.
    pub fn from_values(values: Vec<F>) -> Result<AngleTable, String> {
        if values.len() != AngleTable::LEN {
            return Err(format!("the angle table has {} entries, not {}", values.len(), AngleTable::LEN));
        }
        Ok(AngleTable { values: values.into_boxed_slice() })
    }

    /// The entries, for a test that compares them.
    pub fn values(&self) -> &[F] {
        &self.values
    }

    /// The arc cosine of an `x` already in `[-1, 1]`: index, then blend the two entries around it.
    ///
    /// Named for what it returns rather than for the function it replaces, because the simulation's
    /// sources are not allowed to contain the host's spelling of it — see `tests/platforms.rs`.
    ///
    /// The index cannot leave the table for any finite input, because the FPU this came from saturates
    /// instead of producing infinities and NaNs: the worst case is `x = ±1`, which lands on entry
    /// 29,999 and blends with the copy above it.
    pub fn angle(&self, x: F) -> F {
        let scaled = (x + F::ONE) * F(AngleTable::SCALE);
        let i = scaled.to_i32();
        let frac = scaled - F(fpu::from_i32(i));
        let at = (i.max(0) as usize).min(AngleTable::GRID - 1);
        let (lo, hi) = (self.values[at], self.values[at + 1]);
        // mula.s with the complement, madd.s with the fraction — the original's operand order.
        fpu::Acc::mula(F::ONE - frac, lo).madd(frac, hi)
    }
}

/// Which way the stick is pointing, as the trick decoder counts it. The numbers are the original's.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum Dir {
    /// Not far enough from centre to mean anything.
    #[default]
    None = 0,
    Up = 1,
    Down = 2,
    Left = 3,
    Right = 4,
}

/// Where the trick decoder thinks the stick is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Look {
    /// The angle from "right", in radians, positive clockwise on screen (so down is positive). The
    /// original keeps this on the rider for other subsystems to read, and so does this.
    pub angle: F,
    /// How far from centre the stick is: the length of the deflection, not its square.
    pub reach: F,
    /// The direction that angle quantises to.
    pub dir: Dir,
}

/// The deflection the trick decoder reads, after the d-pad has had its say.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stick {
    pub x: F,
    pub y: F,
    /// Whether each axis came from the d-pad rather than the stick. The original records this for the
    /// steering code; nothing in here reads it.
    pub digital: [bool; 2],
}

/// The d-pad, in the order the original polls it: left, right, up, down.
pub const DPAD: usize = 4;

/// The stick as the trick decoder uses it: the analogue deflection, overridden axis by axis by the
/// d-pad, left and up winning over right and down.
pub fn stick(x: F, y: F, dpad: [bool; DPAD]) -> Stick {
    let mut out = Stick { x, y, digital: [false, false] };
    if dpad[0] {
        out.x = F(-1.0);
        out.digital[0] = true;
    } else if dpad[1] {
        out.x = F::ONE;
        out.digital[0] = true;
    }
    if dpad[2] {
        out.y = F(-1.0);
        out.digital[1] = true;
    } else if dpad[3] {
        out.y = F::ONE;
        out.digital[1] = true;
    }
    out
}

/// An eighth of a turn, and three of them: the two thresholds that cut the circle into four. The
/// image holds them as these singles, not as a `π` divided by anything.
const EIGHTH: F = F(0.7853982);
const THREE_EIGHTHS: F = F(2.3561945);

/// Classify a stick deflection into one of the four trick directions.
///
/// The angle is the table's `acos` of the normalised x, negated when y is above centre, which makes
/// "up" the negative half. Then: within an eighth of a turn of straight right is `Right`, the next
/// quarter clockwise is `Down`, anything past three eighths either way is `Left`, and the rest —
/// counter-clockwise of straight right — is `Up`. Right wins the exact boundaries because the original
/// tests `<` and `<=` the way it does; `Up` wins nothing, since it is the one that falls through.
///
/// Nothing is a direction at all until the deflection is past half of full — the test is against the
/// *square* of the length, so it is `0.25`.
pub fn look(table: &AngleTable, s: Stick) -> Look {
    // mula.s on y, madd.s on x: the square of the length, in the original's order.
    let square = fpu::Acc::mula(s.y, s.y).madd(s.x, s.x);
    // rsqrt.s against one, then one multiply: x / |(x, y)|.
    let nx = s.x * F::ONE.rsqrt(square);
    // The original clamps twice over, for nothing; kept because it is two instruction pairs either way.
    let clamped = clamp_unit(clamp_unit(nx));
    let mut angle = table.angle(clamped);
    if s.y < F::ZERO {
        angle = -angle;
    }
    let dir = if !(F(0.25) < square) {
        Dir::None
    } else if angle < -EIGHTH {
        if angle <= -THREE_EIGHTHS { Dir::Left } else { Dir::Up }
    } else if !(EIGHTH < angle) {
        Dir::Right
    } else if !(THREE_EIGHTHS < angle) {
        Dir::Down
    } else {
        Dir::Left
    };
    Look { angle, reach: square.sqrt(), dir }
}

/// `if (x <= 1.0) { if (x < -1.0) { -1.0 } else { x } } else { 1.0 }` — the original's own two
/// comparisons, which is not quite `clamp`: a NaN would come out as `1.0`.
fn clamp_unit(x: F) -> F {
    if !(x <= F::ONE) {
        F::ONE
    } else if x < F(-1.0) {
        F(-1.0)
    } else {
        x
    }
}

// ---------------------------------------------------------------------------------------------------
// the gesture
// ---------------------------------------------------------------------------------------------------

/// How many trick buttons there are. The original polls three controls and names them by index; which
/// pad buttons those are is the input layer's business, and on the shipped pad they are circle,
/// triangle and the right shoulder, in this order.
pub const TRICK_BUTTONS: usize = 3;

/// The buttons the trick decoder reads. Any non-zero strength counts as held, including an analogue
/// button barely pressed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Buttons {
    pub trick: [bool; TRICK_BUTTONS],
    pub dpad: [bool; DPAD],
    /// **The port's own (MD16):** Cross and Square, the two buttons the new control scheme freed,
    /// which a rider on MX vs. ATV's tricks reads as two more groups ([`group_with`]). Both false for
    /// every rider on the original's tricks, which is every replay and every acceptance run.
    pub extra: [bool; EXTRA_BUTTONS],
}

/// How many buttons past the original's three a rider on MX vs. ATV's tricks reads (MD16): Cross
/// and Square, in that order.
pub const EXTRA_BUTTONS: usize = 2;
/// The gesture slots those two make: one group each, by the four stick directions.
pub const EXTRA_SLOTS: usize = EXTRA_BUTTONS * 4;

/// [`group`] with the port's two extra buttons (MD16): the original's group when any of its three
/// buttons is down, so an original gesture always wins; otherwise 7 for Cross and 8 for Square
/// (Cross first when both are down), and 0 for nothing. With both extras up it is [`group`].
pub fn group_with(trick: [bool; TRICK_BUTTONS], extra: [bool; EXTRA_BUTTONS]) -> u8 {
    match group(trick) {
        0 if extra[0] => 7,
        0 if extra[1] => 8,
        g => g,
    }
}

/// The extra slot (0..[`EXTRA_SLOTS`]) a group of 7 or 8 and a direction name, in the order
/// [`slot_of`] numbers the original's: up, down, left, right within a group.
pub fn extra_of(group: u8, dir: Dir) -> Option<usize> {
    if !(7..=8).contains(&group) || dir == Dir::None {
        return None;
    }
    Some((group as usize - 7) * 4 + (dir as usize - 1))
}

/// The trick id a gesture names: one of the 24 slots, or one of the extra ones (MD16).
pub fn trick_of(group: u8, dir: Dir) -> Option<Trick> {
    slot_of(group, dir).map(|s| Trick(s as u8)).or_else(|| extra_of(group, dir).map(|k| Trick(Trick::EXTRA + k as u8)))
}

/// Which of the six button groups is held, or zero for none.
///
/// The combinations are tested in this order and the first match wins, so holding all three is group 6
/// rather than any of the pairs it contains. The numbering is what makes the groups come out in the
/// order the table lists them: one button each, then the three pairs, then all three.
pub fn group(trick: [bool; TRICK_BUTTONS]) -> u8 {
    let [b1, b2, b3] = trick;
    if b1 && b2 && b3 {
        6
    } else if b2 && b3 {
        5
    } else if b1 && b3 {
        4
    } else if b1 && b2 {
        3
    } else if b2 {
        2
    } else if b1 {
        1
    } else {
        0
    }
}

/// What the rider makes of the controller this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gesture {
    /// The button group, 0..=6.
    pub group: u8,
    /// The direction, from the stick or the d-pad.
    pub dir: Dir,
    /// The stick, after the d-pad override.
    pub stick: Stick,
    /// The angle and reach the classifier worked out, which the original keeps on the rider.
    pub look: Look,
    /// Set when no trick button is held: a new trick cannot start until the buttons have been let go,
    /// which is why holding a combination does not fire the same trick over and over.
    pub armed: bool,
}

impl Gesture {
    /// Read the controller.
    ///
    /// `wrecked` is the rider's own "I am not driving" state — while it is set the original zeroes the
    /// stick, the group and the armed flag, so a trick cannot be queued up during a crash and fire on
    /// the way out of it. It leaves the direction, the look and which axes the d-pad drove as the last
    /// driven frame had them: a crash with the stick held left still reads "left" until the rider is
    /// back on (recording2, S30).
    pub fn read(&mut self, table: &AngleTable, deflection: (F, F), buttons: Buttons, wrecked: bool) {
        if wrecked {
            let stick = Stick { digital: self.stick.digital, ..Stick::default() };
            *self = Gesture { armed: false, dir: self.dir, look: self.look, stick, ..Gesture::default() };
            return;
        }
        self.stick = stick(deflection.0, deflection.1, buttons.dpad);
        self.group = group_with(buttons.trick, buttons.extra);
        if self.group == 0 {
            // The release that arms the next trick.
            self.armed = true;
        }
        self.look = look(table, self.stick);
        self.dir = self.look.dir;
        // With a group held but the stick centred, the d-pad names the direction on its own.
        if self.group != 0 && self.dir == Dir::None {
            self.dir = if buttons.dpad[0] {
                Dir::Left
            } else if buttons.dpad[1] {
                Dir::Right
            } else if buttons.dpad[2] {
                Dir::Up
            } else if buttons.dpad[3] {
                Dir::Down
            } else {
                Dir::None
            };
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// the trick itself
// ---------------------------------------------------------------------------------------------------

/// Why the rider came off. The numbers are the original's reason codes, which the animation and the
/// camera both switch on, so they are part of the behaviour and not an invention of this port.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wreck(pub u8);

impl Wreck {
    /// The trick's own wreck: landed before the trick was safe, or landed a no-hander too slowly.
    pub const TRICK: Wreck = Wreck(12);
    /// Thrown off on purpose, at the end of a throw-away.
    pub const THROWN: Wreck = Wreck(7);
    /// Landed at an angle the bike cannot take. Not decided here — it is the clause in front of the
    /// trick's, and it belongs to the bike-in-the-air task; named so the codes read as a set.
    pub const ATTITUDE: Wreck = Wreck(11);
}

/// What the trick machine needs to know about the rest of the bike this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bike {
    /// A wheel is on the ground.
    pub on_ground: bool,
    /// Something other than a wheel is touching: the collision list is not empty.
    pub contact: bool,
    /// The rider is wrecking: off the bike, or being put back on it.
    pub wrecked: bool,
    /// The rider has not touched down since it was put on the track. A respawn sets this and the first
    /// frame with a wheel on the ground clears it, which is what stops a gesture from counting while
    /// the bike is still dropping in.
    pub fresh: bool,
    /// Speed, for the one landing test that cares about it.
    pub speed: F,
}

/// The two numbers from the bike's own parameter file that the trick machine reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Params {
    /// `StuntOverlapTime`: how early into the end of a trick a queued one may take over. It is capped
    /// at half the trick's duration, so a short trick cannot be cut in half twice over.
    pub overlap: F,
    /// `NoHanderWreckSpeedLimit`: below this, landing a no-hander is a wreck.
    ///
    /// **This is a mis-reading, found in P3 and kept only because `landing` is no longer called.**
    /// The word the clause compares against is the bike's up axis, not its speed, and the parameter
    /// it compares it to is `NoHanderMaxRollAngle` already turned into a cosine by the loader. See
    /// `air::Params::no_hander_max_roll_angle`, which is the same clause read correctly, and the P3
    /// journal entry for the evidence.
    pub no_hander_speed: F,
}

/// Everything about the trick the rider is doing: what the controller said, what was asked for, what
/// is running, and how far through it is.
///
/// The original keeps all of this on the vehicle, next to the suspension and the engine; it is one
/// value here so that a snapshot is a copy. Nothing in it is wired to the clock or to the pad — the
/// caller reads the pad into [`Gesture::read`] and hands the frame time to [`Tricks::update`].
#[derive(Clone, Debug, PartialEq)]
pub struct Tricks {
    /// What the controller says this frame.
    pub gesture: Gesture,
    /// Tricks asked for while one was running, oldest first. The original's is a heap list with no
    /// limit, so this one has none either.
    pub queue: VecDeque<Trick>,
    /// What has been asked for. The rider latches this into its state once a frame, which is how a
    /// networked rider's trick arrives, and then runs the latched copy.
    pub requested: Trick,
    /// What is running.
    pub current: Trick,
    /// Set the frame a trick is asked for, and cleared by the update that starts it.
    pub restart: bool,
    /// How many tricks have been asked for in this flight. It is how a repeat of the *same* trick is
    /// told from the same trick still running: both ids are equal, but the count moved.
    pub seq: i32,
    /// `seq` as it was at the end of the previous frame.
    pub prev_seq: i32,
    /// How long the running trick has been going, in seconds. Not simply a sum of frame times: while
    /// the trick buttons are held it eases towards the hold and then stops there.
    pub elapsed: F,
    /// `elapsed / duration`, which is what the animation is played at. During the hold it wobbles
    /// around the held value instead of advancing.
    pub progress: F,
    /// How long the hold has been going.
    pub hold_elapsed: F,
    /// The clock the hold's wobble is sampled from.
    pub wiggle_time: F,
    /// Total time spent in the hold or easing into it. Once this is non-zero the trick has been held,
    /// and the hold cannot be entered a second time.
    pub held: F,
    /// Set once the trick has been held and released, so it runs to the end from there.
    pub hold_done: bool,
    /// A trick is running: it has not reached the end of its duration.
    pub in_progress: bool,
    /// The trick has passed its safe time and can be landed.
    pub safe: bool,
    /// The rider has both hands off: the no-hander, held, on the ground. The landing test treats this
    /// as its own case.
    pub no_hander: bool,
    /// The hand-hold pose is on, which the rider animation reads.
    pub hand_hold: bool,
}

impl Default for Tricks {
    fn default() -> Tricks {
        Tricks {
            gesture: Gesture::default(),
            queue: VecDeque::new(),
            requested: Trick::NONE,
            current: Trick::NONE,
            restart: false,
            seq: 0,
            prev_seq: 0,
            elapsed: F::ZERO,
            progress: F::ZERO,
            hold_elapsed: F::ZERO,
            wiggle_time: F::ZERO,
            held: F::ZERO,
            hold_done: false,
            in_progress: false,
            safe: false,
            no_hander: false,
            hand_hold: false,
        }
    }
}

impl Tricks {
    /// Back to nothing: what a respawn does. The queue goes with it, so a trick asked for on the way
    /// down does not fire on the way out.
    pub fn reset(&mut self) {
        self.queue.clear();
        self.requested = Trick::NONE;
        self.current = Trick::NONE;
        self.restart = false;
        self.in_progress = false;
    }

    /// Turn this frame's gesture into a trick request. Returns whether a trick is wanted or running.
    ///
    /// A gesture only counts in the air, with no contact, with the buttons freshly pressed, and when
    /// the rider is in control. If nothing is running the trick starts at once; if something is, the
    /// gesture joins the queue — and either way the buttons must be released before another gesture
    /// can be read.
    ///
    /// With nothing asked for and something running, this is where a queued trick gets its turn: once
    /// the running one is within `overlap` of its end, the queue's head replaces it.
    pub fn request(&mut self, set: &Set, bike: Bike, params: Params) -> bool {
        let ready = !bike.on_ground
            && !bike.contact
            && self.gesture.armed
            && self.gesture.group != 0
            && self.gesture.dir != Dir::None
            && !bike.fresh;
        // An extra slot the pack left empty (MD16) asks for nothing; the 24 are always the original's.
        let filled = |t: &Trick| t.extra().is_none() || set.stunt(*t).is_some();
        if let Some(id) = ready.then(|| trick_of(self.gesture.group, self.gesture.dir)).flatten().filter(filled) {
            if self.in_progress {
                self.queue.push_back(id);
            } else {
                self.requested = id;
                self.restart = true;
                self.seq = 1;
            }
            self.gesture.armed = false;
            return true;
        }
        if !self.in_progress {
            return false;
        }
        // The running trick's own duration, halved, is the most of itself it will give up.
        let duration = set.stunt(self.current).map(|s| s.duration).unwrap_or(F::ZERO);
        let half = duration * F(0.5);
        let overlap = if params.overlap < half { params.overlap } else { half };
        if duration - overlap <= self.elapsed {
            if let Some(next) = self.queue.pop_front() {
                self.requested = next;
                self.restart = true;
                self.seq += 1;
            }
        }
        true
    }

    /// What the rider does when no trick has been asked for: lean in the air, nothing on the ground.
    ///
    /// The original runs this once a frame, from the same place it reads the gesture, whenever no stunt
    /// animation is playing. The airborne case is a trick id of its own rather than a stunt — it is the
    /// three-way blend between the forward, middle and back airborne poses that the stick drives.
    pub fn idle(&mut self, bike: &mut Bike) {
        if bike.wrecked {
            self.requested = Trick::NONE;
        } else if bike.on_ground {
            // Landing is what ends the state a respawn put the rider in.
            bike.fresh = false;
            self.requested = Trick::NONE;
        } else {
            self.requested = Trick::WHEELIE;
        }
        self.seq = 0;
    }

    /// Run the clocks one frame.
    ///
    /// `wiggle` samples the engine's shared smooth-noise wave, which is the only thing in here that
    /// does not come from this module: it is what makes a held pose wobble, it is read once per frame
    /// and it reaches nothing but [`Tricks::progress`].
    ///
    /// `frozen` is the rider's "do not restart anything" state, which the original sets while the
    /// level is paused mid-animation.
    ///
    /// What it answers is the three calls the clocks make on the rider (S79), which is where the
    /// points hang off: [`Clocked`]. A caller with no points to keep drops it.
    pub fn update(&mut self, dt: F, set: &Set, bike: Bike, frozen: bool, wiggle: impl FnOnce(F) -> F) -> Clocked {
        let mut out = Clocked::default();
        // The id the rider latched into its own state, which is what it runs. For a local rider that is
        // the one it asked for this frame; for a networked one it arrived in the packet.
        let latched = self.requested;
        let mut restart = latched != self.current
            || self.restart
            || (self.seq >= 2 && self.seq != self.prev_seq);
        if frozen {
            restart = false;
        }
        self.current = latched;
        self.no_hander = false;

        if let Some(stunt) = set.stunt(self.current).cloned() {
            let duration = stunt.duration;
            if restart {
                if self.restart {
                    self.hand_hold = false;
                    out.stopped_before = true;
                    self.restart = false;
                }
                out.started = Some(self.current);
                self.elapsed = F::ZERO;
                self.held = F::ZERO;
                self.hold_elapsed = F::ZERO;
                self.hold_done = false;
            }
            let hold = stunt.hold_time;
            let remaining = hold - self.elapsed;
            let group_held = self.gesture.group != 0;
            // The hand-hold pose goes on as soon as the trick is as old as its hold time, and only
            // while the buttons that started it are still down.
            self.hand_hold = group_held && !(self.elapsed < hold);
            // Inside the hold: the clock stops and the pose wobbles around where it stopped.
            let holding = group_held
                && !self.hold_done
                && !(hold <= F::ZERO)
                && !(self.elapsed < hold)
                && !(hold + F(0.1) < self.elapsed);
            // Just short of the hold: the clock slows down into it.
            let easing = group_held
                && !(hold <= F::ZERO)
                && !self.hold_done
                && !(remaining <= F::ZERO)
                && remaining < F(0.1);
            if holding {
                let amplitude = remap(self.hold_elapsed, F::ZERO, F(0.5), F::ZERO, F(0.07));
                let wave = wiggle(self.wiggle_time * F(2.0));
                self.wiggle_time = self.wiggle_time + dt;
                self.hold_elapsed = self.hold_elapsed + dt;
                self.held = self.held + dt;
                self.progress = (self.elapsed + amplitude * wave) / duration;
            } else if easing {
                let rate = remap(remaining, F::ZERO, F(0.5), F(0.1), F::ONE);
                self.elapsed = self.elapsed + rate * dt;
                self.held = self.held + dt;
                self.progress = self.elapsed / duration;
            } else {
                if self.held > F::ZERO {
                    self.hold_done = true;
                }
                self.elapsed = self.elapsed + dt;
                self.progress = self.elapsed / duration;
            }

            // Both hands off: the no-hander, still held, with a wheel down.
            if group_held && !(self.held < F::ZERO) && bike.on_ground && self.current.slot() == Some(set.no_hander) {
                self.no_hander = true;
            }
            self.safe = false;
            if duration < self.elapsed {
                self.elapsed = duration;
                self.in_progress = false;
                self.hand_hold = false;
                out.stopped = true;
            } else {
                self.in_progress = true;
                // The no-hander is the one trick that is safe to land from the moment it is held.
                if stunt.safe_time <= self.elapsed
                    || (self.current.slot() == Some(set.no_hander) && stunt.hold_time <= self.elapsed)
                {
                    self.safe = true;
                }
            }
        }
        self.prev_seq = self.seq;
        out
    }

    /// The trick's clause of the landing test: what makes a landing a crash rather than a landing.
    ///
    /// Three ways, in the order the original tests them:
    ///
    /// - touching down with a trick still running that is neither safe yet nor a held no-hander,
    /// - a throw-away taken past 95% of its duration, which is what it is for,
    /// - a no-hander landed below the speed the bike's parameters allow.
    ///
    /// The attitude tests — too much pitch, too much roll, landing backwards — come *before* these in
    /// the original and belong to the bike-in-the-air task; this answers for the trick only, and a
    /// caller that has already decided on one of those never gets here.
    ///
    /// **Nothing calls this.** `air::wreck_test` is the same tail of the same original function with
    /// the attitude tests in front of it, and its reading of the third way is the right one — see
    /// `Params::no_hander_speed`. A rider wires up `air::wreck_test` and hands it the trick machine's
    /// state through `air::Loads`; this stays for the first two ways, which it has right, and for its
    /// tests.
    pub fn landing(&self, set: &Set, bike: Bike, params: Params) -> Option<Wreck> {
        if (bike.on_ground || bike.contact) && self.in_progress && !self.no_hander && !self.safe {
            return Some(Wreck::TRICK);
        }
        // The id itself, not the slot: when the rider's 24 hold no throw-away the search leaves the
        // sentinel 24 here, and 24 is the backflip's id, so the original really does throw a rider off
        // a backflip taken past 95% of whatever progress the last stunt left behind. Comparing slots
        // instead would quietly drop that.
        if usize::from(self.current.0) == set.throwaway && !(self.progress <= F(0.95)) {
            return Some(Wreck::THROWN);
        }
        if self.no_hander && bike.speed < params.no_hander_speed {
            return Some(Wreck::TRICK);
        }
        None
    }
}

/// **What the clocks told the rider this frame** (S79), in the order they told it: the trick that was
/// running stopped because another restarted over it, a trick started, a trick ran its whole length.
/// The original makes these as three calls on the rider's own vtable, and the rider's answer to them
/// is its points ([`crate::score::Score`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Clocked {
    /// A restart over a running trick stopped it first.
    pub stopped_before: bool,
    /// The clock (re)started on this trick.
    pub started: Option<Trick>,
    /// The trick's clock is at its end: told every frame it stays there.
    pub stopped: bool,
}

/// The engine's one-line remap, which the trick clocks use twice: take `x`'s position between `in_lo`
/// and `in_hi`, read it off between `out_lo` and `out_hi`, and clamp it to the output pair whichever
/// way round that pair is. An empty input range gives `out_lo`.
fn remap(x: F, in_lo: F, in_hi: F, out_lo: F, out_hi: F) -> F {
    if in_lo == in_hi {
        return out_lo;
    }
    let t = (x - in_lo) / (in_hi - in_lo);
    let y = out_lo.lerp(out_hi, t);
    y.max(out_lo.min(out_hi)).min(out_lo.max(out_hi))
}

impl hash::State for Tricks {
    fn walk(&self, w: &mut hash::Walk<'_>) {
        w.group("gesture", |w| {
            w.u8("group", self.gesture.group);
            w.u8("dir", self.gesture.dir as u8);
            w.f("x", self.gesture.stick.x);
            w.f("y", self.gesture.stick.y);
            w.f("angle", self.gesture.look.angle);
            w.f("reach", self.gesture.look.reach);
            w.bool("armed", self.gesture.armed);
        });
        let queued: Vec<u8> = self.queue.iter().map(|t| t.0).collect();
        w.blob("queue", &queued);
        w.u8("requested", self.requested.0);
        w.u8("current", self.current.0);
        w.bool("restart", self.restart);
        w.i32("seq", self.seq);
        w.i32("prev_seq", self.prev_seq);
        w.f("elapsed", self.elapsed);
        w.f("progress", self.progress);
        w.f("hold_elapsed", self.hold_elapsed);
        w.f("wiggle_time", self.wiggle_time);
        w.f("held", self.held);
        w.bool("hold_done", self.hold_done);
        w.bool("in_progress", self.in_progress);
        w.bool("safe", self.safe);
        w.bool("no_hander", self.no_hander);
        w.bool("hand_hold", self.hand_hold);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The grid step is the image's single, not a rounded decimal: one digit short and every angle in
    /// the game drifts. `tests/trick.rs` proves the table itself against the original.
    #[test]
    fn the_angle_grid_is_the_image_s_own() {
        assert_eq!(F(AngleTable::STEP).to_bits(), 0x388b_d096);
        assert_eq!(F(AngleTable::SCALE).to_bits(), 0x466a_5e00);
        // The grid spans the whole of the cosine's range: 29,999 steps from -1 lands one bit short of
        // +1, which is why the builder clamps the last few entries rather than trusting the walk.
        let span = F(AngleTable::STEP) * F(fpu::from_i32(29_999));
        assert_eq!(span.to_bits(), F(2.0).to_bits() - 1);
    }

    /// The thresholds are eighths of a turn as the image spells them, which is not what `π/4` rounds
    /// to in either direction — so they cannot be written as a division.
    #[test]
    fn the_direction_thresholds_are_the_image_s_own() {
        assert_eq!(EIGHTH.to_bits(), 0x3f49_0fdb);
        assert_eq!(THREE_EIGHTHS.to_bits(), 0x4016_cbe4);
    }

    /// Six groups by four directions, in the order the table lists its first 24 stunts.
    #[test]
    fn every_gesture_names_one_slot() {
        let dirs = [Dir::Up, Dir::Down, Dir::Left, Dir::Right];
        let mut seen = Vec::new();
        for group in 1..=6u8 {
            for dir in dirs {
                seen.push(slot_of(group, dir).expect("a gesture with a group and a direction"));
            }
        }
        assert_eq!(seen, (0..SLOTS).collect::<Vec<_>>());
        assert_eq!(slot_of(0, Dir::Up), None, "no buttons is no gesture");
        assert_eq!(slot_of(1, Dir::None), None, "no direction is no gesture");
        assert_eq!(slot_of(7, Dir::Up), None);
    }

    /// Holding more buttons means a later group, and all three is its own group rather than one of the
    /// pairs inside it.
    #[test]
    fn the_button_groups_are_ordered() {
        assert_eq!(group([false, false, false]), 0);
        assert_eq!(group([true, false, false]), 1);
        assert_eq!(group([false, true, false]), 2);
        assert_eq!(group([true, true, false]), 3);
        assert_eq!(group([true, false, true]), 4);
        assert_eq!(group([false, true, true]), 5);
        assert_eq!(group([true, true, true]), 6);
        assert_eq!(group([false, false, true]), 0, "the third button alone is not a group");
    }

    /// The d-pad stands in for the stick, axis by axis, and left and up win the pairs.
    #[test]
    fn the_dpad_overrides_the_stick() {
        let s = stick(F(0.5), F(0.5), [true, true, false, false]);
        assert_eq!((s.x, s.y, s.digital), (F(-1.0), F(0.5), [true, false]));
        let s = stick(F(0.5), F(0.5), [false, false, true, true]);
        assert_eq!((s.x, s.y, s.digital), (F(0.5), F(-1.0), [false, true]));
    }

    /// The four quadrants and the dead middle, off a table this test builds.
    #[test]
    fn the_stick_quantises_to_four_directions() {
        let table = AngleTable::build();
        let dir = |x: f32, y: f32| look(&table, stick(F(x), F(y), [false; DPAD])).dir;
        assert_eq!(dir(1.0, 0.0), Dir::Right);
        assert_eq!(dir(0.0, 1.0), Dir::Down);
        assert_eq!(dir(-1.0, 0.0), Dir::Left);
        assert_eq!(dir(0.0, -1.0), Dir::Up);
        assert_eq!(dir(0.3, 0.3), Dir::None, "inside the dead zone of the gesture");
        assert_eq!(dir(0.0, 0.0), Dir::None);
        // Each quadrant is an eighth of a turn either side of its axis. The exact diagonals sit *on*
        // the comparisons and go whichever way the table's last bit falls, so these are just inside.
        assert_eq!(dir(0.9, 0.3), Dir::Right);
        assert_eq!(dir(0.3, 0.9), Dir::Down);
        assert_eq!(dir(-0.9, 0.3), Dir::Left);
        assert_eq!(dir(-0.9, -0.3), Dir::Left);
        assert_eq!(dir(0.3, -0.9), Dir::Up);
        // Sideways and a little down is still down, not left: the quadrant is wider than a quarter of
        // the dial would be.
        assert_eq!(dir(-0.3, 0.9), Dir::Down);
    }

    /// The identity map is what the loader writes, and a rider's own copy of it is the table's first
    /// 24 records in order.
    #[test]
    fn the_default_map_is_the_identity() {
        assert_eq!(Stunts::IDENTITY[0], 0);
        assert_eq!(Stunts::IDENTITY[SLOTS - 1], (SLOTS - 1) as i32);
        let mut stunts = Stunts {
            stunts: (0..STUNTS)
                .map(|i| Stunt { name: format!("s{i}"), index: i as i32, ..Stunt::default() })
                .collect(),
            ..Stunts::default()
        };
        let set = Set::new(stunts.selection(0));
        assert_eq!(set.stunts.len(), SLOTS);
        assert_eq!(set.stunts[5].name, "s5");
        assert_eq!(set.no_hander, Set::ABSENT, "a table without one stores one past the end");
        assert_eq!(set.throwaway, Set::ABSENT);
        // A remap reaches a rider built after it, and the records come with their own index.
        let mut map = Stunts::IDENTITY;
        map[0] = 56;
        stunts.set_map(1, map);
        let set = Set::new(stunts.selection(1));
        assert_eq!(set.stunts[0].index, 56);
    }

    /// An empty input range gives the low end of the output, which is the guard the original has in
    /// front of the division.
    #[test]
    fn a_remap_with_no_range_takes_the_low_end() {
        assert_eq!(remap(F(3.0), F(1.0), F(1.0), F(0.1), F(1.0)), F(0.1));
        assert_eq!(remap(F(0.25), F::ZERO, F(0.5), F::ZERO, F(0.07)), F(0.035));
        assert_eq!(remap(F(9.0), F::ZERO, F(0.5), F::ZERO, F(0.07)), F(0.07), "clamped");
        assert_eq!(remap(F(-9.0), F::ZERO, F(0.5), F::ZERO, F(0.07)), F::ZERO, "clamped");
    }
}
