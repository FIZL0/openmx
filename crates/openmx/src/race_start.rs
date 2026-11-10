//! P327: a race's start as the shell lays it out from what `game_sim::race` has — the start gate off
//! the scene's drop-gate prop ([`Supercross::place_drop_gate`], S13e), the activity's reset with the
//! track's start calls (S13a), the grid slot's lanes and spot for the human's card, and the closed
//! gate with its countdown to the drop that `Sim::race` steps (S13b).
//!
//! The port's own where the original's code was not ported (S297's prop lookup and bound): the prop is
//! the first scene model whose name has `dropgate` in it, as the original looks its props up by that
//! name, and its bound is the box round the model's finest triangles. The start hold (S290, S304) and
//! the intro are not run: the gate's watch is what keeps a rider behind it until the drop.

use game_render::scene::Placement;
use game_render::V3;
use game_sim::race::{self, GridStart, Prop, Race, Supercross};
use game_sim::rng::Rng;
use ps2_float::{F, Vf};

/// The scene prop the start gate stands on: its name has this in it (sx01's `THQsxdropgate`, the
/// nationals' `Nat15_Dropgate01`).
pub const DROP_GATE: &str = "dropgate";

/// What a race's start puts the human at, and the gate that holds them.
#[derive(Clone, Debug)]
pub struct Start {
    /// The grid spot before the ground snaps it, and the way it faces.
    pub spot: Vf,
    pub facing: Vf,
    /// The human's lane and starting place (from 1), of [`Start::riders`].
    pub lane: i32,
    pub place: i32,
    pub riders: i32,
    /// The start gate, closed, and the seconds from the reset to its drop.
    pub gate: race::Gate,
    pub delay: F,
    /// The race's own draws, after the reset's and the lanes'.
    pub rng: Rng,
}

impl Start {
    /// The race `Sim::race` steps, with the rider standing at `rider` (its bike's translation and
    /// forward row).
    pub fn race(&self, rider: (Vf, Vf)) -> Race {
        Race::new(self.gate, &[rider], self.delay, self.rng.clone())
    }
}

/// The drop-gate prop off the scene: the first placement named [`DROP_GATE`], its frame's across row
/// (`up × look`), its look and its place, and half its model's box across and up (`bound`, the
/// model's box as a centre and half extents).
pub fn drop_prop(models: &[Placement], bound: impl Fn(&str) -> Option<(V3, V3)>) -> Option<Prop> {
    let p = models.iter().find(|p| p.model.to_ascii_lowercase().contains(DROP_GATE))?;
    let (_, half) = bound(&p.model)?;
    let (u, l) = (p.up, p.look);
    let across = V3 { x: u.y * l.z - u.z * l.y, y: u.z * l.x - u.x * l.z, z: u.x * l.y - u.y * l.x };
    Some(Prop {
        across: Vf::new(across.x, across.y, across.z, 0.0),
        look: Vf::new(l.x, l.y, l.z, 0.0),
        at: Vf::new(p.pos.x, p.pos.y, p.pos.z, 1.0),
        half_width: F(half.x),
        height: F(half.y),
    })
}

/// The start calls' gate-drop times out of the race events' `event.aud`: the `SoundNGateDropTime`
/// keys of the `[RaceStart<class>]` section for the bike's class (the digits its name starts with,
/// `125sx` → 125), or of the first `RaceStart` section when there is none for it.
pub fn drop_times(event_aud: &str, bike: &str) -> Vec<F> {
    let class: String = bike.chars().take_while(|c| c.is_ascii_digit()).collect();
    let mut sections: Vec<(String, Vec<F>)> = Vec::new();
    for line in event_aud.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            sections.push((name.to_string(), Vec::new()));
            continue;
        }
        let (Some((key, value)), Some((_, times))) = (line.split_once('='), sections.last_mut()) else { continue };
        let key = key.trim();
        if key.starts_with("Sound")
            && key.ends_with("GateDropTime")
            && let Ok(t) = value.trim().parse::<f32>()
        {
            times.push(F(t));
        }
    }
    let starts = || sections.iter().filter(|(n, t)| n.starts_with("RaceStart") && !t.is_empty());
    let mine = starts().find(|(n, _)| n.eq_ignore_ascii_case(&format!("RaceStart{class}")));
    mine.or_else(|| starts().next()).map(|(_, t)| t.clone()).unwrap_or_default()
}

/// A race's start for `riders` riders off the drop-gate prop and the start calls' drop times: the
/// activity reset (the intro counted as seen, so it is skipped), the lanes shuffled with every rider
/// a player's, and card 0's grid spot. `None` without a prop or a start call.
pub fn lay_out(prop: &Prop, drops: Vec<F>, riders: i32, seed: u32) -> Option<Start> {
    if drops.is_empty() {
        return None;
    }
    let riders = riders.clamp(1, race::LANES as i32);
    let mut start = race::Gate::new();
    Supercross::place_drop_gate(prop, None, &mut start);
    let mut s = Supercross::new();
    s.lane_offsets = Supercross::lane_table();
    s.drop_times = drops;
    s.intro_seen = 1;
    let mut rng = Rng::from_seed(seed);
    s.reset(&mut start, &mut rng);
    s.shuffle_lanes(riders as usize, false, &mut rng);
    let (spot, facing) = s.grid_spot(GridStart::Gate { centre: start.centre, normal: start.normal, width: start.width }, 0, riders);
    Some(Start { spot, facing, lane: s.lanes[0], place: s.starting_place(0), riders, gate: start, delay: s.start_delay, rng })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bikes_class_picks_its_start_calls() {
        let aud = "[RaceStart50]\nSound1GateDropTime=1.9\nSound2GateDropTime=2.1\n[RaceStart125]\nSound1 = x\nSound1GateDropTime = 1.54\n[Other]\nSound1GateDropTime=9\n";
        assert_eq!(drop_times(aud, "125sx"), vec![F(1.54)]);
        assert_eq!(drop_times(aud, "50cc"), vec![F(1.9), F(2.1)]);
        assert_eq!(drop_times(aud, "250f"), vec![F(1.9), F(2.1)]);
        assert!(drop_times("", "125sx").is_empty());
    }

    #[test]
    fn the_sx01_prop_stands_the_gate_where_the_image_has_it() {
        // sx01's `THQsxdropgate` placement, and its box's half extents as S196's image holds them.
        let p = Placement {
            model: "THQsxdropgate".into(),
            pos: V3 { x: 101.493164, y: 90.12307, z: 458.47986 },
            look: V3 { x: 0.0, y: 0.5, z: 0.866025 },
            up: V3 { x: 0.0, y: 0.866025, z: -0.5 },
        };
        let prop = drop_prop(&[p], |_| Some((V3 { x: 0.0, y: 0.0, z: 0.0 }, V3 { x: 59.273155, y: 1.3735971, z: 1.0 }))).unwrap();
        let s = lay_out(&prop, vec![F(1.54)], 1, 1).unwrap();
        let want = [101.49316, 87.87306, 454.5827];
        for (got, want) in [s.gate.centre.x(), s.gate.centre.y(), s.gate.centre.z()].into_iter().zip(want) {
            assert!((got - want).abs() < 1e-3, "{:?}", s.gate.centre);
        }
        assert!((s.gate.across.x() - 1.0).abs() < 1e-6);
        assert!(s.gate.flags & race::flags::CLOSED != 0);
        assert_eq!(s.delay, F(1.5) + F(1.54));
        assert_eq!((s.lane, s.place), (0, 1));
        // Behind the gate, facing through it.
        assert!(s.spot.z() < s.gate.centre.z() && s.facing.z() > 0.9);
    }
}
