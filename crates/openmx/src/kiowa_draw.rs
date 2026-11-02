//! The Kiowa as the shell draws it (V113): its model posed by the rotor — the main rotor turned
//! about the mast and the tail rotor about its hub — with the blades' blur models (or the bent tail
//! once wrecked) hung on the posed blade nodes, for the GPU. The pose and the parts are
//! `game_render::kiowa`'s; this is where they meet the draw.
//!
//! It is drawn standing where the track's `machine.txt` parks it, see-through like every bay
//! ([`crate::vehicle::BAY_ALPHA`]), its rotor at rest; and since P216, solid while a player flies it,
//! at its measure's matrix with the flight's rotor angle, the blades gone and the tail bent once it
//! is wrecked. Since P216b the flown one is drawn by the software path too ([`skinned`]), solid,
//! and its pilot sits in it on both; the bays' see-through ones are still the GPU's alone, as the
//! plane's are.

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

/// The flown helicopter as the software path draws it (P216b), as `plane_draw::skinned_with` the
/// plane: the model skinned by the rotor's pose into `copy`, the seat's own copy of it (made here the
/// first time), then each of the rotor's parts up at its posed node.
pub fn skinned<'a>(drawn: &'a Kiowa, copy: &'a str, assets: &mut Assets, rotor: &Rotor, at: M4) -> Vec<(&'a str, M4)> {
    let pose = drawn.pose(assets, rotor);
    let mut out: Vec<(&'a str, M4)> = vec![(drawn.model.as_str(), at)];
    out.extend(drawn.parts(at, &pose, rotor));
    if !assets.models.contains_key(copy)
        && let Some(own) = assets.models.get(&drawn.model).cloned()
    {
        assets.models.insert(copy.to_string(), own);
    }
    if let (Some(loaded), false) = (assets.models.get_mut(copy), pose.is_empty()) {
        loaded.tris = game_render::skin::skin(&loaded.model, &pose, 0);
        out[0].0 = copy;
    }
    out
}
