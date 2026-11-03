# 1 — reverse a tick behind the pad read (FINAL)

Task F13. 2026-11-02.

## Measurement

On C10's 2p capture (pad bytes and stamps around every 33/34 edge), both riders' body states 33/34
at row r are `Controls::reverse_rule(pad of row r−2, req_axes.z of row r−2)`. Put another way, they
come two rows after the row whose older pad half first shows the change. This holds for the
speed-led entry too (P2 at 132128, where the pad was already held and the speed fell under 5).

In port terms, tick t plays row t−1's pad (PAD_LEADS = 1), and the body asks with last tick's
`held.reverse`. P72's 1p capture agrees: the body runs 34 the row after +0x648 rises.

The old V90 replay was wrong for both riders, each in its own way:
- P1: it used rule(pad r, z r−1), which is 2 ticks early. That is the brief's 0.7997 ramp.
- P2: it used `hands[r−1]`, which is 1 tick early on pad edges and right on speed edges.

## Change

- `rider::PutBack::reverse: bool`. `frame_bumped` sets it from `ride.held.reverse` at the top of the
  tick, before the pad read changes it. `body_frame`'s Ask takes `reversing` from it. The table in
  `body_frame`'s doc has a row for it.
- The snapshot wire carries it (`wire!` in snapshot.rs). This adds 1 byte per rider, so
  `RIDERS_WIRE_LEN` is now 13611 and the digest moves. The rider hash does not walk `put_back`.
  `size_of::<Sim>` is unchanged because the byte fits in padding.
- rider_anim.rs `two_p_poses::run_seeded`: reverse is now the rule on `levers[r−2]` with row r−2's
  speed and airborne, for both riders.
- openmx tests/reverse_clips.rs: the body asks for the pair one tick after the command rises.

## Result

- Every 33/34 entry and exit of both riders in the 2p capture lands on the original's tick:
  P1 at 140354 and 140397; P2 at 132128, 132153, 132646, 132689, 138871 and 139067.
- With the eased, time and nodes seeds, P1's 34 fade (140354–140367) is within ~6e-6 and its 30
  fade (140397–140410) within ~5e-5. Both were ~4e-2 before. The fade test now counts 42 P1
  fading ticks within 1e-4 (was 14) and P2 still 50.
- Pins all went up:
  - `TWO_P_SAME` (12884, 11896), (12879, 11951)
  - `TWO_P_DRAWN` (12248, 9971), (12549, 6502)
  - `TWO_P_EASED_POSES` (12238, 11526, 11413, 9968, 10158), (12538, 6532, 3866, 6316, 7812)
- The rng pool sequence shifted as a side effect. P2's 83-tick 42-vs-40 wreck-limb run at 134389
  is now a single tick.

Some mismatches are left, and none of them come from reverse:
- one-tick 33↔34 flips that the axis drives;
- P1's riding residue at 140346–140353, where the ramp is 0;
- the wreck-limb picks.

## Not verified

- How the original delays the command is inferred from timing on two captures. I did not trace it
  in the ELF.
- I did not watch it in the shell, and did not compare it against PCSX2 live.
- The replay's throttle still uses the pad phase it had before. Only reverse was moved.
