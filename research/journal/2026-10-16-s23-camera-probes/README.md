# S23 — camera probes

2026-10-16. Task: `./kit plan S23`.

The follow camera asks the world two questions and S7's port answered neither, because the class S7
read answers both with nothing: its ground probe is a `return 0.0f` and its collision object is never
created. This task found the class the game actually creates for a race, ported its ground probe — the
lift that keeps the look-at out of the dirt — and the base class's post-filter push-out, put both
behind two small traits (`camera::Surface`, `camera::Obstacles`) answered from the track the shell
already has, and settled the brief's question on the way: the argument S7 called "the nearest allowed
distance" is the viewport's **near clip plane**, and nothing probes for it. Also made the
horizontal-to-vertical field-of-view conversion read the window's real aspect instead of the disc's
4:3. Fifteen oracle proofs in `tests/camera.rs` now — three of them new, covering the probe's point, the
lift's own arithmetic and the push-out in its frame — and 444 tests green; what is read rather than run
is listed in `1-camera-probes-FINAL.md`, and the biggest item there is that the rest of that derived
camera class — four more differences in its look-ahead — is still not ported (S24).
