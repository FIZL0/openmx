# MD64b — atv tire phy

2026-11-02. Task: `./kit plan MD64b`.

MD64b ported the quad's tire reader, `atv_quad_tire_phy`, into `crates/game-sim/src/quad.rs` as
`quad::tire_phy`. It reads `[Front Tire]` and `[Rear Tire]` into the four wheels (`quad::TireSetup`,
`quad::WheelTire`, `quad::TireSurface`). Two tests in `tests/atv_quad.rs` prove it:

- `the_quads_tire_reader_is_the_originals` runs SLUS-21104's own reader in `ps2-oracle` on 10,000
  generated tire files and compares every word of the quad and the four wheels.
- `the_quad_images_wheels_are_their_phy_files` checks MD37a's six RAM images against the quad `.phy`
  files.

Details in `1-atv-tire-phy-FINAL.md`.
