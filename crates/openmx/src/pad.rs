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
//!   [`game_sim::input::Report::from_buttons`] does, and what the kit's virtual pad sends.
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
        // The keyboard moves the left stick too, since half of riding is on it.
        if let Some(byte) = key_axis(window, key::LEFT, key::RIGHT) {
            report.set_axis(REPORT_LEFT_X, byte);
        }
        if let Some(byte) = key_axis(window, key::UP, key::DOWN) {
            report.set_axis(REPORT_LEFT_Y, byte);
        }
        if let Some(byte) = key_axis(window, key::J, key::L) {
            report.set_axis(REPORT_RIGHT_X, byte);
        }
        if let Some(byte) = key_axis(window, key::I, key::K) {
            report.set_axis(REPORT_RIGHT_Y, byte);
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

/// Two keys as one stick axis, or `None` while neither is held, so that a pad's own stick keeps the
/// axis when the keyboard is not touching it.
fn key_axis(window: &Window, low: u8, high: u8) -> Option<u8> {
    match (window.held(low), window.held(high)) {
        (true, false) => Some(0),
        (false, true) => Some(255),
        (true, true) => Some(AXIS_CENTRE),
        (false, false) => None,
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
