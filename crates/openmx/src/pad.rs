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
//! RAM — [`CONTROL_MAP`]), so on the disc the throttle was never a switch: a DualShock 2's face
//! buttons travel, and part throttle out of a corner is a fraction of that travel. Hardly any pad
//! sold since reports a face button as anything but one bit, so this module gives the rider three
//! ways to it, in the order of how much of it is the device's own: a per-button pressure where the
//! driver publishes one ([`Wiring::pressure`]), gas on a trigger axis where it does not
//! ([`Throttle::Trigger`]), and a ramp under the keyboard's throttle key ([`KeyRamp`]) for a host
//! with no travel anywhere.
//!
//! The report is built the way a DualShock 2 would have reported it, not the way the game sees it:
//! raw stick bytes, the button word active-low, and a pressure byte per analogue control. Whether an
//! axis is then inverted or dead-zoned is the port's business.

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

/// Which control each joystick button index is, in the order Linux reports a gamepad's buttons.
///
/// The kernel orders them by keycode, so a pad with no `BTN_SELECT` — the kit's own virtual pad, for
/// one — slides start down into index 6. Both 6 and 7 are therefore Start, which costs nothing: Select
/// does nothing in this shell, and the one control the project has never been able to measure live is
/// Select anyway (see I1).
const PAD_BUTTONS: [Option<Control>; BUTTONS] = [
    Some(Control::Cross),    // south
    Some(Control::Circle),   // east
    Some(Control::Square),   // west
    Some(Control::Triangle), // north
    Some(Control::L1),       // left shoulder
    Some(Control::R1),       // right shoulder
    Some(Control::Start),    // select on a pad that has one, start on a pad that does not
    Some(Control::Start),
    None, // the guide button, which is the window manager's business and not the game's
    Some(Control::L3),
    Some(Control::R3),
    None,
];

/// The joystick axes the shell reads: the two sticks, the two triggers and the d-pad hat.
const AXIS_LEFT_X: usize = 0;
const AXIS_LEFT_Y: usize = 1;
const AXIS_L2: usize = 2;
const AXIS_RIGHT_X: usize = 3;
const AXIS_RIGHT_Y: usize = 4;
const AXIS_R2: usize = 5;
const AXIS_HAT_X: usize = 6;
const AXIS_HAT_Y: usize = 7;

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
    /// A trigger's travel, which gives up that trigger's own control for the session — R2 is the
    /// front brake and L2 the clutch in this shell's binding, so this is a trade and not a free win.
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
}

impl Default for Wiring {
    fn default() -> Wiring {
        Wiring { throttle: Throttle::default(), pressure: [None; AXES] }
    }
}

impl Wiring {
    /// Read a `--pad-pressure` spec: `none`, `ds2@AXIS` — twelve consecutive axes from `AXIS` in
    /// [`DS2_PRESSURE_ORDER`] — or a comma-separated list of `AXIS:CONTROL` for a device that lays
    /// them out some other way.
    ///
    /// A spec rather than a built-in table per pad because no device the project has ever had in
    /// front of it reports per-button pressure: the project cannot say which axis a given driver
    /// puts Cross's pressure on, and a wrong default would read a stick as a throttle. With the
    /// overlay on (`--hud`) the right number is a minute's work — press the button, watch which
    /// control's strength follows it.
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

/// One seat's pad and keyboard, as one report a frame: a joystick device, the keyboard, or both
/// folded together (the first seat's pad and the keyboard, on a machine nobody has set seats up on).
/// [`Seats`] owns one of these per pad that is plugged in and one for the keyboard.
pub struct Pads {
    /// The joystick device, if one opened.
    device: Option<std::fs::File>,
    /// Its name, for the line the shell prints when it joins.
    pub name: Option<String>,
    /// The device stopped answering — unplugged, or the read failed — and was closed.
    gone: bool,
    /// The last value of each axis, kept because a joystick event only mentions what changed.
    axis: [i16; AXES],
    /// Which pad buttons are down.
    button: [bool; BUTTONS],
    /// The keyboard's stick axes, in [`KEY_STICKS`]' order, each part way to its edge.
    key_stick: [KeyAxis; KEY_STICKS.len()],
    /// The keyboard's throttle, part way to full.
    key_gas: KeyRamp,
    /// Where the throttle comes from and which axes carry a button's pressure.
    wiring: Wiring,
    /// Which key and which pad button is which of the game's controls (`controls.txt`).
    bindings: Bindings,
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
        for (slot, control) in axis.iter_mut().zip(wiring.pressure) {
            if control.is_some() {
                *slot = TRIGGER_REST;
            }
        }
        Pads {
            device: None,
            name: None,
            gone: false,
            axis,
            button: [false; BUTTONS],
            key_stick: [KeyAxis::CENTRED; KEY_STICKS.len()],
            key_gas: KeyRamp::SHUT,
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
        let mut buttons = 0u16;

        // The pad's buttons and the keyboard's keys, as one set of the game's own controls.
        buttons |= self.bindings.buttons(&self.button, held);
        buttons |= hat(self.axis[AXIS_HAT_X], Control::Left, Control::Right);
        buttons |= hat(self.axis[AXIS_HAT_Y], Control::Up, Control::Down);

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
        // The keyboard moves the right stick too: `IJKL`, ramped toward its edge for as long as the
        // key is held and let back to centre when it is not ([`KeyAxis`]). The arrows are the d-pad,
        // in [`KEYBOARD`].
        for (i, (axis, (slot, _, _))) in self.key_stick.iter_mut().zip(KEY_STICKS).enumerate() {
            let (low, high) = self.bindings.stick_keys(i, held);
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
            let pressure = trigger_pressure(self.axis[axis]);
            if pressure > PRESSURE_THRESHOLD {
                report.0[binding.pressure as usize] = pressure;
            }
        }

        // The analogue triggers, which are a pressure and not a bit: a pad's L2 and R2 travel, and
        // the game reads those two controls from their pressure byte alone. One of them is the
        // throttle instead when the session asked for that, and then it is not itself at all — a
        // front brake that came on with the gas would be worse than no part throttle.
        for (axis, control) in [(AXIS_L2, Control::L2), (AXIS_R2, Control::R2)] {
            let pressure = trigger_pressure(self.axis[axis]);
            let control = match self.wiring.throttle {
                Throttle::Trigger(wired) if wired == control => Control::Cross,
                _ => control,
            };
            if pressure > 0 {
                press(&mut report, control, pressure);
            }
        }

        // And the keyboard's throttle, which is the one control a key cannot do justice to: ramped
        // ([`KeyRamp`]), so a stab is part throttle and a held key is still pinned.
        if let Some(pressure) = self.key_gas.step(seconds, self.bindings.throttle_key(held)) {
            press(&mut report, Control::Cross, pressure);
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

    /// Read every joystick event waiting, updating the axis and button state.
    fn drain(&mut self) {
        use std::io::Read;
        let Some(device) = self.device.as_mut() else {
            return;
        };
        let mut event = [0u8; EVENT_BYTES];
        loop {
            match device.read(&mut event) {
                Ok(EVENT_BYTES) => {
                    let value = i16::from_le_bytes([event[4], event[5]]);
                    let kind = event[6] & !EVENT_INIT;
                    let number = event[7] as usize;
                    match kind {
                        EVENT_AXIS if number < AXES => self.axis[number] = value,
                        EVENT_BUTTON if number < BUTTONS => self.button[number] = value != 0,
                        _ => {}
                    }
                }
                // A short read cannot happen on this device. End of file, or any error but "nothing
                // waiting", is a device that went away — the kernel answers ENODEV once a pad is
                // unplugged — and it is closed and said, so that [`Seats`] can pause its player and
                // hold the seat for it, rather than read a dead file for ever.
                Ok(0) => break,
                Ok(_) => return,
                Err(e) if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
                {
                    return;
                }
                Err(_) => break,
            }
        }
        self.device = None;
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
        }
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
}

impl<P> Default for PadSlots<P> {
    fn default() -> PadSlots<P> {
        PadSlots { held: std::array::from_fn(|_| None), seats: None, last: std::array::from_fn(|_| None) }
    }
}

impl<P: Clone + PartialEq> PadSlots<P> {
    /// Seats set up by `--pads`.
    pub fn with_seats(seats: [Option<Seat>; SEATS]) -> PadSlots<P> {
        PadSlots { seats: Some(seats), ..PadSlots::default() }
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
        let wants = |i: usize| {
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
    pub fn keys(&self) -> Option<usize> {
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

/// The keyboard's right-stick keys, as `controls.txt` names them, in [`KEY_STICKS`]' order: each
/// axis' low key, then its high key.
const STICK_KEYS: [&str; 4] = ["stick_left", "stick_right", "stick_up", "stick_down"];

/// How many lines `controls.txt` has: the sixteen controls, then the four stick keys.
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
/// are not. `cross`'s keys are the keyboard's throttle ramp ([`KeyRamp`]) rather than a bit.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Bindings([Vec<Input>; BINDINGS]);

impl Default for Bindings {
    /// [`PAD_BUTTONS`], [`KEYBOARD`], `X`/space on the throttle and `IJKL` on the right stick: the
    /// shell's binding before there was a file.
    fn default() -> Bindings {
        let mut b: [Vec<Input>; BINDINGS] = std::array::from_fn(|_| Vec::new());
        for (code, control) in KEYBOARD {
            b[control as usize].push(Input::Key(code));
        }
        b[Control::Cross as usize].extend([Input::Key(key::X), Input::Key(key::SPACE)]);
        for (i, control) in PAD_BUTTONS.iter().enumerate() {
            if let Some(control) = control {
                b[*control as usize].push(Input::Button(i));
            }
        }
        for (i, (_, low, high)) in KEY_STICKS.iter().enumerate() {
            b[16 + 2 * i].push(Input::Key(*low));
            b[16 + 2 * i + 1].push(Input::Key(*high));
        }
        Bindings(b)
    }
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
            if !set.is_empty() {
                b.0[i] = set;
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
             # Pad0 to Pad11). cross's keys are the throttle, ramped; stick_* are the right stick's keys.\n\
             # The sticks, the triggers (l2, r2) and the d-pad hat are axes and always do what they do;\n\
             # Escape quits, H the overlay and M the machine swap.\n",
        );
        for (i, set) in self.0.iter().enumerate() {
            let names: Vec<_> = set.iter().map(|x| x.name()).collect();
            s += &format!("{} = {}\n", Bindings::line_name(i), names.join(" "));
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

    /// The game's controls held on the pad's buttons and the keyboard's keys, as its bitmask. Cross
    /// on a key is not here: that is the throttle ramp's ([`Bindings::throttle_key`]).
    fn buttons(&self, button: &[bool; BUTTONS], held: &dyn Fn(u8) -> bool) -> u16 {
        let mut mask = 0;
        for (control, set) in Control::ALL.iter().zip(&self.0) {
            let down = set.iter().any(|x| match *x {
                Input::Button(i) => button[i],
                Input::Key(code) => *control != Control::Cross && held(code),
            });
            if down {
                mask |= control.mask();
            }
        }
        mask
    }

    /// Whether a throttle key — a key on `cross` — is down.
    fn throttle_key(&self, held: &dyn Fn(u8) -> bool) -> bool {
        self.0[Control::Cross as usize].iter().any(|x| matches!(x, Input::Key(c) if held(*c)))
    }

    /// The keyboard stick axis `i`'s two keys, low and high: whether any key on each is down.
    fn stick_keys(&self, i: usize, held: &dyn Fn(u8) -> bool) -> (bool, bool) {
        let any = |line: usize| self.0[line].iter().any(|x| matches!(x, Input::Key(c) if held(*c)));
        (any(16 + 2 * i), any(16 + 2 * i + 1))
    }

    /// Whether any key bound to `control` is down: the keyboard's own Start, for [`Seats`]' take-over.
    fn key_on(&self, control: Control, held: &dyn Fn(u8) -> bool) -> bool {
        self.0[control as usize].iter().any(|x| matches!(x, Input::Key(c) if held(*c)))
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
    /// What changed, for the shell to print: pads joining, going and coming back.
    said: Vec<String>,
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
            said: Vec::new(),
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
        for (_, path) in found {
            if self.joined.iter().any(|j| j.path == path) {
                continue;
            }
            let node = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            let info = DeviceInfo::read(&self.sys, node);
            if !readable(&info) {
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

    /// The pad holding each seat now, by name, for the startup lines and the overlay.
    pub fn name(&self, seat: usize) -> Option<&str> {
        let id = self.slots.held[seat].as_ref()?;
        self.joined.iter().find(|j| &j.id == id)?.pads.name.as_deref()
    }

    /// What changed since the last time this was asked: one line per pad joining, going or back.
    pub fn said(&mut self) -> Vec<String> {
        std::mem::take(&mut self.said)
    }

    /// This frame: read every pad, notice the ones that went and the ones that came, and build each
    /// seat's report. `held` is the keyboard.
    pub fn frame(&mut self, held: &dyn Fn(u8) -> bool) -> [SeatInput; SEATS] {
        for j in &mut self.joined {
            j.pads.drain();
        }
        self.joined.retain(|j| !j.pads.gone);
        if self.since_scan.is_none_or(|t| t.elapsed().as_secs_f32() >= RESCAN_SECONDS) {
            self.scan();
        }
        let present: Vec<String> = self.joined.iter().map(|j| j.id.clone()).collect();
        self.slots.update(&present);

        // The keyboard can take a waiting seat over: Start on its keys lets the seat's pad go and
        // rides on, as opentennis's keyboard plays on for a 1P whose pad is out.
        let keyboard = self.slots.keys();
        let start = self.bindings.key_on(Control::Start, held);
        if let Some(i) = keyboard
            && start
            && !self.start_key
            && self.slots.waiting(i)
        {
            self.slots.forget(i);
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
        let mut out = [SeatInput::Empty; SEATS];
        for (i, seat) in out.iter_mut().enumerate() {
            let keys: &dyn Fn(u8) -> bool = if keyboard == Some(i) { held } else { &no_keys };
            *seat = match &self.slots.held[i] {
                Some(id) => {
                    let j = self.joined.iter_mut().find(|j| &j.id == id).expect("a held pad is joined");
                    SeatInput::Playing(j.pads.report(keys))
                }
                None if self.slots.waiting(i) => SeatInput::Waiting,
                None if keyboard == Some(i) => SeatInput::Playing(self.keys.report(held)),
                None => SeatInput::Empty,
            };
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
mod tests {
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

    /// Nobody's hands on anything, for a test that only cares about one control.
    fn nothing(_: u8) -> bool {
        false
    }

    /// What the gas is after one frame of `report`, through the port's own poll and binding.
    fn gas_of(report: Report) -> F {
        let mut manager = Manager::connected();
        manager.tick(F(FRAME), &[report; PADS]);
        Controls::from_pad(manager.pad(0)).gas
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
        pads.axis[14] = 0;
        let half = gas_of(pads.build(FRAME, &nothing));
        assert_eq!(half, F((trigger_pressure(0) as f32 + 1.0) / 256.0));
        assert!(half > F(0.3) && half < F(0.7), "half a button's travel reads {half:?}");

        // A touch the dead zone swallows is still the press the bit says it is.
        pads.axis[14] = TRIGGER_REST;
        assert_eq!(gas_of(pads.build(FRAME, &nothing)), F::ONE);

        // And the button up is no throttle, however hard the axis is leaning.
        pads.button[0] = false;
        pads.axis[14] = i16::MAX;
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

        // Half way: part throttle, and the front brake it would otherwise have been is not applied.
        pads.axis[AXIS_R2] = 0;
        let c = controls(&mut pads);
        assert_eq!(c.gas, F((trigger_pressure(0) as f32 + 1.0) / 256.0));
        assert!(c.gas > F(0.3) && c.gas < F(0.7), "half a trigger reads {:?}", c.gas);
        assert_eq!(c.front_brake, F::ZERO, "the throttle's trigger is not the brake as well");

        // Pulled: full throttle. The same trigger on a pad wired the ordinary way is the front brake
        // and no throttle at all, which is the one line of difference between the two sessions.
        pads.axis[AXIS_R2] = i16::MAX;
        assert_eq!(controls(&mut pads).gas, F::ONE);
        let mut plain = Pads::unplugged(Wiring::default());
        plain.axis[AXIS_R2] = i16::MAX;
        let c = controls(&mut plain);
        assert_eq!((c.gas, c.front_brake), (F::ZERO, F::ONE));
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
        assert_eq!(b.0[Control::Square as usize], [Input::Key(56), Input::Button(3), Input::Button(11)]);
        assert_eq!(b.0[Control::Circle as usize], d.0[Control::Circle as usize], "nothing understood");
        assert_eq!(errors.len(), 2);
        assert_eq!(Bindings::parse(&b.text()).0, b);
        // The default is the shell's binding from before there was a file.
        assert_eq!(d.0[Control::Cross as usize], [Input::Key(key::X), Input::Key(key::SPACE), Input::Button(0)]);
        assert_eq!(d.0[Control::Start as usize], [Input::Key(key::RETURN), Input::Button(6), Input::Button(7)]);
        assert_eq!(d.0[16..20], [[Input::Key(key::J)], [Input::Key(key::L)], [Input::Key(key::I)], [Input::Key(key::K)]]);
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
        assert_eq!((b.0[Control::Square as usize].as_slice(), said.len()), (&[Input::Key(56)][..], 0));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// An empty directory of this test's own.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("openmx-pad-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A stand-in for a `/dev/input/jsN` node: a fifo the test writes joystick events into and
    /// closes to unplug it — a closed fifo reads end-of-file, as an unplugged pad reads ENODEV.
    struct FakePad {
        path: std::path::PathBuf,
        /// A reader of the test's own, so that the writer can open before [`Seats`] does.
        _hold: std::fs::File,
        writer: std::fs::File,
    }

    /// Held by every test that plugs a [`FakePad`]. A fifo only reads end-of-file once every writer
    /// is closed, and `mkfifo`'s child holds a copy of every open file between its fork and its exec:
    /// one test spawning it while another unplugs would keep that one's pad in for a moment.
    static FIFOS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    impl FakePad {
        fn plug(dir: &std::path::Path, node: &str) -> FakePad {
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
            FakePad { path, _hold: hold, writer }
        }

        fn button(&mut self, number: u8, down: bool) {
            use std::io::Write;
            let event = [0, 0, 0, 0, down as u8, 0, EVENT_BUTTON, number];
            self.writer.write_all(&event).unwrap();
        }

        /// Pull it out: the node goes, and the device reads end-of-file.
        fn unplug(self) {
            std::fs::remove_file(&self.path).unwrap();
        }
    }

    /// A sysfs entry for a node, as the kernel would write one for a USB pad.
    fn sys_entry(sys: &std::path::Path, node: &str, name: &str, phys: &str) {
        let dir = sys.join(node).join("device");
        std::fs::create_dir_all(dir.join("id")).unwrap();
        std::fs::write(dir.join("name"), format!("{name}\n")).unwrap();
        std::fs::write(dir.join("id/vendor"), "045e\n").unwrap();
        std::fs::write(dir.join("id/product"), "028e\n").unwrap();
        std::fs::write(dir.join("uniq"), "\n").unwrap();
        std::fs::write(dir.join("phys"), format!("{phys}\n")).unwrap();
    }

    /// Ride a frame now, without waiting for the rescan's half second.
    fn frame(seats: &mut Seats, held: &dyn Fn(u8) -> bool) -> [SeatInput; SEATS] {
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
        let now = frame(&mut seats, &|code| code == key::Z);
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
        let now = frame(&mut seats, &|code| code == key::Z);
        assert!(down(playing(now[0]), Control::Square));
        assert_eq!(now[1], SeatInput::Empty);
        assert_eq!(seats.said(), ["1P: the keyboard"]);

        // --pads 0,k: a pad is 1P and the keyboard 2P, each with only its own hands.
        let pads = Seat::parse_list("0,k").unwrap();
        let mut a = FakePad::plug(&dev, "js0");
        let mut seats =
            Seats::in_dir(&dev, &sys, None, Some(pads), Wiring::default(), Bindings::default());
        a.button(0, true);
        let now = frame(&mut seats, &|code| code == key::Z);
        assert!(down(playing(now[0]), Control::Cross) && !down(playing(now[0]), Control::Square));
        assert!(down(playing(now[1]), Control::Square) && !down(playing(now[1]), Control::Cross));
        assert_eq!(now[2], SeatInput::Empty);
        // 1P's pad out: 1P waits, and the keyboard stays 2P — it has a seat of its own.
        a.unplug();
        let now = frame(&mut seats, &|code| code == key::Z);
        assert_eq!(now[0], SeatInput::Waiting);
        assert!(down(playing(now[1]), Control::Square));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
