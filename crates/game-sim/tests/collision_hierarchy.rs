//! Collision class 1, the hierarchy: several meshes in one `.col`, each tied to a bone of the model that
//! loads it.
//!
//! The generated files are written the way the loader reads them — a part count and a word it drops, then
//! per part a bone, a surface word from version 2 on, and an ordinary mesh body — and each must come back
//! as the parts the loader keeps: every part with vertices, in file order, each one the same mesh the mesh
//! class reads out of the same body. The disc test reads the class-1 files MX Unleashed ships.

use game_sim::archive::{Psr, Res};
use game_sim::track::{BonePart, Collision, Hierarchy, Mesh, Node, Points, Triangle, V3};
use ps2_float::F;
use ps2_oracle::{Kit, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// A small deterministic generator, so a failure names a case that can be run again.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }

    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }

    /// A float the reader takes as it lies: whole eighths, never a denormal.
    fn f(&mut self) -> F {
        F((self.below(2001) as i32 - 1000) as f32 / 8.0)
    }

    fn v3(&mut self) -> V3 {
        [self.f(), self.f(), self.f()]
    }
}

/// One part as the generator lays it out, before the loader decides whether to keep it.
struct Raw {
    bone: u32,
    surface: u32,
    mesh: Mesh,
    /// The transform's fourth column, which the file stores and the loader ignores.
    column: u32,
}

fn word(out: &mut Vec<u8>, w: u32) {
    out.extend_from_slice(&w.to_le_bytes());
}

fn float(out: &mut Vec<u8>, f: F) {
    word(out, f.0.to_bits());
}

fn header(out: &mut Vec<u8>, version: u32, class: u32) {
    word(out, 1234);
    out.extend_from_slice(b"col\0");
    word(out, version);
    word(out, class);
}

fn points(out: &mut Vec<u8>, p: &Points) {
    match p {
        Points::Wide(v) => v.iter().flatten().for_each(|&f| float(out, f)),
        Points::Packed(v) => v.iter().flatten().for_each(|&s| out.extend_from_slice(&s.to_le_bytes())),
    }
}

/// A mesh body: the transform (with its fourth column), the counts, the points, the tree, the triangles.
fn body(out: &mut Vec<u8>, m: &Mesh, column: u32) {
    m.transform.iter().flatten().for_each(|&f| float(out, f));
    word(out, column);
    word(out, m.vertices.len() as u32);
    word(out, m.nodes.len() as u32);
    word(out, m.triangles.len() as u32);
    word(out, matches!(m.vertices, Points::Packed(_)) as u32);
    points(out, &m.vertices);
    match &m.normals {
        None => word(out, 0),
        Some(n) => {
            word(out, 1);
            points(out, n);
        }
    }
    for n in &m.nodes {
        float(out, n.volume);
        n.centre.iter().chain(&n.half_extent).for_each(|&f| float(out, f));
        n.children.iter().for_each(|c| out.extend_from_slice(&c.to_le_bytes()));
    }
    for t in &m.triangles {
        float(out, t.distance);
        t.normal.iter().for_each(|&f| float(out, f));
        t.vertices.iter().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
    }
}

fn hierarchy_file(version: u32, unread: u32, parts: &[Raw]) -> Vec<u8> {
    let mut out = Vec::new();
    header(&mut out, version, 1);
    word(&mut out, parts.len() as u32);
    word(&mut out, unread);
    for p in parts {
        word(&mut out, p.bone);
        if version > 1 {
            word(&mut out, p.surface);
        }
        body(&mut out, &p.mesh, p.column);
    }
    out
}

/// The same body as an ordinary class-0 file, which is how the loader reads each part.
fn mesh_file(version: u32, surface: u32, mesh: &Mesh, column: u32) -> Vec<u8> {
    let mut out = Vec::new();
    header(&mut out, version, 0);
    if version > 1 {
        word(&mut out, surface);
    }
    body(&mut out, mesh, column);
    out
}

fn random_points(g: &mut Lcg, count: usize, packed: bool) -> Points {
    if packed {
        Points::Packed((0..count).map(|_| [g.next() as i16, g.next() as i16, g.next() as i16]).collect())
    } else {
        Points::Wide((0..count).map(|_| g.v3()).collect())
    }
}

fn random_part(g: &mut Lcg, version: u32) -> Raw {
    // One part in four has no vertices, which the loader reads past and drops.
    let vertex_count = if g.below(4) == 0 { 0 } else { 1 + g.below(9) as usize };
    let packed = g.below(2) == 1;
    let surface = g.next();
    let mesh = Mesh {
        version,
        surface: (version > 1).then_some(surface),
        transform: [g.v3(), g.v3(), g.v3(), g.v3()],
        vertices: random_points(g, vertex_count, packed),
        normals: (g.below(2) == 1).then(|| random_points(g, vertex_count, packed)),
        nodes: (0..g.below(5))
            .map(|_| Node {
                volume: g.f(),
                centre: g.v3(),
                half_extent: g.v3(),
                children: [g.next() as u16, g.next() as u16],
            })
            .collect(),
        triangles: (0..g.below(5))
            .map(|_| Triangle {
                distance: g.f(),
                normal: g.v3(),
                vertices: [g.next() as u16, g.next() as u16, g.next() as u16],
            })
            .collect(),
    };
    Raw { bone: g.below(24), surface, mesh, column: g.next() }
}

/// The loader's shape: every part with vertices, in file order, with its bone and its own surface word.
fn kept(parts: &[Raw]) -> Vec<BonePart> {
    parts
        .iter()
        .filter(|p| !p.mesh.vertices.is_empty())
        .map(|p| BonePart { bone: p.bone, mesh: p.mesh.clone() })
        .collect()
}

#[test]
fn a_generated_hierarchy_parses_to_the_parts_the_loader_keeps() {
    let mut g = Lcg(0x6d64_3500);
    let mut dropped = 0;
    let mut kept_parts = 0;
    for case in 0..10_000 {
        let version = 1 + g.below(2);
        let unread = if g.below(2) == 0 { 0 } else { g.next() };
        let parts: Vec<Raw> = (0..g.below(6)).map(|_| random_part(&mut g, version)).collect();
        let bytes = hierarchy_file(version, unread, &parts);

        let want = Hierarchy { version, unread, parts: kept(&parts) };
        dropped += parts.len() - want.parts.len();
        kept_parts += want.parts.len();
        let got = Hierarchy::parse(&bytes).unwrap_or_else(|e| panic!("case {case}: {e}"));
        assert_eq!(got, want, "case {case}");
        assert_eq!(Collision::parse(&bytes), Ok(Collision::Hierarchy(want)), "case {case}: the dispatch");

        // Each kept part is exactly what the mesh class reads out of the same body.
        for (p, part) in parts.iter().filter(|p| !p.mesh.vertices.is_empty()).zip(&got.parts) {
            let alone = Mesh::parse(&mesh_file(version, p.surface, &p.mesh, p.column)).expect("the mesh alone");
            assert_eq!(part.mesh, alone, "case {case}: bone {}", p.bone);
        }

        // Every byte is accounted for: one short and one over both fail.
        assert!(Hierarchy::parse(&bytes[..bytes.len() - 1]).is_err(), "case {case}: truncated");
        let mut long = bytes.clone();
        long.push(0);
        assert!(Hierarchy::parse(&long).is_err(), "case {case}: a byte over");
    }
    assert!(dropped > 1000 && kept_parts > 10_000, "dropped {dropped}, kept {kept_parts}");
}

#[test]
fn a_hierarchy_is_its_own_class() {
    let mut g = Lcg(1);
    let parts = vec![random_part(&mut g, 2), random_part(&mut g, 2)];
    let bytes = hierarchy_file(2, 0, &parts);
    assert!(Mesh::parse(&bytes).unwrap_err().contains("Hierarchy"), "the mesh class refuses it");
    let mesh = mesh_file(2, 7, &parts[0].mesh, 0);
    assert!(Hierarchy::parse(&mesh).is_err(), "and it refuses a mesh");
}

/// A one-part version-2 file, the shape of MX vs. ATV's windmill blade: bone 1, surface 0.
#[test]
fn a_single_bladed_hierarchy_keeps_its_one_part() {
    let mut g = Lcg(2);
    let mut part = random_part(&mut g, 2);
    part.bone = 1;
    part.surface = 0;
    part.mesh.surface = Some(0);
    part.mesh.vertices = random_points(&mut g, 40, false);
    part.mesh.normals = Some(random_points(&mut g, 40, false));
    let bytes = hierarchy_file(2, 0, std::slice::from_ref(&part));
    let h = Hierarchy::parse(&bytes).expect("the blade");
    assert_eq!(h.parts.len(), 1);
    assert_eq!(h.parts[0].bone, 1);
    assert_eq!(h.parts[0].mesh, part.mesh);
}

/// The class-1 files on MX Unleashed's own disc, in the archives that carry them: every one parses to its
/// last byte, with the parts the probe in the MD5 journal counted.
#[test]
fn the_discs_hierarchies_parse() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => return skip(&why),
    };
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut f) = File::open(&at) else {
        return skip(&format!("no {} — `./kit extract MX.PSR` writes it", at.display()));
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");

    let want: &[(&str, &str, usize)] = &[
        ("Tracks\\free09.res", "col\\utilityshack1.col", 2),
        ("Tracks\\free10.res", "tracks\\free10\\anim\\shovel.col", 10),
        ("Tracks\\nat01.res", "col\\watertower.col", 2),
        ("Tracks\\nat01.res", "col\\mainhouse.col", 3),
        ("Tracks\\nat07.res", "tracks\\nat07\\anim\\mill.col", 2),
        ("Tracks\\nat11.res", "tracks\\nat11\\anim\\gondola.col", 2),
        ("Tracks\\free06_2.res", "col\\machineracegate.col", 2),
    ];
    for &(archive, member, count) in want {
        let e = psr.find(archive).unwrap_or_else(|| panic!("{archive} is not in MX.PSR"));
        f.seek(SeekFrom::Start(e.at)).expect("seek");
        let mut file = vec![0u8; e.size as usize];
        f.read_exact(&mut file).expect("read");
        let res = Res::parse(&file).unwrap_or_else(|e| panic!("{archive}: {e}"));
        let entry = res.find(member).unwrap_or_else(|| panic!("{member} is not in {archive}"));
        let bytes = res.read(&file, entry).expect("the member");
        let Ok(Collision::Hierarchy(h)) = Collision::parse(&bytes) else {
            panic!("{member}: {:?}", Collision::parse(&bytes).map(|_| "not a hierarchy"));
        };
        assert_eq!((h.version, h.unread, h.parts.len()), (2, 0, count), "{member}");
        for p in &h.parts {
            assert!(p.mesh.surface.is_some() && !p.mesh.triangles.is_empty(), "{member}: bone {}", p.bone);
        }
    }
}
