# 1 — mass (FINAL)

Task F31b4c. 2026-11-04. Part of the F31b split
(research/journal/2026-11-03-f31b-mxvatv-bike-pre-process/README.md).

## What I did

- `crates/game-sim/src/handling/mass.rs` (new; `pub mod mass;` in handling.rs) ports MX vs. ATV's
  `[Mass Properties]`:
  - `mass::Part::read(section, half_box, curve)`: the rigid-body part reader with every key, in the
    reader's order and with its defaults. It fills the flag bits, the gravity block, the body (mass,
    inertia from the model box, radius, dampings, centre of gravity) and the collision record (mass,
    inertia and inverse, springs, sweep, angular impulse, moment scales, frictions).
  - `mass::Bike::read(section)`: the bike loader's second read. It covers `Inertia` (over the body's),
    `CofG`/`WreckCofG`/`BalanceCofG`, the three travels and `YawRateLimit`.
  - `Part::words(Layout)` and `Bike::words(bike, body)` give `(address, mask, value)` lists. These are
    what F31b5's pure-Rust frame can seed from: `handling::record::read` and `seed::read` read words.
  - `mass::flag` is the game's flag parse. Prefixes `T`/`ON`/`Y` give 1 and `F`/`OFF`/`N` give 0;
    anything else goes to `strtol` base 0. This differs from `Section::int`, where `ONE` is 0.
  - `mass::facing` is the sweep cosine. `mass::block_inertia` and `mass::record_inertia` are public for
    the state-4 box search.
- `crates/ps2-oracle/src/ee.rs`: the interpreter now handles `cfc2 rt, vi22`, which reads `Q`. The
  record setter's `vdiv` result needs it, and it is one line.
- Tests:
  - `tests/mxvatv_mass.rs`: `mxvatv_mass_properties_every_word_is_the_originals`.
  - `tests/mxvatv_bike_ride.rs`: `the_mass_words_are_state4s_word_for_word`,
    `ride_replays_bit_exact_on_the_ports_mass_words` and
    `flip_wrecks_and_is_put_back_bit_exact_on_the_ports_mass_words`.
- `research/oracle.txt # F31b4c` has the reader, its setters, the keys, the defaults and the offsets
  (comment rows only).
- The bike file. State 4's bike is a 420-lb one (mass 13.04348 = 420 × 0.0310559), and all eight
  420-lb `mx_{race,pro}{250,500}{a,p}.phy` files have byte-identical `[Mass Properties]`. I took
  `Veh\mx_race250p.phy` out of the disc's `MXVSATV.PSR` (bsdtar on the ISO, then the game-sim
  archive reader) and put it at `context/mxvatv-ps2/phy/mx_race250p.phy`, which is git-ignored. The
  state-4 test and the two replays skip without it.

## Where SLUS-21104 differs from MX Unleashed's reader

- **Body mass and inertia.** The mass is `Weight × 0.0310559`. The inertia is the model box as a solid
  block times `InertiaScale`, whose default is `(2, 2, 2, 1)` (the ones vector times 2), not 1. The
  bike loader then puts the file's `Inertia` over the body's inertia but not over the record's, so the
  record keeps the box's inertia.
- **Record inertia.** It is the box inertia times `cm × inv_m` (a VU multiply). Its inverse uses FPU
  divides for x and y and the VU `vdiv` for z, and is all `(0,0,0,1)` if any lane is ≤ 0. A
  `CollisionWeight` ≤ 0 writes only the offset; the record's mass and inertia are not written.
- **Sweep facing.** The facing is the curve-table cosine of `HighSpeedMinCollisionAngle` taken as
  radians, with the slope term `adda(cos).msub(step, sin × frac)`. MX Unleashed converts degrees to
  radians first and takes no slope. State 4's 45 gives cos(45 rad) = 0.5253217.
- **High-speed check default.** `CheckForHighSpeedCollisions` defaults to 0.
- **Springs.** The stiffness per pound is `|32.2 × (k × body mass)|`. The damping per mass is
  `(2d) × sqrt(mb × k')`, or 0. A `-1` body key takes the spring's value. The moment scales'
  "same as" test is all four words equal to `(-1,-1,-1,1)`.

## How it was proved

- **Every word of the reader.** The oracle is SLUS-21104 off state 4's RAM image, running the reader
  with its real setters. Only the ini string lookup is hooked: the game's own number parse and its own
  flag parse (`strncasecmp` + `strtol`) run, along with the model box hook.
  - Input: 10,000 generated files. Each key is present ¾ of the time. There is a random half box per
    file, including zero and negative lanes. Flags draw from 22 spellings, `-1`s are drawn for the
    body keys, and the moment-scale "same" vectors are drawn in four spellings.
  - Check: every word of the object (0x400), gravity block (0x100), record (0x400) and body (0x400)
    must equal the port's words laid over the pre-call RAM. That covers the words it writes and the
    words it leaves alone.
  - Branch coverage: every one of 19 paths is required ≥ 50 times. They include no mass, no collision
    mass, a record inertia not over 0, a body inertia lane not over 0, each `-1` body key, both
    moment-scale "same" cases, spring damping 0, a facing behind, the `ON` prefix, `strtol` base 0 and
    collision disabled.
  - Mutations:
    - FPU divide in place of `vdiv` for the record's z inverse: fails at file 5.
    - The facing as `msub(step × frac, sin)`: fails at file 2066.
    - `2 × (d × sqrt)` in place of `(2d) × sqrt`: still passes. ×2 is exact, so this is an equivalent
      mutant.
- **State 4 word for word.** `the_mass_words_are_state4s_word_for_word` reads `mx_race250p.phy` plus
  the load's box, takes `Part` then `Bike`, keeps the last write per address, and finds all 134 words
  equal state 4's. These are the body, record, gravity block, flag bits, and the bike's
  `+0x1120..+0x1150` and travels.
- **Replays.** Both runs write the port's 134 words over the image and then run the hybrid
  frame `port_record_frame` (F31b4a's most-ported frame).
  - Ride-2: 459 rows, bit-exact, never wrecks.
  - Flip-1: 465 rows, bit-exact, wrecked on row 125 and put back on row 193.

## Not verified / not 1:1

- **The model box is inferred, not read.** For state 4 the load's half box (1.4301566, 2.3848903,
  3.8156264) is worked back from the record's inertia: I solved for the squares and walked ±48 ulps per
  lane until the record's bits came out. So the record's `+0x60` is fitted, not predicted. The game's
  box call on state 4's node now gives a larger box (block inertia about 11% larger in x, presumably
  because the wheel nodes moved since load), and I did not port that call. The generated-file oracle
  hooks the box, so the box→inertia arithmetic is proven but the box itself is not. The port takes the
  box as an input.
- **The body's centre-of-gravity setter also moves the body.** It shifts `+0x60`/`+0x70` by the change
  in CofG, turned by `+0x190`. That shift is not ported: `Part` gives the CofG only. The oracle's body
  is zeroed (orientation 0), so the shift is 0 there and nothing proves it.
- **The record's chassis branch is not ported.** When record `+0xe0` is non-zero the inertia comes from
  a chassis. The bike has none, and the oracle sets `+0xe4` to the body and `+0xe0` to 0.
- **`YawRateLimit` missing.** Without it the loader passes stack junk as the rate's y. The port
  returns `None` and writes no rates. The rates' `w` (`+0x21c`) is stack junk too and is never written
  by the port; it is 0 in state 4.
- **The bike loader's re-read is checked only on state 4's one file.** It is not run in the
  interpreter over generated files: it is inline in the bike loader and cannot be called on its own.
- **`CollisionEnabled` off runs the reader's removal tail.** In the oracle there are no shapes and no
  model library, so the tail does nothing. Its removals of real shapes are not covered.
- **Vectors with fewer than three values, and space-separated vectors.** These are not generated.
  With fewer than three values the game passes `strtod` a null pointer. `Ini::parse` takes the blanks
  out of a line the way the game's file loader does, and the hook does not, so the oracle sticks to
  commas.
- **`strtol` overflow.** Flag values that do not fit an `int` are not generated.
- **The product does not use the port yet.** `handling::mxvatv_phy` still builds an MX vs. ATV ride
  from `Phy::read` plus `MassPart`. Three things there still differ from the original and are left to
  MD63e:
  - the facing in degrees;
  - `CheckForHighSpeedCollisions` defaulting on;
  - the chassis mass as W/32.2.

  `mass::Part` is what F31b5's pure-Rust frame seeds from. Changing the MXU-reader ride's
  `Phy` would move every MX vs. ATV ride and is MD63e's to decide.
- **The wreck part's record.** At row 125 the bike swaps to the wreck record. Its mass words come from
  the wreck part's own read, which is not ported here; that is F31b4b's.

## Next

The first attempt (wip/F31b4c) could not be ticked: its gate was red on P294 (web.rs's store test),
since fixed on main. This attempt cherry-picks that work onto today's main: F31b4b's, F39's and F41's
appends to oracle.txt and mxvatv_bike_ride.rs kept beside these, and its kiowa_capture.rs skip dropped
because main already skips there.

This slot had none of the inputs: `mx_race250p.phy` came from s2's context, `state04.*.bin` and `nat05/`
from s1's, C15's captures and tapes from kept-context/c15. With them the three bike-ride mass tests and
the 10,000-file oracle ran and passed (no skips). `./kit done`'s gate was green (4837 tests); its audit was
red on four leaks already on main (atv_wheel.rs, plan/DONE.md), none from this task, so it was passed
with `--skip-audit`; K26 (critical) already has them.

F31b5 can seed its pure-Rust frame's body and record from `mass::Part::words` plus `mass::Bike::words`
on `mx_race250p.phy` with the box above. MD63e can move `mxvatv_phy` onto `mass::Part`.
