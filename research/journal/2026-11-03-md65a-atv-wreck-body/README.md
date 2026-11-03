# MD65a — atv wreck body

2026-11-03. Task: `./kit plan MD65a`.

The quad's wreck body, ported into `crates/game-sim/src/quad.rs`:

- `quad::build_wreck_body` (with `quad::WreckBody`, `quad::Col`, `quad::List`): the last block of the
  quad's init. It makes the trigger probe and the `WreckATVm.col` wreck rays, and sets the ignore and
  category lists that keep the quad's own collidables off each other.
- `quad::swap_rays` (with `quad::Rays`): what the wreck does with the wreck rays. The wreck entry moves
  the quad's collision onto them, and the unwreck moves it back to the riding rays.

Three tests in `tests/atv_quad.rs` prove them against SLUS-21104. Details in `1-atv-wreck-body-FINAL.md`.
