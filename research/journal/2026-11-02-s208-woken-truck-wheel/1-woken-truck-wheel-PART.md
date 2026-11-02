# 1 — a woken truck's mesh is in the wheel's reach (PART)

Task S208. 2026-11-02.

## What I did

- **The brief's first check, answered.** It asked whether a woken HasPhysics truck's physics mesh has
  the nearby bit. S168 found the sleeping mesh's bit clear, so the wheel's search (which asks the pair
  filter with the nearby bit) refuses it. **A woken truck's mesh has it set.** So the wheel's owner
  branch is reachable on a woken truck, and the wiring S208 asks for is real work, not a no-op.
- New test in `crates/game-sim/tests/bike_col.rs`: `a_woken_trucks_mesh_is_in_the_wheels_reach`.
- `ride.rs`: one doc comment on `wheel_contact_velocity`. It pointed at S169 for the woken truck; it
  now says S208 proved the mesh is in reach and S186 is the missing simulation. No code changed.
- `research/oracle.txt`: an S208 comment block under S168's. No new name lines.
- **Not wired.** The port has no woken truck. A struck HasPhysics truck still drives on as a path car
  (S207's ctruck, B8), and its body, the f.col mesh at the body's pose and the settle are all S186
  (priority later). With no body there is nothing for `contact::other_at` to read, so putting the mesh in
  `wheel_hits` now would add a collidable that is never stood up. Blocked `--after S186`.
- Nothing taken from opentennis: it has no counterpart.

## What the original does

- `physics_car_wake` (S127's name) is the truck's struck listener. I ran it in the EE interpreter on
  each of s89-along's two trucks (free06 ctruck_03 / ctruck_02):
  - `+0x588` (physical) goes 0 → 1;
  - the physics object's `f.col` mesh (`+0x698` → `+0x128`): `+0x31` gains bits 0 and 1
    (0x00 → 0x03, 0xd0 → 0xd3), and `+0x116` gains 0x10;
  - the car's own path-car mesh (`+0x428`): `+0x31` loses bits 0 and 1 (0xc3 → 0xc0, 0x33 → 0x30).
    It is stood down, as S127's notes say.
- After the wake, `collision_pair_filter` accepts each of the four riding bikes' wheel probes
  (`+0x7c8`) against the mesh with the nearby argument 1 as well as 0. Before it, the answers were
  `[true, false]` (S168).
- The mesh's collision object (`+0x80`) has `+0xf4` = the physics object's body (`+0x110`) and
  `+0xf0` (chassis) = 0. So a wheel on it takes the contact-velocity step's owner branch on a dynamics
  body: `contact::other_at` with `Side::Body`, the same as a collection prop (S169).
- The ELF has six `sb …, 0x31(…)` stores. `0x00215a90` sets bit 0 on a list of collidables
  (`+0xb0` → `+0x300`/`+0x304`), and `0x00215b30` clears it; both are reached only from vtables. I did
  not trace which one the wake's body-on (physics object vt+0x11c) reaches. The interpreter run is the
  proof, not the reading.

## How it was proved

- `a_woken_trucks_mesh_is_in_the_wheels_reach`, on s89-along, a function oracle on a real RAM image:
  - finds both trucks by `physics_car_vtable`, and the riding bikes as S168's test does (4 found);
  - checks the mesh's owner names the body and no chassis;
  - checks the truck is asleep in the state, and the bits and filter answers before the wake;
  - runs the original's wake, then checks `+0x588`, both meshes' nearby bits, and that the filter now
    answers `[true, true]` for every bike's wheel probe.
  - The before-and-after asserts are the mutation check: the same filter call must change its answer
    across the wake. It ran (its `--nocapture` lines: 2 trucks, 4 wheel probes each), not skipped.
- `cargo test --release -p game-sim --test bike_col`: 5 passed.

## Not verified / not 1:1

- **Not wired, so a ride still never stands on a truck.** The port has no woken truck (S186). The
  wheel's owner word stays 0 on a truck, as it does on everything in the port today.
- **Not seen live.** The wake ran in the interpreter on a RAM image, not in a capture. Whether a struck
  truck's mesh also gets into a bike's nearby search (the broad phase, `+0x7bc`) at the body's pose was
  not checked: in S207's ctruck capture the bike is thrown off the truck, and no state after the strike
  was saved. S186's capture of a bike striking a free06 truck is the place to check it.
- **The settle was not run.** `physics_car_settle` stands the meshes back up after ResetTime; I did not
  run it to see the f.col mesh's bit cleared again.
- **Only free06's two trucks.** The other tracks' HasPhysics cars (free07, free09 Car7, free10, free11)
  share the class, so they should be the same, but no state of them was read.

## Next

After S186 lands (a woken truck with its body and f.col mesh at the body's pose): put that mesh in
`ride.rs` `wheel_hits` with its owner, set the wheel's owner word, and read `under()` through
`contact::other_at`'s body side, as the brief says. Then prove it against S186's live capture, where
the bike's wheel meets the woken truck.
