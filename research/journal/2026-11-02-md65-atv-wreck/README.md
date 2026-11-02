# MD65 — atv wreck

2026-11-02. Task: `./kit plan MD65`.

MD65 ported two of the quad's table entries into `crates/game-sim/src/quad.rs`:

- `quad::put_back` (with `quad::PutBack`): the quad's put-back, which is the bike's put-back override
  run over four wheels.
- `quad::net_unpack` (with `quad::Remote`): a remote quad placed from its network snapshot. Its wheels
  are spun and steered, then run through the Force body's wheel rounds.

Two tests in `tests/atv_quad.rs` prove them. Each runs SLUS-21104's own function on 10,000 generated
states off MD37a's `atv-r-spawn.ee.bin`, hooks every callee and checks its arguments, then compares
the call lists and every word.

The rest of the brief went into two new tasks:

- MD65a: the wreck body on WreckATVm.col. It is built in the quad's init and needs the part reader.
- MD65b: the PC slots MD37 left unpaired.

While checking, I found that MD37 had paired the net unpack with the wrong PC slot. The real one is
vf70, so vf70 is no longer unpaired; vf104 now is. Details in `1-atv-wreck-FINAL.md`.
