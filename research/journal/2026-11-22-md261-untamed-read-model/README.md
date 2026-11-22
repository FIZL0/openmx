# MD261 — untamed read model

2026-11-22. Task: `./kit plan MD261`.

Untamed quads loaded through the model reader now take Untamed's part reader box (MD250's): a new
`quad::Load::read_untamed_model`, used by the shell's quad on Untamed's step. A new route.rs test proves the
untamed_atv race quad's record inverse inertia equals MD147's spawn record `+0x70` bit for bit, and that
SLUS-21104's box misses it by up to 35 ulps. Details in 1-untamed-read-model-FINAL.md.
