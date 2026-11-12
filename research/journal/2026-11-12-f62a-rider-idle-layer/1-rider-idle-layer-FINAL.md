# 1 — rider idle layer (FINAL)

Task F62a. 2026-11-12.

## What I did

- `handling::rider::idle` (vt `+0xd4`): `idle_state` (the state picker into `+0x86e`) then `idle_clips`
  (the 3 KB clip chooser), then `+0x870` kept from the record's `+0x1d8`. New `Call`s: `Seed`, `ClipStart`,
  `ClipAdvance`, `SavedPose`; the controller's stop/blend/play are `anim`'s ports, with a `Saving` adapter
  so `anim::stop` gets its saved-pose buffer through `Call::SavedPose`.
- `anim::blend4` (the four-clip blend: three slerps per bone and the twin for positions, `mix_positions`).
- `anim::rotation` now handles the **narrow** rotation-channel kind (8-byte keys: u8 turn into the narrow
  turn table, 3×i8 axis) as well as the wide one. `Tables` gained `narrow`, the word naming that kind's
  sampler. The rider's idle clips are all narrow. Before this, the first idle-layer blend on ride-2 row 0
  posed the wrong bones.
- tests/mxvatv_bike_ride.rs: `rider_leaf` serves `Call::Idle` with the port. `Seed` is asserted (the
  clock-seeded flag is already set); `ClipStart` / `ClipAdvance` call the original (F62b);
  `SavedPose` goes through the heap with the original's file and lines. The anim oracle
  (`the_anim_controller_calls_are_the_originals`) gained Blend4 and random narrow channels.
- research/oracle.txt `# F62a`: addresses, args, both samplers, counts.

## What the original does

`./kit fn -p atv 0x004eb380` (picker), `./kit dis -p atv 0x004eb6f0 0x004ec32c` (chooser),
`./kit fn -p atv 0x00140720` (blend4), and `./kit dis -p atv 0x0012cac0 …` vs `0x0012c550` (narrow vs wide
samplers). In words:
- When wrecked, the picker holds the wreck pose until the wreck settles, then picks once by lot from `A`
  plus `5u`.
- Otherwise it takes the record's asked state, or the turn, or a slow state while the stick is held.
- When slow and centred, a pause clock is drawn and run down, then the pause state starts; otherwise the
  rider is still.
- The chooser sets the rate from five rates. On a change it stops the controller and sets the fade flag.
- It then fills the two blend slots by state, with clips by id (vt `+0x174`) or from a named set, and
  calls blend4, blend or play, then the controller's clip advance.

See research/oracle.txt `# F62` (the call census) and `# F62a`.

## How it was proved

- **Bit-exact replays** (tests/mxvatv_bike_ride.rs, C15):
  - `ride_replays_with_the_rider_leaf_on_the_ports` (ride-2, 459 rows): the served list has
    `= idle layer` 459, and the census shows no `> idle layer`; RAM is alike.
  - `flip_replays_with_the_rider_leaf_on_the_ports` (flip-1, 465 rows): `= idle layer` 465; the only
    outside idle calls left are `> clip advance` 68 and `> clip start` 2 (both F62b), plus `> rand` 7
    (which F31b5i accepts); RAM is alike.
- During the work, a temporary probe compared the port with the original at the idle-layer boundary on
  all 459 ride-2 calls. Both the blend4 arguments and the RAM (stack aside) were identical. The probe has
  been removed.
- **Function oracle** `the_anim_controller_calls_are_the_originals`: generated cases over all 8 controller
  calls, now with Blend4 (two clips arranged four ways across the slots). Rotation channels are narrow at
  random, each sampled through its own vtable. The required-path list includes every Blend4 path and
  "Rotation on … narrow". It passes.

## Not verified / not 1:1

- **Picker paths C15 never reaches**:
  - the record's asked state byte (`rec+0x1d4`), `rec+0x1d0`, the turn ask (`rec+0x130`);
  - `R_HELD` / `R_LEFT` (the stick-held slow states and their fmodf lap);
  - the pause clock's draw and run-down and the pause state with its end;
  - the respawn slot reset.

  These are ported from the disassembly but no capture exercises them. Across both captures `+0x86e`
  only takes `0x38`, `0x3d`, `0x29`, `0x40`, `0x43` (F62's count).
- **Chooser paths C15 never reaches**: the named clip-set path (`+0x874` is always -1 on C15), the off
  states' own clip by id, and the default `n = 0` states. Ported from the disassembly but unexercised.
- **`Call::Seed`** is only asserted: the test checks that the seeded-clock flag is already non-zero. The
  original's clock seed (what happens when it is 0) is not ported. C15 calls it once, on flip-1.
- **`Call::SavedPose`** is never called on C15 (0 times). Its heap file/line answer is from the
  disassembly only.
- **blend4** is proven by the oracle, but with only two distinct clips arranged over its four slots, not
  four independent clips. The replays add 23 (ride-2) and 36 (flip-1) real calls.
- **Narrow keys** are proven by the oracle (random narrow channels) and by the replays. Turn indices are
  limited to what the oracle's generator draws.
- Nothing was taken from opentennis.

## Next

F62b: the clip advance (vt `+0x88`) and the wreck states' clip start, after which flip-1's rider census is
only rand/heap/cue.
