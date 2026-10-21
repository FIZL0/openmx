//! **The drawing side of the port: the disc's own models and textures, on screen.**
//!
//! `game-sim` is a pure function of (state, input) and is not allowed to touch a file or a pixel. This
//! crate is the other half: it opens the user's disc, pulls the track and the bike out of the archives,
//! turns the PS2's vertex streams into triangles and its palettes into colours, and rasterises them.
//! Nothing here has to be bit-exact — a render only has to look right — so it uses host `f32` freely.
//!
//! ```text
//! crates/game-render/src/
//!   disc.rs      ISO 9660 -> MX.PSR -> a `.res` member, read from the user's disc at runtime
//!   psg.rs       `.psg` geometry: nodes, materials, LOD groups, VIF streams -> triangle strips
//!   pst.rs       `.pst` textures: a 256-colour palette in the GS's order, plus its mip chain
//!   terrain.rs   `.trn` ground: a patched height field, its palette and its ground texture
//!   sky.rs       `.cub` skies: a cube of vertex-coloured strips around the camera
//!   splat.rs     the freeride ground's paint: masked layers, a detail map and a baked light map
//!   skin.rs      `.mot` motion clips: the channels, the quaternion blend, and the joints on a model
//!   anim.rs      the animation seam: the sim's driving on the disc's clips, bound to one skeleton
//!   biplane.rs   the biplane: its model, the prop cone its state calls for, the pilot, its emitters
//!   ini.rs       the plain-text format `.psm` and `.scn` share
//!   scene.rs     `.scn` scenes (start point, fog, lights, 1506 placed props) and `.psm` materials
//!   toy.rs       `.toy` moving scenery: the cars that drive a clip's path, free06's train among them
//!   world.rs     a track in memory: models, materials and textures matched up by name, and drawn
//!   rider.rs     the rider: its `.sub`, the `.mcf` it names, its skeleton and how it bolts on
//!   chassis.rs   where the renderer put the bike, measured against where the solver put it
//!   gpu.rs       the same world through wgpu: uploaded once, skinned and batched on the GPU
//!   raster.rs    a depth-buffered, perspective-correct, textured triangle rasteriser
//!   png.rs       a PNG writer, so a frame can be looked at
//!   mat.rs       vectors and 4x4 transforms in the files' own convention
//!   bin/first-draw.rs   the whole chain: disc in, one frame of a track out
//! ```
//!
//! The format notes are in each module's own docs; they were recovered from the bytes on the disc, so
//! where a field is still unexplained the doc says so rather than inventing a name for it.

pub mod anim;
pub mod biplane;
pub mod chassis;
pub mod disc;
#[cfg(feature = "gpu")]
pub mod gpu;
pub mod ini;
pub mod mat;
pub mod particle;
pub mod png;
pub mod psg;
pub mod pst;
pub mod raster;
pub mod rider;
pub mod scene;
pub mod skin;
pub mod sky;
pub mod splat;
pub mod tally;
pub mod terrain;
pub mod toy;
pub mod vehicle;
pub mod world;

pub use anim::{Rig, Set};
pub use disc::{Disc, Res};
pub use mat::{M4, V3, v3};
pub use psg::Model;
pub use pst::Texture;
pub use raster::Frame;
pub use rider::{Rider, Suit};
pub use scene::{Material, Scene};
// One skeleton, in `skin` where the clips are: `rider::Skeleton` is a re-export of it, so the name
// a rider's model is read into and the name a clip is bound against are the same type.
pub use skin::{Bone, Clip, Pose, Skeleton};
pub use terrain::{Terrain, draw_terrain};
pub use world::{Assets, Camera, draw_scene};
