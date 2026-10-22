//! `--shot-pose x,y,z,yaw,pitch,fov` (MD10): the first player's camera held at a chosen pose, so a
//! shot of any track or mod looks from where a shot of the original looked — openmx-mods' accuracy
//! harness puts the original's viewport at the same numbers over PINE and lays the pictures side by
//! side.
//!
//! The numbers are the harness's (its `tools/accuracy/src/pose.rs`): the eye in the disc's feet; yaw
//! in degrees about the vertical, 0 looking along +z and 90 along +x (`atan2(forward.x, forward.z)`);
//! pitch in degrees, positive looking up (`asin(forward.y)`); no roll. The fov is the viewport's own
//! number, which is the horizontal angle the cameras' presets carry (85 on the ride camera, whose 4:3
//! makes the 69 vertical a moment reads out of the original's projection), so it goes through the
//! same conversion to the view's shape as a preset's does.
//!
//! Presentation only: the camera is drawn from and nothing else. The ride under it and its sound
//! are the ride camera's business still.

use game_render::mat::v3;
use game_render::world::Camera;

/// A pose to shoot from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub eye: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    /// Horizontal, in degrees.
    pub fov: f32,
}

impl Pose {
    /// `x,y,z,yaw,pitch,fov`. A pitch of ±90 or more has no yaw to roll by and is refused, as is a
    /// fov a camera cannot open to.
    pub fn parse(text: &str) -> Result<Pose, String> {
        let n: Vec<f32> = text
            .split(',')
            .map(|w| w.trim().parse::<f32>().ok().filter(|v| v.is_finite()))
            .collect::<Option<_>>()
            .ok_or_else(|| format!("--shot-pose {text}: not six numbers x,y,z,yaw,pitch,fov"))?;
        let [x, y, z, yaw, pitch, fov] = n[..] else {
            return Err(format!("--shot-pose {text}: not six numbers x,y,z,yaw,pitch,fov"));
        };
        if pitch.abs() >= 90.0 {
            return Err(format!("--shot-pose {text}: a pitch of {pitch} looks straight up or down; keep it inside ±90"));
        }
        if !(1.0..179.0).contains(&fov) {
            return Err(format!("--shot-pose {text}: a fov of {fov} degrees; it opens between 1 and 179"));
        }
        Ok(Pose { eye: [x, y, z], yaw, pitch, fov })
    }

    /// The unit direction the pose looks along.
    pub fn forward(&self) -> [f32; 3] {
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        [sy * cp, sp, cy * cp]
    }

    /// The camera a `width` by `height` view is drawn through from the pose. The rasteriser's
    /// `look_at` keeps the world's up, which is the pose's no roll.
    pub fn camera(&self, width: usize, height: usize, near: f32, far: f32) -> Camera {
        let [x, y, z] = self.eye;
        let f = self.forward();
        Camera {
            eye: v3(x, y, z),
            at: v3(x + 100.0 * f[0], y + 100.0 * f[1], z + 100.0 * f[2]),
            fov: crate::aspect::fov(self.fov, width, height),
            near,
            far,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where `at` lands in a `w` by `h` view through `camera`, in pixels.
    fn pixel(camera: &Camera, (w, h): (usize, usize), at: [f32; 3]) -> (f32, f32) {
        let post = crate::overlay::marker_post(camera, (w, h), at).expect("in front of the eye");
        post.foot
    }

    #[test]
    fn reads_the_harness_numbers() {
        let p = Pose::parse("2217.5, 404, -2980,90,-12.5,85").unwrap();
        assert_eq!(p, Pose { eye: [2217.5, 404.0, -2980.0], yaw: 90.0, pitch: -12.5, fov: 85.0 });
        for bad in ["1,2,3,4,5", "1,2,3,4,5,6,7", "1,2,x,4,5,85", "1,2,3,4,90,85", "1,2,3,4,-95,85", "1,2,3,4,5,0", "1,2,3,nan,5,85"] {
            assert!(Pose::parse(bad).is_err(), "{bad} was read");
        }
    }

    /// Yaw 0 looks along +z, 90 along +x, and pitch is up positive — `atan2(x, z)` and `asin(y)` of
    /// the direction give the numbers back.
    #[test]
    fn yaw_and_pitch_are_the_harness_axes() {
        let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-6);
        let along = |yaw, pitch| Pose { eye: [0.0; 3], yaw, pitch, fov: 85.0 }.forward();
        assert!(close(along(0.0, 0.0), [0.0, 0.0, 1.0]));
        assert!(close(along(90.0, 0.0), [1.0, 0.0, 0.0]));
        assert!(close(along(180.0, 0.0), [0.0, 0.0, -1.0]));
        assert!(close(along(0.0, 30.0), [0.0, 0.5, 30f32.to_radians().cos()]));
        for (yaw, pitch) in [(37.0f32, -20.0f32), (-123.0, 45.0), (170.0, -80.0)] {
            let f = along(yaw, pitch);
            assert!((f[0].atan2(f[2]).to_degrees() - yaw).abs() < 1e-3, "{yaw}");
            assert!((f[1].asin().to_degrees() - pitch).abs() < 1e-3, "{pitch}");
        }
    }

    /// A marker on the ground where the pose looks lands at the middle of the shot, at any size, and
    /// one toward a larger yaw lands right of the middle — the shot looks from the pose's eye along
    /// its yaw and pitch, and yaw turns to the right as the harness's compass does (0 along +z, 90
    /// along +x).
    #[test]
    fn the_poses_ground_marker_is_centred() {
        for (eye, yaw, pitch) in [([4309.0, 215.0, 3308.0], 0.0f32, -18.0f32), ([-50.0, 20.0, 900.0], 135.0, -40.0), ([10.0, 300.0, 10.0], -60.0, -75.0)] {
            let pose = Pose { eye, yaw, pitch, fov: 85.0 };
            let f = pose.forward();
            // Where the line of sight meets the ground 30 feet under the eye.
            let t = 30.0 / -f[1];
            let ground = [eye[0] + t * f[0], eye[1] - 30.0, eye[2] + t * f[2]];
            for size in [(640, 448), (1920, 1080), (300, 800)] {
                let camera = pose.camera(size.0, size.1, 2.4, 4000.0);
                let (x, y) = pixel(&camera, size, ground);
                assert!((x - size.0 as f32 / 2.0).abs() < 0.5 && (y - size.1 as f32 / 2.0).abs() < 0.5, "{eye:?} {size:?}: ({x}, {y})");
                let (sy, cy) = yaw.to_radians().sin_cos();
                let right = [ground[0] + 5.0 * cy, ground[1], ground[2] - 5.0 * sy];
                assert!(pixel(&camera, size, right).0 > size.0 as f32 / 2.0 + 1.0, "{eye:?}: right of the marker is not right");
            }
        }
    }

    /// The fov is the horizontal one, as the presets' is: 85 at 4:3 is the original's 69 vertical.
    #[test]
    fn the_fov_is_the_presets_horizontal_one() {
        let camera = Pose { eye: [0.0; 3], yaw: 0.0, pitch: 0.0, fov: 85.0 }.camera(640, 480, 2.4, 4000.0);
        assert!((camera.fov.to_degrees() - 69.0).abs() < 0.05, "{}", camera.fov.to_degrees());
    }
}
