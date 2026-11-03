# S220a1 — kiowa wreck

2026-11-02. Task: `./kit plan S220a1`.

This task ported the Kiowa's wreck, unwreck, blade throw and put-back, so that `Crash::step`'s `Crashed`
answer can be acted on. They are `game_sim::kiowa::{Wreckage, Nodes, Blade, Throw}`, plus
`Kiowa::{unwreck_controls, put_back_controls}`. Each of the six functions (base and MX wreck, the blade
throw, base and MX unwreck, the MX put-back) is called in the original on S148's live helicopter. Over
10,000 generated cases it equals the port bit for bit (tests/kiowa.rs). Nothing in openmx calls it yet.
Details in 1-kiowa-wreck-FINAL.md.
