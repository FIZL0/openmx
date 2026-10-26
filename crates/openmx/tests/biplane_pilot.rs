//! V39: the biplane has its pilot in the shell. The shell starts free06 in the plane
//! (`--vehicle biplane`), headless, parked, and says under `--trace-picture` what each drawn frame
//! put in the seat: the rider's own body, the root and the middle of what was drawn of it in the
//! plane's own space — the software path's skinned triangles, the GPU's posed nodes. Both
//! renderers draw it, at the root the original seats it at (`game_render::biplane::PILOT_SEAT`,
//! read off the biplane recording's states), and the body is in the cockpit, not off out of sight.

use std::path::Path;
use std::process::Command;

/// The root of the pilot's node tree in the plane's space, as the original has it.
const SEAT: [f32; 3] = [0.0, 3.5, -3.0];

/// The shell on free06 in the plane, headless, a few frames: what it said.
fn ride(renderer: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", "free06", "--size", "320x224", "--vehicle", "biplane", "--overlay", "none"])
        .args(["--renderer", renderer, "--trace-picture", "--frames", "4"])
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    said
}

/// A `pilot: seat P RENDERER MODEL root X Y Z middle X Y Z of N` line's renderer, model, root,
/// middle and count.
fn pilot(line: &str) -> (String, String, [f32; 3], [f32; 3], usize) {
    let w: Vec<&str> = line.split_whitespace().collect();
    assert_eq!((w.len(), w[5], w[9], w[13]), (15, "root", "middle", "of"), "{line}");
    let n = |i: usize| -> f32 { w[i].parse().unwrap_or_else(|_| panic!("not a number at {i}: {line}")) };
    (w[3].to_string(), w[4].to_string(), [n(6), n(7), n(8)], [n(10), n(11), n(12)], w[14].parse().unwrap())
}

#[test]
fn the_plane_is_drawn_with_its_pilot_in_the_seat() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    for renderer in ["gpu", "software"] {
        let said = ride(renderer);
        let lines: Vec<&str> = said.lines().filter(|l| l.starts_with("pilot: seat 0 ")).collect();
        assert!(!lines.is_empty(), "{renderer}: no frame drew a pilot:\n{said}");
        for line in lines {
            let (drawn_by, model, root, middle, count) = pilot(line);
            assert_eq!(drawn_by, renderer, "{line}");
            assert!(!model.is_empty() && count > 0, "{renderer}: nothing of the pilot was drawn: {line}");
            for i in 0..3 {
                assert!((root[i] - SEAT[i]).abs() < 1e-3, "{renderer}: the pilot's root is not in the seat: {line}");
            }
            // In the cockpit: on the plane's centre line, and within a body's reach of the seat.
            let off = [middle[0] - SEAT[0], middle[1] - SEAT[1], middle[2] - SEAT[2]];
            let far = (off[0] * off[0] + off[1] * off[1] + off[2] * off[2]).sqrt();
            assert!(middle[0].abs() < 0.5 && far < 3.0, "{renderer}: the pilot's body is {far:.2} ft from the seat: {line}");
        }
    }
}
