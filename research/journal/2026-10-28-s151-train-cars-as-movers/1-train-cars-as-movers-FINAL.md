# 1 — train cars as movers (FINAL)

Task S151. 2026-10-28.

## What I did

- **Worked out how the original files a path car on the bike's collision record:** as a **mover**, not
  as plain scenery. The facts are under S151 in `research/oracle.txt`.
- **Made the port's car search (S137's `mover_contact`) name the car as the entry's mover again.** S145
  wrote this and then backed it out in its wip commit bf4f9aa4, because it turned train.rs's nose test
  from reason 6 into reason 1. I re-applied that commit's mover_contact.rs and rider.rs hunks unchanged:
  - `Probe::mover` / `Search::velocity`;
  - `inside`'s mover;
  - the `first` argument;
  - `Bump::movers` falling back to the world's props when the scenery search named none.
- **New test, `tests/traffic.rs` `the_car_search_names_the_car_as_its_mover`.** It puts the disc's bike
  collision files on S89's s89-along cars, beside the moving sixth car's side and facing it. Every entry
  of `mover_contact::struck` names a car's prop as its mover and none names a collision object. One entry
  names the wagon, and the wagon's prop mover is its own `Moving::mover()`. The wreck test still reads
  every entry as scenery. Control: with the cars' motion taken away, the same rays still strike, and no
  entry names a mover.
- **`crates/openmx/tests/train.rs`**: the nose test now accepts wreck 1 or 6. It used to pin 6. The
  comment says why, and that S170's capture decides which one the original gives.
- **Whether a bike can stand on a moving roof** was already answered, and nothing here changes it:
  - S146: the original's 250, braked on a moving boxcar's roof, rides it for 9 s with no wreck. The
    port's replay agrees bit for bit.
  - S145: the wreck bike rides the roof through the body contacts' movers.

  That S146 replay and S129's two train replays still pass with this change.
- From opentennis I took nothing: it has no moving platforms.

## What the original does

- In `collision_record_collide`'s actor loop (`./kit dis 0x002708a0`, the part after the search
  `0x0011a150`), `collision_record_add` gets a ninth argument on the stack. The adder stores it at the
  entry's `+0x78`, the mover. Its value is the struck collidable's `+0x7c`, but only when that object's
  `+0x1b0` is 2 (a vehicle of kind 2); otherwise it is 0. Ghidra's decompile drops this argument, so
  `./kit fn` shows an 8-argument call. Read the disassembly instead.
- Two other entries get mover 0:
  - the mirrored entry that goes into the other actor's record (kind 1);
  - the terrain rays' entries (the delay-slot store of zero).
- Each path car's mesh collidable (`car+0x428`) has `+0x7c` = the car and `+0x80` = 0, and the car's
  `+0x1b0` is 2. This holds for all 21 cars in s129-across, s89-along, s146-roof and s129-braked; I
  scanned for the path-car vtable in each image. S141's test checks the same thing on the biplane state.
- So when a riding bike's body ray strikes a car, the result is an own entry with:
  - other (`+0x74`) = 0, so the wreck test reads it as scenery (reason 6 on the head ray's reach);
  - mover (`+0x78`) = the car, so the resolve's relative velocity meets the car's velocity and spin
    (`collision_relative_velocity`, vt+0xb8/+0xbc).

  This is not a still wall, which is what S137's response was.

## How it was proved

- **Disassembly** of the stack argument at both of the actor loop's calls and at the terrain call
  (oracle.txt S151).
- **RAM images**: a scan of 4 images × 21 cars (owner == car, no object, class 2).
- **Port test**: `tests/traffic.rs` `the_car_search_names_the_car_as_its_mover`, with its control. It
  checks the port's naming, not the original's arithmetic. The resolve's mover arithmetic was already
  proved bit for bit against the original in S142 (tests/contact.rs, ragdoll_cars.rs).
- `./kit check -- -p game-sim --test traffic`: 12 passed. `-p openmx --test train`: 3 passed. replay.rs
  `train` filter (S129 braked/across, S129 gather, S146 roof): 3 passed with 0 skips in s2.
- Effect in the shell: the nose test's first wreck is reason 1 with the mover and 6 without. S145
  measured the same.

## Not verified / not 1:1

- **Which wreck the original enters when the train's nose hits a standing bike (1 or 6) is not checked.**
  The port now gives 1. There is no capture of the original's bike meeting the nose, and that capture is
  S170. train.rs accepts both until then. S170's line ("if 1, name the car as the mover") is out of date:
  the car is named now, so S170 only has to confirm the reason, and if the original gives 6, find what in
  the port's g-test or resolve makes the harder hit.
- **The original's whole actor loop was not run in the interpreter.** That means the search
  (`0x0011a150`) gathering a car and the entry write together. The mover write is read off the
  disassembly, and the port's search was not compared hit for hit with the original's.
- **Whether a non-ghost riding bike's body-ray search admits the cars' category 0x48 was not checked
  separately.** S137 assumed it does. The ghost's mask clause (oracle.txt P57) only says the ghost leaves
  vehicles out.
- **S129's across ride still parts at the wagon (S190).** The original's bike is pushed back out to −z
  (vel.z +44 → −20 over ticks 146–165) and rides off without a wreck, mostly through the wheels' swept
  contacts. Naming the mover does not fix that, and S190 is unchanged.
- **The head-ray `inside` contact is still the port's own (S137).** It now names its car as mover too.
  There is nothing in the original to compare it with.

## Next

S170: capture the nose's wreck reason. S190: the wheels' swept contacts on the wagon.
