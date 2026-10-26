//! The biplane as the shell draws it (V38): its model posed by its moving parts every frame — the
//! wing flaps by the ailerons, the rear flaps by the elevator, the rudder, and the prop — for each
//! renderer. The surface step is `game_sim::biplane_anim`'s (S35) off the plane's own controls
//! ([`Plane::surfaces`]), the matrices are `game_render::biplane::pose`'s; this is where the two meet
//! the draw.

use crate::vehicle::Plane;
use game_render::biplane::Biplane;
use game_render::gpu::Actor;
use game_render::mat::M4;
use game_render::world::Assets;

/// A plane as the GPU draws it (V38): the plane posed by its moving parts — the flaps, the rudder and
/// the prop as [`Plane::surfaces`] has them — and the prop cone at the posed `PropDummy`. The pose goes
/// on the end of `poses`, the plane's actor names it, and the cone, a model of its own, is unposed.
pub fn actors<'a>(drawn: &'a Biplane, assets: &Assets, plane: &Plane, at: M4, alpha: f32, poses: &mut Vec<Vec<M4>>) -> Vec<Actor<'a>> {
    let surfaces = plane.surfaces();
    let pose = drawn.pose(assets, &surfaces);
    let parts = drawn.posed_parts(at, &pose, &surfaces);
    let posed = (!pose.is_empty()).then(|| {
        poses.push(pose);
        poses.len() - 1
    });
    parts
        .into_iter()
        .enumerate()
        .map(|(i, (model, at))| Actor { model, at, pose: posed.filter(|_| i == 0), lod_from: None, alpha })
        .collect()
}

/// A plane as the software path draws it (V38): its model skinned by the moving parts' pose into a
/// copy of its own — `copy`, made off the livery's model the first time — since a skinned model's
/// triangles are its pose and two players can fly one livery. The cone is the livery's own.
pub fn skinned<'a>(drawn: &'a Biplane, copy: &'a str, assets: &mut Assets, plane: &Plane, at: M4) -> Vec<(&'a str, M4)> {
    let surfaces = plane.surfaces();
    let pose = drawn.pose(assets, &surfaces);
    let mut parts = drawn.posed_parts(at, &pose, &surfaces);
    if !assets.models.contains_key(copy)
        && let Some(own) = assets.models.get(&drawn.model).cloned()
    {
        assets.models.insert(copy.to_string(), own);
    }
    if let (Some(loaded), false) = (assets.models.get_mut(copy), pose.is_empty()) {
        loaded.tris = game_render::skin::skin(&loaded.model, &pose, 0);
        parts[0].0 = copy;
    }
    parts
}
