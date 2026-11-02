# MD64a — atv update tail

2026-11-02. Task: `./kit plan MD64a`.

MD64a ported the quad's once-a-frame update, `atv_quad_update`, into `crates/game-sim/src/quad.rs` as
`quad::update` with its own state, `quad::Tail`. That is everything after the bike's vehicle update: the
hazard surfaces, the rumble, the wheelie and stoppie clocks, the ride stats and the player's countdown.
It is proved bit-exact by `the_quads_update_is_the_originals` in `tests/atv_quad.rs`, a 10,000-state oracle
off MD37a's `atv-r-spawn.ee.bin`. The bike's update and the other virtuals the tail calls are not ported.
They are handed out as `quad::Call`s, and the oracle checks their order and arguments. Details in
`1-atv-update-tail-FINAL.md`.
