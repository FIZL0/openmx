# 1 — markers up down hold (FINAL)

Task P106. 2026-10-22.

## What I did

- `crates/game-sim/src/markers.rs`: `HOLD_MS = 200` is back. `fired` gives Left/Right on
  `Table::pressed`, as P93 had them. Up/Down use P88's test against the first binding's
  `ControlState`: `is_held && prev_held_ms < HOLD_MS && held_ms >= HOLD_MS`. That fires once per hold
  and does nothing for a tap. `holding` is back too, now counting Up/Down only, because Left/Right have
  no hold to show. Both still give nothing while a trick button is down, since that d-pad is the
  gesture's. The module doc says so.
- `crates/openmx/src/hud.rs`: `draw_markers` takes `holding` again and draws P88's bar under the
  label. The bar is always drawn, empty when nothing is held, so the panel stays the same size while it
  is up. It shows the hold colour while filling and the pressed colour once full.
- `crates/openmx/src/main.rs`: `MarkerShown` carries `holding`, read with `markers::holding` off the
  seat's pad (0 when the seat is not riding), for both the GPU path and the software path.
- Tests (`crates/openmx/tests/markers.rs`):
  - `down_held_199_ms_places_nothing_and_200_ms_once`: 30 Hz ticks, then 1 ms ticks, bring the timer
    to exactly 199 and then 200. At 199 nothing fires. At 200 a marker is placed on that tick, and 90
    more held ticks fire nothing. Up behaves the same way: no teleport at 199, a teleport at 200.
  - `left_fires_on_its_first_tick`.
  - `the_bar_fills_with_an_up_or_down_hold`: `holding` rises 0 → 200 and stays at 200, is 0 for
    Left/Right and with a trick button down, and the bar draws about half lit at 100 ms and full at
    200 ms and beyond. This mounts `hud.rs` and `aspect.rs` into the test.
  - The existing Up/Down cases now hold for `HOLD_MS`. At 30 Hz that fires on the hold's eighth tick
    (index 7, asserted).
- `crates/openmx/tests/markers_saved.rs` (P99's) pressed Down/Up for one tick. Its `press` and a new
  `held` now hold for 8 ticks, and the tape script holds them the same way. Its assertions are
  unchanged.

## What the original does

Nothing: markers are the port's own (see markers.rs's doc). The hold is read off the original input
layer's timer, `held_ms`, which comes from the port's input.rs.

## How it was proved

Tests only, through `./kit check -- -p openmx --test markers --test markers_saved` (57 passed), then
the full gate in `./kit done`.

## Not verified / not 1:1

- No proof against the original is possible: markers are the port's own feature.
- The bar was not looked at in a running shell. The check is the pixel count in
  `the_bar_fills_with_an_up_or_down_hold`, against the same display with no hold.
- At 30 Hz the 200 ms hold fires on the eighth tick, after about 233 ms of wall time, because the timer
  steps by 33 ms. That matches how P88's 250 ms behaved. It is not exactly 200 ms of wall time.
- Only the first binding of Up/Down is read for the hold (P88's way, using `Table::control`). A second
  binding of Up/Down, if a preset has one, fires on the press through nothing and holds through nothing.
  Left/Right's press reads both bindings, as P93 had it.
- Online and tapes: the hold is read from the same pad state on the agreed ticks, so they are unchanged
  in kind. No online session was run to check this.

## Next

Nothing.
