//! S49 — what the sweep probe's port leaves out, and why that is the whole of the original on this disc.
//!
//! The sweep probe (crates/game-sim/src/sweep_probe.rs) has three paths S47 did not port: the pass over a
//! search's terrain cell grid (its class slot `+0xb0`), the finite plane (a candidate of kind 8) and the
//! smoothed-normal sweep (the probe's `+0x164`). None of them can run in this game as shipped:
//!
//! - a search only gets a grid from the level, and the level only loads one for a `.scn` with a
//!   `[SoultreeTerrain]` section naming a `.tcg` — no `.scn` on the disc has one, and no archive holds a
//!   `.tcg` ([`no_level_has_a_terrain_grid`]);
//! - a finite plane is only made by the collision-file loader, for a `.col` whose type word is 8 — none
//!   of the disc's `.col` files is ([`no_collision_file_is_a_finite_plane`]);
//! - the smoothed sweep is switched by a flag the two probes' pair tests copy from their own fields,
//!   which their constructors zero and nothing writes again (research/oracle.txt, S49); in every save
//!   state here those fields, the flag, the level's grid and the finite planes are all absent
//!   ([`no_state_has_a_grid_a_finite_plane_or_a_smoothing_probe`]).
//!
//! The first two read the disc and the third the save states' RAM images; each skips, saying what is
//! missing, when its input is not there.

use game_sim::archive::{Psr, Res};
use ps2_oracle::{Kit, addr, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// A collidable's class vtable and its shape kind.
const CLASS: u32 = 0x38;
const KIND: u32 = 0x68;
/// The sweep probe's and the ray probe's smoothing fields, which their pair tests copy into the flag.
const SWEEP_SMOOTH: u32 = 0x164;
const RAY_SMOOTH: u32 = 0x174;
/// The level's terrain grid, which every search that has one copies into its own `+0x38`.
const LEVEL_GRID: u32 = 0x42c;

/// The save states' RAM images, `./kit state ram` of the recordings' `.p2s` files.
const STATES: [&str; 2] = ["biplane.p2m2_SaveState.eeMemory.bin", "first_recording.p2m2_SaveState.eeMemory.bin"];

fn kit() -> Option<Kit> {
    match Kit::load() {
        Ok(k) => Some(k),
        Err(why) => {
            skip(&why);
            None
        }
    }
}

/// Every `.res` archive in `MX.PSR`, opened, with its file bytes.
fn archives(kit: &Kit) -> Option<Vec<(String, Vec<u8>, Res)>> {
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut f) = File::open(&at) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", at.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let mut out = Vec::new();
    for e in psr.entries.iter().filter(|e| e.name.to_lowercase().ends_with(".res")) {
        f.seek(SeekFrom::Start(e.at)).expect("seek");
        let mut bytes = vec![0u8; e.size as usize];
        f.read_exact(&mut bytes).expect("read");
        // The audio archive has no block table; it holds no collision or scene files.
        if let Ok(res) = Res::parse(&bytes) {
            out.push((e.name.clone(), bytes, res));
        }
    }
    assert!(out.len() > 100, "only {} archives opened", out.len());
    Some(out)
}

fn has(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w.eq_ignore_ascii_case(needle))
}

#[test]
fn no_level_has_a_terrain_grid() {
    let Some(kit) = kit() else { return };
    let Some(archives) = archives(&kit) else { return };
    let mut scenes = 0;
    for (name, bytes, res) in &archives {
        for e in &res.entries {
            let member = e.name.to_lowercase();
            assert!(!member.ends_with(".tcg"), "{name} holds a terrain grid, {}", e.name);
            if member.ends_with(".scn") {
                let scn = res.read(bytes, e).unwrap_or_else(|why| panic!("{name} {}: {why}", e.name));
                assert!(!has(&scn, b"SoultreeTerrain"), "{name} {} has a terrain grid section", e.name);
                scenes += 1;
            }
        }
    }
    assert!(scenes > 50, "only {scenes} scenes read");
}

#[test]
fn no_collision_file_is_a_finite_plane() {
    let Some(kit) = kit() else { return };
    let Some(archives) = archives(&kit) else { return };
    let word = |b: &[u8], i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
    let mut types = std::collections::BTreeMap::<(u32, u32), usize>::new();
    for (name, bytes, res) in &archives {
        for e in res.entries.iter().filter(|e| e.name.to_lowercase().ends_with(".col")) {
            let col = res.read(bytes, e).unwrap_or_else(|why| panic!("{name} {}: {why}", e.name));
            // The loader's header: 1234, "col", a version of 1 or 2, then the type.
            assert!(col.len() >= 16 && word(&col, 0) == 1234 && &col[4..8] == b"col\0", "{name} {}", e.name);
            let (version, kind) = (word(&col, 8), word(&col, 12));
            assert!(version == 1 || version == 2, "{name} {}: version {version}", e.name);
            assert_ne!(kind, 8, "{name} {} is a finite plane", e.name);
            *types.entry((version, kind)).or_default() += 1;
        }
    }
    eprintln!("collision files by (version, type): {types:?}");
    assert!(types.values().sum::<usize>() > 4000, "{types:?}");
}

#[test]
fn no_state_has_a_grid_a_finite_plane_or_a_smoothing_probe() {
    let Some(kit) = kit() else { return };
    let (sweep, ray, plane) =
        (addr("sweep_probe_class").unwrap(), addr("ray_probe_class").unwrap(), addr("finite_plane_class").unwrap());
    let (level, flag) = (addr("level_object").unwrap(), addr("probe_smooth_flag").unwrap());
    let mut read = 0;
    for state in STATES {
        let path = kit.ctx("states").join(state);
        let Ok(ram) = std::fs::read(&path) else {
            skip(&format!("{} is not there: `./kit state ram` of its .p2s", path.display()));
            continue;
        };
        let w = |a: u32| u32::from_le_bytes(ram[a as usize..a as usize + 4].try_into().unwrap());
        let (mut sweeps, mut rays) = (0, 0);
        for a in (0..ram.len() as u32 - 0x200).step_by(4) {
            let class = w(a + CLASS);
            assert_ne!(class, plane, "{state}: a finite plane at {a:#x}");
            if class == sweep && w(a + KIND) == 3 {
                assert_eq!(w(a + SWEEP_SMOOTH), 0, "{state}: the sweep probe at {a:#x} smooths");
                sweeps += 1;
            }
            if class == ray && w(a + KIND) == 2 {
                assert_eq!(w(a + RAY_SMOOTH), 0, "{state}: the ray probe at {a:#x} smooths");
                rays += 1;
            }
        }
        assert!(sweeps > 0 && rays > 0, "{state}: {sweeps} sweep probes, {rays} ray probes");
        assert_eq!(w(flag), 0, "{state}: the smoothing flag is set");
        assert_ne!(w(level), 0, "{state}: no level");
        assert_eq!(w(w(level) + LEVEL_GRID), 0, "{state}: the level has a terrain grid");
        eprintln!("{state}: {sweeps} sweep probes and {rays} ray probes, none smoothing; no grid, no plane");
        read += 1;
    }
    if read == 0 {
        skip("no save state's RAM image to read");
    }
}
