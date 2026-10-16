# 1 — a5 whole frame (FINAL)

Task A5. 2026-10-16.

## What I did

One frame of the original's bike, captured off the live emulator and compared against the port's same
frame — the proof S6 ends by asking for in its own words: "a whole-frame comparison would catch an
ordering mistake between the animation header and the rest of the rider update that these
per-function proofs cannot".

Four files:

- **`watch/frame-bike.toml`** — the spec. One row per tick of the human's rider from the save state
  the human's recording is anchored to, carrying the three stages of the rider's frame side by side:
  the physics' outputs, the attachment walk's seat position beside the chassis' own world transform,
  and the six fields the animation header writes.
- **`watch/pads/frame-bike-ride.pad`** — the ride, because a parked bike proves nothing: throttle
  held, both stick axes swept, several jumps and two wrecks in 47 seconds.
- **`research/frame/bike_probe.py`** — walks the live actor list, marks the human's actors, reads the
  twelve dispatch slots the rider's update calls through and dumps the fields of each stage. This is
  what says which list entry the human is riding *today* rather than trusting a hop count.
- **the A5 section at the end of `crates/game-sim/tests/replay.rs`** — five tests: the capture's own
  soundness, the whole-frame comparison of `motion::Driver` against the original, the same comparison
  fed last frame's physics (which must fail), the id latch, and the attachment walk's distance test.

Plus an A5 section in `research/oracle.txt` with the call order, the addresses and every field the
capture pins down. The capture itself (`context/capture/frame-bike-ride.csv`, 1,400 ticks and its
`.json` provenance) is game data and is not committed; the tests say so and pass when it is absent.

## What the original does

**The order of the frame.** The ridden actor's per-frame update is `0x002520d0`; its riding branch is
`./kit dis 0x002521a0 0x00252268` ($s0 the actor, $f20 the dt), and it calls, in this order: slot
`+0xec`, the ground step (only while the ride state's reason at `+0x748` is 0), the air state, the
attachment update `0x00133500(obj+0x50, dt)`, the stunt idle, the stunt state pack, the stunt clocks —
whose tail past `clock_section_end` is the animation header — then `obj+0xa08 = obj+0xa04`, then the
ride state machine. The dispatch table is `obj+0x48`, and `bike_probe.py` reads all twelve slots out
of the live object, so that order is this save state's own table rather than a reading of a listing.

**S6's journal cites `./kit fn 0x00257e00` for the header, and that address is wrong**: nothing starts
there, it is inside the ground step. The header is the tail of the stunt clocks (`+0x130` on the table
= `0x002593e0`), which is what `oracle.txt` already said.

**What each stage hands the next.** Three relations, all within one frame:

- the air state's crossup angle at `+0x600` → the header's airborne arm remaps it onto the shown
  crossup `+0x788` **and** the shown pose `+0x9e8`;
- the rider's two leans, which no listing named and which this task found by capturing the whole
  2,688-byte object and searching it for the field each output is an exact function of: the **roll**
  lean at `+0x670` → `+0x9e8 = 0.5 + 0.5 * it` on a grounded frame, and the **fore-aft** lean at
  `+0x674` → `+0x9ec = (it + 1) * 0.5`, every frame;
- the state pack's requested id `+0x864` → the id the header latched and ran on, `+0xa78`.

**`MaxCrossupAngle` is 45 degrees**: `0x3f490fd9`, 0.7853980660438538, in both the physics' copy
(`+0x6d8`) and the animation's (`+0x9e4`), on all 1,400 captured ticks. S6 listed this as unproven —
"`Params::max_crossup` has no proven retail value" — so that item is now closed by measurement.

**The reset clears the shown crossup.** On the two ticks of the capture where the rider finished being
put back on the bike after a wreck, `+0x788` is set to exactly 0.5 — straight ahead — by the reset
rather than by the airborne arm. `motion::Driver` has no reset, so the port would carry the last
airborne crossup through a wreck, and `+0x788` is what the landing test judges a wreck on. That is a
real gap this comparison found, and it is in the list below rather than fixed here (the state machine
is S5's, and this task was told not to edit files it did not come for).

## How it was proved

**The capture.** `./kit record watch/frame-bike.toml --state 8 --pad watch/pads/frame-bike-ride.pad
-n 1400` on the task's own emulator copy: 1,400 consecutive ticks of the game's own counter, 0 torn
reads, 0 counter gaps. Read back by the tests: the rider chain resolves to type 2 with control source
1 and the chassis chain to type 4 with control source 1 on every row, `dt` is the 30 fps clamp
(`0x3d0888a0`, 0.033330000936985016) bit for bit on every row, no empty cells, and the ride holds 513
airborne ticks, 22 cross-fade restarts, 44 distinct lean values, 1,369 distinct crossup angles and 158
ticks with the rider down. The floor those have to clear is a constant in the test, so a capture of a
parked bike fails instead of passing vacuously.

**The whole-frame comparison.** `motion::Driver::frame` is run over all 1,399 steps of the capture,
handed the row's own `dt`, blend rate, `MaxCrossupAngle`, requested id, request count and previous
copy, crossup angle and both leans; all six fields it writes (`action`, `ramp`, `faded`, `fore_aft`,
`pose`, `shown_crossup`) are compared against what the original wrote on that tick, **bit for bit**.
It agrees on every one of the 1,399, with two excused disagreements, both printed in full by the test:
the two reset ticks above. The driver is re-anchored to the capture after each tick so that the *first*
disagreement is the one reported.

This is the test that has to be in Rust: the same comparison in numpy's IEEE float32 misses by one ULP
on five of the 1,399 ticks (`(lean + 1) * 0.5` rounds the other way), and `ps2_float::F` gets them
right. A whole-frame comparison that allowed a tolerance would have found nothing worth having.

**The ordering proof.** The same replay, handed the physics outputs sampled on the *previous* tick,
breaks on 1,092 of the 1,399 ticks (1,387 of them had the physics moving), first at tick 35. A port
that ran the animation header before the air state would compute exactly that: every ported function
right, the frame wrong. The id latch is the same argument on integers — `+0xa78 == +0x864` on
1,399/1,399 ticks and on 1,377/1,399 a tick apart — and the attachment walk is the same argument on
distances: the seat position sits 0.134 units from this tick's chassis and 1.734 from the previous
tick's, against 1.853 units of travel in the median tick.

**`./kit check`**: 389 tests pass. `./kit audit` clean.

## Not verified / not 1:1

- **The chassis is not compared numerically at all.** The port has no bike (S3/S17/S18/S19 are open),
  so nothing in the capture's `veh_vel`/`veh_xform` columns is checked against a ported number. They
  are recorded for the day it exists, and used here only as a distance.
- **The attachment walk's order is a tolerance argument, not a bit-exact one.** I tried to make it
  exact and failed: no field in the rider's 2,688 bytes satisfies `pos[t] - pos[t-1] == vel * dt` at
  lag 0 or 1 (the ratio runs 1.20–1.27 and is not constant), no 16-float block in the object equals
  the chassis' `mat4`, and there is no orthonormal 3×3 at any 16-byte alignment inside that window —
  the chassis transform lives outside the captured object. So the seat-position distances (0.134 vs
  1.734 against 1.853) are evidence of the order and not of the arithmetic.
- **`motion::Driver` has no reset**, which is why the two post-wreck ticks are excused rather than
  matched. Whoever lands S5's state machine should set `shown_crossup` and `pose` back to 0.5 there,
  and this test will stop excusing them.
- **The stunt arm and the missing-clip arm were never reached.** The ride asked for ids 25 and 28
  only, so `Request::progress`, `Clips::stunt` and the fallback pose are not exercised by the live
  comparison (S6's oracle proofs cover them over 10,000 generated frames).
- **`Blend` — what the motion library was asked for — is not compared.** Nothing in the rider object
  records it, so the live capture cannot see it.
- **`Request::force` and the request count are not exercised.** `force` lives in the trick machine and
  not in the rider, so it is fed `false`; `+0xa04` and `+0xa08` are 0 on all 1,400 ticks, so the
  "same gesture asked for again" restart path never ran. All 22 restarts in the capture are action
  changes. A forced restart would have failed the bit-exact ramp comparison, so this is a gap in
  coverage rather than a hole in the proof.
- **`Request::frozen` never happened**, so `Driver::freeze` and the frozen branch of the ramp are not
  touched by the live comparison.
- **`dt` was the clamp on every tick**, so the comparison says nothing about how the original behaves
  at any other step — which is the thing S8's entry warns about.
- **The hop counts are this save state's.** Two `next` hops to the human's rider and eight to its
  chassis are measured on Free09 from the recording's save state; another area or mode reorders the
  list, which is why both chains carry a type and a control-source column and the reader refuses a
  capture whose rows are not 2/1 and 4/1.
- **One capture in three showed a stalled sample**: a row identical to its predecessor in every
  sampled field, with the next row carrying two frames of cross-fade decay against one frame of
  chassis travel (ticks 686 and 687 of a capture I re-recorded for an unrelated reason). The test
  detects that generically and excuses those two rows; the committed capture has none, so that branch
  of the test is unexercised by it. I do not know whether it is the recorder sampling between the
  counter's increment and the rider's update, or the engine catching up after a host hitch.
- **The input is still not a human's.** This is the kit's own pad script, not the `.p2m2`: PCSX2 plays
  a recording back only from its GUI, which is S8's `plan/HUMAN.md` ask and is still open. So the
  frame order is proved on real play, but not on *the* recorded play.
- **`+0x8a0` is not the request count.** `oracle.txt`'s note on the state pack says it latches the
  count there; live it holds a packed word (`0xd4002100`, `0xd40c2900`). I did not re-read the listing
  to work out what the pack really writes, so the note stands with my measurement beside it.
- **Wreck reason 1 is unidentified.** 11 matches S11's attitude reason; the capture's other wreck
  reports 1 and nothing in `oracle.txt` names it.
- The roll lean is proved only through the grounded arm's `0.5 + 0.5 * lean`, which is the only place
  the simulation reads it; whether the renderer reads `+0x670` for anything else is not this task's.

## Next

Nothing for A5. For S3/S17 when the ported chassis lands: re-record with this same spec — the
`veh_vel` and `veh_xform` columns are already in it — and the attachment-walk test can stop being a
distance and start being a comparison. For S5: the reset's two pose writes, above. For S18: the
`MaxCrossupAngle` value is now measured, so `Params::max_crossup` can be loaded rather than handed in
by a test.
