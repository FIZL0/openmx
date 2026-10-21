# 1 — ragdoll exact (PART)

Task S56. 2026-10-21.

## What I did

- **Captured four wrecks of the original** on Free09, starting from recording2's save state, with the
  pad driven by `research/s56/*.pad`. `research/s56_wreck.py` runs `watch/s56-ragdoll.toml`, one row a
  tick, and saves a state into slots 4, 5, 8 and 9 (one each tick) from the first tick the human's
  rider reads a wreck reason.
  - The four wrecks: `front` (reason 1, over the bars on a landing), `side` (reason 2, at speed),
    `back` (reason 11, a loop-out) and `tip` (reason 3).
  - Each row holds every bone body's last_pos/pos/vel/last_quat/quat/rates, the actor's wreck fields,
    the holds and the skeleton's rests.
  - Each row also holds what the body's update reads off the rest of the game: the whole request
    block, the 0x200 before it (the hand dummies), the 0x100 at the request's +0x1ac (the bike's
    centre and radius), and 178 absolute world blocks found by read tracking (see below).
  - `research/s56_state_check.py` checks a saved state against its row word for word. A state saved
    mid-tick is torn: tip's 2473 and 2474 were, and 2475 is good.
  - The captures are three sets, all in context/capture/s56 and git-ignored:
    - `v1/`: bones only.
    - `v2/`: adds the request.
    - the top level (v3): adds the world blocks.
- **The oracle runs the original's own tick.** `rider_body_update` (oracle.txt, S56 block) is called
  in the Ee on the state's RAM and scratchpad, with each tick's request and world laid in from the
  capture. Every one of the 18 bones' six quads is compared against the next row
  (`crates/game-sim/tests/ragdoll_wrecks.rs`).
- **`Ee::reads`** (ps2-oracle) optionally records every byte address a call reads. The exploration
  `what_the_wrecks_read` uses it to find the moving reads that a replay must lay in: reads that were
  not written first, that sit outside the actor and the request, and that change between two
  consecutive states.
- **Fixed `ps2_float::vu::sqrt`** (see the first divergence). The port's self-pinned hash vectors moved
  with it: HUNDRED_TICKS ×3 and platforms.rs' HASHES, TRACE_DIGEST, VEL_Y, FINAL_DUMP_DIGEST,
  FIXTURE_TEXT_DIGEST and RIDERS_*. Only values changed; the leaf counts and the dumps' heads did not.
  No test against the original's data changed verdict: the full `./kit check` passed, after one rerun
  of the known four_peers netplay flake.
- **Filed S60** (the ragdoll's per-tick mode machine in the port) and **S61** (bone contact with the
  bike and other moving collidables, plus a capture of what they read).

## First divergences, in the order they fell

1. **The Ee missed the original by one ulp in 207–261 lanes on the first tick.** The cause was
   `vu::sqrt`: it computed `chop(sqrtf(x) as f64)`, which "chops" an f32 that is already rounded to
   nearest, so it does nothing. PCSX2 runs VU0 at Roundmode 3 (chop), so `vsqrt`, and `vrsqrt` through
   it, must be chopped too.
   - F1's journal (2026-10-15-f2-f1-ee-fpu, FINAL) had already flagged this, and it was never fixed.
   - Now the nearest root steps down one ulp whenever its square, exact in f64, exceeds x.
   - With that, every lane of the tick matches.
   - Test: `crates/ps2-float/tests/vu.rs` (√5: nearest 0x400f1bbd, chopped 0x400f1bbc).
   - Ruled out on the way: the GameDB COP2 patch, whose words are already in the RAM, and the
     scratchpad (loading the state's Scratchpad.bin changed nothing, but it is kept).
2. **The entry tick drew the wrong numbers.** The rest of the game calls `rand()` in the same tick
   before the body updates. The count varies (0 to about 20), so the oracle tries each count from the
   previous row's `rand_state` until the bones match.
   - The wreck entry draws 6 inside the update (side's tick 2605: 12 draws before it, 6 by the body).
   - The damping/settle switch draws 1 each time it fires.
3. **A row could repeat the tick before it.** When PINE asks for a save state, the pause can land
   between the frame counter's step and the simulation's tick. The recorder then logs the previous
   tick again under the next counter, with the rng and every bone identical. The replay skips such
   rows (`repeated`); tip's row 2476 is one.
4. **Bike contact.** With 1–3 fixed and the request laid in, the oracle replays a wreck exactly until a
   bone first touches something moving that the capture does not carry: the wreck bike's own
   collision objects, or another actor.
   - With the v2 captures (no bike contact), back and tip replay exactly from the entry to the end of
     the capture (2780 and 2779), through the respawn, about 300 ticks each.
   - With the v3 captures, the bike runs into the rider early:
     - front parts at 2608, side at 2613, back at 2491, tip at 2482.
     - Without the world blocks, front parts at 2608 too (86 lanes rather than 65) and back at 2489.
     - So the blocks help but are not enough. The reads still missing (13 ranges) are in
       research/oracle.txt.
   - Not fixed: see Next.

## What the original does

The rider body actor is the human's type-4 vehicle record less 0x400. In this session it sits at
0x017a7700.

Its update is `./kit fn 0x0024e640`, called with dt in f12 and the actor in a0. It runs, in order:

1. the request's +0x238/+0x23c effects;
2. the two look clocks;
3. `./kit fn 0x0024c7e0` (the mode machine) while the request's +0x8 is set;
4. the physics step (S52's `update`);
5. `./kit fn 0x0024e250`.

The mode machine covers the throw clock, the hand holds and their targets, the ground query, and the
switch at `./kit fn 0x0024c310` that settles, flings or goes limp. The entry draws 6 numbers from
`rand_state`.

## How it was proved

Save-state captures are replayed through the original's own code in the oracle, with every bone lane
compared bit for bit.

`the_originals_update_replays_each_wreck` (crates/game-sim/tests/ragdoll_wrecks.rs, not ignored) pins
exact replays from each state up to the tick before contact:

| wreck | from | through | ticks | covers |
|---|---|---|---|---|
| front | 2600 | 2607 | 7 | |
| side | 2604 | 2612 | 8 | the entry and its 6 draws |
| back | 2475 | 2490 | 15 | |
| tip | 2475 | 2481 | 6 | |

To check that the test really ran rather than skipped, front's bound was raised to 2608 for one run.
It failed: "tick 2608 parts in 65 lanes".

The test skips where the captures are not in context/.

## Not verified / not 1:1

- **The port is not compared at all.** The task's done condition (each wreck replays bit-exact in the
  port through to rest) is not met.
  - The port has no per-tick mode machine. `ragdoll_ride::Down` steps the physics with its own fixed
    drive, and its settle gate is inferred (S53). Comparing it tick by tick against the captures would
    part on the first tick, for that reason alone. → S60.
  - Bone contact with the wreck bike and with other moving actors is not ported (S52 left the
    0x001b2ff0/0x001b3180 paths and bike_box +0x14 out). → S61.
- **The oracle replay stops at the first bike contact.** The capture carries absolute world blocks
  found by read tracking in this session's heap. They are incomplete: 13 ranges were still missing.
  They are also tied to this boot's allocation, and a new session's heap would differ.
- **None of the four wrecks reaches "the rider coming to rest" in the v3 captures.** Only v2's back and
  tip ran exactly to the end of the capture.
- **"tip" is not a slow tip-over.** Its bones move at about 14 units/s at the entry. The slower pads tried
  (`research/s56/tip-*.pad`, tip-slow.pad) did not give a capture that was kept. A tip-over from
  standing still was not captured.
- **The 6 entry draws and the 1 switch draw are observed, not read off the code.** The oracle counts
  them; which call makes each one was not traced.
- **The vsqrt fix changes every vector port that normalises through `vu::rsqrt`.** Its effect was
  checked only by the full `./kit check`, which has no capture-backed test that the old rounding
  passed and the new one fails, or the reverse.
- **The read-tracking ranges are this session's heap addresses.** They live in oracle.txt and the
  watch spec, not in code.

## Next

S60 first: port the whole tick, mode machine included, and compare port against oracle tick by tick
on the spans the oracle already replays. That needs `ragdoll_of`/`drive_of`/`tables_of` from
tests/ragdoll.rs generalised from its fixed actor to 0x017a7700.

Then S61: watch the bike's and nearby actors' records by chain, recapture, and port the moving
contacts until the oracle, then the port, run each wreck to rest.
