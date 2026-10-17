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
//! The report is built the way a DualShock 2 would have reported it, not the way the game sees it:
//! raw stick bytes, the button word active-low, and a pressure byte per analogue control. Whether an
//! axis is then inverted or dead-zoned is the port's business.

use game_sim::input::{AXIS_CENTRE, CONTROL_MAP, Control, Report};

use crate::window::{Window, key};

/// Which `/dev/input/js*` a pad is read from, when the shell is not told.
const PAD_DEVICES: [&str; 4] = [
    "/dev/input/js0",
    "/dev/input/js1",
    "/dev/input/js2",
    "/dev/input/js3",
];

/// `O_NONBLOCK`, so a frame with no pad event does not wait for one. The value is Linux's, and this
/// is the one place the shell needs a number from a header rather than from std.
const O_NONBLOCK: i32 = 0o4000;

/// One joystick event: a millisecond stamp, a value, a type and which control it is.
const EVENT_BYTES: usize = 8;
/// The event types, with the "this is the initial state" bit masked off.
const EVENT_BUTTON: u8 = 0x01;
const EVENT_AXIS: u8 = 0x02;
const EVENT_INIT: u8 = 0x80;

/// How many of each the shell tracks. A pad with more says nothing the shell listens to.
const AXES: usize = 8;
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

/// How long a held direction key takes to push its stick all the way over, and how long a stick
/// takes to come back to centre once the keys are let go, in seconds.
///
/// A key is a switch and a stick is not, so these two numbers are the keyboard's whole analogue
/// feel, and there is no original behaviour to match: the disc's input is a pad. What they are
/// chosen against is the one measured number in the area, I2's stick curve: a fifth of the travel either side of centre is
/// dead, so the first ~35 ms of a held key is the stick crossing the dead zone and the lean, the
/// steer and the preload grow from nothing after it rather than slamming to full on frame one.
/// Coming back is quicker than going out, because a rider who lets go of a key has already decided.
const KEY_TO_EDGE: f32 = 0.18;
const KEY_TO_CENTRE: f32 = 0.10;

/// The longest step a keyboard stick will take in one frame, in seconds. A frame that stalled — the
/// first one after a level load, or a window dragged across a workspace — would otherwise hand the
/// ramp a tenth of a second of travel at once, which is the slam this is here to stop.
const KEY_STEP_MAX: f32 = 1.0 / 15.0;

/// Which two keys drive which stick axis, and where that axis goes in the report. The low key is the
/// one at the low end of the byte, which is up or left — the hardware's own sense (see [`stick_byte`]).
const KEY_STICKS: [(usize, u8, u8); 4] = [
    (REPORT_LEFT_X, key::LEFT, key::RIGHT),
    (REPORT_LEFT_Y, key::UP, key::DOWN),
    (REPORT_RIGHT_X, key::J, key::L),
    (REPORT_RIGHT_Y, key::I, key::K),
];

/// The host's pad and keyboard, as one report a frame.
pub struct Pads {
    /// The joystick device, if one opened.
    device: Option<std::fs::File>,
    /// Its name, for the line the shell prints at startup.
    pub name: Option<String>,
    /// The last value of each axis, kept because a joystick event only mentions what changed.
    axis: [i16; AXES],
    /// Which pad buttons are down.
    button: [bool; BUTTONS],
    /// The four keyboard stick axes, in [`KEY_STICKS`]' order, each part way to its edge.
    key_stick: [KeyAxis; KEY_STICKS.len()],
    /// When the last report was built, so the ramps travel at a rate in seconds rather than one a
    /// frame: the shell paces itself to 60 Hz but a heavy frame overruns, and a keyboard that leans
    /// further when the picture is cheap would be its own bug.
    clock: Option<std::time::Instant>,
}

impl Pads {
    /// Open a pad: the one named, or the first of `/dev/input/js*` that opens.
    ///
    /// No pad is not an error — the keyboard is a pad in this shell — so this always returns a
    /// `Pads`, with `name` saying whether a device was found.
    pub fn open(named: Option<&str>) -> Pads {
        let mut pads = Pads {
            device: None,
            name: None,
            axis: [0; AXES],
            button: [false; BUTTONS],
            key_stick: [KeyAxis::CENTRED; KEY_STICKS.len()],
            clock: None,
        };
        let candidates: Vec<String> = match named {
            Some(path) => vec![path.to_string()],
            None => PAD_DEVICES.iter().map(|p| p.to_string()).collect(),
        };
        for path in candidates {
            use std::os::unix::fs::OpenOptionsExt;
            match std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(O_NONBLOCK)
                .open(&path)
            {
                Ok(file) => {
                    pads.name = Some(match device_name(&path) {
                        Some(name) => format!("{name} ({path})"),
                        None => path.clone(),
                    });
                    pads.device = Some(file);
                    break;
                }
                Err(e) if named.is_some() => {
                    eprintln!("openmx: {path}: {e}");
                    break;
                }
                Err(_) => {}
            }
        }
        pads
    }

    /// Read whatever the pad has said since the last frame, then build this frame's report.
    ///
    /// The joystick interface sends the whole current state as "initial" events when the device is
    /// opened, so the first frame after opening already knows where the sticks are.
    pub fn report(&mut self, window: &Window) -> Report {
        self.drain();
        let mut report = Report::idle();
        let mut buttons = 0u16;

        // The pad's buttons and the keyboard's keys, as one set of the game's own controls.
        for (i, control) in PAD_BUTTONS.iter().enumerate() {
            if let (Some(control), true) = (control, self.button[i]) {
                buttons |= control.mask();
            }
        }
        buttons |= hat(self.axis[AXIS_HAT_X], Control::Left, Control::Right);
        buttons |= hat(self.axis[AXIS_HAT_Y], Control::Up, Control::Down);
        buttons |= keyboard(window);

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
        // The keyboard moves both sticks too, since half of riding is on them: the arrows are the
        // left stick and `IJKL` the right one, each ramped toward its edge for as long as the key is
        // held and let back to centre when it is not ([`KeyAxis`]).
        let seconds = self.since_the_last_report();
        for (axis, (slot, low, high)) in self.key_stick.iter_mut().zip(KEY_STICKS) {
            if let Some(byte) = axis.step(seconds, window.held(low), window.held(high)) {
                report.set_axis(slot, byte);
            }
        }

        // The analogue triggers, which are a pressure and not a bit: a pad's L2 and R2 travel, and
        // the game reads those two controls from their pressure byte alone.
        for (axis, control) in [(AXIS_L2, Control::L2), (AXIS_R2, Control::R2)] {
            let pressure = trigger_pressure(self.axis[axis]);
            if pressure > 0 {
                let binding = CONTROL_MAP[control.index()];
                report.0[binding.pressure as usize] = pressure;
                // The word is active low and the control is down, so clear its bit.
                let bit = binding.mask;
                let word = (((report.0[2] as u16) << 8) | report.0[3] as u16) & !bit;
                report.0[2] = (word >> 8) as u8;
                report.0[3] = word as u8;
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
                // A short read cannot happen on this device, and a closed one (unplugged mid-ride)
                // leaves the state where it was rather than dropping the rider.
                Ok(_) => return,
                Err(_) => return,
            }
        }
    }
}

/// The name the kernel gives a joystick, for the startup line.
fn device_name(path: &str) -> Option<String> {
    let node = path.rsplit('/').next()?;
    let name = std::fs::read_to_string(format!("/sys/class/input/{node}/device/name")).ok()?;
    Some(name.trim().to_string())
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

/// The keyboard as the game's own bitmask.
///
/// Laid out so that the four face keys are where the four face buttons are on a pad: `Z` is Square,
/// `X` Cross — the throttle — `C` Circle and `V` Triangle, with the shoulders on `A`, `S`, `D`, `F`
/// above them, Start on Return, and the arrow keys on the left stick.
fn keyboard(window: &Window) -> u16 {
    let mut mask = 0;
    for (code, control) in [
        (key::Z, Control::Square),
        (key::X, Control::Cross),
        (key::SPACE, Control::Cross),
        (key::C, Control::Circle),
        (key::V, Control::Triangle),
        (key::A, Control::L1),
        (key::S, Control::L2),
        (key::D, Control::R1),
        (key::F, Control::R2),
        (key::RETURN, Control::Start),
        (key::TAB, Control::Select),
        (key::Q, Control::L3),
        (key::E, Control::R3),
    ] {
        if window.held(code) {
            mask |= control.mask();
        }
    }
    mask
}

/// Two keys as one stick axis, ramped: where the stick has got to, as a fraction of full deflection
/// from −1 at the low key's edge to 1 at the high key's edge.
///
/// A key has nothing between off and on, so answering 0 or 255 the frame it goes down — which is what
/// this module used to do — pushes the stick to the stop instantly and holds it there. Everything the
/// sticks feed is then all-or-nothing: the lean is full lean, the steer is full lock, and a preload is
/// either nothing or a bottomed-out suspension, with I2's dead zone and its two segments crossed in a
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

#[cfg(test)]
mod tests {
    use super::*;
    use game_sim::input::{AXIS_CENTRE_RAW, AxisRange, Manager, PADS};
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
            manager.pad(0).axis(game_sim::input::Axis::LeftX)
        };
        // Frame one: inside I2's dead zone, so the lean, the steer and the preload all still read
        // nothing. The byte the module used to answer here was 255, which is `STICK.hi`.
        let first = axis.step(FRAME, false, true).unwrap();
        assert_eq!(read(first), F(0.0), "frame one of a tap is not a steering input");
        assert_eq!(read(255), AxisRange::STICK.hi, "what frame one used to be");
        // Three frames in it is out of the dead zone but nowhere near the stop.
        for _ in 0..2 {
            axis.step(FRAME, false, true);
        }
        let third = read(axis.step(FRAME, false, true).unwrap());
        assert!(
            third > F(0.0) && third < F(0.5),
            "three frames of a held key reads {third:?}"
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
}
