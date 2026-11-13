# F58 — effects items steps

2026-11-13. Task: `./kit plan F58`.

This task ported the effects items' steps (vt `+0x14`) to Rust: the six kinds that C15's flip-1
steps, in `game_sim::handling::effects::item_step`. A 20 000-case oracle against SLUS-21104 proves
them, and four mutants each fail it. Both every-port replays pass with the steps in Rust. The item's
move turned out to reach the first emitter kind's step, which F57 already ported, so PartWalk answers
it with `emitter::step`; the flip-1 steps that draw rand now run in Rust up to the draw itself. The
seventh kind, which no C15 item steps, is F58a.
