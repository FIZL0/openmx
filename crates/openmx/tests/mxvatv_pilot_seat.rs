//! P316: MX vs. ATV's fliers seat their pilot where SLUS-21104 does.
//!
//! What the original does (research/journal/*-mxvatv-pilot-seat): the plane's pilot request
//! (`atv_biplane_pilot_request`) hands the body the model's matrix as its base, where MX Unleashed's
//! hands it the rigid body's centre of mass, and a pointer to the skin's model's `[Animation]`
//! `PelvisBoxPosition` as its offset; the body puts its root at the base plus the turned offset. So
//! in the model's space the root is the pelvis box itself: `game_render::biplane::mxvatv_seat`. The
//! helicopter's is the same.
//!
//! Proved on MD80a's four flier images (`context/mxvatv-ps2/md80a/{biplane,biplane3,kiowa,kiowa3}.ee.bin`,
//! each skipped where the slot does not have it; the `.p2s` are in kept-context/md80a): the request's
//! base is the model's position, its offset pointer is the pelvis box, the pelvis box is the file's
//! for the object's model (where the slot has the file), and a node of the pilot sits at
//! `mxvatv_seat` of it with the model's rotation.

use game_render::biplane::mxvatv_seat;
use ps2_oracle::{Kit, skip};

/// A flier image: its name, the player's object in it, the model word's offset and the pelvis box's.
struct Image {
    name: &'static str,
    object: usize,
    model_word: usize,
    pelvis_box: usize,
    plane: bool,
}

const IMAGES: [Image; 4] = [
    Image { name: "biplane", object: 0x014d_e810, model_word: 0x1320, pelvis_box: 0x1330, plane: true },
    Image { name: "biplane3", object: 0x01b1_3790, model_word: 0x1320, pelvis_box: 0x1330, plane: true },
    Image { name: "kiowa", object: 0x01ab_7d20, model_word: 0x1340, pelvis_box: 0x1350, plane: false },
    Image { name: "kiowa3", object: 0x014c_8f00, model_word: 0x1340, pelvis_box: 0x1350, plane: false },
];

/// The object's model matrix (rows x, y, z, position), which both fliers keep at the same place.
const MODEL: usize = 0x370;
/// The plane's pilot request: its base (the model's position, copied each tick) and its offset pointer.
const PLANE_BASE: usize = 0x8e0;
const PLANE_OFFSET: usize = 0x830;
/// Where the plane's base is copied from.
const MODEL_POSITION: usize = 0xc40;

fn u32_at(ram: &[u8], a: usize) -> u32 {
    u32::from_le_bytes(ram[a..a + 4].try_into().unwrap())
}

fn f32_at(ram: &[u8], a: usize) -> f32 {
    f32::from_bits(u32_at(ram, a))
}

fn matrix(ram: &[u8], a: usize) -> [[f64; 4]; 4] {
    std::array::from_fn(|r| std::array::from_fn(|c| f32_at(ram, a + 16 * r + 4 * c) as f64))
}

/// A world point in a matrix's own space (rows x, y, z, position; the rotation orthonormal).
fn local(m: &[[f64; 4]; 4], p: [f64; 3]) -> [f64; 3] {
    let d = [p[0] - m[3][0], p[1] - m[3][1], p[2] - m[3][2]];
    std::array::from_fn(|r| d[0] * m[r][0] + d[1] * m[r][1] + d[2] * m[r][2])
}

fn apart(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).abs()).fold(0.0, f64::max)
}

#[test]
fn the_original_seats_an_mxvatv_pilot_on_its_models_pelvis_box() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => return skip(&why),
    };
    let plane_phy = std::fs::read(kit.ctx("mxvatv-ps2/md80b").join("biplane.phy")).ok();
    let kiowa_phy = std::fs::read(kit.ctx("mxvatv-ps2/md80c").join("kiowa.phy")).ok();
    let mut read = 0;
    for image in &IMAGES {
        let name = image.name;
        let Ok(ram) = std::fs::read(kit.ctx("mxvatv-ps2/md80a").join(format!("{name}.ee{}", ".bin"))) else {
            skip(&format!("no {name}.ee.bin in this slot's context/mxvatv-ps2/md80a (eeMemory.bin of kept-context/md80a/{name}.p2s)"));
            continue;
        };
        read += 1;
        let at = |o: usize| image.object + o;
        let model_one = u32_at(&ram, at(image.model_word)) == 1;
        let pelvis = ps2_float::Vf::new(
            f32_at(&ram, at(image.pelvis_box)),
            f32_at(&ram, at(image.pelvis_box + 4)),
            f32_at(&ram, at(image.pelvis_box + 8)),
            f32_at(&ram, at(image.pelvis_box + 12)),
        );

        // The pelvis box is the file's for the model, as the port reads it.
        let file = if image.plane {
            plane_phy.as_ref().map(|b| game_sim::phy::Plane::read_mxvatv_bytes(b, model_one).plane.pelvis_box.position)
        } else {
            kiowa_phy.as_ref().map(|b| game_sim::kiowa::Phy::read_mxvatv_bytes(b, model_one).phy.pelvis_box_position)
        };
        match file {
            Some(file) => assert_eq!(file, pelvis, "{name}: the model's PelvisBoxPosition"),
            None => skip(&format!("{name}: no MX vs. ATV .phy in md80b/md80c to read the pelvis box from")),
        }

        let model = matrix(&ram, at(MODEL));
        if image.plane {
            // The request's base is the model's position, not the body's centre of mass; its offset
            // is the pelvis box.
            let base: [f32; 4] = std::array::from_fn(|i| f32_at(&ram, at(PLANE_BASE + 4 * i)));
            let position: [f32; 4] = std::array::from_fn(|i| f32_at(&ram, at(MODEL_POSITION + 4 * i)));
            assert_eq!(base, position, "{name}: the request's base");
            assert_eq!(position.map(f64::from)[..3], model[3][..3], "{name}: the base is the model's position");
            assert_eq!(u32_at(&ram, at(PLANE_OFFSET)) as usize & 0x01ff_ffff, at(image.pelvis_box), "{name}: the offset pointer");
        }

        // A node with the model's rotation at the seat, in the model's space.
        let seat = mxvatv_seat(pelvis);
        let seat = [seat.x as f64, seat.y as f64, seat.z as f64];
        let mut roots = 0;
        for a in (0..ram.len() - 64).step_by(16) {
            if f32_at(&ram, a + 60) != 1.0 || [12, 28, 44].iter().any(|o| f32_at(&ram, a + o) != 0.0) {
                continue;
            }
            let m = matrix(&ram, a);
            let p = local(&model, [m[3][0], m[3][1], m[3][2]]);
            let turned = (0..3).flat_map(|r| (0..3).map(move |c| (r, c))).map(|(r, c)| (m[r][c] - model[r][c]).abs()).fold(0.0, f64::max);
            if turned < 1e-5 && apart(p, seat) < 1e-3 {
                roots += 1;
            }
        }
        assert!(roots > 0, "{name}: no node at {seat:?} with the model's rotation");
        println!("{name}: model {}, the root at {seat:?} in {roots} matrices", if model_one { "one" } else { "two" });
    }
    if read == 0 {
        skip("no MX vs. ATV flier image in this slot");
    }
}

/// The shell on free06 in MX vs. ATV's plane of skin `skin`, headless, a few frames, under
/// `--trace-picture`: the roots its `pilot: seat 0` lines say, in the plane's space. `None` with no
/// disc or no fliers pack in this checkout's `mods/machines/`.
fn drawn_roots(skin: &str) -> Option<Vec<[f32; 3]>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    if let Err(e) = game_render::disc::Disc::open_configured() {
        skip(&format!("{e}"));
        return None;
    }
    if !root.join("mods/machines/mxvatv_fliers/mod.json").is_file() {
        skip("no mods/machines/mxvatv_fliers (openmx-mods' import-atv fliers makes it)");
        return None;
    }
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", "free06", "--size", "320x224", "--vehicle", "biplane", "--overlay", "none"])
        .args(["--renderer", "software", "--trace-picture", "--frames", "3", "--mods", "mods", "--fliers", "mxvatv", "--flier-skin", skin])
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    let roots: Vec<[f32; 3]> = said
        .lines()
        .filter(|l| l.starts_with("pilot: seat 0 "))
        .map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            assert_eq!(w[5], "root", "{l}");
            std::array::from_fn(|i| w[6 + i].parse().unwrap_or_else(|_| panic!("not a number: {l}")))
        })
        .collect();
    assert!(!roots.is_empty(), "skin {skin}: no frame drew a pilot:\n{said}");
    Some(roots)
}

#[test]
fn the_shell_seats_each_models_pilot_on_its_pelvis_box() {
    // Skin 1 is model one, skin 3 model two (the Spitfire): biplane.phy's ModelOne/ModelTwo
    // PelvisBoxPosition.
    for (skin, seat) in [("1", [0.0, 3.2, -3.0]), ("3", [0.0, 4.4, 0.7])] {
        let Some(roots) = drawn_roots(skin) else { return };
        for root in roots {
            assert!((0..3).all(|i| (root[i] - seat[i]).abs() < 1e-3), "skin {skin}: the pilot's root {root:?} is not at {seat:?}");
        }
    }
}
