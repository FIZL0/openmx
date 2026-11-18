# 1 — bike clock in every bike spec (FINAL)

Task S277. 2026-11-17.

## What I did

- Added S194's `bike_clock` field (rider +0x13fc) to the end of 26 one-rider bike specs in `watch/`:
  acceptance, b7-truck-strike, frame-bike, p72-reverse, p85-edge-sound, rider-joints, s134-bike-hit,
  s195-knock, s197b-wreck, s207-free09, s219-trailer, s219-trucks, s22-standing, s240-ragdoll,
  s251-truck-list, s56-ragdoll, s58-slip, s59-edge, s64-wheels, s68-backwards, s72-page, s79-score,
  s84-chord, s89-rails, s8-start, v70-zones.
- Each uses the rider chain that spec's own `wreck_reason` (+0x748) already reads: the two-hop
  `… 0x8 -> 0x8 -> 0x0` in most, the three-hop `… 0x8 -> 0x8 -> 0x8 -> 0x0` in s219-trucks, s251 and
  s89-rails (as s129-train).
- `clock_ticks` also needs a `dt` column. b7-truck-strike, p72-reverse, s251-truck-list and v70-zones had
  none, so they got `dt` (`@frame_dt`) too.
- Appended at the end of each file, so the existing columns keep their places.
- replay.rs: `quarter_ticks`' doc now says every one-rider bike spec carries the clock. No code change:
  it already switches to `clock_ticks` when the column is there.
- Nothing is re-recorded. Existing captures have no `bike_clock` column, so every test reads them as
  before. The clock takes over when a capture is re-recorded with these specs.
- opentennis: nothing to take (it is a capture-spec change only).

## What the original does

The player update adds the frame's dt to rider +0x13fc as its last store, after the physics, every tick
(S194; `./kit fn 0x0027c890`).

## How it was proved

- Save-state RAM images: in all five `context/recordings/*_SaveState.p2s` (first_recording,
  recording2, animandpump, biplane, fly_and_collide), the two-hop and three-hop chains give
  `rider_type` 2 and the same +0x13fc value. So the two-hop specs read the same clock word that S194
  proved live on the three-hop chain.
- The TOML parses: `./kit record` ran a scratch spec with the same field form.

## Not verified / not 1:1

- No live capture with the edited specs. Kit copy 5 would not take the first_recording state:
  `./kit pine load 8` left `@game_manager` the same as slot 4's, and both chains stayed null. The proof
  is the RAM-image chain walk above plus S194's live proof, not a fresh recording.
- No capture is re-recorded. So `clock_ticks` has not run on any of these 26 specs' captures yet. A
  capture whose rows break its rule (a row neither before nor after its frame's bike) will fail loudly
  when it is re-recorded.
- The multi-rider specs (2p, split, c16-fly, n24-meet) are left out. `bike_clock` names one bike, and
  `clock_ticks` numbers one bike's rows. Two riders would need a per-rider column and a rule for which
  one numbers the rows.
- Plane and kiowa specs (biplane*, c14, c19, r25), and those with no rider chain (s241, s252*, s281,
  car*, pad, input, axes, timing, struct, frame, example) are left out: no bike rider to read.

## Next

Nothing for S277. Re-record a capture with one of these specs when a task next needs it.
