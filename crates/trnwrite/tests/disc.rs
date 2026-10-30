//! Every terrain on the MX Unleashed disc: read with openmx → decoded → rewritten → read again gives
//! the same cells (heights, normals, surfaces) at every sample, the same decoded terrain, and the same
//! leaf block bytes.
//!
//! `./kit check -- -p trnwrite --test disc -- --ignored`. The disc is the one the shell is configured
//! with (kit.toml's `[game] iso`, game-render's `Disc::open_configured`). openmx-mods' copy of this
//! test also read MX vs. ATV's and Untamed's discs; those are its to keep (P232).

use game_render::disc::Disc;
use game_sim::track::{BLOCK_BYTES, Terrain};
use trnwrite::{header, openmx_view, read, write};

/// Every `.trn` inside the disc's track archives, by name.
fn terrains() -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut disc = Disc::open_configured()?;
    let archives: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| n.to_ascii_lowercase().starts_with("tracks\\") && n.to_ascii_lowercase().ends_with(".res"))
        .collect();
    let mut out = Vec::new();
    for name in archives {
        let res = disc.res(&name)?;
        let trns: Vec<String> = res.entries().iter().map(|e| e.name.clone()).filter(|n| n.to_ascii_lowercase().ends_with(".trn")).collect();
        for t in trns {
            out.push((format!("{name}:{t}"), res.read(&t)?));
        }
    }
    Ok(out)
}

/// Every sample's cell, at steps 1 and 2, compared between two readings, over the root cells that hold
/// anything (plus one sample past them, which has to be a miss in both).
fn same_cells(a: &Terrain, b: &Terrain, extent: (u32, u32)) -> Result<usize, String> {
    let mut n = 0;
    for step in [1, 2] {
        for sz in 0..=extent.1 {
            for sx in 0..=extent.0 {
                let (qa, qb) = (a.cell(sx, sz, step), b.cell(sx, sz, step));
                if qa != qb {
                    return Err(format!("cell {sx},{sz} step {step}: {qa:?} against {qb:?}"));
                }
                n += usize::from(qa.is_some());
            }
        }
    }
    Ok(n)
}

#[test]
#[ignore = "needs the disc images"]
fn every_disc_terrain_round_trips() {
    let mut total = 0;
    {
        let game = "mxu";
        let list = match terrains() {
            Ok(l) => l,
            Err(e) => panic!("the configured disc: {e}"),
        };
        assert!(!list.is_empty(), "{game}: no terrains found");
        let (mut blocks_in, mut blocks_out) = (0, 0);
        for (name, src) in &list {
            let model = read(src).unwrap_or_else(|e| panic!("{name}: {e}"));
            let (out, w) = write(&model).unwrap_or_else(|e| panic!("{name}: {e}"));
            let again = read(&out).unwrap_or_else(|e| panic!("{name} rewritten: {e}"));
            assert!(
                again == model,
                "{name}: the rewritten terrain decodes differently"
            );

            let (va, vb) = (openmx_view(src).unwrap(), openmx_view(&out).unwrap());
            let (ta, tb) = (Terrain::parse(&va).unwrap(), Terrain::parse(&vb).unwrap());
            let (cw, cd) = model.root_cells();
            let cells = same_cells(&ta, &tb, (cw * 256, cd * 256))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(cells > 0, "{name}: no cells");

            // Each leaf's block, byte for byte (less the three unused height columns of a row, zero on
            // every disc block, which the rewrite writes as zero too).
            let (ha, hb) = (header(src).unwrap(), header(&out).unwrap());
            assert_eq!(ha.pages, hb.pages, "{name}: pages");
            assert_eq!(ha.palette, hb.palette, "{name}: palette");
            assert_eq!(hb.tree_at, w.tree_at);
            let blk = |b: &[u8], h: &trnwrite::Header, i: u16| {
                let at = h.blocks_at + usize::from(i) * BLOCK_BYTES;
                b[at..at + BLOCK_BYTES].to_vec()
            };
            for lz in 0..cd * 16 {
                for lx in 0..cw * 16 {
                    let (la, lb) = (ta.leaf(lx * 16, lz * 16), tb.leaf(lx * 16, lz * 16));
                    assert_eq!(la.is_some(), lb.is_some(), "{name}: leaf {lx},{lz}");
                    if let (Some((na, ..)), Some((nb, ..))) = (la, lb) {
                        assert_eq!(
                            blk(src, &ha, na.block),
                            blk(&out, &hb, nb.block),
                            "{name}: leaf {lx},{lz}"
                        );
                    }
                }
            }
            blocks_in += ta.blocks;
            blocks_out += w.blocks;
            total += 1;
            eprintln!(
                "{game} {name}: {} leaves, {} blocks → {}, {} pages, {cells} cells, {} → {} bytes",
                w.leaves,
                ta.blocks,
                w.blocks,
                w.pages,
                src.len(),
                out.len()
            );
        }
        eprintln!(
            "{game}: {} terrains, {blocks_in} blocks on the disc, {blocks_out} rewritten",
            list.len()
        );
    }
    eprintln!("{total} terrains round-tripped");
}
