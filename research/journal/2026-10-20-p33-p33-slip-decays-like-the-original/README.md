# P33 — p33 slip decays like the original

2026-10-20. Task: `./kit plan P33`.

The port's heading came back after a slide more slowly than the original's, because the port's tire
was a stand-in. This task ports the original's tire force law (linear and parametric side curves,
the spin/lock flags, the slow-speed boost), the contact frame (with the forward turned about the
fork's raked steering axis on a steered wheel) and the axis-angle rotation. All three are bit-exact
against the oracle over 10,000 cases each (tests/tire.rs). The ride now uses the law's side force on
parametric tires, which is every bike on the disc. On race125p's slide the peak slip drops from 5.8°
to 2.8°, the heading stays inside 3° the whole time after the release, and
`it_straightens_after_a_slide` now holds the original's bound. Linear (fixture-only) tires keep the
stand-in. Details in 1-…-FINAL.md.
