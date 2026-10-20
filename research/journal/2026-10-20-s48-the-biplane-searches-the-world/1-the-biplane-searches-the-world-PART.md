# 1 — the biplane searches the world (PART)

Task S48. 2026-10-20.

## What I did

- Checked the four prerequisites against main (a2054d0) and the task branches: `task/S45`, `task/S46`
  and `task/S47` are checked out in slots s2, s3 and s4 and have no commits past main; `task/S41`
  (origin) has none past main either, and plan/TODO.md has S41 as `[~]`, waiting on S38.
  `crates/game-sim/src/flight.rs` has no substep loop yet (`Flight::frame` runs the controls' preforce
  only), so there is nowhere to call `contact_search::collide` before `contact::resolve`, and no
  `contact_search::Search` impl can be built without the broad phase (S45) and the two narrow phases (S46,
  S47). I wrote no code: a stub `Search` or a listener without the ride to run it in would be unproven
  work that S45-S47's merges would conflict with.
- Read the plane's ray-probe hit listener (below), so the next session starts with it.

## What the original does

- `./kit dis 0x0015acf0` (plane_probe_hit_listener), reached through the thunk `./kit dis 0x0015c290`,
  which subtracts 0x48 from `this`. The listener sits at plane+0x98 and the rigid body at plane+0x50, so
  `this` after the thunk is most likely the rigid body (the listener is the body's +0x48). It does: if
  the word at this+0x58 is non-zero and the struck collidable's category (+0x84) ANDed with the global
  mask (`./kit state` word at 0x00468f68, 0x40 in biplane.p2m2_SaveState) is non-zero, call
  `(this+0x38)->vt[+0x124]` with `this` still in `$a0` (no other argument is set up).
- Lead, not proved: if `this+0x38` is the plane's +0x88 vtable (0x0045b0ac, from biplane_ctor in
  oracle.txt), then vt +0x124 is `./kit fn 0x15b790`, which, when a pointer at +0x110 says so, resets
  things at +0x138's +0x50/+0x68/+0x98/+0xb0 and calls a virtual on +0x110's +0x100 vt +0xc with 1.
  Which object `this+0x38` really is was not checked against the save state's live plane.

## How it was proved

Nothing was ported, so nothing was proved.

## Not verified / not 1:1

- The whole task: not started, because S41, S45, S46 and S47 have not landed (see above).
- The hit listener's `this`, and so its vt +0x124 target: worked out from the thunk's -0x48 and the
  ctor's layout notes, not read from the live plane in the save state.
- plane_probe_sound_listener and bike_struck_listener: not read.

## Next

Once S41 (the substeps), S45, S46 and S47 are on main: read the plane's ray-probe listener `this` from
biplane.p2m2_SaveState (plane+0x98's listener, +0x38's vtable) to confirm the vt +0x124 target above,
then port the listeners, build `contact_search::Search` on `world_search::search_nearby` /
`probe_collide_world` with S46/S47's narrow phases, call `contact_search::collide` before
`contact::resolve` in each substep, and raise BIPLANE_AGREES_THROUGH.
