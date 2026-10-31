//! The Windows build's pads (P192): gilrs' XInput backend (Xbox pads, and every pad Steam Input
//! drives) and Windows.Gaming.Input's raw controllers for PlayStation pads, read into the standard
//! mapping and put out as js events, as `ios.rs` and `web_host.rs` put theirs out, so the pad layer
//! reads them like `/dev/input/js*`.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};

use gilrs::{Axis, Button, Gilrs};

thread_local! {
    // ponytail: one Gilrs per thread that asks; the pad layer asks from one. Leaked: dropped at the thread's
    // end it joins its own backend thread inside a thread-local's destructor, which std aborts on.
    static GILRS: &'static RefCell<Option<Gilrs>> = Box::leak(Box::new(RefCell::new(Gilrs::new().ok())));
}

/// A pad now: the standard mapping's seventeen buttons (0..1) and four axes (-1..1, down positive).
#[derive(Clone, Copy)]
struct Pad {
    buttons: [f32; 17],
    axes: [f32; 4],
}

/// The standard mapping's buttons in order: south, east, west, north, the bumpers, the triggers,
/// select, start, the sticks' clicks, the d-pad up/down/left/right, and the guide button.
const BUTTONS: [Button; 17] = [
    Button::South,
    Button::East,
    Button::West,
    Button::North,
    Button::LeftTrigger,
    Button::RightTrigger,
    Button::LeftTrigger2,
    Button::RightTrigger2,
    Button::Select,
    Button::Start,
    Button::LeftThumb,
    Button::RightThumb,
    Button::DPadUp,
    Button::DPadDown,
    Button::DPadLeft,
    Button::DPadRight,
    Button::Mode,
];

/// Every pad connected now, by gilrs' id, its events drained first so the state is this frame's.
fn snapshot() -> Vec<(u32, String, Pad)> {
    GILRS.with(|g| {
        let mut g = g.borrow_mut();
        let Some(g) = g.as_mut() else { return Vec::new() };
        while g.next_event().is_some() {}
        g.gamepads()
            .map(|(id, p)| {
                let buttons = BUTTONS.map(|b| p.button_data(b).map_or(0.0, |d| d.value()));
                // gilrs' sticks are up positive; the standard mapping's down
                let axes = [p.value(Axis::LeftStickX), -p.value(Axis::LeftStickY), p.value(Axis::RightStickX), -p.value(Axis::RightStickY)];
                (usize::from(id) as u32, p.name().to_string(), Pad { buttons, axes })
            })
            .chain(sony())
            .collect()
    })
}

/// Sony's USB vendor id: the DualShock 4 and the DualSense.
const SONY: u16 = 0x054c;

/// The PlayStation pads connected now, by 100 + their place in Windows.Gaming.Input's list. Both
/// pads report buttons square, cross, circle, triangle, L1, R1, L2, R2, share, options, L3, R3, PS;
/// the d-pad as switch 0; axes 0..1 as LX, LY, RX, L2, R2, RY.
// ponytail: only Sony's; a Steam Controller or Deck without Steam running is still unseen, add its HID here if asked.
fn sony() -> Vec<(u32, String, Pad)> {
    use windows::Gaming::Input::{GameControllerSwitchPosition as S, RawGameController};
    let Ok(all) = RawGameController::RawGameControllers() else { return Vec::new() };
    (0..all.Size().unwrap_or(0))
        .filter_map(|i| {
            let c = all.GetAt(i).ok()?;
            if c.HardwareVendorId().ok()? != SONY {
                return None;
            }
            let mut b = vec![false; c.ButtonCount().ok()?.max(13) as usize];
            let mut s = vec![S::Center; c.SwitchCount().ok()?.max(1) as usize];
            let mut a = vec![0.5; c.AxisCount().ok()?.max(6) as usize];
            c.GetCurrentReading(&mut b, &mut s, &mut a).ok()?;
            let hat = s[0];
            let up = matches!(hat, S::Up | S::UpLeft | S::UpRight);
            let down = matches!(hat, S::Down | S::DownLeft | S::DownRight);
            let left = matches!(hat, S::Left | S::UpLeft | S::DownLeft);
            let right = matches!(hat, S::Right | S::UpRight | S::DownRight);
            let on = |x: bool| x as u8 as f32;
            // square cross circle triangle → south (cross), east (circle), west (square), north (triangle)
            let buttons = [
                on(b[1]), on(b[2]), on(b[0]), on(b[3]), on(b[4]), on(b[5]), a[3] as f32, a[4] as f32,
                on(b[8]), on(b[9]), on(b[10]), on(b[11]), on(up), on(down), on(left), on(right), on(b[12]),
            ];
            let axes = [a[0], a[1], a[2], a[5]].map(|v| (v * 2.0 - 1.0) as f32);
            let name = c.DisplayName().map(|n| n.to_string()).unwrap_or_default();
            Some((100 + i, if name.is_empty() { "PlayStation pad".into() } else { name }, Pad { buttons, axes }))
        })
        .collect()
}

/// The pads as the pad layer's device nodes: `gamepad/<id>`.
pub fn pads() -> Vec<(u32, PathBuf)> {
    snapshot().into_iter().map(|(i, _, _)| (i, format!("gamepad/{i}").into())).collect()
}

/// A pad's name, for the pad layer.
pub fn pad_name(node: &str) -> String {
    let Ok(i) = node.parse::<u32>() else { return String::new() };
    snapshot().into_iter().find(|(id, _, _)| *id == i).map(|(_, name, _)| name).unwrap_or_default()
}

/// The js numbers of the standard mapping's buttons, as `web_host.rs` has them.
const JS_BUTTON: [(usize, u8); 11] = [(0, 0), (1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (8, 6), (9, 7), (16, 8), (10, 9), (11, 10)];

/// A pad read as a js device: each [`Read`] one event, a pad no longer connected the end of the file.
pub struct PadNode {
    index: u32,
    last: Option<Pad>,
    queue: VecDeque<[u8; 8]>,
    gone: bool,
}

impl PadNode {
    pub fn open(path: &Path) -> std::io::Result<PadNode> {
        let index = path
            .to_str()
            .and_then(|p| p.strip_prefix("gamepad/")?.parse().ok())
            .ok_or_else(|| std::io::Error::other("not a gamepad"))?;
        Ok(PadNode { index, last: None, queue: Default::default(), gone: false })
    }

    fn event(&mut self, kind: u8, number: u8, value: i16, init: bool) {
        let ms = (crate::window::shell_us() / 1000) as u32;
        let mut e = [0u8; 8];
        e[..4].copy_from_slice(&ms.to_le_bytes());
        e[4..6].copy_from_slice(&value.to_le_bytes());
        e[6] = kind | if init { 0x80 } else { 0 };
        e[7] = number;
        self.queue.push_back(e);
    }

    fn take(&mut self) {
        let Some((_, _, now)) = snapshot().into_iter().find(|(i, _, _)| *i == self.index) else {
            self.gone = true;
            return;
        };
        let init = self.last.is_none();
        let was = self.last.unwrap_or(now);
        let axis = |v: f32| (v.clamp(-1.0, 1.0) * 32767.0) as i16;
        let trigger = |v: f32| (v.clamp(0.0, 1.0) * 65534.0 - 32767.0) as i16;
        for (from, js) in JS_BUTTON {
            let down = now.buttons[from] > 0.5;
            if init || down != (was.buttons[from] > 0.5) {
                self.event(0x01, js, down as i16, init);
            }
        }
        let value = |p: &Pad, n: u8| match n {
            0 => axis(p.axes[0]),
            1 => axis(p.axes[1]),
            2 => trigger(p.buttons[6]),
            3 => axis(p.axes[2]),
            4 => axis(p.axes[3]),
            5 => trigger(p.buttons[7]),
            6 => axis((p.buttons[15] - p.buttons[14]).round()),
            _ => axis((p.buttons[13] - p.buttons[12]).round()),
        };
        for n in 0..8 {
            let v = value(&now, n);
            if init || v != value(&was, n) {
                self.event(0x02, n, v, init);
            }
        }
        self.last = Some(now);
    }
}

impl Read for PadNode {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.queue.is_empty() && !self.gone {
            self.take();
            if self.queue.is_empty() && !self.gone {
                return Err(std::io::ErrorKind::WouldBlock.into());
            }
        }
        match self.queue.pop_front() {
            Some(e) => {
                let n = e.len().min(buf.len());
                buf[..n].copy_from_slice(&e[..n]);
                Ok(n)
            }
            None => Ok(0),
        }
    }
}

/// The sound out of the default device through WASAPI, cpal pulling the mix as it plays. Shared mode
/// takes only the device's own mix format (float, its rate, its channels), so the mix is rendered at
/// `rate` stereo and resampled to it.
pub struct Sound(#[allow(dead_code)] cpal::Stream);

impl Sound {
    pub fn start(rate: u32, mut fill: Box<dyn FnMut(&mut [i16]) + Send>) -> Result<Sound, String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        let device = cpal::default_host().default_output_device().ok_or("no sound device")?;
        let config = device.default_output_config().map_err(|e| e.to_string())?;
        if config.sample_format() != cpal::SampleFormat::F32 {
            return Err(format!("the sound device mixes in {}, not float", config.sample_format()));
        }
        let config = config.config();
        let channels = config.channels as usize;
        let mut resample = crate::audio::Resample::new(rate, config.sample_rate.0);
        let stream = device
            .build_output_stream(
                &config,
                move |out: &mut [f32], _| {
                    for frame in out.chunks_mut(channels) {
                        let [l, r] = resample.next(&mut fill);
                        frame[0] = l;
                        if channels > 1 {
                            frame[1] = r;
                        }
                        frame.iter_mut().skip(2).for_each(|s| *s = 0.0);
                    }
                },
                |e| eprintln!("sound: {e}"),
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Sound(stream))
    }
}
