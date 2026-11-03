# S270 — the Kiowa's throw rows from its own model tree

2026-11-03. Task: `./kit plan S270`.

The wreck throws the MX helicopter's four blades from the blade nodes' world rows and the model
root's. Until now the caller had to supply them, and the shell passed the measure matrix for every
row. The port now builds them the way the original's nodes hold them on the wreck frame:

- the root comes from the body's post;
- the rotor spindle is turned by the rotor's angle;
- the file's nodes run from the spindle down to `Blade_Node01..04`;
- the lazy rebuild runs down that chain.

This matches the original bit for bit, both on the saved frame and on 10,000 generated poses and angles.

Code: `game_sim::kiowa::{RotorTree, Throw::of, KIOWA_MODEL}`. The shell's `KiowaRide::with_model`
loads `helicopter\anim\kiowa.psg`.
Tests: `tests/kiowa.rs` `the_rotor_tree_is_the_live_helicopters` and `the_throws_rows_are_the_originals`.
Entry: [1-kiowa-throw-rows-FINAL.md](1-kiowa-throw-rows-FINAL.md).
