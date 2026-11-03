# MD75 — the quad's physics frame

2026-11-02. Task: `./kit plan MD75`.

This is part 1 of MD75. SLUS-21104's rigid body (its integrate and its end of substep) is ported as
`atv_body::Body`, and the physics world's frame over the quad is `quad::frame`. In `tests/atv_quad.rs`,
two new oracles run against the original's own code off MD37a's `atv-r-spawn.ee.bin`:

- the body step, over 10,000 generated bodies;
- the world step on a world of the quad alone, over 3,000 states, with every Moto call hooked.

Still to do: the Moto that runs the bike steps in place of each `Call`, the steps not yet ported, and the
brief's whole-frame proof. They are filed as MD75 subtasks. Details are in the numbered file.

MD75a (`2-moto-steps.md`): `quad::Steps` runs the ported bike steps (input limiter, clutch, preload,
backflip, airborne control, and atv_wheel's suspension/tire forces/wheel integrate) on the quad. Ten
oracles prove it, each with that call unhooked so the original's own code runs, every region equal.
The rear-brake gap in `step::input_limiter` is filed as B17.
