# 1 — parked bike in the node world (FINAL)

Task C45. 2026-11-20.

## What I did

- Read the level node's dynamics world's lists out of R63's three RAM images (in, s34, pre) and R22's
  (`research/c45/lists.py`).
- Ran R63's slot-34 image (frame 9656, before the tape) whole in the interpreter on the tape's pads,
  73 ticks to frame 9729, logging what runs (R63's hooks). Found the frames the pads are first read
  held by matching the capture: player 1's Square from 9658, player 2's Cross from 9688.
- New test `crates/game-sim/tests/ai/parked_knock.rs`:
  `c45_the_parked_bike_is_in_the_node_world_before_the_knock`.
- Read the port's split-screen step (`crates/openmx/src/main.rs` World::simulate, `vehicle.rs`
  Garage::switch, `rider_contact.rs` contacts_met).
- Filed S495 (`later`): the parked bike as the original's, for a person to decide (it runs against P102).
- Did not look in ../opentennis: this is research on the original's world, with no port code changed.

## What the original does

- Leaving a bike for another machine leaves the bike as a physics object of the level node's
  dynamics world. It is the same body wrapper the props and the wreck bike use
  (`./kit fn 0x001590e0`). Its collision record is R63's unexplained 0x01362d30.
- While it is awake, its body is in the world's bodies, and each frame the world resolves its record
  after the car's in every one of the four passes (`./kit fn 0x001f29a0`).
- In R63 the bike was awake and among the world's bodies already in the frame-9596 image, and on every
  frame from 9657 it was still sliding (0.83 ft/s down to 0.71) and its record was resolved, sixty
  ticks before player 2's bike reached it. The knock (frame 9716) throws it to 12.2 ft/s and
  changes nothing about what runs. It did not join the world by being knocked: it was already in it.
- In R22's image, a bike parked long before is asleep: its body's enabled word is 0 and it is not in
  the world's bodies. Its record is still among the node's records, so it is still collided, but the
  world does not resolve it (R22 logged the car's record only).

## The port

- It does not do the same. The port's switch (P102, the user's design) takes the bike out of the world:
  not stepped, not drawn, not collided. In the split step a seat in a car is not "on", so its bike is
  not in the others' contacts. Another seat's bike rides through where it was left.

## How it was proved

- A bit-exact replay of the original in the interpreter, from `context/states/r63/s34.ram`, against
  R63's capture `context/capture/r63/strike.csv` on every frame 9657..9729: the ridden bike's chassis
  centre and velocity, its record's list (including the knock's entries naming the parked bike's
  record), the buggy's place and velocity. Each frame the test asserts the call order, that the
  parked body is enabled and in the world's bodies, and that the knock frames are 9716..9728.
- R22's image: checked statically (body asleep, not in the world's bodies, record still in the node).
- Run: `./kit check -- -p game-sim --test ai c45_` (1 passed). All three files it needs are on this
  disk, so it did not skip (the earlier logged runs printed every frame).

## Not verified / not 1:1

- Whether a knock wakes an asleep parked bike (R22's case). R63's parked bike was awake already, and
  making one sleep then riding into it needs a new, long capture. S200's hit listener suggests it
  wakes like a prop, but that is a reading, not a proof. Left to S495.
- When the parked bike falls asleep, and by what rule. Not captured.
- The parked bike's own position and velocity are not in R63's capture. They were checked only
  through the ridden bike's and the buggy's columns, which match bit for bit and depend on it.
- The pad read frames (9658, 9688) were found by matching, not from the tape's own timing.
- The port does not match (P102). Not changed: that is a user decision, filed as S495 (`later`).

## Next

Nothing for C45. S495 waits for a person.
