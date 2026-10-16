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
//!   skin.rs      `.mot` motion clips: the channels, the quaternion blend, and the joints on a model
//!   ini.rs       the plain-text format `.psm` and `.scn` share
//!   scene.rs     `.scn` scenes (start point, fog, lights, 1506 placed props) and `.psm` materials
//!   world.rs     a track in memory: models, materials and textures matched up by name, and drawn
//!   raster.rs    a depth-buffered, perspective-correct, textured triangle rasteriser
//!   png.rs       a PNG writer, so a frame can be looked at
//!   mat.rs       vectors and 4x4 transforms in the files' own convention
//!   bin/first-draw.rs   the whole chain: disc in, one frame of a track out
//! ```
//!
//! The format notes are in each module's own docs; they were recovered from the bytes on the disc, so
//! where a field is still unexplained the doc says so rather than inventing a name for it.

pub mod disc;
pub mod ini;
pub mod mat;
pub mod png;
pub mod psg;
pub mod pst;
pub mod raster;
pub mod scene;
pub mod skin;
pub mod terrain;
pub mod world;

pub use disc::{Disc, Res};
pub use mat::{M4, V3, v3};
pub use psg::Model;
pub use pst::Texture;
pub use raster::Frame;
pub use scene::{Material, Scene};
pub use skin::{Clip, Pose, Skeleton};
pub use terrain::{Terrain, draw_terrain};
pub use world::{Assets, Camera, draw_scene};
