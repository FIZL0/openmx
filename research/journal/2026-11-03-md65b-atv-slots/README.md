# MD65b — atv slots

2026-11-03. Task: `./kit plan MD65b`.

MXVehicleAtv's last unpaired PC slots (0, 33, 35, 38, 39, 67, 80, 86, 104, 111 and `vf3_at_1084`), each
paired with the SLUS-21104 function in the quad's vtable. Rows: research/oracle.txt `# MD65b` and
research/mxvatv-pc.md. Ported into `crates/game-sim/src/quad.rs`: the five that are simulation
(`remap_surfaces`, `surface_under`, `strut_travel`, `trigger_segments`, `settle_wheels`). The animation
driver (vf67) is filed as MD65c. Details in `1-atv-slots-FINAL.md`.
