# 1 — mxvatv 360 stunt end (FINAL)

Task R47. 2026-11-18.

## What I did

- Read the caller of the stunt clock (bike vt `+0x4e8`). It is the trick step (vt `+0x4e4`), and it calls
  the clock only for a remote bike.
- Wrote `crates/game-sim/tests/mxvatv_stunt_end.rs`: 4 tests that run SLUS-21104's trick step on
  state04's bike through a long 360.
- Added an R47 block to `research/oracle.txt`, after R43's. Summary in `research/r_stunt_end/README.md`.
- Filed S467 (low): keep the port's 360 animation state from the clip's first start to the 360's end,
  restart it on the second pass, and never end it at 1.5 s.
- Nothing from ../opentennis: it has no stunts.

## What the original does

Recipes: trick step `./kit dis 0x004e49c0 0x004e4cd0 -p atv`; stunt clock `./kit dis 0x004e4900 0x004e49c0 -p atv`;
the hold `./kit dis 0x004e4810 0x004e4900 -p atv`.

- **The stunt clock MD248 ran is the remote bike's.** The trick step clears the held word. With `+0x48c`
  set, it answers whatever vt `+0x4e8` answers. Nothing else calls vt `+0x4e8`.
  - On a 0 the lean puts the riding pose in the air and none (0xff) on the ground. This is the same as
    when the local step answers 0 (`handling::update::lean`, already ported).
  - **Correction to MD248's journal:** the phase store sits in the branch's delay slot, so the remote
    clock stores phase 1 on the tick it zeroes. It does not keep the last value below 1.
- **A local bike runs the clock in the trick step.**
  - The clock goes up by dt and the phase is `min(clock / Duration, 1)`. Both are stored, and phase 1
    is stored too.
  - The clock never goes back to 0 here. Only the stunt start zeroes it.
  - The id is "kept" when it is one of the five ids at `+0x13b8..+0x13c8` (backflip, the two 360s,
    wheelie, stoppie) and a torque is applied (the backflip's `+0x1194` or the 360's `+0x11d0`). A kept
    id gets kind 4 and the clock never ends it.
  - Any other id ends once the clock reaches `Duration`: the trick end (vt `+0x4ac`, scoring only) runs
    and the step answers 0.
  - **The hold.** The hold starts when the stunt's kind equals the asked kind (`+0x14ec` low nibble, the
    trick buttons) and the clock is between `min(+0x44, 0.3)` and `+0x44 + 0.04`. First the mark goes on.
    Once the clock reaches `+0x44`, vt `+0x4ec` holds the clip with a small wobble, the held word is set,
    and the clock stops. On the 360 rows `+0x44` is 0.72 and the kind is 0.
- **So a 360 ends only through the 360 step** (`atv_moto::spin_air`, proved in `tests/atv_quad.rs`). The
  step sets the id to 255 on landing, on a touch, or when the yaw drops under its floor. All the while
  the torque is applied, the trick step keeps the 360.
  - Coming round (clip 2 -> 0) calls the trick end, which only scores. The id stays the 360, so the
    rider's record byte stays 44/45 and the rider stays in the 360 state. The phase holds at 1 once the
    clock passes 1.5 s.
  - A second pass past 35° calls the stunt start again (`handling::update::stunt_start`, F31b5g). The id
    is kept and the current id is ≥ 41, so the id is written again. The clocks go to 0 and `+0x1650`
    (the restart ask) to 1. The rider's restart takes the fifth rate (R43), so the clip fades in from
    the saved pose and plays again from 0.
- **The port is different.** The rider (`rider.rs` `spin_state`) and the bike (`game-render`
  `vehicle.rs` `want_with_pack`) hold the 360 only while `spin.clip != 0`. They cross out to the air row
  at the round and back in on the second pass, and they leave the 360 when it comes round, not when it
  lands. The answer to the brief's question is **no**: the port's 360 should not end at 1.5 s. It
  should last longer than it does now (S467).

## How it was proved

Function oracle on state04's RAM image. SLUS-21104's own trick step runs tick by tick, with only the
syscall stubs hooked (`crates/game-sim/tests/mxvatv_stunt_end.rs`):

- `a_turning_360_never_ends_on_its_clock`: with the torque on, all 90 ticks (3 s) answer 1 and the id stays
  45. The clock matches a running sum of dt bit for bit and is never zeroed. The phase is 1 from tick 46.
- `a_360_without_its_torque_ends_at_its_duration`: torque off and R1 held. Ticks 1..45 answer 1 and tick 46
  answers 0. The clock is not zeroed and the step writes no id.
- `only_a_remote_bikes_stunt_clock_zeroes`: with `+0x48c` set, even with the torque on, tick 46 answers 0
  with the clock at 0 and the phase at 1.
- `r1_held_stops_a_360s_clock_at_its_hold`: torque on and asked kind 4. The clock runs to 0.733 (tick 22,
  the first value at or past 0.72), then the held word is set and the clock stays there through tick 90.

`./kit check -- -p game-sim --test mxvatv_stunt_end`: 4 passed.

## Not verified / not 1:1

- **Not watched live.** The trick step was run on a save state with the 360's id, torque word and buttons
  poked in, not seen during a real 360 in PCSX2.
- **The frame order was not run.** The claim that the stunt lasts until the 360 step clears the id rests on
  two things. The 360 step sets the applied word on every turning substep (`spin_air`, proved
  separately). The trick step reads it once a frame. The two were not run together in one frame.
- **Whether players hold R1 through a 360 was not seen.** The 360 arms with R1 alone. If R1 is still held
  0.72 s into the stunt, the original freezes the clip there with a wobble. The wobble's values (vt
  `+0x4ec`) were not checked against anything, and the port has no such hold. This is noted in S467.
- **The hold with no buttons.** With the torque off and no button, the 360 row's kind 0 matches the asked
  kind 0, so the clip would be held at 0.72 s, not ended at 1.5 s. I believe a real 360 never gets there,
  because the torque goes off on the same substep the id goes to 255. That was read, not run.
- **The second clip start's restart was read, not run.** The stunt start is the port's proved one
  (F31b5g). The rider's response to the restart ask is R43's reading.
- **Only state04 and the right 360 (45).** The left one (44) has the same row values.

## Next

S467 makes the port's 360 animation state last as the original's does.
