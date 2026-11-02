//! The Kiowa as the shell draws it (V113): its model posed by the rotor — the main rotor turned
//! about the mast and the tail rotor about its hub — with the blades' blur models (or the bent tail
//! once wrecked) hung on the posed blade nodes, for the GPU. The pose and the parts are
//! `game_render::kiowa`'s; this is where they meet the draw.
//!
//! Nothing flies the Kiowa yet (P216, with S220b's tick): what is drawn is the helicopter standing
//! where the track's `machine.txt` parks it, see-through like every bay ([`crate::vehicle::BAY_ALPHA`])
//! and on the GPU alone, as the plane's bays are, its rotor at rest.

use game_render::gpu::Actor;
use game_render::kiowa::{Kiowa, Rotor};
use game_render::mat::M4;
use game_render::world::Assets;

/// A helicopter placed at `at` as the GPU draws it: the model skinned by the rotor's pose (pushed on
/// the end of `poses`, which the model's actor names) and each of the rotor's parts up, unposed, at
/// its posed node. None of it casts a shadow, as no plane does.
pub fn actors<'a>(drawn: &'a Kiowa, assets: &Assets, rotor: &Rotor, at: M4, alpha: f32, poses: &mut Vec<Vec<M4>>) -> Vec<Actor<'a>> {
    let pose = drawn.pose(assets, rotor);
    let parts = drawn.parts(at, &pose, rotor);
    let posed = (!pose.is_empty()).then(|| {
        poses.push(pose);
        poses.len() - 1
    });
    let mut out = vec![Actor { model: drawn.model.as_str(), at, pose: posed, lod_from: None, alpha, casts: false }];
    out.extend(parts.into_iter().map(|(model, at)| Actor { model, at, pose: None, lod_from: None, alpha, casts: false }));
    out
}
