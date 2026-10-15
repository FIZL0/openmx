# 1 — s1 fixed update (FINAL)

Task S1. 2026-10-15.

## What I did

- Ported the fixed update as `crates/game-sim/src/tick.rs`: `Engine::step(now, pad, game)` is one frame
  of the game, in the original's order — one mode request serviced, the Timer 0 clock sampled, the raw
  delta pushed through a thirty-entry moving average with its 30 fps clamp and its time scale, the pads
  latched, the top-of-frame marker, the simulation, the second marker, and then the bookkeeping the
  frame ends with. Everything arithmetic goes through `ps2-float`'s `F`; the module denies
  `clippy::float_arithmetic` so it cannot quietly stop.
- The other pieces of the frame are types rather than prose: `Timer0` and `clock_ms`, `DeltaTime`,
  `Request`/`Mailbox`/`Serviced` (the mode switch), `Phase`, `Marker`/`Markers`, `Step`, `Outcome`, and
  a `Game` trait that is the seam between this crate's frame and the simulation that will fill it.
  `impl hash::State` for the three stateful ones, so A1's hash covers the frame from today.
- Proved it against the original's own `engine_tick` and `game_tick` running in the oracle:
  `crates/game-sim/tests/tick.rs`, ten tests, 12,000 generated frames and 10,000 clock samples.
- Proved it a second way against C4's 600-frame recording of the game actually running, replayed from
  the clock column alone.
- Named thirteen functions and six globals for the frame in `research/oracle.txt`.
- Corrected two things earlier tasks had half-right (both below, under *What the original does*): the
  engine tick's return value, and the flag byte the end of the frame latches.

## What the original does

`./kit fn 0x00201d00` (the game's tick) and `./kit fn 0x00131100` (the engine's) are the whole frame.
`./kit fn 0x00131800` is the first-frame clock prime, and `./kit dis 0x131100 0x1317f4` is the only way
to see the `dt` sum's shape, which the decompiler tidies into something a port would get wrong.

**The game's tick** is the loop body: a switch on the mode-request word (`app+0x404`), then the engine's
tick, then the quit flag (`app+0x454`). Exactly one request is serviced per frame and the word is
cleared by the case that handled it. Requests 2 and 7 are guarded by the UI manager's still-loading pair
(`ui+0x100`, and `**(ui+0x104)` — a flag *behind a pointer*, which is the half a port drops); a guarded
request leaves the word alone and the frame runs normally. Requests 4 and 6 arm a two-frame delay
(`app+0x578`, `app+0x57c`) and take three frames in total: arm, count, act. Request 7 does not clear the
word, it writes 1 into it, so a restart is a world teardown this frame and a world build next frame.
Request 8 is the only one that returns before the engine tick. Case 3 does not exist: a 3 in the word
sits there forever. On quit the loop flushes the display object and returns false — *after* the frame
has run, so the frame a quit is raised on still simulates.

**The engine's tick**, in order:

1. If `app+0x33c` is zero, prime: sample the clock and write it to all four of `+0x330`, `+0x334`,
   `+0x338`, `+0x33c`, so the first frame's delta is exactly zero and the accumulated clock starts at
   the wall clock rather than at zero. Otherwise sample into `+0x338` only.
2. The clock itself: `di`, read the 100 ms tick word and Timer 0's counter, `ei`, then
   `ms = count * 256000.0 / 147456000.0 + (float)(ticks * 100)` in single precision, converted back to
   an integer with the unsigned split the EE needs (`< 2³¹` one way, `(x − 2³¹) | 0x80000000` the
   other). Both the multiply-divide and the `ticks * 100` unsigned-to-float conversion are chopped
   single precision, so the millisecond value is *not* `ticks * 100 + count / 576` for large clocks.
3. `dt_raw = (now − prev) * 0.001`. If that is over 0.1 s, the history is thrown away (`primed = 0`) and
   the frame is treated as 0.033 s — this is what a level load leaves behind. `dt_raw` is stored at
   `app+0x358` whichever branch ran.
4. If the ring is not primed, fill all thirty entries with this frame's value. Then store it at the
   current index and advance the index modulo 30.
5. Sum the ring and divide by 30. The sum is a strict left fold, unrolled by six — `add.s` in source
   order — and that order is the answer: thirty chopped adds of thirty *identical* 0.033s do not give
   0.033 back, they give 0.032999992, so a port that short-circuits the all-equal case diverges from the
   first frame.
6. The 30 fps clamp (`fps30_cap`, set by the game object's constructor and never cleared): if the mean
   is over 0.03333, it becomes 0.03333 and the frame's millisecond step becomes 0x21. Then the time
   scale, if a time-scale object exists and the mean is positive: `dt += scale * dt`, floored at 0.001
   (which also forces the millisecond step to 1), otherwise the step is scaled too.
7. `app+0x330 += step` — the accumulated *simulated* clock, which is not the wall clock once either of
   those has bitten.
8. The device manager's per-frame update, handed `dt` in `f12`: this is where the pads are read, and it
   happens before anything simulates.
9. Marker 1 through the application's dispatch table.
10. **The gate**: bit 3 of the byte at `app+0x324`. Clear means simulate. Inside it: phase = 2, store
    `dt` at `+0x344` and `1/dt` at `+0x348` (or 1.0 when `dt` is not positive), bump the frame counter
    at `+0x374` and wrap it to 0 when it passes 0x7ffffffe, then call the scene root's simulate pass.
    **If that returns zero the function returns immediately** — no second marker, no render, and none
    of step 11 either, so the next frame measures its delta from the frame before the halted one. Phase
    is left at 2. Otherwise: phase = 0, marker 2, render, the boot countdown, the display ring.
11. The IOP command flush, the optional profiling block (`app+0x3c0`, zero in this build), then
    `+0x33c = +0x338` and `+0x334 = +0x330`, and finally: if bit 4 of the byte at `app+0x325` is set,
    set bit 3 of that byte and clear bit 4.

Two corrections to earlier tasks:

- R3 recorded the engine tick's return value as the loop's exit condition. It is not: `MXGame::Tick`
  discards it and returns on the quit flag alone. A simulation that asks to stop therefore costs one
  rendered frame and one stale clock, and the loop goes round again.
- The latch in step 11 looks like the counterpart of the gate in step 10 and is not: the gate is bit 3
  of the byte at `+0x324`, the latch is in the byte at `+0x325`, one address along. Bit 3 of *that* byte
  is read in four other places in the image (`./kit fn 0x00115540` has one, and a game object at
  `0x00113280` saves and restores it), none of them in the frame. So the end of the frame raises a
  deferred flag that gates work elsewhere, one frame after it was asked for, and the simulation gate
  has nothing to set it. The port models them as two separate fields (`frozen`, and
  `suspended`/`suspend_requested`) for that reason.

## How it was proved

**A function oracle over the frame itself** — `crates/game-sim/tests/tick.rs`. The frame is not pure, so
two things were needed to call it at all, and both are in the test's module docs:

- *Four hooks* for the calls the frame makes through its own pointers (the device-manager update, the
  marker notify, the scene root's simulate pass, the render), reached through a scratch application
  object the test builds at 0x800000, plus no-op hooks on the two interrupt guards, the display ring,
  the IOP flush and the eleven mode handlers. The hooks record their arguments and nothing else; this is
  also the only way to see the `dt` the simulation is handed, which the frame stores nowhere on a halted
  frame.
- *One patched instruction per clock read.* Both clock samples form Timer 0's address with
  `lui $2, 0x1000`, and `0x10000000` is a register the interpreter refuses. The test rewrites only that
  `lui`, to point at a word it owns, and asserts the original encoding first so a different build fails
  loudly. Address formation changes; not one float instruction does.

What that buys, test by test:

| test | what it covers |
|---|---|
| `the_frame_matches_the_original` | 200 sequences × 60 frames = **12,000 frames**, comparing clock, raw dt, all 30 ring entries, index, primed flag, dt, 1/dt, frame counter, accumulated and previous clocks, phase, the deferred flag byte, the dt handed to the pads and to the simulation, and the markers in order. Over 30 and 60 fps clocks, jitter, 0.2–20 s stalls, the cap on and off, slow motion, fast forward, a scale past the floor, and a clock starting just short of 2³¹ ms |
| `the_wall_clock_matches_the_original` | **10,000 clock samples**, including tick counts whose `×100` overflows into the unsigned conversion |
| `the_first_frame_primes_the_clock` | dt exactly zero, step zero, accumulated clock seeded from the wall clock |
| `a_frozen_frame_keeps_time_but_does_not_simulate` | the gate bit: clock and pads yes, simulation, counter, second marker and render no |
| `a_halted_simulation_leaves_the_clock_where_it_was` | the early return, and the 66 ms delta the *next* frame then measures |
| `the_deferred_flag_is_raised_at_the_end_of_the_frame` | bit 4 → bit 3, one frame late, frame unaffected |
| `the_quit_flag_ends_the_loop_after_the_frame` | the frame a quit is raised on still runs |
| `the_frame_counter_wraps_where_the_original_wraps` | 0x7ffffffe, then 0, then 1 |
| `the_phase_is_two_only_inside_the_simulation` | sampled from inside the simulation hook |
| `the_mode_requests_are_serviced_the_way_the_original_services_them` | every request: 1, 2 (both halves of the guard, then the six handlers in order), 5, 4 and 6 (arm, count, act), 7's chain into 1, 3's nothing, 8's shutdown — each frame checked against the port's `Serviced`, mailbox and whether the frame ran |

**A recording of the game running** — `the_recorded_frames_replay_bit_for_bit` replays C4's
`context/captures/timing-menu-600.csv` (600 consecutive engine frames at the main menu, 0 torn reads)
through `DeltaTime`, seeded from the first row's ring. The clock column is the only input; dt, raw,
every ring entry, the index and 1/dt are compared bit for bit on all 599 following frames and match.
That is the proof the oracle cannot give, because the oracle's clock is a word this test writes and the
console's was a real timer. The capture is game data and git-ignored, so the test skips with the
`./kit record` line that remakes it.

`./kit check`: 114 tests pass.

## Not verified / not 1:1

- **The timer hardware.** The two clock reads were repointed at a test-owned word, so nothing here
  proves Timer 0's rate (BUSCLK/256 = 576,000 Hz), its TARGET of 57,600, or that INTC handler 9 is what
  bumps the 100 ms tick word. Those come from R3's reading of the timer setup; the port takes the
  256000/147456000 ratio from the frame's own instructions, which *is* proved. The capture agrees with
  them end to end, which is evidence and not proof.
- **The eleven mode handlers are hooked**, so what a transition *does* — building a world, tearing down
  a page group, committing the settings' event — is entirely unproved. What is proved is which handlers
  a request calls, in what order, on which of its frames, and what the request word holds afterwards.
  They are also where a world load's 19 seconds go, and the port models that as time passing between
  two `step`s rather than as anything inside the frame.
- **The render is hooked and not ported**: marker 2, the four scene passes, the flip, the display ring,
  the IOP command flush and the boot countdown are all outside this task. The frame's *order* around
  them is proved; nothing about their contents is.
- **The profiling block never runs.** `app+0x3c0` is zero in this build and the test leaves it zero, so
  the second clock sample and the three profiling accumulators at `+0x3a4`/`+0x3a8`/`+0x3ac` are not
  ported and not exercised.
- **Nothing found so far sets the simulation gate.** `Engine::frozen` is proved by the test setting the
  bit itself; no code path in the image has been shown to set it, so it may be dead in retail. Kept
  because a port that cannot stop its simulation cannot be debugged.
- **`suspended` is a name, not a finding.** The latch is proved exactly; what bit 3 of that byte gates
  elsewhere in the game was only glanced at (four read sites) and is not ported.
- **The arming frame of request 4 is not reported to the port's `Game`.** The original calls its UI
  flush as it arms and sets a UI flag; the port returns `Serviced::Waiting` and tells `Game::transition`
  only on the acting frame. The oracle test pins the original's behaviour either way, but an app layer
  built on this trait sees one fewer callback than the original made.
- **The time-scale object's layout is taken from the frame's one use of it** (`+0x40` is the factor).
  Nothing has been found that constructs it, so the normal case — no object at all — is the only one the
  capture covers; the scaled cases are oracle-only.
- **`Input` is still a placeholder.** The frame proves the pads are read exactly once, before anything
  simulates, with this frame's `dt` — not what is read. The device manager is S-something else.
- **The fixed update draws no random numbers** was checked by enumerating every `jal` target in both
  functions, which is sound for the frame itself but says nothing about the hooked handlers' own
  callees: a mode handler could reseed the generator and this test would not see it.
- **No live emulator run by this task.** The second proof is C4's recording, made on PCSX2 rather than
  on hardware, so every timing number in it inherits whatever PCSX2's Timer 0 does. S1 started no
  emulator copy and made no capture of its own; a frame-by-frame comparison of `Engine::step` against a
  running game (feeding it the recorded pad and comparing the state hash) is the thing this task has not
  done and A-something should.
- **The first-frame prime triggers on a zero previous clock**, so a game whose clock genuinely reads 0
  on frame two primes twice. The original does the same thing; it is matched, not fixed.

## Next

The frame is a shell with a `Game` trait where the simulation goes, and the next task that ports
anything simulated should implement that trait rather than inventing its own entry point. Two concrete
leftovers: the device manager behind step 8 (the pads, and what `Input` actually is), and a live
frame-by-frame comparison against a running game, which needs an emulator copy and a pad script and is
the one proof S1 could not make from the disc alone.
