# 1 — fx and rain triggers struck by the body rays (FINAL)

Task V103. 2026-11-02.

## What I did

- `toy::Trigger` (the `particles` and `Weather` triggers' volume) gets `shape()`, `struck_by(&Actor)` and
  `keeps(&Actor)`: the same volume as a `creature::Trigger`, so the V75 strike (`probe_strikes_box` for a
  ray probe, `segment_touches_box` for segments) and the V104 exit test (`ray_probe::group_reaches`) serve
  all three. They are one trigger class in the original; only the actions differ.
- `TrackFx::touch_among(&[Actor])` and `Rain::touch_among(&[Actor])` replace the point test. Each frame,
  per trigger: last frame's touch kept and cleared; actors in it that the exit test lets go taken out; then
  for each actor that strikes it, the contact: enabled → touched; a fired `OneTimeOnly` stops; under 100
  percent it stops if touched last frame, else rolls; fires; runs its actions for an actor not already in
  it and takes it in. `touch(&[V3])` stays, as bare points (the old tests still use it).
- `main.rs` passes `creature_rays` (the body-ray probe at `ride.transform()`, or the point with no bike
  collision files) to both, as it does to the creatures.
- Rain's trigger tuple gains a fifth field, touched last frame.

## How it was checked

- `crates/openmx/tests/track_particles.rs` `a_segment_across_a_geyser_trigger_sets_it_off`: a segment across
  Geyser_a_trigger's box, both ends outside, strikes it and sets the blast off; held across it, one coin
  toss only (the rng does not move after the first frame).
- `crates/openmx/tests/weather.rs` `the_bikes_body_rays_strike_the_rain_triggers`: the disc's `bikem.col`
  probe, facing along RainBegins' look (the slab is 2.5 ft thick), at seats from −20 to +20 ft: some seat
  outside the slab has rays across it; the rain comes up for that bike; one 40 ft out does nothing.
- Mutations: `toy::Trigger::struck_by` reduced to the probe's origin point fails both new tests; a weather
  contact that ignores every strike after the first frame fails `rain_begins_and_ends`.
- `./kit check -- -p game-render --test creature --test trigger_box --test trigger_contact --test toy -p
  openmx --test track_particles --test weather`: 28 passed, with the disc present (no skips).
- Every fx and rain trigger on nat01 and nat12 has `LookVector` ⟂ `UpVector`, so `toy.rs`'s squared-off
  frame (up = look × right) is the original's basis (up and look as read, right = up × look).

## Not verified / not 1:1

- No PCSX2 run on nat01 or nat12: as V75, no save state on a nat track exists. The strike and the contact
  rest on V75's and V104's proofs, which are for the same trigger class.
- The rain's percent roll uses the rain's own generator, not the original's `rand` (presentation, as V73).
- Which rays strike while wrecked: as V75, the body rays at `ride.transform()` in every state.
- The bike's sweep and second ray probe are taken not to strike (V75's reading), not run here.

## Next

None.
