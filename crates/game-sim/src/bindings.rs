//! The original's binding table (P73): which control each of the game's commands is on.
//!
//! The original never asks a pad for a button. Everything that reads the player — the bike, the
//! biplane, the cameras, the front end's screens — asks for a *command*, a number from 0 to 0x4d,
//! and the input manager's binding table says which of the pad's sixteen controls that command is
//! this frame. There are two of them for each pad, a first binding and a second, and two rows the
//! shipped game never fills (a keyboard-like device's and one more kind), which the port leaves out.
//!
//! The table is built by one routine, from a per-player *preset*: eight commands move between four
//! layouts, and the rest are the same in every one. A player's preset comes from the profile (the
//! front end's Controller Presets screen sets it); on any vehicle but the bike the game builds preset
//! 0 whatever the profile says. Three save states (the human's first recording, recording2 and the
//! biplane's) all hold preset 0 on both pads, with no second binding anywhere, which is
//! [`Table::SHIPPED`].
//!
//! Reading a command the original's two ways:
//!
//! - its **value** ([`Table::value`]): the strength of the control on its *first* binding — a
//!   pressure for an analogue control, 1 for a digital one — and 0 when it has none. The bike, the
//!   biplane and the camera's modifiers read this, and "held" is that value not being 0;
//! - its **event** ([`Table::pressed`]): a press of the control on either binding. The pause, the
//!   camera buttons and the reset chord are handlers on the event queue that test this.
//!
//! A command the table leaves unbound reads nothing either way, whatever is pressed.

use ps2_float::F;

use crate::input::{Control, Pad, PADS};

/// How many commands the table has a slot for.
pub const COMMANDS: usize = 0x4e;

/// One of the table's commands, by its number in the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Command(pub u8);

impl Command {
    /// The d-pad's four, as the bike reads them: a digital stick (left and right lean and steer, up
    /// and down move the rider fore and aft) and the direction half of a trick's gesture. The
    /// biplane reads the same four as bank left, bank right, dive and climb.
    pub const LEFT: Command = Command(0x00);
    pub const RIGHT: Command = Command(0x01);
    pub const UP: Command = Command(0x02);
    pub const DOWN: Command = Command(0x03);
    /// The pause, an event.
    pub const PAUSE: Command = Command(0x04);
    /// The throttle. On the bike a switch: any value but 0 is the whole lever.
    pub const GAS: Command = Command(0x0e);
    /// The brake, both wheels. A switch on the bike, like the gas.
    pub const BRAKE: Command = Command(0x0f);
    /// The clutch lever, a switch too.
    pub const CLUTCH: Command = Command(0x10);
    /// The three trick buttons, in the order the trick decoder reads them.
    pub const TRICK_1: Command = Command(0x11);
    pub const TRICK_2: Command = Command(0x12);
    pub const TRICK_3: Command = Command(0x13);
    /// The biplane's rudder.
    pub const RUDDER_LEFT: Command = Command(0x14);
    pub const RUDDER_RIGHT: Command = Command(0x15);
    /// Held, the right stick moves the follow camera in and out instead of round.
    pub const CAMERA_ZOOM: Command = Command(0x1e);
    /// Held, the right stick widens and narrows the follow camera's view instead.
    pub const CAMERA_WIDE: Command = Command(0x1f);
    /// Pressed, the follow camera turns to look behind the rider until it is let go.
    pub const LOOK_BACK: Command = Command(0x20);
    /// Pressed with neither camera command above held, the next follow camera preset.
    pub const NEXT_CAMERA: Command = Command(0x21);
    /// Pressed while moving, the other camera until it is let go.
    pub const OTHER_CAMERA: Command = Command(0x22);
    /// The reset chord's two halves: either pressed with the other held puts the rider back.
    pub const RESET_LEFT: Command = Command(0x4c);
    pub const RESET_RIGHT: Command = Command(0x4d);

    /// The commands a rider on the bike has. Every one is read in a ride by the bike or the shell; the camera commands the shell does not follow
    /// yet (look back, the other camera) are not here, and nor are the right stick's two modifiers,
    /// which the shell's camera reads beside the stick (P114) rather than the ride.
    pub const RIDE: [Command; 14] = [
        Command::GAS,
        Command::BRAKE,
        Command::CLUTCH,
        Command::TRICK_1,
        Command::TRICK_2,
        Command::TRICK_3,
        Command::UP,
        Command::LEFT,
        Command::DOWN,
        Command::RIGHT,
        Command::NEXT_CAMERA,
        Command::RESET_LEFT,
        Command::RESET_RIGHT,
        Command::PAUSE,
    ];

    /// Its slot in a row of the table.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The name the project gives it, for a command whose readers have been read; `None` for the
    /// rest (other vehicles', the free camera's and the front end's screens').
    pub const fn name(self) -> Option<&'static str> {
        Some(match self {
            Command::LEFT => "left",
            Command::RIGHT => "right",
            Command::UP => "up",
            Command::DOWN => "down",
            Command::PAUSE => "pause",
            Command::GAS => "gas",
            Command::BRAKE => "brake",
            Command::CLUTCH => "clutch",
            Command::TRICK_1 => "trick 1",
            Command::TRICK_2 => "trick 2",
            Command::TRICK_3 => "trick 3",
            Command::RUDDER_LEFT => "rudder left",
            Command::RUDDER_RIGHT => "rudder right",
            Command::CAMERA_ZOOM => "camera zoom",
            Command::CAMERA_WIDE => "camera wide",
            Command::LOOK_BACK => "look back",
            Command::NEXT_CAMERA => "camera",
            Command::OTHER_CAMERA => "other camera",
            Command::RESET_LEFT => "reset (with R1)",
            Command::RESET_RIGHT => "reset (with L1)",
            _ => return None,
        })
    }
}

/// How many presets the builder knows.
pub const PRESETS: usize = 4;

/// The eight commands a preset moves, in the order the builder sets them.
pub const PRESET_COMMANDS: [Command; 8] = [
    Command::LOOK_BACK,
    Command::OTHER_CAMERA,
    Command::GAS,
    Command::BRAKE,
    Command::CLUTCH,
    Command::TRICK_1,
    Command::TRICK_2,
    Command::TRICK_3,
];

/// Each preset's controls for [`PRESET_COMMANDS`], preset 0 first. Preset 0 is the shipped one.
pub const PRESET_CONTROLS: [[Control; 8]; PRESETS] = {
    use Control::*;
    [
        [L2, R2, Cross, Square, L1, Circle, Triangle, R1],
        [L2, R2, Square, Cross, L1, Circle, Triangle, R1],
        [L2, R1, Cross, Square, R2, Circle, Triangle, L1],
        [L1, R1, R2, Square, L2, Circle, Triangle, Cross],
    ]
};

/// The commands every preset binds alike, in the order the builder sets them: both pads' first
/// binding each.
pub const FIXED: [(Command, Control); 52] = {
    use Control::*;
    const fn c(n: u8, control: Control) -> (Command, Control) {
        (Command(n), control)
    }
    [
        c(0x00, Left),
        c(0x01, Right),
        c(0x02, Up),
        c(0x03, Down),
        c(0x04, Start),
        c(0x05, Select),
        c(0x06, L2),
        c(0x07, R3),
        c(0x08, R2),
        c(0x09, Up),
        c(0x0a, Down),
        c(0x0b, Left),
        c(0x0c, Right),
        c(0x0d, L3),
        c(0x14, L1),
        c(0x15, R1),
        c(0x16, Circle),
        c(0x17, L1),
        c(0x18, R1),
        c(0x19, Cross),
        c(0x1a, Cross),
        c(0x1b, Square),
        c(0x1c, L1),
        c(0x1d, R1),
        c(0x21, Select),
        c(0x2e, Select),
        c(0x25, Circle),
        c(0x26, Triangle),
        c(0x27, L1),
        c(0x28, R1),
        c(0x29, L2),
        c(0x2a, R2),
        c(0x2b, Cross),
        c(0x2d, L3),
        c(0x30, Square),
        c(0x1e, L3),
        c(0x1f, R1),
        c(0x23, R3),
        c(0x32, Left),
        c(0x33, Right),
        c(0x34, Up),
        c(0x35, Down),
        c(0x36, Cross),
        c(0x3a, Square),
        c(0x38, Triangle),
        c(0x37, Circle),
        c(0x39, Start),
        c(0x3e, R1),
        c(0x3f, L1),
        c(0x40, L2),
        c(0x4c, L1),
        c(0x4d, R1),
    ]
};

/// The binding table: per pad, a first and a second control for each command, `None` unbound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Table {
    pub first: [[Option<Control>; COMMANDS]; PADS],
    pub second: [[Option<Control>; COMMANDS]; PADS],
}

impl Table {
    /// The table as it is allocated, before the builder: nothing bound.
    pub const UNBOUND: Table = Table { first: [[None; COMMANDS]; PADS], second: [[None; COMMANDS]; PADS] };

    /// The shipped game's: preset 0 on both pads.
    pub const SHIPPED: Table = Table::built([0; PADS]);

    /// A fresh table with each pad built on its preset, pad 0 first, as the game builds one when a
    /// ride starts.
    pub const fn built(presets: [usize; PADS]) -> Table {
        let mut t = Table::UNBOUND;
        let mut pad = 0;
        while pad < PADS {
            t = t.build(pad, presets[pad]);
            pad += 1;
        }
        t
    }

    /// The builder, for one pad: the preset's eight commands on that pad's first binding, then
    /// [`FIXED`] on both pads' first binding. Nothing is cleared and no second binding is touched.
    pub const fn build(mut self, pad: usize, preset: usize) -> Table {
        let mut i = 0;
        while i < PRESET_COMMANDS.len() {
            self.first[pad][PRESET_COMMANDS[i].index()] = Some(PRESET_CONTROLS[preset][i]);
            i += 1;
        }
        let mut i = 0;
        while i < FIXED.len() {
            let (command, control) = FIXED[i];
            let mut p = 0;
            while p < PADS {
                self.first[p][command.index()] = Some(control);
                p += 1;
            }
            i += 1;
        }
        self
    }

    /// The row a pad reads: its own, and the second pad's for any pad past it — the original has
    /// two, the port's extra seats ride the second's (which is the first's in every shipped table).
    fn row(pad: &Pad) -> usize {
        (pad.index as usize).min(PADS - 1)
    }

    /// The control on a command's first binding for a pad, `None` unbound.
    pub fn control(&self, pad: usize, command: Command) -> Option<Control> {
        self.first[pad.min(PADS - 1)][command.index()]
    }

    /// A command's value: its first binding's strength, 0 unbound.
    pub fn value(&self, pad: &Pad, command: Command) -> F {
        self.first[Table::row(pad)][command.index()].map_or(F::ZERO, |c| pad.strength(c))
    }

    /// A command is held: its first binding's control is down. The original's test is the value not
    /// being 0, which is the same thing — a released control's strength is 0, a held one's is not.
    pub fn held(&self, pad: &Pad, command: Command) -> bool {
        self.first[Table::row(pad)][command.index()].is_some_and(|c| pad.held(c))
    }

    /// A command went down this frame, on either binding.
    pub fn pressed(&self, pad: &Pad, command: Command) -> bool {
        let row = Table::row(pad);
        [self.first[row][command.index()], self.second[row][command.index()]]
            .into_iter()
            .flatten()
            .any(|c| pad.pressed(c))
    }

    /// The commands a control is a binding of for a pad, first or second, in command order.
    pub fn commands_on(&self, pad: usize, control: Control) -> Vec<Command> {
        let row = pad.min(PADS - 1);
        (0..COMMANDS)
            .filter(|&i| self.first[row][i] == Some(control) || self.second[row][i] == Some(control))
            .map(|i| Command(i as u8))
            .collect()
    }
}
