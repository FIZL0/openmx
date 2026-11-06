# 1 — rider touching flag

Task S278. 2026-11-05.

## What I did

- `crates/game-sim/src/handling/rider.rs` (new, mounted in `handling.rs`): `touching` (the wreck
  arm's walk), `clear` and `clear_all` (a part's four flags, and the mode machine's clear loop),
  `impact` (the step's impact sum) and `hand_over` (into the target record: the flag, the impact, the
  main body's position, the nine-part block). All of them work on RAM through `node::Ram`, like the
  rest of the SLUS-21104 bike ports.
- `crates/game-sim/tests/mxvatv_bike_ride.rs`: three tests (below), and the F41 test's comment
  pointing at them.
- research/oracle.txt `# S278`: the rider's words, the stretches, the touched words' writers, the
  rider's world census.
- The task was split. It asked to "port the rider's ragdoll world and that flag, or give the wreck
  clock another proved source". The flag is ported. Its inputs, the parts' touched words, come from a
  whole ragdoll world (18 bodies, 35 constraints, 3 iterations). That world is a port of the size of
  MX Unleashed's S52–S110 line, not one task. Filed as **S278a** (the world, the touched words' source)
  and **S278b** (the rest of the rider's mode machine).
- Nothing taken from opentennis: it has no ragdoll or anything like this rider.

## What the original does

The rider's frame (`./kit fn 0x004f1790 -p atv`) runs its mode machine (`./kit fn 0x004ef320 -p atv`)
and then its own physics world. In the mode machine's **wreck arm**, after the ground ask
(`0x001d6a60`), the machine walks the rider's part list (+0x5b4, count +0x5b0). It reads the count
once and stops at the first part whose +0x14 (touched) is nonzero. If the walk stopped short of the
count, the flag (+0x37c) is set and its clock (+0x380) gets `add.s dt`. A negative count also counts as
stopping short, because the walk never starts and 0 is not the count. Otherwise both are 0. Outside the
wreck arm the flag is left alone, so it stays 1 after the put-back. At its end, in every mode, the
mode machine clears every part's four flags (+0x14 touched, +0x18 ground, +0x1c scene, +0x20 swept).
It re-reads the count on each part.

The flag therefore says "a part was pushed out in the last world step". Then the rider's world steps.
Its type-8 constraints are the parts. A watch on all nine +0x14s over flip-1 shows that they are set
only at pc 0x0049b258, in `0x0049b220`, the push-out called from the bone collision's solve
`0x0049bb00`. They are cleared only by the clear loop, plus one write by the mode machine itself.
`0x0049bb00`'s fields (+0x78 terrain, +0x7c body, +0x84 pool, +0x88 sphere, +0x70/+0x20 cleared each
solve) and `0x004f1c30`'s named-part sum are MX Unleashed's bone collision and `ragdoll_body::impact`.
This rider is MXU's ragdoll actor in a later layout.

Inside the world step the rider's virtual calls the hand-over (`./kit fn 0x004f1210 -p atv`). It copies:

- the flag to the record's +0x1f0 (the bike's `SETTLES`);
- the impact (the rest quadword with the flag off, else `0x004f1c30`'s sum) to rider +0x670 and the
  record's +0x160;
- the main body node's +0x70 to the record's +0x200;
- the nine named part pointers into the record's part block, if it has one.

Its quadword moves are `lq`/`sq` and `lqc2`/`sqc2`, so the port drops the low four bits of each address.

## How it was proved

- **Oracle, `the_riders_hand_over_and_part_clear_are_the_originals`**: 20 000 calls on state 4's rider,
  about half of them the hand-over and half `0x0049ae10`. Every word they read is drawn at random: the
  flag (0, 1, any bits), the nine named parts (any of the rider's, repeats allowed), each part's touched
  word and velocity change (signed zeros), the old impact, the main node position, its holder's
  pointer (off by 4 one time in four), and the part block (none or a fake). The rider, record, parts,
  node and block all come out word for word the same. Neither side writes outside them. The paths
  counted: flag off 3 455, on 6 535; no named part touched 139, one 343, all nine 137; block 6 687,
  none 3 303; unaligned node 2 535; clears 10 010.
- **Oracle, `the_riders_walk_and_clear_loop_are_the_originals`**: 20 000 runs of the mode machine's
  own two stretches (0x004effb4..0x004f0020 and 0x004f0274..0x004f02b0, the rider in s2, dt in f20, a
  hook at the stretch's end returning to the call). The list is a fake one with a count from −2 to 12,
  the first touched part anywhere or nowhere, any flag, a clock with signed zeros, and the game's dt or
  any other. The paths: negative count 1 350, no parts 694, touched 2 826, none touched 5 130, clear
  loop with parts 7 972, without 2 028. A mutant that changes the walk's `i == n` to `i >= n` fails at
  case 12 (count −2).
- **Replay, `the_riders_flag_reaches_the_wreck_clock_through_the_port`**: both C15 replays run whole in
  the interpreter. The port serves the part clear (every call) and the hand-over (every frame), and at
  the ground ask the port's walk is worked out from the parts' words there and checked against the
  original's at the next clear. Every row is still bit-exact (the physics blocks and the wreck state),
  and the bike's `SETTLES` matches the captured bike object on every row. ride-2: no walk, 459
  hand-overs, `SETTLES` 0. flip-1: 68 walks (65 set; rows 125–127 not yet), 465 hand-overs, `SETTLES`
  1 from row 128 on, the put-back on row 193 as before.
- `./kit check -- -p game-sim --test mxvatv_bike_ride -- the_riders_`: 5 tests passed: the three
  new ones, F41's and one more of the file's `the_riders_*` tests.

## Not verified / not 1:1

- **The parts' touched words are still the original's.** They come from the rider's ragdoll world's
  bone collision push-outs, which are not ported (S278a). So the port's wreck clock gets its flag
  through ported code, but from inputs the original's world makes. The task's aim, a wreck clock that
  runs without the original rider, is not met yet.
- The walk is proved only as a stretch of the mode machine (oracle) and on the replays' wreck arm. The
  choice of *when* the wreck arm runs (the rest of `0x004ef320`: modes +0x368/+0x36c, the wreck entry
  `0x004ed700` and the rest) is not ported (S278b). The replay check only covers frames where the
  original took the arm. Its trigger, the ground ask with the rider's +0x5b8, was not shown to be
  unique to the wreck arm beyond reading the decompilation and the replay's 68 asks.
- The hand-over's read order differs from the original's where words could alias. The first block
  write reads the holder without re-reading the record, and the original reads each part pointer after
  the block pointer for the first word only. The oracle keeps the rider, record, block and parts
  apart, so an aliased layout is not proved. No real object aliases them.
- The impact (`impact`) is proved only inside the hand-over with the flag set, not called alone. The
  hand-over is its only caller seen.
- Every replay result covers only C15's two replays: one wreck (the flip, thrown front, all nine parts
  being the list's).
- The rider world's census (iterations, rate word 27.0, the lists, the constraint vtables) was read off
  state 4's image once. It is not proved to be the same in other states.

## Next

S278a: the rider's ragdoll world on MX Unleashed's ragdoll modules, proving the nine touched words per
frame on flip-1. Then S278b.
