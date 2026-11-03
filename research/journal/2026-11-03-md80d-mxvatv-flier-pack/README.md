# MD80d — mxvatv flier pack

2026-11-03. Task: `./kit plan MD80d`.

MX vs. ATV Unleashed's biplane and Kiowa, all four skins of each over their two models, imported off
the PS2 disc into an openmx-mods machine pack (`import-atv fliers`, openmx-mods branch openmx-MD80d).
openmx's `machine_packs.rs` now reads that pack's skins and fliers, and `tests/machine_packs.rs`
proves every model, node, collision pair, texture, `.phy`, camera, sound and emitter reads. Nothing
flies it yet (MD80b/MD80c, MD80e). Details in the numbered files; the newest one's suffix says whose
move it is (FINAL · PART · READY).
