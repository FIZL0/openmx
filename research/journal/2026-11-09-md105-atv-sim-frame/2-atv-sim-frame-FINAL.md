# 2 — atv sim frame: the all-Rust chain (FINAL)

Task MD105. 2026-11-10. Slot s2.

## What I did

- MD105a (the tail in Rust) and MD105b (the record's outside and the world step in Rust) have both
  landed. Part 1's split asked for one more step: run all the port's pieces in a single chain.
- New test `the_quads_frame_is_the_ports_through_the_palm_strike_wreck_and_put_back` in
  `crates/game-sim/tests/atv_quad/sim_frame.rs`: `palm(PALM_RIDE, Some(Pieces::Port), None, true)`.
  - The quad's record's clear and pass are the port's (`port_record` → `handling::record`).
  - The record's part search, sweep and node points are answered by Desert Oasis (`WorldStep`).
  - The world step is `quad::frame`, holding the pass's contacts (`atv_contact::Held::refill`).
  - The tail is `sim_tail::port_tail`: the walk, `quad::update` (`tail::inner`, `tail::stuck`), the
    wreck test, entry and unwreck, `quad::swap_rays`, `quad::put_back` and `quad::animate`.
  - These run against the original's whole sim frame (vt `+0x40`). The test checks `check_palm`'s
    events, that the port's step ran every frame and resolved contacts, and that each frame had one
    part search and one sweep answered by the port.
- The module doc now names the combined test. No product code changed: the two sub-tasks had already
  put all of it in `quad.rs` / `atv_contact.rs`.

Nothing was taken from opentennis: it has no vehicle physics.

## How it was proved

`./kit check -- -p game-sim --test atv_quad -- the_quads_frame_is_the_ports`: it passes. I confirmed it
really ran (the image and terrain are present) with a temporary failing assert, since removed:
- 300 frames, with the port's step resolving held contacts on 73 of them;
- the port's outside answered 300 part searches (18 hits), 300 sweeps (239 pairs) and 1,126 node points;
- 8,944 ground queries, 1,126 of them by the port's record;
- first contact on frame 89, part swaps on [96, 164, 256], 2 wrecks.

After every frame:
- every RAM word outside the stack, the ground ring and the two stack-pointer globals equals the
  original's whole frame;
- the port step's ride (`image::compare`), its body and its resolution words equal the original's step.

The mutation checks from the parts still stand. `a_spoiled_contact_on_the_strike_goes_red` covers the
record, `a_spoiled_held_contact_turns_the_ports_step_red` covers the step, and MD105a's temporary
mutations on frame 164 covered the tail.

## Not verified / not 1:1

- **The original's world step still runs on the machine after the port's.** `WorldStep::step` checks
  the port's `quad::frame` against it word for word, then the machine carries the interpreter's RAM
  forward, not the port's. The two are equal on every word the port keeps, so the chain is the
  same. But words the port's ride does not model (anything `image::compare` does not read) are the
  original's step's, not the port's.
- **The record's outside calls still run for their RAM side effects** (part place, searched/hit flags,
  plant parts stood, sweep set/place, scene node update). MD105b checked the port's answers against
  them, but the RAM writes are the original's. Follow-up MD118.
- **The sweep pair is MX Unleashed's** (`sweep_probe::pair`). It misses all 239 times, as the
  original's does. No hit has been compared. Follow-up MD119.
- **The bike vehicle's own slots in the tail** (lean, ride, sync, snapshot, fade, network, parts), the
  ground reset, the clips, the node pose and every other listener's step are still the interpreter's
  (MD105a's list).
- **Some port writes land at the port's return**, not where the original stores them (put-back words,
  ray swaps, the animation driver's words: MD105a). The order inside a frame is not checked.
- **The port's state is re-read from RAM every frame** (the ride, the body, the record, the scene).
  Only `atv_search::Oasis` persists. This is a frame-by-frame chain over the interpreter's RAM, not
  a free-running port sim.
- **Not compared**: the stack, the ground ring and the two stack-pointer globals (part 1).
- **The interpreter is the oracle, not PCSX2.** MD37a's captures are gone (C28).
- **The pad is laid, not read** (raw levers and stick only), as in part 1.
- **Only the atv_ quad on Desert Oasis's palm ride.** No tricks, countdown, net interpolation or
  player respawn point on this path (MD105a's list).
