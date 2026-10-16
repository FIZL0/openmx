//! One frame of the original, drawn by the port: the same place, the same bike, the same camera.
//!
//! `--moment FILE` poses the shell from a moment taken out of the original and draws exactly one
//! frame of it, with no window, no pad and no simulation. The file is plain text, written by
//! `research/shots/moment.py` out of a save state's EE RAM:
//!
//! ```text
//! track free09
//! bike_pos 4309.5640 208.0300 3319.6233
//! bike_look -0.002623 -0.014706 0.999888
//! bike_up 0.026384 0.999542 0.014770
//! camera_eye 4309.6528 215.0176 3308.5303
//! camera_look -0.001245 -0.314115 0.949384
//! camera_up -0.000412 0.949385 0.314115
//! camera_fov_v 68.9974
//! ```
//!
//! The port is handed numbers, never addresses: where in RAM each of these was read from is in the
//! script and in the journal entry, which is where research belongs.
//!
//! `bike_look` and `bike_up` are rows 2 and 1 of the vehicle's own world transform, in the same
//! convention [`M4::placement`] builds — so the matrix the port puts the bike on is the matrix the
//! original had it on, to the digits the file carries. The camera is the frame's own: its eye, the
//! direction it faced and the vertical field of view its projection was built with.

use game_render::mat::{M4, V3, v3};
use game_render::png;
use game_render::raster::Frame;
use game_render::scene::Vehicle;
use game_render::scene::Scene as ViewScene;
use game_render::world::{Assets, Camera, Lighting, draw_scene};

use crate::ground::Ground;

/// A moment of the original: where the bike was, and what the camera saw it from.
pub struct Moment {
    /// The track the state was in, for the shell to check it was pointed at the same one.
    pub track: String,
    pub bike_pos: V3,
    pub bike_look: V3,
    pub bike_up: V3,
    pub camera_eye: V3,
    pub camera_look: V3,
    pub camera_up: V3,
    /// Vertical field of view, in degrees.
    pub camera_fov_v: f32,
}

impl Moment {
    pub fn read(path: &std::path::Path) -> Result<Moment, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut moment = Moment {
            track: String::new(),
            bike_pos: v3(0.0, 0.0, 0.0),
            bike_look: v3(0.0, 0.0, 1.0),
            bike_up: v3(0.0, 1.0, 0.0),
            camera_eye: v3(0.0, 0.0, 0.0),
            camera_look: v3(0.0, 0.0, 1.0),
            camera_up: v3(0.0, 1.0, 0.0),
            camera_fov_v: 69.0,
        };
        let mut seen = 0;
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let mut word = line.split_whitespace();
            let Some(key) = word.next() else { continue };
            let rest: Vec<&str> = word.collect();
            let number = |i: usize| -> Result<f32, String> {
                rest.get(i)
                    .ok_or(format!("{key}: wants {} numbers", i + 1))?
                    .parse::<f32>()
                    .map_err(|_| format!("{key}: {} is not a number", rest[i]))
            };
            let vector = || -> Result<V3, String> { Ok(v3(number(0)?, number(1)?, number(2)?)) };
            match key {
                "track" => moment.track = rest.first().unwrap_or(&"").to_string(),
                "bike_pos" => moment.bike_pos = vector()?,
                "bike_look" => moment.bike_look = vector()?,
                "bike_up" => moment.bike_up = vector()?,
                "camera_eye" => moment.camera_eye = vector()?,
                "camera_look" => moment.camera_look = vector()?,
                "camera_up" => moment.camera_up = vector()?,
                "camera_fov_v" => moment.camera_fov_v = number(0)?,
                other => return Err(format!("{}: unknown key {other}", path.display())),
            }
            seen += 1;
        }
        if seen == 0 {
            return Err(format!("{}: nothing in it", path.display()));
        }
        Ok(moment)
    }

    /// The camera as the renderer takes one. The rasteriser's `look_at` carries its own up axis, so
    /// the roll the original's camera had (a tenth of a degree on a chase camera) is not kept.
    pub fn camera(&self) -> Camera {
        Camera {
            eye: self.camera_eye,
            at: self.camera_eye.add(self.camera_look.norm().scale(100.0)),
            fov: self.camera_fov_v.to_radians(),
            near: 0.05,
            far: 4000.0,
        }
    }
}

/// What to draw and where to put it, so the one frame can be drawn without the loop's state.
pub struct Shot<'a> {
    pub width: usize,
    pub height: usize,
    pub ground_radius: u32,
    pub chassis: &'a str,
}

/// Draw the moment and write it out. One frame, no window: this is the picture that goes beside the
/// original's own screenshot of the same save state.
pub fn draw(
    moment: &Moment,
    out: &std::path::Path,
    shot: &Shot,
    ground: &Ground,
    assets: &Assets,
    vehicle: &Vehicle,
    look: &ViewScene,
) -> Result<(), String> {
    let camera = moment.camera();
    let light = Lighting::of(look);
    let mut frame = Frame::new(shot.width, shot.height, look.fog_color);
    for pixel in frame.color.chunks_exact_mut(3) {
        pixel.copy_from_slice(&look.fog_color);
    }
    frame.depth.fill(0.0);

    let mut drawn = ground.draw(
        &mut frame,
        &camera,
        &light,
        moment.bike_pos,
        shot.ground_radius,
    );
    let bike_at = M4::placement(moment.bike_pos, moment.bike_look, moment.bike_up);
    let mut actor: Vec<(&str, M4)> = vec![(shot.chassis, bike_at)];
    if let Some(chassis) = assets.models.get(shot.chassis) {
        for part in &vehicle.parts {
            let node = chassis
                .model
                .nodes
                .iter()
                .position(|n| n.name.eq_ignore_ascii_case(&part.placement));
            let at = match node {
                Some(i) => chassis.model.world(i).mul(bike_at),
                None => bike_at,
            };
            actor.push((part.model.as_str(), at));
        }
    }
    drawn += draw_scene(&mut frame, look, assets, &actor, &camera);

    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    png::write_rgb(out, frame.width, frame.height, &frame.color)?;
    println!(
        "moment: bike at {:.0}, {:.0}, {:.0}; camera at {:.0}, {:.0}, {:.0}, {:.1} degrees across \
         — {drawn} triangles into {}",
        moment.bike_pos.x,
        moment.bike_pos.y,
        moment.bike_pos.z,
        moment.camera_eye.x,
        moment.camera_eye.y,
        moment.camera_eye.z,
        moment.camera_fov_v,
        out.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# a moment
track free09
bike_pos 4309.5640 208.0300 3319.6233
bike_look -0.002623 -0.014706 0.999888
bike_up 0.026384 0.999542 0.014770
camera_eye 4309.6528 215.0176 3308.5303
camera_look -0.001245 -0.314115 0.949384
camera_up -0.000412 0.949385 0.314115
camera_fov_v 68.9974
";

    fn written(name: &str, text: &str) -> std::path::PathBuf {
        // One file per test: the tests run in parallel and a shared name is a race, not a fixture.
        let path = std::env::temp_dir().join(format!("openmx-moment-{name}.txt"));
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn a_moment_is_read_as_the_numbers_the_state_had() {
        let moment = Moment::read(&written("read", SAMPLE)).unwrap();
        assert_eq!(moment.track, "free09");
        assert_eq!(moment.bike_pos.z, 3319.6233);
        assert_eq!(moment.camera_eye.y, 215.0176);
        assert_eq!(moment.camera_fov_v, 68.9974);
        // The bike's look and up are two rows of a rotation, so they are a right angle apart.
        assert!(moment.bike_look.dot(moment.bike_up).abs() < 1e-3);
    }

    #[test]
    fn the_camera_looks_where_the_moment_says() {
        let moment = Moment::read(&written("camera", SAMPLE)).unwrap();
        let camera = moment.camera();
        let towards = camera.at.sub(camera.eye).norm();
        let look = moment.camera_look.norm();
        assert!((towards.x - look.x).abs() < 1e-5);
        assert!((towards.y - look.y).abs() < 1e-5);
        assert!((towards.z - look.z).abs() < 1e-5);
        // Degrees in the file, radians in the renderer.
        assert!((camera.fov - 1.204_2).abs() < 1e-3);
    }

    #[test]
    fn a_key_nobody_wrote_is_an_error_rather_than_a_silently_default_pose() {
        let text = format!("{SAMPLE}camera_roll 3\n");
        assert!(Moment::read(&written("unknown", &text)).is_err());
    }
}
