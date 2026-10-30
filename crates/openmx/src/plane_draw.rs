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
use game_sim::biplane_anim::Surfaces;

/// The moving parts a frame draws (V79): the plane's surfaces `was` the tick before and as they are
/// now, blended by how far the frame is between the two ticks, so a surface glides along the control
/// law's ramp instead of stepping once a tick under a picture drawn faster than the ticks. No `was`
/// (the plane just taken out, or the frame cut) draws them as they are.
pub fn shown(was: Option<&Surfaces>, plane: &Plane, t: f32) -> Surfaces {
    let now = plane.surfaces();
    was.map_or(now, |was| game_render::biplane::between(was, &now, t))
}

/// A plane as the GPU draws it (V38): the plane posed by its moving parts — the flaps, the rudder and
/// the prop as [`Plane::surfaces`] has them — and the prop cone at the posed `PropDummy`. The pose goes
/// on the end of `poses`, the plane's actor names it, and the cone, a model of its own, is unposed.
pub fn actors<'a>(drawn: &'a Biplane, assets: &Assets, plane: &Plane, at: M4, alpha: f32, poses: &mut Vec<Vec<M4>>) -> Vec<Actor<'a>> {
    actors_with(drawn, assets, plane.surfaces(), at, alpha, poses)
}

/// [`actors`] posed by `surfaces` rather than the plane's last tick's: what a frame drawn between two
/// ticks hands it, the two ticks' surfaces blended ([`shown`], V79).
pub fn actors_with<'a>(drawn: &'a Biplane, assets: &Assets, surfaces: Surfaces, at: M4, alpha: f32, poses: &mut Vec<Vec<M4>>) -> Vec<Actor<'a>> {
    let pose = drawn.pose(assets, &surfaces);
    let parts = drawn.posed_parts(at, &pose, &surfaces);
    let posed = (!pose.is_empty()).then(|| {
        poses.push(pose);
        poses.len() - 1
    });
    parts
        .into_iter()
        .enumerate()
        .map(|(i, (model, at))| Actor { model, at, pose: posed.filter(|_| i == 0), lod_from: None, alpha, casts: false })
        .collect()
}

/// A plane as the software path draws it (V38): its model skinned by the moving parts' pose into a
/// copy of its own — `copy`, made off the livery's model the first time — since a skinned model's
/// triangles are its pose and two players can fly one livery. The cone is the livery's own.
///
/// The shell draws through [`skinned_with`] (V79); this one, at the last tick's pose, is the tests'.
#[allow(dead_code)]
pub fn skinned<'a>(drawn: &'a Biplane, copy: &'a str, assets: &mut Assets, plane: &Plane, at: M4) -> Vec<(&'a str, M4)> {
    skinned_with(drawn, copy, assets, plane.surfaces(), at)
}

/// [`skinned`] posed by `surfaces` rather than the plane's last tick's ([`actors_with`]).
pub fn skinned_with<'a>(drawn: &'a Biplane, copy: &'a str, assets: &mut Assets, surfaces: Surfaces, at: M4) -> Vec<(&'a str, M4)> {
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
