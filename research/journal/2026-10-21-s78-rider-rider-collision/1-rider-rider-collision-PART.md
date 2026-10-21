# 1 — rider rider collision (PART)

Task S78. 2026-10-21.

## What I did

- Split the task, because it was too big for one session:
  - S86: the bike's `m.col` and `f.col` collidables off the disc, with the ray-vs-kind-5 narrow phase and the run-time mask words.
  - S87: the riding bike's record wired into the ride, including the substep order, the ghost masks and rollback order.
  - S88: a two-player capture of the original and its bit-exact replay, after S64.
- Did the first provable piece, in `crates/game-sim/src/contact.rs`: the contact resolution with the own side on a vehicle chassis.
  - New types `Ridden` (the chassis, plus its `+0x60` centre and the `+0x130`/`+0x140` force and moment) and `Own` (a body or a ridden chassis).
  - New entry point `resolve_ridden`.
  - The old `resolve` and the pub `relative_velocity` / `spring` / `impulse` / `friction` keep their signatures and now go through the shared `*_on` internals.
- Added the oracle test `the_contact_resolution_on_a_chassis_matches_the_original` to `crates/game-sim/tests/contact.rs`.
- Added the `# S78` block to research/oracle.txt (comments only).

## What the original does

- **The record.** A riding bike's collision record is `bike+0x7d0`. `./kit fn 0x0026fbd0` wires it:
  - Its own side is the chassis through the chassis' transform (`+0xf0` chassis, `+0xf4` = 0, `+0xf8` = chassis `+0x254`).
  - `+0x1f0` is the 11 body rays (`%sm.col`).
  - `+0x1f8` is a kind-5 collidable (`%sf.col`, loaded via `./kit fn 0x001ed1e0`). Its `+0x80` points back at the record, so another rider's rays striking it reach this record.
  - `+0x1f4` is the sweep probe.
  - The actors mask and the collidable's flags OR in the bike's `+0x390`/`+0x394` and two run-time mask words. Collides-with stays all-ones, so a pair of bikes is mutual.
- **Collide.** `collision_record_collide` (`./kit fn 0x002708a0`) runs the terrain rays, then the actor query. Each actor hit leaves an own entry (kind 0) and a mirrored entry (kind 1) in the struck record.
- **Resolve.** `collision_resolve` (`./kit fn 0x00270dc0`) runs every substep. Its flag means "first substep", so impulses only happen there. Damping needs a dynamics body (`+0xf4`), so it never runs on a chassis.
- **The chassis branches**, in each of relative velocity, spring, impulse, friction and apply (`./kit fn 0x00270f60`, `0x00271590`, `0x002719b0`, `0x002722d0`, `0x00272880`):
  - Arms are measured in the chassis' frame from `+0x190`. The point comes in through `node.unplace` and the result goes back out through `node.rotate`.
  - The impulse goes to shock `+0x170`, vel `+0x260`, twist `+0x180` and rates `+0x290`, with the moment taken through `node.unrotate`. The inertia is the three diagonal floats `+0xf0/+0x104/+0x118`.
  - Spring and friction forces go to `+0x130`, and their moments, taken from `+0x60` + cofg_world and unrotated, go to `+0x140`. Only `xyz` is written (checked in the disassembly).
  - The exact expressions are in oracle.txt's S78 block.
- **The wreck.** A rider on rider hit causes reason 7 (joust), when the head ray hits a rider whose chassis is more than JoustWreckHeight higher. S54 already ported it (`air::hit_wreck`).

## How it was proved

- Function oracle: `crates/game-sim/tests/contact.rs::the_contact_resolution_on_a_chassis_matches_the_original`, 10,000 generated cases.
- The plane's record in the biplane save state is pointed at a generated chassis and transform in scratch memory (`+0xf4` = 0). The others are a chassis two times in three, otherwise a dynamics body.
- The original's `collision_resolve` runs through the record's own vtable, with impulses on and off.
- Bit for bit, the test compares:
  - the record's written fields and both arms of every contact;
  - the own chassis' velocity, rates, shock, twist, force and moment;
  - the others' chassis or bodies.
- Coverage of the single-contact cases (each count must be ≥ 100, and the run was confirmed by forcing the assert):
  - 16878 contacts;
  - 2293 hard contacts, 1425 impulses and 616 impulses chassis on chassis;
  - 1543 mutual and 1724 actor contacts;
  - 3778 contacts with a mover.
- The S42 body-path test still passes.

## Not verified / not 1:1

- **Rider-rider collision in the ride.** The port's riders still pass through each other. Nothing calls `resolve_ridden` from `ride.rs` yet, because the ride has no 11-ray record and no other bike's kind-5 collidable to strike. That is S86 and S87.
- **The `f.col` kind-5 shape and its ray query.** Not read or ported (S86).
- **The two run-time mask words** (`0x00468f50`, `0x00468f68`). They are 0 in the program image and weren't read from a riding state, so which classes a bike's rays hit isn't confirmed at run time (S86).
- **The ghost exclusion for bike-on-bike.** It goes through the ignore masks (S63) and isn't exercised, because there's nothing to exclude yet. ghost.rs still keeps ghosts out of rival lists.
- **A two-player capture of the original** (head-on, side swipe, landing on another). Not recorded:
  - No two-player save state exists (plan/HUMAN.md slots 6/7).
  - Single-rider replays already part at tick 1 until S64.
  - This is S88.
- **The mover branch with a chassis own side.** Generated, but movers were only tested with hooked virtuals, the same as in S42.
- **Determinism for rollback (N1).** `resolve_ridden` is pure `F`/`Vf` with no host floats. The order two records resolve in is S87's to fix.

## Next

S78 is blocked after S86, S87, S88 and S64. Do S86 first: load `bike\col\*m.col` / `*f.col` the way S69 loaded wreckbikem.col, and read the two mask words from a riding `./kit state ram`.

opentennis: nothing to take. Its nearest thing is hst-play's `bodyhit.rs`, a ball against a player's posed bones that ends the point. It has no rigid-on-rigid impulse and no player-on-player contact.
