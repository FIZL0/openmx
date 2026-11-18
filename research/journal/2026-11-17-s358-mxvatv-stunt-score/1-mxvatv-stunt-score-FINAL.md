# 1 — mxvatv stunt score (FINAL)

Task S358. 2026-11-17.

## What I did

- A ride with a trick pack now scores MX vs. ATV's way. Rides with no pack still score MX Unleashed's
  way, and that code path is unchanged.
- `score.rs`: new `Score::atv_*` calls next to the old ones:
  - `atv_start`, `atv_mark`, `atv_end` for one stunt.
  - `atv_bank` (close ×1, then tally × node count, then repeat per node).
  - `atv_cancel` (a wreck: close ×0, tally ×0, nothing repeated).
  - `atv_grow`: the manager's tick. Every live, held node grows, not only the newest.
  - `atv_frame`: the bike's per-frame part. Wheelie and stoppie after 1 s on one wheel, the 0.2 s bank
    clock on the ground, the backflip and front flip watches.
  - `Atv` keeps the repeat scales (sparse; an entry back at 1 is dropped), the bank clock and the two
    wheel clocks.
- `trick_pack.rs`: `Moves.values` keeps every stunt row's PointValue and HoldPointValue by name.
  `trick_pack::State` carries the `Atv`, so only pack rides have the new words on the wire and in the hash.
- `rider.rs`: on a pack ride a trick starts its node (off the ground and not wrecked), is marked while
  its hold grows, and ends with its clock. The 360 starts held when its clip runs and ends when it
  comes round or its clip stops. The frame calls `atv_frame` and `atv_grow`.
- The front flip's watch step moved into its own function (`step_front_watch`). `frame_with` calls it,
  and the behaviour is unchanged.

## What the original does

- R17's journal (2026-11-16-r17-mxvatv-backflip-score) has the calls. The repeat is `./kit fn 0x00268de0 -p atv`:
  - If the stunt's scale is 1, every one of the 46 scales is first divided by the table's +4, capped at 1.
  - Then scale = max(scale × table +8, table +0xc).
- The table head in state 4: +4 = 0x3f733334, which is **one bit over 0.95**. +8 = 0.8, +0xc = 0x3ed1b719.
  The first oracle run caught this, and the port now uses the exact bits.

## How it was proved

- `tests/mxvatv_stunt_score.rs` `the_port_scores_stunts_as_the_original`: a function oracle on state04's RAM.
  - 12 000 generated calls in runs of up to 40: stunt start (tricks 0–23 and the five kept stunts, held
    or not), end, mark, manager tick (state 4's dt or any dt up to 0.1 s), bank, and wreck cancel.
  - Then one backflip banked ten times, down to the repeat floor.
  - After every call it compares every node's words, the combo and running flags, the total, the
    combo's points and all 29 stunts' repeat scales. All bit-exact.
- `tests/score.rs`: two tests that run without the image. They repeat the R17 numbers on the port:
  the 0x3d node, 6999 after 15 ticks, 0x0d once ended, no growth after the end, the bank ×1, the scale
  0.8, the next backflip at 1600/8000, two stunts banked × 2, and a wreck adding nothing. The second
  test checks the 0.2 s ground bank through `atv_frame`.

## Not verified / not 1:1

- **The ride itself.** Nothing was compared against a live MX vs. ATV capture. The calls are proved, but
  not when the ride makes them.
- **When a trick is held (marked).** The original marks a trick while its gesture is held and its clip
  time is between min(HoldTime, 0.3) and HoldTime plus a small constant. The port marks it on any frame
  the trick clock's hold grew. This is an approximation, because the port's trick clock is MX Unleashed's.
- **The backflip and front flip timing.** The original starts and ends them on its clip words and
  up/forward heights. The port uses its own flip watch (0→1 starts, →3 ends). A watch that is forgotten
  part way through ends the node as landed, so a live node can't block the bank forever. That is my
  choice, not the original's.
- **The front flip** is the port's own stunt and uses the backflip's row values.
- **The bank clock's "running" test.** The port counts a trick as running if its clock runs or any
  non-wheel node is live. "On the ground" means not airborne. The original reads +0x324 and the
  stunt id. Close, but not proved.
- **The 360 starts held (marked).** I read this from the clip, not from an oracle.
- **The trick start gate.** I approximated "grounded / +0xc80" as airborne and not wrecked.
- **Row values.** A missing pack row falls back to state 4's numbers (trick 1000/1000, backflip
  2000/10000, wheel 1000/1000, 360 2000/15000). With stacked packs, only the laid pack's rows are read.
- **The repeat constants** are state 4's table words, not read from the pack's ini.
- **Listeners.** The best combo, the HUD and the trick menu's points are not ported. The menu still shows
  the MX Unleashed-style numbers.
- **The pool** is MX Unleashed's 100 nodes. I didn't check MX vs. ATV's pool size.

## Next

- Follow-ups filed: S410 (trick hold window) and S411 (ride timing capture).
