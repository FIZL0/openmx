# 1 — signed zeros in controllers (FINAL)

Task MD73. 2026-11-17.

## What I did

- `tests/mxvatv_step.rs`: a second generator, `Gen::rider_zeros`. It is `Gen::rider` with a signed
  zero in one of four of every float the three controllers integrate or multiply into a sum (the
  balance and pitch parameters, the ride's inertia, command, point, noise time and pitch integral,
  the stick, lean and brake, the body's force, torque, centre of gravity, Euler angles and both
  angular velocities, gravity), and `dt` 0 one time in eight.
- Three new oracles on it, 10,000 states each, with their own seeds:
  `mxvatv_stand_upright_keeps_signed_zeros`, `mxvatv_balance_keeps_signed_zeros`,
  `mxvatv_roll_pitch_keeps_signed_zeros`. The old tests are the same bodies with `Gen::rider` and the
  old seeds, so they cover what they covered before.
- Stand-upright and balance were green at once. Roll/pitch was red: state 4522 (a wheelie) left the
  pitch integral `+0` in the original and `-0` in the port.
- `handling/step.rs` `roll_pitch` now folds the pitch integral to `0 + x` on the ground before
  calling `move.rs`. `move.rs` is MX Unleashed's and stays as it is.

## What the original does

`./kit dis 4dc4f0 4dc970 -p atv` (roll/pitch) has six `adda.s` with a zero register:

- Wheelie: `integral = (0 + integral) + dt·(500·err)`, then `torque = fade·((0 + integral) + rate_gain·err)`.
- Stoppie: the speed fade's `map_clamp` (`0 + 0`, a constant), the target
  `(0 + max_cmd) + (2·lean)·max_cmd`, and the same two integral sums as the wheelie.

Only the first integral sum can change a stored word. After it, the integral starts from `+0`, so the
second sum can never see a `-0`. The stoppie's target can come out `-0` in the port where the
original has `+0` (a `-0` MaxStoppiePitchCommand and lean above 0). But that only reaches `err`, and
`err` only enters sums that start from a `+0`, so no stored word changes. On the ground every path
either does the first sum or zeroes the integral, so the port folds it there and leaves an airborne
integral alone (the airborne path does not touch it).

`./kit dis 4dc240 4dc4f0 -p atv` (balance): the inertia, command (twice) and point sums all start
`0 + x`. F31b2 had already folded those three words to `+0` in `step::balance`. The fold before the
noise sum reads the command after its first sum, which starts from `+0`, so it cannot be `-0`. The
point's `sub` reads the raw point, but `(±0 − c)·…` only feeds a sum starting from `+0`. The
flattened normal's `0 + y·y` never sees a `-0`.

`./kit dis 4dc970 4dcc00 -p atv` (stand-upright): the only `adda` is a map-clamp's constant `0 + 0`.

## How it was proved

- Function oracle against SLUS-21104 off `state04.ee.bin`, 10,000 signed-zero states per controller,
  every word of the bike, body, engine and both wheels compared (`tests/mxvatv_step.rs`).
- Red before the fix (roll/pitch state 4522), green after.
- Mutation: taking F31b2's fold out of `step::balance` reds `mxvatv_balance_keeps_signed_zeros`
  (state 86, the balance point `-0` in the port, `+0` in the original). Restored after.
- `./kit check -- -p game-sim --test mxvatv_step`: 20 passed.

## Not verified / not 1:1

- The stoppie target's `0 + max_cmd` is not ported. By the reasoning above it cannot change a stored
  word, and the oracle with `-0` MaxStoppiePitchCommand cases stays green without it.
- A sum whose result underflows past the smallest normal and flushes to `-0` is not generated. The
  ranges never come near 1e-38.
- No MX vs. ATV ride capture exists to replay a whole ride through these controllers (as in MD63b).

## Next

none
