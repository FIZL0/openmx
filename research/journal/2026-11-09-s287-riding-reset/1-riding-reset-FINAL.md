# 1 — riding reset (FINAL)

Task S287. 2026-11-09.

## What I did

- Ported `game_sim::handling::rider::riding_reset(m, rider, dt)`. It is in its own section at the end of
  `rider.rs` (S287). The mode machine still makes `Call::RidingReset` (into modes 1 and 4), and the
  harnesses now serve that call with the port.
- I also ported the four ragdoll-body virtuals the reset calls: `body_mass` (vt `+0x18`), `body_inertia`
  (`+0x60`), `body_at` (`+0x20`), plus the existing `body_target` (`+0x68`). Each goes through a new
  `Call` (`BodyMass`, `BodyInertia`, `BodyAt`), following S285's `BodyTarget`, so a harness can check
  the vtable slot before it serves the call.
- Ported the shape's ignore-list add (0x00115600) as `rider::ignore`. This is the counterpart of S285's
  `release`. Its heap resize is `Call::Grow`.
- Fixed `rider.rs`'s `copy_quad`. It went through `Vf::from_bits`, which flushes denormals, so it was
  not the raw `lq`/`sq` move it claims to be. It now uses `copy_words`. The fix reaches `body_target`, the
  respawn and the aim, which all use `copy_quad`. The S278b, S285 and S286 oracles still pass, and so do
  both rider-leaf replays.
- `tests/mxvatv_bike_ride.rs`: `LeafRider` now serves `RidingReset` with the port and the new body calls
  with their ports, and serves `Grow` with the original's resize. The flip-1 tally is updated: one
  `= riding reset`; 18 body mass/inertia/at calls; 18 more body targets; 19 more switches (5 + 4 + the
  force + 9 parts); one more pose and one more settle. The original's reset used to make those calls
  itself.
- opentennis has no ragdoll, so I took nothing from it. The shape comes from S285/S286 in this file.

## What the original does

Read with `./kit fn 0x004eef40 -o atv`, `./kit dis -p atv 0x004eef40 0x004ef320`, the body vtable
`./kit dis -p atv 0x003cd200 0x003cd290`, its slots `./kit dis -p atv 0x00493cc0 / 0x00493b20 /
0x00493bf0 0x00493cc0 / 0x00493b00`, and `./kit dis -p atv 0x00115600 0x001156e4`. Full notes are in
research/oracle.txt `# S287`. In words:

1. If `+0x25c` is 0, the ragdoll is posed (vt `+0xd0`, a1 0) and settled (vt `+0xcc`).
2. The ragdoll world's iterations and rate are set to the riding ones (`+0x544`, `+0x54c`). On state 4
   they already match: 3 and 27.0.
3. The four holds and `+0x578` are switched on. The four parts at `+0x57c..+0x588` are switched on or
   off by `+0x4f0`. The aim force is switched off.
4. Each part of the list is switched off and its touched flag cleared. With a record, the part's sphere
   gets the record's collidable on its ignore list. This undoes S285's release, so the parts again ignore
   the bike they ride.
5. Each joint of the pose list (`+0x3b0`, 17 on state 4) is stopped (`+0x80` = 0) and gets the entry's
   strength (`+0x10`) and damping ratio (`+0x20`). Its damping is set per axis to ratio × 2 ×
   √(second body's inertia × strength), which is critical damping, and `+0xd4` gets the entry's `+0xc`.
6. Each of the 18 bodies gets its mass (`+0x18` and its inverse at `+0x1c`) and its inertia (`+0x130`,
   with inverses at `+0x140`, 0 where the inertia is not above 0; the call also writes 1.0 into the
   source entry's `w`). It also gets its pose target, and rodata's `(1, 1, 1, 1)` at `+0x30`.
7. `+0x38c`, `+0x264` and `+0x254` are set to 1.

## How it was proved

- Function oracle, `tests/mxvatv_rider_modes.rs` `the_riding_reset_is_the_originals`, 20,000 generated
  cases on state 4's rider image. The original runs whole in the interpreter. Pose, settle and every
  switch slot are hooked to log; the heap resize is hooked to a fake. The body virtuals run as they are.
  The port runs from the same words. Every word written off the stack is equal, and the call logs are
  equal, arguments included. Each case draws:
  - `+0x25c` and `+0x4f0`, and the riding words (including inf and denormals);
  - the three counts: the real one, 0 or −1;
  - the record: there or none; its collidable: none or the fake one;
  - each sphere's ignore list: count 0..4 or −1, entries being the collidable, 0 or others;
  - strengths, ratios, `+0xc`, the joints' bodies' inertias and the pose list's masses, inertias and
    targets: signed zeros, denormals, inf, negatives (√ of a negative) and positives.
- Path counts from one run: posed 9,967 / not 10,033; extra parts on 9,902 / off 10,098; ignore with no
  record 2,354, no collidable 1,987, listed already 7,202, into an empty entry 6,333, grown 7,627;
  joints run 12,057 / none 7,943; bodies run 12,042 / none 7,958; inertia axes above 0 533,333 / not
  116,935. Each path is required by name, at least 20 times.
- Mutant: `x <= 0.0` changed to `x < 0.0` in `body_inertia`, and the oracle failed at body 2 `+0x140`.
- The first oracle run caught the `copy_quad` denormal flush (body 2 `+0x130`).
- Live: `flip_replays_with_the_rider_leaf_on_the_ports` (C15 flip-1, put back on row 193) still replays
  bit-exact with the reset served by the port. So does ride-2, which never resets.

## Not verified / not 1:1

- **The heap resize (0x001b7470) is not ported.** It is the engine allocator, and it is a call
  (`Call::Grow`). The oracle hooks it to the same fake on both sides, so it proves the arguments and
  everything done with the answer, but not the allocator. The flip-1 replay never grows a list. Its
  spheres already have room.
- **The PC exe's version was not read.** I found no mapping for this function in research/mxvatv-pc.md.
  The function makes no decisions beyond the counts and the `<= 0` test, and the oracle covers every
  branch.
- **`+0x264` and `+0x254` are named only by what writes them here.** `+0x264` is S285's targets latch.
  What reads `+0x254` and `+0x38c` is outside this function.
- **The `copy_quad` fix was checked against existing oracles only.** Those are S278b, S285 and S286, plus
  the two rider-leaf replays. None of the earlier oracles fed denormals into the quadwords it moves, so
  the old behaviour was wrong but never exercised.

## Next

Nothing for S287.
