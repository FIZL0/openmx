//! Photo mode (P385): the ride frozen under the pause, a free camera flown round it, the HUD hidden and
//! a full-size picture written of the view.
//!
//! The camera starts where the ride's camera was and is flown off the 1P pad, which the frozen ride
//! no longer reads:
//!
//! * **Left stick** moves along the view (up is forward), and sideways.
//! * **Right stick** looks round; with **L1** held it orbits the rider instead, keeping the distance.
//! * **R2 / L2** rise and sink. **R1** held moves three times as fast.
//! * **D-pad left / right** roll the horizon; **up / down** narrow and widen the field of view.
//! * **Square** puts the roll and the field of view back.
//!
//! The camera is presentation only: it reads the pad and writes a [`Camera`], and nothing in the
//! simulation reads it back, so its flight never moves a tick, a timer or the RNG.
//!
//! Auto Photos (P390) take the same picture without the pause: a wreck, a landed trick or a race's
//! results screen, each when the Display tab (or `--auto-photos`) turns it on ([`Auto`]).

use game_render::mat::v3;
use game_render::world::Camera;
use game_sim::input::{Control, Report};

/// How fast the camera flies, feet a second at full stick.
const SPEED: f32 = 30.0;
/// How fast it turns, degrees a second at full stick.
const TURN: f32 = 90.0;
/// How fast it rolls, degrees a second.
const ROLL: f32 = 45.0;
/// How fast the field of view opens or closes, degrees a second.
const ZOOM: f32 = 30.0;
/// The narrowest and widest field of view, horizontal degrees.
const FOV_RANGE: (f32, f32) = (10.0, 120.0);
/// How far from straight up or down the camera may look, so its up stays defined.
const PITCH_LIMIT: f32 = 89.0;
/// The share of a stick's travel read as resting.
const DEAD: f32 = 0.15;

/// The controls one frame of the pad gives the camera, each -1 to 1 (or on and off).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sticks {
    /// Forward (+) and back.
    pub ahead: f32,
    /// Right (+) and left.
    pub across: f32,
    /// Up (+) and down.
    pub rise: f32,
    /// Look right (+) and left.
    pub turn: f32,
    /// Look up (+) and down.
    pub tilt: f32,
    /// Roll clockwise (+) and back.
    pub roll: f32,
    /// Widen (+) and narrow the field of view.
    pub zoom: f32,
    /// L1: the right stick orbits the rider.
    pub orbit: bool,
    /// R1: fly faster.
    pub fast: bool,
    /// Square: the roll and the field of view put back.
    pub reset: bool,
}

/// A stick axis byte as -1 to 1, the rest band read as 0.
fn axis(byte: u8) -> f32 {
    let v = ((byte as f32 - 127.5) / 127.5).clamp(-1.0, 1.0);
    if v.abs() < DEAD { 0.0 } else { (v - DEAD * v.signum()) / (1.0 - DEAD) }
}

impl Sticks {
    /// The camera's controls off a pad's report ([`Report::axis`]'s order: right X, right Y, left X,
    /// left Y; a stick pushed up reads low). A report that failed to read gives none.
    pub fn of(report: &Report) -> Sticks {
        if report.status() == 0xff {
            return Sticks::default();
        }
        let b = report.buttons();
        let on = |c: Control| b & c.mask() != 0;
        let pair = |plus: Control, minus: Control| on(plus) as i32 as f32 - on(minus) as i32 as f32;
        Sticks {
            ahead: -axis(report.axis(3)),
            across: axis(report.axis(2)),
            rise: pair(Control::R2, Control::L2),
            turn: axis(report.axis(0)),
            tilt: -axis(report.axis(1)),
            roll: pair(Control::Right, Control::Left),
            zoom: pair(Control::Down, Control::Up),
            orbit: on(Control::L1),
            fast: on(Control::R1),
            reset: on(Control::Square),
        }
    }
}

/// The free camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Photo {
    /// Where it is, in the world's feet.
    pub eye: [f32; 3],
    /// Which way it looks, degrees: yaw 0 along +z and 90 along +x (as `--shot-pose` has it), pitch
    /// up from level.
    pub yaw: f32,
    pub pitch: f32,
    /// The horizon's tilt, degrees.
    pub roll: f32,
    /// The field of view, horizontal degrees.
    pub fov: f32,
    /// The field of view it started with, for [`Sticks::reset`].
    pub start_fov: f32,
    /// What it orbits: the rider, where the ride froze.
    pub pivot: [f32; 3],
}

impl Photo {
    /// The camera where `camera` is, drawn into a `width` by `height` view, orbiting `pivot`.
    pub fn from_camera(camera: &Camera, width: usize, height: usize, pivot: [f32; 3]) -> Photo {
        let d = camera.at.sub(camera.eye);
        let flat = (d.x * d.x + d.z * d.z).sqrt();
        let aspect = width as f32 / height.max(1) as f32;
        let fov = (2.0 * ((camera.fov / 2.0).tan() * aspect).atan()).to_degrees();
        Photo {
            eye: [camera.eye.x, camera.eye.y, camera.eye.z],
            yaw: d.x.atan2(d.z).to_degrees(),
            pitch: d.y.atan2(flat).to_degrees().clamp(-PITCH_LIMIT, PITCH_LIMIT),
            roll: 0.0,
            fov: fov.clamp(FOV_RANGE.0, FOV_RANGE.1),
            start_fov: fov.clamp(FOV_RANGE.0, FOV_RANGE.1),
            pivot,
        }
    }

    /// The unit direction it looks along.
    pub fn forward(&self) -> [f32; 3] {
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        [sy * cp, sp, cy * cp]
    }

    /// One frame of `dt` seconds flown by `s`.
    pub fn step(&mut self, s: Sticks, dt: f32) {
        if s.reset {
            self.roll = 0.0;
            self.fov = self.start_fov;
        }
        if s.orbit {
            self.orbit(s.turn * TURN * dt, s.tilt * TURN * dt);
        } else {
            self.yaw += s.turn * TURN * dt;
            self.pitch = (self.pitch + s.tilt * TURN * dt).clamp(-PITCH_LIMIT, PITCH_LIMIT);
        }
        let speed = SPEED * if s.fast { 3.0 } else { 1.0 } * dt;
        let f = self.forward();
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        // Right of a look along yaw: yaw 0 looks along +z and its right is +x, the way a larger yaw
        // turns (as `--shot-pose`'s marker test has it).
        let right = [cy, 0.0, -sy];
        for k in 0..3 {
            self.eye[k] += (f[k] * s.ahead + right[k] * s.across) * speed;
        }
        self.eye[1] += s.rise * speed;
        self.roll = (self.roll + s.roll * ROLL * dt).clamp(-180.0, 180.0);
        self.fov = (self.fov + s.zoom * ZOOM * dt).clamp(FOV_RANGE.0, FOV_RANGE.1);
    }

    /// Swing round the pivot by `yaw` and `pitch` degrees, keeping the distance and looking at it.
    fn orbit(&mut self, yaw: f32, pitch: f32) {
        let p = self.pivot;
        let d = [self.eye[0] - p[0], self.eye[1] - p[1], self.eye[2] - p[2]];
        let r = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1.0);
        // The pivot as seen from the eye, then turned.
        let mut look_yaw = (-d[0]).atan2(-d[2]).to_degrees() + yaw;
        let flat = (d[0] * d[0] + d[2] * d[2]).sqrt();
        let look_pitch = ((-d[1]).atan2(flat).to_degrees() + pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);
        if !look_yaw.is_finite() {
            look_yaw = self.yaw;
        }
        self.yaw = look_yaw;
        self.pitch = look_pitch;
        let f = self.forward();
        self.eye = [p[0] - f[0] * r, p[1] - f[1] * r, p[2] - f[2] * r];
    }

    /// The camera a `width` by `height` view is drawn through. Its roll is not the camera's: it is
    /// the renderer's ([`game_render::world::set_view_roll`]), which the shell sets from
    /// [`Photo::roll_radians`].
    pub fn camera(&self, width: usize, height: usize, near: f32, far: f32) -> Camera {
        let [x, y, z] = self.eye;
        let f = self.forward();
        Camera {
            eye: v3(x, y, z),
            at: v3(x + 100.0 * f[0], y + 100.0 * f[1], z + 100.0 * f[2]),
            fov: crate::aspect::fov(self.fov, width, height),
            near,
            far,
            up: None,
        }
    }

    /// The roll, in the radians the renderer takes.
    pub fn roll_radians(&self) -> f32 {
        self.roll.to_radians()
    }
}

/// What an auto-shot (P390) is of: a moment of the ride that takes a picture on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moment {
    /// A rider thrown off: shot [`WRECK_DELAY`] ticks after the wreck starts, the rider in the air.
    Wreck,
    /// A trick landed, shot as it lands.
    Trick,
}

/// How many ticks after a wreck starts its picture is taken: 0.4 s, the rider off the bike.
pub const WRECK_DELAY: u64 = 12;
/// The fewest ticks from one auto-shot to the next (2 s), so a run of landed tricks or a wreck that
/// ends a trick fills the folder with one picture, not a burst.
pub const AUTO_GAP: u64 = 60;

/// The ride's auto-shots (P390): the moments heard, and the tick each one's picture falls due. One
/// waits at a time; a moment heard while one waits, or within [`AUTO_GAP`] of the last picture,
/// takes none. Presentation only, as the free camera is: it reads the ride's ticks and moments and
/// nothing reads it back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Auto {
    /// The tick the waiting picture is due on, and its moment.
    due: Option<(u64, Moment)>,
    /// The tick of the last picture taken.
    last: Option<u64>,
}

impl Auto {
    /// `moment` heard on tick `now`.
    pub fn heard(&mut self, moment: Moment, now: u64) {
        self.rewound(now);
        if self.due.is_some() || self.last.is_some_and(|t| now < t + AUTO_GAP) {
            return;
        }
        let delay = if moment == Moment::Wreck { WRECK_DELAY } else { 0 };
        self.due = Some((now + delay, moment));
    }

    /// The moment whose picture falls due on tick `now`, once.
    pub fn due(&mut self, now: u64) -> Option<Moment> {
        self.rewound(now);
        let (at, moment) = self.due.filter(|(at, _)| now >= *at)?;
        self.due = None;
        self.last = Some(at.max(now));
        Some(moment)
    }

    /// The ride put back to an earlier tick than it has heard (a restart, a tape rewound): nothing
    /// waits, and the gap starts again.
    fn rewound(&mut self, now: u64) {
        let heard = self.last.into_iter().chain(self.due.map(|(at, _)| at.saturating_sub(WRECK_DELAY))).max();
        if heard.is_some_and(|t| now < t) {
            *self = Auto::default();
        }
    }
}

/// The name of the `n`th picture in a folder: `photo-0001.png` and on.
pub fn file_name(n: u32) -> String {
    format!("photo-{n:04}.png")
}

/// The first picture name in `dir` that is not taken yet.
pub fn next_path(dir: &std::path::Path) -> std::path::PathBuf {
    (1..)
        .map(|n| dir.join(file_name(n)))
        .find(|p| !p.exists())
        .expect("a free picture name")
}

/// A `width` by `height` frame's `rgb` written as the next picture in `dir`, made if it is not
/// there: the line said of it, `Photo saved: …` or `Photo not saved: …` (a failed write is said,
/// not fatal).
pub fn save(dir: &std::path::Path, width: usize, height: usize, rgb: &[u8]) -> String {
    let wrote = std::fs::create_dir_all(dir).map_err(|e| e.to_string()).map(|()| next_path(dir));
    match wrote.and_then(|path| game_render::png::write_rgb(&path, width, height, rgb).map(|()| path)) {
        Ok(path) => format!("Photo saved: {}", path.display()),
        Err(e) => format!("Photo not saved: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn camera() -> Camera {
        Camera { eye: v3(0.0, 10.0, -20.0), at: v3(0.0, 10.0, 0.0), fov: 1.0, near: 1.0, far: 1000.0, up: None }
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn starts_where_the_ride_camera_was() {
        let c = camera();
        let p = Photo::from_camera(&c, 640, 448, [0.0, 0.0, 0.0]);
        let back = p.camera(640, 448, c.near, c.far);
        assert_eq!(p.eye, [0.0, 10.0, -20.0]);
        assert!(close(p.yaw, 0.0) && close(p.pitch, 0.0));
        assert!(close(back.fov, c.fov), "{} against {}", back.fov, c.fov);
        let d = back.at.sub(back.eye).norm();
        assert!(close(d.z, 1.0));
    }

    #[test]
    fn the_left_stick_flies_along_the_view() {
        let mut p = Photo::from_camera(&camera(), 640, 448, [0.0; 3]);
        p.step(Sticks { ahead: 1.0, ..Sticks::default() }, 1.0);
        assert!(close(p.eye[2], -20.0 + SPEED), "{:?}", p.eye);
        p.step(Sticks { across: 1.0, ..Sticks::default() }, 1.0);
        // Right of a look along +z is +x.
        assert!(close(p.eye[0], SPEED), "{:?}", p.eye);
        p.step(Sticks { rise: 1.0, fast: true, ..Sticks::default() }, 1.0);
        assert!(close(p.eye[1], 10.0 + 3.0 * SPEED), "{:?}", p.eye);
    }

    #[test]
    fn orbiting_keeps_the_distance_and_the_rider_in_view() {
        let pivot = [0.0, 10.0, 0.0];
        let mut p = Photo::from_camera(&camera(), 640, 448, pivot);
        p.step(Sticks { turn: 1.0, orbit: true, ..Sticks::default() }, 1.0);
        let d = [p.eye[0] - pivot[0], p.eye[1] - pivot[1], p.eye[2] - pivot[2]];
        let r = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        assert!(close(r, 20.0), "{r}");
        assert!(close(p.yaw, TURN), "{}", p.yaw);
        let f = p.forward();
        assert!(close(f[0], -d[0] / r) && close(f[2], -d[2] / r), "{f:?} {d:?}");
    }

    #[test]
    fn roll_and_zoom_are_held_in_range_and_reset() {
        let mut p = Photo::from_camera(&camera(), 640, 448, [0.0; 3]);
        let start = p.fov;
        p.step(Sticks { roll: 1.0, zoom: 1.0, ..Sticks::default() }, 1.0);
        assert!(close(p.roll, ROLL) && close(p.fov, (start + ZOOM).min(FOV_RANGE.1)));
        p.step(Sticks { zoom: -1.0, ..Sticks::default() }, 100.0);
        assert!(close(p.fov, FOV_RANGE.0));
        p.step(Sticks { reset: true, ..Sticks::default() }, 0.0);
        assert!(close(p.roll, 0.0) && close(p.fov, start));
    }

    #[test]
    fn the_pad_maps_to_the_controls() {
        let mut r = Report::from_buttons(Control::L1.mask() | Control::R2.mask() | Control::Up.mask());
        r.set_axis(3, 0); // left stick up
        r.set_axis(0, 255); // right stick right
        let s = Sticks::of(&r);
        assert!(close(s.ahead, 1.0) && close(s.turn, 1.0));
        assert!(s.orbit && !s.fast);
        assert_eq!((s.rise, s.zoom, s.across, s.tilt), (1.0, -1.0, 0.0, 0.0));
        assert_eq!(Sticks::of(&Report::idle()), Sticks::default());
    }

    #[test]
    fn a_landed_trick_shoots_at_once_and_a_wreck_a_moment_later() {
        let mut a = Auto::default();
        a.heard(Moment::Trick, 100);
        assert_eq!(a.due(100), Some(Moment::Trick));
        assert_eq!(a.due(101), None);
        a.heard(Moment::Wreck, 100 + AUTO_GAP);
        assert_eq!(a.due(100 + AUTO_GAP + WRECK_DELAY - 1), None);
        assert_eq!(a.due(100 + AUTO_GAP + WRECK_DELAY), Some(Moment::Wreck));
    }

    #[test]
    fn moments_close_together_take_one_picture() {
        let mut a = Auto::default();
        a.heard(Moment::Wreck, 10);
        // A trick heard while the wreck's picture waits takes none.
        a.heard(Moment::Trick, 11);
        assert_eq!(a.due(10 + WRECK_DELAY), Some(Moment::Wreck));
        // Nor one inside the gap after it.
        a.heard(Moment::Trick, 10 + WRECK_DELAY + AUTO_GAP - 1);
        assert_eq!(a.due(10 + WRECK_DELAY + AUTO_GAP - 1), None);
        a.heard(Moment::Trick, 10 + WRECK_DELAY + AUTO_GAP);
        assert_eq!(a.due(10 + WRECK_DELAY + AUTO_GAP), Some(Moment::Trick));
        // A restart's ticks begin again: the gap does too.
        a.heard(Moment::Trick, 3);
        assert_eq!(a.due(3), Some(Moment::Trick));
    }

    #[test]
    fn pictures_take_the_next_free_name() {
        let dir = std::env::temp_dir().join(format!("openmx-photo-names-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(next_path(&dir), dir.join("photo-0001.png"));
        std::fs::write(dir.join("photo-0001.png"), b"").unwrap();
        assert_eq!(next_path(&dir), dir.join("photo-0002.png"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
