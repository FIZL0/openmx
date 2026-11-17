//! The drop gate's sink (P313c): on a track whose `.toy` `[DropGate]` sets `MoveGateDown`, the
//! gate object lowers its pieces out of the way once a rider is past the track's `[LowerGate]`.
//!
//! The race activity's gate object (P313b ported its swing, `race_start::GateSwing` in the shell)
//! gathers its pieces by name when the words ask for the sink: the scene props named after
//! `GateName` ([`soultree_names`]), hung under a group node of its own, and the scene's `.col`
//! meshes numbered after them ([`collision_names`]). Its bind finds the path spot nearest the
//! `[LowerGate]`'s `TriggerPosition` and measures from there to the lap line — the race file's own
//! line, as its load spotted it: the supercross loader binds the gate before it moves the line onto
//! the finish gate (C40, measured on sx17). Each tick it then
//! walks the race's cards, and the first card whose kept spot is farther from the line than that
//! and whose spot is nearer sets the sink off: `MoveTime` seconds in which the group's height runs
//! toward −100, and then stands at −100.
//!
//! The height's step is the original's own: `y = (−100 − y) × (t / MoveTime)`, each tick from the
//! height it holds, not a blend between two ends, so it reaches about −50 at the end of the time and
//! then jumps to −100. Each tick of the sink also tells every gathered collision mesh to take its
//! node's matrix, and the static collision object's node is none, so the matrix is the identity: the
//! meshes leave the gate for the world's origin, where nothing rides ([`Sink::stand`]).
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::race::{Entry, Laps, Spot};
use crate::math::Mat4;
use crate::world::Props;
use ps2_float::{F, Vf, fpu};

/// How deep the sink takes the group: its height at the end.
pub const DOWN: F = F(-100.0);

/// The scene prop names of the gate's pieces, for `count` pieces (`NumberSoultreePieces`): a lone
/// piece is `name` itself; more are `name` with its last two characters cut (when it has more than
/// two) and each piece's number after it, two digits from 1.
pub fn soultree_names(name: &str, count: i32) -> Vec<String> {
    if count == 1 {
        return vec![name.to_string()];
    }
    let base = stem(name, count);
    (1..=count).map(|k| numbered(base, k, "")).collect()
}

/// The `.col` file names of the gate's collision pieces, for `count` of them
/// (`NumberCollisionPieces`) after `soultree` scene pieces: numbered on from the scene pieces, on the
/// same stem ([`soultree_names`]), `.col` after each. A lone scene piece keeps the whole name as the
/// stem.
pub fn collision_names(name: &str, soultree: i32, count: i32) -> Vec<String> {
    let base = stem(name, soultree);
    (soultree + 1..=soultree + count).map(|k| numbered(base, k, ".col")).collect()
}

/// The name's stem the numbers go after: its last two characters cut unless it is one piece or the
/// name has two characters or fewer. A name is ASCII on the disc; a cut that would split a character
/// keeps the whole name.
fn stem(name: &str, soultree: i32) -> &str {
    if soultree != 1 && name.len() > 2 {
        name.get(..name.len() - 2).unwrap_or(name)
    } else {
        name
    }
}

/// `%s0%d` below ten, `%s%d` from ten.
fn numbered(base: &str, k: i32, tail: &str) -> String {
    if k < 10 { format!("{base}0{k}{tail}") } else { format!("{base}{k}{tail}") }
}

/// Where each of `names` is in `files` (a scene's collision list), ignoring case: every entry that
/// matches any name, in the list's order, as the gate gathers them. Missing names gather nothing.
pub fn gathered<'a>(files: impl IntoIterator<Item = &'a str>, names: &[String]) -> Vec<usize> {
    files.into_iter().enumerate().filter(|(_, f)| names.iter().any(|n| n.eq_ignore_ascii_case(f))).map(|(k, _)| k).collect()
}

/// The gate object's sink: its trigger, its clock and the group's height.
#[derive(Clone, Debug, PartialEq)]
pub struct Sink {
    /// The `[LowerGate]` spot's distance to the lap line (`+0xb0`).
    pub trigger: F,
    /// No card has set the sink off yet (`+0xb8`).
    pub armed: bool,
    /// The cards counted past the trigger (`+0xc0`): one is enough, and then the count stops.
    pub count: i32,
    /// The sink's clock and its length (`+0x50`, `+0x54`).
    pub t: F,
    pub length: F,
    /// The sink runs (`+0x58`): set from `MoveGateDown` when the trigger fires.
    pub on: bool,
    /// The group's height (`+0x44`): how far below the scene the pieces are drawn.
    pub y: F,
    /// The words' `MoveTime` (`+0x7c`) and `MoveGateDown` (`+0x78`).
    pub move_time: F,
    pub move_down: bool,
    /// The gathered collision meshes, as indices into the world's props.
    pub pieces: Vec<usize>,
}

impl Sink {
    /// The gate bound to the race: the path spot nearest `trigger_at` (the `[LowerGate]`'s
    /// `TriggerPosition`, every stretch), and its distance along the paths to `line`, kept-off
    /// paths counting −1. `line` is the race file's own line spot, as the load spotted it, not the
    /// course's: the supercross loader binds the gate before it moves the line onto the finish gate
    /// and spots it again (C40: sx17's trigger is 2475.76 to the file's line, 2477.97 to the moved
    /// one). A track with no paths, or a pool run dry, leaves the spot at no path; the original then
    /// measures from a null path, and the port counts the distance 0.
    pub fn bind(laps: &mut Laps, line: Spot, trigger_at: Vf, move_down: bool, move_time: F, pieces: Vec<usize>) -> Sink {
        let track = std::sync::Arc::make_mut(&mut laps.track);
        let mut spot = Spot::default();
        crate::ai::follow::spot(track, trigger_at, false, &mut spot, None);
        let trigger = if spot.at().is_some() && line.at().is_some() { track.distance(spot, line, true) } else { F::ZERO };
        Sink { trigger, armed: true, count: 0, t: F::ZERO, length: F::ZERO, on: false, y: F::ZERO, move_time, move_down, pieces }
    }

    /// **The gate's update**, as far as the sink goes, `dt` long, the race's cards in list order.
    /// Returns true on a tick the sink ran, when the collision pieces are to be stood again
    /// ([`Sink::stand`]).
    ///
    /// While armed, each card is asked in turn: kept spot farther from the line than the trigger,
    /// spot nearer. The first that is sets the clock to 0 over `MoveTime`, the sink on as
    /// `MoveGateDown` says, and disarms. Then, with the sink on: inside its time the height steps
    /// toward [`DOWN`] from where it is (`sub`, `div`, `mul`, in that order), past it the height is
    /// [`DOWN`] and the sink stops; either way the clock runs on by `dt`.
    pub fn tick(&mut self, dt: F, cards: &[Entry]) -> bool {
        if self.armed {
            for e in cards {
                let c = &e.card.lap;
                if self.armed && self.trigger < c.to_line_kept && c.to_line < self.trigger {
                    self.count += 1;
                    if self.count > 0 {
                        self.t = F::ZERO;
                        self.length = self.move_time;
                        self.on = self.move_down;
                        self.armed = false;
                    }
                }
            }
        }
        if !self.on {
            return false;
        }
        if self.t < self.length {
            let left = fpu::sub(DOWN.0, self.y.0);
            self.y = F(fpu::mul(left, fpu::div(self.t.0, self.length.0)));
        } else {
            self.y = DOWN;
            self.on = false;
        }
        self.t = F(fpu::add(self.t.0, dt.0));
        true
    }

    /// The gathered collision meshes told to take their node's matrix: the identity, for a static
    /// collision object, which has no node. A piece the props do not hold is passed over.
    pub fn stand(&self, props: &mut Props) {
        for &k in &self.pieces {
            if let Some(p) = props.props.get_mut(k) {
                p.stand(&Mat4::IDENTITY);
            }
        }
    }
}
