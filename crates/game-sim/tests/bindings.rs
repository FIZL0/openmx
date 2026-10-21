//! P73 — the original's binding table, against the original.
//!
//! 1. **The shipped table is the save states'.** The whole table read out of three riding save
//!    states' RAM (both pads, both bindings, and the two rows the game never fills) is
//!    [`Table::SHIPPED`], slot for slot.
//! 2. **The builder is the original's.** The original's own table builder, run in the interpreter on
//!    each of those states for both pads and all four presets, writes what [`Table::build`] writes.
//! 3. **The ride reads every command off its binding and nothing else.** Each of the sixteen controls
//!    held alone moves exactly the [`Controls`] fields of the commands the table binds it to; a
//!    command the table leaves unbound reads nothing with every control held.
//!
//! The first two need private RAM images and skip without them; the third is the port alone.

use game_sim::bindings::{COMMANDS, Command, PRESETS, Table};
use game_sim::input::{CONTROLS, Control, Manager, PADS, PlaneCommand, Report, mask_of};
use game_sim::ride::Controls;
use ps2_float::F;
use ps2_oracle::{Ee, Kit, addr, skip};

/// The riding states whose tables are read: the bike in both human recordings, and the biplane.
const STATES: [&str; 3] = [
    "first_recording.p2m2_SaveState.eeMemory.bin",
    "recording2.p2m2_SaveState.eeMemory.bin",
    "biplane.p2m2_SaveState.eeMemory.bin",
];

/// The application's input manager, and the table's pointer in it (research/oracle.txt,
/// `binding_table_build`).
const MANAGER: u32 = 0x104;
const TABLE: u32 = 0x648;
/// The table's rows, a 4-byte control index per command, -1 unbound.
mod row {
    /// The keyboard-like device's row, never filled by the shipped game.
    pub const KEYS: u32 = 0x000;
    pub const FIRST: u32 = 0x138;
    pub const SECOND: u32 = 0x3a8;
    /// The other kind's row, never filled either.
    pub const OTHER: u32 = 0x618;
    /// One pad's row.
    pub const PAD: u32 = 0x138;
}
const TABLE_SIZE: u32 = 0x750;

fn states() -> Vec<(&'static str, Ee)> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for name in STATES {
        let path = kit.ctx("states").join(name);
        if !path.exists() {
            skip(&format!("{} is not there: `./kit state ram` of its .p2s", path.display()));
            continue;
        }
        let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
        ee.set(28, u64::from(addr("boot_gp").unwrap()));
        out.push((name, ee));
    }
    out
}

fn table_at(ee: &Ee) -> u32 {
    let app = ee.u32_at(addr("app_pointer").unwrap());
    ee.u32_at(ee.u32_at(app + MANAGER) + TABLE)
}

/// One slot as the port reads it: a control index, or `None` for -1.
fn slot(ee: &Ee, at: u32) -> Option<Control> {
    match ee.i32_at(at) {
        -1 => None,
        i => Some(Control::from_index(i as usize).unwrap_or_else(|| panic!("slot {at:#x} holds {i}"))),
    }
}

/// A table out of RAM: both pads' first and second rows.
fn read(ee: &Ee, map: u32) -> Table {
    let mut t = Table::UNBOUND;
    for pad in 0..PADS {
        for c in 0..COMMANDS {
            let at = |base: u32| map + base + pad as u32 * row::PAD + c as u32 * 4;
            t.first[pad][c] = slot(ee, at(row::FIRST));
            t.second[pad][c] = slot(ee, at(row::SECOND));
        }
    }
    t
}

/// Every slot of the table a riding save state holds is the shipped table's, and the two rows the
/// port leaves out are empty there.
#[test]
fn the_shipped_table_is_the_save_states() {
    let states = states();
    for (name, ee) in &states {
        let map = table_at(ee);
        assert_eq!(read(ee, map), Table::SHIPPED, "{name}");
        for base in [row::KEYS, row::OTHER] {
            for c in 0..COMMANDS as u32 {
                assert_eq!(ee.i32_at(map + base + c * 4), -1, "{name}: row {base:#x} command {c:#x}");
            }
        }
    }
    println!("P73: {} save states' tables are the shipped one", states.len());
}

/// The original's builder, called on each state for both pads and every preset over a table that is
/// as allocated (every slot -1), writes [`Table::build`]'s table — the presets this port carries are
/// the disc's, not only preset 0 the states hold.
#[test]
fn the_builder_is_the_originals() {
    let states = states();
    let build = addr("binding_table_build").unwrap();
    let mut built = 0;
    for (name, mut ee) in states {
        let map = table_at(&ee);
        let app = ee.u32_at(addr("app_pointer").unwrap());
        for pad in 0..PADS {
            for preset in 0..PRESETS {
                for k in 0..TABLE_SIZE / 4 {
                    ee.set_u32(map + k * 4, u32::MAX);
                }
                ee.call(build, &[u64::from(app), pad as u64, preset as u64], &[], 100_000);
                // It writes the table and its own stack frame, and nothing else.
                let stack = ee.stack;
                for (from, to) in ee.written_ranges() {
                    let in_table = from >= map && to <= map + TABLE_SIZE;
                    let in_stack = from >= stack - 0x1000 && to <= stack + 0x100;
                    assert!(in_table || in_stack, "{name}: wrote {from:#x}..{to:#x}");
                }
                let want = Table::UNBOUND.build(pad, preset);
                assert_eq!(read(&ee, map), want, "{name}: pad {pad} preset {preset}");
                for base in [row::KEYS, row::OTHER] {
                    for c in 0..COMMANDS as u32 {
                        assert_eq!(ee.i32_at(map + base + c * 4), -1, "{name}: row {base:#x} command {c:#x}");
                    }
                }
                built += 1;
            }
        }
    }
    println!("P73: the original's builder matched {built} times");
}

/// One frame up and one down of `held` on pad 0, as the poll sees it.
fn pad_with(held: &[Control]) -> Manager {
    let mut m = Manager::connected();
    m.tick(F(1.0 / 60.0), &[Report::from_buttons(0); PADS]);
    m.tick(F(1.0 / 60.0), &[Report::from_buttons(mask_of(held)); PADS]);
    m
}

/// Every command reads the control its first binding names, as a value, a hold and a press; a
/// command the table leaves unbound reads nothing with all sixteen held.
#[test]
fn every_command_reads_its_binding_and_an_unbound_one_reads_nothing() {
    let t = &Table::SHIPPED;
    for control in Control::ALL {
        let m = pad_with(&[control]);
        for pad in [m.pad(0), m.pad(1)] {
            for c in 0..COMMANDS {
                let command = Command(c as u8);
                let on = t.control(pad.index as usize, command) == Some(control);
                assert_eq!(t.held(pad, command), on, "{command:?} with {control:?}");
                assert_eq!(t.pressed(pad, command), on, "{command:?} with {control:?}");
                assert_eq!(t.value(pad, command), if on { F::ONE } else { F::ZERO }, "{command:?}");
            }
        }
    }
    let all = pad_with(&Control::ALL);
    let unbound: Vec<usize> = (0..COMMANDS).filter(|&c| t.first[0][c].is_none()).collect();
    assert_eq!(unbound.len(), COMMANDS - 60, "{unbound:x?}");
    for c in unbound {
        let command = Command(c as u8);
        assert!(!t.held(all.pad(0), command) && !t.pressed(all.pad(0), command), "{command:?}");
        assert_eq!(t.value(all.pad(0), command), F::ZERO, "{command:?}");
    }
}

/// What the ride makes of each control held alone, under the shipped table: the commands it is
/// the binding of and nothing else. L2, R2, Select, Start, L3 and R3 are on no command the bike
/// reads, so they move nothing — there is no front brake, no reverse and no balance button.
#[test]
fn the_ride_reads_each_command_off_the_control_the_table_names() {
    let rest = format!("{:?}", Controls::default());
    let at_rest = Controls::from_pad(Manager::connected().pad(0));
    assert_eq!(format!("{at_rest:?}"), rest, "an idle pad asks for nothing");
    for control in Control::ALL {
        let c = Controls::from_pad(pad_with(&[control]).pad(0));
        let mut want = Controls::default();
        match control {
            Control::Cross => want.gas = F::ONE,
            Control::Square => want.brake = F::ONE,
            Control::L1 => want.clutch = F::ONE,
            Control::Circle => {
                want.trick[0] = true;
                want.stunt = true;
            }
            Control::Triangle => want.trick[1] = true,
            Control::R1 => want.trick[2] = true,
            Control::Left => {
                want.dpad[0] = true;
                want.dpad_dir = 7;
            }
            Control::Right => {
                want.dpad[1] = true;
                want.dpad_dir = 3;
            }
            Control::Up => {
                want.dpad[2] = true;
                want.dpad_y = true;
                want.dpad_dir = 1;
            }
            Control::Down => {
                want.dpad[3] = true;
                want.dpad_y = true;
                want.dpad_dir = 5;
            }
            Control::L2 | Control::R2 | Control::Select | Control::Start | Control::L3 | Control::R3 => {}
        }
        assert_eq!(format!("{c:?}"), format!("{want:?}"), "{control:?}");
    }
    // The table names these, and they are the controls just tested.
    let t = &Table::SHIPPED;
    for (command, control) in [
        (Command::GAS, Control::Cross),
        (Command::BRAKE, Control::Square),
        (Command::CLUTCH, Control::L1),
        (Command::TRICK_1, Control::Circle),
        (Command::TRICK_2, Control::Triangle),
        (Command::TRICK_3, Control::R1),
        (Command::LEFT, Control::Left),
        (Command::RIGHT, Control::Right),
        (Command::UP, Control::Up),
        (Command::DOWN, Control::Down),
        (Command::RESET_LEFT, Control::L1),
        (Command::RESET_RIGHT, Control::R1),
        (Command::NEXT_CAMERA, Control::Select),
        (Command::PAUSE, Control::Start),
    ] {
        assert_eq!(t.control(0, command), Some(control), "{command:?}");
    }
    assert_eq!(Command::RIDE.len(), 14);
    // A table that binds nothing leaves the ride at rest whatever is held.
    let all = pad_with(&Control::ALL);
    assert_eq!(format!("{:?}", Controls::through(all.pad(0), &Table::UNBOUND)), rest);
    assert_eq!(CONTROLS, Control::ALL.len());
}

/// The biplane's eight commands, which it reads through the table too, are where the table has them.
#[test]
fn the_planes_commands_are_the_tables() {
    for c in PlaneCommand::ALL {
        assert_eq!(Table::SHIPPED.control(0, Command(c as u8)), Some(c.control()), "{c:?}");
        assert_eq!(Table::SHIPPED.control(1, Command(c as u8)), Some(c.control()), "{c:?}");
    }
}
