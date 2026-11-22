# 1 — wreck bike gates (FINAL)

Task S503. 2026-11-22.

## What I did

- `flight_search.rs`: `Gate`, `GatePart`, `GATE` (the category 0x200020) and `GATES` (the struck number of
  a gate's part). `Scene::gated` adds each gate as a collidable after the rest and a candidate at the end of
  the nearby list, with the gather's distance (`broad_phase::reach_distance` of its sphere from the ray
  probe's centre). `pair_gate` runs the ray probe's hierarchy case (box gate, then the switched-on parts)
  and the sweep's (sphere, then the parts). A hit on a gate is only itself, standing still.
- `wreck_bike.rs`: `WreckBike::tick_gated` = `tick_ridden` with gates; `tick_ridden` now calls it with none.
- Test `a_wreck_bike_meeting_a_gate_replays_bit_exact` in `tests/wreck_bike.rs`.
- Notes in `research/oracle.txt` (S503 block, after S134's).
- `flight_search.rs` is not in the task's file list. It had to change: the scene the wreck bike searches
  is built there, and every other kind of object (bikes, knocks, wrecks) is added there the same way.

## What the original does

- The wreck bike's nearby list (record `+0x10c`) in every s62 state holds 23 entries: six toys (0x48),
  one 0x20a, one 0x52058, three AI bikes (0x9a048), then twelve race gates (0x200020).
- A gate is a hierarchy collidable: `machineracegate.col`'s two meshes on the gate's frame, under a box.
- Its `+0x31` bit 0 (the nearby bit) is clear on every gate in the five s62 states and in the c33 race
  image. The nearby search refuses a candidate without that bit, so the original lists the gates and
  never tests them.
- Field offsets are in `research/oracle.txt` (S503).

## How it was proved

- Bit-exact replay fixture, in the interpreter: s62 `side-2607`, collision node `0x00e1_7ef0`, the node
  update `0x0026_f800` per tick with each row's dt. Unpoked, it gives `side.csv`'s body bit for bit (checked
  on 5 ticks while exploring).
- The wreck bike's velocity is poked to 60 ft/s at gate `0x016a_f9c0`'s near post. Two runs, 78 ticks each
  (2608..=2685):
  - gates as the original has them (bit clear): the port, ticked with free09's props, the three AI riders'
    bikes and all twelve gates, is the original's body to the bit every tick. The bike flies through.
  - the gate's bit set in the image and `nearby: true` in the port: still bit-exact every tick. The bike
    strikes the gate (2 ticks on an object) and parts from the first run at tick 2644.
- Gates are built from the image: sphere, box, frame, part transforms and switches. The part meshes come
  from the disc's `machineracegate.col`; the test checks the part count and that each part's owner is the gate.
- The whole `tests/wreck_bike.rs` passes (10 tests).

## Not verified / not 1:1

- The proof runs the original in the interpreter on a save state, not a PCSX2 capture of a real wreck into
  a gate: no capture has one, and the original never tests a gate anyway (bit clear).
- The bike was thrown at the gate by poking its velocity, and the gate made testable by poking its nearby
  bit: the meeting is real code on both sides, but not a ride the game makes.
- Gates are read from the image (sphere, box, frame, part transforms). How the game derives them from the
  waypoints is not ported; nothing in the game loop calls `tick_gated` with gates yet.
- The nearby bit was checked only in the s62 states and the c33 race image (free09). Other tracks' gates
  are not checked.
- Gates are put last in the port's list. If the original put tree items after them, the order would
  differ. It cannot change a result while gates are never tested.
- The AI riders' bike transforms are read before each node update. In the original, rider `0x01035d10`
  resolves before the wreck bike in the same node, so its bike has moved by then. It did not matter here
  (the riders are 170+ ft away); a wreck into a moving AI bike is S134's case, not re-proved here.
- The AI riders' bikes are assumed to use the player's bike mesh (`bikef.col`); their sphere radii match.
- No captured wreck meets a gate: the spec's "captured wreck bike meeting a gate" is met only as the
  interpreter's run on a captured state, poked (above). A real capture would show the bike flying through.
- Toys, bikes and gates are handed in together, but in these runs only the gate is struck. Toys alone (S76)
  and bikes alone (S134) are proved by their own tests; a tick striking two kinds at once is not.
- The list's 0x20a and 0x52058 entries (between the toys and the bikes) are not handed in by this task.
  Neither is near the side wreck (1840 and 336 ft), so they leave the run alone.
- The part meshes are compared to the image only by count and owner; the bit-exact strike is the evidence
  they are the same.

## Next

- Nothing filed. Wiring gates into the game loop would change nothing the game does while every gate's
  nearby bit is clear. Do it only if a track turns up a gate with the bit set.
