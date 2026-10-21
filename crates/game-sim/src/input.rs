//! The input layer: the pad, as the game's own code keeps it.
//!
//! The original reads the pad **once a frame, before the simulation**, and that single read is the whole
//! input layer: there is no polling anywhere else, and nothing in the simulation ever looks at the
//! hardware. One frame is, in order: sample the clock, derive `dt`, **update the input manager**, run
//! the simulation, render. So a press is latched before the frame that acts on it, and the frame that
//! services a state change (a menu being torn down, a race starting) latches the pad *after* the change
//! — which is why a button held across a transition is seen by the new state on its very first frame.
//!
//! # What the original keeps, and what it does not
//!
//! The thing to know before reading any of this: **the game keeps no "pressed this frame" bitmask, and
//! no per-pad bitmask at all.** It keeps sixteen *controls* per pad ([`Control`]), each a six-word
//! record ([`ControlState`]):
//!
//! | word | meaning |
//! |---|---|
//! | `held` | 1 while down, 0 otherwise |
//! | `strength` | 1.0 for a digital control; `(pressure + 1) / 256` for an analogue one |
//! | `since_release_ms` | ms since this control was last released |
//! | `held_ms` | ms this control has been held |
//! | `prev_since_release_ms` | last frame's `since_release_ms` |
//! | `prev_held_ms` | last frame's `held_ms` |
//!
//! and it raises an **event** ([`Event`]) on each transition, onto a queue that is cleared at the top of
//! every update and handed to the front end at the bottom of the *same* update ([`EventQueue`]). Nothing
//! about the edge survives into the next frame except the timers: on the frame of a press `held_ms` is
//! 0, and on the frame of a release `since_release_ms` is 0. That is the whole of "pressed this frame"
//! in this game, and it is why [`ControlState::pressed`] is spelled the way it is.
//!
//! # Mapping a pad to the game's own bitmask
//!
//! The port's own layers speak in [`Control`]s, but the original speaks in libpad's button word, so the
//! two mappings both live here:
//!
//! * [`Report`] is the 32 bytes `scePadRead` leaves behind — status, mode, the **active-low**
//!   big-endian button word, the four stick axes, the twelve button pressures. A host pad is mapped
//!   into the game's own bitmask ([`Control::mask`]) and then into a report with
//!   [`Report::from_buttons`], which is the only place the port invents bytes the hardware would have
//!   produced.
//! * [`Pad::poll`] is the game's own read of that report: it turns the word and the pressures into the
//!   sixteen control records and the events, in the original's order and with the original's arithmetic.
//!
//! The binding of control to bit and pressure byte is [`CONTROL_MAP`]. In the original that table is
//! built at runtime when a pad is bound (an unbound second pad has it all zero), so it is not read-only
//! data in the image; it is reproduced here as a constant, from the game's own RAM.
//!
//! # The sticks are not controls
//!
//! The four stick halves are none of the sixteen: the same poll walks six separate *axes* ([`Axis`]),
//! each a report byte stored as a float and then mapped onto −1..1 through a dead zone
//! ([`AxisRange`]). The dead zone is ±0.2 of the mapped range — a fifth of the travel either side of
//! centre does nothing — and each segment outside it is stretched back out to the full range, so the
//! rest of the stick is 1.25× steeper than the byte. There is no curve beyond those two straight
//! segments and no clamp anywhere (a report byte cannot leave the range), and the arithmetic is one
//! divide, one multiply and one add, in that order (I2).
//!
//! # Timers are integers, re-read every frame
//!
//! `held_ms += dt * 1000` is done in float and **stored back as an integer every frame**, so the
//! fraction is discarded each time and not accumulated: at the 30 fps cap (`dt` = 0.03333, so 33.33 ms)
//! the sequence is 0, 33, 66, 99, 132, 165, 198 — a flat +33 a frame, not the +33, +33, +34 a float
//! accumulator would give. A port that keeps a float accumulator drifts one millisecond per frame
//! against the original, which is one frame of difference in a 33-frame menu repeat. The conversions
//! both ways are the *unsigned* ones a compiler emits on this hardware, so the timers are `u32` and
//! their arithmetic goes through [`ps2_float`] like everything else in the simulation.
//!
//! # The loop
//!
//! ```
//! use game_sim::input::{Control, Manager, Report};
//! use ps2_float::F;
//!
//! let mut input = Manager::connected(); // the platform layer says there is a pad here
//! let dt = F(0.03333); // the engine's 30 fps cap
//! let down = Report::from_buttons(Control::Cross.mask());
//!
//! input.tick(dt, &[down, Report::missing()]);
//! assert!(input.pad(0).held(Control::Cross));
//! assert!(input.pad(0).pressed(Control::Cross)); // the frame of the press, and only that frame
//!
//! input.tick(dt, &[down, Report::missing()]);
//! assert!(input.pad(0).held(Control::Cross));
//! assert!(!input.pad(0).pressed(Control::Cross));
//! assert_eq!(input.pad(0).control(Control::Cross).held_ms, 33);
//! ```

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use ps2_float::{F, fpu};

use crate::hash::{self, State, Walk};

/// How many controls a pad has. Not a count of buttons: four of the sixteen are the same four buttons a
/// DualShock 2 reports no pressure for, and the four stick axes are not controls at all.
pub const CONTROLS: usize = 16;

/// How many pads the input manager makes. The original creates two and opens all eight port/slot pairs
/// behind them, so a multitap is a matter of which pad a report comes from, not of more pads.
pub const PADS: usize = 2;

/// The sixteen controls, in the order the game's own map holds them — which is the order events are
/// queued in, so it is part of the observable behaviour and not a presentation choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Control {
    Triangle = 0,
    Circle,
    Cross,
    Square,
    L1,
    R1,
    L2,
    R2,
    /// Present in the map and reachable by the game, but the kit's virtual pad refuses to send it
    /// (select plus a shoulder button is PCSX2's own hotkey), so its bit is the one that was never
    /// measured live.
    Select,
    Start,
    R3,
    L3,
    Up,
    Right,
    Down,
    Left,
}

impl Control {
    /// Every control, in map order.
    pub const ALL: [Control; CONTROLS] = {
        use Control::*;
        [
            Triangle, Circle, Cross, Square, L1, R1, L2, R2, Select, Start, R3, L3, Up, Right, Down,
            Left,
        ]
    };

    /// Its index in the map, which is what an [`Event`] carries.
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The control at an index, or `None` past the sixteenth.
    pub const fn from_index(i: usize) -> Option<Control> {
        if i < CONTROLS {
            Some(Control::ALL[i])
        } else {
            None
        }
    }

    /// Its bit in the game's own button bitmask — libpad's word, read active *high*.
    pub const fn mask(self) -> u16 {
        CONTROL_MAP[self as usize].mask
    }

    /// How this control is read off a pad report.
    pub const fn binding(self) -> Binding {
        CONTROL_MAP[self as usize]
    }

    /// The name the project calls it, for a dump or a log line.
    pub const fn name(self) -> &'static str {
        CONTROL_NAMES[self as usize]
    }
}

/// The names, in map order, so a dump reads as words rather than indices.
const CONTROL_NAMES: [&str; CONTROLS] = [
    "triangle", "circle", "cross", "square", "l1", "r1", "l2", "r2", "select", "start", "r3", "l3",
    "up", "right", "down", "left",
];

/// `flags` bit 1: this control is read from its pressure byte rather than from its bit. The poll reads
/// no other bit of `flags`, though every bound entry also has bit 0 set.
pub const ANALOGUE: u8 = 0x02;

/// A pressure byte above this counts as held. Strictly above: 0x19 itself is not a press.
pub const PRESSURE_THRESHOLD: u8 = 0x19;

/// What the game's own bitmask is worth as a force: a digital control reads 1.0 when it is down.
pub const DIGITAL_STRENGTH: F = F(1.0);

/// 1/256, the factor a pressure byte is scaled by. A power of two, so the scaling itself is exact and
/// the only question is the integer conversion before it.
const PRESSURE_SCALE: f32 = 0.003_906_25;

/// How one control is read off a pad report: the flags byte, the index of its pressure byte in the
/// report, and its bit in the button word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub flags: u8,
    pub pressure: u8,
    pub mask: u16,
}

impl Binding {
    /// Read from the pressure byte (above the threshold) rather than from the bit.
    pub const fn is_analogue(&self) -> bool {
        self.flags & ANALOGUE != 0
    }
}

/// The game's binding of control to bit and pressure byte, read out of the game's own RAM.
///
/// The pressure indices are the standard DualShock 2 order (right, left, up, down, triangle, circle,
/// cross, square, L1, R1, L2, R2 at report bytes 8 to 19), and the four controls with no pressure byte
/// are exactly the four a DualShock 2 reports none for. **The analogue entries are read from their
/// pressure byte only** — the poll never looks at their bit — so a pad that reports no pressures leaves
/// twelve of the sixteen controls permanently released, however much its button word says.
pub const CONTROL_MAP: [Binding; CONTROLS] = [
    Binding { flags: 0x03, pressure: 12, mask: 0x0010 }, // triangle
    Binding { flags: 0x03, pressure: 13, mask: 0x0020 }, // circle
    Binding { flags: 0x03, pressure: 14, mask: 0x0040 }, // cross
    Binding { flags: 0x03, pressure: 15, mask: 0x0080 }, // square
    Binding { flags: 0x03, pressure: 16, mask: 0x0004 }, // L1
    Binding { flags: 0x03, pressure: 17, mask: 0x0008 }, // R1
    Binding { flags: 0x03, pressure: 18, mask: 0x0001 }, // L2
    Binding { flags: 0x03, pressure: 19, mask: 0x0002 }, // R2
    Binding { flags: 0x01, pressure: 0, mask: 0x0100 },  // select
    Binding { flags: 0x01, pressure: 0, mask: 0x0800 },  // start
    Binding { flags: 0x01, pressure: 0, mask: 0x0400 },  // R3
    Binding { flags: 0x01, pressure: 0, mask: 0x0200 },  // L3
    Binding { flags: 0x03, pressure: 10, mask: 0x1000 }, // up
    Binding { flags: 0x03, pressure: 8, mask: 0x2000 },  // right
    Binding { flags: 0x03, pressure: 11, mask: 0x4000 }, // down
    Binding { flags: 0x03, pressure: 9, mask: 0x8000 },  // left
];

/// The bitmask of a list of controls, for a caller mapping a host pad or writing a test.
pub fn mask_of(controls: &[Control]) -> u16 {
    let mut m = 0;
    for &c in controls {
        m |= c.mask();
    }
    m
}

// ---------------------------------------------------------------------------------------------------
// The report: what the pad library leaves in RAM
// ---------------------------------------------------------------------------------------------------

/// How many bytes one pad read produces.
pub const REPORT_BYTES: usize = 32;

/// A centred stick axis.
pub const AXIS_CENTRE: u8 = 0x7f;

/// The mode byte of a DualShock 2 reporting pressures — what the game asks every pad to become, and
/// what every measurement in this project was taken against.
pub const MODE_PRESSURES: u8 = 0x79;

/// One pad read: the 32 bytes the pad library hands over.
///
/// Byte 0 is the status (0 means the read is good; **any** non-zero value means "no pad" to the poll),
/// byte 1 the mode, bytes 2 and 3 the button word big-endian and **active low** — a bit is 0 while its
/// button is down — bytes 4 to 7 the right and left sticks, and bytes 8 to 19 the twelve pressures.
///
/// The pad library keeps two of these per port and refreshes them alternately, once per video field,
/// which is twice as often as the game reads them; a capture taken after a tick therefore sees the
/// *other* half refreshed, and the half the game actually read is the one with the smaller stamp. That
/// matters for comparing a port against a capture, and not at all for the port itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report(pub [u8; REPORT_BYTES]);

impl Default for Report {
    fn default() -> Report {
        Report::missing()
    }
}

impl Report {
    /// A good read of a pad with nothing pressed: sticks centred, every pressure zero.
    pub fn idle() -> Report {
        let mut bytes = [0u8; REPORT_BYTES];
        bytes[1] = MODE_PRESSURES;
        bytes[2] = 0xff;
        bytes[3] = 0xff;
        bytes[4] = AXIS_CENTRE;
        bytes[5] = AXIS_CENTRE;
        bytes[6] = AXIS_CENTRE;
        bytes[7] = AXIS_CENTRE;
        Report(bytes)
    }

    /// No pad: a non-zero status byte, which is all the poll tests. The poll's other "no pad" path is
    /// the read failing outright, and it takes the same branch as this one.
    pub fn missing() -> Report {
        let mut r = Report::idle();
        r.0[0] = 0xff;
        r
    }

    /// The report a pad holding exactly these buttons would produce: the game's own bitmask, written
    /// back into the word active-low, with a full pressure byte for every analogue control in it.
    ///
    /// This is the port's seam with the host: a keyboard, an XInput pad or a replay fixture becomes a
    /// bitmask, and the bitmask becomes the bytes the original's poll reads. Full pressure (255, so
    /// strength 1.0) is what a digital source has to mean, and is also exactly what the kit's virtual
    /// pad produces, so a capture driven by it replays through here unchanged.
    pub fn from_buttons(buttons: u16) -> Report {
        let mut r = Report::idle();
        let low = !buttons;
        r.0[2] = (low >> 8) as u8;
        r.0[3] = low as u8;
        for binding in CONTROL_MAP {
            if binding.is_analogue() && buttons & binding.mask != 0 {
                r.0[binding.pressure as usize] = 0xff;
            }
        }
        r
    }

    /// The bytes of a report as the pad library left them — for a capture replayed through the port.
    pub fn from_bytes(bytes: [u8; REPORT_BYTES]) -> Report {
        Report(bytes)
    }

    /// The status byte: 0 is a good read.
    pub fn status(&self) -> u8 {
        self.0[0]
    }

    /// The mode byte.
    pub fn mode(&self) -> u8 {
        self.0[1]
    }

    /// The button word as the poll computes it: the two bytes big-endian, inverted, so a bit is 1 while
    /// its button is down. This is the game's own bitmask.
    pub fn buttons(&self) -> u16 {
        (((self.0[2] as u16) << 8) | self.0[3] as u16) ^ 0xffff
    }

    /// One of the twelve pressures, by its index in the report (8 to 19).
    pub fn pressure(&self, index: u8) -> u8 {
        self.0[index as usize]
    }

    /// A stick axis byte, 0 to 3 being right X, right Y, left X, left Y — the report's own order, which
    /// is not the pad's axis order ([`AXIS_MAP`] is that). The poll reads these bytes through the map
    /// rather than through here.
    pub fn axis(&self, index: usize) -> u8 {
        self.0[4 + index]
    }

    /// Write one stick axis byte, in the report's own order — the other half of the port's seam with a
    /// host pad, the way [`Report::from_buttons`] is for the buttons.
    pub fn set_axis(&mut self, index: usize, byte: u8) {
        self.0[4 + index] = byte;
    }
}

// ---------------------------------------------------------------------------------------------------
// One control's state
// ---------------------------------------------------------------------------------------------------

/// The six words the game keeps per control, in the order it keeps them.
///
/// The timers are milliseconds as unsigned integers, and in the original they are **never
/// initialised**: the constructor zeroes `held` and `strength` and leaves the four timers as whatever
/// was in the heap, so a control that has not changed state since the game booted has a large
/// meaningless `since_release_ms` that grows by a frame's worth each frame. The port starts them at 0
/// instead — the one place this module is knowingly not bit-identical, and harmless because every timer
/// is zeroed by the first transition of its own control and nothing reads one before then. A capture
/// replayed through the port therefore has to be seeded with the first row's values.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ControlState {
    /// 1 while down, 0 otherwise. An integer because that is what the game stores and compares.
    pub held: i32,
    /// 1.0 for a digital control, `(pressure + 1) / 256` for an analogue one, 0.0 while released.
    pub strength: F,
    /// Milliseconds since this control was last released; 0 on the frame of the release.
    pub since_release_ms: u32,
    /// Milliseconds this control has been held; 0 on the frame of the press.
    pub held_ms: u32,
    /// Last frame's [`ControlState::since_release_ms`].
    pub prev_since_release_ms: u32,
    /// Last frame's [`ControlState::held_ms`].
    pub prev_held_ms: u32,
}

impl ControlState {
    /// Down this frame.
    pub fn is_held(&self) -> bool {
        self.held != 0
    }

    /// Went down this frame — held, with the held timer still reading 0, which is the only mark the
    /// game leaves on a press once the event queue has been handed over and cleared.
    ///
    /// Exactly equivalent to "this frame's queue holds a press for this control" whenever `dt` is
    /// non-zero. On a frame where `dt` is zero — a paused clock, a single-stepped debug frame — the
    /// timer cannot leave 0 and this reads true for as long as the pause lasts, while the queue holds
    /// the press for one frame only. The original has the same two answers and the same disagreement;
    /// which one a caller wants is the caller's choice, and the front end's own choice is the queue.
    pub fn pressed(&self) -> bool {
        self.held != 0 && self.held_ms == 0
    }

    /// Came up this frame: released, with the release timer still reading 0. The same caveat as
    /// [`ControlState::pressed`] applies, and one more — a pad that vanishes mid-press takes its
    /// release event *and* this with it (see [`Pad::poll`]).
    pub fn released(&self) -> bool {
        self.held == 0 && self.since_release_ms == 0
    }
}

// ---------------------------------------------------------------------------------------------------
// The one-frame event queue
// ---------------------------------------------------------------------------------------------------

/// The event kind a pad control raises. The original's dispatcher knows two others, for device classes
/// this game never builds.
pub const EVENT_CONTROL: u32 = 2;

/// The device id the game's own pads carry. The dispatcher only compares it against the id of the
/// *other* device class it knows, so for a pad it is a value that is never equal to anything.
pub const PAD_DEVICE_ID: i32 = -1;

/// What [`EventQueue::push`] writes into an event's stamp. The dispatcher overwrites it with the
/// engine's time source before handing the event over, within the same update, so it is never
/// observable from outside — it is here because it is what the original writes.
pub const QUEUED_STAMP: u32 = 0x3f;

/// One transition, as the queue holds it: six words in the order the game writes them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Event {
    /// Which control, as an index into [`Control::ALL`].
    pub control: u32,
    /// [`EVENT_CONTROL`] for everything this game raises.
    pub kind: u32,
    /// The raising device's own id: [`PAD_DEVICE_ID`] for a pad.
    pub device: i32,
    /// Which entry of the manager's device list raised it, so the dispatcher can find the device again.
    pub index: u32,
    /// 1 for a press, 0 for a release.
    pub pressed: u32,
    /// Stamped by the dispatcher; see [`QUEUED_STAMP`].
    pub stamp: u32,
}

impl Event {
    /// The control this is about, or `None` if the index is not one of the sixteen.
    pub fn control(&self) -> Option<Control> {
        Control::from_index(self.control as usize)
    }

    /// A press rather than a release.
    pub fn is_press(&self) -> bool {
        self.pressed != 0
    }
}

/// The most events one update can raise: every control of every pad can change at most once in a
/// frame, and a pad control is the only thing in this game that raises one.
///
/// The original's queue has no bounds check at all — it writes where the count says and bumps the
/// count — so this is not the original's array length (which nothing in the image reveals) but a bound
/// the original can never exceed either.
pub const MAX_EVENTS: usize = PADS * CONTROLS;

/// The transitions of one frame, in the order they were raised.
///
/// Cleared at the top of every [`Manager::tick`] and handed to the front end at the bottom of the same
/// tick, so an event is visible for exactly one frame and a non-empty queue *is* an edge frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventQueue {
    pub(crate) count: usize,
    pub(crate) events: [Event; MAX_EVENTS],
}

impl Default for EventQueue {
    fn default() -> EventQueue {
        EventQueue {
            count: 0,
            events: [Event::default(); MAX_EVENTS],
        }
    }
}

impl EventQueue {
    /// Empty.
    pub fn new() -> EventQueue {
        EventQueue::default()
    }

    /// This frame's events, in the order they were raised.
    pub fn events(&self) -> &[Event] {
        &self.events[..self.count]
    }

    /// How many transitions this frame raised.
    pub fn len(&self) -> usize {
        self.count
    }

    /// No transition this frame.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Drop everything, as the update does before polling anything.
    ///
    /// Only the count is reset, exactly as in the original: the stale events past the count stay in the
    /// array, where nothing reads them.
    pub fn clear(&mut self) {
        self.count = 0;
    }

    /// Append one transition.
    pub fn push(&mut self, event: Event) {
        if self.count < MAX_EVENTS {
            self.events[self.count] = event;
            self.count += 1;
        }
    }

    /// The press for this control, if this frame raised one.
    pub fn press(&self, control: Control) -> Option<&Event> {
        self.events()
            .iter()
            .find(|e| e.is_press() && e.control as usize == control.index())
    }

    /// The release for this control, if this frame raised one.
    pub fn release(&self, control: Control) -> Option<&Event> {
        self.events()
            .iter()
            .find(|e| !e.is_press() && e.control as usize == control.index())
    }
}

// ---------------------------------------------------------------------------------------------------
// One pad
// ---------------------------------------------------------------------------------------------------

/// One pad: the sixteen controls, and the two words an event quotes from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pad {
    /// The id its events carry. The game's pads all carry [`PAD_DEVICE_ID`].
    pub device: i32,
    /// Its index in the manager's device list, which its events also carry.
    pub index: u32,
    /// Whether the pad library says there is a pad here. The negotiation behind this — connect,
    /// analogue mode, pressure mode, the actuators — is the platform layer's, not the simulation's.
    pub connected: bool,
    /// The sixteen controls, in [`Control::ALL`] order.
    pub controls: [ControlState; CONTROLS],
    /// Each axis's report byte as a float, exactly as the poll stores it: `127.0` for an axis with no
    /// report byte bound to it, and for every axis of a pad that has gone away. In [`Axis::ALL`] order.
    pub axis_raw: [F; AXES],
    /// What the poll hands each axis's consumers, through [`AxisRange::STICK`] — the one range every
    /// stick consumer in the game carries. The original stores this in each consumer rather than in the
    /// pad, and hands every consumer of an axis the same number, so one per axis is what there is to
    /// keep; see [`AxisRange`].
    pub axis_value: [F; AXES],
    /// The last poll took the bad-read branch, which hands every consumer a centred axis and leaves
    /// [`Pad::axis_raw`] stale. [`Pad::axis_through`] needs it to give a consumer of its own range
    /// what that branch would have handed it. Derived from the poll and not hashed: it is the same
    /// fact as the consumers' values, which [`Pad::axis_value`] already carries into the hash.
    pub axes_centred: bool,
}

impl Default for Pad {
    fn default() -> Pad {
        Pad::new(0)
    }
}

impl Pad {
    /// A pad as the game builds one: every control released, every timer at 0 (the original leaves the
    /// timers uninitialised — see [`ControlState`]), and not connected until the platform layer says
    /// otherwise.
    pub fn new(index: u32) -> Pad {
        Pad {
            device: PAD_DEVICE_ID,
            index,
            connected: false,
            controls: [ControlState::default(); CONTROLS],
            axis_raw: [AXIS_CENTRE_RAW; AXES],
            axis_value: [F::ZERO; AXES],
            axes_centred: false,
        }
    }

    /// One control's whole record.
    pub fn control(&self, control: Control) -> &ControlState {
        &self.controls[control.index()]
    }

    /// Down this frame.
    pub fn held(&self, control: Control) -> bool {
        self.control(control).is_held()
    }

    /// Went down this frame; see [`ControlState::pressed`] for what that means exactly.
    pub fn pressed(&self, control: Control) -> bool {
        self.control(control).pressed()
    }

    /// Came up this frame; see [`ControlState::released`].
    pub fn released(&self, control: Control) -> bool {
        self.control(control).released()
    }

    /// How hard: 1.0 for a digital control, `(pressure + 1) / 256` for an analogue one.
    pub fn strength(&self, control: Control) -> F {
        self.control(control).strength
    }

    /// One axis, after the dead zone and the two segments: −1 to 1, 0 while the stick is inside the
    /// dead zone. This is the number the original hands its consumers, for a consumer with the game's
    /// own [`AxisRange::STICK`] range — the main menu's. The ride's consumers carry
    /// [`AxisRange::PLAYER`] instead, through [`Pad::axis_through`].
    pub fn axis(&self, axis: Axis) -> F {
        self.axis_value[axis.index()]
    }

    /// One axis's raw report byte as the poll stored it, before the inversion and before the range —
    /// 0 to 255 as a float, 127.0 for an axis with nothing bound to it.
    pub fn raw_axis(&self, axis: Axis) -> F {
        self.axis_raw[axis.index()]
    }

    /// One axis as a consumer carrying `range` is handed it: the same byte, inversion and arithmetic
    /// as [`Pad::axis`], through a range of the caller's own.
    ///
    /// The game's consumers do not all carry the same range: the main menu's carry
    /// [`AxisRange::STICK`] (I2), and the player's four in a ride carry [`AxisRange::PLAYER`] (I6).
    /// After a bad read the poll hands every consumer a centred axis, which this repeats.
    pub fn axis_through(&self, axis: Axis, range: &AxisRange) -> F {
        let i = axis.index();
        let out = match (self.axes_centred, i == AXIS_INVERTED) {
            (true, true) => AXIS_CENTRE_RAW_INVERTED,
            (true, false) => AXIS_CENTRE_RAW,
            (false, true) => AXIS_FULL - self.axis_raw[i],
            (false, false) => self.axis_raw[i],
        };
        range.apply(out, AXIS_FULL)
    }

    /// Everything held, back in the game's own bitmask.
    ///
    /// The original never builds this word — it maps the word *to* controls and keeps the controls — so
    /// this is the port's convenience and not a thing to compare against the original's RAM. It is
    /// exact in the direction that matters: a bit is set here exactly when its control is held.
    pub fn held_mask(&self) -> u16 {
        self.mask_where(ControlState::is_held)
    }

    /// Everything that went down this frame, as a bitmask.
    pub fn pressed_mask(&self) -> u16 {
        self.mask_where(ControlState::pressed)
    }

    /// Everything that came up this frame, as a bitmask.
    pub fn released_mask(&self) -> u16 {
        self.mask_where(ControlState::released)
    }

    fn mask_where(&self, f: impl Fn(&ControlState) -> bool) -> u16 {
        let mut m = 0;
        for (i, state) in self.controls.iter().enumerate() {
            if f(state) {
                m |= CONTROL_MAP[i].mask;
            }
        }
        m
    }

    /// Read one pad report into the sixteen controls, raising an event for every transition. The
    /// original's own frame, in the original's order:
    ///
    /// 1. If the pad library says this pad is gone, centre all six axes and clear every control's
    ///    `held` and `strength`, then stop. No events, and the timers are not touched.
    /// 2. Copy this frame's `held` flags aside and clear them all — so a failed read leaves everything
    ///    released.
    /// 3. Read the pad. A non-zero status byte (or a read that fails outright) stops the frame here,
    ///    with every control released, every axis handed out centred, **no events** and the timers
    ///    untouched: a pad unplugged mid-press loses its release event, and its release timer then
    ///    keeps counting from whatever it held before the press. The stored axis bytes are *not*
    ///    touched on this path — only the value the consumers are handed is.
    /// 4. Store all six axis bytes and hand them out ([`Pad::axis`]).
    /// 5. Compute the button word once, then walk the sixteen controls in map order: an analogue
    ///    control is held when its pressure byte is above [`PRESSURE_THRESHOLD`], with strength
    ///    `(pressure + 1) / 256`; any other is held when its bit is in the word, with strength 1.0.
    /// 6. Walk the sixteen again, comparing against the copies: unchanged, the matching timer takes
    ///    another `dt * 1000` and its old value is kept beside it; changed, the *other* timer pair is
    ///    zeroed and an event is queued.
    ///
    /// The two walks are separate passes, which is why events come out in control order rather than in
    /// the order the buttons were actually pressed, and the only reason that is observable is that the
    /// front end consumes them in queue order.
    ///
    /// Returns whether the read was good, which is what the original's update uses to decide whether
    /// the pad is worth asking anything else.
    pub fn poll(&mut self, dt: F, report: &Report, events: &mut EventQueue) -> bool {
        if !self.connected {
            self.read_axes(None);
            for state in &mut self.controls {
                state.strength = F::ZERO;
                state.held = 0;
            }
            return false;
        }

        let mut was_held = [0i32; CONTROLS];
        for (state, was) in self.controls.iter_mut().zip(&mut was_held) {
            *was = state.held;
            state.held = 0;
            state.strength = F::ZERO;
        }

        if report.status() != 0 {
            self.centre_axes();
            return false;
        }

        let buttons = report.buttons();
        self.read_axes(Some(report));
        for (i, binding) in CONTROL_MAP.iter().enumerate() {
            let state = &mut self.controls[i];
            if binding.is_analogue() {
                let pressure = report.pressure(binding.pressure);
                if pressure > PRESSURE_THRESHOLD {
                    // `(pressure + 1)` as an integer, converted and scaled: one convert, one multiply,
                    // in that order.
                    state.strength = F::from(pressure as i32 + 1) * PRESSURE_SCALE;
                    state.held = 1;
                }
            } else if buttons & binding.mask == 0 {
                state.strength = F::ZERO;
            } else {
                state.held = 1;
                state.strength = DIGITAL_STRENGTH;
            }
        }

        let dt_ms = dt * 1000.0;
        let (device, index) = (self.device, self.index);
        for (i, (state, &was)) in self.controls.iter_mut().zip(&was_held).enumerate() {
            if state.held == was {
                if state.held == 1 {
                    state.prev_held_ms = state.held_ms;
                    state.held_ms = add_ms(state.held_ms, dt_ms);
                } else {
                    state.prev_since_release_ms = state.since_release_ms;
                    state.since_release_ms = add_ms(state.since_release_ms, dt_ms);
                }
            } else if state.held == 0 {
                state.prev_since_release_ms = 0;
                state.since_release_ms = 0;
                events.push(Event {
                    control: i as u32,
                    kind: EVENT_CONTROL,
                    device,
                    index,
                    pressed: 0,
                    stamp: QUEUED_STAMP,
                });
            } else {
                state.prev_held_ms = 0;
                state.held_ms = 0;
                events.push(Event {
                    control: i as u32,
                    kind: EVENT_CONTROL,
                    device,
                    index,
                    pressed: 1,
                    stamp: QUEUED_STAMP,
                });
            }
        }
        true
    }

    /// The poll's own axis pass: store each axis's report byte as a float — `127.0` where the map has
    /// nothing bound, and for every axis when the pad is gone (`report` is `None`) — and hand each one
    /// to its consumers, inverting [`AXIS_INVERTED`] on the way out.
    ///
    /// The byte goes to float through the *unsigned* conversion the original uses, which for a value a
    /// report byte can hold is the plain one.
    fn read_axes(&mut self, report: Option<&Report>) {
        self.axes_centred = false;
        for i in 0..AXES {
            self.axis_raw[i] = match report {
                Some(r) if AXIS_MAP[i] != 0 => F::from(r.0[AXIS_MAP[i] as usize] as i32),
                _ => AXIS_CENTRE_RAW,
            };
            let out = if i == AXIS_INVERTED {
                AXIS_FULL - self.axis_raw[i]
            } else {
                self.axis_raw[i]
            };
            self.axis_value[i] = AxisRange::STICK.apply(out, AXIS_FULL);
        }
    }

    /// The axis pass of the branch the original takes when the read fails or the status byte is bad: it
    /// hands the consumers a centred axis — the constants its own code carries, 127.0 and the inverted
    /// axis's 128.0 — and leaves the stored bytes exactly as they were, stale.
    fn centre_axes(&mut self) {
        self.axes_centred = true;
        for i in 0..AXES {
            let raw = if i == AXIS_INVERTED {
                AXIS_CENTRE_RAW_INVERTED
            } else {
                AXIS_CENTRE_RAW
            };
            self.axis_value[i] = AxisRange::STICK.apply(raw, AXIS_FULL);
        }
    }
}

/// `timer + dt * 1000`, as the original does it: the stored integer is read back as an *unsigned*
/// float, one FPU add, and the sum converted back to unsigned — which is why the fraction is lost every
/// frame instead of accumulating, and why a timer wraps through the top of the `u32` range rather than
/// saturating.
///
/// The conversion back is the two-case sequence a compiler emits for an unsigned cast on this hardware:
/// below 2³¹ it is a plain convert, and at or above it the bias is subtracted first and the sign bit
/// put back by hand.
pub fn add_ms(timer: u32, dt_ms: F) -> u32 {
    /// 2³¹ as a float: exact, and the point where a signed convert would saturate.
    const BIAS: f32 = 2_147_483_648.0;

    let sum = F(fpu::from_u32(timer)) + dt_ms;
    if sum < BIAS {
        sum.to_i32() as u32
    } else {
        ((sum - BIAS).to_i32() as u32) | 0x8000_0000
    }
}

// ---------------------------------------------------------------------------------------------------
// The analogue axes: the sticks
// ---------------------------------------------------------------------------------------------------

/// How many axes a pad keeps. Six, not four: the game's pad class has six axis slots and this game
/// binds four of them — the two stick halves of each stick — leaving two with no report byte at all.
pub const AXES: usize = 6;

/// Which report byte each axis takes its value from, `0` meaning nothing is bound to it. The game fills
/// this in when it binds a pad, so it is RAM rather than a table in the image; this is pad 0's, read
/// out of a RAM image, and it agrees with the mapping the game's own single-axis reader has compiled in
/// (0 → 6, 1 → 7, 2 → 4, 3 → 5 — the same four bytes, with the right stick's X inverted the same way).
pub const AXIS_MAP: [u8; AXES] = [6, 7, 4, 0, 0, 5];

/// The one axis the poll hands out **inverted**: it dispatches `255 - raw` for it and `raw` for the
/// other five. The original's other axis reader inverts the same index, so the inversion belongs to the
/// axis and not to this one call site.
pub const AXIS_INVERTED: usize = 2;

/// The divisor the poll passes with every axis: the full range of a report byte, as a float.
pub const AXIS_FULL: F = F(255.0);

/// A centred axis byte as a float. The poll stores this for an axis with no report byte bound, and for
/// every axis of a pad the pad library says is gone.
pub const AXIS_CENTRE_RAW: F = F(127.0);

/// A centred *inverted* axis, which is `255 - 127` and is the constant the game's own code carries for
/// the one branch that centres the axes without storing them (a read that fails or a bad status byte).
pub const AXIS_CENTRE_RAW_INVERTED: F = F(128.0);

/// The six axes, in the order the pad keeps them, named for what this game binds each to.
///
/// The two unbound slots are kept because the poll walks all six every frame — it stores a centred
/// 127.0 for them and hands that to anything registered on them — so dropping them would change the
/// loop rather than tidy it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Axis {
    LeftX = 0,
    LeftY,
    /// The right stick's X, and the one axis the poll inverts.
    RightX,
    /// No report byte: permanently centred.
    Unbound3,
    /// No report byte: permanently centred.
    Unbound4,
    RightY,
}

/// The names, in axis order, so a dump reads as words rather than indices.
const AXIS_NAMES: [&str; AXES] = ["left_x", "left_y", "right_x", "unbound3", "unbound4", "right_y"];

impl Axis {
    /// Every axis, in the pad's own order.
    pub const ALL: [Axis; AXES] = {
        use Axis::*;
        [LeftX, LeftY, RightX, Unbound3, Unbound4, RightY]
    };

    /// Its index in the pad's axis slots.
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The axis at an index, or `None` past the sixth.
    pub const fn from_index(i: usize) -> Option<Axis> {
        if i < AXES {
            Some(Axis::ALL[i])
        } else {
            None
        }
    }

    /// Which report byte it reads, or `None` if nothing is bound to it.
    pub const fn report_byte(self) -> Option<u8> {
        match AXIS_MAP[self as usize] {
            0 => None,
            b => Some(b),
        }
    }

    /// Whether the poll hands this axis out as `255 - raw`.
    pub const fn is_inverted(self) -> bool {
        self as usize == AXIS_INVERTED
    }

    /// The name the project calls it, for a dump or a log line.
    pub const fn name(self) -> &'static str {
        AXIS_NAMES[self as usize]
    }
}

/// How one consumer turns an axis byte into a number it can use: a linear map onto `lo..hi`, a dead
/// zone around the centre, and the two straight segments either side of it.
///
/// In the original these five floats live in the *consumer*, not in the pad: a consumer registers
/// itself on one of the pad's six axes, the poll walks that axis's list every frame and writes the
/// mapped value into each entry (at the consumer's own `+0x24`). Every stick consumer in this game
/// carries the same five numbers — [`AxisRange::STICK`], measured in a RAM image across every
/// registered entry of both pads — so the port keeps the one value they would all have been handed,
/// in [`Pad::axis`], and this type for anything that wants a range of its own.
///
/// There is no clamp anywhere in it, and none is needed: the input is a report byte, so `raw / full` is
/// within `0..=1` and the result is within `lo..=hi` by construction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisRange {
    /// What comes out while the axis is inside the dead zone, and the hinge both segments pass through.
    pub centre: F,
    /// What comes out at `raw == 0`.
    pub lo: F,
    /// What comes out at `raw == full`.
    pub hi: F,
    /// The top of the dead zone, in output units — not in bytes.
    pub dead_hi: F,
    /// The bottom of the dead zone, in output units.
    pub dead_lo: F,
}

impl AxisRange {
    /// What every stick consumer in the game carries: the byte mapped onto −1..1, centred at 0, with a
    /// dead zone of ±0.2 and each segment stretched back out to the full range.
    ///
    /// A fifth of the travel either side of centre is dead, so the first fifth of a stick's movement
    /// does nothing at all and the rest is 1.25× steeper than the raw byte — which is why an
    /// approximated curve here changes every movement in the game rather than just the small ones.
    pub const STICK: AxisRange = AxisRange {
        centre: F(0.0),
        lo: F(-1.0),
        hi: F(1.0),
        dead_hi: F(0.2),
        dead_lo: F(-0.2),
    };

    /// What the player's own four stick consumers carry once a ride is up — left x, left y, right x
    /// and right y, the ones the player record copies into its sticks every tick and hands to the
    /// vehicle it controls: the same −1..1 as [`AxisRange::STICK`] with a dead zone of **±0.3**, so
    /// report bytes 90..=165 read exactly zero and the rest is 1/0.7 steeper than the byte.
    ///
    /// Read out of two gameplay save states' RAM (first_recording on the bike, biplane on the plane),
    /// where these four are the only consumers registered, and proved against the biplane capture's
    /// sticks tick by tick (`the_biplane_reads_the_pad`), and against a bike playback's
    /// (`the_bike_reads_the_players_dead_zone`, I7), where ±0.2 is wrong on every byte in the band
    /// the two disagree on. The ride's hands read this range; I2's ±0.2 is the main menu's.
    pub const PLAYER: AxisRange = AxisRange {
        centre: F(0.0),
        lo: F(-1.0),
        hi: F(1.0),
        dead_hi: F(0.3),
        dead_lo: F(-0.3),
    };

    /// One axis byte through the dead zone and the two segments, in the original's own arithmetic:
    /// one divide, then the linear map, then whichever segment the result lands in.
    ///
    /// `full` is the divisor the caller passes — always 255.0 from the pad poll, and the reason it is
    /// an argument rather than a constant is that it is one in the original too.
    ///
    /// The comparisons are the original's, including which way each boundary falls: at exactly
    /// `dead_hi` it takes the upper segment and at exactly `dead_lo` the lower one, and both of those
    /// come out at `centre` anyway, so the boundaries are only visible in the arithmetic, never in the
    /// result.
    pub fn apply(&self, raw: F, full: F) -> F {
        let v = self.lo + (raw / full) * (self.hi - self.lo);
        if v < self.dead_hi {
            if v <= self.dead_lo {
                (v - self.dead_lo) * ((self.centre - self.lo) / (self.dead_lo - self.lo)) + self.centre
            } else {
                self.centre
            }
        } else {
            (v - self.dead_hi) * ((self.hi - self.centre) / (self.hi - self.dead_hi)) + self.centre
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// The manager: the whole input layer, one call a frame
// ---------------------------------------------------------------------------------------------------

/// The game's input manager: the pads, and the queue their transitions go onto.
///
/// One [`Manager::tick`] a frame, before the simulation, with the same `dt` the simulation is about to
/// get. That is the whole of the original's input handling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Manager {
    /// The pads, in device-list order.
    pub pads: [Pad; PADS],
    /// This frame's transitions.
    pub events: EventQueue,
    /// The engine time the dispatcher stamps this frame's events with. The original takes it from the
    /// manager's time source, and this game's manager has none, so it is 0 — which is what every event
    /// in a capture reads, the capture being sampled after the dispatch.
    pub time_ms: u32,
}

impl Default for Manager {
    fn default() -> Manager {
        Manager::new()
    }
}

impl Manager {
    /// The manager as the game builds it: two pads, neither connected, an empty queue.
    pub fn new() -> Manager {
        Manager {
            pads: [Pad::new(0), Pad::new(1)],
            events: EventQueue::new(),
            time_ms: 0,
        }
    }

    /// The manager with every pad connected — what a port driven by a host pad wants, and what every
    /// test here uses.
    pub fn connected() -> Manager {
        let mut m = Manager::new();
        for pad in &mut m.pads {
            pad.connected = true;
        }
        m
    }

    /// One pad.
    pub fn pad(&self, index: usize) -> &Pad {
        &self.pads[index]
    }

    /// One pad, to connect or disconnect it.
    pub fn pad_mut(&mut self, index: usize) -> &mut Pad {
        &mut self.pads[index]
    }

    /// One frame of input: clear last frame's events, read every pad in device-list order, then stamp
    /// the events the way the dispatcher does on its way to the front end.
    ///
    /// The order of the two loops is the order the queue ends up in, and the queue's order is the order
    /// the front end sees the presses in: pad 0's controls in map order, then pad 1's.
    pub fn tick(&mut self, dt: F, reports: &[Report; PADS]) {
        self.events.clear();
        for (i, pad) in self.pads.iter_mut().enumerate() {
            pad.poll(dt, &reports[i], &mut self.events);
        }
        let stamp = self.time_ms;
        for i in 0..self.events.count {
            self.events.events[i].stamp = stamp;
        }
    }

    /// One frame of input from the game's own bitmask per pad, for a caller that has buttons rather
    /// than pad reports — a replay fixture, a keyboard, a test.
    pub fn tick_buttons(&mut self, dt: F, buttons: &[u16; PADS]) {
        let reports = [
            Report::from_buttons(buttons[0]),
            Report::from_buttons(buttons[1]),
        ];
        self.tick(dt, &reports);
    }
}

// ---------------------------------------------------------------------------------------------------
// The state hash
// ---------------------------------------------------------------------------------------------------
// These walks live beside the structs they describe rather than in `hash.rs` with the simulation's own,
// because what they list is this module's business: a field added below has to be added to its walk in
// the same edit, and that is easier to get right when the two are ten lines apart.

impl State for ControlState {
    fn walk(&self, w: &mut Walk<'_>) {
        w.i32("held", self.held);
        w.f("strength", self.strength);
        w.u32("since_release_ms", self.since_release_ms);
        w.u32("held_ms", self.held_ms);
        w.u32("prev_since_release_ms", self.prev_since_release_ms);
        w.u32("prev_held_ms", self.prev_held_ms);
    }
}

impl State for Event {
    fn walk(&self, w: &mut Walk<'_>) {
        w.u32("control", self.control);
        w.u32("kind", self.kind);
        w.i32("device", self.device);
        w.u32("index", self.index);
        w.u32("pressed", self.pressed);
        w.u32("stamp", self.stamp);
    }
}

impl State for EventQueue {
    /// Only the events this frame raised: the slots past the count are stale in the original too, and
    /// hashing them would make two identical frames differ on their history.
    fn walk(&self, w: &mut Walk<'_>) {
        w.list("events", self.events());
    }
}

impl State for Pad {
    fn walk(&self, w: &mut Walk<'_>) {
        w.i32("device", self.device);
        w.u32("index", self.index);
        w.bool("connected", self.connected);
        w.list("controls", &self.controls);
        w.group("axis_raw", |w| {
            for (axis, raw) in Axis::ALL.iter().zip(&self.axis_raw) {
                w.f(axis.name(), *raw);
            }
        });
        w.group("axis", |w| {
            for (axis, v) in Axis::ALL.iter().zip(&self.axis_value) {
                w.f(axis.name(), *v);
            }
        });
    }
}

impl State for Manager {
    fn walk(&self, w: &mut Walk<'_>) {
        w.list("pads", &self.pads);
        w.field("events", &self.events);
        w.u32("time_ms", self.time_ms);
    }
}

impl Manager {
    /// The input layer's whole state in 64 bits, the same on every platform.
    pub fn hash(&self) -> u64 {
        hash::of(self)
    }
}

// ---------------------------------------------------------------------------------------------------
// The biplane's eight commands
// ---------------------------------------------------------------------------------------------------

/// The eight game commands the biplane reads, numbered as the binding table numbers them.
///
/// The original never asks the pad for a button: the plane's input step looks each command up in the
/// front end's binding table (pad, command → control) and reads that control's strength out of the
/// player record's copy of the sixteen control records. The numbers are the table's; which control
/// each is bound to is [`PlaneCommand::control`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum PlaneCommand {
    BankLeft = 0x00,
    BankRight = 0x01,
    Dive = 0x02,
    Climb = 0x03,
    ThrottleUp = 0x0e,
    ThrottleDown = 0x0f,
    RudderLeft = 0x14,
    RudderRight = 0x15,
}

impl PlaneCommand {
    /// The eight, in the order the help screen lists them and the overlay draws them.
    pub const ALL: [PlaneCommand; 8] = {
        use PlaneCommand::*;
        [ThrottleUp, ThrottleDown, Climb, Dive, BankLeft, BankRight, RudderLeft, RudderRight]
    };

    /// The control the shipped binding table puts this command on, for pad 0 and pad 1 alike (read
    /// out of two save states' RAM, R8 and C7). The table is the front end's and a remapped profile
    /// would differ; the port carries the shipped one, as [`CONTROL_MAP`] carries the shipped pad.
    pub const fn control(self) -> Control {
        match self {
            PlaneCommand::BankLeft => Control::Left,
            PlaneCommand::BankRight => Control::Right,
            PlaneCommand::Dive => Control::Up,
            PlaneCommand::Climb => Control::Down,
            PlaneCommand::ThrottleUp => Control::Cross,
            PlaneCommand::ThrottleDown => Control::Square,
            PlaneCommand::RudderLeft => Control::L1,
            PlaneCommand::RudderRight => Control::R1,
        }
    }

    /// The name the project calls it, as the freeride's info screen does (`STR_FREE_INFO_PLANE_*`).
    pub const fn name(self) -> &'static str {
        match self {
            PlaneCommand::BankLeft => "bank left",
            PlaneCommand::BankRight => "bank right",
            PlaneCommand::Dive => "dive",
            PlaneCommand::Climb => "climb",
            PlaneCommand::ThrottleUp => "throttle up",
            PlaneCommand::ThrottleDown => "throttle down",
            PlaneCommand::RudderLeft => "rudder left",
            PlaneCommand::RudderRight => "rudder right",
        }
    }
}

/// What the biplane's input step makes of the pad: the five numbers it writes into the plane, which
/// the joystick step then rate-limits into the throttle and the three control surfaces.
///
/// | field | from |
/// |---|---|
/// | `throttle_up` | [`PlaneCommand::ThrottleUp`]'s strength, as it is — so a pressure, not a switch |
/// | `throttle_down` | [`PlaneCommand::ThrottleDown`]'s strength |
/// | `elevator` | `-sign(y)·|y|³` of the left stick's y; +1 while [`PlaneCommand::Dive`] is held, else −1 while [`PlaneCommand::Climb`] is |
/// | `aileron` | `-sign(x)·|x|³` of the left stick's x; +1 while [`PlaneCommand::BankLeft`] is held, else −1 while [`PlaneCommand::BankRight`] is |
/// | `rudder` | −1 while [`PlaneCommand::RudderLeft`] is held, else +1 while [`PlaneCommand::RudderRight`] is, else 0 |
///
/// So the stick is analogue and cubed, the d-pad is digital and overrides it, the rudder is digital
/// and left wins a tie, and the two throttle commands are analogue (their pressure bytes). "Held" is
/// the original's own test, a strength that is not 0.0. Stick forward (y < 0) is +elevator, a dive;
/// stick left (x < 0) is +aileron, a bank to the left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlaneInput {
    pub throttle_up: F,
    pub throttle_down: F,
    pub elevator: F,
    pub aileron: F,
    pub rudder: F,
}

impl PlaneInput {
    /// The input step, given the player's two stick values and a command's strength.
    ///
    /// In the original order and arithmetic: the two throttle commands, the stick's cubes (`y·(y·y)`,
    /// its absolute value, times the negated sign as a converted integer — so a centred stick is −0.0),
    /// the rudder, then the d-pad over the aileron and the elevator.
    pub fn from_sticks(stick: (F, F), value: impl Fn(PlaneCommand) -> F) -> PlaneInput {
        let held = |c: PlaneCommand| value(c) != F::ZERO;
        let cube = |v: F| {
            let sign = if v < F::ZERO { -1 } else { 1 };
            F::from(-sign) * (v * (v * v)).abs()
        };
        let (x, y) = stick;
        let mut out = PlaneInput {
            throttle_up: value(PlaneCommand::ThrottleUp),
            throttle_down: value(PlaneCommand::ThrottleDown),
            elevator: cube(y),
            aileron: cube(x),
            rudder: F::ZERO,
        };
        if held(PlaneCommand::RudderLeft) {
            out.rudder = -F::ONE;
        } else if held(PlaneCommand::RudderRight) {
            out.rudder = F::ONE;
        }
        if held(PlaneCommand::BankLeft) {
            out.aileron = F::ONE;
        } else if held(PlaneCommand::BankRight) {
            out.aileron = -F::ONE;
        }
        if held(PlaneCommand::Dive) {
            out.elevator = F::ONE;
        } else if held(PlaneCommand::Climb) {
            out.elevator = -F::ONE;
        }
        out
    }

    /// The input step on one pad through the shipped bindings: the player's sticks are the left
    /// stick through [`AxisRange::PLAYER`], and a command's value is its first binding's strength in
    /// the shipped table ([`crate::bindings::Table::value`], P73).
    ///
    /// The player record also merges a second pad bound to the same player (its sticks where they
    /// are further out, its controls where the first's are 0); a player with one pad, which is every
    /// player the port has, is this.
    pub fn from_pad(pad: &Pad) -> PlaneInput {
        let stick =
            (pad.axis_through(Axis::LeftX, &AxisRange::PLAYER), pad.axis_through(Axis::LeftY, &AxisRange::PLAYER));
        let table = &crate::bindings::Table::SHIPPED;
        PlaneInput::from_sticks(stick, |c| table.value(pad, crate::bindings::Command(c as u8)))
    }
}
