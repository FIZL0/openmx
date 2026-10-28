//! A `.toy`'s animated toys (V43): the windmills, the windsock, the shovel, the church bell, the
//! gator cook and his pipe, nat11's gondolas, the steam saw, and every supercross track's finish gate
//! and blimp — each a model on a clip of its own, at a place of its own.
//!
//! **The section.** `[AnimatedToys] NumberOfAnimatedToys = n`, then `[AnimatedToy1]` …
//! `[AnimatedToyn]`, read the way the original's reader reads them (`game_sim::phy::Ini`, which is
//! case-blind and squeezes the spaces out of `Name = gondola_01`). The count is what the loader walks:
//! free11's eight balloons are written under a count of 0, and the original loads none of them. The
//! keys a toy here reads, with the original's defaults:
//!
//! - `MCFName`: the animation set, whose `SLTFile` is the model. Its stem with `.col` is the
//!   collision file.
//! - `InitialMotion` (`none`): the clip, by the set's name for it.
//! - `InitialTime` (0): one advance of the clip at load. No toy on the disc writes it.
//! - `HasCollision`: whether the `.col` is loaded.
//! - `AnimateWhenNotVisible` (on): off, the toy steps only after a frame that drew it (V76,
//!   [`game_sim::toys::Toys::seen`]); free09's five toys, nat02's bell and free10's windsock turn it off.
//! - `UpdateSearch` (on): off, the toy's step leaves its collision parts where they stand.
//! - `Position` (the origin), `LookVector` (0,0,1) and `UpVector` (0,1,0): the placement, which is
//!   `mat4_basis` of the three with both of its flags set ([`game_sim::math::basis`]). The reader
//!   takes `Yaw`, `Pitch` and `Roll` instead when there is a `Yaw`; no animated toy on the disc has
//!   one, and that branch is not ported.
//!
//! **The clip is the simulation's** ([`game_sim::toys`]). A toy with collision moves what the bike
//! hits, so its clock is part of the sim's state and rolls back with it; the toys without collision
//! are stepped there too, since one clip's hints are shared by every toy that plays it. This module
//! builds the sim's toys out of the disc and draws them where the first seat's sim has them: posed at
//! the clock blended between the last two ticks ([`palette`]), in host floats.
//!
//! **The collision.** A `.col` of the mesh class stands at the placement. One of the hierarchy class
//! (free10's shovel, nat11's gondolas) is split into its parts, and each part rides one bone of the
//! model, counted the way the hierarchy's loader flattens the node tree ([`flatten`]): bone 0 is the
//! toy's own root node, then the root's children, then each child's subtree in the same way.

use crate::anim::Set;
use crate::disc::Res;
use crate::mat::{M4, V3, v3};
use crate::world::Camera;
use crate::psg::Model;
use crate::rider::MotionSet;
use crate::skin::{self, Skeleton};
use game_sim::math::{self, Mat4};
use game_sim::motion::Animations;
use game_sim::phy::{Ini, Section};
use game_sim::toys::{self, Motion, Part, Rig, Spec, Toys};
use game_sim::track::Collision;
use ps2_float::{F, Vf};

/// One `[AnimatedToyN]` as the reader keeps it.
#[derive(Clone, Debug)]
pub struct Toy {
    /// `Name`, for messages.
    pub name: String,
    /// `MCFName`.
    pub mcf: String,
    /// `InitialMotion`.
    pub motion: String,
    /// `InitialTime`.
    pub start: F,
    /// `HasCollision`.
    pub collision: bool,
    /// `AnimateWhenNotVisible`.
    pub unseen: bool,
    /// `UpdateSearch`.
    pub search: bool,
    /// The placement: `mat4_basis(Position, LookVector, UpVector, 1, 1)`.
    pub place: Mat4,
}

impl Toy {
    /// `MCFName` without its extension, which is what the `.col` is named after.
    pub fn stem(&self) -> &str {
        self.mcf.rsplit_once('.').map_or(self.mcf.as_str(), |(stem, _)| stem)
    }
}

/// The animated toys a `.toy` lists, `AnimatedToy1` to `AnimatedToyN` for the count its
/// `[AnimatedToys]` gives. A section the count reaches but the file does not have names no set,
/// so it loads nothing and is left out here.
pub fn toys(bytes: &[u8]) -> Vec<Toy> {
    let ini = Ini::parse_bytes(bytes);
    let count = ini.section("AnimatedToys").int("NumberOfAnimatedToys").unwrap_or(0).max(0);
    (1..=count)
        .map(|i| ini.section(&format!("AnimatedToy{i}")))
        .filter(|s| s.text("MCFName").is_some())
        .map(|s| toy(&s))
        .collect()
}

fn toy(s: &Section) -> Toy {
    let text = |key: &str, default: &str| s.text(key).unwrap_or(default).to_string();
    let pos = s.vector("Position", Vf::new(0.0, 0.0, 0.0, 1.0));
    let look = s.vector("LookVector", Vf::new(0.0, 0.0, 1.0, 1.0));
    let up = s.vector("UpVector", Vf::new(0.0, 1.0, 0.0, 1.0));
    Toy {
        name: text("Name", ""),
        mcf: text("MCFName", ""),
        motion: text("InitialMotion", "none"),
        start: s.float_or("InitialTime", F::ZERO),
        collision: s.flag("HasCollision", false),
        unseen: s.flag("AnimateWhenNotVisible", true),
        search: s.flag("UpdateSearch", true),
        place: math::basis(pos, look, up, true, true),
    }
}

/// A node tree in the order the collision hierarchy's loader numbers its bones: the root first (here
/// `None`, the toy's own root node, whose children are the model's top nodes), then every node's
/// children before any of their own children, each subtree in turn. A sibling's order is the
/// model's node order.
pub fn flatten(parents: &[Option<usize>]) -> Vec<Option<usize>> {
    fn under(of: Option<usize>, parents: &[Option<usize>], out: &mut Vec<Option<usize>>) {
        let children: Vec<usize> = (0..parents.len()).filter(|&j| parents[j] == of && Some(j) != of).collect();
        out.extend(children.iter().map(|&c| Some(c)));
        for c in children {
            under(Some(c), parents, out);
        }
    }
    let mut out = vec![None];
    under(None, parents, &mut out);
    out
}

/// A model-space matrix of the sim's, as the renderer's.
pub fn m4(m: &Mat4) -> M4 {
    M4(m.0.map(|r| [r.x(), r.y(), r.z(), r.w()]))
}

/// A bound clip as the simulation's: its timing as the set left it, and its keys as the file's bits.
pub fn motion(clip: &skin::Clip) -> Motion {
    let rotation = clip
        .rotation
        .iter()
        .map(|c| toys::RotChannel {
            name: c.name as u32,
            on: true,
            keys: match &c.keys {
                skin::RotKeys::Wide(k) => {
                    toys::RotKeys::Wide(k.iter().map(|k| toys::RotKey { t: F(k.t), turn: k.turn, axis: k.axis }).collect())
                }
                skin::RotKeys::Narrow(k) => {
                    toys::RotKeys::Narrow(k.iter().map(|k| toys::NarrowKey { t: F(k.t), turn: k.turn, axis: k.axis }).collect())
                }
            },
        })
        .collect();
    let position = clip
        .position
        .iter()
        .map(|c| toys::PosChannel {
            name: c.name as u32,
            on: true,
            keys: c.keys.iter().map(|k| toys::PosKey { t: F(k.t), p: k.p.map(F) }).collect(),
        })
        .collect();
    Motion { timing: clip.timing.clone(), rotation, position }
}

/// A skeleton as the toy's motion object holds it, under a clip's joint map.
pub fn rig(skeleton: &Skeleton, map: &[u8]) -> Rig {
    let rows = |m: &M4| Mat4(m.0.map(|r| Vf::new(r[0], r[1], r[2], r[3])));
    Rig {
        parents: skeleton.bones.iter().map(|b| b.parent).collect(),
        rest: skeleton.bones.iter().map(|b| rows(&b.local)).collect(),
        map: map.to_vec(),
    }
}

/// The track's animated toys: the sim's, and what the picture needs to draw each of them.
#[derive(Default)]
pub struct Animated {
    /// For each of the sim's toys, in order: the model the picture draws it with.
    pub models: Vec<String>,
    /// The sim's toys, which every seat gets a copy of.
    pub sim: Toys,
    /// The toys left out, each with why.
    pub skipped: Vec<String>,
    /// For each of the sim's toys: its name in the `.toy`, and its skeleton's bone names by joint
    /// (V71: the emitters that ride a toy's node find it by these).
    pub names: Vec<String>,
    pub bones: Vec<Vec<String>>,
}

impl Animated {
    /// The animated toys of the track whose `.toy` is `<track>.toy` in one of these archives, each
    /// on its clip and at its place, with its collision. The archives are lent to one animation set
    /// after another and handed back.
    pub fn load(archives: &mut Vec<Res>, track: &str) -> Animated {
        let mut out = Animated::default();
        let Some(text) = archives.iter().find_map(|a| a.read_suffix(&format!("{track}.toy")).ok()) else { return out };
        let listed = toys(&text);
        if listed.is_empty() {
            return out;
        }
        // One set per `.mcf`, one motion per clip of a set: the original's clip objects are its
        // animation set's, one per name, so the toys that play one name share its channels' hints.
        // A set holds the archives while its clips are bound, and gives them back before the next.
        let mut sets: Vec<Loaded> = Vec::new();
        let mut motions: Vec<Motion> = Vec::new();
        for toy in &listed {
            let key = toy.mcf.to_ascii_lowercase();
            if sets.iter().any(|l| l.key == key) {
                continue;
            }
            let mut set = match set_of(archives, &toy.mcf) {
                Ok(set) => set,
                Err(why) => {
                    sets.push(Loaded { key, skeleton: Skeleton::default(), geometry: String::new(), clips: Vec::new(), failed: Some(why) });
                    continue;
                }
            };
            let mut clips = Vec::new();
            for name in listed.iter().filter(|t| t.mcf.eq_ignore_ascii_case(&toy.mcf)).map(|t| t.motion.as_str()) {
                if clips.iter().any(|(n, _, _): &(String, usize, Vec<u8>)| n.eq_ignore_ascii_case(name)) {
                    continue;
                }
                set.load(name);
                if let Some(bound) = set.id(name).and_then(|id| set.get(id)) {
                    motions.push(motion(&bound.clip));
                    clips.push((name.to_string(), motions.len() - 1, bound.joints.clone()));
                }
            }
            let (skeleton, geometry) = (set.skeleton.clone(), set.set.geometry());
            *archives = set.into_archives();
            sets.push(Loaded { key, skeleton, geometry, clips, failed: None });
        }
        let mut specs = Vec::new();
        for toy in &listed {
            let Some(set) = sets.iter().find(|l| l.key.eq_ignore_ascii_case(&toy.mcf)) else { continue };
            if let Some(why) = &set.failed {
                out.skipped.push(format!("{}: {why}", toy.name));
                continue;
            }
            let Some((_, motion, joints)) = set.clips.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(&toy.motion)) else {
                out.skipped.push(format!("{}: {} has no clip {}", toy.name, toy.mcf, toy.motion));
                continue;
            };
            let (mesh, parts) = if toy.collision { collision(archives, toy, &set.skeleton) } else { (None, Vec::new()) };
            specs.push(Spec { motion: *motion, rig: rig(&set.skeleton, joints), place: toy.place, start: toy.start, mesh, parts, unseen: toy.unseen, search: toy.search });
            out.models.push(set.geometry.clone());
            out.names.push(toy.name.clone());
            out.bones.push(set.skeleton.bones.iter().map(|b| b.name.clone()).collect());
        }
        out.sim = Toys::new(motions, specs);
        out
    }
}

/// One `.mcf`'s part of the load: its skeleton and model, and its clips by name with the motion each
/// became and its joint map — or why it did not load.
struct Loaded {
    key: String,
    skeleton: Skeleton,
    geometry: String,
    clips: Vec<(String, usize, Vec<u8>)>,
    failed: Option<String>,
}

/// The animation set a `.mcf` names, with the model its `SLTFile` is as the skeleton, built over all
/// of `archives`, which it takes ([`Set::into_archives`] gives them back). On an error the archives are
/// untouched.
fn set_of(archives: &mut Vec<Res>, mcf: &str) -> Result<Set, String> {
    let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(name).ok());
    let bytes = read(&format!("\\{mcf}")).or_else(|| read(mcf)).ok_or_else(|| format!("no archive holds {mcf}"))?;
    let set = MotionSet::parse(&crate::ini::text(&bytes));
    let geometry = set.geometry();
    if geometry.is_empty() {
        return Err(format!("{mcf}: no [General info] SLTFile"));
    }
    let psg = format!("{geometry}.psg");
    let bytes = read(&psg).ok_or_else(|| format!("{mcf} names {psg}, which no archive has"))?;
    let model = Model::parse(&bytes).map_err(|e| format!("{psg}: {e}"))?;
    let lent = std::mem::take(archives);
    Ok(Set::new(set, Skeleton::of(&model), lent))
}

/// A toy's `.col`: a mesh standing at the placement, or a hierarchy's parts on their bones. A bone
/// past the last is the last, as the loader clamps it.
fn collision(archives: &[Res], toy: &Toy, skeleton: &Skeleton) -> (Option<game_sim::track::Mesh>, Vec<Part>) {
    let file = format!("\\{}.col", toy.stem());
    let parsed = archives
        .iter()
        .find_map(|a| a.read_suffix(&file).ok())
        .ok_or_else(|| "no such member".to_string())
        .and_then(|b| Collision::parse(&b));
    match parsed {
        Ok(Collision::Mesh(mesh)) => (Some(mesh), Vec::new()),
        Ok(Collision::Hierarchy(h)) => {
            let parents: Vec<Option<usize>> = skeleton.bones.iter().map(|b| b.parent).collect();
            let bones = flatten(&parents);
            let parts = h
                .parts
                .into_iter()
                .map(|p| Part { joint: bones[(p.bone as usize).min(bones.len() - 1)], mesh: p.mesh })
                .collect();
            (None, parts)
        }
        Ok(Collision::Rays(_)) => {
            println!("animated toys: {}'s {file} is a ray set, which a toy does not load", toy.name);
            (None, Vec::new())
        }
        Err(why) => {
            println!("animated toys: {}'s {file}: {why}", toy.name);
            (None, Vec::new())
        }
    }
}

/// The clip time a picture `by` of the way from the tick before the last to the last shows: the
/// clock's last step, from where it was to where it is, wrapped at the clip's end for a clip that
/// loops. A clip that has finished holds its end.
pub fn shown_time(clock: &toys::Clock, timing: &game_sim::motion::Clip, by: f32) -> f32 {
    let length = timing.duration.0;
    if clock.finished {
        return length;
    }
    let mut step = clock.time.0 - clock.prev.0;
    if step < 0.0 {
        step += length;
    }
    let t = clock.prev.0 + step * by.clamp(0.0, 1.0);
    if timing.looping && length > 0.0 && t >= length { t - length } else { t }
}

/// One toy's joints in its model's space at the picture's moment ([`shown_time`]): the sim's own
/// sample of its clip there, from a copy of its hints, composed down the model's chain in host
/// floats. What the GPU's palette and the software skin both take.
pub fn palette(sim: &Toys, k: usize, by: f32) -> Vec<M4> {
    let spec = &sim.specs[k];
    let motion = &sim.motions[spec.motion];
    let time = shown_time(&sim.clocks[k], &motion.timing, by);
    let mut hints = sim.hints[sim.base[spec.motion]..].to_vec();
    let locals = toys::pose(motion, &spec.rig, F(time), &mut hints);
    let mut out: Vec<M4> = Vec::with_capacity(locals.len());
    for (j, local) in locals.iter().enumerate() {
        let local = m4(local);
        out.push(match spec.rig.parents[j] {
            Some(p) if p < j => local.mul(out[p]),
            _ => local,
        });
    }
    out
}

/// One toy's placement, model space to world.
pub fn place(sim: &Toys, k: usize) -> M4 {
    m4(&sim.specs[k].place)
}

/// A model's box in its own space, as a centre and a half extent: the box round its finest level of
/// detail's triangles at rest. `None` for a model with none.
pub fn bound(tris: &[crate::psg::Tri]) -> Option<(V3, V3)> {
    let mut points = tris.iter().flat_map(|t| t.v.iter().map(|v| v.pos));
    let first = points.next()?;
    let (lo, hi) = points.fold((first, first), |(lo, hi), p| {
        (v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z)), v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z)))
    });
    Some((lo.add(hi).scale(0.5), hi.sub(lo).scale(0.5)))
}

/// Whether a view drew toy `k` (V76): its model's box ([`bound`]), at its place, not wholly outside
/// any one side of the camera's clip volume for a view `width` by `height` — the test the original's
/// draw makes of the toy's root node, whose answer its next step reads ([`Toys::seen`]).
pub fn drawn(sim: &Toys, k: usize, (centre, half): (V3, V3), camera: &Camera, width: usize, height: usize) -> bool {
    let to_clip = place(sim, k).mul(camera.view_projection(width.max(1), height.max(1)));
    let corners: Vec<[f32; 4]> = (0..8)
        .map(|c| {
            let side = |bit: usize, h: f32| if c & bit != 0 { h } else { -h };
            to_clip.clip(v3(centre.x + side(1, half.x), centre.y + side(2, half.y), centre.z + side(4, half.z)))
        })
        .collect();
    let outside = |out: &dyn Fn(&[f32; 4]) -> bool| corners.iter().all(out);
    !(outside(&|p| p[0] < -p[3])
        || outside(&|p| p[0] > p[3])
        || outside(&|p| p[1] < -p[3])
        || outside(&|p| p[1] > p[3])
        || outside(&|p| p[2] < 0.0)
        || outside(&|p| p[2] > p[3]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hierarchys_bones_count_the_root_and_go_a_level_at_a_time() {
        // root ─ a ─ c
        //      └ b ─ d
        //          └ e
        let parents = [None, None, Some(0), Some(1), Some(1)];
        assert_eq!(flatten(&parents), vec![None, Some(0), Some(1), Some(2), Some(3), Some(4)]);
        // A child listed before its parent's sibling's children: a's children come first, then a's
        // grandchildren, then b's.
        let parents = [None, Some(0), None, Some(1), Some(0)];
        assert_eq!(flatten(&parents), vec![None, Some(0), Some(2), Some(1), Some(4), Some(3)]);
    }

    #[test]
    fn the_count_decides_and_the_keys_default_as_the_reader_has_them() {
        let text = b"[AnimatedToys]\nNumberOfAnimatedToys=1\n\n[AnimatedToy1]\nName = gondola_01\nMCFName = gondola.mcf\n\
                     InitialMotion = gondola_01\nHasCollision = true\n\n[AnimatedToy2]\nMCFName=balloon_a.mcf\n";
        let listed = toys(text);
        assert_eq!(listed.len(), 1, "the count, not the sections");
        let t = &listed[0];
        assert_eq!((t.name.as_str(), t.mcf.as_str(), t.motion.as_str(), t.stem()), ("gondola_01", "gondola.mcf", "gondola_01", "gondola"));
        assert!(t.collision);
        assert!(t.unseen && t.search, "AnimateWhenNotVisible and UpdateSearch are on unless written off");
        assert_eq!(t.start, F::ZERO);
        let rows = t.place.0.map(|r| [r.x(), r.y(), r.z(), r.w()]);
        assert_eq!(rows, [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]);
        let free11 = b"[animatedtoys]\nNumberOfanimatedtoys=0\n\n[AnimatedToy1]\nMCFName=balloon_a.mcf\n";
        assert!(toys(free11).is_empty(), "free11 writes its balloons under a count of 0");
    }

    #[test]
    fn a_toy_is_drawn_while_any_of_its_box_is_in_the_view() {
        let motion = Motion { timing: game_sim::motion::Clip { duration: F(1.0), looping: true, ..Default::default() }, rotation: Vec::new(), position: Vec::new() };
        let at = |x: f32, z: f32| Spec {
            motion: 0,
            rig: Rig { parents: Vec::new(), rest: Vec::new(), map: Vec::new() },
            place: math::basis(Vf::new(x, 0.0, z, 1.0), Vf::new(0.0, 0.0, 1.0, 1.0), Vf::new(0.0, 1.0, 0.0, 1.0), true, true),
            start: F::ZERO,
            mesh: None,
            parts: Vec::new(),
            unseen: false,
            search: true,
        };
        // Ahead, behind, past the far plane, and off to the side with one corner of its box still in.
        let sim = Toys::new(vec![motion], vec![at(0.0, 50.0), at(0.0, -50.0), at(0.0, 500.0), at(60.0, 50.0)]);
        let camera = Camera { eye: v3(0.0, 0.0, 0.0), at: v3(0.0, 0.0, 1.0), fov: 1.2, near: 0.5, far: 300.0 };
        let box_ = (v3(0.0, 0.0, 0.0), v3(5.0, 5.0, 5.0));
        let seen: Vec<bool> = (0..4).map(|k| drawn(&sim, k, box_, &camera, 640, 448)).collect();
        assert_eq!(seen, [true, false, false, false]);
        assert!(drawn(&sim, 3, (v3(0.0, 0.0, 0.0), v3(40.0, 5.0, 5.0)), &camera, 640, 448), "a box reaching into the view");
    }

    #[test]
    fn the_picture_runs_the_last_step_and_wraps_a_loop() {
        let timing = game_sim::motion::Clip { duration: F(1.0), looping: true, ..Default::default() };
        let clock = toys::Clock { time: F(0.1), prev: F(0.9), finished: false };
        assert!((shown_time(&clock, &timing, 0.5) - 0.0).abs() < 1e-6);
        assert!((shown_time(&clock, &timing, 1.0) - 0.1).abs() < 1e-6);
        assert!((shown_time(&clock, &timing, 0.0) - 0.9).abs() < 1e-6);
        let done = toys::Clock { time: F::ZERO, prev: F(0.95), finished: true };
        assert_eq!(shown_time(&done, &game_sim::motion::Clip { duration: F(1.0), ..Default::default() }, 0.3), 1.0);
    }
}
