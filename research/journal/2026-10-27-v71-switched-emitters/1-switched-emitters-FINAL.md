# 1 — switched emitters (FINAL)

Task V71. 2026-10-27.

## What I did

- `particles.rs`: `Emt` now reads `EmitMode`, `DelayTime` and each mode's times the way emt_read
  does: only the emitter's own mode's keys, and every one 0 when missing. A new `Gate` is the
  original's emit gate together with its emitter's on, off and reset. All four modes are ported,
  with the arithmetic done the PS2 way (`ps2_float::fpu`). An `Entry` that is `held` (a track
  emitter) asks its gate whether to emit. The vehicles' entries keep their `lit` path untouched,
  because every `.emt` they name on the disc is mode 0 with no delay, and for that case `lit` and
  the gate do the same thing. I also left it alone because bike_particles.rs seeds `lit`.
- `TrackFx` now runs every plain emitter the `.toy` lists. That includes the start-off ones and the
  ones attached with `AttachToToy`. Only emitters on a scene object are still left out, and no
  track lists one. The toy reset happens at load: each gate is reset, then switched on or off by
  `StartOn`. New calls:
  - `switch`: the toy's on/off message.
  - `fire`: the trigger's particles action, a reset and then on.
  - `finish`: switches on every `Fireworks%02d` from 01 up to the first missing number.
  - `reset`: the race's reset.
  - `touch(riders)`: the `.toy`'s triggers, with the toss on entry and delayed actions.
  - `place(node)`: emitters that ride a toy go to their toy node's world place. Offset and rotate
    work as the toy link does (`RotateWithObject` defaults to 1).
- `toy.rs`: `Emitter` gains `toy`, `sub_object`, `offset` and `rotate`. A new `particle_triggers`
  reads the `[TriggerN]` sections and their `particles` actions. Sound actions are not read.
- `animated_toy.rs`: `Animated` keeps each toy's name and its bone names.
- `main.rs`: the triggers are built from the `.toy`. Each frame, the riders' ride positions go to
  `touch`, and the toy emitters are placed on the first seat's animated toys (FinishGate's
  `FireworksDummy01..06`, the joint at the last tick, times the toy's place).
- On the disc, `EmitMode`/`DelayTime` are non-zero only in the fireworks_*.emt (delays 0.75 to
  2.8 s, shots 0.1 to 3 s) and nat12's GeyserBlast/GeyserSpray/MistPuff (delay 0, shot 15 s). Nothing
  uses mode 2 or 3. fireworks_smoke.emt is mode 0 with MinimumEmissionSpeed 1. The brief's
  "24/30/43" are the counts: 24 fireworks on the gate, 30 in sx01's list, 43 in sx04/sx20 (13 of
  those are water mists).
- Follow-ups filed: S186 (the race calls `finish`/`reset` and restarts the FireworksAnimators) and
  V102 (V44's creature triggers toss every tick; the original tosses on entry).

## What the original does

The full trace is in research/oracle.txt § V71 (`./kit fn 0x0014daa0`, `./kit dis 0x0014e490`).

- **The gate**, called at the start of each frame's emit:
  - It returns "nothing" while the pause flag is set.
  - It follows the node, if the emitter has one.
  - If the emitter was just switched on, it only takes its point that frame.
  - Then the delay counts down, but only while the emitter is on.
  - Then the view cull runs.
  - Then the mode decides. Mode 1 (one-shot) emits while time is left, including the frame where
    the time crosses 0. After that it switches itself off, but only if this shot made a particle;
    if it made none, it behaves as mode 0. Modes 2 and 3 alternate between on and off times, random
    for 2 and fixed for 3. Mode 0 emits while on.
- **Emit** (`0x0015084c`): sets the shot bit when the frame's count is above 0 in mode 1.
- **Emitter toy**:
  - On/off message: no reset.
  - Reset: emitter reset, then `StartOn`.
- **Trigger particles action**: reset then on, or off if `TurnOn` is false.
- **Trigger contact**: a trigger set below 100 % tosses only when nothing was inside it the frame
  before. Its actions skip an actor that was already in contact. A `TriggerActionDelayTimer` counts
  down in the trigger's update.
- **Race**: the race finish (when the winner, place 1, finishes) restarts each FireworksAnimator's
  motion and sends each Fireworks toy "on". The race reset sends them a reset.

## How it was proved

- `crates/openmx/tests/emit_gate.rs` `the_gate_is_the_originals`: a function oracle on the boot ELF
  over 20,000 generated emitters, covering a frame, on, off and reset (about 5,000 each). It uses
  modes 0, 1, 2, 3 and 5, random low flag bits, delays and times on both sides of 0, three kinds of
  dt, and the game's `rand` hooked to the same number the port draws. It compares the return value,
  the low flag bits, and the bits of delay, time left and idle. All 20,000 match.
- `a_one_shot_runs_its_delay_then_its_shot`: the fireworks_red shape. One rest frame, 23 frames of
  delay, then about 15 frames of shot, then the emitter is off.
- `tests/track_particles.rs`, run against the disc:
  - `every_tracks_emitters`: every emitter on 13 tracks now runs.
  - `nat12_geysers_blow_when_a_rider_comes_by`: the triggers read as 50 %, not one-time-only, with
    blast/spray/mist actions. A rider in Geyser_a's box makes particles within 2 frames. The emitter
    is on for 15 s, then off. Geyser_b never fires.
  - `the_finish_sets_the_fireworks_off`: nothing happens until `finish`. Every one-shot starts its
    shot after its own delay, makes particles, and is off after its shot. The still fireworks_smoke
    makes nothing until its node moves.
  - `the_finish_gate_has_the_fireworks_nodes`: sx01's FinishGate skeleton has FireworksDummy01..06.
- The release shell on nat12 and sx01 (`--headless --frames 30`) prints both geyser triggers with 3
  actions each and runs the geyser and firework entries.

## Not verified / not 1:1

- **Nothing calls `finish` in a ride**: the port has no race (S13 is a stretch task). The fireworks
  only go off in the test. The FireworksAnimator motion restart is not ported (S186).
- **I never saw it on screen**: no screenshot or capture of a geyser blast or of the fireworks. A
  rider has to ride into a geyser's box on nat12, which I did not do. The timing is proven by the
  oracle and the tests; the look is not.
- **Trigger contact is the rider's ride position inside the box**: the original uses its actor
  collision against the trigger volume, and I did not check which point or shape it tests.
- **The trigger toss uses TrackFx's own Rng, not the game's `rand`**, and the gate's mode-2 draws use
  the fx's Rng. The formulas are the original's, but the sequences are not.
- **The trigger's update and contact list are approximated**: per trigger, "riders inside last
  frame". The 0x001c6f60 call after the actions is not ported. The order of the toss's float ops
  (`int(rand · 2⁻³¹ · 100)`) is read from the decompile and was not oracled.
- **Sound actions** (geyser_shoot) are not ported.
- **The gate parts not ported**:
  - the pause flag;
  - the view cull (+0x248), which the port never culls;
  - the parent-system call (+4).
  The oracle ran with all three off.
- **Placement**:
  - Toy emitters ride the joint at the last tick, not blended.
  - They are placed from seat 0's toys only.
  - An unplaced one (its node not found) has no node and its gate does not run, while the
    original's would run at its fixed point. No track has this case.
  - Node scale is carried into the turn as it is.
- **I did not check whether FinishGate actually animates.** If it stands still, the original's
  fireworks_smoke never emits either, and the port matches that.
- **The shot bit is set from the particles actually pushed**, not from the original's planned count.
  These are the same unless the pool is full.

## Next

`./kit done V71`.
