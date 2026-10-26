# 1 — air wrecks cut 360 (FINAL)

Task S154. 2026-10-26.

## What I did

- **The cause.** Not `air::wreck_test`: under Air Wrecks Off its airborne clause was already skipped,
  and while a 360 is turning (or cut short and still off the ground) `spin.ramp` holds that clause off
  even with the switch on. The wreck was the 360's own. `air::three_sixty` ends a 360 when a wheel
  touches, the body touches, **or the yaw torque has decayed under `MinThreeSixtyTorqueY`** — which is
  what pushing the stick against the turn does — and if the clip is still part way round (more than
  35° off the take-off heading) it latches `Spin::wrecked`. rider.rs turned that latch into a Tail
  wreck (reason 11) with no regard for the switch. So a 360 stopped midway wrecked the frame its torque
  ran out, in the air. Found by a probe ride on the MD16 pack: cut at tick 190/200/210 off the flip
  ramp, the switch on and off both wrecked Tail at 218/228/238, all in the air (take-off 180, landing
  304).
- **The fix.** `Spin::cut` (new, the port's own, on the snapshot wire): set with `wrecked` when the
  360 ended neither on the ground nor touching. rider.rs takes it beside `wrecked`, and inside the
  wreck closure drops the 360's wreck when `!allowed.air && cut`. The landing is then judged by
  `wreck_test_in` as P138 has it (crossup, ground pitch, g loads, trick endings). On is untouched:
  the filter passes everything when `allowed.air`.
- `spin.cut` is not walked by trick_pack's hash (it is taken each frame with `spin.wrecked`, so it is
  always false between frames), so no pinned pack-ride hash moved.
- **The other airborne attitude wreck.** `wreck_test`'s trick-ending block answers `NoHander` for a
  no-hander rolled past `NoHanderMaxRollAngle` in the air as well as on the ground. That is a wreck
  for an attitude before touchdown, which the brief says Off must not throw; `wreck_test_in(.., false)`
  now skips it while neither on the ground nor touching, and still answers it on the touch.
- **Flips** were already clean: a backflip/front flip let go clears `flip_active` but keeps
  `flipping` until upright or down, and nothing but the airborne clause (already off) and
  "torque applied while touching" (a touch) can wreck it. The tests prove it rather than assume it.
- Checked opentennis: nothing there for this (no air wrecks in tennis).

## What the original does

On (`air_wrecks` true) is unchanged and is still the original's / MX vs. ATV's as ported: the 360 cut
short in the air with the clip playing wrecks Tail on that substep (MD16/MD30's reading of MX vs.
ATV's 360 end test), and a rolled no-hander wrecks in the air.

## How it was proved

- `tests/ride.rs` `s154_cut_short_moves_ride_down_with_air_wrecks_off`, on the disc's race125p with the
  MD16 pack: the 360, the backflip and the front flip, each held through once, then cut (stick against
  the turn for 40 ticks) at a quarter, a half and three quarters of the way from take-off to once
  round. Off: no wreck before touchdown, and the bike comes down, in all nine. On: the ride is equal
  tick for tick to Off's up to On's wreck, and all three 360 cuts wreck Tail in the air (231, 254, 277;
  take-off 180, landing ~304) — today's behaviour. A held 360 is the same ride either way and wrecks
  on the touch with the clip part way round. With the filter forced through the test goes red ("360
  let go at 203: never came down" — the ride stopped at its airborne wreck).
- `tests/air_wrecks.rs` `off_leaves_a_rolled_no_hander_to_the_touch`: the rolled no-hander in the air
  wrecks with On, not with Off, and wrecks with Off on a touch or on the ground.
- `tests/air.rs` `the_wreck_test_is_the_originals` (8000 generated states against the original's
  bike wreck test, ran here with the disc): On is still the original bit for bit; Off may now also
  differ where On answered NoHander off the ground and untouched.

## Not verified / not 1:1

- Not reproduced in the original (MX Unleashed or MX vs. ATV): Air Wrecks Off is an addition with no
  original to compare, and On's code path is unchanged, so the proof is the port's own rides above.
- The user's exact ride (which track, which jump, how the 360 was stopped) was not replayed; the
  generated flip ramp with the MD16 pack stands in. A cut by some other route than the torque decay
  (e.g. the body touching a wall mid-air) is a touch, and still wrecks under Off by design.
- What the landing does after a cut-short 360 under Off is whatever `wreck_test_in` says: in the
  tests some cuts land clean and some wreck on the touch (crossup / ground pitch). The 360's own
  "landed part way round" is not re-asked at touchdown for a 360 that already ended in the air,
  because the clip state was cleared when it ended — a design choice, not the original's (it has no
  Off).
- The airborne throwaway "Joust" (a throwaway trick held to the end in the air) still fires under
  Off: it is the rider throwing the bike away on purpose, not an attitude or rotation, so I left it.

## Next

Nothing.
