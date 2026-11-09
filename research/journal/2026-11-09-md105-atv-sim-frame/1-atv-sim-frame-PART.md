# 1 — atv sim frame (PART)

Task MD105. 2026-11-09. Slot s2.

## What I did

- Ran SLUS-21104's whole sim frame (the sim object's vt `+0x40`) on `atv-r-spawn.ee.bin` in the
  interpreter, frame after frame over `frame.rs`'s palm ride (`at_the_palm`), with F31's hooks (DIntr/EIntr,
  the syscall stubs, the stunt packet's counter read) and the ground query answered by the port's terrain.
  **The whole frame strikes the palm. The world step alone does not.**
- New `crates/game-sim/tests/atv_quad/sim_frame.rs`, mounted from `frame.rs`. Its `palm()` runs two machines
  side by side: one runs the original's whole frame, the other runs the frame in F31a's pieces. After every
  frame it compares all of RAM. Three tests:
  - `the_quads_whole_sim_frame_strikes_the_palm_wrecks_and_is_put_back`: the pieces are all the
    original's, which shows that driving the frame by hand is the same frame. It also asserts the ride's
    events.
  - `the_quads_record_is_the_ports_through_the_palm_strike_wreck_and_put_back`: the quad's collision
    record's two virtuals are the port's (`handling::record::clear` / `pass`, F31b4's code; the record
    is the same class as the bike's). Everything else stays the original's.
  - `a_spoiled_contact_on_the_strike_goes_red`: the mutation check. It flips one bit in each lane of the
    port's first contact normal on frame 89, and the compare goes red there (and in the quad, once the
    same frame's world step pushes with it).
- The record glue (`RecordInRam`) is copied from `tests/mxvatv_bike_ride.rs`'s F31b4 glue, with one
  addition: after the port's terrain query, it writes the terrain service's copy of the answer, as
  the service's vt `+8` does.
- Split the rest of the task: **MD105a** (the tail in Rust: update, animate, wreck, put-back) and
  **MD105b** (the record's world search and node calls in Rust, and its contacts fed to `quad::frame`,
  which is MD98's refill). MD105 is blocked until both land. Its own proof, the all-Rust chain, needs both.

Nothing was taken from opentennis: it has no vehicle physics. The glue is F31's, from this repo.

## What the original does

- The sim object `0x0065bba0` (vtable `0x003b9320`) holds its physics world inline at `+0x6c`. The
  world is not a pointer: `0x0065bc0c` is the object's own `+0x6c`. Its `+0x24` list holds the quad's
  collision record `0x016286d0`, vtable `0x003b9290`, the same as state 4's bike record. So its clear
  (`+0x7c`) and pass (`+0x84`) are F31b4's functions.
- On the palm ride:
  - The record's world rays file the palm's contacts from **frame 89**, 1–4 per frame.
  - On **frame 96** the record's active part (`+0x200`) swaps from the riding part to the wreck part,
    and the quad's wreck count `+0x6e4` goes to 1.
  - On **frame 164** the quad is put back at its spawn spot on the riding part.
  - On **frame 256** it strikes the palm and wrecks again (`+0x6e4` = 2).
  - A wrecked frame runs up to about 550,000 instructions; a riding frame about 200,000.
- The record pass asks the terrain once per heightfield ray, and it only does that when the record's
  `+0x24` is set (after the wreck). Each query runs the query function, then the terrain service's
  vt `+8` (service `*(0x003eab60)+4` = `0x0082cdd0`). That call copies the answer into the service
  (point `+0`, normal `+0x10`, surface word `+0x20`, weights `+0x30`), and the record reads it back
  through the service's other slots. The port's record made 1,126 such queries over the ride.
- Two globals, `0x003dce40` and `0x003dce48`, keep a pointer into a caller's stack frame. They first
  differ on frame 81, because the hand-driven pieces nest the calls at another depth.

## How it was proved

`./kit check -- -p game-sim --test atv_quad -- sim_frame`: 3 tests pass. I confirmed they really ran
with a temporary failing assert: 300 frames, 8,944 ground queries, and 1,126 terrain queries by the
port's record. First contact on frame 89, part swaps at [96, 164, 256], 2 wrecks.

- In both chain tests, every word of the 32 MB RAM is compared after each of the 300 frames. Three
  things are left out:
  - the interpreter's stack: the top 1 MB plus 16 KB, which holds the frames and scratch slots of
    separate calls;
  - a word whose values on both machines are stack addresses: the two globals above;
  - the ground query's ring of read cells (`0x00541550`, 10 × 0x70) and the terrain's ring words
    `+0x9e0`/`+0x9e4`. The port reads the map straight and keeps no ring (`atv_ground`'s doc; S278b
    also leaves these out).
- The mutation test shows that a one-bit change in the port's record result is caught.

## Not verified / not 1:1

- **The task's own proof is not met.** The brief asks for the port's update + animate + `quad::frame`
  + the record's collide in Rust, every word equal on the palm ride. This session proves one piece,
  the record's two virtuals in Rust. The tail (the quad's update, animate, wreck, put-back) and the
  world step still run as the original's code in the interpreter on the port side. Those are MD105a
  and MD105b, and MD105 is blocked after them.
- **The record's outside is still the interpreter's**: the probe sets, the world searches that find
  the palm, the scene node transforms and the body's damping setters. Only the record's own logic and
  the terrain query are Rust (MD105b).
- **The record's contacts reach the world step through RAM.** The original's world step resolves
  them. The port's `atv_contact::Held` is not filled from `record::pass` yet (MD98 / MD105b).
- **The ground ring and the two stack-pointer globals are not compared**, for the reasons above. The
  ring is a cache, and `atv_ground` already shows that it changes no answer. The globals are left out
  because their values are stack addresses. I did not trace what reads them.
- **The interpreter is the oracle, not PCSX2.** MD37a's captures are gone (C28), so no PCSX2 row
  checks this ride. That the interpreter matches PCSX2 is F31's result for the bike's frame. Nobody
  has shown it for the quad's.
- **The pad is laid, not read.** As in MD37b: only the raw levers and the stick are written each
  frame. The pad reader does not run, and the d-pad, trick byte, clutch and put-back request are never
  laid. The put-back on frame 164 is the original's own timer, not a requested one.
- **Only the atv_ (Race) quad, and only this one ride.**

## Next

MD105a or MD105b, either one first. For MD105a: in `sim_frame.rs`'s `frame_in_pieces`, replace the
`TAIL` call with the port's tail walk as `tests/mxvatv_bike_ride.rs`'s `port_tail` does for the bike.
First list what the tail calls for the quad: the quad's vtable `0x003ce250` update, animate, the wreck
test/entry onto the wreck part (`0x0162cef0`) and the put-back. Then reach the same RAM every frame. When
both have landed, MD105 runs the all-Rust chain: `quad::frame` with `quad::Steps` holding the record, plus
the port's tail, compared word for word with the original's whole frame as `frame.rs`'s `chain` does.
