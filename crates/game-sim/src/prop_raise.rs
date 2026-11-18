//! The race's bale raiser (V134): on a race track whose `.toy` has both `[Funnel_Up]` and
//! `[Funnel_Down]` (sx01 among the disc's), the race activity makes an object that moves two groups of
//! knockable props by 50 ft once a card is past each section's spot — the `up_bale` collections rise
//! out of the floor they stand 50 ft under, the `down_bale` ones sink into it, and the start's funnel
//! opens onto the lap.
//!
//! Each section names its group by `ObjectNameSubstring` (`up_bale` and `down_bale` when the file says
//! nothing, as sx01's does): every prop collection whose name holds it (case counts), in the props'
//! order, every body of each. Its `TriggerPosition` is put on the nearest path spot, the spot's point
//! written back over it, and measured along the paths to the course's line, the way the drop gate's
//! sink is ([`crate::drop_gate::Sink::bind`]) but to the line where the supercross loader moved it
//! (sx01's image holds the moved line's 3369.951, not the race file's line's 3371.399). The tick then walks the race's cards, and the first
//! card whose kept spot is not nearer the line than a spot and whose spot is nearer sets that group
//! off: `MoveTime` seconds in which each body is placed, every tick, a share `t / MoveTime` of the
//! way from where it stands to 50 ft over (or under), then at the end of the way, which becomes the
//! place it is put back to. Placing a body is its reset: at rest, its node with it
//! ([`crate::prop_world::Collection::place_at`]). The down group places first, then the up group.
//!
//! The props are not always where the race's tick can reach them (split screen's seats lend one set
//! about, [`crate::prop_seats`]), so the tick keeps what it is to place ([`Raiser::due`]) and
//! [`Raiser::place`] does it to the set, before the set's own step. It draws no `rand()`.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::prop_set::PropSet;
use crate::race::{Entry, Laps, Spot};
use ps2_float::{F, Vf, fpu};

/// How far each group moves.
pub const LIFT: F = F(50.0);

/// One `[Funnel_Up]` / `[Funnel_Down]` section, as the race reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct Words {
    /// `TriggerPosition`, (0,0,0,1) without the key.
    pub trigger_at: Vf,
    /// `MoveTime`, 5 without the key.
    pub move_time: F,
    /// `ObjectNameSubstring`: the up section's `up_bale`, the down one's `down_bale`, without it.
    pub substring: String,
}

/// One group: its spot, its clock and its bodies.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// The section's spot's distance along the paths to the line (`+0x80` up, `+0x84` down).
    pub trigger: F,
    /// No card has set the group off yet (`+0x88`, `+0x8c`).
    pub armed: bool,
    /// The cards counted past the spot (`+0x90`, `+0x94`): one is enough.
    pub count: i32,
    /// The group moves (`+0x68`, `+0x6c`), its clock and its length (`+0x60`/`+0x64`,
    /// `+0x4c`/`+0x50`).
    pub on: bool,
    pub t: F,
    pub length: F,
    /// The section's `MoveTime`, the length the trigger sets.
    pub move_time: F,
    /// Up 50 ft, or down.
    pub rises: bool,
    /// Each body as (collection, object) in the props.
    pub bodies: Vec<(u32, u32)>,
}

/// What the tick left a group to place: a share of the way, or its end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Due {
    Share(F),
    End,
}

/// The raiser: its two groups, and what this tick is to place.
#[derive(Clone, Debug, PartialEq)]
pub struct Raiser {
    pub up: Group,
    pub down: Group,
    /// What the last tick is to place, in its order: (rises, how far). Taken by [`Raiser::place`];
    /// empty between ticks, so not hashed or sent.
    pub due: Vec<(bool, Due)>,
}

/// The collections a section's substring picks out of the props, every body of each, in order.
pub fn gathered(props: &PropSet, substring: &str) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    for (c, name) in props.names.iter().enumerate() {
        if name.contains(substring) {
            let n = props.collections.get(c).map_or(0, |w| w.objects.len());
            out.extend((0..n).map(|o| (c as u32, o as u32)));
        }
    }
    out
}

/// The spot nearest `at` on the paths, its point written back over `at`, and its distance to `line`
/// with kept-off paths counting −1. No spot (no paths), or a line on none: 0.
pub fn trigger(laps: &mut Laps, line: Spot, at: &mut Vf) -> F {
    let track = std::sync::Arc::make_mut(&mut laps.track);
    let mut spot = Spot::default();
    crate::ai::follow::spot(track, *at, false, &mut spot, None);
    track.spot_point(&mut spot, at);
    if spot.at().is_some() && line.at().is_some() { track.distance(spot, line, true) } else { F::ZERO }
}

impl Group {
    fn bind(laps: &mut Laps, line: Spot, words: &Words, props: &PropSet, rises: bool) -> Group {
        let mut at = words.trigger_at;
        let trigger = trigger(laps, line, &mut at);
        let bodies = gathered(props, &words.substring);
        Group { trigger, armed: true, count: 0, on: false, t: F::ZERO, length: F::ZERO, move_time: words.move_time, rises, bodies }
    }

    /// A card's two distances against the spot: the kept one not nearer (`c.ole` then `bc1t`), the
    /// spot's nearer.
    fn passed(&mut self, kept: F, now: F) -> bool {
        if self.armed && !(kept.0 <= self.trigger.0) && now.0 < self.trigger.0 {
            self.count += 1;
            if self.count > 0 {
                self.t = F::ZERO;
                self.length = self.move_time;
                self.on = true;
                self.armed = false;
                return true;
            }
        }
        false
    }

    /// The group's clock for a tick: what to place, then the clock on by `dt`. The end turns the group
    /// off only when it has a body to place, as the original's loop does.
    fn run(&mut self, dt: F) -> Option<Due> {
        if !self.on {
            return None;
        }
        let due = if self.t < self.length {
            Due::Share(F(fpu::div(self.t.0, self.length.0)))
        } else {
            if !self.bodies.is_empty() {
                self.on = false;
            }
            Due::End
        };
        self.t = F(fpu::add(self.t.0, dt.0));
        Some(due)
    }
}

impl Raiser {
    /// The raiser bound to the race: each section's group gathered off the props and its spot measured
    /// to `line`, the course's line. `None` when neither section picks a body out.
    pub fn bind(laps: &mut Laps, line: Spot, up: &Words, down: &Words, props: &PropSet) -> Option<Raiser> {
        let up = Group::bind(laps, line, up, props, true);
        let down = Group::bind(laps, line, down, props, false);
        if up.bodies.is_empty() && down.bodies.is_empty() {
            return None;
        }
        Some(Raiser { up, down, due: Vec::new() })
    }

    /// **The raiser's tick**, `dt` long, the race's cards in list order: each card asked about the up
    /// spot, then the down one; then the down group's clock and the up group's. What they are to place
    /// is kept in [`Raiser::due`].
    pub fn tick(&mut self, dt: F, cards: &[Entry]) {
        if self.up.armed || self.down.armed {
            for e in cards {
                let c = &e.card.lap;
                self.up.passed(c.to_line_kept, c.to_line);
                self.down.passed(c.to_line_kept, c.to_line);
            }
        }
        if let Some(d) = self.down.run(dt) {
            self.due.push((false, d));
        }
        if let Some(d) = self.up.run(dt) {
            self.due.push((true, d));
        }
    }

    /// What the tick kept, done to `props`: each body of the group placed `share` of the way from
    /// where it is put back to (its place's height) to 50 ft over or under it, or at the end of the
    /// way, which is then where it is put back to. A body the set does not hold is passed over.
    pub fn place(&mut self, props: &mut PropSet) {
        for (rises, due) in std::mem::take(&mut self.due) {
            let g = if rises { &self.up } else { &self.down };
            for &(c, o) in &g.bodies {
                let Some(w) = props.collections.get_mut(c as usize) else { continue };
                let o = o as usize;
                let Some(obj) = w.objects.get(o) else { continue };
                let from = obj.place.y();
                let to = if rises { fpu::add(from, LIFT.0) } else { fpu::sub(from, LIFT.0) };
                match due {
                    Due::Share(q) => {
                        let y = fpu::add(from, fpu::mul(fpu::sub(to, from), q.0));
                        w.place_at(o, F(y));
                    }
                    Due::End => {
                        w.place_at(o, F(to));
                        w.objects[o].place.0[1] = to;
                    }
                }
            }
        }
    }
}
