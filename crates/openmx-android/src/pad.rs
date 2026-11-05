//! A controller on Android (P308, as opentennis's M101): the patched winit
//! (`third_party/winit`, `platform::android::gamepad`) queues a pad's key and motion events, and
//! [`Held`] keeps what they say as the standard mapping's seventeen buttons and four axes — the
//! snapshot the iPhone's and the browser's pads give the shell's pad layer. Buttons map as Android's
//! standard layout names them (`KEYCODE_BUTTON_A` = Cross, `BUTTON_X` = Square, …), the left stick
//! is AXIS_X/Y, the right one Z/RZ, the d-pad either key events or the hat axes, the analog triggers
//! L2/R2 as the larger of each one's two axes (pads report one or the other).

/// The standard mapping's button for an Android key code (`KeyEvent.KEYCODE_*`).
pub fn button(keycode: u32) -> Option<usize> {
    Some(match keycode {
        96 => 0,         // BUTTON_A: Cross
        97 => 1,         // BUTTON_B: Circle
        99 => 2,         // BUTTON_X: Square
        100 => 3,        // BUTTON_Y: Triangle
        102 => 4,        // BUTTON_L1
        103 => 5,        // BUTTON_R1
        104 => 6,        // BUTTON_L2
        105 => 7,        // BUTTON_R2
        109 | 4 => 8,    // BUTTON_SELECT; some layouts give a pad's View/Back button KEYCODE_BACK
        108 => 9,        // BUTTON_START
        106 => 10,       // BUTTON_THUMBL
        107 => 11,       // BUTTON_THUMBR
        19 => 12,        // DPAD_UP
        20 => 13,        // DPAD_DOWN
        21 => 14,        // DPAD_LEFT
        22 => 15,        // DPAD_RIGHT
        110 => 16,       // BUTTON_MODE
        _ => return None,
    })
}

/// One pad's state, from its events in the order they came.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Held {
    keys: [bool; 17],
    /// The motion events' axes, in the patched winit's `AXES` order: X, Y, Z, RZ, RX, RY, HAT_X,
    /// HAT_Y, LTRIGGER, RTRIGGER, BRAKE, GAS.
    axes: [f32; 12],
}

impl Held {
    /// A key went down or up (`pressed`).
    pub fn key(&mut self, keycode: u32, pressed: bool) {
        if let Some(b) = button(keycode) {
            self.keys[b] = pressed;
        }
    }

    /// A motion event's axes.
    pub fn motion(&mut self, axes: [f32; 12]) {
        self.axes = axes;
    }

    /// The standard mapping's buttons (0 or 1, the triggers how far) and its sticks (y down).
    pub fn snapshot(&self) -> ([f32; 17], [f32; 4]) {
        let a = &self.axes;
        let mut b = self.keys.map(|k| k as u8 as f32);
        let mut press = |i: usize, on: bool| b[i] = b[i].max(on as u8 as f32);
        press(14, a[6] < -0.5);
        press(15, a[6] > 0.5);
        press(12, a[7] < -0.5);
        press(13, a[7] > 0.5);
        b[6] = b[6].max(a[8].max(a[10]).clamp(0.0, 1.0));
        b[7] = b[7].max(a[9].max(a[11]).clamp(0.0, 1.0));
        (b, [a[0], a[1], a[2], a[3]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_buttons_map_to_the_standard_pad() {
        assert_eq!(button(96), Some(0));
        assert_eq!(button(97), Some(1));
        assert_eq!(button(99), Some(2));
        assert_eq!(button(100), Some(3));
        assert_eq!(button(108), Some(9));
        assert_eq!(button(109), Some(8));
        assert_eq!(button(21), Some(14));
        assert_eq!(button(29), None);
    }

    #[test]
    fn hat_triggers_and_sticks_read_as_the_standard_pad() {
        let mut p = Held::default();
        let mut a = [0.0; 12];
        a[0] = 0.5;
        a[1] = 0.25; // stick pushed down-right
        a[3] = -1.0; // right stick up
        a[7] = -1.0; // hat up
        a[11] = 0.75; // GAS
        p.motion(a);
        p.key(96, true);
        let (b, axes) = p.snapshot();
        assert_eq!(axes, [0.5, 0.25, 0.0, -1.0]);
        assert_eq!((b[0], b[12], b[13], b[7], b[6]), (1.0, 1.0, 0.0, 0.75, 0.0));
        p.key(96, false);
        p.motion([0.0; 12]);
        assert_eq!(p.snapshot(), ([0.0; 17], [0.0; 4]));
    }
}
