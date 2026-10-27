# 1 — 2p animations (FINAL)

Task V90. 2026-10-27.

## What I did

- Replayed both riders' body machines (`Body::mode` + `Body::frame_clocked` on the drawn thq1 rider's
  own clip table) over all 12,917 rows of C10's `context/capture/2p/2p.csv`. Each machine was driven by
  the original's request words (held, wreck, the three axes, progress, the latch count) and the bike's
  words (airborne, flip_ramp as the backflip, trick_id as the gesture), plus the pad's reverse rule off
  the 2p tapes. Each tick's (state, mode) and cross-fade ramp were compared with the original's.
  - Test: `the_2p_riders_poses` in `crates/game-sim/tests/rider_anim.rs`.
- Posed the original's single-clip ticks (V12's method) on the drawn rider's clips, 22 joints within 1e-4.
  - Test: `the_2p_riders_joints`.
- Posed the drawn rider off every replayed frame (`posed_by`) and compared all 22 joints per tick.
  - Test: `the_2p_riders_drawn_poses`, which pins the count within 1e-4 as a ratchet.
- No sim or render change. No part found in the machine is the port's (see below), and the joint parts I
  looked at did not resolve to a port fault I could prove.

### Parts by tick and clip (state, mode)

| rider | ticks the same | ramp bit for bit (steady ticks) |
|---|---|---|
| P1 | 12,880 / 12,916 | 11,864 / 12,209 |
| P2 | 12,875 / 12,916 | 11,891 / 12,428 |

Only two ticks part by more than two ticks of row phase:

- **P1 134857**: theirs (30 riding, 1), ours (35 air, 1).
  - It is a one-tick riding pose in the middle of a take-off. The row's airborne word already reads 1, but
    the original's body saw the bike down for one more tick.
  - After it, the original restarted the ramp twice (134859..134872 are off by a non-step amount).
- **P2 140364**: theirs (30, 1), ours (40 wreck, 2).
  - The wreck request is on the row a tick before the original's body takes it.

Every other differing tick is within ±2 ticks (row phase), or is a limb 41..45 drawn differently. The
limbs are random draws and the pool is not the original's.

Per clip, (ticks, the same), P1:
`0:(13,13) 1:(128,128) 3:(78,78) 4:(89,89) 5:(23,23) 6:(7,7) 12:(7,6) 30:(8746,8731) 33:(19,17) 34:(24,24) 35:(2224,2215) 40:(851,851) 42..45 limbs`.

Per clip, P2:
`2:(86,86) 7:(481,477) 30:(9531,9521) 33:(107,103) 34:(157,152) 35:(1471,1464) 38:(48,47) 40:(582,579) 41..43 limbs`.

So the ride covers these, and they match at the clip level:
- riding lean (30);
- air (35);
- reverse (33/34);
- gestures/tricks (0..7, 12);
- the backflip (38, P2);
- wrecks (40) and limbs;
- the put-back (mode 3 → riding).

### Joints

- **Single-clip ticks (V12's method):**
  - P1: 248 ticks (states 1, 3, 4, 5), worst 2.1e-5.
  - P2: 497 ticks (states 2, 7), worst 4.8e-7.
  - Each clip pointer the original sampled answers one state only.
- **The drawn rider per tick:** ticks where the state and mode are the same, counting those within 1e-4 /
  1e-2 of this row or the rows either side.

| rider, (state, mode) | ticks | ≤1e-4 | ≤1e-2 |
|---|---|---|---|
| P1 (30,1) riding | 8705 | 7435 | 7803 |
| P1 (35,1) air | 2206 | 1488 | 1587 |
| P1 (40,2) wreck | 851 | 703 | 707 |
| P1 (33,1)/(34,1) reverse | 17/24 | 3/0 | 3/1 |
| P1 gestures (1,3)(3,3)(4,3)(5,3) | 127/76/88/22 | 114/50/75/9 | same |
| P1 (0,3)(6,3)(12,3) | 12/6/5 | 0 | 0 |
| P2 (30,1) riding | 9490 | 5035 | 8031 |
| P2 (35,1) air | 1417 | 352 | 888 |
| P2 (33,1)/(34,1) reverse | 103/152 | 8/38 | 60/86 |
| P2 (7,3)/(2,3) | 477/86 | 425/72 | 428/72 |
| P2 (38,3) backflip | 47 | 1 | 1 |
| P2 (40,2) wreck | 579 | 442 | 494 |

Totals pinned: P1 9,959 of 12,244 and P2 6,428 of 12,463.

## What the original does

At the clip level, the port's body machine is the original's on this ride. The test's own setup needed
four things to see that:

- **The pad must be `connected`.** `Pad::new()` starts disconnected, so gas/brake read 0 and reverse
  never fired. With it set, reverse matches.
- **A 2p row's dt is the next frame's.** Stepping row N on row N−1's dt made the ramps bit for bit.
- **P2's bike words (airborne, flip_ramp, the reverse controls) need a one-row lag; P1's need none.**
  trick_id and req_wreck are read at the row for both. The P2 lane is read on the other side of its body
  step.
- **The settle flag is the ragdoll's (S160).** It is handed in from row−1 *before* `mode()`, so the wreck
  entry clears it as the original's does.

## How it was proved

- A capture compared frame by frame: `context/capture/2p/2p.csv`, 12,916 ticks × 2 riders.
  - `the_2p_riders_poses` pins the parts list and the same/ramp counts exactly. It was checked to fire
    by bumping a count by one.
- `the_2p_riders_joints`: 745 single-clip ticks within 1e-4.
- `the_2p_riders_drawn_poses`: a ratchet on the drawn joints.

## Not verified / not 1:1

- **The drawn joints of blended arms part from the original's.** Each one below has its reason, and none
  was proved to be the port's.
  - **Reverse (33/34)**: P1 3 of 41, P2 46 of 255 within 1e-4.
    - A best-fit sweep over the Two blend's (phase, mix) at P1 140370 (ramp 0) found mix 0.75, which is
      the same as ours (0.748).
    - The worst lane stayed at 0.011 for every phase 0..1, so the residue is in joints the reverse pair's
      phase does not move. It may be the snapshot/fade layer or the eased axes.
    - Not found.
  - **Air (35,1)**: P1 67%, P2 25% within 1e-4.
  - **P2's riding (30,1)**: 53% within 1e-4 (85% within 1e-2), against P1's 85%. P2's lane is read at a
    different phase of its step (its joints fit this row or the next about evenly), so part of this is the
    row phase.
  - The port eases phase/mix (`Lag`, MIX_TAU 0.125 / PHASE_TAU 0.15) off the request axes. The original's
    own eased records are not in `watch/2p.toml`, so the eased values cannot be held directly. Follow-up
    added.
- **Gestures in mode 3 that never fit**: P1 (0,3) 12 ticks, (6,3) 6, (12,3) 5; P2 (30,3) 18 and (38,3)
  backflip 47 (1 fit).
  - Also: the single "mode 1" gesture entry ticks (one each) don't fit.
  - These are short runs inside fades. Not investigated past the counts.
- **The wreck clips' joints are not compared single.** During a wreck `playing` is 0 and the ragdoll
  poses the body (S160), so V12's method never sees a 40..59 clip running free.
- **The settle flag is taken from the capture**, not from the port's ragdoll (S160's).
- **The limbs 41..45** are random draws, so they are compared as a class, not by which limb.
- **Row phase is tolerated rather than proved:**
  - ±2 ticks on states;
  - P2's one-row lag on its bike words;
  - ramps one or two rate-ticks off.
- **The port's own Ask derivation** (`game-render/src/rider.rs` building the Ask from the bike) is not
  driven by this test. The Ask is built from the capture's words.
- **Not in the ride, so not checked:**
  - foot down (no foot-down clip played);
  - the gate / getting on (mode 4);
  - no_hander and spin.

## Next

- Follow-up C11 (plan):  put the original's eased lean/phase records in `watch/2p.toml` (or a new
  capture), so that the blended arms' joints can be held against the port's `Lag` values tick by tick.
