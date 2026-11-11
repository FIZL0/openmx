# 1 — ai tick callouts (FINAL)

Task S312. 2026-11-11.

## What I did

The callouts S281 left behind `ai::Steps` (its journal, part 1, lists them) are now ported into
crates/game-sim/src/ai.rs, in a new section "the AI tick's callouts (S312)":

- `ai::update_base`: the base AI's update, the whole tick of a brain that is not riding. `update`
  reaches it through `Steps::other_mode`. `Steps` gained `mode_two` and `mode_four`, which it calls.
- `ai::rescue` / `ai::Rescue`: what the stuck rescue (interface vt+0x40) asks for.
- `ai::hit_reaction` (interface vt+0x3c), with `ai::hit`, `RECOVERY_TIME` and `SWERVE_REPLY`. It sets
  the back-up and swerve timers and draws the swerve bar (one `rand()`).
- `ai::Recovery::count_down`: the bike's countdown of those two timers, in the player update.
- `ai::listener_walk` / `ai::Listener`: which listeners the walk tells, in order.
- `ai::filter_steering` / `ai::SteerFilter`: the brain's vt+0xa8, the step under `tick_flag::EXTRA`. It
  is the steering filter (SteeringRateFilter / SteeringJitterFilter, and a gain lane).
- `ai::habits` / `ai::Habits` / `ai::HabitNode`, with `ai::habit` and `ai::node_flag`: the interface's
  vt+0x28, under `tick_flag::APPROACH`. S281 called this "the target-approach set-up". It is really the
  rider's human touches: the preload into a take-off, the brilliant moment, the landing lag, the start
  gate, the clutch, the wheelie off the start and the cross-up. Up to five `rand()` draws.

None of it is wired into the Sim; that is S12e.

Nothing was taken from opentennis.

## What the original does

Read with `./kit dis` (each is short, and two of them are not Ghidra functions). research/oracle.txt
has a line for each, with its field map: `ai_update_base`, `ai_stuck_rescue`, `ai_hit_reaction`,
`ai_recovery_countdown` (+ `_end`), `ai_steer_filter` and `ai_habits`.

- **Base update** (`./kit dis 0x00282af0`):
  - With an interface, the brain's ground bit goes into its was-on-ground bit, and the interface's
    copy runs.
  - Mode 2 and mode 4 run the brain's own vt+0x98 and vt+0x9c, which give the answer. Every other mode
    answers 0 (3 is spelled out, the rest fall through).
  - Then come the controller (vt+0x90), the interface's vt+0x28 under the interface and 0x200000, and
    vt+0xa8 under 0x4000.
  - The interface's hand-off (vt+0xc) runs without a null test.
  - Last, the listener walk; its answer is ANDed in.
- **Stuck rescue** (`./kit dis 0x00281460`):
  - It reads the app's race object (app +0x3cc → +0x74).
  - With no race object, or one in state 3 (+0x34), the rider's own vtable +0x138 is called with
    (rider, 1, 1).
  - Otherwise it calls the ride-reset request with (rider, 1, 0).
- **Hit reaction** (`./kit dis 0x002812b0`):
  - Kind & 6 sets the brain's 0x100 and both timers to 1.
  - Otherwise, kind & 0x10 writes 150 into the caller's a3 and sets the swerve timer to 1.
  - Either way it draws `2^-31·rand() <= 0.5 ? -1 : 1` into the bar. It returns 1.
  - The direction in t0 is normalised (vrsqrt) into a stack local that nothing reads.
- **Countdown** (`./kit dis 0x0027d5f0`): inside the player update, with s2 the rider and f20 dt, each
  of +0x14c0 and +0x14c8 loses dt when it is over 0.
- **Listener walk** (`./kit dis 0x00133500`):
  - It runs only with obj +0x24 bit 1 set.
  - It walks the list at +0x18 (next at +0x14).
  - A listener is told when its +0x29 byte has bit 0 set and bits 2 and 3 clear, and its +0x20 word
    has bit 1 set. Telling it calls vt+0x38 with dt.
  - It returns 1.
- **Steering filter** (`./kit dis 0x00286b90 --limit 80`): the lanes are written out in oracle.txt.
- **Habits** (`./kit dis 0x00280070 --limit 620`): `ai::habits`'s doc follows its order step by step.

## How it was proved

Function oracles against the PS2 ELF, each over 10,000 generated cases with branch-coverage counts.
They are in crates/game-sim/tests/ai.rs, section "S312":

- `the_base_update_matches_over_ten_thousand_ticks`:
  - Every call out is hooked, as in S281's tick test.
  - Compared: the calls in order, the answer, the flag word, and the six fields the scripted calls
    write.
  - Coverage: no interface 2578, mode 2 2247, mode 4 2257, approach 1860, extra 2517, true 2463.
- `the_stuck_rescue_matches_over_ten_thousand_calls`:
  - Compared: which of the two was called, and its three arguments.
  - Coverage: no race 2508, state 3 2842, reset 4650.
- `the_hit_reaction_matches_over_ten_thousand_hits`:
  - Compared: the timers, the bar, the brain flags, the reply, `rand_state` and the answer.
  - Coverage: back up 4677, swerve only 2471, neither 2852, left 3490, right 3658.
- `the_recovery_countdown_matches_over_ten_thousand_ticks`:
  - The countdown runs as a stretch, from its first pc to a hook on the next pc.
  - Coverage: back-up counted 1190 / held 8810; swerve counted 1166 / held 8834.
- `the_listener_walk_matches_over_ten_thousand_walks`:
  - Up to 8 listeners. Each listener's vt+0x38 is a hook that logs (this, dt).
  - Coverage: not listening 3467, empty 782, told 4784, passed over 5206.
- `the_steering_filter_matches_over_ten_thousand_ticks`:
  - Each lane's move is set at the jitter, at rate·dt and 1 ulp either side.
  - Compared: +0x130 and +0xf0, all four lanes.
  - Coverage: up gained 6598 / passed 6769 / clamped 4100; down 5875 / 2624 / 4034.
- `the_rider_habits_match_over_ten_thousand_ticks`:
  - Compared: all 47 floats, the second flag word, the preload's firing word, `rand_state` and the
    answer.
  - The track's spot (`path_spot_point`) is a hook that plays a scripted point and checks its
    arguments.
  - 20 branch counts, each at least 50.

`./kit check -- -p game-sim --test ai`: 29 tests passed. The S312 counts were read from
context/notes/check.log, so they ran and did not skip.

**The gate**:
- `./kit check --all` is red in one file I did not touch. `mxvatv_car_frame`'s
  `all_six_pieces_are_mx_unleasheds` and `all_six_pieces_with_the_tire_step_are_mx_unleasheds` fail
  with "preload: fired, degenerate seen only 17 times" (they want 20).
- It is red on this branch's base too. I put research/oracle.txt back to HEAD's and it was still red;
  nothing else I changed is read by that test.
- Main has since rewritten the file (MD127, cb1a898c3). I cherry-picked it to try: rerere
  resolved the conflicts to main's own atv_car.rs and test file, byte for byte. It was still red:
  17, 18, and 19 for main's new `all_eight_pieces_are_mx_unleasheds`. So I took it back out
  (`git reset --keep`); this branch carries none of it.
- Filed as MD132. `kit done` refuses on this red, so S312 lands `--blocked --after MD132` with its
  work committed.

## Not verified / not 1:1

- **The listeners are not ported.** Only the walk's choice of which to tell is. Which objects sit on an
  AI brain's listener list, and what their vt+0x38 does, is not traced. They are arbitrary objects.
- **Mode 2 and mode 4 are not ported.** The base update calls the brain's vt+0x98 (0x00284de0) and
  vt+0x9c (0x00285090) through `Steps::mode_two` and `mode_four`. Their bodies are not read.
- **The original's null-interface hand-off** (with no interface, `update_base` calls the hand-off
  through a null pointer) is kept as a plain call to `Steps::hand_off`. The oracle points low RAM at a
  vtable to stand in for it, as S281's tick test does.
- **The hit's direction normalisation is left out.** The original computes it into a local nothing
  reads.
- **Answers are bools.** The original's answers are 64-bit words ANDed together. The port keeps bools,
  which agree for the 0/1 answers every callee here gives.
- **The habit field names are guesses** read off their use: the clutch, the brilliant moment, the
  cross-up, the low-air bar and the gate wheelie. I did not check them against the character file's
  key names that S283 reads.
- **Just off the ground, with no cross-up due, the cross-up side is always −1.** The original crosses
  a zero vector there; ported as written and oracled.
- **No live capture.** On free09 the habits step never runs, because APPROACH (0x200000) and EXTRA
  (0x4000) are never set; nor do the other callouts, outside a hit or a stuck rider. Everything is
  proved by function oracles only.
- **The PC exe was not read.**
- **Nothing is wired into the Sim.** That is S12e.
- **The gate is red on base**, in `mxvatv_car_frame` (see above; MD132). It is not S312's.

## Next

- S12e can now give `ai::update`'s `Steps` real bodies from these callouts.
- Filed as S314: mode 2 and mode 4's drivers, and which listeners an AI brain carries.
