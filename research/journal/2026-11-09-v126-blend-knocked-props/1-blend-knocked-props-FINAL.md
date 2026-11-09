# 1 — blend knocked props (FINAL)

Task V126. 2026-11-09.

## What I did

- The knocked props (S131: balls, crates, barrels, signs) were drawn at their bodies' nodes on the last
  tick, so above 30 fps a rolling ball stepped. The shell now keeps the props' nodes from the tick before
  the last (`props_before`, set beside `cars_before` where the other "before" copies are taken) and draws
  each prop at `pace::Blend::placement(before, now)`: the two placements' transforms through
  `Blend::at` — the same blend the bikes, planes and helicopters use, a jump past `pace::SNAP` (a reset,
  a teleport) shown where it lands — read back as pos / look / up for the scene's `Placement`.
- A prop whose node is the same on both ticks is drawn as before (its rest placement, or its node), and
  the existing "only move when the drawn placement changed" test still holds, so props at rest get no
  `move_scene_prop` per frame. A moving one is moved every drawn frame, as it must be to look smooth.
- Nothing in the simulation changes: `props_before` is a copy of `traffic.props.nodes()`, read by the
  picture only.
- opentennis: nothing to take; its ball is drawn by its own interpolation, the blend here is pace.rs's.

## What the original does

Render-side only; the original draws at 30 fps, there is nothing to match (AGENT.md: interpolating between
ticks is ours).

## How it was proved

`pace::tests::a_rolling_prop_is_drawn_between_its_ticks`: a ball two feet down a hill and a half-radian
roll apart is drawn at blend 0.5 at the midpoint, turned a quarter radian with whole axes; blends 0 and 1
are the two ticks; a move past SNAP is not blended.

## Not verified / not 1:1

- No headless ride was run to watch a knocked ball on screen at high frame rates: the blend is the tested
  `pace` function and the shell's wiring is a copy of the path cars' (`cars_before`); I judged the unit
  test plus the gate enough for a render-only change.
- Index pairing: `props_before[i]` pairs with the i-th of `props.nodes()`; that list's order and length
  are fixed after load (a reload builds a new set, and a length change falls back to the unblended node
  via `get(i)` only if shorter — a same-length reorder would blend across bodies for one frame; none is
  known to happen).
- A reset that moves a prop less than SNAP (30 ft) back to its rest is slid across for one tick rather
  than snapped, the same as the cars and bikes.

## Next

none
