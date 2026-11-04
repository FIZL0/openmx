# F31b2: the bike's Force body

2026-11-03. Task: `./kit plan F31b2` (F31b split, 2 of 5).

The bike's Force body (its ground substep, run four times a frame) is now ported as
`game_sim::handling::bike::ground_step`, on the quad's types (`quad::{Quad, Body, Params}`, the bike's
wheels in the quad's left front and left rear). Addresses and the call map are in research/oracle.txt
under `# F31b2`.

## What it does

- It is guarded the same way as the PreProcess (`Quad::active`). Then the vehicle's gravity.
- With control on (`Quad::control_off` unset):
  - the rider's controllers, MD63b's `handling::step::{stand_upright, balance, roll_pitch}`, with the
    balance only when the balance words agree;
  - the preload, backflip, air spin, engine, air drag and airborne control;
  - the ground push. It is gated on the rear wheel's own contact flag (the quad gates on its rear-down
    flag) and on the push's gate and flag, and adds `gain·(target − speed)` along the bike's `z`.
- Then, with control on or not: both wheels cleared, the contacts, both suspensions, both tire forces,
  the rear's pair flag, both wheels integrated, the rev limiter, the peak rates.
- The words the controllers need that `Quad` does not hold (the balance keys and state, the stick, the
  body's centre of gravity) are a new `bike::Rider`, read and written by `read_rider` and
  `rider_words`. The `bike::Rides` trait (a `Moto` that owns the `Rider` and answers the smooth noise)
  hands it over, so a caller can put those words in RAM before a step that reads them. The tire forces
  read the centre of gravity, so this matters.

## A fix in MD63b's balance

The oracle found that the original integrates the balance inertia, command and point as
`adda.s +0, x; madd.s`. Because `+0 + −0 = +0`, a `−0` accumulator comes back `+0`. `move::balance`
(MX Unleashed's) adds straight to `x` and keeps the `−0`. `handling::step::balance` (MX vs. ATV's
wrapper) now folds a `−0` to `+0` first, when the front wheel is down, which is where the original
reaches those instructions. Its doc had said the `0 + x` form was "the same one add", and it now says
otherwise. `move.rs` itself is unchanged. mxvatv_step's oracle never drew a signed zero, which is why it
did not catch this. It still passes.

## Checked

- `the_bikes_force_body_is_the_originals`: 12 000 bikes, each one state 4's with the body's inputs
  drawn at random (guards, wreck reason, balance words, balance keys and state, pitch keys and state,
  levers, signed speed, leans, wheel contacts/normals/steering, the rear pair flag, the body's force,
  moment, spin, quaternion and centre of gravity, the bike's euler and world spin, the push; edges and
  signed zeros among them). Each one runs through SLUS-21104's Force body (callees hooked to log; noise
  hooked to a stand-in table on both sides) and through `ground_step`. Every word of the bike
  (0x1800), both wheels (0x920 each) and the body (0x230) matches. The calls match in order, target
  (bike, wheel or engine) and `dt`. Path counts: off 2998, wrecked 1543, riding 5028, balancing 2431,
  balancing with the rear up 593, in the air 948, wheelie moment 6511, stoppie on 45, held 130, ended
  3588, pushed 1862, push with the rear up 633, rear pair set 1648, cleared 7354.
  Mutations: flipping the push's sign fails on bike 0. Adding 0.001 to the noise fails on bike 17.
- `ride_replays_bit_exact_on_the_ports_force_body`,
  `flip_wrecks_and_is_put_back_bit_exact_on_the_ports_force_body`: F31b1's hybrid frame with the bike's
  force element (`*(e+0xc)` = bike `+0x3c`) run by `ground_step` as well as its preforce by
  `ground_head`, four times a frame (asserted). Its callees run in the interpreter on the bike in RAM,
  with only the changed words written around each one. The exceptions are the air drag and the peak
  rates, which run through `atv_moto`'s Rust ports. The smooth noise is the original's. All 459 and 465
  rows match their capture word for word, and the flip wrecks on row 125 and is put back on row 193.
  Mutations: skipping the controllers fails both (rows 354 and 376). Never setting the rear pair flag
  fails both (rows 136 and 125). A push-sign or noise mutation does *not* fail the replays: neither ride
  balances or pushes, and only the oracle covers those.
- `./kit check -- -p game-sim --test mxvatv_bike_ride --test mxvatv_step`: 26 passed. The same in
  `cargo test --release`.
- `./kit audit`: the same 4 leaks as on main (atv_wheel.rs, plan/DONE.md); none are in these files.

## Not verified / not 1:1

- In the replays, the Force body's callees (gravity, preload, backflip, air spin, engine, air control,
  wheel clear, contacts, suspension, tire forces, wheel integrate, rev limit) are still the
  original's. Only the air drag and peak rates run through `quad::Steps`' Rust ports. Running all of
  `quad::Steps` on the bike needs a reader for the bike's whole engine, wheels and collision state into
  the quad's types. That reader is F31b5's (the whole frame in pure Rust), together with F31b1's note on
  the two empty right-hand wheel slots.
- The balance and the push never fire in ride-2 or flip-1. Their only evidence is the oracle, whose
  noise is a stand-in table: the real smooth noise runs only in the replays, where the balance does not
  run.
- The `−0` fold covers the accumulators as they enter `balance`. If a `madd` mid-function came out
  `−0` (an underflow flushed to a signed zero), the original's second `0 + x` for the command would fold
  it and `move::balance` would not. That would take a flushed denormal, and nothing drew one.
- The oracle's guards read the wreck reason as non-zero, the push flag as 0/1 and the balance words as
  random or tied, as F31b1's did.
- The world's gravity is the image's (it lies outside the regions compared). The oracle does not vary it.
- Only the targeted test files were run, not the full replay suites. Original rides do not reach
  `handling::bike`. The one change shared with existing code is `step::balance`'s `−0` fold, and
  mxvatv_step passes with it.
