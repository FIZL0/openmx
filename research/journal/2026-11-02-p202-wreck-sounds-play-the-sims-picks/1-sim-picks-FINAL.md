# 1 — the sim's picks in the shell (FINAL)

Task P202. 2026-11-02.

## What I did

- `crates/openmx/src/audio.rs`:
  - `WreckEmitters` is a copy of `Rider::wreck_sounds` and `Rider::wreck_clocks` after a tick, plus whether
    the rider has a ragdoll kit (only the body actor picks the voice). `SeatFrame::emitters` carries it;
    `SeatSound` keeps the last one.
  - `WreckEmitters::picked(was)` reads the picks between two frames. A pick sets the emitter's clock to
    clip + 0.4 s tail and only happens once the clock has run out, so a pick is exactly a clock that rose
    (a tick only runs the clock down, and dt is far under the tail). The index is the list's `last`; the
    voice's list is `[Fall]` when `fall.last` moved, else `[IMPACT]`. Reading the state rather than a
    per-tick event also works online, where a drawn frame can sit a few re-run ticks after the last
    (the same idea as P184's `Said`). No sim code or hashed state changed.
  - `Sound::frame` plays those picks: the voice at the bike on the wreck's entry frame, on the thrown
    body after it; the clank at `wreck_level` of the entry impulse on the entry frame, of the wreck body's
    shock peak after it (`WreckClank::picked`). The shell's own gates and dice stay only for frames with
    no emitters (the synthetic test frames) and, for the voice, a rider with no ragdoll kit.
  - `List::get(i)`.
- `crates/openmx/src/main.rs` `seat_sound` hands `WreckEmitters::of(&seat.sim.rider)`.
- `crates/openmx/tests/audio.rs` `s109s_wreck_plays_the_sims_picks`: game-sim's `Clocks`/`Emitter`
  stepped over `context/capture/s173/voices.csv` (the original's gates, generator and clip lengths, as
  game-sim's `the_s109_wreck_draws_as_the_original` drives them), handed to `Sound::frame` with the
  shell's own gates held wide open. Heard: `[(3726, Fall, 2), (3792, IMPACT, 0), (3792, Wreck, 3)]` over
  seeds 0..8. With `emitters: None` the same run hears ten clips on the shell's dice, so the test
  tells the two apart.

Checked: `cargo test -p openmx --test audio` (38 passed, 1 ignored) and `--test online_wreck_sound`
(20 passed); `cargo build -p openmx`.

## Not verified / not 1:1

- The test's pick indices are the original's, written into the emitters as game-sim's S173 test does,
  because where in the tick each pick draws is not in the capture. So the test proves the shell plays
  what the emitters hold on the ticks they changed, not that a full port ride of S109 picks 2/3/0; no
  replay of S109 with the wreck emitters exists yet.
- A `[Fall]` list of one sound reads as `[IMPACT]` after its first pick (its `last` never moves).
  Retail lists are five long.
- The voice's placement (bike at the entry, body after) and both levels are the shell's P144/P180
  choices, unchanged; not re-measured.
- Not listened to in the running shell.
