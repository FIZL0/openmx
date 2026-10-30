//! The `.scn` a generated terrain goes with: CRLF INI text with every key the standard marks *must*
//! (`standard/OPENMX-TRACK-STANDARD.md` §2a), in the spelling and number format of MXU's own scenes.
//! A map taken off a disc keeps its own scene byte for byte; this is only for maps that have none.

use crate::Trn;

/// What a generated scene says.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneText {
    /// `[SceneInfo] SceneName`.
    pub name: String,
    /// `[Environment] TerrainFile`, as the scenes write it: without the platform tag (`free06.trn`);
    /// the lookup inserts `.ps2`.
    pub terrain_file: String,
    pub start: [f32; 3],
    pub direction: [f32; 3],
    /// Root cells across and along: the map edge is width × 256 × scale, as the game's position query has it.
    pub width: u32,
    pub breadth: u32,
    /// Feet a sample, equal to the terrain's own scale (the edge and the radar use it).
    pub scale: f32,
    pub border_width: f32,
    pub min_division_size: f32,
    /// The `.trn` loader's detail tiling; 8 is the game's default when a scene has none.
    pub detail_tiling: i32,
}

impl SceneText {
    /// The scene for a terrain written as `<stem>.ps2.trn`, starting a rider at `start` facing `direction`.
    pub fn for_terrain(t: &Trn, stem: &str, start: [f32; 3], direction: [f32; 3]) -> SceneText {
        let (width, breadth) = t.root_cells();
        SceneText {
            name: stem.to_owned(),
            terrain_file: format!("{stem}.trn"),
            start,
            direction,
            width,
            breadth,
            scale: t.scale.to_f32(),
            // free06, nat01 and sx01 all say 0 and 96.
            border_width: 0.0,
            min_division_size: 96.0,
            detail_tiling: 8,
        }
    }

    pub fn to_text(&self) -> String {
        let v = |p: [f32; 3]| format!("{:.2},{:.2},{:.2}", p[0], p[1], p[2]);
        let lines = [
            "[SceneInfo]".to_owned(),
            format!("SceneName={}", self.name),
            format!("DefaultStartPosition={}", v(self.start)),
            format!("DefaultStartDirection={}", v(self.direction)),
            String::new(),
            "[Environment]".to_owned(),
            format!("TerrainFile={}", self.terrain_file),
            format!("TerrainWidth={}", self.width),
            format!("TerrainBreadth={}", self.breadth),
            format!("TerrainWidthScale={:.6}", self.scale),
            format!("TerrainBreadthScale={:.6}", self.scale),
            format!("TerrainBorderWidth={:.6}", self.border_width),
            format!("MinDivisionSize={:.6}", self.min_division_size),
            format!("DetailMapTilingMultiplier={}", self.detail_tiling),
            String::new(),
        ];
        lines.join("\r\n")
    }
}
