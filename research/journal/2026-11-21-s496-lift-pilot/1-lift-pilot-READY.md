# 1 — lift pilot (PART)

Task S496. 2026-11-21.

## What I did

- **New crate `crates/game-lift`.** `Mem` is a flat byte array addressed by the original's own pointers,
  little-endian, an address taken modulo its size. `Mem::new` is the EE's 32 MiB. `Mem::from_ram` loads a
  RAM image. `Mem::sized` gives a small one for a bridge. Floats go in and out as raw patterns through
  ps2-float's `F` and `Vf`, so `lwc1`/`lqc2` behave as on the EE.
- **Lifted, close to line for line from the disassembly, each over `Mem`:**
  - the start gate's crossing test (`gate::crossed`; `./kit fn 246720`);
  - the AI brain's ground controller (`ai::controls`; `./kit fn 283ef0`), with the arc cosine it calls
    (`ai::acos`; `./kit fn 2849d0`) and the curve-table sine and cosine it inlines.
  - Each keeps the original's compare order and both of its duplicate compares: the twice-clamped arc
    cosine argument, the twice-wrapped angle, and the landing-height test that is asked twice. Each also
    keeps its stores, including the ones a clamp makes only when the value is outside.
- **No retail global in the lift.** The controller reads two startup-built tables. It gets their places
  as `ai::Tables` from its caller. The oracle test fills `Tables` from `ps2_oracle::addr(...)`. The bridge
  lays the sim's own tables (the game's to the bit) at addresses it picks itself.
- **The bridge (`game_lift::bridge`, 144 lines).** It lays the sim's `Situation`/`Tuning` out as the brain
  object in a 128 KiB `Mem`, runs the lift, and reads `Controls` back. It does the same for `race::Gate`
  and a step past it. The sim gets one hook each, `game_sim::ai::LIFTED` and `game_sim::race::LIFTED`
  (7 lines each, unset by default). The shell gets `--ai N` (11 lines): a race on Supercross01 (a later
  `--track` moves it) with N AI riders, with both hooks set. Without the flag, nothing changes.
- **Result.** `openmx --ai 1 --laps 1 --race-seed 0x3270042a --headless --steady-clock --frames 6000`:
  - The gate drops at tick 90.
  - The one AI bike laps Supercross01 and finishes in place 1 at 66.36 s, with the human idle on the grid.
  - With `--race-bot` (the human on a brain too), all 38 `race:` lines match the port's run
    (`--race --racers 2 --track sx01`, same seed) line for line.
  - I checked that the lift really runs under `--ai`. I made its full-throttle store write 0.9: the race
    then ends 70.42 s / 81.90 s instead of 66.36 s / 73.77 s. Then I restored it.

## Hours per function, and against the port

Wall clock from the runner's start (01:26:51) and the commits. This is an agent's time, not a human's.

| function | lift | proof failures | port task (from plan/DONE.md and run.log) |
|---|---|---|---|
| crate + `Mem` + gate crossing (asm 0x170 B) | ~4 min (01:27–01:31) | none | S13a ported `Gate::crossed` as part of the activity class: 1078 min wall, but that includes a long stall at a prompt, so it is not comparable. S13b (drop and watch): 112 min, 321 calls |
| ground controller + arc cosine + curve sine/cosine (asm 2772 B + 428 B) | ~4 min (01:31–01:35) | one: a debug-build overflow panic (`8 * k` on an index made from a wild input), not a wrong bit; fixed with wrapping arithmetic, as the EE's `sll`/`addu` do | S12b: 98 min, 214 calls, +281 lines in ai.rs, +301 test lines |
| bridge + hooks + `--ai` + race runs | ~13 min (01:35–01:48) | none (both bridge tests passed first time) | none: the port needs no bridge, but its own types are the same work in another place |

What the comparison does **not** show:

- The lift came after the port. S12b and S13a had already found these functions, named every field, and
  written research/oracle.txt. I read their notes. A lift of an unknown function would cost the finding
  as well.
- These are the leaf functions with the least dispatch. The controller is called through MXVehicleAI's
  vtable slot, and the lift calls it directly.

Sizes:

- The lift is 228 lines (controller, about half of it doc and helpers) plus 95 (gate). The port is
  67 + 49 for the same two functions, plus types.
- The bridge is 144 + 14 (hooks) + 11 (shell) + 4 (Cargo) = 173 lines.

## What the original does

- **Gate crossing:**
  - A null end answers 0. A gate without flag 1 answers 4.
  - Out of reach of the centre (when there is a reach) answers 0, and so does a step of no length.
  - `t` is the fraction back along the step where the gate's plane falls. Outside 0..1 answers 0.
  - A plane gate (0x10) answers 1 there.
  - Otherwise the point on the plane is measured across the gate as a fraction of half the width, and up
    against the height less `across.y × side`. Outside either answers 2. Flag 8 (left half only) answers
    2 for a negative side.
  - Inside: flag 4 (either way) answers 1. Otherwise 1 only if the step goes the normal's way.
- **Ground controller:**
  - The target speed is raised to the minimum.
  - The forward is flattened (w zeroed) and made unit on VU0.
  - Three signed angles go through the arc cosine: the heading off the target, the aim, and the roll.
  - The bar is the sine of the heading (less yaw × oversteer gain with 0x10000), clamped to ±π/2.
  - The lean is 0 on the ground (0x20). High in the air it is the sine of the roll-rate correction
    against the aim. Near the ground it is a filtered push toward the landing attitude.
  - Both are scaled and clamped to ±1.
  - Throttle and brake come from the speed against the target, with the slow/fast tolerances, cruise
    (0x10000000) and graded brake (0x20000000), and are clamped to 0..1.
  - It returns 1.

## How it was proved

- **Function oracles, RAM in and RAM out** (`crates/game-lift/tests/lift.rs`):
  - `the_lifted_gate_crossing_matches_over_twenty_thousand_steps`: 20,000 generated gates and steps,
    ELF-only RAM. The same words go into both memories, and the answer and the gate's 0x80 bytes must
    match. Every answer code comes up more than 300 times.
  - `the_lifted_ground_controller_matches_on_the_three_riders_as_they_are`: the recording2 free09 image's
    three AI riders, untouched. Bytes 0..0x400 of each rider must match.
  - `the_lifted_ground_controller_matches_over_ten_thousand_cases`: on that image, every float the
    controller reads is redrawn, plus the flag bits. One case in five carries stray words (denormals,
    Inf/NaN patterns, signed zeros, any bits). Each case must match on the rider's 0x400 bytes and the
    answer. Each path (ground, high air, near ground, near ground with strays) comes up more than 200 times.
  - I checked that it bites: a 1.0001× error in the cosine fails at case 6, by 1 ulp in the lean.
- **Bridge against the port** (`crates/game-lift/tests/bridge.rs`): 20,000 random `Situation`/`Tuning`,
  where all 12 output words must match `game_sim::ai::controls`, and 20,000 random gates and steps against
  `race::Gate::crossed`. I checked that it bites: two swapped tuning offsets fail it.
- **Shell:** the `--ai 1` race above, line for line against the port's.

## Findings for the decision

- **The AGENT.md exemption.** `./kit audit` flags only addresses listed in research/oracle.txt or kit.toml
  `[symbols]`, plus `FUN_`/`DAT_` names. It does not flag struct offsets (`ai + 0x1e0`), and it flags
  nothing in game-lift today. Offsets already appear in race.rs and ai.rs doc comments. So the gate does
  not need the exemption. What the rule's wording ("bare hex addresses") means for code made of offsets
  is the user's call. I did not edit AGENT.md (the brief says to ask first).
- **Globals were the real conflict, and parameters solve them.** The controller's retail globals (the
  curve table pointer, its scale and step, the fine arc cosine) come in as `Tables` from the caller. The
  oracle passes the retail addresses, and the bridge passes its own. The same works for any lifted
  function that reads globals.
- **Vtables are the next conflict.** A lifted function that calls through a vtable needs either the
  original's objects and vtables in `Mem` (retail addresses) or a dispatch table the bridge fills. Neither
  is in this pilot.
- **The bridge costs more than the lift.** For two leaf functions it is 173 lines against 323 lifted. The
  bridge shrinks per function only if whole subsystems are lifted and share one `Mem`.

## Not verified / not 1:1

- **The AI bike's physics, route follower and AI tick are the port's, not lifted.** Only the ground
  controller and the gate crossing run lifted under `--ai`. Lifting the whole AI bike (bike body, ride,
  route follow, AI tick) into one arena would cost what S12/S13/S281/S282 cost together. That is beyond a
  pilot and is the user's go/no-go.
- **The race manager tick and the drop are not lifted.** The gate's drop and watch (S13b) are the port's.
  The drop calls through the activity's vtable.
- **`--ai N` hooks are process-wide.** They hold for every sim in the process, the human's brain under
  `--race-bot` included. They are off by default. The lift and the port agree to the bit, so this changes
  no outcome, but it is a global.
- **The time comparison is uneven**, for the reasons in the table above: the lift came after the port's
  research.
- **S231, S232 and S233 cannot be compared.** They are still open umbrellas in plan/TODO.md, with no port
  times.
- **`Mem` addresses wrap modulo its size.** The EE maps the scratchpad (0x70000000) apart from RAM. The
  lifted functions here never touch it, but a future lift that does needs its own segment.
- **The browser and Android builds** link game-lift through openmx but were not run with `--ai`.

## Next

The user decides on the pick sheet (S496.json): whether to go on lifting (and how far), the AGENT.md
wording for game-lift, and how lifted code gets vtables. Then either land the pilot as it is, or file
the next lift (the AI tick and the bike) as a task.
