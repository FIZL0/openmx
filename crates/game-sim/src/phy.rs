//! **The bike's `.phy` file, as the port reads it**: the text format the disc ships every vehicle's
//! physics in, the whole key-to-parameter map the game's loader walks, and the unit conversions it
//! does on the way.
//!
//! Every number the bike moves with is in one of these files. Before this module the port's
//! parameters came from tests — a generated [`crate::r#move::Params`] for a proof, a hand-written one
//! for a demo — which proves the arithmetic and nothing about the bike. [`Phy::read`] replaces that
//! with the disc: hand it the bytes of `Bike\Race125P.phy` and it gives back the 125 the human rode,
//! with [`Phy::mv`], [`Phy::air`] and [`Phy::max_crossup`] ready for the ground step, the air step and
//! the animation blend.
//!
//! # The format
//!
//! An INI file, read by the engine's own reader rather than by anything standard, and the differences
//! matter:
//!
//! * **Whitespace is squeezed out of the whole line** — not trimmed from the ends, *removed*, except
//!   inside `[...]` or inside double quotes. `Max Roll Command = 45` is the key `MaxRollCommand`.
//! * **Quotes disappear** with the whitespace, so `"Bike\A.phy"` and `Bike\A.phy` are the same value.
//! * A line starting `[` opens a section, one starting `;` is a comment, and anything else is a pair
//!   split at its **first** `=`. There is no trailing-comment syntax: `Spring = 5 ; soft` has the
//!   value `5;soft`, which [`number`] reads as 5.
//! * **A pair with an empty value is dropped**, so `Spring =` leaves `Spring` unset rather than zero.
//! * Section and key matching is **case-insensitive**.
//!
//! And four typed getters on top of that, whose exact behaviour is most of why this module is not
//! three lines of `parse()`:
//!
//! | The original's getter | Here | When the key is missing |
//! |---|---|---|
//! | a float with no default | [`Section::float`] | the destination is **left alone** |
//! | a float with a default | [`Section::float_or`] | the default is written |
//! | a quadword | [`Section::vector`] | the default quadword is written |
//! | an int, or `T`/`F`/`ON`/`OFF`/`Y`/`N` | [`Section::int`], [`Section::flag`] | left alone / the default |
//!
//! "Left alone" is not a detail: the retail files have no `MaxRollRateCommand`, so what the bike ends
//! up with is whatever its constructor left there, which is zero — and [`Phy::read`] starting from
//! [`Default`] is what makes that come out right.
//!
//! # The conversions
//!
//! The file is in the units a rider would quote and the simulation is not, so the loader scales as it
//! reads. All of them, in one place, because they are easy to lose: speeds are in miles per hour and
//! are multiplied by [`MPH`]; angles are in degrees and are multiplied by [`DEG`]; `Weight` is in
//! pounds and becomes a mass through `/32.2`, which also turns the two spring rates per wheel into
//! chassis units and `CollisionWeight` into the collision mass; `WheelieTopSpeed` is inverted as well
//! as scaled; `MinDonutAngle` is *negated* as it is turned into radians, and a second angle `-pi`
//! away from it is stored beside it; and two angles — `NoHanderMaxRollAngle` and
//! `HighSpeedMinCollisionAngle` — are turned into their **cosines** through the engine's curve table,
//! because what reads them compares them against a dot product. See [`curve_cos`].
//!
//! # What is here and what is not
//!
//! This module is the loader: the reader, the key map, and every piece of arithmetic the loader does
//! itself. The functions it *calls* — the wheel's suspension and tire setters, the engine's three, the
//! collision record's seven — belong to the wheel, engine and collision modules, so what is here is
//! their arguments: the value the file gives, scaled the way the loader scales it, in a named field.
//! The two exceptions are the arithmetic whose result the loader reads back out and keeps itself
//! ([`spring_rate`], which is why [`crate::r#move::Params`] has the four spring fields) and the
//! engine's three setters, whose output [`crate::r#move::Engine`] already models field for field.
//!
//! `tests/phy.rs` proves it: the reader against the original's reader over every key of both retail
//! Race125 files and a set of generated edge cases, the whole load against the original's loader run
//! in the interpreter, and the result against the bike in a capture of the human's own freeride
//! session.

#![deny(clippy::float_arithmetic)]

use crate::camera::{CURVE_ENTRIES, CurveTable};
use crate::{air, motion, r#move as mv};
use ps2_float::{F, Vf, fpu};

// --- the units the file is written in ---------------------------------------------------------------

/// Miles per hour to the simulation's speed unit (feet per second: 5280/3600 as the file's author
/// rounded it).
pub const MPH: f32 = 1.46666;
/// Degrees to radians, as the original rounds it — not as `f32` rounds `pi/180`.
pub const DEG: f32 = 0.01745329;
/// The pi the loader's angle wrap uses, and its reciprocal turn and double.
const PI: f32 = 3.1415927;
const TWO_PI: f32 = 6.2831855;
const INV_TWO_PI: f32 = 0.15915494;
/// Index scale of the engine's curve table: `(angle + pi) * INV_STEP` is the entry.
const CURVE_INV_STEP: f32 = 954.77045;
/// Feet per second squared, the gravity the file's `Weight` is a weight under: `Weight/G` is the mass
/// and `mass*G` is a force again.
pub const G: f32 = 32.2;
/// Pounds to the unit the collision record wants its mass in (1/32.2 again, rounded differently).
const COLLISION_MASS_PER_POUND: f32 = 0.0310559;

// --- the text ---------------------------------------------------------------------------------------

/// One `.phy` (or `.ini`, or any of the other files the engine reads with this reader), parsed into
/// its sections and pairs and nothing else: no typing, no defaults, no interpretation.
///
/// Parsing is the cheap half — [`Section`]'s getters are where the original's behaviour lives.
#[derive(Clone, Debug, Default)]
pub struct Ini {
    /// Section names as the file writes them, in file order.
    names: Vec<String>,
    /// `(section, key, value)`, in file order, with the whitespace and quotes already squeezed out.
    pairs: Vec<(usize, String, String)>,
}

impl Ini {
    /// Read a file's bytes. Anything that is not valid UTF-8 is read as Latin-1, because the reader
    /// works a byte at a time and some of the disc's files have a stray high byte in a comment.
    pub fn parse_bytes(bytes: &[u8]) -> Ini {
        let text: String = bytes.iter().map(|&b| b as char).collect();
        Ini::parse(&text)
    }

    /// Read a file's text, line by line, the way the reader's line classifier does.
    pub fn parse(text: &str) -> Ini {
        let mut ini = Ini::default();
        let mut section = None;
        for raw in text.split('\n') {
            let line = squeeze(raw);
            let mut chars = line.chars();
            match chars.next() {
                None | Some(';') => {}
                Some('[') => {
                    let name = match line.rfind(']') {
                        Some(end) => &line[1..end],
                        None => &line[1..],
                    };
                    section = Some(ini.names.len());
                    ini.names.push(name.to_string());
                }
                _ => {
                    // The pair splits at the *first* `=`; a line with none, or with nothing after it,
                    // is not a pair at all.
                    let Some(at) = line.find('=') else { continue };
                    let (key, value) = (&line[..at], &line[at + 1..]);
                    if value.is_empty() {
                        continue;
                    }
                    if let Some(which) = section {
                        ini.pairs.push((which, key.to_string(), value.to_string()));
                    }
                }
            }
        }
        ini
    }

    /// The section of that name, for reading values out of. Matching is case-insensitive, and a name
    /// the file does not have gives an empty section rather than nothing, so a load of a file that is
    /// missing a whole section comes out as that section's defaults.
    pub fn section(&self, name: &str) -> Section<'_> {
        let which = self.names.iter().position(|n| eq_ignore_case(n, name));
        Section { ini: self, which }
    }

    /// Every pair in the file, as `(section, key, value)`. For a proof that wants to walk the lot.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str, &str)> {
        self.pairs.iter().map(|(s, k, v)| (self.names[*s].as_str(), k.as_str(), v.as_str()))
    }

    /// The section names, in file order.
    pub fn names(&self) -> &[String] {
        &self.names
    }
}

/// Whitespace out, quotes out — except between `[` and `]`, where the reader keeps spaces so that a
/// section may be called `Front Tire`, and inside quotes, where it keeps them so that a path may
/// have one.
fn squeeze(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut depth = 0i32;
    let mut quoted = false;
    for c in line.chars() {
        let mut keep = true;
        match c {
            ' ' | '\t' | '\r' | '\n' => keep = depth >= 1 || quoted,
            ']' => depth -= 1,
            '"' => {
                quoted = !quoted;
                keep = false;
            }
            '[' => depth += 1,
            _ => {}
        }
        if keep {
            out.push(c);
        }
    }
    out
}

fn eq_ignore_case(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).all(|(x, y)| x.eq_ignore_ascii_case(&y))
}

/// One section of an [`Ini`], and the four typed getters the loader reads it with.
#[derive(Clone, Copy, Debug)]
pub struct Section<'a> {
    ini: &'a Ini,
    which: Option<usize>,
}

impl<'a> Section<'a> {
    /// The value of a key as the file writes it, or `None` when the section does not have one. The
    /// first match wins, as it must for a linear scan.
    pub fn text(&self, key: &str) -> Option<&'a str> {
        let which = self.which?;
        self.ini
            .pairs
            .iter()
            .find(|(s, k, _)| *s == which && eq_ignore_case(k, key))
            .map(|(_, _, v)| v.as_str())
    }

    /// A float, or `None` when the key is missing — which the original signals by leaving the
    /// destination alone, so every caller here keeps what it had.
    pub fn float(&self, key: &str) -> Option<F> {
        self.text(key).map(number)
    }

    /// A float with a default.
    pub fn float_or(&self, key: &str, default: F) -> F {
        self.float(key).unwrap_or(default)
    }

    /// A quadword: up to four numbers separated by spaces or commas. A value with fewer than four
    /// takes the default's `w`, and one with fewer than three reads the missing lanes as zero (the
    /// original hands `strtod` a null pointer there, which is a bug in a file nobody shipped).
    pub fn vector(&self, key: &str, default: Vf) -> Vf {
        let Some(text) = self.text(key) else { return default };
        // Three different delimiter sets, as the original's three `strtok` calls pass: the `w` is the
        // only one that may not be separated by a comma, which costs nothing on a well-formed line.
        let mut rest = text;
        let x = token(&mut rest, " ,");
        let y = token(&mut rest, " ,");
        let z = token(&mut rest, " ,\n");
        let w = token(&mut rest, " \n");
        Vf::new(
            number(x.unwrap_or("")).0,
            number(y.unwrap_or("")).0,
            number(z.unwrap_or("")).0,
            match w {
                Some(w) => number(w).0,
                None => default.w(),
            },
        )
    }

    /// An integer, or a `T`/`F`/`ON`/`OFF`/`Y`/`N` word. `None` when the key is missing.
    pub fn int(&self, key: &str) -> Option<i32> {
        self.text(key).map(|v| {
            let first = v.as_bytes().first().copied().unwrap_or(0).to_ascii_uppercase();
            // The original tests the first letter for `T`, `F`, `Y` and `N` and the whole value for
            // `ON` and `OFF`, in that order, and falls through to `strtol` with base 0.
            match first {
                b'T' => 1,
                b'F' => 0,
                _ if eq_ignore_case(v, "ON") => 1,
                _ if eq_ignore_case(v, "OFF") => 0,
                b'Y' => 1,
                b'N' => 0,
                _ => integer(v),
            }
        })
    }

    /// A flag with a default: [`Section::int`] read as a bool.
    pub fn flag(&self, key: &str, default: bool) -> bool {
        self.int(key).map_or(default, |v| v != 0)
    }
}

/// One `strtok` step: the next run of non-delimiter characters, with `rest` left after it.
fn token<'a>(rest: &mut &'a str, delims: &str) -> Option<&'a str> {
    let start = rest.find(|c| !delims.contains(c))?;
    let text = &rest[start..];
    match text.find(|c| delims.contains(c)) {
        Some(end) => {
            *rest = &text[end + 1..];
            Some(&text[..end])
        }
        None => {
            *rest = "";
            Some(text)
        }
    }
}

/// A value as a float: the C library's `strtod` on the leading number, and then the EE's
/// double-to-float conversion, which **truncates** — `0.1` in a file is the `f32` *below* a tenth,
/// and a port that rounds to nearest is one bit out before it has done any arithmetic.
pub fn number(text: &str) -> F {
    let v = fpu::chop(decimal(text));
    // The game's double-to-float conversion is a routine rather than an instruction, and it drops
    // the sign of a zero: `-0.0` in a file reaches the destination as a plain zero.
    if v == 0.0 { F::ZERO } else { F(v) }
}

/// The leading number of a string as a double, the way `strtod` reads one: optional sign, digits with
/// an optional point, an optional exponent, and zero for a string that does not start with one.
fn decimal(text: &str) -> f64 {
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    let from = i;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let digits = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let before = i - digits;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
    }
    if i - digits == before && before == 0 {
        return 0.0; // nothing numeric at all, or a lone point
    }
    if i < b.len() && b[i] | 32 == b'e' {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let exponent = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > exponent {
            i = j;
        }
    }
    text[from..i].parse::<f64>().unwrap_or(0.0)
}

/// A value as an integer: `strtol` with base 0, so `0x20` is 32 and `020` is 16.
pub fn integer(text: &str) -> i32 {
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    let negative = i < b.len() && b[i] == b'-';
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let mut radix = 10;
    if b.len() > i + 1 && b[i] == b'0' && b[i + 1] | 32 == b'x' {
        radix = 16;
        i += 2;
    } else if i < b.len() && b[i] == b'0' {
        radix = 8;
    }
    // The library's `long` is sixty-four bits wide here, so a value an `int` cannot hold is not
    // clamped to one: it is worked out in full, clamped only at a `long`'s own limits, and the
    // destination keeps the low word of it.
    let mut value: i64 = 0;
    while i < b.len() {
        let Some(d) = (b[i] as char).to_digit(radix) else { break };
        value = value.saturating_mul(radix as i64).saturating_add(d as i64);
        i += 1;
    }
    if negative { value.wrapping_neg() as i32 } else { value as i32 }
}

// --- what a file says -------------------------------------------------------------------------------

/// `[Mass Properties]`, minus the four values that belong to the ground step (which are in
/// [`Phy::mv`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mass {
    /// Pounds, as the file gives it, and the mass the chassis gets: `Weight/32.2`.
    pub weight: F,
    pub mass: F,
    /// Multiples of normal gravity. The force itself is [`crate::r#move::Params::gravity`].
    pub gravity: F,
    /// The diagonal of the inertia tensor, about the three body axes.
    pub inertia: Vf,
    /// Where the centre of gravity moves to while the rider is down.
    pub wreck_cofg: Vf,
    /// The limit the chassis clamps its yaw rate to, and its negative.
    pub yaw_rate_limit: F,
}

/// One row of a tire's per-surface tables: what the tire does on dirt, on mud, on tarmac. The file
/// numbers them from 1 and the tables are indexed from 0; a tire may have up to sixteen.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Surface {
    pub side_force: F,
    pub slipping_side_force: F,
    pub max_drive_force: F,
    pub max_side_force: F,
    pub max_slipping_side_force: F,
    pub skid_drive_force: F,
    pub skid_side_force: F,
    pub skid_slipping_side_force: F,
    pub slipping_side_force_round_off: F,
    pub slipping_side_force_drop_off: F,
    pub slipping_side_force_scale: F,
    pub slipping_side_force_speed_scale: F,
    pub braking_side_force_round_off: F,
    pub braking_side_force_drop_off: F,
    pub braking_side_force_scale: F,
    pub braking_side_force_speed_scale: F,
    pub side_force_round_off: F,
    pub side_force_drop_off: F,
    pub side_force_scale: F,
    pub side_force_speed_scale: F,
    pub max_braking_torque: F,
    pub wheel_friction: F,
    pub wheel_drag: F,
    /// `[Rear Tire]` only: the front's call passes a zero in its place, so zero is what the front
    /// keeps here.
    pub donuts_side_force_speed_scale: F,
    /// `[Front Tire]` only, and the only two of a surface's numbers that are kept on the bike rather
    /// than on the wheel: how long a pass through this surface muddies the rider, and by how much.
    pub mud_time: F,
    pub mud_amount: F,
}

/// `[Front Tire]` or `[Rear Tire]`: the suspension, the tire, the steering and the surfaces. The two
/// sections share every key but four — the three `Steering*` and `MaxSteerAngle` are read for the
/// front only, and `OtherTireNormalLoadScale` for the rear only — so one struct covers both, with the
/// unread ones left at what the wheel's constructor has.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tire {
    pub use_parametric_curve: bool,
    /// The suspension: how far it travels, how fast it may compress and extend, the two spring rates
    /// and the two damping ratios. The spring rates here are the file's; [`spring_rate`] is what the
    /// wheel gets.
    pub strut_compression: F,
    pub max_compression_rate: F,
    pub max_decompression_rate: F,
    pub spring: F,
    pub spring_when_stopped: F,
    pub bounce_damping_ratio: F,
    pub rebound_damping_ratio: F,
    /// The tire itself.
    pub tire_radius: F,
    pub tire_inertia: F,
    /// The front's steering: the torque the rider's lean makes, the damping and the inertia it turns
    /// against, and the limit, which the loader has already turned into radians (with its negative
    /// for the other side).
    pub steering_torque: F,
    pub steering_damping: F,
    pub steering_inertia: F,
    pub max_steer_angle: F,
    /// Where on the chassis this tire's force is applied.
    pub body_tire_force_position: Vf,
    /// The rear's share of the *other* wheel's normal load. One, unless the file says otherwise.
    pub other_tire_normal_load_scale: F,
    /// `NumberOfSurfaces` rows, in the file's order.
    pub surfaces: Vec<Surface>,
}

/// The two numbers `[Engine]` leaves in the engine that [`crate::r#move::Engine`] has no field for:
/// the highest torque on the curve and the revs it happens at.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Peak {
    pub torque: F,
    pub rpm: F,
}

/// `[Roll Control]`: what the stick may ask of the bike's lean, and the control loop that answers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Roll {
    /// Radians, converted from the file's degrees.
    pub max_roll_command: F,
    /// Not in either retail file, so this is whatever the bike's constructor left — zero.
    pub max_roll_rate_command: F,
    pub max_roll_speed: F,
    pub max_roll_yaw_rate: F,
    pub roll_gain: F,
    /// The speed band the rider stands the bike upright over, in the simulation's units.
    pub min_stand_upright_speed: F,
    pub max_stand_upright_speed: F,
    pub roll_control_gain: F,
    pub roll_control_rate_gain: F,
    pub roll_control_max_cg: F,
    pub max_roll_rate_disturbance: F,
}

/// `[Pitch Control]`: the wheelie and the stoppie.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pitch {
    /// Radians, from the file's degrees.
    pub max_pitch_command: F,
    pub pitch_gain: F,
    pub pitch_rate_gain: F,
    /// `1/(WheelieTopSpeed * MPH)`: the loader keeps the reciprocal, because what reads it divides.
    pub inv_wheelie_top_speed: F,
    pub max_stoppie_pitch_command: F,
    pub stoppie_pitch_gain: F,
    pub stoppie_yaw_rate_scale: F,
    pub stoppie_top_speed: F,
    /// Not one of the pitch controller's own: its stoppie latch compares the front brake against a
    /// literal zero and never touches this word. The pad layer is what reads it — while the latch is
    /// set, a held brake button is worth this much instead of the 1.0 it is otherwise worth.
    pub stoppie_brake: F,
}

/// `[Joystick]`: how fast each control may move, which is the only thing between the pad and the
/// bike. The two steering fades are in [`crate::r#move::Params`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Joystick {
    pub x_axis_rate_limit: F,
    pub y_axis_rate_limit: F,
    pub x_dpad_rate_limit: F,
    pub y_dpad_rate_limit: F,
    pub gas_rate_limit: F,
    pub brake_rate_limit: F,
    pub clutch_in_rate_limit: F,
    pub clutch_out_rate_limit: F,
}

/// `[Donuts]`, minus the three the ground step reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Donuts {
    pub min_donut_speed: F,
    pub roll_angle_scale: F,
}

/// `[No Hander Lander]`: the balancing act the rider does on landing with both hands off.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Balance {
    pub min_balance_inertia: F,
    pub max_balance_inertia: F,
    pub balance_inertia_rate: F,
    pub input_inertia: F,
    pub max_balance_point: F,
    pub balance_noise_amplitude: F,
    pub balance_noise_time_scale: F,
}

/// The two clocks `[Wrecks]` keeps that the air module's wreck test does not read.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WreckTimes {
    pub max_wreck_time: F,
    pub cannonball_wreck_time: F,
}

/// `[Collisions]`: the bike's own collision record — a wheel-side set, a body-side set that falls
/// back to the wheel's where the file leaves it at `-1`, and the high-speed sweep's settings.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Collisions {
    pub max_compression: F,
    pub spring: F,
    pub damping: F,
    pub restitution: F,
    pub friction: F,
    pub spring_moment_scale: Vf,
    pub friction_moment_scale: Vf,
    pub angular_impulse_enabled: bool,
    pub body_max_compression: F,
    pub body_spring: F,
    pub body_damping: F,
    pub body_restitution: F,
    pub body_friction: F,
    pub body_spring_moment_scale: Vf,
    pub body_friction_moment_scale: Vf,
    pub body_angular_impulse_enabled: bool,
    pub check_for_high_speed_collisions: bool,
    pub high_speed_ray_offset: Vf,
    pub high_speed_ray_speed_scale: F,
    pub high_speed_unpenetration_distance: F,
    /// The **cosine** of the file's angle, through the curve table — see [`curve_cos`].
    pub high_speed_min_collision_angle: F,
    /// One friction, written into all sixteen of the record's surface slots.
    pub terrain_friction: F,
    /// Pounds, defaulting to `[Mass Properties]`'s `Weight`, and the mass the record gets.
    pub collision_weight: F,
    pub collision_mass: F,
    pub collision_cofg_offset: Vf,
}

/// `[FirstPersonCamera]`: where the rider's eyes sit, per shoulder.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shoulders {
    pub left: Vf,
    pub right: Vf,
}

/// `[Animation]`: the one blend time the file sets, and the box the rider's pelvis collides with.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Animation {
    pub stunt_overlap_time: F,
    pub pelvis_box_position: Vf,
    pub pelvis_box_size: Vf,
}

/// A whole `.phy` file, loaded: every parameter the original's loader writes into a bike, in the
/// units it writes them in.
///
/// The three fields the rest of the port is waiting for are [`Phy::mv`], [`Phy::air`] and
/// [`Phy::max_crossup`]; the rest are the arguments of the setters this module does not call, kept
/// because a parameter that is read and dropped is a parameter nobody can find later.
#[derive(Clone, Debug)]
pub struct Phy {
    pub mass: Mass,
    pub front: Tire,
    pub rear: Tire,
    /// The engine, as its three setters leave it: the power curve already in torque, the gearbox in
    /// first, and the reciprocal of the inertia.
    pub engine: mv::Engine,
    pub peak: Peak,
    pub roll: Roll,
    pub pitch: Pitch,
    pub joystick: Joystick,
    pub donuts: Donuts,
    pub balance: Balance,
    pub wreck_times: WreckTimes,
    pub collisions: Collisions,
    pub shoulders: Shoulders,
    pub animation: Animation,
    /// The ground step's parameters, filled from six sections of the file.
    pub mv: mv::Params,
    /// The air step's parameters, filled from four.
    pub air: air::Params,
    /// `[Airborne Control]`'s `MaxCrossupAngle`, in radians: the full crossup the animation blend
    /// maps the pose against.
    pub max_crossup: F,
}

impl Phy {
    /// Read a file's bytes.
    pub fn read_bytes(bytes: &[u8], curve: &CurveTable) -> Phy {
        Phy::read(&Ini::parse_bytes(bytes), curve)
    }

    /// Load a parsed file, in the order the original's loader reads it — which matters in three
    /// places: `Weight` becomes the mass that the spring rates and the gravity force are scaled by,
    /// and it is also the default for `CollisionWeight`.
    pub fn read(ini: &Ini, curve: &CurveTable) -> Phy {
        let mut mv = mv::Params::default();

        // --- [Mass Properties] ---
        let s = ini.section("Mass Properties");
        let weight = s.float_or("Weight", F::ZERO);
        let gravity = s.float_or("Gravity", F::ONE);
        // The chassis' mass, which three later sections are scaled by. `div.s` rounds to nearest
        // where the rest of the EE's arithmetic chops, so this one is not a truncating multiply.
        let mass = weight / G;
        let mass_properties = Mass {
            weight,
            mass,
            gravity,
            inertia: s.vector("Inertia", UNIT_W),
            wreck_cofg: s.vector("WreckCofG", UNIT_W),
            yaw_rate_limit: s.float("YawRateLimit").unwrap_or(F::ZERO),
        };
        mv.cofg = s.vector("CofG", UNIT_W);
        mv.balance_cofg = s.vector("BalanceCofG", UNIT_W);
        mv.cofg_travel_forward = s.float_or("CofGTravelForward", F::ZERO);
        mv.cofg_travel_back = s.float_or("CofGTravelBack", F::ZERO);
        mv.brake_cofg_travel_forward = s.float_or("BrakeCofGTravelForward", F::ZERO);
        // A force, not an acceleration: the mass is already in it. The `w` is a literal.
        mv.gravity = Vf::new(0.0, (-mass * gravity * G).0, 0.0, 1.0);

        // --- [Front Tire] and [Rear Tire] ---
        let front = Tire::read(ini.section("Front Tire"), Side::Front);
        let rear = Tire::read(ini.section("Rear Tire"), Side::Rear);
        mv.front_spring_when_stopped = spring_rate(front.spring_when_stopped, mass);
        mv.front_spring = spring_rate(front.spring, mass);
        mv.rear_spring_when_stopped = spring_rate(rear.spring_when_stopped, mass);
        mv.rear_spring = spring_rate(rear.spring, mass);

        // --- [Engine] ---
        let s = ini.section("Engine");
        let mut engine = mv::Engine::default();
        let points = s.int("NumberOfRPMPoints").unwrap_or(0).clamp(0, mv::MAX_RPM_POINTS as i32);
        let mut peak = Peak::default();
        for i in 0..points as usize {
            let rpm = s.float(&format!("RPM{}", i + 1)).unwrap_or(F::ZERO);
            let hp = s.float(&format!("HP{}", i + 1)).unwrap_or(F::ZERO);
            engine.curve_rpm[i] = rpm;
            // Horsepower to torque. A point at zero revs would divide by zero, so it gets no torque.
            engine.curve_torque[i] = if rpm > F::ZERO { hp * 5252.0 / rpm } else { F::ZERO };
            if engine.curve_torque[i] > peak.torque {
                peak = Peak { torque: engine.curve_torque[i], rpm };
            }
        }
        // The rev limiter is the last point of the curve, so the engine is never asked for a torque
        // past the end of its own table.
        engine.rev_limit = engine.curve_rpm[points.max(1) as usize - 1];
        engine.curve = mv::Curve::new(points as u32);
        let gears = s.int("NumberOfGears").unwrap_or(0).clamp(0, engine.gear_ratios.len() as i32);
        engine.gears = gears;
        for i in 0..gears as usize {
            engine.gear_ratios[i] = s.float(&format!("GearRatio{}", i + 1)).unwrap_or(F::ZERO);
        }
        engine.final_drive = s.float("FinalGearRatio").unwrap_or(F::ZERO);
        let inertia = s.float("EngineInertia").unwrap_or(F::ZERO);
        if inertia > F::ZERO {
            engine.inv_inertia = F::ONE / inertia;
        }
        engine.min_rpm = s.float("MinRPM").unwrap_or(F::ZERO);
        engine.idle_throttle = s.float("IdleThrottle").unwrap_or(F::ZERO);
        engine.engine_braking = s.float("EngineBraking").unwrap_or(F::ZERO);
        engine.clutch_static = s.float("ClutchStaticFriction").unwrap_or(F::ZERO);
        engine.clutch_kinetic = s.float("ClutchKineticFriction").unwrap_or(F::ZERO);
        // The gearbox setter leaves the box in first, which fills the gear's two ratios.
        mv::set_gear(&mut engine, 1);
        mv.clutch_torque_scale_time = s.float("ClutchTorqueScaleTime").unwrap_or(F::ZERO);
        mv.max_clutch_torque_scale = s.float("MaxClutchTorqueScale").unwrap_or(F::ZERO);
        mv.clutch_cofg_travel_back = s.float_or("ClutchCofGTravelBack", F::ZERO);
        mv.up_shift_rpm = s.float("UpShiftRPM").unwrap_or(F::ZERO);
        mv.down_shift_rpm = s.float("DownShiftRPM").unwrap_or(F::ZERO);
        mv.reverse_gain = s.float_or("ReverseGain", F::ZERO);
        mv.reverse_speed = s.float_or("ReverseSpeed", F::ZERO) * MPH;
        mv.max_clutch_torque_scale_speed = s.float("MaxClutchTorqueScaleSpeed").unwrap_or(F::ZERO) * MPH;

        // --- [Roll Control] ---
        let s = ini.section("Roll Control");
        let roll = Roll {
            max_roll_command: s.float("MaxRollCommand").unwrap_or(F::ZERO) * DEG,
            max_roll_rate_command: s.float("MaxRollRateCommand").unwrap_or(F::ZERO),
            max_roll_speed: s.float("MaxRollSpeed").unwrap_or(F::ZERO) * MPH,
            max_roll_yaw_rate: s.float("MaxRollYawRate").unwrap_or(F::ZERO),
            roll_gain: s.float("RollGain").unwrap_or(F::ZERO),
            min_stand_upright_speed: s.float_or("MinStandUprightSpeed", F(10.0)) * MPH,
            max_stand_upright_speed: s.float_or("MaxStandUprightSpeed", F(30.0)) * MPH,
            roll_control_gain: s.float("RollControlGain").unwrap_or(F::ZERO),
            roll_control_rate_gain: s.float("RollControlRateGain").unwrap_or(F::ZERO),
            roll_control_max_cg: s.float("RollControlMaxCG").unwrap_or(F::ZERO),
            max_roll_rate_disturbance: s.float("MaxRollRateDisturbance").unwrap_or(F::ZERO),
        };
        // Nine of those eleven words are also the ground step's, and it reads the same words the
        // loader wrote — so they are copied here rather than taken off the file a second time.
        // `MaxRollRateCommand` and `MaxRollYawRate` are the two nothing on the ground reads.
        mv.max_roll_command = roll.max_roll_command;
        mv.max_roll_speed = roll.max_roll_speed;
        mv.roll_gain = roll.roll_gain;
        mv.min_stand_upright_speed = roll.min_stand_upright_speed;
        mv.max_stand_upright_speed = roll.max_stand_upright_speed;
        mv.roll_control_gain = roll.roll_control_gain;
        mv.roll_control_rate_gain = roll.roll_control_rate_gain;
        mv.roll_control_max_cg = roll.roll_control_max_cg;
        mv.max_roll_rate_disturbance = roll.max_roll_rate_disturbance;

        // --- [Pitch Control] ---
        let s = ini.section("Pitch Control");
        let wheelie_top_speed = s.float("WheelieTopSpeed").unwrap_or(F::ZERO);
        let pitch = Pitch {
            max_pitch_command: s.float("MaxPitchCommand").unwrap_or(F::ZERO) * DEG,
            pitch_gain: s.float("PitchGain").unwrap_or(F::ZERO),
            pitch_rate_gain: s.float("PitchRateGain").unwrap_or(F::ZERO),
            inv_wheelie_top_speed: F::ONE / (wheelie_top_speed * MPH),
            max_stoppie_pitch_command: s.float("MaxStoppiePitchCommand").unwrap_or(F::ZERO) * DEG,
            stoppie_pitch_gain: s.float("StoppiePitchGain").unwrap_or(F::ZERO),
            stoppie_yaw_rate_scale: s.float("StoppieYawRateScale").unwrap_or(F::ZERO),
            stoppie_top_speed: s.float("StoppieTopSpeed").unwrap_or(F::ZERO) * MPH,
            stoppie_brake: s.float("StoppieBrake").unwrap_or(F::ZERO),
        };
        // All of them but `StoppieBrake`, which the pitch controller does not read: its stoppie latch
        // compares the front brake against a literal zero, and the only thing that reads the key is
        // the pad layer, which makes it the brake the lever asks for while the latch is set.
        mv.max_pitch_command = pitch.max_pitch_command;
        mv.pitch_gain = pitch.pitch_gain;
        mv.pitch_rate_gain = pitch.pitch_rate_gain;
        mv.inv_wheelie_top_speed = pitch.inv_wheelie_top_speed;
        mv.max_stoppie_pitch_command = pitch.max_stoppie_pitch_command;
        mv.stoppie_pitch_gain = pitch.stoppie_pitch_gain;
        mv.stoppie_yaw_rate_scale = pitch.stoppie_yaw_rate_scale;
        mv.stoppie_top_speed = pitch.stoppie_top_speed;

        // --- [Aerodynamics] ---
        mv.drag = ini.section("Aerodynamics").float("Drag").unwrap_or(F::ZERO);

        // --- [Airborne Control], [Preload], [Backflip], [Wrecks] ---
        let s = ini.section("Airborne Control");
        let max_crossup = s.float("MaxCrossupAngle").unwrap_or(F::ZERO) * DEG;
        let pre = ini.section("Preload");
        let flip = ini.section("Backflip");
        let wrecks = ini.section("Wrecks");
        let air = air::Params {
            airborne_control_moment: s.vector("AirborneControlMoment", UNIT_W),
            airborne_damping: s.float("AirborneDamping").unwrap_or(F::ZERO),
            airborne_control_force: s.float("AirborneControlForce").unwrap_or(F::ZERO),
            brake_gyro_torque: s.float("BrakeGyroTorque").unwrap_or(F::ZERO),
            unroll_gain: s.float("UnrollGain").unwrap_or(F::ZERO),
            unroll_torque: s.float("UnrollTorque").unwrap_or(F::ZERO),
            unyaw_gain: s.float("UnyawGain").unwrap_or(F::ZERO),
            unyaw_torque: s.float("UnyawTorque").unwrap_or(F::ZERO),
            // Not in the file: the one the bike's constructor writes.
            control_authority_time: F::ONE,

            preload_time: pre.float("PreloadTime").unwrap_or(F::ZERO),
            max_preload: pre.float("MaxPreload").unwrap_or(F::ZERO),
            min_preload_time: pre.float("MinPreloadTime").unwrap_or(F::ZERO),
            max_preload_time: pre.float("MaxPreloadTime").unwrap_or(F::ZERO),
            min_dpad_preload_time: pre.float("MinDPadPreloadTime").unwrap_or(F::ZERO),
            max_dpad_preload_time: pre.float("MaxDPadPreloadTime").unwrap_or(F::ZERO),

            min_backflip_time: flip.float_or("MinBackflipTime", F::ZERO),
            max_backflip_time: flip.float_or("MaxBackflipTime", F::ZERO),
            min_backflip_torque: flip.float_or("MinBackflipTorque", F::ZERO),
            max_backflip_torque: flip.float_or("MaxBackflipTorque", F::ZERO),
            backflip_torque_rate_increase: flip.float_or("BackflipTorqueRateIncrease", F::ZERO),
            backflip_torque_rate_decrease: flip.float_or("BackflipTorqueRateDecrease", F::ZERO),

            front_wreck_speed_limit: wrecks.float("FrontWreckSpeedLimit").unwrap_or(F::ZERO) * MPH,
            back_wreck_speed_limit: wrecks.float("BackWreckSpeedLimit").unwrap_or(F::ZERO) * MPH,
            side_wreck_speed_limit: wrecks.float("SideWreckSpeedLimit").unwrap_or(F::ZERO) * MPH,
            racing_down_wreck_speed_limit: wrecks.float("RacingDownWreckSpeedLimit").unwrap_or(F::ZERO) * MPH,
            freestyle_down_wreck_speed_limit: wrecks.float("FreestyleDownWreckSpeedLimit").unwrap_or(F::ZERO) * MPH,
            ground_pitch_down_wreck_angle: wrecks.float("GroundPitchDownWreckAngle").unwrap_or(F::ZERO) * DEG,
            ground_pitch_up_wreck_angle: wrecks.float("GroundPitchUpWreckAngle").unwrap_or(F::ZERO) * DEG,
            airborne_pitch_down_wreck_angle: wrecks.float("AirbornePitchDownWreckAngle").unwrap_or(F::ZERO) * DEG,
            airborne_pitch_up_wreck_angle: wrecks.float("AirbornePitchUpWreckAngle").unwrap_or(F::ZERO) * DEG,
            airborne_roll_wreck_angle: wrecks.float("AirborneRollWreckAngle").unwrap_or(F::ZERO) * DEG,
            no_hander_wreck_speed_limit: wrecks.float("NoHanderWreckSpeedLimit").unwrap_or(F::ZERO) * MPH,
            // A cosine, because what reads it has a dot product in its hand.
            no_hander_max_roll_angle: curve_cos(
                wrecks.float("NoHanderMaxRollAngle").unwrap_or(F::ZERO) * DEG,
                curve,
            ),
            joust_wreck_height: wrecks.float("JoustWreckHeight").unwrap_or(F::ZERO),
            crossup_wreck: wrecks.float("CrossupWreck").unwrap_or(F::ZERO),
            crossup_wreck_time: wrecks.float("CrossupWreckTime").unwrap_or(F::ZERO),
        };
        let wreck_times = WreckTimes {
            max_wreck_time: wrecks.float("MaxWreckTime").unwrap_or(F::ZERO),
            cannonball_wreck_time: wrecks.float("CannonballWreckTime").unwrap_or(F::ZERO),
        };

        // --- [Joystick] ---
        let s = ini.section("Joystick");
        let joystick = Joystick {
            x_axis_rate_limit: s.float("XAxisRateLimit").unwrap_or(F::ZERO),
            y_axis_rate_limit: s.float("YAxisRateLimit").unwrap_or(F::ZERO),
            x_dpad_rate_limit: s.float("XDPadRateLimit").unwrap_or(F::ZERO),
            y_dpad_rate_limit: s.float("YDPadRateLimit").unwrap_or(F::ZERO),
            gas_rate_limit: s.float("GasRateLimit").unwrap_or(F::ZERO),
            brake_rate_limit: s.float("BrakeRateLimit").unwrap_or(F::ZERO),
            clutch_in_rate_limit: s.float("ClutchInRateLimit").unwrap_or(F::ZERO),
            clutch_out_rate_limit: s.float("ClutchOutRateLimit").unwrap_or(F::ZERO),
        };
        mv.steer_scale_1 = s.float("SteerScale1").unwrap_or(F::ZERO);
        mv.steer_scale_2 = s.float("SteerScale2").unwrap_or(F::ZERO);
        mv.steer_speed_1 = s.float("SteerSpeed1").unwrap_or(F::ZERO) * MPH;
        mv.steer_speed_2 = s.float("SteerSpeed2").unwrap_or(F::ZERO) * MPH;

        // --- [Donuts] ---
        let s = ini.section("Donuts");
        let donuts = Donuts {
            min_donut_speed: s.float_or("MinDonutSpeed", F::ZERO) * MPH,
            roll_angle_scale: s.float_or("DonutRollAngleScale", F::ONE),
        };
        mv.max_donut_speed = s.float_or("MaxDonutSpeed", F::ZERO) * MPH;
        // One angle in the file, two in the bike: the stick angle a donut starts at, negated into
        // radians, and the one a turn away from it.
        mv.donut_angle = s.float_or("MinDonutAngle", F::ZERO) * -DEG;
        mv.donut_angle_back = F(-PI) - mv.donut_angle;
        // The one the no-hander's roll widens itself by, whose default of 1.0 the ground step would
        // otherwise never see.
        mv.donut_roll_angle_scale = donuts.roll_angle_scale;

        // --- [No Hander Lander] ---
        let s = ini.section("No Hander Lander");
        let balance = Balance {
            min_balance_inertia: s.float("MinBalanceInertia").unwrap_or(F::ZERO),
            max_balance_inertia: s.float("MaxBalanceInertia").unwrap_or(F::ZERO),
            balance_inertia_rate: s.float("BalanceInertiaRate").unwrap_or(F::ZERO),
            input_inertia: s.float("InputInertia").unwrap_or(F::ZERO),
            max_balance_point: s.float("MaxBalancePoint").unwrap_or(F::ZERO),
            balance_noise_amplitude: s.float("BalanceNoiseAmplitude").unwrap_or(F::ZERO),
            balance_noise_time_scale: s.float("BalanceNoiseTimeScale").unwrap_or(F::ZERO),
        };
        // All seven: the balance controller is the ground step's, so every word of this section is.
        mv.min_balance_inertia = balance.min_balance_inertia;
        mv.max_balance_inertia = balance.max_balance_inertia;
        mv.balance_inertia_rate = balance.balance_inertia_rate;
        mv.input_inertia = balance.input_inertia;
        mv.max_balance_point = balance.max_balance_point;
        mv.balance_noise_amplitude = balance.balance_noise_amplitude;
        mv.balance_noise_time_scale = balance.balance_noise_time_scale;

        // --- [Collisions] ---
        let s = ini.section("Collisions");
        let collision_weight = s.float_or("CollisionWeight", weight);
        let collisions = Collisions {
            max_compression: s.float("MaxCompression").unwrap_or(F::ZERO),
            spring: s.float("Spring").unwrap_or(F::ZERO),
            damping: s.float("Damping").unwrap_or(F::ZERO),
            restitution: s.float("Restitution").unwrap_or(F::ZERO),
            friction: s.float("Friction").unwrap_or(F::ZERO),
            spring_moment_scale: s.vector("SpringMomentScale", MINUS_UNIT_W),
            friction_moment_scale: s.vector("FrictionMomentScale", MINUS_UNIT_W),
            angular_impulse_enabled: s.flag("AngularImpulseEnabled", true),
            body_max_compression: s.float_or("BodyMaxCompression", F(-1.0)),
            body_spring: s.float_or("BodySpring", F(-1.0)),
            body_damping: s.float_or("BodyDamping", F(-1.0)),
            body_restitution: s.float_or("BodyRestitution", F(-1.0)),
            body_friction: s.float("BodyFriction").unwrap_or(F::ZERO),
            body_spring_moment_scale: s.vector("BodySpringMomentScale", MINUS_UNIT_W),
            body_friction_moment_scale: s.vector("BodyFrictionMomentScale", MINUS_UNIT_W),
            body_angular_impulse_enabled: s.flag("BodyAngularImpulseEnabled", true),
            check_for_high_speed_collisions: s.flag("CheckForHighSpeedCollisions", true),
            high_speed_ray_offset: s.vector("HighSpeedRayOffset", UNIT_W),
            high_speed_ray_speed_scale: s.float_or("HighSpeedRaySpeedScale", F(2.0)),
            high_speed_unpenetration_distance: s.float_or("HighSpeedUnpenetrationDistance", F(2.0)),
            high_speed_min_collision_angle: curve_cos(
                s.float_or("HighSpeedMinCollisionAngle", F(-1.0)) * DEG,
                curve,
            ),
            terrain_friction: s.float("TerrainFriction").unwrap_or(F::ZERO),
            collision_weight,
            collision_mass: collision_weight * COLLISION_MASS_PER_POUND,
            collision_cofg_offset: s.vector("CollisionCofGOffset", UNIT_W),
        };

        // --- [FirstPersonCamera] and [Animation] ---
        let s = ini.section("FirstPersonCamera");
        let shoulders = Shoulders {
            left: s.vector("Left1stPerShoulderOffset", UNIT_W),
            right: s.vector("Right1stPerShoulderOffset", UNIT_W),
        };
        let s = ini.section("Animation");
        let animation = Animation {
            stunt_overlap_time: s.float_or("StuntOverlapTime", F(0.5)),
            pelvis_box_position: s.vector("PelvisBoxPosition", UNIT_W),
            pelvis_box_size: s.vector("PelvisBoxSize", UNIT_W),
        };

        Phy {
            mass: mass_properties,
            front,
            rear,
            engine,
            peak,
            roll,
            pitch,
            joystick,
            donuts,
            balance,
            wreck_times,
            collisions,
            shoulders,
            animation,
            mv,
            air,
            max_crossup,
        }
    }

    /// The animation blend's parameters: its own blend rate, which is not in the file, and the
    /// crossup angle, which is.
    pub fn motion(&self) -> motion::Params {
        motion::Params { max_crossup: self.max_crossup, ..motion::Params::default() }
    }
}

/// The default quadword almost every vector key in the file falls back to, and the one the four
/// collision moment scales fall back to.
const UNIT_W: Vf = Vf([0.0, 0.0, 0.0, 1.0]);
const MINUS_UNIT_W: Vf = Vf([-1.0, -1.0, -1.0, 1.0]);

/// Which tire a section is, which decides four keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Front,
    Rear,
}

impl Tire {
    fn read(s: Section<'_>, side: Side) -> Tire {
        let front = side == Side::Front;
        let mut tire = Tire {
            use_parametric_curve: s.flag("UseParametricCurve", false),
            strut_compression: s.float_or("StrutCompression", F::ONE),
            max_compression_rate: s.float_or("MaxCompressionRate", F(20.0)),
            max_decompression_rate: s.float_or("MaxDecompressionRate", F::ONE),
            spring: s.float_or("Spring", F::ZERO),
            spring_when_stopped: s.float_or("SpringWhenStopped", F::ZERO),
            bounce_damping_ratio: s.float_or("BounceDampingRatio", F::ZERO),
            rebound_damping_ratio: s.float_or("ReboundDampingRatio", F::ZERO),
            tire_radius: s.float_or("TireRadius", F::ONE),
            tire_inertia: s.float_or("TireInertia", F::ONE),
            steering_torque: if front { s.float_or("SteeringTorque", F::ZERO) } else { F::ZERO },
            steering_damping: if front { s.float_or("SteeringDamping", F::ZERO) } else { F::ZERO },
            steering_inertia: if front { s.float_or("SteeringInertia", F::ZERO) } else { F::ZERO },
            // Degrees in the file, radians on the wheel, which also keeps the negative.
            max_steer_angle: if front { s.float_or("MaxSteerAngle", F::ZERO) * DEG } else { F::ZERO },
            body_tire_force_position: s.vector("BodyTireForcePosition", UNIT_W),
            other_tire_normal_load_scale: if front {
                F::ONE
            } else {
                s.float_or("OtherTireNormalLoadScale", F::ONE)
            },
            surfaces: Vec::new(),
        };
        // `NumberOfSurfaces` has no default: a file without it leaves the loader's own count at
        // whatever was on its stack, so a missing count here means no surfaces at all.
        let n = s.int("NumberOfSurfaces").unwrap_or(0).clamp(0, MAX_SURFACES as i32);
        for i in 1..=n {
            tire.surfaces.push(Surface {
                side_force: s.float_or(&format!("SideForce{i}"), F::ZERO),
                slipping_side_force: s.float_or(&format!("SlippingSideForce{i}"), F::ZERO),
                max_drive_force: s.float_or(&format!("MaxDriveForce{i}"), F::ZERO),
                max_side_force: s.float_or(&format!("MaxSideForce{i}"), F::ZERO),
                max_slipping_side_force: s.float_or(&format!("MaxSlippingSideForce{i}"), F::ZERO),
                skid_drive_force: s.float_or(&format!("SkidDriveForce{i}"), F::ZERO),
                skid_side_force: s.float_or(&format!("SkidSideForce{i}"), F::ZERO),
                skid_slipping_side_force: s.float_or(&format!("SkidSlippingSideForce{i}"), F::ZERO),
                slipping_side_force_round_off: s.float_or(&format!("SlippingSideForceRoundOff{i}"), F::ZERO),
                slipping_side_force_drop_off: s.float_or(&format!("SlippingSideForceDropOff{i}"), F::ZERO),
                slipping_side_force_scale: s.float_or(&format!("SlippingSideForceScale{i}"), F::ZERO),
                slipping_side_force_speed_scale: s.float_or(&format!("SlippingSideForceSpeedScale{i}"), F::ZERO),
                braking_side_force_round_off: s.float_or(&format!("BrakingSideForceRoundOff{i}"), F::ZERO),
                braking_side_force_drop_off: s.float_or(&format!("BrakingSideForceDropOff{i}"), F::ZERO),
                braking_side_force_scale: s.float_or(&format!("BrakingSideForceScale{i}"), F::ZERO),
                braking_side_force_speed_scale: s.float_or(&format!("BrakingSideForceSpeedScale{i}"), F::ZERO),
                side_force_round_off: s.float_or(&format!("SideForceRoundOff{i}"), F::ZERO),
                side_force_drop_off: s.float_or(&format!("SideForceDropOff{i}"), F::ZERO),
                side_force_scale: s.float_or(&format!("SideForceScale{i}"), F::ZERO),
                side_force_speed_scale: s.float_or(&format!("SideForceSpeedScale{i}"), F::ZERO),
                max_braking_torque: s.float_or(&format!("MaxBrakingTorque{i}"), F::ZERO),
                wheel_friction: s.float_or(&format!("WheelFriction{i}"), F::ZERO),
                wheel_drag: s.float_or(&format!("WheelDrag{i}"), F::ZERO),
                donuts_side_force_speed_scale: if front {
                    F::ZERO
                } else {
                    s.float_or(&format!("DonutsSideForceSpeedScale{i}"), F::ZERO)
                },
                mud_time: if front { s.float_or(&format!("MudTime{i}"), F::ZERO) } else { F::ZERO },
                mud_amount: if front { s.float_or(&format!("MudAmount{i}"), F::ZERO) } else { F::ZERO },
            });
        }
        tire
    }
}

/// How many surfaces a tire's tables hold. The setters drop any index past this.
pub const MAX_SURFACES: usize = 16;

/// A spring rate as the wheel gets it: the file's rate is per unit of the bike's own weight, so it is
/// multiplied back out into a force per unit of travel. The absolute value is the original's.
pub fn spring_rate(rate: F, mass: F) -> F {
    (rate * mass * G).abs()
}

/// The damping rate as the wheel works it out: the file gives a *ratio of critical damping*, and the
/// wheel turns it into a rate against the spring it has just been handed —
/// `ratio * 2 * sqrt(mass * rate)`. Both of a wheel's two damping rates come out of the **last**
/// spring the loader sets, which is `Spring` and not `SpringWhenStopped`, because the loader calls
/// the setter again with the moving rate and the setter recomputes the damping each time.
pub fn damping(ratio: F, mass: F, rate: F) -> F {
    ratio * F(2.0) * (mass * rate).sqrt()
}

/// The cosine of an angle, the way the loader takes one: wrapped into `-pi..pi` — twice, because the
/// original's first correction can leave it a turn out — and then read **straight out of the
/// engine's curve table**, with none of the derivative correction a rotation would use. So this is
/// the cosine of the table's nearest entry *below* the angle, a fifteenth of a degree coarse.
pub fn curve_cos(angle: F, curve: &CurveTable) -> F {
    let a = wrap_turn(wrap_turn(angle));
    // The original indexes without checking; an angle it has wrapped cannot leave the table, and the
    // clamp here only catches one it has not (a NaN cannot happen on this hardware).
    let i = ((a + PI) * CURVE_INV_STEP).to_i32().clamp(0, CURVE_ENTRIES as i32);
    F(curve.entries()[i as usize][0])
}

/// One wrap into `-pi..pi`: the whole turns taken off, and then one correction either way.
fn wrap_turn(angle: F) -> F {
    let a = angle - F(fpu::from_i32((angle * INV_TWO_PI).to_i32())) * TWO_PI;
    if a > PI {
        a - TWO_PI
    } else if a < -PI {
        a + TWO_PI
    } else {
        a
    }
}
