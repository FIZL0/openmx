//! P79 — a ride on one of the original's other controller presets.
//!
//! P73 proved the builder ([`Table::build`]) against the original's for all four presets; this is
//! that the simulation reads its pads through the table its presets build, and nothing else:
//!
//! 1. **The poll reads the preset's table.** [`Sim::poll`] and [`Sim::poll_all`] on every preset
//!    hand out exactly [`Controls::through`] the table [`Table::built`] makes, for each control held
//!    alone; on preset 0 that is [`Controls::from_pad`], the shipped table, as before there was a
//!    choice.
//! 2. **The presets move what the original's move.** Preset 1's gas is Square, preset 2's clutch is
//!    R2 and its third trick L1, preset 3's gas R2 — and the fixed commands (the reset chord on
//!    L1 + R1, the d-pad, the pause) stay where every preset has them.
//! 3. **A plane reads preset 0**, as the original builds preset 0 for any vehicle but the bike.
//! 4. **The table is a constant of the ride**: a snapshot copies it, a resync keeps the local one,
//!    and the hash does not walk it.

use game_sim::Sim;
use game_sim::bindings::{PRESET_COMMANDS, PRESET_CONTROLS, PRESETS, Table};
use game_sim::flight::Flight;
use game_sim::input::{Control, PADS, Report, mask_of};
use game_sim::phy::Plane;
use game_sim::ride::Controls;
use game_sim::snapshot;
use ps2_float::F;

const DT: F = F(1.0 / 60.0);

/// A fresh state on `preset` for pad 0, polled one frame idle and one frame with `held` on both
/// pads.
fn polled(preset: usize, held: &[Control]) -> (Sim, Controls) {
    let mut sim = Sim::new(0);
    sim.input.pad_mut(1).connected = true;
    sim.set_presets([preset, 0]);
    sim.poll(DT, &[Report::from_buttons(0); PADS]);
    let c = sim.poll(DT, &[Report::from_buttons(mask_of(held)); PADS]);
    (sim, c)
}

#[test]
fn the_poll_reads_the_presets_table() {
    for preset in 0..PRESETS {
        let table = Table::built([preset, 0]);
        for control in Control::ALL {
            let (sim, c) = polled(preset, &[control]);
            assert_eq!(*sim.table(), table, "preset {preset}");
            let want = Controls::through(sim.input.pad(0), &table);
            assert_eq!(format!("{c:?}"), format!("{want:?}"), "preset {preset}, {control:?} held");
            if preset == 0 {
                let shipped = Controls::from_pad(sim.input.pad(0));
                assert_eq!(format!("{c:?}"), format!("{shipped:?}"), "preset 0 is the shipped table");
            }
            // With a second player, each reads their own pad's row: pad 0's the preset's, pad 1's
            // preset 0's.
            let mut both = Sim::new(0);
            both.input.pad_mut(1).connected = true;
            both.set_presets([preset, 0]);
            both.add_player((*both.phy).clone(), both.rider.clone());
            both.poll_all(DT, &[Report::from_buttons(0); PADS]);
            let all = both.poll_all(DT, &[Report::from_buttons(mask_of(&[control])); PADS]);
            assert_eq!(format!("{:?}", all[0]), format!("{want:?}"), "player 1 on preset {preset}");
            let other = Controls::through(both.input.pad(1), &Table::SHIPPED);
            assert_eq!(format!("{:?}", all[1]), format!("{other:?}"), "pad 1 under pad 0's preset {preset}");
        }
    }
    assert_eq!(Table::built([0, 0]), Table::SHIPPED);
}

#[test]
fn the_presets_move_what_the_originals_move() {
    let gas = |preset: usize, control: Control| polled(preset, &[control]).1.gas != F::ZERO;
    let clutch = |preset: usize, control: Control| polled(preset, &[control]).1.clutch != F::ZERO;
    let trick = |preset: usize, control: Control| polled(preset, &[control]).1.trick;
    assert!(gas(0, Control::Cross) && !gas(0, Control::Square));
    assert!(gas(1, Control::Square) && !gas(1, Control::Cross));
    assert!(polled(1, &[Control::Cross]).1.brake != F::ZERO);
    assert!(clutch(2, Control::R2) && !clutch(2, Control::L1));
    assert_eq!(trick(2, Control::L1), [false, false, true]);
    assert_eq!(trick(2, Control::R1), [false, false, false]);
    assert!(gas(3, Control::R2) && clutch(3, Control::L2));
    assert_eq!(trick(3, Control::Cross), [false, false, true]);
    // The eight commands each preset moves are on the controls the table says.
    for preset in 0..PRESETS {
        let t = Table::built([preset, 0]);
        for (command, control) in PRESET_COMMANDS.iter().zip(PRESET_CONTROLS[preset]) {
            assert_eq!(t.control(0, *command), Some(control), "preset {preset}");
        }
    }
    // The fixed ones: the reset chord is L1 + R1 on every preset, whatever else L1 and R1 are.
    for preset in 0..PRESETS {
        let mut sim = Sim::new(0);
        sim.set_presets([preset, 0]);
        sim.poll(DT, &[Report::from_buttons(0); PADS]);
        sim.poll(DT, &[Report::from_buttons(mask_of(&[Control::L1])); PADS]);
        let c = sim.poll(DT, &[Report::from_buttons(mask_of(&[Control::L1, Control::R1])); PADS]);
        assert!(c.respawn, "preset {preset}: L1 then R1 is the reset chord");
        let (_, c) = polled(preset, &[Control::Left]);
        assert_eq!(c.dpad, [true, false, false, false], "preset {preset}: the d-pad is fixed");
    }
}

#[test]
fn a_plane_reads_preset_0() {
    let mut sim = Sim::new(0);
    sim.set_presets([3, 0]);
    assert_eq!(*sim.table(), Table::built([3, 0]));
    sim.plane = Some(Flight::new(&Plane::read_bytes(b"")));
    assert_eq!(*sim.table(), Table::SHIPPED);
}

#[test]
fn the_table_is_a_constant_of_the_ride() {
    let (sim, _) = polled(2, &[Control::R2]);
    let copy = snapshot::save(&sim);
    assert_eq!(copy.sim().bindings, sim.bindings, "a snapshot copies it");
    let mut shipped = sim.clone();
    shipped.bindings = Table::SHIPPED;
    assert_eq!(shipped.hash(), sim.hash(), "the hash does not walk it");
    let mut other = Sim::new(0);
    other.set_presets([1, 0]);
    snapshot::decode(&mut other, &snapshot::encode(&sim)).unwrap();
    assert_eq!(other.bindings, Table::built([1, 0]), "a resync keeps the local table");
}
