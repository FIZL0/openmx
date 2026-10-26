//! The host's pad, as a DualShock 2 report.
//!
//! The port's input layer ([`game_sim::input`]) is the original's: it reads a 32-byte pad report — the
//! bytes the PS2's pad library left in RAM — and nothing else. So this module's whole job is to produce
//! those bytes from whatever the person at the keyboard is holding, and then get out of the way: the
//! dead zone, the pressure threshold, the press and release timers and the event queue are all I1's and
//! I2's, measured from the game, and none of them is re-decided here.
//!
//! Two sources, folded into one report:
//!
//! * **A pad**, read as a `/dev/input/js*` character device — the kernel's joystick interface, which
//!   hands over eight-byte events and normalises every axis to ±32767, so no `ioctl` and therefore no
//!   unsafe code is needed to know a stick's range. Buttons come back as indices rather than names,
//!   and the order is the one Linux gives a modern gamepad (south, east, west, north, then the two
//!   shoulders, then select/start); see [`BUTTONS`].
//! * **The keyboard**, through the window's key state, so the shell is playable on a laptop. A key is
//!   a digital control, which the game's own binding reads as a full pressure byte — exactly what
//!   [`game_sim::input::Report::from_buttons`] does, and what the kit's virtual pad sends. The two
//!   sticks are where that stops being enough: a switch has no travel, so the keyboard's stick axes
//!   are ramped toward the edge while a key is held and let back to centre when it is not
//!   ([`KeyAxis`]), which is the difference between leaning and slamming.
//!
//! The throttle is the third source, and the reason this module has a [`Wiring`]. The original reads
//! Cross from its *pressure byte* and not from its bit (the game's own control map, read out of its
//! RAM — [`CONTROL_MAP`]), so on the disc the plane's throttle was never a switch: a DualShock 2's
//! face buttons travel, and part throttle is a fraction of that travel. The bike drops the fraction
//! and takes any press as the whole lever (P77, [`game_sim::ride::switch`]). Hardly any pad
//! sold since reports a face button as anything but one bit, so this module gives the rider three
//! ways to it, in the order of how much of it is the device's own: a per-button pressure where the
//! driver publishes one ([`Wiring::pressure`]), gas on a trigger axis where it does not
//! ([`Throttle::Trigger`]), and a ramp under the keyboard's throttle key ([`KeyRamp`]) for a host
//! with no travel anywhere.
//!
//! The report is built the way a DualShock 2 would have reported it, not the way the game sees it:
//! raw stick bytes, the button word active-low, and a pressure byte per analogue control. Whether an
//! axis is then inverted or dead-zoned is the port's business.

use game_sim::bindings::{Command, Table};
use game_sim::input::{AXIS_CENTRE, CONTROL_MAP, Control, PRESSURE_THRESHOLD, Report};

use crate::window::key;

/// Where the kernel puts its joystick devices, and where it says what each one is.
const PAD_DIR: &str = "/dev/input";
const PAD_SYS: &str = "/sys/class/input";

/// `O_NONBLOCK`, so a frame with no pad event does not wait for one. The value is Linux's, and this
/// is the one place the shell needs a number from a header rather than from std.
const O_NONBLOCK: i32 = 0o4000;

/// One joystick event: a millisecond stamp, a value, a type and which control it is.
const EVENT_BYTES: usize = 8;
/// The event types, with the "this is the initial state" bit masked off.
const EVENT_BUTTON: u8 = 0x01;
const EVENT_AXIS: u8 = 0x02;
const EVENT_INIT: u8 = 0x80;

/// How many of each the shell tracks.
///
/// Eight axes is a modern gamepad — two sticks, two triggers and a hat — and that is all the shell
/// reads by itself. A device that also reports how hard each of its buttons is pressed publishes
/// those pressures as further axes after them, which is what the room past eight is for: those axes
/// are read only where a [`Wiring`] says which button each one belongs to. A pad with more axes than
/// this says nothing the shell listens to.
const AXES: usize = 32;
const BUTTONS: usize = 12;

/// Which control each of the shell's own button slots is: the order the kernel gives an Xbox pad
/// (xpad) its buttons, which is what the bindings' names (`PadSouth`, `PadStart`, …) mean.
///
/// The kernel numbers a js device's buttons by keycode, among the keys *that device* has, so the
/// same number is a different button on another pad: a pad with no `BTN_SELECT` slides Start down
/// to 6, a PlayStation pad has its digital L2 and R2 at 6 and 7 and Options at 9, and a Switch Pro
/// Controller has Capture at 4 and + at 10 (P53, where + did nothing and ZL was Start). So a device
/// whose sysfs entry lists its keys has each js number turned into one of these slots by the code
/// behind it ([`JsMap`]), and only a node with no sysfs entry is read in this order as it comes.
const PAD_BUTTONS: [Option<Control>; BUTTONS] = [
    Some(Control::Cross),    // south
    Some(Control::Circle),   // east
    Some(Control::Square),   // west
    Some(Control::Triangle), // north
    Some(Control::L1),       // left shoulder
    Some(Control::R1),       // right shoulder
    Some(Control::Select),   // select (back)
    Some(Control::Start),
    None, // the guide button, which is the window manager's business and not the game's
    Some(Control::L3),
    Some(Control::R3),
    None,
];

/// The joystick axes the shell reads, in its own order (an Xbox pad's, and a PlayStation pad's): the
/// two sticks, the two triggers and the d-pad hat. A device with a sysfs entry has its axes put in
/// these slots by their codes ([`JsMap`]); a Switch Pro Controller has no trigger axes, so its right
/// stick was read as L2 and its d-pad as the right stick and R2 until P53.
const AXIS_LEFT_X: usize = 0;
const AXIS_LEFT_Y: usize = 1;
const AXIS_L2: usize = 2;
const AXIS_RIGHT_X: usize = 3;
const AXIS_RIGHT_Y: usize = 4;
const AXIS_R2: usize = 5;
const AXIS_HAT_X: usize = 6;
const AXIS_HAT_Y: usize = 7;

/// The shell's own axis slots, [`AXIS_LEFT_X`] to [`AXIS_HAT_Y`].
const PAD_AXES: usize = 8;

/// What one js button number of a device is to the shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JsButton {
    /// One of [`PAD_BUTTONS`]' slots, which the bindings name.
    Pad(usize),
    /// A trigger with no axis of its own (a Switch pad's ZL and ZR): its axis slot, all the way down
    /// while the button is.
    Trigger(usize),
    /// A d-pad direction sent as a button rather than as a hat (a left Joy-Con's).
    Dpad(Control),
    /// Nothing the game has: a Capture or a Home button, or a key past the gamepad's.
    Nothing,
}

/// How a device's js numbers land on the shell's slots (P53), built from the keys and axes its sysfs
/// entry lists: the joystick interface numbers them in code order among the ones the device has,
/// buttons from `BTN_JOYSTICK` up and then the `BTN_MISC` block, axes from `ABS_X` up.
#[derive(Clone, Debug, PartialEq, Eq)]
struct JsMap {
    button: Vec<JsButton>,
    /// The shell's axis slot for each js axis; `None` for one it does not read (an accelerometer, a
    /// pressure the [`Wiring`] reads by its raw number).
    axis: Vec<Option<usize>>,
}

/// The kernel's codes the map needs.
const BTN_MISC: u16 = 0x100;
const BTN_JOYSTICK: u16 = 0x120;
const KEY_MAX: u16 = 0x2ff;
const ABS_MAX: u16 = 0x3f;
const ABS_Z: u16 = 0x02;
const ABS_RZ: u16 = 0x05;
const ABS_GAS: u16 = 0x09;
const ABS_BRAKE: u16 = 0x0a;
const ABS_HAT0X: u16 = 0x10;
const ABS_HAT0Y: u16 = 0x11;

impl JsMap {
    /// A node with no sysfs entry: its numbers are taken as the shell's own, as before P53.
    fn identity() -> JsMap {
        JsMap {
            button: (0..BUTTONS).map(JsButton::Pad).collect(),
            axis: (0..PAD_AXES).map(Some).collect(),
        }
    }

    /// The map for a device with these keys and axes, js order. `positional` is a pad whose driver
    /// names its four face buttons by where they are (`BTN_NORTH` the top one, `BTN_WEST` the left
    /// one: hid-playstation, hid-nintendo); xpad, and every pad that pretends to be one, sends an
    /// Xbox pad's X — the left button — as `BTN_X`, which is the same code as `BTN_NORTH`. Either
    /// list empty (no sysfs) keeps that half the identity.
    fn of(keys: &[u16], abs: &[u16], positional: bool) -> JsMap {
        let mut map = JsMap::identity();
        let has_abs = |code: u16| abs.contains(&code);
        if !keys.is_empty() {
            map.button = keys
                .iter()
                .map(|&code| match code {
                    0x130 => JsButton::Pad(0),
                    0x131 => JsButton::Pad(1),
                    0x133 => JsButton::Pad(if positional { 3 } else { 2 }),
                    0x134 => JsButton::Pad(if positional { 2 } else { 3 }),
                    0x136 => JsButton::Pad(4),
                    0x137 => JsButton::Pad(5),
                    0x138 if !has_abs(ABS_Z) && !has_abs(ABS_BRAKE) => JsButton::Trigger(AXIS_L2),
                    0x139 if !has_abs(ABS_RZ) && !has_abs(ABS_GAS) => JsButton::Trigger(AXIS_R2),
                    0x13a => JsButton::Pad(6),
                    0x13b => JsButton::Pad(7),
                    0x13c => JsButton::Pad(8),
                    0x13d => JsButton::Pad(9),
                    0x13e => JsButton::Pad(10),
                    0x220 => JsButton::Dpad(Control::Up),
                    0x221 => JsButton::Dpad(Control::Down),
                    0x222 => JsButton::Dpad(Control::Left),
                    0x223 => JsButton::Dpad(Control::Right),
                    _ => JsButton::Nothing,
                })
                .collect();
        }
        if !abs.is_empty() {
            map.axis = abs
                .iter()
                .map(|&code| match code {
                    0 | 1 | 3 | 4 => Some(code as usize),
                    ABS_Z => Some(AXIS_L2),
                    ABS_RZ => Some(AXIS_R2),
                    ABS_BRAKE if !has_abs(ABS_Z) => Some(AXIS_L2),
                    ABS_GAS if !has_abs(ABS_RZ) => Some(AXIS_R2),
                    ABS_HAT0X => Some(AXIS_HAT_X),
                    ABS_HAT0Y => Some(AXIS_HAT_Y),
                    _ => None,
                })
                .collect();
        }
        map
    }

    fn button(&self, number: usize) -> JsButton {
        self.button.get(number).copied().unwrap_or(JsButton::Nothing)
    }

    fn axis(&self, number: usize) -> Option<usize> {
        self.axis.get(number).copied().flatten()
    }
}

/// The codes a sysfs capability bitmap lists, in the order the joystick interface numbers them.
fn js_order(bitmap: &str, codes: impl Iterator<Item = u16>) -> Vec<u16> {
    codes.filter(|&c| has_key(bitmap, c as usize)).collect()
}

/// How many presses of one control the shell keeps for the frames after a slow one (P53): a
/// double tap inside one frame is two presses, handed over one a frame.
const QUEUE_MAX: u8 = 4;

/// A control let go for less than this many milliseconds, by the events' own times, and down again
/// is the press it was, not another (P109): a d-pad contact bouncing would otherwise be two
/// presses in the queue, two rows of a menu for one push. A hand's quickest double tap leaves it up
/// for longer than this.
const BOUNCE_MS: u32 = 35;

/// Where each stick axis goes in the report: the report's own order is right X, right Y, left X,
/// left Y, which is not the pad's.
const REPORT_RIGHT_X: usize = 0;
const REPORT_RIGHT_Y: usize = 1;
const REPORT_LEFT_X: usize = 2;
const REPORT_LEFT_Y: usize = 3;

/// A trigger has to come this far out of its rest position to count at all, as a fraction of its
/// travel. A pad that rests its triggers at the middle of the range rather than at the bottom would
/// otherwise read as half-braking for ever.
const TRIGGER_DEAD: f32 = 0.12;

/// Where a trigger axis sits when the trigger is not pulled: the bottom of its travel.
const TRIGGER_REST: i16 = i16::MIN + 1;

/// The twelve pressure bytes of a DualShock 2 report, in the order the report holds them (bytes 8
/// to 19 — see [`CONTROL_MAP`], whose pressure indices were read out of the game's own RAM).
///
/// A driver that publishes a pad's per-button pressures as joystick axes publishes them in the order
/// the device sent them, which for a DualShock-family pad is this one, so `ds2@N` in a
/// [`Wiring`] spec is twelve consecutive axes from `N` mapped through this table.
const DS2_PRESSURE_ORDER: [Control; 12] = [
    Control::Right,
    Control::Left,
    Control::Up,
    Control::Down,
    Control::Triangle,
    Control::Circle,
    Control::Cross,
    Control::Square,
    Control::L1,
    Control::R1,
    Control::L2,
    Control::R2,
];

/// Where the shell takes the throttle from.
///
/// The game reads Cross from its pressure byte, so [`game_sim::ride::Controls::gas`] is the fraction
/// `(pressure + 1) / 256` and a pad with pressure-sensitive face buttons rides part throttle with no
/// help from this module. A pad without them reports Cross as one bit, this module has to answer a
/// full byte for it (a digital source cannot mean anything else), and the throttle is a switch: full
/// or nothing, with the bike's own `GasRateLimit` — 10 a second on the 125, so three ticks of the
/// engine's 30 Hz — the only thing between the two.
///
/// A trigger is the way out on such a pad. It is travel the kernel already reports and the port
/// already reads L2 and R2 as pressures, so wiring gas onto one costs nothing but that trigger's own
/// control.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Throttle {
    /// Cross: a bit on most pads, a pressure on a pad that reports one.
    #[default]
    Cross,
    /// A trigger's travel, which gives up that trigger's own control for the session. In a ride the
    /// original's L2 and R2 are its look-back and other camera (P73), which the shell does not follow
    /// yet, so today nothing in a ride is lost.
    Trigger(Control),
}

/// How the shell is wired to the host's pad: where the throttle comes from, and which joystick axes
/// carry a per-button pressure.
///
/// Both are facts about the device in somebody's hands rather than about the game, which is why they
/// are asked for (`--throttle`, `--pad-pressure`) instead of guessed. The default is the plain
/// gamepad every machine has: Cross on its bit, no pressure axes, and nothing read past the eighth
/// axis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wiring {
    /// Where gas comes from.
    pub throttle: Throttle,
    /// The control each joystick axis carries the pressure of, for a device that reports one.
    pub pressure: [Option<Control>; AXES],
    /// Which layout each seat rides on (P40): the new one, or the original's.
    pub schemes: [Scheme; SEATS],
    /// How each seat's biplane stick reads (P50, P69).
    pub planes: [PlaneScheme; SEATS],
    /// Each seat's controller preset under the original's layout (P79): which of the original's four
    /// binding presets its bike is read through ([`game_sim::bindings::PRESET_CONTROLS`]), 0 the
    /// shipped one. The new layout rides preset 0 whatever this says ([`Scheme::preset`]).
    pub presets: [usize; SEATS],
    /// A Nintendo pad's A and B by where they are (P53): B, the bottom one, is Cross and confirms,
    /// as on every other pad — the default (P64). Off, they go by what they say — A, the right one,
    /// confirms and B goes back, the way a Switch's own menus have it ([`Pads::nintendo`]).
    pub nintendo_positions: bool,
}

impl Default for Wiring {
    fn default() -> Wiring {
        Wiring {
            throttle: Throttle::default(),
            pressure: [None; AXES],
            schemes: [Scheme::default(); SEATS],
            planes: [PlaneScheme::default(); SEATS],
            presets: [0; SEATS],
            nintendo_positions: true,
        }
    }
}

/// Which controls a seat's pad is laid out as (P40).
///
/// The simulation reads the original's binding table and nothing else — Cross the gas, Square the
/// brake, L1 the clutch, Circle Triangle R1 the tricks, L1+R1 the reset, Select the camera
/// ([`game_sim::ride::Controls::from_pad`], [`game_sim::bindings::Table::SHIPPED`], P73) — so a
/// scheme is a remap of the pad's report *before* the ride sees it ([`Scheme::ride`]),
/// and what a tape records is the remapped report: the controls the simulation read, which replay
/// the same whatever scheme replays them. Under [`Scheme::Original`] the remap is the identity, so a
/// replay or an acceptance run is bit for bit what it was before there was a choice.
///
/// The menus read the pad as it is (Cross is OK there under either), because a scheme is about the
/// ride and the front end is the port's own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scheme {
    /// R2 the gas and L2 the brake, both on their trigger's travel; everything else where the
    /// original has it (L1 the clutch, Select the camera). Cross and Square do nothing in a ride,
    /// and are kept free for what comes later (more tricks, other functions). The original's two
    /// camera commands on L2 and R2 (look back, the other camera) have no button here; the shell
    /// does not follow them under either scheme yet.
    ///
    /// L1 is the original's clutch (P67), on its button as the original's: no trigger travel, because
    /// the original's bike reads its clutch as a switch (P73; P77 has the port's levers). Held at a
    /// standstill with the gas on it lets the engine rev with the rear wheel free, and let go it
    /// launches on the clutch's ramp. L1+R1 is still the put-back, but pressed together
    /// (`game_sim::ride::Controls::chord_together`): a clutch already held when R1 goes down for a
    /// trick is the clutch and the trick, where the original puts the bike back.
    #[default]
    New,
    /// The original's: the pad as the simulation reads it, the shipped table's bindings exactly.
    Original,
}

impl Scheme {
    pub const ALL: [Scheme; 2] = [Scheme::New, Scheme::Original];

    /// The word `--controls` and `controls.txt` spell it with.
    pub fn word(self) -> &'static str {
        match self {
            Scheme::New => "new",
            Scheme::Original => "original",
        }
    }

    pub fn from_word(w: &str) -> Option<Scheme> {
        Scheme::ALL.into_iter().find(|s| s.word() == w.trim())
    }

    /// `new`, `original`, or one per seat (`new,original,…`); seats past the list ride the last one.
    pub fn parse_list(s: &str) -> Result<[Scheme; SEATS], String> {
        let words: Vec<&str> = s.split(',').collect();
        if words.len() > SEATS {
            return Err(format!("{s}: at most {SEATS} seats"));
        }
        let mut out = [Scheme::default(); SEATS];
        let mut last = Scheme::default();
        for (i, slot) in out.iter_mut().enumerate() {
            if let Some(w) = words.get(i) {
                last = Scheme::from_word(w).ok_or_else(|| format!("{w}: new or original"))?;
            }
            *slot = last;
        }
        Ok(out)
    }

    /// [`Scheme::parse_list`]'s text back: one word when every seat agrees, for the line a ride
    /// starts with.
    pub fn list_text(schemes: &[Scheme; SEATS]) -> String {
        if schemes.iter().all(|s| *s == schemes[0]) {
            return schemes[0].word().to_string();
        }
        schemes.iter().map(|s| s.word()).collect::<Vec<_>>().join(",")
    }

    /// The controller preset a seat on this scheme rides on, `preset` being the one it picked (P79):
    /// the pick under the original's layout, and preset 0 under the new one, whose remap is written
    /// against the shipped table.
    pub fn preset(self, preset: usize) -> usize {
        match self {
            Scheme::Original => preset.min(game_sim::bindings::PRESETS - 1),
            Scheme::New => 0,
        }
    }

    /// `--preset`'s value: one of the original's four controller presets (`0` to `3`), or one per
    /// seat (`0,2,…`); seats past the list ride the last one.
    pub fn parse_presets(s: &str) -> Result<[usize; SEATS], String> {
        let words: Vec<&str> = s.split(',').collect();
        if words.len() > SEATS {
            return Err(format!("{s}: at most {SEATS} seats"));
        }
        let mut out = [0; SEATS];
        let mut last = 0;
        for (i, slot) in out.iter_mut().enumerate() {
            if let Some(w) = words.get(i) {
                last = w
                    .trim()
                    .parse::<usize>()
                    .ok()
                    .filter(|p| *p < game_sim::bindings::PRESETS)
                    .ok_or_else(|| format!("{w}: a preset from 0 to {}", game_sim::bindings::PRESETS - 1))?;
            }
            *slot = last;
        }
        Ok(out)
    }

    /// The control the shell changes the camera on, read off the remapped report: the shipped
    /// table's next-camera command, Select, under either scheme (P73: the original's is Select, not
    /// L1, which is its clutch and half its reset chord).
    pub fn camera(self) -> Control {
        Table::SHIPPED.control(0, Command::NEXT_CAMERA).unwrap_or(Control::Select)
    }

    /// The commands this scheme carries, each with the pad's control that reaches it (P73): under
    /// the original's, [`Command::RIDE`] on the shipped table's bindings; under the new one the same
    /// commands, the gas and the brake moved to R2 and L2 and the two camera holds the triggers had,
    /// look back and the other camera, to the faces they left, Square and Cross (P78). In
    /// [`Command::RIDE`]'s order.
    pub fn commands(self) -> Vec<(Command, Control)> {
        self.commands_on(&Table::SHIPPED)
    }

    /// [`Scheme::commands`] on another table: a controller preset's (P79).
    pub fn commands_on(self, table: &Table) -> Vec<(Command, Control)> {
        Command::RIDE
            .into_iter()
            .filter_map(|c| {
                let bound = table.control(0, c)?;
                let on = match (self, bound) {
                    (Scheme::New, Control::Cross) => Control::R2,
                    (Scheme::New, Control::Square) => Control::L2,
                    (Scheme::New, Control::R2) => Control::Cross,
                    (Scheme::New, Control::L2) => Control::Square,
                    _ => bound,
                };
                Some((c, on))
            })
            .collect()
    }

    /// Whether this frame of the port's own poll asks for the next camera, as the original's camera
    /// handler decides it: the command's press, with neither look-back nor the other camera held.
    /// Under the new scheme those two are on Square and Cross (P78), which reach the ride as L2 and R2.
    pub fn changes_camera(self, pad: &game_sim::input::Pad) -> bool {
        self.changes_camera_on(pad, &Table::SHIPPED)
    }

    /// [`Scheme::changes_camera`] through a seat's own table (P79): under presets 2 and 3 the two
    /// commands that hold the camera back are on other buttons.
    pub fn changes_camera_on(self, pad: &game_sim::input::Pad, t: &Table) -> bool {
        t.pressed(pad, Command::NEXT_CAMERA) && !t.held(pad, Command::LOOK_BACK) && !t.held(pad, Command::OTHER_CAMERA)
    }

    /// The control that puts a camera the right stick has turned back where its preset says (P114),
    /// the port's own: R3, the right stick's click, under either scheme. The original binds R3 to two
    /// commands (0x07, 0x23) that nothing in it reads, and the shell had nothing on it, so no layout
    /// loses a control to it.
    pub const RESET_CAMERA: Control = Control::R3;

    /// Whether this frame asks for the camera's reset: [`Scheme::RESET_CAMERA`]'s press.
    pub fn resets_camera(self, pad: &game_sim::input::Pad) -> bool {
        pad.pressed(Scheme::RESET_CAMERA)
    }

    /// The report the simulation is handed for a pad that reported `report`.
    ///
    /// Under the new scheme R2 and Cross trade places, and so do L2 and Square, bit and pressure byte
    /// together — the game reads the gas and the brake from the pressure, so a trigger's travel is
    /// the original's part throttle and part brake. So the pad's R2 and L2 are the original's gas
    /// and brake, and its Cross and Square the original's L2 and R2 commands, look back and the
    /// other camera (P78: the faces the gas and the brake left; nothing in the ride reads L2 or R2).
    /// Everything else, Select among it, reaches the ride as the pad said it. A pad that is not there
    /// is left as it is.
    pub fn ride(self, report: Report) -> Report {
        if self == Scheme::Original || report.status() == 0xff {
            return report;
        }
        let mut out = report;
        let was = report.buttons();
        let swaps = [(Control::Cross, Control::R2), (Control::Square, Control::L2), (Control::R2, Control::Cross), (Control::L2, Control::Square)];
        let mut held = was & !game_sim::input::mask_of(&[Control::Cross, Control::Square, Control::L2, Control::R2]);
        let pressure = |c: Control| c.binding().pressure as usize;
        for (to, from) in swaps {
            if was & from.mask() != 0 {
                held |= to.mask();
            }
            out.0[pressure(to)] = report.0[pressure(from)];
        }
        let low = !held;
        out.0[2] = (low >> 8) as u8;
        out.0[3] = low as u8;
        out
    }

    /// [`Scheme::ride`] on a ride with MX vs. ATV's tricks (MD16), which reads its two extra trick
    /// buttons off the report's R2 and L2 (`game_sim::trick_pack::EXTRA_CONTROLS`). Under the new
    /// scheme the pad's Cross and Square, which [`Scheme::ride`] puts there as the two camera holds
    /// (P78), are them, and the holds go with them: the shell follows neither while a ride has the
    /// extras. The extras are the new layout's only: under the original's the pad's R2 and L2
    /// are let go instead, so they never start one. With `tricks` off this is [`Scheme::ride`] exactly.
    pub fn ride_tricks(self, report: Report, tricks: bool) -> Report {
        let mut out = self.ride(report);
        if !tricks || report.status() == 0xff {
            return out;
        }
        let pressure = |c: Control| c.binding().pressure as usize;
        let mut held = out.buttons() & !game_sim::input::mask_of(&[Control::L2, Control::R2]);
        out.0[pressure(Control::R2)] = 0;
        out.0[pressure(Control::L2)] = 0;
        if self == Scheme::New {
            let was = report.buttons();
            for (to, from) in game_sim::trick_pack::EXTRA_CONTROLS.into_iter().zip([Control::Cross, Control::Square]) {
                if was & from.mask() != 0 {
                    held |= to.mask();
                }
                out.0[pressure(to)] = report.0[pressure(from)];
            }
        }
        let low = !held;
        out.0[2] = (low >> 8) as u8;
        out.0[3] = low as u8;
        out
    }
}

/// How the biplane's stick reads at every attitude (P50, P69).
///
/// The original never turns the plane's controls round: its input step (I6) and its control law
/// (`game_sim::biplane::controls`, S34) read the stick, the d-pad and L1/R1 with the same sign at
/// every attitude, so their sign is already the plane's own: stick down pulls the nose toward the
/// belly, L1 yaws to the pilot's left, whichever way up the plane is. One thing in it is not the
/// plane's: the control law fades the elevator out by the roll while the nose is near level, from
/// all of it at `ElevatorRampOffRollAngle` (30 degrees on the disc) to `MinElevator` (0 — the file
/// does not set it) at `ElevatorOffRollAngle` (60), and back the same way past 120. On its side the
/// original's plane does not answer the stick's y at all.
///
/// - [`PlaneScheme::New`], the default (P69): relative to the plane, never the camera. The report
///   reaches the plane as the pad said it, and the plane's step lifts the fade
///   ([`PlaneScheme::every_bank`], `vehicle::Plane::every_bank`), so down on the stick pulls the
///   nose toward the belly by the same amount at every bank and attitude. A report cannot do that
///   part: at knife edge the fade multiplies every stick and d-pad value by zero. Since P89 the
///   step also reads the stick round and keeps the pitch loop whatever the aileron does
///   ([`PlaneScheme::round_stick`], `vehicle::round_stick`), so a held diagonal pulls and rolls
///   together instead of a fifth of each with the nose falling.
/// - [`PlaneScheme::Screen`], P50's: relative to the follow camera, which keeps the world's up. Upside
///   down the elevator and the rudder are turned back round in the report ([`PlaneScheme::fly`]),
///   so they move the plane the same way on the screen; it changes over just past knife edge.
/// - [`PlaneScheme::Original`]: the plane reads the pad as it is, faded as on the disc.
///
/// Like [`Scheme`], each is decided in the shell before the simulation reads the pad, so under the
/// original's the plane's simulation is the original's bit for bit and a tape records what it read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaneScheme {
    /// Relative to the plane (P69): the pad as it is, and the elevator at every bank.
    #[default]
    New,
    /// Relative to the screen (P50): upside down, the left stick's y is turned round, and L1 and R1
    /// are each other's (Up and Down were too, until the d-pad was the markers', P107).
    Screen,
    /// The original's: the plane reads the pad as it is, at any attitude, and its elevator fades
    /// out on its side.
    Original,
}

impl PlaneScheme {
    /// The word `--biplane-controls` and `controls.txt` spell it with.
    pub fn word(self) -> &'static str {
        match self {
            PlaneScheme::New => "new",
            PlaneScheme::Screen => "screen",
            PlaneScheme::Original => "original",
        }
    }

    fn from_word(w: &str) -> Option<PlaneScheme> {
        [PlaneScheme::New, PlaneScheme::Screen, PlaneScheme::Original]
            .into_iter()
            .find(|s| s.word().eq_ignore_ascii_case(w.trim()))
    }

    /// `new`, `screen`, `original`, or one per seat, the way [`Scheme::parse_list`] reads them.
    pub fn parse_list(s: &str) -> Result<[PlaneScheme; SEATS], String> {
        let words: Vec<&str> = s.split(',').collect();
        if words.len() > SEATS {
            return Err(format!("{s}: at most {SEATS} seats"));
        }
        let mut out = [PlaneScheme::default(); SEATS];
        let mut last = PlaneScheme::default();
        for (i, slot) in out.iter_mut().enumerate() {
            if let Some(w) = words.get(i) {
                last = PlaneScheme::from_word(w).ok_or_else(|| format!("{w}: new, screen or original"))?;
            }
            *slot = last;
        }
        Ok(out)
    }

    /// [`PlaneScheme::parse_list`]'s text back: one word when every seat agrees.
    pub fn list_text(schemes: &[PlaneScheme; SEATS]) -> String {
        if schemes.iter().all(|s| *s == schemes[0]) {
            return schemes[0].word().to_string();
        }
        schemes.iter().map(|s| s.word()).collect::<Vec<_>>().join(",")
    }

    /// Whether the plane's elevator answers the stick at every bank (P69): the plane-relative
    /// scheme's half that a report cannot carry, which the plane's step applies
    /// (`vehicle::Plane::every_bank`). Only under [`PlaneScheme::New`].
    pub fn every_bank(self) -> bool {
        self == PlaneScheme::New
    }

    /// Whether the plane reads its stick round and keeps its pitch loop at any roll (P89): the
    /// plane-relative scheme's other half, also the plane's step's (`vehicle::Plane::round_stick`),
    /// because what the stick means is the input step's arithmetic and not a report's bytes. Only
    /// under [`PlaneScheme::New`].
    pub fn round_stick(self) -> bool {
        self == PlaneScheme::New
    }

    /// The report the plane is handed for a pad that reported `report`, `inverted` being whether the
    /// plane is upside down this frame (`vehicle::Plane::inverted`).
    ///
    /// Upside down under the screen's scheme: the left stick's y byte is mirrored (`255 - b`, which
    /// the poll's `-1 + 2b/255` reads as exactly the other side), and L1 and R1 are swapped, bit and
    /// pressure byte together. Everything else, and every report under the plane's or the original's
    /// or the right way up, is passed as it is. A pad that is not there is left alone.
    ///
    /// The d-pad is not swapped (P107): the plane no longer reads it (`vehicle::Plane::step`), the
    /// session markers do, and their Down puts a marker down whichever way up the plane is.
    pub fn fly(self, report: Report, inverted: bool) -> Report {
        if self != PlaneScheme::Screen || !inverted || report.status() == 0xff {
            return report;
        }
        let mut out = report;
        let was = report.buttons();
        let pairs = [(Control::L1, Control::R1)];
        let mut held = was & !game_sim::input::mask_of(&[Control::L1, Control::R1]);
        for (a, b) in pairs {
            if was & a.mask() != 0 {
                held |= b.mask();
            }
            if was & b.mask() != 0 {
                held |= a.mask();
            }
            let (pa, pb) = (a.binding().pressure as usize, b.binding().pressure as usize);
            (out.0[pa], out.0[pb]) = (report.0[pb], report.0[pa]);
        }
        let low = !held;
        out.0[2] = (low >> 8) as u8;
        out.0[3] = low as u8;
        out.set_axis(LEFT_Y, 255 - report.axis(LEFT_Y));
        out
    }

    /// [`Scheme::changes_camera`] for a seat flying under this scheme. The camera is Select under
    /// either layout (P73), which [`PlaneScheme::fly`] never swaps, so upside down or not it is the
    /// layout's own test.
    pub fn changes_camera(self, scheme: Scheme, pad: &game_sim::input::Pad, _inverted: bool) -> bool {
        scheme.changes_camera(pad)
    }
}

/// The left stick's y, in the report's own axis order ([`Report::axis`]).
const LEFT_Y: usize = 3;

impl Wiring {
    /// Read a `--pad-pressure` spec: `none`, `ds2@AXIS` — twelve consecutive axes from `AXIS` in
    /// [`DS2_PRESSURE_ORDER`] — or a comma-separated list of `AXIS:CONTROL` for a device that lays
    /// them out some other way.
    ///
    /// A spec rather than a built-in table per pad because no device the project has ever had in
    /// front of it reports per-button pressure: the project cannot say which axis a given driver
    /// puts Cross's pressure on, and a wrong default would read a stick as a throttle. (The debug
    /// HUD, `--hud`, showed each control's strength to find it by; P170 took it out.)
    pub fn parse_pressures(spec: &str) -> Result<[Option<Control>; AXES], String> {
        let mut map = [None; AXES];
        let spec = spec.trim();
        if spec.is_empty() || spec == "none" {
            return Ok(map);
        }
        if let Some(first) = spec.strip_prefix("ds2@") {
            let first: usize = first
                .parse()
                .map_err(|_| format!("ds2@{first}: {first} is not an axis number"))?;
            for (i, control) in DS2_PRESSURE_ORDER.iter().enumerate() {
                let axis = first + i;
                if axis >= AXES {
                    return Err(format!(
                        "ds2@{first}: the twelve pressures would reach axis {axis}, and the shell \
                         reads {AXES}"
                    ));
                }
                map[axis] = Some(*control);
            }
            return Ok(map);
        }
        for pair in spec.split(',') {
            let (axis, control) = pair
                .split_once(':')
                .ok_or_else(|| format!("{pair}: an axis map is AXIS:CONTROL, like 14:cross"))?;
            let axis: usize = axis
                .trim()
                .parse()
                .map_err(|_| format!("{pair}: {axis} is not an axis number"))?;
            if axis >= AXES {
                return Err(format!("{pair}: the shell reads axes 0 to {}", AXES - 1));
            }
            let control = control_named(control.trim())
                .ok_or_else(|| format!("{pair}: no control is called {control}"))?;
            if !control.binding().is_analogue() {
                return Err(format!(
                    "{pair}: the game reads {} from its bit, so a pressure for it would never be \
                     looked at",
                    control.name(),
                ));
            }
            map[axis] = Some(control);
        }
        Ok(map)
    }
}

/// One of the sixteen controls by the name the project calls it ([`Control::name`]).
pub fn control_named(name: &str) -> Option<Control> {
    Control::ALL.into_iter().find(|c| c.name() == name)
}

/// How long a held direction key takes to push its stick all the way over, and how long a stick
/// takes to come back to centre once the keys are let go, in seconds.
///
/// A key is a switch and a stick is not, so these two numbers are the keyboard's whole analogue
/// feel, and there is no original behaviour to match: the disc's input is a pad. What they are
/// chosen against is the one measured number in the area, the ride's stick curve (I7's ±0.3, the
/// player's own range): 30% of the travel either side of centre is
/// dead, so the first ~54 ms of a held key is the stick crossing the dead zone and the lean, the
/// steer and the preload grow from nothing after it rather than slamming to full on frame one.
/// Coming back is quicker than going out, because a rider who lets go of a key has already decided.
///
/// Since P30 these ramp `IJKL`, the right stick, only. The arrow keys were the left stick on this
/// ramp until then, and P16 measured that a pump-and-flip on them fired on the same frames as one on
/// a stick — because the port read the backflip's stick through the ride's rate limit, which no key
/// ramp could get ahead of. The original reads that stick raw and sizes the flip by how fast it
/// crossed, and on the disc's 125 the ramp cannot make the three crosses inside the lip at all. The
/// arrows are the d-pad now ([`KEYBOARD`]), which is the original's own digital stick.
const KEY_TO_EDGE: f32 = 0.18;
const KEY_TO_CENTRE: f32 = 0.10;

/// The longest step a keyboard stick will take in one frame, in seconds. A frame that stalled — the
/// first one after a level load, or a window dragged across a workspace — would otherwise hand the
/// ramp a tenth of a second of travel at once, which is the slam this is here to stop.
const KEY_STEP_MAX: f32 = 1.0 / 15.0;

/// How long the keyboard's throttle key takes to reach full throttle, and how long it takes to shut
/// again once it is let go, in seconds.
///
/// The same argument as [`KEY_TO_EDGE`]'s and a different number, because what this ramp has to be
/// slower than is not a dead zone but the bike's own rate limit: `GasRateLimit` is 10 a second on the
/// retail 125 (`Race125P.phy`, read off the disc), so the ride walks a stabbed throttle from nothing
/// to full in a tenth of a second — three ticks of the engine's 30 Hz — all by itself. A key ramp
/// quicker than that would be invisible: the limit would swallow it and the throttle would still be a
/// switch with a three-tick fade. At 0.3 s a stab of two frames is a fifth of the throttle and a held
/// key is still pinned a third of a second later, which is what makes part throttle something a
/// keyboard can hold rather than something it passes through.
const KEY_TO_FULL: f32 = 0.30;
const KEY_TO_SHUT: f32 = 0.12;

/// Which two keys drive which stick axis, and where that axis goes in the report. The low key is the
/// one at the low end of the byte, which is up or left — the hardware's own sense (see [`stick_byte`]).
/// The left stick has no keys: the arrows are the d-pad ([`KEYBOARD`]).
const KEY_STICKS: [(usize, u8, u8); 2] = [
    (REPORT_RIGHT_X, key::J, key::L),
    (REPORT_RIGHT_Y, key::I, key::K),
];

/// The new layout's stick keys (P54), in [`KEY_STICK_SLOTS`]' order: `YUIO` the right stick, the
/// row above `HJKL` and vim's way along it (Y left, U down, I up, O right) — `HJKL` itself is the
/// gas, the brake and two face buttons ([`KEYBOARD_NEW`]) — and `WASD` the left, which steers and leans (A and D
/// steer, W leans forward and S back — up is the low end of the byte, as on the pad).
const KEY_STICKS_NEW: [(usize, u8, u8); 4] = [
    (REPORT_RIGHT_X, key::Y, key::O),
    (REPORT_RIGHT_Y, key::I, key::U),
    (REPORT_LEFT_X, key::A, key::D),
    (REPORT_LEFT_Y, key::W, key::S),
];

/// The stick axes the keyboard can move, in `controls.txt`'s order of stick lines ([`STICK_KEYS`]):
/// the right stick's two, then the left stick's.
const KEY_STICK_SLOTS: [usize; 4] = [REPORT_RIGHT_X, REPORT_RIGHT_Y, REPORT_LEFT_X, REPORT_LEFT_Y];

/// One seat's pad and keyboard, as one report a frame: a joystick device, the keyboard, or both
/// folded together (the first seat's pad and the keyboard, on a machine nobody has set seats up on).
/// [`Seats`] owns one of these per pad that is plugged in and one for the keyboard.
pub struct Pads {
    /// The joystick device, if one opened.
    device: Option<std::fs::File>,
    /// Which of the shell's slots each of the device's js numbers is ([`JsMap`]).
    map: JsMap,
    /// A Nintendo pad, whose A (the right button) confirms only if the wiring says labels.
    nintendo: bool,
    /// Its name, for the line the shell prints when it joins.
    pub name: Option<String>,
    /// The device stopped answering — unplugged, or the read failed — and was closed.
    gone: bool,
    /// The last value of each axis, kept because a joystick event only mentions what changed: in
    /// the shell's own slots ([`AXIS_LEFT_X`]…), and by the device's own numbers, which is what a
    /// [`Wiring`]'s pressure axes are.
    axis: [i16; AXES],
    raw: [i16; AXES],
    /// Presses since the last report, a count per control ([`Control`] order), so a press and its
    /// release inside one slow frame is still a press (P53): at 5 fps a tap is shorter than a frame,
    /// and the held state alone never saw it.
    queue: [u8; 16],
    /// The controls the last report pressed out of [`Pads::queue`].
    taps: u16,
    /// When each control ([`Control::ALL`]'s order) was last let go, by the events' clock, for
    /// [`BOUNCE_MS`].
    up_at: [Option<u32>; 16],
    /// Which pad buttons are down, in [`PAD_BUTTONS`]' slots.
    button: [bool; BUTTONS],
    /// The d-pad directions a device sends as buttons ([`JsButton::Dpad`]) held.
    dpad: u16,
    /// The keyboard's stick axes, in [`KEY_STICK_SLOTS`]' order, each part way to its edge.
    key_stick: [KeyAxis; KEY_STICK_SLOTS.len()],
    /// The keyboard's throttle, part way to full.
    key_gas: KeyRamp,
    /// Where the throttle comes from and which axes carry a button's pressure.
    wiring: Wiring,
    /// Which key and which pad button is which of the game's controls (`controls.txt`).
    bindings: Bindings,
    /// The scheme of the seat this pad is reporting for (P40), which [`Seats`] sets each frame
    /// because a pad's seat can change: it decides what a wired trigger and the throttle keys are.
    /// A `Pads` on its own is the original's, so this module's tests and `tests/ride.rs` read the
    /// report the simulation reads.
    scheme: Scheme,
    /// When the last report was built, so the ramps travel at a rate in seconds rather than one a
    /// frame: the shell paces itself to 60 Hz but a heavy frame overruns, and a keyboard that leans
    /// further when the picture is cheap would be its own bug.
    clock: Option<std::time::Instant>,
}

impl Pads {
    /// A `Pads` with no device: the keyboard on a machine with no pad, and what this module's own
    /// tests ride, since a test that opened `/dev/input/js0` would read whatever is plugged in.
    /// `pub(crate)` for `tests/ride.rs`, which compiles this file in to ride a flip on the keys.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn unplugged(wiring: Wiring) -> Pads {
        Pads::bound(wiring, Bindings::default())
    }

    /// A `Pads` with no device, on these bindings.
    ///
    /// A stick and a hat rest in the middle of their range, but a trigger rests at the bottom of
    /// its: zero on a trigger axis is the trigger pulled half way, which on a machine with no pad at
    /// all is a permanent 43% of clutch and of front brake. A device that opens sends an initial
    /// event for every axis and overwrites both within a frame; a machine with no device never does,
    /// so the rest position is what they start at. (Found with P6's overlay, which is what it is
    /// for.) A button's pressure axis rests at the bottom of its travel too, for the same reason and
    /// with the same consequence if it is not said: a mapped axis left at zero would read as a
    /// button held half way down before the device had spoken.
    fn bound(wiring: Wiring, bindings: Bindings) -> Pads {
        let mut axis = [0i16; AXES];
        axis[AXIS_L2] = TRIGGER_REST;
        axis[AXIS_R2] = TRIGGER_REST;
        let mut raw = [0i16; AXES];
        for (slot, control) in raw.iter_mut().zip(wiring.pressure) {
            if control.is_some() {
                *slot = TRIGGER_REST;
            }
        }
        Pads {
            device: None,
            map: JsMap::identity(),
            nintendo: false,
            name: None,
            gone: false,
            axis,
            raw,
            queue: [0; 16],
            taps: 0,
            up_at: [None; 16],
            button: [false; BUTTONS],
            dpad: 0,
            key_stick: [KeyAxis::CENTRED; KEY_STICK_SLOTS.len()],
            key_gas: KeyRamp::SHUT,
            scheme: Scheme::Original,
            wiring,
            bindings,
            clock: None,
        }
    }

    /// Rewire the pad, which the front end's Controls screen does between frames (P36).
    pub fn set_wiring(&mut self, wiring: Wiring) {
        self.wiring = wiring;
    }

    /// Read whatever the pad has said since the last frame, then build this frame's report.
    ///
    /// The joystick interface sends the whole current state as "initial" events when the device is
    /// opened, so the first frame after opening already knows where the sticks are.
    ///
    /// `held` is the keyboard, for a seat the keyboard drives, and [`no_keys`] for one it does not.
    pub fn report(&mut self, held: &dyn Fn(u8) -> bool) -> Report {
        let seconds = self.since_the_last_report();
        self.build(seconds, held)
    }

    /// This frame's report, from the pad's state and whatever `held` says about the keyboard.
    ///
    /// The key state arrives as a function rather than as a [`Window`] so that the ramps — which are
    /// nothing but time and key state — can be ridden by a test: a window needs an X server, and the
    /// whole point of a ramp is what it answers on the second frame of a press — `tests/ride.rs`'s
    /// flip among them.
    pub(crate) fn build(&mut self, seconds: f32, held: &dyn Fn(u8) -> bool) -> Report {
        self.drain();
        let mut report = Report::idle();
        let mut buttons = self.take_taps();

        // The pad's buttons and the keyboard's keys, as one set of the game's own controls.
        buttons |= self.bindings.buttons(self.scheme, &self.button, held);
        buttons |= self.hat_mask() | self.dpad;

        // The button word, active low, and a full pressure byte for every analogue control in it —
        // which is what a digital source has to mean, and what the game's own binding reads.
        let low = !buttons;
        report.0[2] = (low >> 8) as u8;
        report.0[3] = low as u8;
        for binding in CONTROL_MAP {
            if binding.is_analogue() && buttons & binding.mask != 0 {
                report.0[binding.pressure as usize] = 0xff;
            }
        }

        // The two sticks, as the bytes a DualShock 2 would have reported.
        report.set_axis(REPORT_LEFT_X, stick_byte(self.axis[AXIS_LEFT_X]));
        report.set_axis(REPORT_LEFT_Y, stick_byte(self.axis[AXIS_LEFT_Y]));
        report.set_axis(REPORT_RIGHT_X, stick_byte(self.axis[AXIS_RIGHT_X]));
        report.set_axis(REPORT_RIGHT_Y, stick_byte(self.axis[AXIS_RIGHT_Y]));
        // The keyboard moves the sticks too — the right stick on `IJKL` under the original's layout,
        // and under the new one (P54, P75) `YUIO` and the left stick on `WASD` — ramped toward the edge
        // for as long as the key is held and let back to centre when it is not ([`KeyAxis`]). The
        // arrows are the d-pad, in [`KEYBOARD`].
        for (i, (axis, slot)) in self.key_stick.iter_mut().zip(KEY_STICK_SLOTS).enumerate() {
            let (low, high) = self.bindings.stick_keys(self.scheme, i, held);
            if let Some(byte) = axis.step(seconds, low, high) {
                report.set_axis(slot, byte);
            }
        }

        // How hard each button the *device* reports a pressure for is pressed, where a [`Wiring`]
        // says which axis that is. The bit still decides whether the control is down — the device
        // raised it — so this only ever replaces the full byte above with a smaller one, and a
        // pressure the dead zone swallows leaves the press at full rather than dropping it.
        for (axis, control) in self.wiring.pressure.iter().enumerate() {
            let Some(binding) = control.map(Control::binding) else { continue };
            if !binding.is_analogue() || buttons & binding.mask == 0 {
                continue;
            }
            let pressure = trigger_pressure(self.raw[axis]);
            if pressure > PRESSURE_THRESHOLD {
                report.0[binding.pressure as usize] = pressure;
            }
        }

        // The analogue triggers, which are a pressure and not a bit: a pad's L2 and R2 travel, and
        // the game reads those two controls from their pressure byte alone. One of them is the
        // throttle instead when the session asked for that, and then it is not itself at all — a
        // front brake that came on with the gas would be worse than no part throttle. Under the new
        // scheme the triggers are already the gas and the brake ([`Scheme::ride`]), so there they
        // are only themselves.
        for (axis, control) in [(AXIS_L2, Control::L2), (AXIS_R2, Control::R2)] {
            let pressure = trigger_pressure(self.axis[axis]);
            let control = match self.wiring.throttle {
                Throttle::Trigger(wired) if wired == control && self.scheme == Scheme::Original => Control::Cross,
                _ => control,
            };
            if pressure > 0 {
                press(&mut report, control, pressure);
            }
        }

        // And the keyboard's throttle, which is the one control a key cannot do justice to: ramped
        // ([`KeyRamp`]), so a stab is part throttle and a held key is still pinned. It is Cross in the
        // report whatever the scheme, which is the menus' OK, and under the new scheme R2 as well,
        // which is what the ride takes the gas from there. Its keys are the scheme's gas line's
        // ([`Bindings::gas`]): `cross` under the original's, `r2` under the new one.
        if let Some(pressure) = self.key_gas.step(seconds, self.bindings.throttle_key(self.scheme, held)) {
            press(&mut report, Control::Cross, pressure);
            if self.scheme == Scheme::New {
                press(&mut report, Control::R2, pressure);
            }
        }
        report
    }

    /// How long since the last report was built, in seconds, capped at [`KEY_STEP_MAX`]. The first
    /// report of a run gets nothing, which leaves the sticks where they start: at centre.
    fn since_the_last_report(&mut self) -> f32 {
        let now = std::time::Instant::now();
        match self.clock.replace(now) {
            Some(then) => now.duration_since(then).as_secs_f32().min(KEY_STEP_MAX),
            None => 0.0,
        }
    }

    /// One joystick event: the state it changes, and a press for [`Pads::queue`] when it is a
    /// button or a hat direction going down — not an initial one, which is how the device was when
    /// it was opened and nobody's press.
    ///
    /// A control is pressed when it goes down from every source it has — a d-pad sent both as a hat
    /// and as buttons is one push, not two (P109) — and only once it has been up [`BOUNCE_MS`] by
    /// `time`, the event's own milliseconds.
    fn event(&mut self, kind: u8, number: usize, value: i16, init: bool, time: u32) {
        let before = self.pressable();
        match kind {
            EVENT_AXIS => {
                if let Some(slot) = self.raw.get_mut(number) {
                    *slot = value;
                }
                let Some(slot) = self.map.axis(number) else { return };
                self.axis[slot] = value;
            }
            EVENT_BUTTON => match self.map.button(number) {
                JsButton::Pad(slot) => {
                    let slot = self.slot(slot);
                    self.button[slot] = value != 0;
                }
                JsButton::Trigger(slot) => self.axis[slot] = if value != 0 { i16::MAX } else { TRIGGER_REST },
                JsButton::Dpad(control) => {
                    if value != 0 {
                        self.dpad |= control.mask();
                    } else {
                        self.dpad &= !control.mask();
                    }
                }
                JsButton::Nothing => {}
            },
            _ => {}
        }
        let after = self.pressable();
        let mut pressed = 0;
        for (up_at, control) in self.up_at.iter_mut().zip(Control::ALL) {
            let m = control.mask();
            if before & m != 0 && after & m == 0 {
                *up_at = Some(time);
            } else if before & m == 0 && after & m != 0 {
                let bounced = up_at.is_some_and(|at| time.wrapping_sub(at) < BOUNCE_MS);
                if !init && !bounced {
                    pressed |= m;
                }
            }
        }
        self.queue_mask(pressed);
    }

    /// The controls held that a press can come from: the pad's buttons as its bindings make them,
    /// and the d-pad, hat or buttons. Not the triggers' axes, which are a pressure.
    fn pressable(&self) -> u16 {
        self.bindings.buttons(self.scheme, &self.button, &no_keys) | self.hat_mask() | self.dpad
    }

    /// The slot a button is read in: a Nintendo pad's A and B change places unless the wiring reads
    /// it by position (P53). hid-nintendo names its buttons by where they are — B, the bottom one,
    /// is `BTN_SOUTH` — so by position B is Cross; by its label A is, which is what a Switch's own
    /// menus confirm with.
    fn slot(&self, slot: usize) -> usize {
        match slot {
            0 | 1 if self.nintendo && !self.wiring.nintendo_positions => 1 - slot,
            _ => slot,
        }
    }

    /// The d-pad held on the hat.
    fn hat_mask(&self) -> u16 {
        hat(self.axis[AXIS_HAT_X], Control::Left, Control::Right) | hat(self.axis[AXIS_HAT_Y], Control::Up, Control::Down)
    }

    /// One more press of each control in `mask`.
    fn queue_mask(&mut self, mask: u16) {
        for (n, control) in self.queue.iter_mut().zip(Control::ALL) {
            if mask & control.mask() != 0 {
                *n = (*n + 1).min(QUEUE_MAX);
            }
        }
    }

    /// One press off the queue for every control that has one: the controls this report presses
    /// whether or not they are still down, which [`Pads::taps`] says afterwards.
    fn take_taps(&mut self) -> u16 {
        let mut taps = 0;
        for (n, control) in self.queue.iter_mut().zip(Control::ALL) {
            if *n > 0 {
                *n -= 1;
                taps |= control.mask();
            }
        }
        self.taps = taps;
        taps
    }

    /// The presses the last report was built with ([`Pads::queue`]): a control in it went down since
    /// the report before, even if it is up again. The menus read these as presses on their own, so
    /// a button let go and pressed again inside one frame still presses.
    pub fn taps(&self) -> u16 {
        self.taps
    }

    /// Forget every press waiting: a pad nobody reads this frame, whose presses would otherwise fire
    /// on the first menu after the ride.
    fn clear_queue(&mut self) {
        self.queue = [0; 16];
        self.taps = 0;
    }

    /// Read every joystick event waiting, updating the axis and button state.
    fn drain(&mut self) {
        use std::io::Read;
        // Out of `self` while it is read, so each event can go through [`Pads::event`].
        let Some(mut device) = self.device.take() else {
            return;
        };
        let mut event = [0u8; EVENT_BYTES];
        loop {
            match device.read(&mut event) {
                Ok(EVENT_BYTES) => {
                    let value = i16::from_le_bytes([event[4], event[5]]);
                    let init = event[6] & EVENT_INIT != 0;
                    let time = u32::from_le_bytes([event[0], event[1], event[2], event[3]]);
                    self.event(event[6] & !EVENT_INIT, event[7] as usize, value, init, time);
                }
                // A short read cannot happen on this device. End of file, or any error but "nothing
                // waiting", is a device that went away — the kernel answers ENODEV once a pad is
                // unplugged — and it is closed and said, so that [`Seats`] can pause its player and
                // hold the seat for it, rather than read a dead file for ever.
                Ok(0) => break,
                Ok(_) => {
                    self.device = Some(device);
                    return;
                }
                Err(e) if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
                {
                    self.device = Some(device);
                    return;
                }
                Err(_) => break,
            }
        }
        self.gone = true;
    }
}

/// A seat that reads no keys: every pad seat but the one the keyboard shares.
pub fn no_keys(_: u8) -> bool {
    false
}

// ---- seats ------------------------------------------------------------------------------------
//
// Taken from opentennis (P35): `Seat`, `readable`'s Steam filter and `ignore_list` are
// `crates/hst-base/src/seats.rs`'s, `PadSlots` and `Bindings` are `crates/hst-play/src/play/controls.rs`'s.
// What changed for a raw js device instead of gilrs is said where it changed; the journal
// (research/journal/2026-10-20-p35-p35-seats-and-pads) has the list.

/// How many seats there are: up to eight players on one machine.
pub const SEATS: usize = 8;

/// What drives one seat (`--pads`): the keyboard or a pad — an index into the pads in the order
/// they were found (the kernel's `js` numbers at startup, then the order they were plugged in).
/// A seat that is neither (`-`) is nobody's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Seat {
    Keys,
    Pad(usize),
}

impl Seat {
    /// `k` the keyboard, a number a pad, anything else (`-`) nobody.
    pub fn parse(s: &str) -> Option<Seat> {
        match s.trim() {
            "k" => Some(Seat::Keys),
            n => n.parse().ok().map(Seat::Pad),
        }
    }

    /// `--pads` for these seats (`-` nobody).
    pub fn flag(seats: &[Option<Seat>]) -> String {
        let one = |s: &Option<Seat>| match s {
            Some(Seat::Keys) => "k".to_string(),
            Some(Seat::Pad(n)) => n.to_string(),
            None => "-".into(),
        };
        seats.iter().map(one).collect::<Vec<_>>().join(",")
    }

    /// A whole `--pads` list: up to [`SEATS`] of them, comma-separated, the rest nobody's.
    pub fn parse_list(s: &str) -> Result<[Option<Seat>; SEATS], String> {
        let mut seats = [None; SEATS];
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() > SEATS {
            return Err(format!("{s}: {} seats, and there are {SEATS}", parts.len()));
        }
        for (seat, part) in seats.iter_mut().zip(&parts) {
            let part = part.trim();
            *seat = Seat::parse(part);
            if seat.is_none() && part != "-" {
                return Err(format!("{part}: a seat is k, a pad's number or -"));
            }
        }
        if seats.iter().filter(|s| **s == Some(Seat::Keys)).count() > 1 {
            return Err(format!("{s}: the keyboard can only be one seat"));
        }
        Ok(seats)
    }
}

/// What the kernel says a joystick device is: its name and the USB (or Bluetooth) ids, from sysfs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceInfo {
    pub name: String,
    pub vendor: u16,
    pub product: u16,
    /// The device's own serial, where it has one (a Bluetooth pad's address, most often).
    pub uniq: String,
    /// Where it is plugged in: the USB port path, for a pad that has no serial.
    pub phys: String,
    /// The keys the device says it has from `BTN_MISC` up and its axes, in the order the joystick
    /// interface numbers them ([`JsMap`]); empty for a node with no sysfs entry, which is taken to
    /// number them as an Xbox pad does.
    pub keys: Vec<u16>,
    pub abs: Vec<u16>,
    /// A motion sensor and not a pad (P53): the kernel flags it an accelerometer, or it is named as
    /// hid-nintendo names the second device it makes for a Switch pad ("… (IMU)"). Its axes never
    /// rest, so read as a pad it is a drifting stick, and it can take a seat before the real one.
    pub motion: bool,
    /// The kernel made it for a program, not for hardware: its sysfs node is under
    /// `/sys/devices/virtual/input/`, where uinput puts what it makes — a uinput pad (`ps2kit`'s,
    /// which calls itself an Xbox 360 pad with the real one's ids, another slot's, a remapper's) or
    /// Steam's own virtual pad (P46). A Bluetooth LE pad is under `/sys/devices/virtual/` too,
    /// through BlueZ's uhid, but under `misc/uhid/`: it is hardware, and is not this.
    pub virtual_device: bool,
}

/// The kernel's `BTN_SELECT` and `BTN_SOUTH`: a gamepad has the second, and the first if it has a
/// Select (or Back) button at all.
#[cfg(test)]
const BTN_SELECT: usize = 0x13a;
#[cfg(test)]
const BTN_SOUTH: usize = 0x130;

/// The input property a motion sensor carries, in a sysfs `properties` bitmap.
const INPUT_PROP_ACCELEROMETER: usize = 0x06;

/// Nintendo's USB vendor id, which a Switch pad (and an 8BitDo pad in its Switch mode) reports.
const NINTENDO: u16 = 0x057e;
/// Sony's, whose pads hid-playstation names by position as hid-nintendo does.
const SONY: u16 = 0x054c;

/// Whether `bitmap` — a sysfs `capabilities/key` (or `abs`, or `properties`), hex words most
/// significant first, each a C `long` — has bit `code` set.
fn has_key(bitmap: &str, code: usize) -> bool {
    let bits = usize::BITS as usize;
    let words: Vec<&str> = bitmap.split_whitespace().collect();
    let Some(at) = words.len().checked_sub(1 + code / bits) else { return false };
    u64::from_str_radix(words[at], 16).is_ok_and(|w| w >> (code % bits) & 1 != 0)
}

impl DeviceInfo {
    /// `sys/<node>/device/…`, as much of it as is there. A node with no sysfs entry (a test's fifo,
    /// or a container with no `/sys`) is known by its path alone.
    fn read(sys: &std::path::Path, node: &str) -> DeviceInfo {
        let dir = sys.join(node).join("device");
        let text = |name: &str| {
            std::fs::read_to_string(dir.join(name)).map(|t| t.trim().to_string()).unwrap_or_default()
        };
        let hex = |name: &str| u16::from_str_radix(&text(name), 16).unwrap_or(0);
        DeviceInfo {
            name: text("name"),
            vendor: hex("id/vendor"),
            product: hex("id/product"),
            uniq: text("uniq"),
            phys: text("phys"),
            keys: {
                let keys = text("capabilities/key");
                js_order(&keys, (BTN_JOYSTICK..=KEY_MAX).chain(BTN_MISC..BTN_JOYSTICK))
            },
            abs: js_order(&text("capabilities/abs"), 0..=ABS_MAX),
            motion: has_key(&text("properties"), INPUT_PROP_ACCELEROMETER) || {
                let name = text("name");
                name.ends_with(" (IMU)") || name.ends_with(" IMU")
            },
            virtual_device: std::fs::canonicalize(sys.join(node))
                .is_ok_and(|p| p.to_string_lossy().contains("/devices/virtual/input/")),
        }
    }

    /// A Nintendo pad: by its vendor id, or by its name for one that came through without ids.
    pub fn nintendo(&self) -> bool {
        self.vendor == NINTENDO || self.name.starts_with("Nintendo ")
    }

    /// How its js numbers land on the shell's slots ([`JsMap::of`]).
    fn map(&self) -> JsMap {
        JsMap::of(&self.keys, &self.abs, self.nintendo() || self.vendor == SONY)
    }

    /// Which pad this is, as far as the shell can tell one pad from another across an unplug: the
    /// same ids, serial, port and name. gilrs gives opentennis an entity per connection and a pad
    /// that comes back is a new one; a js device has no such handle either, and its `jsN` is
    /// handed out again to whichever pad comes next, so this is what a seat remembers instead.
    fn identity(&self, path: &std::path::Path) -> String {
        if self.name.is_empty() && self.vendor == 0 && self.product == 0 {
            return path.display().to_string();
        }
        format!(
            "{:04x}:{:04x} {} {} {}",
            self.vendor, self.product, self.uniq, self.phys, self.name
        )
    }
}

/// Whether the shell reads this pad. Under Steam Input, Steam lists the physical pads it stands in
/// for in `SDL_GAMECONTROLLER_IGNORE_DEVICES` (`0xVID/0xPID,…`); those are skipped so only Steam's
/// virtual pad, with the player's Steam layout, drives the game. Outside Steam Input, Steam's
/// background virtual pads (Valve 0x28de/0x11ff) mirror real ones and are skipped instead.
///
/// And every other virtual pad is skipped either way (P46): a uinput device is somebody's program
/// holding a pad open — the kit's own virtual pad for another slot's emulator, sat idle, is the one
/// this machine had — and it borrows a real pad's ids, so the id check alone let it in. Under Steam
/// Input, Steam's own (Valve's ids) is the one virtual pad read. `--pad` names a device outright and
/// is read whatever it is.
pub fn readable(info: &DeviceInfo) -> bool {
    static IGNORED: std::sync::OnceLock<Option<Vec<(u16, u16)>>> = std::sync::OnceLock::new();
    let ignored = IGNORED.get_or_init(|| {
        std::env::var("SDL_GAMECONTROLLER_IGNORE_DEVICES").ok().map(|v| ignore_list(&v))
    });
    readable_under(info, ignored.as_deref())
}

/// [`readable`] with the ignore list given, for the tests.
fn readable_under(info: &DeviceInfo, ignored: Option<&[(u16, u16)]>) -> bool {
    let id = (info.vendor, info.product);
    if info.motion {
        return false;
    }
    // joycond's pair of Joy-Cons is a uinput pad, and the one way two Joy-Cons are one controller.
    let joined_joycons = info.name == "Nintendo Switch Combined Joy-Cons";
    if info.virtual_device && !joined_joycons && !(ignored.is_some() && info.vendor == 0x28de) {
        return false;
    }
    match ignored {
        Some(list) => !list.contains(&id),
        None => id != (0x28de, 0x11ff),
    }
}

/// `SDL_GAMECONTROLLER_IGNORE_DEVICES`' `0xVID/0xPID` pairs; malformed entries are dropped.
fn ignore_list(v: &str) -> Vec<(u16, u16)> {
    let hex = |s: &str| {
        u16::from_str_radix(s.trim().trim_start_matches("0x").trim_start_matches("0X"), 16).ok()
    };
    v.split(',')
        .filter_map(|e| e.split_once('/'))
        .filter_map(|(a, b)| Some((hex(a)?, hex(b)?)))
        .collect()
}

/// The seats 1P..8P: which pad holds each, and which pad each one last had.
///
/// opentennis's, with two changes. It is generic over what names a pad (a [`DeviceInfo::identity`]
/// here, a gilrs entity there), and a seat **waits for its own pad**: opentennis frees a seat whose
/// pad is unplugged and hands it to the next pad plugged in, which a match between points can
/// afford. A ride cannot — the seat is a rider on the track, and a different pad taking it over
/// mid-ride would be somebody else's thumbs on that rider — so here a seat remembers its pad
/// (`last`), its player is paused while the pad is out ([`PadSlots::waiting`]), the same pad
/// takes it back when it is plugged in again, and a pad nobody has seen joins the first seat
/// nobody is waiting in. [`PadSlots::forget`] lets a waiting seat go.
#[derive(Clone, Debug)]
pub struct PadSlots<P> {
    /// The pad holding each seat now.
    pub held: [Option<P>; SEATS],
    /// `--pads`: what each seat was set up as, or `None` for "pads in join order, and the keyboard
    /// with 1P".
    pub seats: Option<[Option<Seat>; SEATS]>,
    /// The pad each seat had last, kept while it is unplugged.
    last: [Option<P>; SEATS],
    /// 1P (P46): the device the front end's splash took 1P from, which sits in the first seat and
    /// alone (with no `--pads`) decides whether the keyboard rides there. `None` before the splash
    /// and without the front end: the keyboard rides with 1P's pad, as before.
    first: Option<Device<P>>,
}

/// A device a seat can be given: the keyboard or a pad.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Device<P> {
    Keys,
    Pad(P),
}

impl<P> Default for PadSlots<P> {
    fn default() -> PadSlots<P> {
        PadSlots {
            held: std::array::from_fn(|_| None),
            seats: None,
            last: std::array::from_fn(|_| None),
            first: None,
        }
    }
}

impl<P: Clone + PartialEq> PadSlots<P> {
    /// Seats set up by `--pads`.
    pub fn with_seats(seats: [Option<Seat>; SEATS]) -> PadSlots<P> {
        PadSlots { seats: Some(seats), ..PadSlots::default() }
    }

    /// The seats start from 1P (P46): every seat let go, then 1P's pad in the first seat — or the
    /// first seat kept for the keyboard — and the other pads behind it in join order at the next
    /// [`PadSlots::update`]. Seats set up by `--pads` stay as they were set up; `None` (the splash is
    /// back) leaves the seats and lets the keyboard ride with 1P again.
    pub fn set_first(&mut self, first: Option<Device<P>>) {
        if self.seats.is_none() && first.is_some() {
            self.held = std::array::from_fn(|_| None);
            self.last = std::array::from_fn(|_| None);
            if let Some(Device::Pad(p)) = &first {
                self.held[0] = Some(p.clone());
                self.last[0] = Some(p.clone());
            }
        }
        self.first = first;
    }

    /// 1P's device, once the front end's splash has one.
    pub fn first(&self) -> Option<&Device<P>> {
        self.first.as_ref()
    }

    /// `present` is every pad plugged in, in the order they were found. Drop the pads gone from it
    /// (their seats wait), give each one that came back its own seat, then a seat set up for pad `n`
    /// pad `n`, then every other new pad the first free seat that wants one, in join order.
    pub fn update(&mut self, present: &[P]) {
        for (held, last) in self.held.iter_mut().zip(&mut self.last) {
            if held.as_ref().is_some_and(|p| !present.contains(p)) {
                *last = held.take();
            }
        }
        // Home first, so a pad back from an unplug is never beaten to its seat by one plugged in
        // beside it.
        for i in 0..SEATS {
            if self.held[i].is_none()
                && let Some(p) = self.last[i].clone()
                && present.contains(&p)
                && !self.held.contains(&Some(p.clone()))
            {
                self.held[i] = Some(p);
            }
        }
        let keys_first = self.seats.is_none() && self.first == Some(Device::Keys);
        let wants = |i: usize| {
            if i == 0 && keys_first {
                return None;
            }
            self.seats.map_or(Some(None), |seats| match seats[i] {
                Some(Seat::Pad(n)) => Some(Some(n)),
                _ => None,
            })
        };
        // Then the seats' own pads, so a seat whose pad is not in yet doesn't take one another empty
        // seat is waiting for. Only a seat that has never had a pad: after that, `last` says which.
        for i in 0..SEATS {
            if let (None, None, Some(Some(n))) = (&self.held[i], &self.last[i], wants(i))
                && let Some(p) = present.get(n).filter(|p| !self.claimed(p))
            {
                self.held[i] = Some(p.clone());
                self.last[i] = Some(p.clone());
            }
        }
        for i in 0..SEATS {
            if self.held[i].is_some() || self.last[i].is_some() || wants(i).is_none() {
                continue;
            }
            let own: Vec<&P> = (0..SEATS)
                .filter(|&j| j != i && self.held[j].is_none() && self.last[j].is_none())
                .filter_map(|j| wants(j).flatten())
                .filter_map(|n| present.get(n))
                .collect();
            let free = present
                .iter()
                .find(|p| !self.claimed(p) && (self.seats.is_none() || !own.contains(p)))
                .cloned();
            if free.is_some() {
                self.last[i] = free.clone();
            }
            self.held[i] = free;
        }
    }

    /// Whether a pad already has a seat, or is the pad a waiting seat is waiting for.
    fn claimed(&self, p: &P) -> bool {
        self.held.iter().chain(&self.last).any(|q| q.as_ref() == Some(p))
    }

    /// Whether seat `i`'s pad is out: its player is paused until the pad is plugged back in or the
    /// seat is let go.
    pub fn waiting(&self, i: usize) -> bool {
        self.held[i].is_none() && self.last[i].is_some()
    }

    /// Let seat `i` go: it holds no pad and waits for none, so the next new pad may take it.
    pub fn forget(&mut self, i: usize) {
        self.held[i] = None;
        self.last[i] = None;
    }

    /// The seat the keyboard drives: 1P without `--pads` (alongside 1P's pad), else its own seat if
    /// it has one; with none, 1P while 1P's pad is out, so the keyboard can play on (opentennis's
    /// B388).
    ///
    /// With a 1P from the splash and no `--pads` (P46): 1P when the keyboard is 1P, and otherwise no
    /// seat — the keyboard does not move a pad's player — except 1P's while its pad is out, the same
    /// take-over.
    pub fn keys(&self) -> Option<usize> {
        if self.seats.is_none()
            && let Some(Device::Pad(_)) = &self.first
        {
            return self.held[0].is_none().then_some(0);
        }
        self.seats.map_or(Some(0), |seats| {
            seats.iter().position(|s| *s == Some(Seat::Keys)).or(
                (matches!(seats[0], Some(Seat::Pad(_))) && self.held[0].is_none()).then_some(0),
            )
        })
    }
}

/// One binding: a key (an X keycode) or a pad button (a js button index).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Input {
    Key(u8),
    Button(usize),
}

/// The keyboard's stick keys, as `controls.txt` names them, in [`KEY_STICK_SLOTS`]' order: each
/// axis' low key, then its high key. `stick_*` are the right stick's, as they were before the left
/// stick had keys (P54).
const STICK_KEYS: [&str; 8] = [
    "stick_left", "stick_right", "stick_up", "stick_down",
    "left_stick_left", "left_stick_right", "left_stick_up", "left_stick_down",
];

/// How many lines `controls.txt` has: the sixteen controls, then the eight stick keys.
const BINDINGS: usize = 16 + STICK_KEYS.len();

/// Which keys and pad buttons are which of the game's controls: `controls.txt`, opentennis's way.
///
/// One line per control, `control = Name Name …`, named as the port names them ([`Control::name`]):
/// keys by name (`X`, `Space`, `Return`, `Up`, `F1`, …, [`KEY_NAMES`]) and pad buttons with a `Pad`
/// prefix (`PadSouth`, `PadL1`, `Pad7`, … — [`BUTTON_NAMES`], or a js button number). Unknown names
/// are reported and skipped; a control left with nothing understood keeps its default.
///
/// What changed from opentennis for a raw js device: its buttons are gilrs' named ones, and a js
/// device's are only numbers, in the order the kernel gives a gamepad (see [`PAD_BUTTONS`]), so
/// the names here are that order's and `PadN` reaches any button past them. Its actions are the
/// game's (tennis shots there, the sixteen DualShock 2 controls here), and the d-pad hat, the
/// sticks and the triggers are axes on a js device and are not rebindable, as opentennis's sticks
/// are not. The gas line's keys are the keyboard's throttle ramp ([`KeyRamp`]) rather than a bit:
/// `cross`'s under the original's layout, `r2`'s under the new one ([`Bindings::gas`]).
///
/// The keyboard's defaults depend on the seat's layout (P54): the original's is the shell's binding
/// from before there was a file ([`KEYBOARD`], `X`/space the throttle, `IJKL` the right stick), and
/// the new one's the usual one ([`KEYBOARD_NEW`], `WASD` and `YUIO` the sticks, J the gas and K the
/// brake, the face buttons beside them). A
/// line the file has is that line under both layouts. The pad buttons' defaults are the same under
/// both ([`PAD_BUTTONS`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Bindings {
    /// Every line under each layout, in [`Scheme::ALL`]'s order: the file's where it has the line,
    /// and that layout's default where it does not.
    lines: [[Vec<Input>; BINDINGS]; 2],
    /// Which lines the file set, which [`Bindings::text`] writes out as lines; the rest it writes
    /// as comments, each layout's default.
    set: [bool; BINDINGS],
}

impl Default for Bindings {
    /// Each layout's own defaults and nothing from a file.
    fn default() -> Bindings {
        Bindings { lines: Scheme::ALL.map(Bindings::defaults), set: [false; BINDINGS] }
    }
}

/// Where a layout is in [`Scheme::ALL`], which is where its lines are in a [`Bindings`].
fn layout_at(scheme: Scheme) -> usize {
    Scheme::ALL.iter().position(|s| *s == scheme).unwrap_or(0)
}

/// Every key a binding may name, by X keycode (the evdev code plus 8, which is what an X server on
/// Linux hands over). Escape is not here: it quits.
const KEY_NAMES: &[(&str, u8)] = &[
    ("Q", 24), ("W", 25), ("E", 26), ("R", 27), ("T", 28), ("Y", 29), ("U", 30), ("I", 31),
    ("O", 32), ("P", 33), ("A", 38), ("S", 39), ("D", 40), ("F", 41), ("G", 42), ("H", 43),
    ("J", 44), ("K", 45), ("L", 46), ("Z", 52), ("X", 53), ("C", 54), ("V", 55), ("B", 56),
    ("N", 57), ("M", 58), ("1", 10), ("2", 11), ("3", 12), ("4", 13), ("5", 14), ("6", 15),
    ("7", 16), ("8", 17), ("9", 18), ("0", 19), ("Minus", 20), ("Equal", 21), ("BackSpace", 22),
    ("Tab", 23), ("BracketLeft", 34), ("BracketRight", 35), ("Return", 36), ("ControlLeft", 37),
    ("Semicolon", 47), ("Quote", 48), ("Backquote", 49), ("ShiftLeft", 50), ("Backslash", 51),
    ("Comma", 59), ("Period", 60), ("Slash", 61), ("ShiftRight", 62), ("AltLeft", 64),
    ("Space", 65), ("F1", 67), ("F2", 68), ("F3", 69), ("F4", 70), ("F5", 71), ("F6", 72),
    ("F7", 73), ("F8", 74), ("F9", 75), ("F10", 76), ("F11", 95), ("F12", 96),
    ("ControlRight", 105), ("AltRight", 108), ("Home", 110), ("Up", 111), ("PageUp", 112),
    ("Left", 113), ("Right", 114), ("End", 115), ("Down", 116), ("PageDown", 117),
    ("Insert", 118), ("Delete", 119),
];

/// The js button indices by name, in the order Linux reports a modern gamepad's buttons
/// ([`PAD_BUTTONS`]).
const BUTTON_NAMES: [&str; BUTTONS] = [
    "South", "East", "West", "North", "L1", "R1", "Back", "Start", "Mode", "L3", "R3", "11",
];

impl Input {
    fn parse(name: &str) -> Option<Input> {
        if let Some(b) = name.strip_prefix("Pad") {
            let named = BUTTON_NAMES.iter().position(|n| n.eq_ignore_ascii_case(b));
            return named
                .or_else(|| b.parse().ok().filter(|&n: &usize| n < BUTTONS))
                .map(Input::Button);
        }
        let name = name.strip_prefix("Key").filter(|k| !k.is_empty()).unwrap_or(name);
        let name = if name.eq_ignore_ascii_case("Enter") { "Return" } else { name };
        KEY_NAMES
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|&(_, code)| Input::Key(code))
    }

    fn name(self) -> String {
        match self {
            Input::Key(code) => KEY_NAMES
                .iter()
                .find(|(_, c)| *c == code)
                .map_or(format!("Key{code}"), |(n, _)| n.to_string()),
            Input::Button(i) => format!("Pad{}", BUTTON_NAMES[i]),
        }
    }
}

impl Bindings {
    /// The name of line `i`.
    fn line_name(i: usize) -> &'static str {
        if i < 16 { Control::ALL[i].name() } else { STICK_KEYS[i - 16] }
    }

    /// Every line's default under `scheme`: the keyboard's for that layout, then [`PAD_BUTTONS`].
    fn defaults(scheme: Scheme) -> [Vec<Input>; BINDINGS] {
        let mut b: [Vec<Input>; BINDINGS] = std::array::from_fn(|_| Vec::new());
        let (keys, sticks): (&[(u8, Control)], &[(usize, u8, u8)]) = match scheme {
            Scheme::Original => (&KEYBOARD, &KEY_STICKS),
            Scheme::New => (&KEYBOARD_NEW, &KEY_STICKS_NEW),
        };
        for &(code, control) in keys {
            b[control as usize].push(Input::Key(code));
        }
        if scheme == Scheme::Original {
            b[Control::Cross as usize].extend([Input::Key(key::X), Input::Key(key::SPACE)]);
        }
        for (i, control) in PAD_BUTTONS.iter().enumerate() {
            if let Some(control) = control {
                b[*control as usize].push(Input::Button(i));
            }
        }
        for &(slot, low, high) in sticks {
            let i = KEY_STICK_SLOTS.iter().position(|s| *s == slot).expect("a stick slot");
            b[16 + 2 * i].push(Input::Key(low));
            b[16 + 2 * i + 1].push(Input::Key(high));
        }
        b
    }

    /// The control whose keys are the keyboard's throttle ramp under `scheme`: the gas, as the
    /// pad's report carries it before [`Scheme::ride`] — Cross under the original's, R2 under the
    /// new one.
    pub fn gas(scheme: Scheme) -> Control {
        match scheme {
            Scheme::New => Control::R2,
            Scheme::Original => Control::Cross,
        }
    }

    /// Whether `set`, a line `i` read from a file, is only what the shell wrote there before the
    /// layouts had keyboards of their own (P54): the original's default, or the line as it was
    /// before P40 moved js button 6 from Start to Select. A file the shell wrote on its first run has
    /// every line, and if they counted as the player's they would pin the original's keys under the
    /// new layout too; so they are read as no line at all, and each layout keeps its default.
    fn as_written(i: usize, set: &[Input]) -> bool {
        let same = |b: &[Input]| b.len() == set.len() && b.iter().all(|x| set.contains(x));
        let before_p40: &[Input] = match i {
            i if i == Control::Select as usize => &[Input::Key(key::TAB)],
            i if i == Control::Start as usize => &[Input::Key(key::RETURN), Input::Button(6), Input::Button(7)],
            _ => &[],
        };
        same(&Bindings::defaults(Scheme::Original)[i]) || (!before_p40.is_empty() && same(before_p40))
    }

    /// Read `text` over the defaults; returns the bindings and a message per name that wasn't
    /// understood.
    pub fn parse(text: &str) -> (Bindings, Vec<String>) {
        let mut b = Bindings::default();
        let mut errors = Vec::new();
        for line in text
            .lines()
            .map(|l| l.split('#').next().unwrap_or("").trim())
            .filter(|l| !l.is_empty())
        {
            let Some((action, names)) = line.split_once('=') else {
                errors.push(format!("not `control = keys`: {line}"));
                continue;
            };
            let Some(i) = (0..BINDINGS).position(|i| Bindings::line_name(i) == action.trim()) else {
                errors.push(format!("unknown control: {}", action.trim()));
                continue;
            };
            let set: Vec<Input> = names
                .split_whitespace()
                .filter_map(|n| {
                    Input::parse(n).or_else(|| {
                        errors.push(format!("unknown key or button: {n}"));
                        None
                    })
                })
                .collect();
            if !set.is_empty() && !Bindings::as_written(i, &set) {
                for lines in &mut b.lines {
                    lines[i] = set.clone();
                }
                b.set[i] = true;
            }
        }
        (b, errors)
    }

    /// The file text for these bindings.
    pub fn text(&self) -> String {
        let mut s = String::from(
            "# openmx controls. One line per control of the game's pad: control = Key Key PadButton ...\n\
             # Keys by name (X, Space, Return, Up, ShiftLeft, F1, ...), pad buttons with a Pad prefix\n\
             # (PadSouth, PadEast, PadWest, PadNorth, PadL1, PadR1, PadBack, PadStart, PadL3, PadR3, or\n\
             # Pad0 to Pad11). stick_* are the right stick's keys, left_stick_* the left stick's.\n\
             # The sticks, the triggers (l2, r2) and the d-pad hat are axes and always do what they do;\n\
             # Escape is the pause (it quits without the menus), F3 the overlay (and H, where H is\n\
             # not a key of the ride) and M the machine swap.\n\
             # A ride reads these through the seat's scheme (--controls new|original, P40). Under the\n\
             # new one r2 is the gas, l2 the brake and select the camera, and cross and square do\n\
             # nothing in a ride: they are kept free for later. Under the original's, cross is the\n\
             # gas, square the brake, l1 the clutch and select the camera, as on the disc. The gas\n\
             # line's keys (r2's under the new one, cross's under the original's) are the throttle,\n\
             # ramped.\n\
             # The keyboard's defaults differ by layout (P54): the new one's is WASD the left stick,\n\
             # YUIO the right (Y left, U down, I up, O right), J the gas and K the brake (Space and\n\
             # Shift too), H square, L circle, G cross, ; triangle, Q/E L1/R1, Tab or C the camera;\n\
             # the original's is the shell's old one. A commented line is the default; a line written\n\
             # out is that line under both layouts. A line exactly as the shell once wrote it out\n\
             # (the original's default) counts as no line, so an old file keeps each layout's keys.\n",
        );
        let words = |set: &[Input]| set.iter().map(|x| x.name()).collect::<Vec<_>>().join(" ");
        for i in 0..BINDINGS {
            let name = Bindings::line_name(i);
            let [new, original] = [&self.lines[0][i], &self.lines[1][i]];
            if self.set[i] {
                s += &format!("{name} = {}\n", words(new));
            } else if new == original {
                s += &format!("# {name} = {}\n", words(new));
            } else {
                s += &format!("# {name} = {}  # new\n", words(new));
                s += &format!("# {name} = {}  # original\n", words(original));
            }
        }
        s
    }

    /// Read `path`, or write the defaults there if there is nothing to read; messages for whatever
    /// in it was not understood, or for a file that could not be written, come back beside it.
    pub fn load(path: &std::path::Path) -> (Bindings, Vec<String>) {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let (b, errors) = Bindings::parse(&text);
                let errors = errors.into_iter().map(|e| format!("{}: {e}", path.display())).collect();
                (b, errors)
            }
            Err(_) => {
                let b = Bindings::default();
                // Written beside and renamed over, so a session reading it never sees half a file.
                let partial = path.with_extension("txt.partial");
                let wrote = std::fs::write(&partial, b.text()).and_then(|_| std::fs::rename(&partial, path));
                let said = match wrote {
                    Ok(()) => format!("wrote the default controls to {}", path.display()),
                    Err(e) => format!("{}: {e} (riding on the default controls)", path.display()),
                };
                (b, vec![said])
            }
        }
    }

    /// The game's controls held on the pad's buttons and the keyboard's keys under `scheme`, as its
    /// bitmask. The gas on a key is not here: that is the throttle ramp's ([`Bindings::throttle_key`]).
    fn buttons(&self, scheme: Scheme, button: &[bool; BUTTONS], held: &dyn Fn(u8) -> bool) -> u16 {
        let mut mask = 0;
        let gas = Bindings::gas(scheme);
        for (control, set) in Control::ALL.iter().zip(&self.lines[layout_at(scheme)]) {
            let down = set.iter().any(|x| match *x {
                Input::Button(i) => button[i],
                Input::Key(code) => *control != gas && held(code),
            });
            if down {
                mask |= control.mask();
            }
        }
        mask
    }

    /// Whether a throttle key — a key on `scheme`'s gas line ([`Bindings::gas`]) — is down.
    fn throttle_key(&self, scheme: Scheme, held: &dyn Fn(u8) -> bool) -> bool {
        self.key_on(scheme, Bindings::gas(scheme), held)
    }

    /// The keyboard stick axis `i`'s two keys under `scheme`, low and high: whether any key on each
    /// is down.
    fn stick_keys(&self, scheme: Scheme, i: usize, held: &dyn Fn(u8) -> bool) -> (bool, bool) {
        let lines = &self.lines[layout_at(scheme)];
        let any = |line: usize| lines[line].iter().any(|x| matches!(x, Input::Key(c) if held(*c)));
        (any(16 + 2 * i), any(16 + 2 * i + 1))
    }

    /// Whether any key bound to `control` under `scheme` is down: the keyboard's own Start, for
    /// [`Seats`]' take-over.
    fn key_on(&self, scheme: Scheme, control: Control, held: &dyn Fn(u8) -> bool) -> bool {
        self.lines[layout_at(scheme)][control as usize].iter().any(|x| matches!(x, Input::Key(c) if held(*c)))
    }

    /// Whether `code` is one of the keyboard's keys under `scheme`, on any line: the shell's own
    /// keys (the overlay's H) stand aside for a key the ride reads.
    pub fn binds_key(&self, scheme: Scheme, code: u8) -> bool {
        self.lines[layout_at(scheme)].iter().flatten().any(|x| *x == Input::Key(code))
    }

    /// The keyboard's layout under `scheme`, as the Controls screen lists it (P54): what each key
    /// does in a ride, as `(what, keys)` pairs, from these bindings — so a `controls.txt` that moved
    /// a key shows where it moved to. The pad's buttons are not here.
    pub fn keyboard_layout(&self, scheme: Scheme) -> Vec<(String, String)> {
        self.keyboard_layout_on(scheme, &Table::SHIPPED)
    }

    /// [`Bindings::keyboard_layout`] with the commands on a controller preset's table (P79).
    pub fn keyboard_layout_on(&self, scheme: Scheme, table: &Table) -> Vec<(String, String)> {
        let lines = &self.lines[layout_at(scheme)];
        let keys = |line: usize| -> Vec<String> {
            let mut out: Vec<String> = Vec::new();
            for x in &lines[line] {
                if let Input::Key(code) = *x {
                    let name = key_label(Input::Key(code).name());
                    if !out.contains(&name) {
                        out.push(name);
                    }
                }
            }
            out
        };
        let of = |controls: &[Control]| controls.iter().flat_map(|c| keys(*c as usize)).collect::<Vec<_>>().join(" ");
        // A stick's four keys, and the d-pad's, in the order a reader expects them: up, left, down,
        // right — and the four arrows as one word.
        let stick = |first: usize| [first + 2, first, first + 3, first + 1].into_iter().flat_map(keys).collect::<Vec<_>>().join(" ");
        let dpad = match of(&[Control::Up, Control::Left, Control::Down, Control::Right]) {
            arrows if arrows == "Up Left Down Right" => "Arrows".to_string(),
            other => other,
        };
        let row = |what: &str, keys: String| (what.to_string(), keys);
        // The original's commands by their names (P73), each on the control the scheme puts it on;
        // the d-pad's four as one row, the reset chord's two halves as one, the pause with the
        // shell's own Escape beside it.
        let commands = scheme.commands_on(table);
        let on = |c: Command| commands.iter().find(|(x, _)| *x == c).map(|(_, control)| *control);
        let named = |c: Command| {
            let name = c.name().unwrap_or("");
            let mut chars = name.chars();
            chars.next().map(|f| f.to_uppercase().chain(chars).collect::<String>()).unwrap_or_default()
        };
        let mut out = Vec::new();
        for c in [Command::GAS, Command::BRAKE, Command::CLUTCH, Command::TRICK_1, Command::TRICK_2, Command::TRICK_3] {
            if let Some(control) = on(c) {
                out.push(row(&named(c), of(&[control])));
            }
        }
        out.push(row("Left stick", stick(16 + 4)));
        out.push(row("Right stick", stick(16)));
        out.push(row("D-pad", dpad));
        // The look-back hold beside the camera (P78); the other camera has no row while the shell
        // has no camera for it.
        for c in [Command::NEXT_CAMERA, Command::LOOK_BACK] {
            if let Some(control) = on(c) {
                out.push(row(&named(c), of(&[control])));
            }
        }
        if let (Some(l), Some(r)) = (on(Command::RESET_LEFT), on(Command::RESET_RIGHT)) {
            out.push(row("Reset", format!("{} + {}", of(&[l]), of(&[r]))));
        }
        let pause = on(Command::PAUSE).map(|c| of(&[c])).unwrap_or_default();
        out.push(row("Pause", format!("{pause} Escape").trim().to_string()));
        out.retain(|(_, k)| !k.trim().is_empty());
        out
    }
}

/// A key's name as the Controls screen shows it: the two Shifts (and Controls, and Alts) are one
/// key to a reader, Return is the Enter key, and Semicolon is `;` (the new layout's Triangle, P80).
fn key_label(name: String) -> String {
    let name = ["Left", "Right"]
        .iter()
        .find_map(|side| {
            name.strip_suffix(side).filter(|k| ["Shift", "Control", "Alt"].contains(k)).map(str::to_string)
        })
        .unwrap_or(name);
    match name.as_str() {
        "Return" => "Enter".to_string(),
        "Semicolon" => ";".to_string(),
        _ => name,
    }
}

/// What a seat gives the frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeatInput {
    /// Nobody is in it.
    Empty,
    /// Its player's pad report this frame.
    Playing(Report),
    /// Its pad was unplugged: the player is paused until it is back (or the keyboard takes over).
    Waiting,
}

impl SeatInput {
    /// The report the engine's port is handed: nobody's and a waiting seat's are a pad that is not
    /// there, the same bytes the shell hands a second port it has no player for.
    pub fn report(self) -> Report {
        match self {
            SeatInput::Playing(report) => report,
            _ => Report::missing(),
        }
    }
}

/// A pad plugged in: its device's state and who it is.
struct Joined {
    path: std::path::PathBuf,
    id: String,
    pads: Pads,
}

/// How often the shell looks for pads it has not opened yet. A frame's worth of `read_dir` is
/// cheap, but nobody notices half a second either way when they plug a pad in.
const RESCAN_SECONDS: f32 = 0.5;

/// Every seat, and every pad plugged in: the hot-plug half of opentennis's controls over the
/// kernel's `/dev/input/js*`.
///
/// gilrs tells opentennis when a pad connects and disconnects. A js device says neither: a pad
/// plugged in is a `jsN` node that was not there before, so the directory is looked at again every
/// [`RESCAN_SECONDS`]; a pad unplugged is a read that fails (ENODEV), and its seat waits
/// ([`PadSlots`]) for a node with the same [`DeviceInfo::identity`] to turn up.
pub struct Seats {
    dir: std::path::PathBuf,
    sys: std::path::PathBuf,
    /// `--pad`: read this one device and nothing else.
    only: Option<std::path::PathBuf>,
    /// Every pad open, in the order found.
    joined: Vec<Joined>,
    pub slots: PadSlots<String>,
    /// The keyboard's own state, for a seat it drives with no pad beside it.
    keys: Pads,
    wiring: Wiring,
    bindings: Bindings,
    since_scan: Option<std::time::Instant>,
    /// What each seat was last frame, for [`Seats::said`].
    was: [SeatKind; SEATS],
    /// Last frame's keyboard Start, so a take-over is a press and not a hold.
    start_key: bool,
    /// The keyboard's own report when it drives no seat ([`Seats::keyboard_alone`]).
    keyboard_alone: Option<Report>,
    /// What changed, for the shell to print: pads joining, going and coming back.
    said: Vec<String>,
    /// The js devices plugged in and not read — virtual pads, motion sensors, Steam's stand-ins —
    /// by name, as the last scan found them, for the splash to say (P53).
    skipped: Vec<String>,
    /// Each seat's presses this frame ([`Pads::taps`]), beside its report.
    taps: [u16; SEATS],
}

/// [`SeatInput`] without its report, to say when a seat changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SeatKind {
    Empty,
    Playing,
    Waiting,
}

impl Seats {
    /// The host's seats: every readable `/dev/input/js*` (or only `--pad`'s), on `--pads`' seats
    /// or in join order, wired and bound as asked.
    pub fn open(
        only: Option<&str>,
        seats: Option<[Option<Seat>; SEATS]>,
        wiring: Wiring,
        bindings: Bindings,
    ) -> Seats {
        Seats::in_dir(PAD_DIR.as_ref(), PAD_SYS.as_ref(), only, seats, wiring, bindings)
    }

    /// [`Seats::open`] over another device directory and sysfs, for the tests.
    fn in_dir(
        dir: &std::path::Path,
        sys: &std::path::Path,
        only: Option<&str>,
        seats: Option<[Option<Seat>; SEATS]>,
        wiring: Wiring,
        bindings: Bindings,
    ) -> Seats {
        let mut seats = Seats {
            dir: dir.to_path_buf(),
            sys: sys.to_path_buf(),
            only: only.map(Into::into),
            joined: Vec::new(),
            slots: seats.map_or_else(PadSlots::default, PadSlots::with_seats),
            keys: Pads::bound(wiring.clone(), bindings.clone()),
            wiring,
            bindings,
            since_scan: None,
            was: [SeatKind::Empty; SEATS],
            start_key: false,
            keyboard_alone: None,
            said: Vec::new(),
            skipped: Vec::new(),
            taps: [0; SEATS],
        };
        seats.scan();
        seats
    }

    /// Open every js node not open yet, in `js` number order.
    fn scan(&mut self) {
        self.since_scan = Some(std::time::Instant::now());
        let mut found: Vec<(u32, std::path::PathBuf)> = match &self.only {
            Some(path) => vec![(0, path.clone())],
            None => std::fs::read_dir(&self.dir)
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|e| {
                    let name = e.file_name().into_string().ok()?;
                    let n = name.strip_prefix("js")?.parse().ok()?;
                    Some((n, e.path()))
                })
                .collect(),
        };
        found.sort();
        self.skipped.clear();
        for (_, path) in found {
            if self.joined.iter().any(|j| j.path == path) {
                continue;
            }
            let node = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            let info = DeviceInfo::read(&self.sys, node);
            if self.only.is_none() && !readable(&info) {
                self.skipped.push(match info.name.as_str() {
                    "" => path.display().to_string(),
                    name => name.to_string(),
                });
                continue;
            }
            use std::os::unix::fs::OpenOptionsExt;
            let Ok(file) = std::fs::OpenOptions::new().read(true).custom_flags(O_NONBLOCK).open(&path)
            else {
                continue;
            };
            // Two of the same pad on one hub port must still be two: the first free of `id`,
            // `id #2`, `id #3`…, so a pad that comes back gets the name it had if it is free.
            let base = info.identity(&path);
            let id = (1..)
                .map(|n| if n == 1 { base.clone() } else { format!("{base} #{n}") })
                .find(|id| !self.joined.iter().any(|j| &j.id == id))
                .expect("some name is free");
            let mut pads = Pads::bound(self.wiring.clone(), self.bindings.clone());
            pads.name = Some(match info.name.as_str() {
                "" => path.display().to_string(),
                name => format!("{name} ({})", path.display()),
            });
            pads.device = Some(file);
            pads.map = info.map();
            pads.nintendo = info.nintendo();
            self.joined.push(Joined { path, id, pads });
        }
    }

    /// Rewire every pad and the keyboard, which the front end's Controls screen does between frames
    /// (P36).
    pub fn set_wiring(&mut self, wiring: Wiring) {
        self.keys.set_wiring(wiring.clone());
        for j in &mut self.joined {
            j.pads.set_wiring(wiring.clone());
        }
        self.wiring = wiring;
    }

    /// Which scheme seat `seat` rides on.
    pub fn scheme(&self, seat: usize) -> Scheme {
        self.wiring.schemes[seat]
    }

    /// How seat `seat`'s biplane stick reads (P50, P69).
    pub fn plane(&self, seat: usize) -> PlaneScheme {
        self.wiring.planes[seat]
    }

    /// The pad holding each seat now, by name, for the startup lines and the overlay.
    pub fn name(&self, seat: usize) -> Option<&str> {
        let id = self.slots.held[seat].as_ref()?;
        self.joined.iter().find(|j| &j.id == id)?.pads.name.as_deref()
    }

    /// What changed since the last time this was asked: one line per pad joining, going or back.
    pub fn said(&mut self) -> Vec<String> {
        std::mem::take(&mut self.said)
    }

    /// Read every pad, and notice the ones that went and the ones that came.
    fn poll(&mut self) {
        for j in &mut self.joined {
            j.pads.drain();
        }
        self.joined.retain(|j| !j.pads.gone);
        if self.since_scan.is_none_or(|t| t.elapsed().as_secs_f32() >= RESCAN_SECONDS) {
            self.scan();
        }
        let present: Vec<String> = self.joined.iter().map(|j| j.id.clone()).collect();
        self.slots.update(&present);
    }

    /// A menu frame (P46): every device's own report, the keyboard's first and then each pad
    /// plugged in by name, so the front end can tell who pressed. It reads the pads as
    /// [`Seats::frame`] does, and a frame is one or the other.
    ///
    /// Each comes with its presses since the last frame ([`Pads::taps`]), which the report also
    /// holds down: a press and release inside one slow frame still reaches the menus (P53).
    pub fn devices(&mut self, held: &dyn Fn(u8) -> bool) -> Vec<(Device<String>, Report, u16)> {
        self.poll();
        self.keyboard_alone = None;
        self.keys.scheme = self.keys_scheme();
        let keys = self.keys.report(held);
        let mut out = vec![(Device::Keys, keys, self.keys.taps())];
        for j in &mut self.joined {
            let report = j.pads.report(&no_keys);
            out.push((Device::Pad(j.id.clone()), report, j.pads.taps()));
        }
        out
    }

    /// The js devices plugged in that the shell does not read, by name (P53): a wrong 1P on the
    /// splash is often a pad the player expected to be read and is not.
    pub fn skipped(&self) -> &[String] {
        &self.skipped
    }

    /// Seat `seat`'s presses this frame ([`Pads::taps`]): what [`Seats::frame`]'s report for it was
    /// built with, for the pause, which reads them as presses on their own (P53).
    pub fn taps(&self, seat: usize) -> u16 {
        self.taps.get(seat).copied().unwrap_or(0)
    }

    /// The seats start from 1P, the device the splash took it from ([`PadSlots::set_first`]).
    pub fn set_first(&mut self, first: Option<Device<String>>) {
        self.slots.set_first(first);
        self.was = [SeatKind::Empty; SEATS];
    }

    /// 1P's device, once the splash has one; the keyboard after it took a ride over from 1P's pad.
    pub fn first(&self) -> Option<&Device<String>> {
        self.slots.first()
    }

    /// A pad plugged in, by the name [`Seats::devices`] gives it, as the startup lines say it.
    pub fn device_name(&self, id: &str) -> Option<&str> {
        self.joined.iter().find(|j| j.id == id)?.pads.name.as_deref()
    }

    /// The layout the keyboard's keys are read under: its seat's, or 1P's when it drives no seat.
    fn keys_scheme(&self) -> Scheme {
        self.wiring.schemes[self.slots.keys().unwrap_or(0)]
    }

    /// Whether `code` is a key the keyboard rides with, under the layout its seat rides: the
    /// shell's own keys (the overlay's H, which the new layout's right stick has) stand aside for
    /// it (P54).
    pub fn binds_key(&self, code: u8) -> bool {
        self.bindings.binds_key(self.keys_scheme(), code)
    }

    /// The keyboard's layout under `scheme`, for the Controls screen ([`Bindings::keyboard_layout`]).
    pub fn keyboard_layout(&self, scheme: Scheme) -> Vec<(String, String)> {
        self.bindings.keyboard_layout(scheme)
    }

    /// The keyboard's layout under `scheme` on a controller preset (P79), for the Controls screen.
    pub fn keyboard_layout_on(&self, scheme: Scheme, preset: usize) -> Vec<(String, String)> {
        self.bindings.keyboard_layout_on(scheme, &Table::built([scheme.preset(preset), 0]))
    }

    /// Seat `seat`'s controller preset (P79), as its layout rides it: 0 under the new layout.
    pub fn preset(&self, seat: usize) -> usize {
        self.wiring.schemes[seat].preset(self.wiring.presets[seat])
    }

    /// The keyboard's report this frame when it drives no seat — a pad is 1P — for the pause, which
    /// the keyboard still opens and works whoever has 1P (P46). `None` when it rides in a seat,
    /// whose report already has it.
    pub fn keyboard_alone(&self) -> Option<Report> {
        self.keyboard_alone
    }

    /// This frame: read every pad, notice the ones that went and the ones that came, and build each
    /// seat's report. `held` is the keyboard.
    pub fn frame(&mut self, held: &dyn Fn(u8) -> bool) -> [SeatInput; SEATS] {
        self.poll();

        // The keyboard can take a waiting seat over: Start on its keys lets the seat's pad go and
        // rides on, as opentennis's keyboard plays on for a 1P whose pad is out.
        let keyboard = self.slots.keys();
        let start = self.bindings.key_on(self.keys_scheme(), Control::Start, held);
        if let Some(i) = keyboard
            && start
            && !self.start_key
            && self.slots.waiting(i)
        {
            self.slots.forget(i);
            if i == 0 && matches!(self.slots.first(), Some(Device::Pad(_))) {
                // The keyboard is 1P now, and the pad, back, joins as anyone new does; the other
                // seats stay as they are.
                self.slots.first = Some(Device::Keys);
            }
            self.said.push(format!("{}P: on the keyboard now", i + 1));
            self.was[i] = SeatKind::Playing;
            // Not this frame's Start as well: the same press would freeze the ride it just resumed.
            self.start_key = start;
            return self.frame_reports(keyboard, &no_keys);
        }
        self.start_key = start;
        self.frame_reports(keyboard, held)
    }

    fn frame_reports(&mut self, keyboard: Option<usize>, held: &dyn Fn(u8) -> bool) -> [SeatInput; SEATS] {
        if keyboard.is_none() {
            self.keys.scheme = self.keys_scheme();
        }
        self.keyboard_alone = keyboard.is_none().then(|| self.keys.report(held));
        let mut out = [SeatInput::Empty; SEATS];
        self.taps = [0; SEATS];
        for (i, seat) in out.iter_mut().enumerate() {
            let keys: &dyn Fn(u8) -> bool = if keyboard == Some(i) { held } else { &no_keys };
            let scheme = self.wiring.schemes[i];
            *seat = match &self.slots.held[i] {
                Some(id) => {
                    let j = self.joined.iter_mut().find(|j| &j.id == id).expect("a held pad is joined");
                    j.pads.scheme = scheme;
                    let report = j.pads.report(keys);
                    self.taps[i] = j.pads.taps();
                    SeatInput::Playing(report)
                }
                None if self.slots.waiting(i) => SeatInput::Waiting,
                None if keyboard == Some(i) => {
                    self.keys.scheme = scheme;
                    SeatInput::Playing(self.keys.report(held))
                }
                None => SeatInput::Empty,
            };
        }
        // A pad in no seat is read by nobody this frame; its presses are not kept for the menus.
        for j in &mut self.joined {
            if !self.slots.held.contains(&Some(j.id.clone())) {
                j.pads.clear_queue();
            }
        }
        for (i, seat) in out.iter().enumerate() {
            let kind = match seat {
                SeatInput::Empty => SeatKind::Empty,
                SeatInput::Playing(_) => SeatKind::Playing,
                SeatInput::Waiting => SeatKind::Waiting,
            };
            let what = || self.name(i).unwrap_or("the keyboard").to_string();
            match (self.was[i], kind) {
                (SeatKind::Waiting, SeatKind::Playing) => {
                    self.said.push(format!("{}P: {} is back", i + 1, what()));
                }
                (_, SeatKind::Playing) if self.was[i] != SeatKind::Playing => {
                    self.said.push(format!("{}P: {}", i + 1, what()));
                }
                (_, SeatKind::Waiting) if self.was[i] != SeatKind::Waiting => self.said.push(format!(
                    "{}P: the pad is out — paused until it is plugged back in{}",
                    i + 1,
                    if keyboard == Some(i) { " (or Start on the keyboard rides on without it)" } else { "" },
                )),
                (SeatKind::Playing, SeatKind::Empty) => self.said.push(format!("{}P: left", i + 1)),
                _ => {}
            }
            self.was[i] = kind;
        }
        out
    }
}

/// A joystick axis as the stick byte a DualShock 2 reports: centred at `0x7f`, full travel at 0 and
/// 255, and up or left at the low end — which is the hardware's own sense, so that the port's dead
/// zone and its one inverted axis land where I2 measured them.
fn stick_byte(value: i16) -> u8 {
    let centred = (value as i32 * 128) / 32767;
    (AXIS_CENTRE as i32 + centred).clamp(0, 255) as u8
}

/// A trigger axis as a pressure byte: rest at the bottom of the travel, with a dead zone so that a
/// pad resting its triggers in the middle of the range does not brake on its own.
fn trigger_pressure(value: i16) -> u8 {
    let travel = (value as f32 + 32767.0) / 65534.0;
    if travel <= TRIGGER_DEAD {
        return 0;
    }
    let scaled = (travel - TRIGGER_DEAD) / (1.0 - TRIGGER_DEAD);
    (scaled * 255.0).clamp(0.0, 255.0) as u8
}

/// Press one control at a pressure: the byte the game's poll reads it from, and its bit in the
/// active-low word.
///
/// **Harder wins.** Two sources on one control — a pad's own Cross and the keyboard's throttle ramp,
/// or a trigger wired to gas on a pad whose Cross is also down — behave like two fingers on one
/// button rather than like the last one written. The bit is only cleared once the pressure is past
/// the threshold the game's poll calls a press, so the word and the pressures agree about whether
/// the control is down even while a ramp is still under it.
fn press(report: &mut Report, control: Control, pressure: u8) {
    let binding = control.binding();
    if binding.is_analogue() {
        let slot = &mut report.0[binding.pressure as usize];
        *slot = (*slot).max(pressure);
    }
    if pressure > PRESSURE_THRESHOLD {
        let word = (((report.0[2] as u16) << 8) | report.0[3] as u16) & !binding.mask;
        report.0[2] = (word >> 8) as u8;
        report.0[3] = word as u8;
    }
}

/// One axis of a d-pad hat as two of the game's controls.
fn hat(value: i16, low: Control, high: Control) -> u16 {
    if value < -16384 {
        low.mask()
    } else if value > 16384 {
        high.mask()
    } else {
        0
    }
}

/// The keyboard as the game's own controls.
///
/// Laid out so that the four face keys are where the four face buttons are on a pad: `Z` is Square,
/// `X` Cross — the throttle — `C` Circle and `V` Triangle, with the shoulders on `A`, `S`, `D`, `F`
/// above them, Start on Return, and the arrow keys on the d-pad.
///
/// **The arrows are the d-pad and not the left stick** (P30). The original's d-pad *is* its digital
/// stick on the bike: the pad read puts ±1 in place of the left stick's x or y while a direction is
/// held, flags the axis digital, and the bike then walks its lean toward it at the `.phy`'s
/// `XDPadRateLimit`/`YDPadRateLimit` instead of the analogue pair and times a preload with the
/// `DPad` preload times — a whole set of numbers written for a control that is a switch, which is
/// what a key is. The backflip and the preload are sized by how fast the stick crossed, and a d-pad
/// crosses in one sample, the way a thumb flicking a stick does; a ramped key stick crossed at a
/// twentieth of that speed and could not fit three crosses on a lip.
///
/// The throttle keys (`X` and the space bar) are not in this table: they are a pressure and not a
/// bit, and [`KeyRamp`] answers both the byte and the bit for them.
///
/// The same keys fly the biplane, with nothing added, because the plane reads the same pad through
/// the shipped binding table ([`game_sim::input::PlaneCommand::control`]): `X`/space is Cross, the
/// throttle up; `Z` is Square, the throttle down; `A` and `D` are L1 and R1, the rudder (and both
/// together are the plane's own reset chord); and the arrows are the d-pad, where the plane's dive,
/// climb and bank commands sit — they write the same ±1 to the elevator and the aileron that a stick
/// at its stop does.
///
/// Since P35 this is the keyboard's half of the default [`Bindings`], which `controls.txt` can
/// rebind; the throttle keys are that file's `cross` line.
const KEYBOARD: [(u8, Control); 15] = [
    (key::Z, Control::Square),
    (key::C, Control::Circle),
    (key::V, Control::Triangle),
    (key::A, Control::L1),
    (key::S, Control::L2),
    (key::D, Control::R1),
    (key::F, Control::R2),
    (key::RETURN, Control::Start),
    (key::UP, Control::Up),
    (key::DOWN, Control::Down),
    (key::LEFT, Control::Left),
    (key::RIGHT, Control::Right),
    (key::TAB, Control::Select),
    (key::Q, Control::L3),
    (key::E, Control::R3),
];

/// The keyboard under the new layout (P54): the usual one. J is the gas and K the brake, on the home
/// row under the right hand (P80), with Space and Shift as second keys for them — R2 and L2, which is
/// where [`Scheme::ride`] reads the new layout's gas and brake from, and R2's keys are the throttle
/// ramp. Q and E are L1 and R1, Tab or C the camera (Select), Enter Start, the arrows the d-pad (and
/// the menus). The face buttons sit along the same row (P75, moved by P80): H Square and L Circle
/// either side of the gas and the brake, G Cross and `;` Triangle beyond them. Z and N are L3 and R3.
/// Escape is the pause, the shell's own key. `WASD` and `YUIO` are the sticks ([`KEY_STICKS_NEW`]).
///
/// R and F, L2 and R2 in the brief where a scheme still needs them, are not bound: under the new
/// layout L2 and R2 are the brake and the gas, already on K/Shift and J/Space, and the original's
/// layout keeps its own keyboard ([`KEYBOARD`]).
const KEYBOARD_NEW: [(u8, Control); 20] = [
    (key::J, Control::R2),
    (key::SPACE, Control::R2),
    (key::K, Control::L2),
    (key::SHIFT_LEFT, Control::L2),
    (key::SHIFT_RIGHT, Control::L2),
    (key::Q, Control::L1),
    (key::E, Control::R1),
    (key::TAB, Control::Select),
    (key::C, Control::Select),
    (key::RETURN, Control::Start),
    (key::UP, Control::Up),
    (key::DOWN, Control::Down),
    (key::LEFT, Control::Left),
    (key::RIGHT, Control::Right),
    (key::H, Control::Square),
    (key::L, Control::Circle),
    (key::G, Control::Cross),
    (key::SEMICOLON, Control::Triangle),
    (key::Z, Control::L3),
    (key::N, Control::R3),
];

/// Two keys as one stick axis, ramped: where the stick has got to, as a fraction of full deflection
/// from −1 at the low key's edge to 1 at the high key's edge.
///
/// A key has nothing between off and on, so answering 0 or 255 the frame it goes down — which is what
/// this module used to do — pushes the stick to the stop instantly and holds it there. Everything the
/// sticks feed is then all-or-nothing: the lean is full lean, the steer is full lock, and a preload is
/// either nothing or a bottomed-out suspension, with the ride's dead zone and its two segments crossed in a
/// single frame. So the axis travels instead, at [`KEY_TO_EDGE`] to the stop and [`KEY_TO_CENTRE`]
/// back, and a tap is a nudge rather than a slam.
#[derive(Clone, Copy)]
struct KeyAxis {
    /// Where the stick is, −1 to 1, 0 at centre.
    at: f32,
}

impl KeyAxis {
    /// A stick nobody has touched.
    const CENTRED: KeyAxis = KeyAxis { at: 0.0 };

    /// One frame of `seconds` with the two keys as they are, as the stick byte a DualShock 2 would
    /// have reported — or `None` once the axis has settled at centre with neither key down, so that a
    /// pad's own stick keeps the axis when the keyboard is not touching it.
    fn step(&mut self, seconds: f32, low: bool, high: bool) -> Option<u8> {
        let target = match (low, high) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            // Both keys, or neither: centre. Both at once is a rider changing their mind, and the
            // stick comes back the way it would if they had simply let go.
            _ => 0.0,
        };
        // Anything that reduces the deflection — letting go, or pushing the other way through
        // centre — happens at the return rate. Only travel further out is a ramp.
        let homeward = target == 0.0 || (self.at != 0.0 && (target < 0.0) != (self.at < 0.0));
        let step = seconds / if homeward { KEY_TO_CENTRE } else { KEY_TO_EDGE };
        self.at = if target > self.at {
            (self.at + step).min(target)
        } else {
            (self.at - step).max(target)
        };
        if self.at == 0.0 && target == 0.0 {
            return None;
        }
        // The same conversion the pad's own sticks go through, so a key and a stick at the same
        // deflection report the same byte.
        Some(stick_byte((self.at * 32767.0) as i16))
    }
}

/// A key as a pressure byte, ramped: the keyboard's throttle, 0 shut and 255 pinned.
///
/// [`KeyAxis`] is this for a stick and the reason is the same — a switch has no travel — but the
/// answer is a different shape. A stick's rest is the middle of its range and it has two keys, one
/// each way; a throttle's rest is one end of its range and it has one. What a rider does with the
/// throttle that this is for is the thing a switch cannot do at all: hold it part way. So the ramp
/// climbs at [`KEY_TO_FULL`] while the key is down and falls at [`KEY_TO_SHUT`] when it is not, and
/// the pressure byte anywhere along it is a fraction of the throttle the game reads as that fraction
/// ([`game_sim::input::Pad::strength`]).
///
/// The first few frames of a press are below the game's own pressure threshold, exactly as the first
/// few frames of a held direction are inside the ride's dead zone: a stab that short is not a throttle
/// input at all, which is the same thing a real button does on its way down.
#[derive(Clone, Copy)]
struct KeyRamp {
    /// How far the throttle is open, 0 to 1.
    at: f32,
}

impl KeyRamp {
    /// A throttle nobody is holding.
    const SHUT: KeyRamp = KeyRamp { at: 0.0 };

    /// One frame of `seconds` with the key as it is, as a pressure byte — or `None` once the throttle
    /// has shut with the key up, so that a pad's own Cross (or a trigger wired to gas) owns the
    /// control when the keyboard is not touching it.
    fn step(&mut self, seconds: f32, held: bool) -> Option<u8> {
        let rate = if held { KEY_TO_FULL } else { KEY_TO_SHUT };
        let step = seconds / rate;
        self.at = if held { (self.at + step).min(1.0) } else { (self.at - step).max(0.0) };
        if self.at == 0.0 {
            return None;
        }
        // The byte a DualShock 2 would have reported at this travel, which the game's poll reads
        // back as `(byte + 1) / 256`: 255 is the throttle pinned and 0 is the lightest touch there
        // is, not the absence of one, so a ramp that has left zero never answers zero.
        Some((self.at * 255.0).round().clamp(1.0, 255.0) as u8)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use game_sim::input::{AXIS_CENTRE_RAW, AxisRange, Manager, PADS};
    use game_sim::ride::Controls;
    use ps2_float::F;

    #[test]
    fn a_centred_stick_is_the_centre_byte() {
        assert_eq!(stick_byte(0), AXIS_CENTRE);
        assert_eq!(stick_byte(-32767), 0);
        assert_eq!(stick_byte(32767), 255);
        // The centre byte is the one the port's own "no pad bound here" value rounds to.
        assert_eq!(F::from(i32::from(stick_byte(0))), AXIS_CENTRE_RAW);
    }

    #[test]
    fn a_resting_trigger_is_no_pressure_and_a_pulled_one_is_all_of_it() {
        assert_eq!(trigger_pressure(-32767), 0);
        assert_eq!(trigger_pressure(-30000), 0);
        assert_eq!(trigger_pressure(32767), 255);
        assert!(trigger_pressure(0) > 100 && trigger_pressure(0) < 160);
    }

    /// One frame of the shell's loop, for a ramp: 60 Hz is what it paces itself to.
    const FRAME: f32 = 1.0 / 60.0;

    #[test]
    fn a_held_direction_reaches_the_edge_over_several_frames() {
        let mut axis = KeyAxis::CENTRED;
        // The frame the key goes down: off centre, and nowhere near the stop.
        let first = axis.step(FRAME, false, true).expect("a held key moves the stick");
        assert!(
            first > AXIS_CENTRE && first < 200,
            "the first frame of a held key reads {first}, which is a slam and not a lean"
        );
        // And from there, up to the edge and no further, climbing every frame.
        let mut last = first;
        let mut frames = 1;
        while last < 255 {
            let byte = axis.step(FRAME, false, true).expect("still held");
            assert!(byte > last, "frame {frames} did not travel: {last} then {byte}");
            last = byte;
            frames += 1;
            assert!(frames < 60, "a held key never reached the edge");
        }
        assert!(
            (8..=15).contains(&frames),
            "full deflection took {frames} frames, which is not about {KEY_TO_EDGE} s"
        );
        assert_eq!(axis.step(FRAME, false, true), Some(255), "and it stays there");

        // Let go: back to centre, quicker, and then out of the way altogether so that a pad's own
        // stick has the axis again.
        let mut frames = 0;
        while let Some(byte) = axis.step(FRAME, false, false) {
            assert!(byte < 255);
            frames += 1;
            assert!(frames < 60, "a released key never came back to centre");
        }
        assert!(
            (4..=9).contains(&frames),
            "the return took {frames} frames, which is not about {KEY_TO_CENTRE} s"
        );
        assert_eq!(axis.step(FRAME, false, false), None, "and it stays out");
    }

    #[test]
    fn the_low_key_goes_the_other_way_and_the_two_together_come_home() {
        let mut axis = KeyAxis::CENTRED;
        for _ in 0..20 {
            axis.step(FRAME, true, false);
        }
        assert_eq!(axis.step(FRAME, true, false), Some(0), "the low key's edge");
        // Both keys at once is a rider changing their mind: the stick comes home rather than
        // sticking at the stop, and it does it at the return rate.
        let both = axis.step(FRAME, true, true).expect("not home yet");
        assert!(both > 0 && both < AXIS_CENTRE, "{both} is not on the way home");
        // The other key, from the far stop: through centre and out the other side.
        let mut axis = KeyAxis::CENTRED;
        for _ in 0..20 {
            axis.step(FRAME, true, false);
        }
        let mut frames = 0;
        while axis.step(FRAME, false, true).unwrap_or(0) <= AXIS_CENTRE {
            frames += 1;
            assert!(frames < 60, "a reversed stick never crossed centre");
        }
        assert!(frames >= 5, "a stick at full left crossed centre in {frames} frames");
    }

    /// What the task is for: one frame of a key press is a nudge the game's own dead zone still
    /// swallows, where the old all-or-nothing byte was full lock on the frame the key went down.
    #[test]
    fn a_tap_no_longer_slams_the_game_s_own_axis() {
        let mut axis = KeyAxis::CENTRED;
        let read = |byte: u8| {
            let mut report = Report::idle();
            report.set_axis(REPORT_LEFT_X, byte);
            let mut manager = Manager::connected();
            manager.tick(F(1.0 / 60.0), &[report; PADS]);
            // What the ride's hands read: the player's range (I7), not the main menu's.
            game_sim::ride::Controls::from_pad(manager.pad(0)).stick.0
        };
        // Frame one: inside the ride's dead zone, so the lean, the steer and the preload all still
        // read nothing. The byte the module used to answer here was 255, which is `PLAYER.hi`.
        let first = axis.step(FRAME, false, true).unwrap();
        assert_eq!(read(first), F(0.0), "frame one of a tap is not a steering input");
        assert_eq!(read(255), AxisRange::PLAYER.hi, "what frame one used to be");
        // Four frames in it is out of the ±0.3 dead zone (three are 0.28 of the travel, still in
        // it) but nowhere near the stop.
        for _ in 0..2 {
            axis.step(FRAME, false, true);
        }
        let fourth = read(axis.step(FRAME, false, true).unwrap());
        assert!(
            fourth > F(0.0) && fourth < F(0.5),
            "four frames of a held key reads {fourth:?}"
        );
    }

    /// The point of the whole module: the bytes it builds go through the port's own poll and come out
    /// as the controls the person pressed.
    #[test]
    fn a_built_report_reads_back_through_the_ports_own_poll() {
        let mut report = Report::idle();
        let buttons = Control::Cross.mask() | Control::Start.mask();
        let low = !buttons;
        report.0[2] = (low >> 8) as u8;
        report.0[3] = low as u8;
        for binding in CONTROL_MAP {
            if binding.is_analogue() && buttons & binding.mask != 0 {
                report.0[binding.pressure as usize] = 0xff;
            }
        }
        report.set_axis(REPORT_LEFT_X, stick_byte(32767));

        let mut manager = Manager::connected();
        manager.tick(F(1.0 / 60.0), &[report; PADS]);
        let pad = manager.pad(0);
        assert!(pad.held(Control::Cross), "the throttle");
        assert!(
            pad.held(Control::Start),
            "a digital control with no pressure byte"
        );
        assert!(!pad.held(Control::Square));
        // Full right, through the game's own dead zone and its 1.25x segment: the top of the range.
        assert_eq!(
            pad.axis(game_sim::input::Axis::LeftX),
            AxisRange::STICK.hi,
            "a stick pushed all the way reads all the way"
        );
    }

    /// I6: the keyboard flies the plane. Through the port's own poll and the plane's own input step,
    /// a held up arrow is a full dive and a held down arrow a full climb, left and right are a full
    /// bank, `X` opens the throttle, `Z` closes it, and `A` and `D` are the rudder.
    #[test]
    fn the_keyboard_reaches_all_eight_of_the_planes_commands() {
        use game_sim::input::PlaneInput;
        // A second of `keys` held, through the shell's report and the game's poll.
        let fly = |keys: &[u8]| {
            let mut pads = Pads::unplugged(Wiring::default());
            let mut manager = Manager::connected();
            let held = |code: u8| keys.contains(&code);
            for _ in 0..60 {
                manager.tick(F(FRAME), &[pads.build(FRAME, &held); PADS]);
            }
            PlaneInput::from_pad(manager.pad(0))
        };
        assert_eq!(fly(&[key::UP]).elevator, F::ONE, "up is stick forward: a dive");
        assert_eq!(fly(&[key::DOWN]).elevator, -F::ONE, "down is a climb");
        assert_eq!(fly(&[key::LEFT]).aileron, F::ONE, "left banks left");
        assert_eq!(fly(&[key::RIGHT]).aileron, -F::ONE, "right banks right");
        assert_eq!(fly(&[key::X]).throttle_up, F::ONE, "x opens the throttle, all the way");
        assert_eq!(fly(&[key::Z]).throttle_down, F::ONE, "z closes it");
        assert_eq!(fly(&[key::A]).rudder, -F::ONE, "a is left rudder");
        assert_eq!(fly(&[key::D]).rudder, F::ONE, "d is right rudder");
        // And nothing held is nothing asked: a centred stick's cube is the original's −0.0.
        let idle = fly(&[]);
        assert_eq!([idle.throttle_up, idle.throttle_down, idle.rudder], [F::ZERO; 3]);
        assert_eq!([idle.elevator.to_bits(), idle.aileron.to_bits()], [0x8000_0000; 2]);
    }

    /// P50, at the pad: upside down under the screen's biplane controls the plane's input step reads
    /// the elevator and the rudder the other way round and everything else as it was — and under the
    /// plane's (P69) or the original's, at any attitude, or the right way up, the plane is handed the
    /// very report the pad made.
    ///
    /// Over 20,000 generated reports: random buttons, pressures and stick bytes, through the game's
    /// own poll and I6's input step.
    #[test]
    fn upside_down_the_screen_biplane_controls_turn_the_elevator_and_rudder_round() {
        use game_sim::input::PlaneInput;
        let read = |report: Report| {
            let mut manager = Manager::connected();
            manager.tick(F(FRAME), &[report; PADS]);
            PlaneInput::from_pad(manager.pad(0))
        };
        let mut seed = 0x5eed_1234_u64;
        let mut next = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as u32
        };
        for _ in 0..20_000 {
            let mut report = Report::idle();
            let buttons = next() as u16;
            report.0[2] = !(buttons >> 8) as u8;
            report.0[3] = !buttons as u8;
            for binding in CONTROL_MAP.iter().filter(|b| b.is_analogue()) {
                if buttons & binding.mask != 0 {
                    report.0[binding.pressure as usize] = next() as u8;
                }
            }
            for axis in 0..4 {
                report.set_axis(axis, next() as u8);
            }

            // The original's and the plane's at any attitude, and the screen's the right way up:
            // the same bytes, so the same report into the simulation, bit for bit.
            for scheme in [PlaneScheme::Original, PlaneScheme::New] {
                assert_eq!(scheme.fly(report, true), report);
                assert_eq!(scheme.fly(report, false), report);
            }
            assert_eq!(PlaneScheme::Screen.fly(report, false), report);

            let (up, down) = (read(report), read(PlaneScheme::Screen.fly(report, true)));
            assert_eq!(PlaneScheme::Screen.fly(PlaneScheme::Screen.fly(report, true), true), report, "twice is not once undone");
            let bits = |i: &PlaneInput| [i.throttle_up, i.throttle_down, i.aileron].map(F::to_bits);
            assert_eq!(bits(&down), bits(&up), "the throttle or the ailerons moved: {report:?}");
            // Both held is the original's left-wins (and up-wins) either way round, and is the
            // plane's own reset chord for L1 and R1, so it is the same both ways.
            let mut manager = Manager::connected();
            manager.tick(F(FRAME), &[report; PADS]);
            let held = |c: Control| manager.pad(0).strength(c) != F::ZERO;
            if !(held(Control::L1) && held(Control::R1)) {
                assert_eq!(down.rudder, -up.rudder, "the rudder is not turned round: {report:?}");
            }
            // The d-pad is the markers' and not swapped (P107), so with it held the elevator here
            // is its own either way up; the plane's step does not read it.
            if !held(Control::Up) && !held(Control::Down) {
                // The mirrored byte reads as the other side of centre to within the poll's rounding.
                let (a, b) = (up.elevator.0, down.elevator.0);
                assert!((a + b).abs() <= 1e-6 * a.abs().max(1.0), "the elevator {a} upside down is {b}: {report:?}");
                assert!(a == 0.0 || a.signum() != b.signum(), "the elevator {a} kept its sign upside down: {report:?}");
            }
        }
        // A pad that is not there stays not there.
        assert_eq!(PlaneScheme::Screen.fly(Report::missing(), true), Report::missing());
    }

    /// P69: the biplane's three schemes by name. `new` is the plane-relative default, which is what
    /// a `controls.txt` saved before P69 says too; P50's screen-relative one is `screen`; and only
    /// the default lifts the knife-edge fade.
    #[test]
    fn the_biplane_schemes_read_and_write_their_words() {
        assert_eq!(PlaneScheme::default(), PlaneScheme::New);
        let all = PlaneScheme::parse_list("new,screen,original").unwrap();
        assert_eq!(&all[..3], &[PlaneScheme::New, PlaneScheme::Screen, PlaneScheme::Original]);
        assert!(all[3..].iter().all(|s| *s == PlaneScheme::Original), "the last word is not the rest's");
        assert_eq!(PlaneScheme::list_text(&all), "new,screen,original,original,original,original,original,original");
        assert_eq!(PlaneScheme::list_text(&PlaneScheme::parse_list(" Screen ").unwrap()), "screen");
        assert!(PlaneScheme::parse_list("sideways").is_err());
        assert!(PlaneScheme::parse_list(&["new"; SEATS + 1].join(",")).is_err());
        let lifts: Vec<bool> = all[..3].iter().map(|s| s.every_bank()).collect();
        assert_eq!(lifts, [true, false, false]);
    }

    /// Nobody's hands on anything, for a test that only cares about one control.
    fn nothing(_: u8) -> bool {
        false
    }

    /// How hard the gas command is held after one frame of `report`, through the port's own poll and
    /// binding: the strength the biplane's throttle reads as it is. The bike reads it as a switch
    /// (P77), and this checks that too: its gas is 1.0 for any strength but 0.
    fn gas_of(report: Report) -> F {
        let mut manager = Manager::connected();
        manager.tick(F(FRAME), &[report; PADS]);
        let strength = Table::SHIPPED.value(manager.pad(0), Command::GAS);
        let bike = Controls::from_pad(manager.pad(0)).gas;
        assert_eq!(bike, if strength == F::ZERO { F::ZERO } else { F::ONE }, "the bike's gas is a switch");
        strength
    }

    /// The task's own case, at the narrowest point it can be asked: a pressure byte between the two
    /// stops is that fraction of the throttle and not all of it.
    ///
    /// This is the game's arithmetic and not the shell's — `(pressure + 1) / 256`, I1's — so what it
    /// pins is the shell's end of the deal: the byte this module writes for a part-pressed button is
    /// read as part throttle, which is what makes the two halves below worth building.
    #[test]
    fn a_pressure_byte_between_the_stops_is_that_fraction_of_the_throttle() {
        for byte in [0x1a, 0x20, 0x40, 0x7f, 0x80, 0xc0, 0xfe, 0xff] {
            let mut report = Report::idle();
            press(&mut report, Control::Cross, byte);
            assert_eq!(
                gas_of(report),
                F((byte as f32 + 1.0) / 256.0),
                "a pressure of {byte:#x} is not {byte:#x}/256 of the throttle"
            );
        }
        // And below the threshold the control is not down at all, which is the game's own rule and
        // the reason a ramp's first frames are a touch and not a throttle.
        let mut report = Report::idle();
        press(&mut report, Control::Cross, PRESSURE_THRESHOLD);
        assert_eq!(gas_of(report), F::ZERO);
    }

    /// Half of the first half: a pad that reports how hard a button is pressed is read that way.
    #[test]
    fn a_pads_own_button_pressure_reaches_the_gas() {
        let pressure = Wiring::parse_pressures("ds2@8").expect("the DualShock 2's own order");
        let mut pads = Pads::unplugged(Wiring { pressure, ..Wiring::default() });
        // Cross is the seventh of the twelve pressures, so `ds2@8` puts it on axis 14.
        assert_eq!(pressure[14], Some(Control::Cross));

        // The button down and the device quiet about how hard: the full byte a digital source has to
        // mean, because the bit is what said the control is down.
        pads.button[0] = true;
        assert_eq!(gas_of(pads.build(FRAME, &nothing)), F::ONE);

        // The same button, half way down. The axis is a travel like a trigger's, so it comes through
        // the same dead zone, and what reaches the gas is a fraction of the throttle and not all of
        // it — which is the thing this module could not do before.
        pads.raw[14] = 0;
        let half = gas_of(pads.build(FRAME, &nothing));
        assert_eq!(half, F((trigger_pressure(0) as f32 + 1.0) / 256.0));
        assert!(half > F(0.3) && half < F(0.7), "half a button's travel reads {half:?}");

        // A touch the dead zone swallows is still the press the bit says it is.
        pads.raw[14] = TRIGGER_REST;
        assert_eq!(gas_of(pads.build(FRAME, &nothing)), F::ONE);

        // And the button up is no throttle, however hard the axis is leaning.
        pads.button[0] = false;
        pads.raw[14] = i16::MAX;
        assert_eq!(gas_of(pads.build(FRAME, &nothing)), F::ZERO);
    }

    /// The second half: gas on a trigger, for a pad whose face buttons are switches. The trigger
    /// stops being its own control, which is the trade.
    #[test]
    fn the_throttle_on_a_trigger_is_part_throttle_and_gives_up_its_own_control() {
        let wiring = Wiring { throttle: Throttle::Trigger(Control::R2), ..Wiring::default() };
        let mut pads = Pads::unplugged(wiring);
        let controls = |pads: &mut Pads| {
            let report = pads.build(FRAME, &nothing);
            let mut manager = Manager::connected();
            manager.tick(F(FRAME), &[report; PADS]);
            Controls::from_pad(manager.pad(0))
        };
        // The trigger at rest: no throttle and no brake.
        let c = controls(&mut pads);
        assert_eq!((c.gas, c.front_brake), (F::ZERO, F::ZERO));

        // Half way: part throttle on the gas command, which the bike takes as all of it (P77), and
        // the front brake it would otherwise have been is not applied.
        pads.axis[AXIS_R2] = 0;
        let c = controls(&mut pads);
        let half = gas_of(pads.build(FRAME, &nothing));
        assert_eq!(half, F((trigger_pressure(0) as f32 + 1.0) / 256.0));
        assert!(half > F(0.3) && half < F(0.7), "half a trigger reads {half:?}");
        assert_eq!(c.gas, F::ONE, "the bike's gas is a switch");
        assert_eq!(c.front_brake, F::ZERO, "the throttle's trigger is not the brake as well");

        // Pulled: full throttle. The same trigger on a pad wired the ordinary way is no throttle at
        // all, and since P73 nothing else in a ride either: the original's table has R2 on its other
        // camera, not on a front brake.
        pads.axis[AXIS_R2] = i16::MAX;
        assert_eq!(controls(&mut pads).gas, F::ONE);
        let mut plain = Pads::unplugged(Wiring::default());
        plain.axis[AXIS_R2] = i16::MAX;
        let c = controls(&mut plain);
        assert_eq!(format!("{c:?}"), format!("{:?}", Controls::default()));
    }

    /// The third half: the keyboard's throttle ramps rather than stepping, and what it ramps through
    /// is the gas the simulation reads.
    #[test]
    fn a_keyboard_throttle_ramps_rather_than_stepping() {
        let mut pads = Pads::unplugged(Wiring::default());
        let throttle = |code: u8| code == key::X;

        // Frame one is a touch, not a throttle: below the game's own pressure threshold, exactly as
        // frame one of a held direction is inside its dead zone. What this module used to answer here
        // was a full byte — the switch the task is about.
        assert_eq!(gas_of(pads.build(FRAME, &throttle)), F::ZERO);
        let mut full = Report::idle();
        press(&mut full, Control::Cross, 0xff);
        assert_eq!(gas_of(full), F::ONE, "what frame one used to be");

        // From there it climbs, every frame, through every fraction of the throttle on its way.
        let mut last = F::ZERO;
        let mut frames = 1;
        let mut partial = 0;
        while last < F::ONE {
            let gas = gas_of(pads.build(FRAME, &throttle));
            assert!(gas >= last, "frame {frames} went backwards: {last:?} then {gas:?}");
            if gas > F::ZERO && gas < F::ONE {
                partial += 1;
            }
            last = gas;
            frames += 1;
            assert!(frames < 120, "a held throttle key never reached full");
        }
        assert!(
            (15..=22).contains(&frames),
            "full throttle took {frames} frames, which is not about {KEY_TO_FULL} s"
        );
        assert!(partial > 10, "only {partial} frames of the climb were part throttle");

        // A stab is a stab: three frames of the key is a fifth of the throttle, where the switch
        // this replaces asked for all of it on frame one.
        let mut pads = Pads::unplugged(Wiring::default());
        pads.build(FRAME, &throttle);
        pads.build(FRAME, &throttle);
        let stab = gas_of(pads.build(FRAME, &throttle));
        assert!(stab > F::ZERO && stab < F(0.3), "a three-frame stab asks for {stab:?}");

        // Let go part way up and it closes through the fractions as well, quicker than it opened —
        // a rider who lets go of the throttle has already decided — and then gets out of the way so
        // that a pad's own Cross owns the control again.
        let closing = gas_of(pads.build(FRAME, &nothing));
        assert!(closing < stab, "the throttle did not close: {stab:?} then {closing:?}");
        let mut frames = 1;
        while pads.key_gas.step(FRAME, false).is_some() {
            frames += 1;
            assert!(frames < 60, "a released throttle key never shut");
        }
        assert!(frames < 15, "closing from a stab took {frames} frames");
        assert_eq!(gas_of(pads.build(FRAME, &nothing)), F::ZERO);
    }

    /// A pad's Cross and the keyboard's ramp are two fingers on one button, not two writers of one
    /// byte: whoever is pressing harder is what the bike gets.
    #[test]
    fn two_sources_on_the_throttle_are_the_harder_of_the_two() {
        let mut pads = Pads::unplugged(Wiring::default());
        pads.button[0] = true;
        let throttle = |code: u8| code == key::X;
        // The pad is pinned and the keyboard's ramp has barely started: full throttle, because the
        // pad's full byte is what a digital press means.
        assert_eq!(gas_of(pads.build(FRAME, &throttle)), F::ONE);
        // The pad let go, the keyboard still part way: the keyboard's own fraction.
        pads.button[0] = false;
        let gas = gas_of(pads.build(FRAME, &throttle));
        assert!(gas > F::ZERO && gas < F::ONE, "the keyboard's share of the throttle is {gas:?}");
    }

    /// A `--pad-pressure` spec, and what it refuses: the mapping is the device's and has to be asked
    /// for, so the parse is the whole of the shell's defence against a wrong one.
    #[test]
    fn a_pressure_spec_maps_axes_to_controls_or_says_why_not() {
        let none = Wiring::parse_pressures("none").expect("none");
        assert!(none.iter().all(Option::is_none));
        assert_eq!(Wiring::parse_pressures(""), Ok([None; AXES]));

        let ds2 = Wiring::parse_pressures("ds2@8").expect("ds2@8");
        assert_eq!(ds2[8], Some(Control::Right));
        assert_eq!(ds2[14], Some(Control::Cross));
        assert_eq!(ds2[19], Some(Control::R2));
        assert_eq!(ds2[7], None);
        assert_eq!(ds2[20], None);
        // Every one of the twelve, and each on the axis its own pressure byte would be at: the table
        // is the report's order, so axis = first + (pressure byte - 8).
        for control in DS2_PRESSURE_ORDER {
            let at = 8 + control.binding().pressure as usize - 8;
            assert_eq!(ds2[at], Some(control), "{} is on the wrong axis", control.name());
        }

        let hand = Wiring::parse_pressures("14:cross, 15:square").expect("a hand-written pair");
        assert_eq!((hand[14], hand[15]), (Some(Control::Cross), Some(Control::Square)));

        assert!(Wiring::parse_pressures("ds2@25").is_err(), "twelve axes from 25 is past the end");
        assert!(Wiring::parse_pressures("ds2@x").is_err());
        assert!(Wiring::parse_pressures("14").is_err(), "an axis with no control");
        assert!(Wiring::parse_pressures("14:gas").is_err(), "no control is called gas");
        assert!(Wiring::parse_pressures("99:cross").is_err(), "axis 99 is not read");
        // Start is read from its bit, so a pressure for it would never be looked at.
        assert!(Wiring::parse_pressures("14:start").is_err(), "start has no pressure byte");
    }

    #[test]
    fn a_control_is_named_the_way_the_port_names_it() {
        assert_eq!(control_named("cross"), Some(Control::Cross));
        assert_eq!(control_named("r2"), Some(Control::R2));
        assert_eq!(control_named("Cross"), None, "the names are the port's, in lower case");
        assert_eq!(control_named("throttle"), None);
    }

    // ---- seats (P35) ---------------------------------------------------------------------------

    /// Whether a report holds `control` down: its bit in the active-low word.
    fn down(report: Report, control: Control) -> bool {
        let word = ((report.0[2] as u16) << 8) | report.0[3] as u16;
        word & control.mask() == 0
    }

    #[test]
    fn seats_are_taken_in_join_order() {
        let mut s = PadSlots::<u32>::default();
        s.update(&[5, 7]);
        assert_eq!(s.held[..3], [Some(5), Some(7), None]);
        // A third pad plugged in later joins behind them, and the first two do not move.
        s.update(&[5, 7, 8]);
        assert_eq!(s.held[..4], [Some(5), Some(7), Some(8), None]);
        // Eight seats, and a ninth pad has nowhere to go.
        let mut s = PadSlots::<u32>::default();
        s.update(&(0..9).collect::<Vec<_>>());
        assert_eq!(s.held, std::array::from_fn(|i| Some(i as u32)));
        // Without --pads the keyboard rides with 1P.
        assert_eq!(s.keys(), Some(0));
    }

    #[test]
    fn a_seat_is_kept_across_an_unplug_and_replug() {
        let (a, b, c, d) = (1u32, 2, 3, 4);
        let mut s = PadSlots::default();
        s.update(&[a, b]);
        // 1P's pad is pulled out: 1P waits for it, and 2P stays 2P.
        s.update(&[b]);
        assert_eq!(s.held[..2], [None, Some(b)]);
        assert!(s.waiting(0) && !s.waiting(1));
        // A pad nobody has had joins a seat nobody is waiting in — not 1P's, whose rider is
        // somebody else's.
        s.update(&[b, c]);
        assert_eq!(s.held[..3], [None, Some(b), Some(c)]);
        // 1P's own pad back: 1P again, and the others where they were — even when it comes back
        // first in the list.
        s.update(&[a, b, c]);
        assert_eq!(s.held[..3], [Some(a), Some(b), Some(c)]);
        assert!(!s.waiting(0));
        // A seat let go is free for the next new pad.
        s.update(&[b, c]);
        s.forget(0);
        assert!(!s.waiting(0));
        s.update(&[b, c, d]);
        assert_eq!(s.held[..3], [Some(d), Some(b), Some(c)]);
        // ... and the pad it let go of, plugged back in, is a new player now.
        s.update(&[b, c, d, a]);
        assert_eq!(s.held[..4], [Some(d), Some(b), Some(c), Some(a)]);
    }

    /// opentennis's `seated_pads`, over eight seats, without its browser case (there is no touch
    /// pad here), and with this module's one difference: an unplugged pad's seat waits for it.
    #[test]
    fn seats_set_up_by_pads() {
        let (a, b, c) = (5u32, 7, 8);
        let (k, p) = (Some(Seat::Keys), |n| Some(Seat::Pad(n)));
        let seats = |s: &[Option<Seat>]| {
            let mut all = [None; SEATS];
            all[..s.len()].copy_from_slice(s);
            all
        };
        // 1P the keyboard, 2P pad 1: seat 1 stays empty, seat 2 takes pad 1.
        let mut s = PadSlots::with_seats(seats(&[k, p(1)]));
        s.update(&[a, b]);
        assert_eq!((&s.held[..2], s.keys()), (&[None, Some(b)][..], Some(0)));
        // A seat set up as nobody's never takes a pad.
        let mut s = PadSlots::with_seats(seats(&[p(0)]));
        s.update(&[a, b]);
        assert_eq!((&s.held[..2], s.keys()), (&[Some(a), None][..], None));
        // Four players, the keyboard as 3P; pad 0 gone: its seat waits instead of taking pad 2's
        // (or 4P's pad 1).
        let mut s = PadSlots::with_seats(seats(&[p(0), p(2), k, p(1)]));
        s.update(&[a, b, c]);
        assert_eq!((&s.held[..4], s.keys()), (&[Some(a), Some(c), None, Some(b)][..], Some(2)));
        s.update(&[b, c]);
        assert_eq!(s.held[..4], [None, Some(c), None, Some(b)]);
        assert!(s.waiting(0));
        s.update(&[b, c, a]);
        assert_eq!(s.held[..4], [Some(a), Some(c), None, Some(b)]);
        // A seat whose pad is not in yet waits for it rather than taking another seat's.
        let mut s = PadSlots::with_seats(seats(&[p(1), p(0)]));
        s.update(&[a]);
        assert_eq!(s.held[..2], [None, Some(a)]);
        // opentennis's B388: 1P's pad out and no keyboard seat, and the keyboard is 1P's stand-in.
        let mut s = PadSlots::with_seats(seats(&[p(0), p(1)]));
        s.update(&[a, b]);
        assert_eq!(s.keys(), None);
        s.update(&[b]);
        assert_eq!(s.keys(), Some(0));
        s.update(&[a, b]);
        assert_eq!(s.keys(), None);
        s.update(&[a]);
        assert_eq!(s.keys(), None, "2P's pad out: the keyboard is 1P's stand-in only");
    }

    #[test]
    fn a_pads_list_reads_back() {
        let (k, p) = (Some(Seat::Keys), |n| Some(Seat::Pad(n)));
        assert_eq!(Seat::flag(&[k, None, p(1)]), "k,-,1");
        assert_eq!(["k", "-", "2", ""].map(Seat::parse), [k, None, p(2), None]);
        let list = Seat::parse_list("0,k,-,3").expect("a list");
        assert_eq!(list[..5], [p(0), k, None, p(3), None]);
        assert_eq!(Seat::flag(&list), "0,k,-,3,-,-,-,-");
        assert!(Seat::parse_list("k,0,k").is_err(), "one keyboard");
        assert!(Seat::parse_list("0,x").is_err());
        assert!(Seat::parse_list("0,1,2,3,4,5,6,7,8").is_err(), "nine seats");
    }

    #[test]
    fn steam_ignore_list() {
        assert_eq!(
            ignore_list("0x045e/0x028e,0x054C/0x09cc,,junk,0x28de/"),
            [(0x045e, 0x028e), (0x054c, 0x09cc)]
        );
        let steam = DeviceInfo { vendor: 0x28de, product: 0x11ff, ..DeviceInfo::default() };
        let pad = DeviceInfo { vendor: 0x045e, product: 0x028e, ..DeviceInfo::default() };
        assert!(!readable_under(&steam, None) && readable_under(&pad, None));
        // Under Steam Input it is the other way round: the physical pad is skipped.
        let list = [(0x045e, 0x028e)];
        assert!(readable_under(&steam, Some(&list)) && !readable_under(&pad, Some(&list)));
    }

    #[test]
    fn bindings_round_trip_and_parse() {
        let d = Bindings::default();
        assert_eq!(Bindings::parse(&d.text()), (d.clone(), vec![]));
        let (b, errors) =
            Bindings::parse("square = B PadNorth Pad11 # a comment\ncircle = nope\nbogus = J\n");
        // A line in the file is that line under both layouts (P54).
        for lines in &b.lines {
            assert_eq!(lines[Control::Square as usize], [Input::Key(56), Input::Button(3), Input::Button(11)]);
        }
        assert_eq!(b.lines, [d.lines[0].clone(), d.lines[1].clone()].map(|mut l| {
            l[Control::Square as usize] = vec![Input::Key(56), Input::Button(3), Input::Button(11)];
            l
        }), "nothing understood, nothing else changed");
        assert_eq!(errors.len(), 2);
        assert_eq!(Bindings::parse(&b.text()).0, b);
        // The original's default is the shell's binding from before there was a file.
        let o = &d.lines[layout_at(Scheme::Original)];
        assert_eq!(o[Control::Cross as usize], [Input::Key(key::X), Input::Key(key::SPACE), Input::Button(0)]);
        // Except that js button 6 is Select since P40 (a pad with none has its Start moved to 7).
        assert_eq!(o[Control::Start as usize], [Input::Key(key::RETURN), Input::Button(7)]);
        assert_eq!(o[Control::Select as usize], [Input::Key(key::TAB), Input::Button(6)]);
        assert_eq!(o[16..20], [[Input::Key(key::J)], [Input::Key(key::L)], [Input::Key(key::I)], [Input::Key(key::K)]]);
        assert!(o[20..].iter().all(Vec::is_empty), "the original's left stick has no keys");
        // The pad's buttons are the same under both.
        let pad_of = |l: &[Vec<Input>; BINDINGS]| l.clone().map(|set| set.into_iter().filter(|x| matches!(x, Input::Button(_))).collect::<Vec<_>>());
        assert_eq!(pad_of(&d.lines[0]), pad_of(&d.lines[1]));
    }

    /// P54: one key held for half a second on a bare `Pads` under `scheme`, and the report after it.
    fn held_for(scheme: Scheme, code: u8) -> Report {
        let mut pads = Pads::unplugged(Wiring::default());
        pads.scheme = scheme;
        let held = |c: u8| c == code;
        for _ in 0..30 {
            pads.build(FRAME, &held);
        }
        pads.build(FRAME, &held)
    }

    /// P54, the new layout's keyboard: WASD the left stick, YUIO the right (P75), J or Space the gas
    /// and K or Shift the brake (P80), Q and E L1 and R1, Tab or C the camera, Enter Start, the
    /// arrows the d-pad, and the face buttons on the home row beside J and K (H Square, L Circle,
    /// G Cross, `;` Triangle — P80) — as the ride reads them through the new scheme.
    #[test]
    fn the_new_layout_s_keyboard_is_wasd_and_hjkl() {
        let centre = Report::idle();
        let axes = |r: Report| [REPORT_LEFT_X, REPORT_LEFT_Y, REPORT_RIGHT_X, REPORT_RIGHT_Y].map(|a| r.axis(a));
        let c = centre.axis(REPORT_LEFT_X);
        for (code, want) in [
            (key::A, [0, c, c, c]),
            (key::D, [255, c, c, c]),
            (key::W, [c, 0, c, c]),
            (key::S, [c, 255, c, c]),
            (key::Y, [c, c, 0, c]),
            (key::O, [c, c, 255, c]),
            (key::I, [c, c, c, 0]),
            (key::U, [c, c, c, 255]),
        ] {
            let report = held_for(Scheme::New, code);
            assert_eq!(axes(report), want, "key {code}");
            assert_eq!(report.buttons(), 0, "key {code} pressed a button too");
        }
        // P80: the four face buttons are on the home row — H and L either side of the gas and the
        // brake, G and `;` beyond them — and move no stick.
        for (code, control) in [
            (key::H, Control::Square),
            (key::L, Control::Circle),
            (key::G, Control::Cross),
            (key::SEMICOLON, Control::Triangle),
        ] {
            let report = held_for(Scheme::New, code);
            assert_eq!(report.buttons(), control.mask(), "key {code}");
            assert_eq!(axes(report), [c; 4], "key {code} moved a stick");
        }
        // The ride reads WASD as its lean and steer: A and D steer, W leans forward and S back,
        // the same as the pad's left stick at those bytes.
        let stick = |code| ride_of(Scheme::New.ride(held_for(Scheme::New, code))).stick;
        let pad_stick = |x: u8, y: u8| {
            let mut r = Report::idle();
            r.set_axis(REPORT_LEFT_X, x);
            r.set_axis(REPORT_LEFT_Y, y);
            ride_of(Scheme::New.ride(r)).stick
        };
        assert_eq!(stick(key::A), pad_stick(0, c));
        assert_eq!(stick(key::D), pad_stick(255, c));
        assert_eq!(stick(key::W), pad_stick(c, 0));
        assert_eq!(stick(key::S), pad_stick(c, 255));
        assert_ne!(stick(key::A), stick(key::D));
        assert_ne!(stick(key::W), stick(key::S));
        // P80: J (or Space) is the gas, ramped, and nothing else in the ride, and moves no stick;
        // X is nothing at all now.
        for gas in [key::J, key::SPACE] {
            let report = held_for(Scheme::New, gas);
            assert_eq!(axes(report), [c; 4], "key {gas} moved a stick");
            let ridden = ride_of(Scheme::New.ride(report));
            assert_eq!((ridden.gas, ridden.brake, ridden.clutch), (F::ONE, F::ZERO, F::ZERO), "key {gas}");
            let mut pads = Pads::unplugged(Wiring::default());
            pads.scheme = Scheme::New;
            pads.build(FRAME, &|c| c == gas);
            let stab = gas_of(Scheme::New.ride(pads.build(FRAME, &|c| c == gas)));
            assert!(stab > F::ZERO && stab < F::ONE, "a stab of key {gas} is part throttle: {stab:?}");
        }
        assert_eq!(held_for(Scheme::New, key::X), Report::idle());
        // K (or either Shift) is the brake.
        for brake in [key::K, key::SHIFT_LEFT, key::SHIFT_RIGHT] {
            let report = held_for(Scheme::New, brake);
            assert_eq!(axes(report), [c; 4], "key {brake} moved a stick");
            let c = ride_of(Scheme::New.ride(report));
            assert_eq!((c.gas, c.brake, c.clutch, c.front_brake), (F::ZERO, F::ONE, F::ZERO, F::ZERO), "key {brake}");
        }
        // The buttons, as the ride is handed them: one each, and the one the brief names.
        let ridden = |code| Scheme::New.ride(held_for(Scheme::New, code)).buttons();
        for (code, control) in [
            (key::Q, Control::L1),
            (key::E, Control::R1),
            (key::TAB, Control::Select),
            (key::C, Control::Select),
            (key::RETURN, Control::Start),
            (key::UP, Control::Up),
            (key::DOWN, Control::Down),
            (key::LEFT, Control::Left),
            (key::RIGHT, Control::Right),
            (key::L, Control::Circle),
            (key::SEMICOLON, Control::Triangle),
            (key::Z, Control::L3),
            (key::N, Control::R3),
        ] {
            assert_eq!(ridden(code), control.mask(), "key {code}");
        }
        // Tab or C changes the camera under the new scheme.
        assert_eq!(Scheme::New.camera(), Control::Select);
        // G and H are Cross and Square on the pad's report — the menus' OK and delete — and the new
        // scheme hands them to a ride as the pad's own, on R2 and L2: the other camera and look
        // back (P78), neither of which moves the bike.
        for (code, control, ridden) in [(key::G, Control::Cross, Control::R2), (key::H, Control::Square, Control::L2)] {
            let report = held_for(Scheme::New, code);
            assert_eq!(report.buttons(), control.mask(), "key {code}");
            assert_eq!(Scheme::New.ride(report).buttons(), ridden.mask(), "key {code} in a ride");
        }
        // And H is a face button now, so the shell's overlay stays off it; M is still the swap's.
        let b = Bindings::default();
        assert!(b.binds_key(Scheme::New, key::H) && !b.binds_key(Scheme::Original, key::H));
        assert!(!b.binds_key(Scheme::New, key::M) && !b.binds_key(Scheme::Original, key::M));
        assert!(!b.binds_key(Scheme::New, key::F3) && !b.binds_key(Scheme::Original, key::F3));
    }

    /// P54: the original's layout keeps the keyboard it had, key for key — every X keycode held
    /// alone, against the table the shell shipped before P54 (literal keycodes, not this module's
    /// constants, so a constant that moved would show).
    #[test]
    fn the_original_layout_s_keyboard_maps_exactly_as_before() {
        let buttons: [(u8, Control); 15] = [
            (52, Control::Square), (54, Control::Circle), (55, Control::Triangle), (38, Control::L1),
            (39, Control::L2), (40, Control::R1), (41, Control::R2), (36, Control::Start),
            (111, Control::Up), (116, Control::Down), (113, Control::Left), (114, Control::Right),
            (23, Control::Select), (24, Control::L3), (26, Control::R3),
        ];
        // I and K the right stick's up and down, J and L its left and right.
        let sticks = [(31, REPORT_RIGHT_Y, 0), (45, REPORT_RIGHT_Y, 255), (44, REPORT_RIGHT_X, 0), (46, REPORT_RIGHT_X, 255)];
        for code in 0..=255u8 {
            let report = held_for(Scheme::Original, code);
            let mut want = Report::idle();
            if let Some((_, control)) = buttons.iter().find(|(c, _)| *c == code) {
                press(&mut want, *control, 0xff);
            } else if [53, 65].contains(&code) {
                // X and Space: the throttle, ramped to full.
                press(&mut want, Control::Cross, report.pressure(Control::Cross.binding().pressure));
                assert_eq!(gas_of(report), F::ONE, "key {code}");
            } else if let Some((_, slot, byte)) = sticks.iter().find(|(c, _, _)| *c == code) {
                want.set_axis(*slot, *byte);
            }
            assert_eq!(report, want, "key {code}");
            // And Scheme::ride leaves it alone.
            assert_eq!(Scheme::Original.ride(report), report);
        }
    }

    /// P54: a `controls.txt` the shell wrote before the layouts had keyboards of their own — every
    /// line as it was then, this machine's own file among them (pre-P40's select and start) — is
    /// read as no file at all, so the new layout gets its keys; a line the player changed is theirs
    /// under both layouts.
    /// MD16: with MX vs. ATV's tricks the new layout's Cross and Square reach the ride as its two
    /// extra trick buttons, the original's layout gets none, and with the original's tricks the
    /// report is [`Scheme::ride`]'s to the byte.
    #[test]
    fn the_new_layout_s_face_buttons_are_the_extra_tricks() {
        let mut report = Report::idle();
        for c in [Control::Cross, Control::Square, Control::R2, Control::L2, Control::R1] {
            press(&mut report, c, 0xc0);
        }
        for scheme in [Scheme::New, Scheme::Original] {
            assert_eq!(scheme.ride_tricks(report, false), scheme.ride(report));
        }
        let new = Scheme::New.ride_tricks(report, true);
        let ridden = Scheme::New.ride(report);
        let extras = game_sim::input::mask_of(&game_sim::trick_pack::EXTRA_CONTROLS);
        assert_eq!(new.buttons() & !extras, ridden.buttons() & !extras, "the gas, the brake and R1 as they were");
        assert_eq!(new.buttons() & extras, extras);
        for c in game_sim::trick_pack::EXTRA_CONTROLS {
            assert_eq!(new.pressure(c.binding().pressure), 0xc0);
        }
        let mut faces = Report::idle();
        press(&mut faces, Control::R1, 0xff);
        let alone = Scheme::New.ride_tricks(faces, true);
        assert_eq!(alone.buttons() & extras, 0, "no face button, no extra");
        let old = Scheme::Original.ride_tricks(report, true);
        assert_eq!(old.buttons() & extras, 0, "the original's layout has no extras");
        assert_eq!(old.buttons(), report.buttons() & !extras);
        assert_eq!(Scheme::New.ride_tricks(Report::missing(), true), Report::missing());
    }

    #[test]
    fn an_old_controls_file_keeps_each_layout_s_keys() {
        let old = "triangle = V PadNorth\ncircle = C PadEast\ncross = X Space PadSouth\nsquare = Z PadWest\n\
                   l1 = A PadL1\nr1 = D PadR1\nl2 = S\nr2 = F\nselect = Tab\nstart = Return PadBack PadStart\n\
                   r3 = E PadR3\nl3 = Q PadL3\nup = Up\nright = Right\ndown = Down\nleft = Left\n\
                   stick_left = J\nstick_right = L\nstick_up = I\nstick_down = K\n";
        assert_eq!(Bindings::parse(old), (Bindings::default(), vec![]));
        // P80: so it moves none of the new layout's keys — J and K the gas and the brake, though the
        // old file wrote them out as the right stick's, and G and `;` Cross and Triangle — and the
        // original's layout rides it as it always did.
        let (b, _) = Bindings::parse(old);
        let c = Report::idle().axis(REPORT_RIGHT_X);
        let j = held_for_on(&b, Scheme::New, key::J);
        assert_eq!((j.axis(REPORT_RIGHT_X), ride_of(Scheme::New.ride(j)).gas), (c, F::ONE));
        let k = held_for_on(&b, Scheme::New, key::K);
        assert_eq!((k.axis(REPORT_RIGHT_Y), ride_of(Scheme::New.ride(k)).brake), (c, F::ONE));
        for (code, control) in [(key::G, Control::Cross), (key::SEMICOLON, Control::Triangle), (key::H, Control::Square), (key::L, Control::Circle)] {
            assert_eq!(held_for_on(&b, Scheme::New, code).buttons(), control.mask(), "key {code}");
        }
        for code in 0..=255u8 {
            assert_eq!(held_for_on(&b, Scheme::Original, code), held_for(Scheme::Original, code), "key {code}");
            assert_eq!(held_for_on(&b, Scheme::New, code), held_for(Scheme::New, code), "key {code}");
        }
        let after_p40 = old.replace("select = Tab\n", "select = Tab PadBack\n").replace("PadBack PadStart", "PadStart");
        assert_eq!(Bindings::parse(&after_p40), (Bindings::default(), vec![]));
        // One line moved: L1 on R (a key neither layout binds), under both layouts; the rest as
        // each layout has it.
        let (b, _) = Bindings::parse(&old.replace("l1 = A PadL1", "l1 = R PadL1"));
        for scheme in Scheme::ALL {
            assert_eq!(held_for_on(&b, scheme, 27).buttons(), Control::L1.mask());
        }
        assert_eq!(held_for_on(&b, Scheme::New, key::Q).buttons(), 0, "Q is nothing now");
        assert_eq!(held_for_on(&b, Scheme::New, key::W).axis(REPORT_LEFT_Y), 0);
        assert_eq!(gas_of(held_for_on(&b, Scheme::Original, key::X)), F::ONE);
        // The file the shell writes now has every keyboard line as a comment, and reads back the same.
        let text = Bindings::default().text();
        assert!(text.contains("# l1 = Q PadL1  # new\n# l1 = A PadL1  # original\n"), "{text}");
        assert!(text.contains("# left_stick_up = W  # new\n"), "{text}");
    }

    /// [`held_for`] on these bindings.
    fn held_for_on(b: &Bindings, scheme: Scheme, code: u8) -> Report {
        let mut pads = Pads::bound(Wiring::default(), b.clone());
        pads.scheme = scheme;
        let held = |c: u8| c == code;
        for _ in 0..30 {
            pads.build(FRAME, &held);
        }
        pads.build(FRAME, &held)
    }

    /// P54: the Controls screen's list is the bindings' — the brief's keys under the new layout,
    /// today's under the original's, and a key the file moved where it moved to. P73: its rows are
    /// the original's commands by their names, on the keys of the control each scheme puts them on.
    #[test]
    fn the_controls_screen_lists_the_keyboard() {
        let rows = |b: &Bindings, s: Scheme| b.keyboard_layout(s).into_iter().collect::<std::collections::HashMap<_, _>>();
        let new = rows(&Bindings::default(), Scheme::New);
        for (what, keys) in [
            ("Gas", "J Space"), ("Brake", "K Shift"), ("Clutch", "Q"), ("Trick 1", "L"), ("Trick 2", ";"),
            ("Trick 3", "E"), ("Left stick", "W A S D"), ("Right stick", "I Y U O"), ("D-pad", "Arrows"),
            ("Camera", "Tab C"), ("Look back", "H"), ("Reset", "Q + E"), ("Pause", "Enter Escape"),
        ] {
            assert_eq!(new.get(what).map(String::as_str), Some(keys), "{what}");
        }
        assert_eq!(new.len(), 13, "{new:?}");
        let original = rows(&Bindings::default(), Scheme::Original);
        for (what, keys) in [
            ("Gas", "X Space"), ("Brake", "Z"), ("Clutch", "A"), ("Trick 1", "C"), ("Trick 2", "V"),
            ("Trick 3", "D"), ("Right stick", "I J K L"), ("D-pad", "Arrows"), ("Camera", "Tab"),
            ("Look back", "S"), ("Reset", "A + D"), ("Pause", "Enter Escape"),
        ] {
            assert_eq!(original.get(what).map(String::as_str), Some(keys), "{what}");
        }
        assert_eq!(original.len(), 12, "{original:?}");
        assert!(!original.contains_key("Left stick"), "the original's left stick has no keys");
        let (b, _) = Bindings::parse("r2 = G PadR1\n");
        assert_eq!(rows(&b, Scheme::New)["Gas"], "G");
    }

    /// P79: the Controls screen's list under the original's other controller presets is each
    /// preset's table — preset 1's gas and brake swapped, preset 2's third trick on preset 0's
    /// clutch key — and the reset chord stays on L1 + R1's keys on every one. The new layout rides
    /// preset 0 whatever was picked.
    #[test]
    fn the_controls_screen_lists_each_presets_commands() {
        let b = Bindings::default();
        let rows = |p: usize| {
            b.keyboard_layout_on(Scheme::Original, &Table::built([p, 0])).into_iter().collect::<std::collections::HashMap<_, _>>()
        };
        let shipped = rows(0);
        assert_eq!(shipped, b.keyboard_layout(Scheme::Original).into_iter().collect(), "preset 0 is the shipped list");
        assert_eq!((rows(1)["Gas"].as_str(), rows(1)["Brake"].as_str()), (shipped["Brake"].as_str(), shipped["Gas"].as_str()));
        assert_eq!(rows(2)["Trick 3"], shipped["Clutch"]);
        for p in 0..game_sim::bindings::PRESETS {
            assert_eq!(rows(p)["Reset"], shipped["Reset"], "preset {p}");
            assert_eq!(rows(p)["Camera"], shipped["Camera"], "preset {p}");
        }
        assert_eq!(Scheme::New.preset(3), 0);
        assert_eq!(Scheme::Original.preset(3), 3);
        assert_eq!(Scheme::parse_presets("2"), Ok([2; SEATS]));
        assert_eq!(Scheme::parse_presets("0,3"), Ok([0, 3, 3, 3, 3, 3, 3, 3]));
        assert!(Scheme::parse_presets("4").is_err() && Scheme::parse_presets("x").is_err());
        assert!(Scheme::parse_presets("0,0,0,0,0,0,0,0,0").is_err());
    }

    /// P79: the camera button is Select on every preset, held back by the preset's own look-back
    /// and other-camera buttons: L1 under preset 3, R1 under 2 and 3, and no longer R2 there.
    #[test]
    fn each_preset_holds_the_camera_back_on_its_own_buttons() {
        let changes = |preset: usize, hold: Control| {
            let t = Table::built([preset, 0]);
            let mut manager = Manager::connected();
            manager.tick(F(FRAME), &[Report::from_buttons(hold.mask()); PADS]);
            manager.tick(F(FRAME), &[Report::from_buttons(hold.mask() | Control::Select.mask()); PADS]);
            Scheme::Original.changes_camera_on(manager.pad(0), &t)
        };
        assert!(!changes(0, Control::L2) && !changes(0, Control::R2) && changes(0, Control::L1) && changes(0, Control::R1));
        assert!(!changes(2, Control::L2) && !changes(2, Control::R1) && changes(2, Control::R2));
        assert!(!changes(3, Control::L1) && !changes(3, Control::R1) && changes(3, Control::L2) && changes(3, Control::R2));
        assert!(changes(1, Control::Cross), "nothing held back but the two commands");
    }

    #[test]
    fn a_rebound_button_drives_its_new_control() {
        // A button on two lines is both controls, as in opentennis, so triangle is moved off it.
        let (b, _) = Bindings::parse(
            "square = PadNorth G\ntriangle = V\nstick_left = A\ncross = Return\n",
        );
        let mut pads = Pads::bound(Wiring::default(), b);
        pads.button[3] = true;
        let report = pads.build(FRAME, &nothing);
        assert!(down(report, Control::Square) && !down(report, Control::Triangle));
        pads.button[3] = false;
        let g = |code| code == 42;
        assert!(down(pads.build(FRAME, &g), Control::Square));
        // Return is the throttle now, ramped, and Space is not.
        assert_eq!(gas_of(pads.build(FRAME, &|code| code == key::SPACE)), F::ZERO);
        for _ in 0..60 {
            pads.build(FRAME, &|code| code == key::RETURN);
        }
        assert_eq!(gas_of(pads.build(FRAME, &|code| code == key::RETURN)), F::ONE);
        // A on the right stick's left: its byte leaves centre.
        for _ in 0..30 {
            pads.build(FRAME, &|code| code == key::A);
        }
        assert_eq!(pads.build(FRAME, &|code| code == key::A).0[4 + REPORT_RIGHT_X], 0);
    }

    #[test]
    fn a_controls_file_is_written_once_and_read_after() {
        let dir = scratch("controls");
        let path = dir.join("controls.txt");
        let (b, said) = Bindings::load(&path);
        assert_eq!((b, said.len()), (Bindings::default(), 1));
        std::fs::write(&path, "square = B\n").unwrap();
        let (b, said) = Bindings::load(&path);
        assert_eq!((b.lines[0][Control::Square as usize].as_slice(), said.len()), (&[Input::Key(56)][..], 0));
        // What the first run wrote is each layout's default, read back as it was.
        std::fs::remove_file(&path).unwrap();
        Bindings::load(&path);
        assert_eq!(Bindings::load(&path), (Bindings::default(), vec![]));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// An empty directory of this test's own.
    pub(crate) fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("openmx-pad-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A stand-in for a `/dev/input/jsN` node: a fifo the test writes joystick events into and
    /// closes to unplug it — a closed fifo reads end-of-file, as an unplugged pad reads ENODEV.
    pub(crate) struct FakePad {
        path: std::path::PathBuf,
        /// A reader of the test's own, so that the writer can open before [`Seats`] does.
        _hold: std::fs::File,
        writer: std::fs::File,
        /// The events' clock, in milliseconds, and how far it moves each event: a hand's pace
        /// unless a test says otherwise ([`FakePad::gap`]).
        time: u32,
        step: u32,
    }

    /// Held by every test that plugs a [`FakePad`]. A fifo only reads end-of-file once every writer
    /// is closed, and `mkfifo`'s child holds a copy of every open file between its fork and its exec:
    /// one test spawning it while another unplugs would keep that one's pad in for a moment.
    pub(crate) static FIFOS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    impl FakePad {
        pub(crate) fn plug(dir: &std::path::Path, node: &str) -> FakePad {
            use std::os::unix::fs::OpenOptionsExt;
            let path = dir.join(node);
            let made = std::process::Command::new("mkfifo").arg(&path).status().expect("mkfifo");
            assert!(made.success());
            let hold = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(O_NONBLOCK)
                .open(&path)
                .unwrap();
            let writer = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
            FakePad { path, _hold: hold, writer, time: 0, step: 50 }
        }

        pub(crate) fn button(&mut self, number: u8, down: bool) {
            self.send(EVENT_BUTTON, number, down as i16);
        }

        pub(crate) fn axis(&mut self, number: u8, value: i16) {
            self.send(EVENT_AXIS, number, value);
        }

        /// The joystick interface's greeting: every button and axis' state, flagged initial.
        pub(crate) fn init(&mut self, buttons: u8, axes: &[i16]) {
            for n in 0..buttons {
                self.send(EVENT_BUTTON | EVENT_INIT, n, 0);
            }
            for (n, &v) in axes.iter().enumerate() {
                self.send(EVENT_AXIS | EVENT_INIT, n as u8, v);
            }
        }

        /// The milliseconds between the events sent from now on (P109): a bounce is a few.
        pub(crate) fn gap(&mut self, ms: u32) {
            self.step = ms;
        }

        fn send(&mut self, kind: u8, number: u8, value: i16) {
            use std::io::Write;
            self.time = self.time.wrapping_add(self.step);
            let [lo, hi] = value.to_le_bytes();
            let [t0, t1, t2, t3] = self.time.to_le_bytes();
            self.writer.write_all(&[t0, t1, t2, t3, lo, hi, kind, number]).unwrap();
        }

        /// Pull it out: the node goes, and the device reads end-of-file.
        pub(crate) fn unplug(self) {
            std::fs::remove_file(&self.path).unwrap();
        }
    }

    /// A sysfs entry for a node, as the kernel would write one for a USB pad.
    pub(crate) fn sys_entry(sys: &std::path::Path, node: &str, name: &str, phys: &str) {
        let dir = sys.join(node).join("device");
        std::fs::create_dir_all(dir.join("id")).unwrap();
        std::fs::write(dir.join("name"), format!("{name}\n")).unwrap();
        std::fs::write(dir.join("id/vendor"), "045e\n").unwrap();
        std::fs::write(dir.join("id/product"), "028e\n").unwrap();
        std::fs::write(dir.join("uniq"), "\n").unwrap();
        std::fs::write(dir.join("phys"), format!("{phys}\n")).unwrap();
    }

    /// A sysfs entry with the device's ids, keys, axes and properties, as hid-nintendo, hid-playstation
    /// or xpad write theirs: the codes are the kernel's, written as its bitmaps.
    pub(crate) fn sys_device(
        sys: &std::path::Path,
        node: &str,
        name: &str,
        ids: (u16, u16),
        keys: &[u16],
        abs: &[u16],
        props: &[u16],
    ) {
        sys_entry(sys, node, name, &format!("usb-{node}/input0"));
        let dir = sys.join(node).join("device");
        std::fs::write(dir.join("id/vendor"), format!("{:04x}\n", ids.0)).unwrap();
        std::fs::write(dir.join("id/product"), format!("{:04x}\n", ids.1)).unwrap();
        std::fs::create_dir_all(dir.join("capabilities")).unwrap();
        std::fs::write(dir.join("capabilities/key"), bitmap(keys)).unwrap();
        std::fs::write(dir.join("capabilities/abs"), bitmap(abs)).unwrap();
        std::fs::write(dir.join("properties"), bitmap(props)).unwrap();
    }

    /// Codes as a sysfs bitmap: 64-bit hex words, most significant first.
    fn bitmap(codes: &[u16]) -> String {
        let top = codes.iter().map(|&c| c as usize / 64).max().unwrap_or(0);
        let words: Vec<String> = (0..=top)
            .rev()
            .map(|w| {
                let word = codes.iter().filter(|&&c| c as usize / 64 == w).fold(0u64, |m, &c| m | 1 << (c % 64));
                format!("{word:x}")
            })
            .collect();
        words.join(" ") + "\n"
    }

    /// The seats over a test's own device directory and sysfs.
    pub(crate) fn seats_in(dev: &std::path::Path, sys: &std::path::Path, wiring: Wiring) -> Seats {
        Seats::in_dir(dev, sys, None, None, wiring, Bindings::default())
    }

    /// A menu frame now ([`Seats::devices`]), without waiting for the rescan's half second.
    pub(crate) fn devices_now(seats: &mut Seats, held: &dyn Fn(u8) -> bool) -> Vec<(Device<String>, Report, u16)> {
        seats.since_scan = None;
        seats.devices(held)
    }

    /// Ride a frame now, without waiting for the rescan's half second.
    pub(crate) fn frame(seats: &mut Seats, held: &dyn Fn(u8) -> bool) -> [SeatInput; SEATS] {
        seats.since_scan = None;
        seats.frame(held)
    }

    fn playing(seat: SeatInput) -> Report {
        match seat {
            SeatInput::Playing(report) => report,
            other => panic!("not playing: {other:?}"),
        }
    }

    /// The whole thing over devices: pads joining in order, one pulled out mid-ride and its player
    /// paused, a new pad joining past it, and the first one back in its own seat under another
    /// `jsN` — the kernel hands the number out again — because the seat knows it by its ids.
    #[test]
    fn pads_join_drop_and_come_back_to_their_seats() {
        let _fifos = FIFOS.lock().unwrap_or_else(|e| e.into_inner());
        let root = scratch("hotplug");
        let (dev, sys) = (root.join("dev"), root.join("sys"));
        std::fs::create_dir_all(&dev).unwrap();
        sys_entry(&sys, "js0", "Pad A", "usb-1/input0");
        sys_entry(&sys, "js1", "Pad B", "usb-2/input0");
        let mut a = FakePad::plug(&dev, "js0");
        let mut b = FakePad::plug(&dev, "js1");
        let mut seats = Seats::in_dir(&dev, &sys, None, None, Wiring::default(), Bindings::default());

        let now = frame(&mut seats, &nothing);
        assert!(matches!(now[..3], [SeatInput::Playing(_), SeatInput::Playing(_), SeatInput::Empty]));
        assert_eq!(seats.name(0), Some(format!("Pad A ({})", dev.join("js0").display()).as_str()));
        assert_eq!(seats.said(), ["1P: Pad A (".to_string() + &dev.join("js0").display().to_string() + ")", "2P: Pad B (".to_string() + &dev.join("js1").display().to_string() + ")"]);
        // Each pad drives its own seat, and the keyboard rides with 1P.
        b.button(0, true);
        let now = frame(&mut seats, &|code| code == key::H);
        assert!(!down(playing(now[0]), Control::Cross) && down(playing(now[1]), Control::Cross));
        assert!(down(playing(now[0]), Control::Square) && !down(playing(now[1]), Control::Square));
        a.button(1, true);
        let now = frame(&mut seats, &nothing);
        assert!(down(playing(now[0]), Control::Circle));

        // A pulled out: 1P waits, paused, and 2P rides on.
        a.unplug();
        let now = frame(&mut seats, &nothing);
        assert_eq!(now[0], SeatInput::Waiting);
        assert_eq!(now[0].report(), Report::missing());
        assert!(down(playing(now[1]), Control::Cross));
        assert!(seats.said()[0].starts_with("1P: the pad is out"));

        // A third pad joins 3P, not the seat 1P is waiting in.
        sys_entry(&sys, "js2", "Pad C", "usb-3/input0");
        let _c = FakePad::plug(&dev, "js2");
        let now = frame(&mut seats, &nothing);
        assert_eq!(now[0], SeatInput::Waiting);
        assert!(matches!(now[2], SeatInput::Playing(_)));

        // A back, as js3 this time: 1P again.
        sys_entry(&sys, "js3", "Pad A", "usb-1/input0");
        let mut a = FakePad::plug(&dev, "js3");
        a.button(3, true);
        let now = frame(&mut seats, &nothing);
        assert!(down(playing(now[0]), Control::Triangle));
        assert!(seats.said().iter().any(|l| l.starts_with("1P: Pad A") && l.ends_with("is back")));

        // Out again, and this time the keyboard's Start rides on without it; the press that let
        // the pad go is not also the game's Start.
        a.unplug();
        assert_eq!(frame(&mut seats, &nothing)[0], SeatInput::Waiting);
        let now = frame(&mut seats, &|code| code == key::RETURN);
        assert!(!down(playing(now[0]), Control::Start));
        assert!(down(playing(frame(&mut seats, &|code| code == key::RETURN)[0]), Control::Start));
        assert!(!seats.slots.waiting(0));
        drop(b);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_keyboard_is_a_seat() {
        let _fifos = FIFOS.lock().unwrap_or_else(|e| e.into_inner());
        let root = scratch("keys");
        let (dev, sys) = (root.join("dev"), root.join("sys"));
        std::fs::create_dir_all(&dev).unwrap();
        // No pad: the keyboard is 1P, as the shell always was.
        let mut seats = Seats::in_dir(&dev, &sys, None, None, Wiring::default(), Bindings::default());
        let now = frame(&mut seats, &|code| code == key::H);
        assert!(down(playing(now[0]), Control::Square));
        assert_eq!(now[1], SeatInput::Empty);
        assert_eq!(seats.said(), ["1P: the keyboard"]);

        // --pads 0,k: a pad is 1P and the keyboard 2P, each with only its own hands.
        let pads = Seat::parse_list("0,k").unwrap();
        let mut a = FakePad::plug(&dev, "js0");
        let mut seats =
            Seats::in_dir(&dev, &sys, None, Some(pads), Wiring::default(), Bindings::default());
        a.button(0, true);
        let now = frame(&mut seats, &|code| code == key::H);
        assert!(down(playing(now[0]), Control::Cross) && !down(playing(now[0]), Control::Square));
        assert!(down(playing(now[1]), Control::Square) && !down(playing(now[1]), Control::Cross));
        assert_eq!(now[2], SeatInput::Empty);
        // 1P's pad out: 1P waits, and the keyboard stays 2P — it has a seat of its own.
        a.unplug();
        let now = frame(&mut seats, &|code| code == key::H);
        assert_eq!(now[0], SeatInput::Waiting);
        assert!(down(playing(now[1]), Control::Square));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// The ride's hands from one report, through the port's own poll.
    fn ride_of(report: Report) -> Controls {
        let mut manager = Manager::connected();
        manager.tick(F(FRAME), &[report; PADS]);
        Controls::from_pad(manager.pad(0))
    }

    /// P40: under the new layout R2's travel is the original's gas and L2's its brake — the same
    /// fraction the original reads off Cross and Square at that pressure — and the pad's own Cross
    /// and Square, and the original's front brake and clutch on the triggers, do nothing in a ride.
    #[test]
    fn under_the_new_layout_the_triggers_are_the_gas_and_the_brake() {
        let mut pads = Pads::unplugged(Wiring::default());
        let original = |control: Control, pressure: u8| {
            let mut r = Report::idle();
            press(&mut r, control, pressure);
            ride_of(r)
        };
        for travel in [i16::MIN + 1, -20000, -5000, 0, 9000, 20000, i16::MAX] {
            pads.axis[AXIS_R2] = travel;
            pads.axis[AXIS_L2] = TRIGGER_REST;
            let pressure = trigger_pressure(travel);
            let c = ride_of(Scheme::New.ride(pads.build(FRAME, &nothing)));
            assert_eq!(c.gas, original(Control::Cross, pressure).gas, "R2 at {travel}");
            assert_eq!((c.brake, c.front_brake, c.clutch), (F::ZERO, F::ZERO, F::ZERO), "R2 at {travel}");
            if pressure > PRESSURE_THRESHOLD {
                assert!(c.gas > F::ZERO, "R2 at {travel}");
            }

            pads.axis[AXIS_R2] = TRIGGER_REST;
            pads.axis[AXIS_L2] = travel;
            let c = ride_of(Scheme::New.ride(pads.build(FRAME, &nothing)));
            assert_eq!(c.brake, original(Control::Square, pressure).brake, "L2 at {travel}");
            assert_eq!((c.gas, c.front_brake, c.clutch), (F::ZERO, F::ZERO, F::ZERO), "L2 at {travel}");
        }
        // Half way is part of the gas command's travel, and the bike takes that as the whole lever,
        // as the original's does a part-pressed Cross (P77: New's gas is the original's switch).
        pads.axis[AXIS_L2] = TRIGGER_REST;
        pads.axis[AXIS_R2] = 0;
        let report = Scheme::New.ride(pads.build(FRAME, &nothing));
        let half = gas_of(report);
        assert!(half > F(0.3) && half < F(0.7), "half a trigger reads {half:?}");
        assert_eq!(ride_of(report).gas, F::ONE, "half a trigger is the whole switch");

        // The face buttons the original rides on are free: Cross and Square held, nothing.
        pads.axis[AXIS_R2] = TRIGGER_REST;
        pads.button[0] = true; // south: Cross
        pads.button[2] = true; // west: Square
        let report = Scheme::New.ride(pads.build(FRAME, &nothing));
        let c = ride_of(report);
        assert_eq!((c.gas, c.brake), (F::ZERO, F::ZERO));
        assert!(!down(report, Control::Cross) && !down(report, Control::Square));
        // And the same pad under the original's is the original's gas and brake.
        let c = ride_of(Scheme::Original.ride(pads.build(FRAME, &nothing)));
        assert_eq!((c.gas, c.brake), (F::ONE, F::ONE));
    }

    /// P40: Select changes the camera under the new layout — it reaches the port's poll as a press
    /// — and L1 does not; under the original's it is L1, and not when R1 is already down (the reset
    /// chord's second half).
    #[test]
    fn select_cycles_the_camera_under_either_layout() {
        let presses = |scheme: Scheme, frames: &[&[Control]]| {
            let mut manager = Manager::connected();
            let mut out = Vec::new();
            for held in frames {
                let report = scheme.ride(Report::from_buttons(game_sim::input::mask_of(held)));
                manager.tick(F(FRAME), &[report; PADS]);
                out.push(scheme.changes_camera(manager.pad(0)));
            }
            out
        };
        let (none, select, l1) = (&[][..], &[Control::Select][..], &[Control::L1][..]);
        assert_eq!(presses(Scheme::New, &[none, select, select, none, select]), [false, true, false, false, true]);
        assert_eq!(presses(Scheme::New, &[none, l1, none]), [false, false, false]);
        // P73: the original's camera is Select too, and L1 (its clutch) is no camera. Its camera
        // handler skips a Select pressed under look-back (L2) or the other camera (R2); under the new
        // layout L2 and R2 are the brake and the gas and reach the ride as Square and Cross, so they
        // block nothing there.
        assert_eq!(presses(Scheme::Original, &[none, select, select, none, select]), [false, true, false, false, true]);
        assert_eq!(presses(Scheme::Original, &[none, l1, l1, none]), [false, false, false, false]);
        let (l2, l2_select) = (&[Control::L2][..], &[Control::L2, Control::Select][..]);
        let (r2, r2_select) = (&[Control::R2][..], &[Control::R2, Control::Select][..]);
        assert_eq!(presses(Scheme::Original, &[none, l2, l2_select, none, r2, r2_select]), [false; 6]);
        assert_eq!(presses(Scheme::New, &[none, l2, l2_select, none, r2, r2_select]), [false, false, true, false, false, true]);
        // P78: under the new layout look-back and the other camera are on Square and Cross, which
        // hold the camera back as the original's L2 and R2 do.
        let (square, square_select) = (&[Control::Square][..], &[Control::Square, Control::Select][..]);
        let (cross, cross_select) = (&[Control::Cross][..], &[Control::Cross, Control::Select][..]);
        assert_eq!(presses(Scheme::New, &[none, square, square_select, none, cross, cross_select]), [false; 6]);
        assert_eq!(presses(Scheme::Original, &[none, square, square_select, none, cross, cross_select]), [false, false, true, false, false, true]);
        // Select through a pad's own button 6, which a pad with a Select has there.
        let mut pads = Pads::unplugged(Wiring::default());
        pads.button[6] = true;
        assert!(down(Scheme::New.ride(pads.build(FRAME, &nothing)), Control::Select));
    }

    /// P78: the two camera holds the original has on L2 and R2 — look back and the other camera — are
    /// on Square and Cross under the new layout, the faces the gas and the brake left for the
    /// triggers: the pad's Square reaches the ride as L2 and its Cross as R2, bit and pressure, so the
    /// shipped table's commands read them there, and the triggers are the gas and the brake and no
    /// camera. Under the original's they are L2 and R2, as the table says.
    #[test]
    fn the_new_layout_puts_the_camera_holds_on_square_and_cross() {
        let held = |scheme: Scheme, buttons: &[Control]| {
            let mut manager = Manager::connected();
            let mut report = Report::idle();
            for c in buttons {
                press(&mut report, *c, 0x9c);
            }
            manager.tick(F(FRAME), &[scheme.ride(report); PADS]);
            let pad = manager.pad(0);
            let t = &Table::SHIPPED;
            [t.held(pad, Command::LOOK_BACK), t.held(pad, Command::OTHER_CAMERA), t.held(pad, Command::GAS), t.held(pad, Command::BRAKE)]
        };
        assert_eq!(held(Scheme::New, &[Control::Square]), [true, false, false, false]);
        assert_eq!(held(Scheme::New, &[Control::Cross]), [false, true, false, false]);
        assert_eq!(held(Scheme::New, &[Control::R2]), [false, false, true, false]);
        assert_eq!(held(Scheme::New, &[Control::L2]), [false, false, false, true]);
        assert_eq!(held(Scheme::Original, &[Control::L2]), [true, false, false, false]);
        assert_eq!(held(Scheme::Original, &[Control::R2]), [false, true, false, false]);
        assert_eq!(held(Scheme::Original, &[Control::Cross]), [false, false, true, false]);
        // The pressure goes with the bit, both ways round.
        let mut report = Report::idle();
        press(&mut report, Control::Square, 0x40);
        press(&mut report, Control::R2, 0xc0);
        let out = Scheme::New.ride(report);
        let pressure = |r: Report, c: Control| r.pressure(c.binding().pressure);
        assert_eq!([pressure(out, Control::L2), pressure(out, Control::Cross)], [0x40, 0xc0]);
        assert_eq!([pressure(out, Control::Square), pressure(out, Control::R2)], [0, 0]);
        assert!(down(out, Control::L2) && down(out, Control::Cross) && !down(out, Control::Square) && !down(out, Control::R2));
        // And the commands the layout lists say so.
        let on = |scheme: Scheme, c: Command| scheme.commands().into_iter().find(|(x, _)| *x == c).map(|(_, b)| b);
        assert_eq!([on(Scheme::New, Command::LOOK_BACK), on(Scheme::New, Command::OTHER_CAMERA)], [Some(Control::Square), Some(Control::Cross)]);
        assert_eq!([on(Scheme::Original, Command::LOOK_BACK), on(Scheme::Original, Command::OTHER_CAMERA)], [Some(Control::L2), Some(Control::R2)]);
    }

    /// P114: R3's click resets the camera under either layout, once a press — the layouts leave it
    /// on the report as it is — and the keyboard's R3 key (N under the new layout, E under the
    /// original's) reaches it too.
    #[test]
    fn r3_resets_the_camera_under_either_layout() {
        for scheme in Scheme::ALL {
            let mut manager = Manager::connected();
            let mut out = Vec::new();
            for held in [&[][..], &[Control::R3][..], &[Control::R3][..], &[][..], &[Control::R3, Control::L3][..]] {
                let report = scheme.ride(Report::from_buttons(game_sim::input::mask_of(held)));
                manager.tick(F(FRAME), &[report; PADS]);
                out.push(scheme.resets_camera(manager.pad(0)));
            }
            assert_eq!(out, [false, true, false, false, true], "{scheme:?}");
        }
        for (scheme, k) in [(Scheme::New, key::N), (Scheme::Original, key::E)] {
            assert!(down(scheme.ride(held_for(scheme, k)), Control::R3), "{scheme:?}");
        }
    }

    /// P73: what each button does in a ride under each layout, read off the ride itself
    /// (`Controls::from_pad` on the remapped report, a frame up and a frame down). Under the original's
    /// a button does what the shipped table binds it to and a button the table leaves out of the
    /// ride's commands does nothing; the new layout carries every one of the original's commands —
    /// the gas and brake on R2 and L2, the rest where the original has them — and adds none.
    #[test]
    fn each_layout_binds_the_originals_commands_and_no_other() {
        use game_sim::ride::Controls;
        let ride = |scheme: Scheme, held: &[Control]| {
            let mut manager = Manager::connected();
            manager.tick(F(FRAME), &[scheme.ride(Report::from_buttons(0)); PADS]);
            manager.tick(F(FRAME), &[scheme.ride(Report::from_buttons(game_sim::input::mask_of(held))); PADS]);
            format!("{:?}", Controls::from_pad(manager.pad(0)))
        };
        for scheme in Scheme::ALL {
            let rest = ride(scheme, &[]);
            let moves: Vec<Control> = Control::ALL.into_iter().filter(|c| ride(scheme, &[*c]) != rest).collect();
            // The camera's three buttons and the pause are the shell's to follow; the ride reads none.
            let shells = [Command::NEXT_CAMERA, Command::LOOK_BACK, Command::OTHER_CAMERA, Command::PAUSE];
            let want: Vec<Control> =
                scheme.commands().into_iter().filter(|(c, _)| !shells.contains(c)).map(|(_, c)| c).collect();
            for c in &moves {
                assert!(want.contains(c), "{scheme:?}: {c:?} moves the ride and carries no original command");
            }
            for c in &want {
                assert!(moves.contains(c), "{scheme:?}: {c:?} carries a command and moves nothing");
            }
        }
        // The new layout's buttons do exactly what the original's do on the control the command is on.
        let original: Vec<(Command, Control)> = Scheme::Original.commands();
        let new = Scheme::New.commands();
        assert_eq!(original.iter().map(|(c, _)| *c).collect::<Vec<_>>(), Command::RIDE.to_vec());
        assert_eq!(new.iter().map(|(c, _)| *c).collect::<Vec<_>>(), Command::RIDE.to_vec());
        for ((command, was), (_, now)) in original.iter().zip(&new) {
            assert_eq!(ride(Scheme::New, &[*now]), ride(Scheme::Original, &[*was]), "{command:?}");
        }
        // The chord, under both: L1 then R1 puts the rider back.
        for scheme in Scheme::ALL {
            let mut manager = Manager::connected();
            let mut put_back = Vec::new();
            for held in [&[][..], &[Control::L1][..], &[Control::L1, Control::R1][..]] {
                manager.tick(F(FRAME), &[scheme.ride(Report::from_buttons(game_sim::input::mask_of(held))); PADS]);
                put_back.push(Controls::from_pad(manager.pad(0)).respawn);
            }
            assert_eq!(put_back, [false, false, true], "{scheme:?}");
        }
        // Both layouts' camera is the original's next-camera command.
        for scheme in Scheme::ALL {
            assert_eq!(scheme.camera(), Control::Select);
        }
    }

    /// P40: the original's layout is the identity — every report reaches the ride as it was built —
    /// and a seat on it reports what a bare `Pads` (which is the original's) always did, so a replay
    /// or an acceptance run is what it was. 20,000 generated reports, every byte random.
    #[test]
    fn the_original_layout_maps_exactly_as_before() {
        let mut seed = 0x2545_f491_u32;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for _ in 0..20_000 {
            let mut r = Report::idle();
            for b in r.0.iter_mut() {
                *b = next() as u8;
            }
            assert_eq!(Scheme::Original.ride(r), r);
            // A pad that is not there is not remapped by either.
            r.0[0] = 0xff;
            assert_eq!(Scheme::New.ride(r), r);
        }
        // An idle pad is idle under either.
        assert_eq!(Scheme::New.ride(Report::idle()), Report::idle());

        // Through the seats: the keyboard on the original's with the throttle key and a trigger
        // wired to gas is the same report a bare `Pads` built before there were layouts.
        let _fifos = FIFOS.lock().unwrap_or_else(|e| e.into_inner());
        let root = scratch("layouts");
        let (dev, sys) = (root.join("dev"), root.join("sys"));
        std::fs::create_dir_all(&dev).unwrap();
        let wiring = Wiring {
            throttle: Throttle::Trigger(Control::R2),
            schemes: [Scheme::Original; SEATS],
            ..Wiring::default()
        };
        // Each on the first frame, which both answer at no time gone.
        for keys in [&[key::X][..], &[key::Z, key::F], &[key::S, key::TAB], &[key::A, key::D], &[]] {
            let mut seats = Seats::in_dir(&dev, &sys, None, None, wiring.clone(), Bindings::default());
            let mut bare = Pads::unplugged(wiring.clone());
            let held = |code: u8| keys.contains(&code);
            let seat = playing(frame(&mut seats, &held)[0]);
            assert_eq!(Scheme::Original.ride(seat), bare.build(0.0, &held), "keys {keys:?}");
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// P40, the keyboard: under the new layout its throttle key is still the ramped gas (and still
    /// Cross for the menus), and a trigger wired to the gas for the original's is only itself.
    #[test]
    fn the_keyboard_and_the_wiring_under_the_new_layout() {
        let _fifos = FIFOS.lock().unwrap_or_else(|e| e.into_inner());
        let root = scratch("new-keys");
        let (dev, sys) = (root.join("dev"), root.join("sys"));
        std::fs::create_dir_all(&dev).unwrap();
        let wiring = Wiring { throttle: Throttle::Trigger(Control::L2), ..Wiring::default() };
        assert_eq!(wiring.schemes, [Scheme::New; SEATS], "the new layout is the default");
        let mut seats = Seats::in_dir(&dev, &sys, None, None, wiring, Bindings::default());
        // A pad on a new-layout seat, a frame at a time: the ramp, and pinned after it. The new
        // layout's throttle key is Space (P54).
        let x = |code: u8| code == key::SPACE;
        let mut pads = Pads::unplugged(Wiring::default());
        pads.scheme = Scheme::New;
        let mut gas = Vec::new();
        for _ in 0..30 {
            let report = pads.build(FRAME, &x);
            assert!(down(report, Control::Cross) || report.pressure(14) <= PRESSURE_THRESHOLD, "the menus' OK");
            gas.push(ride_of(Scheme::New.ride(report)).gas);
        }
        assert!(gas.windows(2).all(|w| w[1] >= w[0]) && gas[0] < F::ONE, "ramped: {gas:?}");
        assert_eq!(*gas.last().unwrap(), F::ONE);
        // L2 on the keyboard (Shift since P54) is the brake.
        let s = |code: u8| code == key::SHIFT_LEFT;
        let c = ride_of(seats.scheme(0).ride(playing(frame(&mut seats, &s)[0])));
        assert_eq!((c.gas, c.brake, c.clutch), (F::ZERO, F::ONE, F::ZERO));
        std::fs::remove_dir_all(&root).unwrap();

        // And L2's travel on a pad is the brake, whatever --throttle said for the original's.
        let mut pads = Pads::unplugged(Wiring { throttle: Throttle::Trigger(Control::L2), ..Wiring::default() });
        pads.scheme = Scheme::New;
        pads.axis[AXIS_L2] = i16::MAX;
        let c = ride_of(Scheme::New.ride(pads.build(FRAME, &nothing)));
        assert_eq!((c.gas, c.brake, c.clutch), (F::ZERO, F::ONE, F::ZERO));
    }

    #[test]
    fn a_layout_list_reads_and_says_why_not() {
        assert_eq!(Scheme::parse_list("original"), Ok([Scheme::Original; SEATS]));
        let mut two = [Scheme::Original; SEATS];
        two[0] = Scheme::New;
        assert_eq!(Scheme::parse_list("new,original"), Ok(two));
        assert_eq!(Scheme::parse_list(&Scheme::list_text(&two)), Ok(two));
        assert_eq!(Scheme::list_text(&[Scheme::New; SEATS]), "new");
        assert!(Scheme::parse_list("new,old").is_err());
        assert!(Scheme::parse_list("context/controls.txt").is_err());
        assert!(Scheme::parse_list(&["new"; SEATS + 1].join(",")).is_err());
    }

    /// A pad whose sysfs entry lists its keys and has no Select gets its Start on 7, as a full pad
    /// has it, so its Start is not the new layout's camera.
    #[test]
    fn a_pad_with_no_select_keeps_its_start() {
        // The kit's virtual pad's keys (ps2kit/pad.py): A, B, X, Y, TL, TR, Start, Mode and the two
        // thumbs, and no Select, in the kernel's words. Start is js 6.
        let word = [0x130u64, 0x131, 0x133, 0x134, 0x136, 0x137, 0x13b, 0x13c, 0x13d, 0x13e]
            .iter()
            .fold(0u64, |w, c| w | 1 << (c - 0x100));
        let keys = format!("{word:x} 0 0 0 0");
        assert!(has_key(&keys, BTN_SOUTH) && !has_key(&keys, BTN_SELECT) && has_key(&keys, 0x13b));
        assert!(!has_key("", BTN_SOUTH) && !has_key("zz", BTN_SOUTH));

        let _fifos = FIFOS.lock().unwrap_or_else(|e| e.into_inner());
        let root = scratch("no-select");
        let (dev, sys) = (root.join("dev"), root.join("sys"));
        std::fs::create_dir_all(&dev).unwrap();
        sys_entry(&sys, "js0", "Virtual pad", "usb-1");
        std::fs::create_dir_all(sys.join("js0/device/capabilities")).unwrap();
        std::fs::write(sys.join("js0/device/capabilities/key"), format!("{keys}\n")).unwrap();
        let mut a = FakePad::plug(&dev, "js0");
        let mut seats = Seats::in_dir(&dev, &sys, None, None, Wiring::default(), Bindings::default());
        a.button(6, true);
        let now = playing(frame(&mut seats, &nothing)[0]);
        assert!(down(now, Control::Start) && !down(now, Control::Select));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// P46: a uinput pad is skipped though it wears a real pad's ids, as `ps2kit`'s does; a
    /// Bluetooth LE pad, under BlueZ's uhid, is hardware and is read; under Steam Input, Steam's
    /// own virtual pad is the one virtual pad read.
    #[test]
    fn virtual_pads_are_skipped_whatever_ids_they_wear() {
        let root = scratch("virtual");
        let sys = root.join("sys");
        // The kernel's own layout: /sys/class/input/jsN is a link into /sys/devices.
        let entry = |node: &str, under: &str, vendor: &str, product: &str| {
            let real = root.join("devices").join(under).join(node);
            std::fs::create_dir_all(real.join("device/id")).unwrap();
            std::fs::write(real.join("device/name"), "Microsoft X-Box 360 pad\n").unwrap();
            std::fs::write(real.join("device/id/vendor"), format!("{vendor}\n")).unwrap();
            std::fs::write(real.join("device/id/product"), format!("{product}\n")).unwrap();
            std::fs::create_dir_all(&sys).unwrap();
            std::os::unix::fs::symlink(&real, sys.join(node)).unwrap();
            DeviceInfo::read(&sys, node)
        };
        let kit = entry("js0", "virtual/input/input30", "045e", "028e");
        let usb = entry("js1", "pci0000:00/0000:00:14.0/usb3/3-1/3-1:1.0/input/input31", "045e", "028e");
        let ble = entry("js2", "virtual/misc/uhid/0005:045E:0B13.0001/input/input32", "045e", "0b13");
        let steam = entry("js3", "virtual/input/input33", "28de", "11ff");
        assert!(kit.virtual_device && !usb.virtual_device && !ble.virtual_device && steam.virtual_device);
        assert_eq!((kit.vendor, kit.product), (usb.vendor, usb.product), "the same ids");
        assert!(!readable_under(&kit, None) && readable_under(&usb, None) && readable_under(&ble, None));
        assert!(!readable_under(&steam, None));
        let list = [(0x045e, 0x028e)];
        assert!(readable_under(&steam, Some(&list)) && !readable_under(&kit, Some(&list)));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// P46: the seats start from the splash's 1P — its pad in the first seat though it joined last,
    /// the others behind it in join order — and the keyboard rides no pad's seat, except 1P's while
    /// its pad is out. With the keyboard 1P, no pad takes the first seat.
    #[test]
    fn the_seats_start_from_1p() {
        let mut s = PadSlots::<u32>::default();
        s.update(&[5, 7, 8]);
        s.set_first(Some(Device::Pad(8)));
        s.update(&[5, 7, 8]);
        assert_eq!(s.held[..4], [Some(8), Some(5), Some(7), None]);
        assert_eq!(s.keys(), None);
        s.update(&[5, 7]);
        assert!(s.waiting(0));
        assert_eq!(s.keys(), Some(0));
        s.update(&[5, 7, 8]);
        assert_eq!(s.held[0], Some(8));

        let mut s = PadSlots::<u32>::default();
        s.set_first(Some(Device::Keys));
        s.update(&[5, 7]);
        assert_eq!(s.held[..3], [None, Some(5), Some(7)]);
        assert_eq!(s.keys(), Some(0));
        // `--pads` seats stay as they were set up.
        let mut s = PadSlots::<u32>::with_seats(Seat::parse_list("0,k").unwrap());
        s.update(&[5]);
        s.set_first(Some(Device::Keys));
        s.update(&[5]);
        assert_eq!((s.held[0], s.keys()), (Some(5), Some(1)));
    }

    /// P46 over devices: each device's own report for the menus; then 1P a pad that is not the
    /// first one found, in the first seat on its own hands, and the keyboard's report on its own
    /// for the pause; and the keyboard taking the ride over when 1P's pad is pulled out is 1P.
    #[test]
    fn devices_report_on_their_own_and_1p_takes_the_first_seat() {
        let _fifos = FIFOS.lock().unwrap_or_else(|e| e.into_inner());
        let root = scratch("first");
        let (dev, sys) = (root.join("dev"), root.join("sys"));
        std::fs::create_dir_all(&dev).unwrap();
        sys_entry(&sys, "js0", "Pad A", "usb-1/input0");
        sys_entry(&sys, "js1", "Pad B", "usb-2/input0");
        let _a = FakePad::plug(&dev, "js0");
        let mut b = FakePad::plug(&dev, "js1");
        let mut seats = Seats::in_dir(&dev, &sys, None, None, Wiring::default(), Bindings::default());
        b.button(0, true);
        seats.since_scan = None;
        let got = seats.devices(&|code| code == key::H);
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].0, Device::Keys);
        assert!(down(got[0].1, Control::Square) && !down(got[0].1, Control::Cross));
        assert!(!down(got[1].1, Control::Cross) && !down(got[1].1, Control::Square));
        assert!(down(got[2].1, Control::Cross) && !down(got[2].1, Control::Square));
        let Device::Pad(b_id) = got[2].0.clone() else { panic!("a pad") };
        assert_eq!(seats.device_name(&b_id), Some(format!("Pad B ({})", dev.join("js1").display()).as_str()));

        seats.set_first(Some(Device::Pad(b_id.clone())));
        let now = frame(&mut seats, &|code| code == key::H);
        let one = playing(now[0]);
        assert!(down(one, Control::Cross) && !down(one, Control::Square), "1P is B's alone");
        assert!(matches!(now[1], SeatInput::Playing(_)));
        let keys = seats.keyboard_alone().expect("the keyboard rides no seat");
        assert!(down(keys, Control::Square));

        // B out: 1P waits; the keyboard's Start rides on, and the keyboard is 1P.
        b.unplug();
        assert_eq!(frame(&mut seats, &nothing)[0], SeatInput::Waiting);
        frame(&mut seats, &|code| code == key::RETURN);
        assert_eq!(seats.first(), Some(&Device::Keys));
        assert!(down(playing(frame(&mut seats, &|code| code == key::H)[0]), Control::Square));
        assert!(seats.keyboard_alone().is_none());
        std::fs::remove_dir_all(&root).unwrap();
    }

    // ---- P53: pads read by what their keys are, and presses between frames ----------------------

    /// A Switch Pro Controller's keys under hid-nintendo, in code order — read out of the kernel's own
    /// module (7.2.8's `procon_button_mappings`): B is `BTN_SOUTH`, A `BTN_EAST`, X `BTN_NORTH`, Y
    /// `BTN_WEST`, Capture `BTN_Z`, L and R, ZL and ZR as `BTN_TL2`/`BTN_TR2`, - and +, Home, and the
    /// two stick clicks. So its js numbers are B 0, A 1, X 2, Y 3, Capture 4, L 5, R 6, ZL 7, ZR 8,
    /// - 9, + 10, Home 11, L3 12, R3 13. An 8BitDo pad in its Switch mode wears these ids too.
    pub(crate) const PRO_KEYS: [u16; 14] =
        [0x130, 0x131, 0x133, 0x134, 0x135, 0x136, 0x137, 0x138, 0x139, 0x13a, 0x13b, 0x13c, 0x13d, 0x13e];
    /// Its axes, from the same module: the two sticks and the d-pad as a hat, and no trigger axes —
    /// js 0 and 1 the left stick, 2 and 3 the right, 4 and 5 the hat.
    pub(crate) const PRO_ABS: [u16; 6] = [0, 1, 3, 4, ABS_HAT0X, ABS_HAT0Y];
    /// The second device hid-nintendo makes for it: an accelerometer and a gyro on six axes.
    const IMU_ABS: [u16; 6] = [0, 1, 2, 3, 4, 5];
    /// An Xbox pad under xpad: A, B, X, Y (X as `BTN_X`, which is `BTN_NORTH`'s code), the
    /// shoulders, Back, Start, Guide and the thumbs; two sticks, two trigger axes and a hat.
    pub(crate) const XPAD_KEYS: [u16; 11] = [0x130, 0x131, 0x133, 0x134, 0x136, 0x137, 0x13a, 0x13b, 0x13c, 0x13d, 0x13e];
    pub(crate) const PAD_ABS: [u16; 8] = [0, 1, ABS_Z, 3, 4, ABS_RZ, ABS_HAT0X, ABS_HAT0Y];
    /// A DualSense under hid-playstation: the face buttons by position (Triangle `BTN_NORTH`, Square
    /// `BTN_WEST`), digital L2 and R2 beside their axes, Create, Options, PS and the thumbs.
    const DUALSENSE_KEYS: [u16; 13] =
        [0x130, 0x131, 0x133, 0x134, 0x136, 0x137, 0x138, 0x139, 0x13a, 0x13b, 0x13c, 0x13d, 0x13e];

    fn pressure(report: Report, control: Control) -> u8 {
        report.0[control.binding().pressure as usize]
    }

    /// One pad's report this menu frame, by js node.
    fn pad_report(seats: &mut Seats, node: &str) -> (Report, u16) {
        let got = devices_now(seats, &nothing);
        let id = seats.joined.iter().find(|j| j.path.ends_with(node)).expect("joined").id.clone();
        got.into_iter().find(|(d, _, _)| *d == Device::Pad(id.clone())).map(|(_, r, t)| (r, t)).unwrap()
    }

    /// P53, the user's pad: a Switch Pro Controller's B (the bottom one) confirms and A goes back
    /// (P64: by position unless the wiring says labels), + is Start and - Select, ZR is a full R2
    /// (the new scheme's gas), its d-pad is the d-pad and its right stick the right stick — where
    /// before P53 they were Circle, Cross, R3, L3, nothing, the right stick, and L2. Its IMU device
    /// is not read, and the splash's list says so. By label, A is Cross.
    #[test]
    fn a_switch_pro_controller_reads_as_its_labels_say() {
        let _fifos = FIFOS.lock().unwrap_or_else(|e| e.into_inner());
        let root = scratch("procon");
        let (dev, sys) = (root.join("dev"), root.join("sys"));
        std::fs::create_dir_all(&dev).unwrap();
        let name = "Nintendo Switch Pro Controller";
        sys_device(&sys, "js0", name, (0x057e, 0x2009), &PRO_KEYS, &PRO_ABS, &[]);
        sys_device(&sys, "js1", &format!("{name} (IMU)"), (0x057e, 0x2009), &[], &IMU_ABS, &[INPUT_PROP_ACCELEROMETER as u16]);
        let mut pro = FakePad::plug(&dev, "js0");
        let mut imu = FakePad::plug(&dev, "js1");
        pro.init(14, &[0; 6]);
        imu.init(0, &[1200, -800, 16000, 30, -12, 7]);
        let mut seats = seats_in(&dev, &sys, Wiring::default());
        assert_eq!(seats.joined.len(), 1, "the IMU is not a pad");
        assert_eq!(seats.skipped(), [format!("{name} (IMU)")]);

        let (idle, _) = pad_report(&mut seats, "js0");
        assert_eq!(idle.buttons(), 0);
        assert_eq!((pressure(idle, Control::L2), pressure(idle, Control::R2)), (0, 0));
        let held = |seats: &mut Seats, pro: &mut FakePad, js: u8| {
            pro.button(js, true);
            let (r, _) = pad_report(seats, "js0");
            pro.button(js, false);
            pad_report(seats, "js0");
            r
        };
        assert_eq!(held(&mut seats, &mut pro, 0).buttons(), Control::Cross.mask(), "B, the bottom one");
        assert_eq!(held(&mut seats, &mut pro, 1).buttons(), Control::Circle.mask(), "A, the right one");
        assert_eq!(held(&mut seats, &mut pro, 2).buttons(), Control::Triangle.mask(), "X, the top one");
        assert_eq!(held(&mut seats, &mut pro, 3).buttons(), Control::Square.mask(), "Y, the left one");
        assert_eq!(held(&mut seats, &mut pro, 10).buttons(), Control::Start.mask(), "+");
        assert_eq!(held(&mut seats, &mut pro, 9).buttons(), Control::Select.mask(), "-");
        assert_eq!(held(&mut seats, &mut pro, 5).buttons(), Control::L1.mask(), "L");
        assert_eq!(held(&mut seats, &mut pro, 6).buttons(), Control::R1.mask(), "R");
        assert_eq!(held(&mut seats, &mut pro, 4).buttons(), 0, "Capture");
        assert_eq!(held(&mut seats, &mut pro, 11).buttons(), 0, "Home");
        assert_eq!(held(&mut seats, &mut pro, 12).buttons(), Control::L3.mask());
        let zr = held(&mut seats, &mut pro, 8);
        assert_eq!((pressure(zr, Control::R2), pressure(zr, Control::L2)), (255, 0), "ZR is all of R2");
        assert!(down(zr, Control::R2));
        assert_eq!(pressure(held(&mut seats, &mut pro, 7), Control::L2), 255, "ZL");

        pro.axis(2, i16::MAX);
        pro.axis(5, -32767);
        let (r, _) = pad_report(&mut seats, "js0");
        assert_eq!((r.axis(REPORT_RIGHT_X), r.axis(REPORT_RIGHT_Y)), (0xff, AXIS_CENTRE));
        assert_eq!(r.buttons(), Control::Up.mask(), "the hat's up is the d-pad's");
        assert_eq!((pressure(r, Control::L2), pressure(r, Control::R2)), (0, 0), "no trigger moved");
        pro.axis(2, 0);
        pro.axis(5, 0);

        // By label, A (the right one) is Cross, as on a Switch.
        seats.set_wiring(Wiring { nintendo_positions: false, ..Wiring::default() });
        assert_eq!(held(&mut seats, &mut pro, 1).buttons(), Control::Cross.mask());
        assert_eq!(held(&mut seats, &mut pro, 0).buttons(), Control::Circle.mask());
        drop(imu);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// P53: the same js numbers are other buttons on other pads. On an Xbox pad (xpad) 2 is X, the
    /// left button — Square — and 6 and 7 are Back and Start; on a DualSense (hid-playstation) 2 is
    /// Triangle, 3 Square, 6 and 7 the digital L2 and R2 (read from their axes instead) and 9 Options.
    #[test]
    fn an_xbox_pad_and_a_dualsense_read_by_their_keys() {
        let _fifos = FIFOS.lock().unwrap_or_else(|e| e.into_inner());
        let root = scratch("by-keys");
        let (dev, sys) = (root.join("dev"), root.join("sys"));
        std::fs::create_dir_all(&dev).unwrap();
        sys_device(&sys, "js0", "Microsoft X-Box 360 pad", (0x045e, 0x028e), &XPAD_KEYS, &PAD_ABS, &[]);
        sys_device(&sys, "js1", "Sony Interactive Entertainment DualSense Wireless Controller", (0x054c, 0x0ce6), &DUALSENSE_KEYS, &PAD_ABS, &[]);
        let mut xbox = FakePad::plug(&dev, "js0");
        let mut ds = FakePad::plug(&dev, "js1");
        let rest = [0, 0, -32767, 0, 0, -32767, 0, 0];
        xbox.init(11, &rest);
        ds.init(13, &rest);
        let mut seats = seats_in(&dev, &sys, Wiring::default());
        let mut press = |pad: &mut FakePad, node: &str, js: u8| {
            pad.button(js, true);
            let (r, _) = pad_report(&mut seats, node);
            pad.button(js, false);
            pad_report(&mut seats, node);
            r.buttons()
        };
        let x: Vec<u16> = [0, 1, 2, 3, 6, 7].iter().map(|&js| press(&mut xbox, "js0", js)).collect();
        let want = [Control::Cross, Control::Circle, Control::Square, Control::Triangle, Control::Select, Control::Start];
        assert_eq!(x, want.map(Control::mask));
        let d: Vec<u16> = [0, 1, 2, 3, 6, 7, 8, 9].iter().map(|&js| press(&mut ds, "js1", js)).collect();
        let want = [
            Control::Cross.mask(),
            Control::Circle.mask(),
            Control::Triangle.mask(),
            Control::Square.mask(),
            0,
            0,
            Control::Select.mask(),
            Control::Start.mask(),
        ];
        assert_eq!(d, want);
        // The triggers are their axes on both.
        ds.axis(5, i16::MAX);
        let (r, _) = pad_report(&mut seats, "js1");
        assert_eq!((pressure(r, Control::R2), pressure(r, Control::L2)), (255, 0));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// P53: a press and its release before the next frame — a tap, at 5 fps — is a press for that
    /// frame: in the report and in its taps. Two inside one frame are two presses, one a frame. The
    /// hat's are the same. A pad in no seat during a ride keeps none for the menu after it.
    #[test]
    fn a_tap_between_two_frames_is_still_a_press() {
        let _fifos = FIFOS.lock().unwrap_or_else(|e| e.into_inner());
        let root = scratch("taps");
        let (dev, sys) = (root.join("dev"), root.join("sys"));
        std::fs::create_dir_all(&dev).unwrap();
        sys_device(&sys, "js0", "Microsoft X-Box 360 pad", (0x045e, 0x028e), &XPAD_KEYS, &PAD_ABS, &[]);
        let mut a = FakePad::plug(&dev, "js0");
        a.init(11, &[0, 0, -32767, 0, 0, -32767, 0, 0]);
        let mut seats = seats_in(&dev, &sys, Wiring::default());
        assert_eq!(pad_report(&mut seats, "js0").1, 0, "the greeting is nobody's press");

        a.button(7, true);
        a.button(7, false);
        let (r, taps) = pad_report(&mut seats, "js0");
        assert!(down(r, Control::Start));
        assert_eq!(taps, Control::Start.mask());
        let (r, taps) = pad_report(&mut seats, "js0");
        assert!(!down(r, Control::Start) && taps == 0);

        // Held across the frame, then let go and pressed again inside the next: down in both
        // reports, and the second press is in the taps.
        a.button(0, true);
        assert_eq!(pad_report(&mut seats, "js0").1, Control::Cross.mask());
        a.button(0, false);
        a.button(0, true);
        let (r, taps) = pad_report(&mut seats, "js0");
        assert!(down(r, Control::Cross) && taps == Control::Cross.mask());
        a.button(0, false);
        assert_eq!(pad_report(&mut seats, "js0").1, 0);

        // Two taps of the d-pad's down inside one frame: two frames of presses.
        for _ in 0..2 {
            a.axis(7, i16::MAX);
            a.axis(7, 0);
        }
        let taps: Vec<u16> = (0..3).map(|_| pad_report(&mut seats, "js0").1).collect();
        assert_eq!(taps, [Control::Down.mask(), Control::Down.mask(), 0]);

        // In a ride with the seats set up for nobody's pad, a tap is not kept for the menu after.
        let mut seats_none = Seats::in_dir(&dev, &sys, None, Some([None; SEATS]), Wiring::default(), Bindings::default());
        devices_now(&mut seats_none, &nothing);
        drop(seats);
        a.button(7, true);
        a.button(7, false);
        frame(&mut seats_none, &nothing);
        let got = devices_now(&mut seats_none, &nothing);
        assert!(got.iter().all(|(_, _, taps)| *taps == 0), "{got:?}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// P109: one push of the d-pad is one press in the taps, however the pad sends it — as a hat and
    /// as d-pad buttons both, or with its contact bouncing up and down again inside a few
    /// milliseconds. A hand's second press, further apart than [`BOUNCE_MS`], is still its own.
    #[test]
    fn one_d_pad_push_is_one_press_from_both_sources_and_through_a_bounce() {
        let _fifos = FIFOS.lock().unwrap_or_else(|e| e.into_inner());
        let root = scratch("bounce");
        let (dev, sys) = (root.join("dev"), root.join("sys"));
        std::fs::create_dir_all(&dev).unwrap();
        // A Pro Controller's codes with the d-pad's buttons as well (js 14 to 17: up, down, left,
        // right), the hat at js 4 and 5.
        let mut keys = PRO_KEYS.to_vec();
        keys.extend([0x220, 0x221, 0x222, 0x223]);
        sys_device(&sys, "js0", "Nintendo Switch Pro Controller", (0x057e, 0x2009), &keys, &PRO_ABS, &[]);
        let mut a = FakePad::plug(&dev, "js0");
        a.init(18, &[0; 6]);
        let mut seats = seats_in(&dev, &sys, Wiring::default());
        assert_eq!(pad_report(&mut seats, "js0").1, 0);
        let taps = |seats: &mut Seats, n: usize| -> Vec<u16> { (0..n).map(|_| pad_report(seats, "js0").1).collect() };
        let down = Control::Down.mask();

        // The hat and the button for the one push, a millisecond apart, and both let go.
        a.gap(1);
        a.axis(5, i16::MAX);
        a.button(15, true);
        assert_eq!(taps(&mut seats, 3), [down, 0, 0]);
        a.axis(5, 0);
        a.button(15, false);
        assert_eq!(taps(&mut seats, 1), [0]);

        // A bounce: down, up three milliseconds later and down again, inside one frame.
        a.gap(50);
        a.axis(5, i16::MAX);
        a.gap(3);
        a.axis(5, 0);
        a.axis(5, i16::MAX);
        assert_eq!(taps(&mut seats, 3), [down, 0, 0]);
        // And across a frame: up in one, down again in the next, still within the bounce.
        a.axis(5, 0);
        assert_eq!(taps(&mut seats, 1), [0]);
        a.axis(5, i16::MAX);
        assert_eq!(taps(&mut seats, 2), [0, 0]);

        // A hand lets go and presses again further apart: two presses.
        a.gap(50);
        a.axis(5, 0);
        a.axis(5, i16::MAX);
        assert_eq!(taps(&mut seats, 2), [down, 0]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// P53: a node with no sysfs entry is read as an Xbox pad's numbers, as before; the map from a
    /// device's codes otherwise.
    #[test]
    fn the_map_follows_the_codes() {
        let id = JsMap::identity();
        assert_eq!((id.button(7), id.axis(5), id.axis(8)), (JsButton::Pad(7), Some(AXIS_R2), None));
        let pro = JsMap::of(&PRO_KEYS, &PRO_ABS, true);
        assert_eq!(pro.button(10), JsButton::Pad(7));
        assert_eq!(pro.button(8), JsButton::Trigger(AXIS_R2));
        assert_eq!((pro.axis(2), pro.axis(4), pro.axis(5)), (Some(AXIS_RIGHT_X), Some(AXIS_HAT_X), Some(AXIS_HAT_Y)));
        let joycon = JsMap::of(&[0x130, 0x131, 0x220, 0x221, 0x222, 0x223], &[0, 1], true);
        assert_eq!(joycon.button(2), JsButton::Dpad(Control::Up));
        assert_eq!(joycon.button(5), JsButton::Dpad(Control::Right));
        // The BTN_MISC block comes after the gamepad's in the joystick interface's numbering.
        let keys = format!("{} 0 0 0 0\n", format!("{:x}", (1u64 << 48) | 1));
        assert_eq!(js_order(&keys, (BTN_JOYSTICK..=KEY_MAX).chain(BTN_MISC..BTN_JOYSTICK)), [0x130, 0x100]);
    }
}
