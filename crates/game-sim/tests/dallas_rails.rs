//! S89: what free06's railway is to the collision, and which surface a wheel on it reads.
//!
//! The railway on Dallas is not rails and sleepers to the collision at all: it is a raised ballast
//! bed, sixty-two collision meshes of the scene's own (`Free06_RR_COL_*`), each carrying surface 1,
//! standing up to about a foot above a terrain that is surface 0 or 12 under it. The original's
//! wheel never reads the mesh's surface: its heightfield query writes the wheel's surface byte, from
//! the terrain alone, at the end of every pass of the ray ([`contact_search::nearest_surface`]).

use game_sim::archive::{Entry, Psr, Res};
use game_sim::contact_search::{self, Cell, Heightfield};
use game_sim::track::{Mesh, Points, Scene, Terrain};
use game_sim::world::{Ground, Place, Props, World};
use ps2_float::{F, Vf};
use ps2_oracle::{Kit, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

fn container() -> Option<File> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => {
            skip(&w);
            return None;
        }
    };
    match File::open(kit.ctx("iso").join("MX.PSR")) {
        Ok(f) => Some(f),
        Err(e) => {
            skip(&format!("no disc container: {e}"));
            None
        }
    }
}

fn member(f: &mut File, psr: &Psr, name: &str) -> Vec<u8> {
    let e = psr.find(name).unwrap();
    f.seek(SeekFrom::Start(e.at)).unwrap();
    let mut b = vec![0u8; e.size as usize];
    f.read_exact(&mut b).unwrap();
    b
}

fn by_leaf<'a>(res: &'a Res, leaf: &str) -> Option<&'a Entry> {
    let want = leaf.to_ascii_lowercase();
    res.entries.iter().find(|e| {
        let n = e.name.to_ascii_lowercase();
        n == want || n.rsplit('\\').next() == Some(want.as_str())
    })
}

#[test]
fn the_railway_is_a_raised_bed_of_its_own_meshes_over_the_terrain() {
    let Some(mut f) = container() else { return };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).unwrap();
    let psr = Psr::parse(&front).unwrap();
    let file = member(&mut f, &psr, "Tracks\\free06.res");
    let res = Res::parse(&file).unwrap();
    let scene = Scene::parse(&res.read(&file, by_leaf(&res, "tracks\\free06\\free06.scn").unwrap()).unwrap()).unwrap();
    let trn = res.read(&file, by_leaf(&res, "tracks\\free06\\free06.ps2.trn").unwrap()).unwrap();
    let terrain = Terrain::parse(&trn).unwrap();
    let ground = Ground::new(&terrain, &scene.terrain);
    let field = ground.field();

    let (mut beds, mut rise, mut under) = (0, f32::MIN, std::collections::BTreeSet::new());
    let mut place = Place { ground: Ground::new(&terrain, &scene.terrain), props: Props::default() };
    for p in &scene.collision {
        if !p.file.to_ascii_lowercase().contains("rr_col") {
            continue;
        }
        beds += 1;
        let m = Mesh::parse(&res.read(&file, by_leaf(&res, &p.file).unwrap()).unwrap()).unwrap();
        assert_eq!(m.surface, Some(1), "{}: the bed's own surface", p.file);
        let Points::Wide(v) = &m.vertices else { panic!("{}: not a v2 mesh", p.file) };
        for q in v {
            let w = Vf::new(p.position[0].0 + q[0].0, p.position[1].0 + q[1].0, p.position[2].0 + q[2].0, 1.0);
            let Some(h) = ground.at(F(w.x()), F(w.z())) else { continue };
            rise = rise.max(w.y() - h.y.0);
            let s = contact_search::nearest_surface(&field, w).unwrap();
            under.insert(s);
            // The wheel reads the terrain under the bed, never the bed's 1.
            assert_eq!(place.wheel_surface(w), Some(s));
        }
    }
    assert_eq!(beds, 62, "Free06_RR_COL_01..62");
    assert!((0.8..1.0).contains(&rise), "the bed stands {rise} above the terrain");
    assert_eq!(under, [0, 12].into(), "terrain surfaces under the bed");
}

/// A terrain of one cell whose four corners carry surfaces 1, 2, 3 and 4.
struct OneCell;

impl Heightfield for OneCell {
    fn min(&self) -> Vf {
        Vf::new(0.0, 0.0, 0.0, 1.0)
    }
    fn max(&self) -> Vf {
        Vf::new(1.0, 0.0, 1.0, 1.0)
    }
    fn per_sample(&self) -> F {
        F::ONE
    }
    fn height_scale(&self) -> F {
        F::ONE
    }
    fn coarse(&self) -> bool {
        false
    }
    fn split_one_way(&self) -> bool {
        true
    }
    fn cell(&self, ix: i32, iz: i32, _step: i32) -> Option<Cell> {
        (ix == 0 && iz == 0).then(|| Cell {
            heights: Vf::ZERO,
            normals: [Vf::new(0.0, 1.0, 0.0, 1.0); 4],
            surfaces: [1, 2, 3, 4],
        })
    }
}

#[test]
fn the_wheel_reads_the_nearest_corner_and_nothing_off_the_terrain() {
    let at = |x: f32, z: f32| contact_search::nearest_surface(&OneCell, Vf::new(x, 0.0, z, 1.0));
    assert_eq!(at(0.2, 0.3), Some(1));
    assert_eq!(at(0.8, 0.3), Some(2));
    assert_eq!(at(0.2, 0.7), Some(3));
    assert_eq!(at(0.8, 0.7), Some(4));
    // A tie goes to the first corner in the cell's order.
    assert_eq!(at(0.5, 0.5), Some(1));
    assert_eq!(at(0.5, 0.75), Some(3));
    // The query clamps below zero onto the first cell, and writes nothing where there is no cell.
    assert_eq!(at(-3.0, 0.2), Some(1));
    assert_eq!(at(1.5, 0.2), None);
}
