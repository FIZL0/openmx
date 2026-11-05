# 1 — still waiting (PART)

Task F31b5. 2026-11-05.

## What I did

- Read the five parts' journals (F31b5b..f) and `posts_frame` in `crates/game-sim/tests/mxvatv_bike_ride.rs`.
  `posts_frame` is the frame with every port so far. It still hands these to the interpreter on
  C15 ride-2 and flip-1:
  - **the record's probe searches** (`> record sweep`, `> record search`, 459 each on ride-2) and **the
    collision sync's database half** (`> PartMove` 918, `> Gather` 459): **F46**;
  - **the steps' handed-on calls**: the gravity's record call (the record's vt `+0x88`, 1836 / 1860),
    the preload cue and the stunt start (once each on flip-1), the stunt end and the wreck (no replay
    reaches them). No open task covered these, so I filed **F31b5g**;
  - **the tail** (`port_tail`): its scene and animation callees (**F43**), the trail, sender and parts
    virtuals (**F44**), and update::trick's and the net calls' remaining originals (**F45**). F31b5c and
    F31b5f showed many of these leave the physics blocks alone. The node outputs (F43) and the sender's
    `+0xcfc` (F44) were left to those tasks;
  - **the `+0x3c` list's walk**: empty on state 4 (F31b5d), so the pure frame can drop it.
- The paths C15 never takes (body reset and respawn, out-of-world message, put-back slot, node bounds in
  the database, parented node move) are **F47**. The pure frame would panic or diverge on a ride that
  reaches them.
- **The rider's `+0x920` flag** (the wreck clock's settle) still comes from the capture in F31b5d's
  dropped frame. Porting it needs the rider's ragdoll world: **S278**.
- The pure frame also has to run without an `Ee`. Its state has to live in `steps::read`'s `quad::Bike`,
  `bike::Post`, `record::Record` and the seed's body, not in RAM. That move can only be checked once
  nothing is handed on, so it stays F31b5's own last step.

## What the original does

Nothing new. The calls' addresses and arguments are in the first entry and in research/oracle.txt
(# F31b5c, # F31b5f, # F39).

## How it was proved

Nothing new proved. The count of what is still handed on comes from `posts_frame`'s own asserts
(`handed` at `crates/game-sim/tests/mxvatv_bike_ride.rs` around lines 7354 and 7375). Those tests are on
main and pass in the gate.

## Not verified / not 1:1

- **The whole acceptance is still open**: neither replay runs in pure Rust. It waits on F31b5g, F43,
  F44, F45, F46, F47 and S278.
- **F45 being needed is my judgement**, not a measurement: update::trick's calls to the originals are
  tail callees. F31b5c's census showed them inert on the physics blocks only, not on every word the
  frame reads.
- **The preload cue may turn out to be presentation** (a sound queued). If F31b5g shows that, it can
  be dropped instead of ported. I have not checked.

## Next

When F31b5g, F43, F44, F45, F46, F47 and S278 have landed: build the frame on `handling::seed`'s
state and `handling::ground`'s terrain with no `Ee`. Start from `posts_frame`, swap each remaining
interpreter call for its port, and assert that nothing is handed on. Then hold ride-2 and flip-1
row by row (wreck on row 125, put-back on row 193).
