# 1 — does a wheel on a prop push it (FINAL)

Task R15. 2026-11-02.

## What I did

- **Read the original's wheel code for the owner.** The answer is no push, so there is nothing to port.
- **New test** `crates/game-sim/tests/prop_knock.rs` `a_wheel_on_a_prop_does_not_push_it`.
- **`crates/game-sim/src/prop_wheel.rs`:** the module docs gain point 4, "no push on the prop".
  Docs only, no code.
- **`research/oracle.txt`:** an R15 comment block under S210's. No new names.
- Nothing taken from opentennis: it has no counterpart.

## What the original does

- **Who reads a wheel's owner word (`+0x1f0`) and vehicle word (`+0x1f4`).** I disassembled the
  whole ELF (llvm-objdump) and grepped for loads and stores at those offsets. Inside the wheel's
  own code there are only these:
  - the wheel ctor (`./kit fn 0x0025a730`) and wheel_clear zero them;
  - wheel_set_contact writes them;
  - wheel_spin_integrate's hanging-wheel branch copies them from the other wheel (the port's
    `share_load`);
  - wheel_contact_velocity reads the other body's velocity through them. It does not write the
    other body.
- **Where the wheel's forces go.** wheel_suspension, wheel_tire_forces, wheel_surface_drag,
  wheel_tire_drive_force and wheel_hinge_force all write the chassis, the wheel's `+0x1e4`:
  - its force `+0x130` and torque `+0x140`;
  - the bottom-out `transform_sync` on the chassis transform.
  None of them takes the owner.
- **Who reads bike `+0x7c8` (the wheel probe).** No one resolves its record against the other
  body:
  - the distributor;
  - the loader and builder;
  - bike_physics_lines;
  - an enable toggle (`./kit fn 0x0024fc10`).
- **What bike_wheel_contacts does with an owner** (`./kit fn 0x00257820`, the port's
  `chassis::wheel_contacts`). For each record contact whose collidable has an owner (`+0x80`), it
  calls slot `+0xc` with `(0, 0)` on every element of `*(owner+0x1f8)`'s `+0xf4` list, and only
  then calls wheel_set_contact.
- **What that list holds for the props.** I read it in S195's four knock states, for all 62 props.
  `owner+0x1f8` is the prop's own `f.col`, and the list has two entries:
  - the physics object's listener (object `+0x48`, vtable `0x0044e7f8`). Its slot `+0xc` is a thunk
    to physics_object_struck, which wakes through vt `+0x124` when `+0x58` is set;
  - the collection's listener (vtable `0x0045db04`). Its slot `+0xc` is a thunk to `0x003b2430`,
    which jumps to the collection's vt `+0xa8`, the activate.

  This is the same struck pair that collide_against_search fires when a pair takes the record
  (S210, the port's `PropSet::struck`). Neither listener reads its arguments, and both only change
  something that is asleep. A contact can only reach the distributor's loop if its pair took the
  record in the same call's search, so by then the prop is already awake and its collection
  active.

## How it was proved

- **The test.** Interpreter run on knock-2897, set up as S210's placings: the bike's node stood so a
  wheel segment passes through a prop, every prop × 24 placings with a fresh seed, and the bike's
  search holding all 62 meshes. For each placing that leaves a contact:
  1. collide_against_search (the wake);
  2. a snapshot of every prop's physics object (0x1c0 bytes), body (0x200), collision object
     (0x200), `f.col` (0x250) and collection (0x340);
  3. the wheels' `+0xd4`/`+0x1f0` cleared, then the original's bike_wheel_contacts, then each
     wheel's wheel_suspension, wheel_tire_forces and wheel_integrate (spin, contact velocity, hinge
     force).

  Then it checks:
  - not one word of the snapshot changed;
  - at least one wheel's `+0x1f0` is a prop's collision object;
  - the chassis' `+0x130`/`+0x140` moved on more than half the placings.
- **Result.** 289 placings, 304 wheels on a prop, the chassis forced on 282, no prop word changed.
- **Mutant.** With the snapshot moved before the search, the test goes red at once: collection 0's
  active flags `+0x20` and its lists show up as changed. So the comparison does see the wake's
  writes. The file was restored afterwards.
- **The port's side.** `prop_wheel::under` → `Collection::other` → `contact::other_at` writes only
  the collision object's `cofg_world`. The original's contact-velocity step writes the same word,
  with the same value, and the RAM comparison above saw no change there either. Nothing else in the
  wheel path takes the props mutably except `PropSet::struck`.
- **Runs.** `./kit check -- -p game-sim --test prop_knock` (all of it) and `--test prop_strike_paths`.

## Not verified / not 1:1

- **The force steps ran on placed, not ridden, wheels.** The node was moved, but the chassis body
  and the wheels' suspension state are the state's own. So the forces they compute are not ones a
  ride would make. The question was which memory the steps write, and the branches that touch the
  contact (touching set, owner set) were taken. A wheel_tire_forces branch that only a real
  compressed contact reaches was not shown to be taken on every placing. I read its decompilation
  instead: it writes only `+0x1e4`'s words.
- **The whole ground step (bike_ground_step) was not run.** Its stand-upright, balance and engine
  calls were read only as far as the `+0x1f0`/`+0x7c8` scan: none of them reads the owner.
- **The port's `chassis::Contact::listeners` stays 0 for a prop's contact** (ride.rs `contacts`).
  The original's count is 2. The port drops the distributor's `ContactCall` list, and the two
  listeners' effect is already the search's `PropSet::struck`, so nothing differs. The count itself
  is not carried.
- **Only free09's props in S195's states were read.** S208's woken truck's owner listeners
  (`owner+0x1f8` on a HasPhysics truck's collision object) were not read. If they do more than wake,
  a wheel on a woken truck could act on it.

## Next

None.
