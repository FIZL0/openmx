# 1 — the bike's PreProcess (FINAL)

Task F31b (sub-task F31b1). 2026-11-03. Slot s3.

## The split

F31b asks for the whole of SLUS-21104's per-frame bike sim. I listed what state 4's frame runs (a
scratch enumeration test; it is gone). Addresses are in research/oracle.txt `# F31b1`.

- The sim's `+0x24` list holds one object, the collision record.
- The world holds one body (the bike's) and one force listener: the bike's Force body, called
  through a forwarding element.
- Its six preforce listeners are all the same thunk to the bike's PreProcess, one per rider; the
  bike's own is its `+0x30`.
- Its one post listener is a thunk to the bike's PostProcess.

The quad's (MD-series `quad.rs`) are these functions' four-wheeled twins, at the same offsets, so the
bike can run on the quad's types. I filed F31b1..F31b5 with `plan add F31b`: PreProcess; Force body;
PostProcess; the record pass, the tail, the wreck and put-back with MD63f's gravity and MD63e's masses;
then the whole frame in pure Rust.

## The PreProcess

It differs from the quad's head as follows:

- it writes one front and one rear wheel, not pairs;
- it sets the body's turn locks through the body's virtual `+0x64` (the angular axes `+0x120..`).
  Balancing, or wrecked (`+0xc80` set): all free unless the hold mode `+0xddc` is set. Steering: free
  in the air; on the ground the yaw locked, or all three locked when held;
- the front's balancing flag (`+0x30`) is 1 while balancing and 0 otherwise;
- wrecked, it zeroes the front's steering command and does nothing more to the steering.

The rest (input limiter, front brake let go in the air, the fade and full-lock steering, the donut on
the rear, the spring map 14.6666..29.3332, the two quadword resets, then the gearbox handed the rear's
`+0x838`, the clutch and the head-and-tail) reads as the quad's does. Its input limiter and
head-and-tail through the vtable are the quad's functions (vt `+0x34c`, `+0x4d0`).

## What I did

- `crates/game-sim/src/handling/bike.rs` (new, `pub mod bike` in handling.rs):
  - `ground_head(p, q, body, held, axes, dt, m)`: on `quad::{Params, Quad, Body}`, with the steps through
    `quad::Moto` (`quad::Steps` has them ported, the quad's);
  - `FRONT`/`REAR`: the quad's left front and left rear;
  - `read_params`, `read_held`, `read_quad` (the guards worked out: vehicle state, the balance words,
    the wreck reason), and `quad_words`, `read_quad`'s inverse, all at the quad's offsets.
- `quad::donut` made `pub(crate)` so the bike's head uses the quad's ramp. No other change to quad.rs.

## How it was proved

`tests/mxvatv_bike_ride.rs`, run as `cargo test --release -p game-sim --test mxvatv_bike_ride`: 8 passed
(the 5 F31/F31a tests and the 3 new ones).

- `the_bikes_pre_process_is_the_originals`: 12 000 bikes off state 4's image, with the head's
  inputs random:
  - the guard (no state; kinds 3, 4 and others), the balance words (tied a quarter of the time), the
    wreck reason, the hold, the turn locks as they were;
  - on the ground or not, the fade's speeds (tied a sixth), full lock on or off, the lean at ±0.987
    and around it, signed zeros;
  - the donut gate open half the time, with its ramp ends at 0, −π/2, −π, and the springs' speed at
    14.6666.

  Each bike runs through the original's head (its steps hooked to log and do nothing; the body's
  virtual its own) and through the port from the same words. Every word of the bike (0x1800), both
  wheels (0x920) and the body (0x230) comes out equal, and the steps are called in the same order with
  the same arguments. Every path counted at least 30 times. A one-word mutation (yaw lock to pitch
  lock) fails it on bike 16.
- `ride_replays_bit_exact_on_the_ports_head`, `flip_wrecks_and_is_put_back_bit_exact_on_the_ports_head`:
  F31a's hybrid frame with the bike's preforce element run by `ground_head`, which writes back only
  the words it changed. The steps it calls stay the original's in the interpreter, as do the other five
  riders' PreProcesses, the Force body, the PostProcess, the record and the tail. All 459 and 465 rows
  match their capture word for word, and the flip wrecks on row 125 and is put back on row 193.
  Moving the spring map's top by one ulp breaks both on row 0.
- `./kit audit`: its 4 leaks are already on main (atv_wheel.rs, plan/DONE.md); none are in these files.

## Not verified / not 1:1

- The steps the head calls (input limiter, gearbox, clutch, head-and-tail) are still the
  original's in the replays. `quad::Steps` ports them for the quad, and they are not yet run on the
  bike's two wheels: that is F31b2's (with the Force body that shares them).
- The quad's right-hand wheel slots are unused here, and `read_quad` leaves them default. A step
  ported for four wheels that walks all four would see two empty wheels: check this when F31b2 runs
  `quad::Steps` on the bike.
- `read_held` and the guards read a flag as non-zero. The oracle only gave the hold 0 or 1, and the
  balance words random or tied.
- The body's `+0xc0` reset goes through `quad::Body::reset_each_frame`. As on the quad, what reads it
  is not traced.
- `Handling::MxVsAtv` does not ride on this yet. Original rides do not touch any of this code, so their
  replay hashes are unchanged, but the full replay suites were not run this session (only the targeted
  test file above).
