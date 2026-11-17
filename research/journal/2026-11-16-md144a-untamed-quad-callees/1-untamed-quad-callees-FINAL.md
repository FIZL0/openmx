# 1 — untamed quad callees (FINAL)

Task MD144a. 2026-11-16.

## What I did

- Classified the six quad callees MD81b's walk left out (research/untamed-machines.md, "The quad's callees
  MD81b did not reach"; addresses in research/oracle.txt `# MD144a`). Pairing was by layout order and by
  the quad's vtable, each pair through `research/md81c/fp.py` and `research/md81d/opdiff.py`, then read side
  by side with `./kit fn` (SLUS-21104) and Untamed's decomp (`^// <addr>` in context/untamed/decomp/game.c).
- Tire reader, quad put-back, unwreck, remote wreck setter, part on/off, the two ray swaps: the same code
  on moved fields. The swaps sit one vtable slot away from where the −8 rule put them (Untamed's +0x484 and
  +0x488). The first pass paired them with the wrong functions.
- Animate, node pose, net unpack: changed, but none is on the sim ride (presentation and online only). Not
  ported.
- The wreck path held the one rework on the ride: **out of bounds and the throw**. The margin is 300 ft
  (`lui 0x4396`, Unleashed `0x4316` = 150). The spin goes through a 16-byte zeroed local and is stored
  whole, so `w` = 0 (Unleashed writes 1.0). Ported in `quad::Ride::throw` behind `Ride::untamed`.
- The quad's wreck test and wreck entry are the bike's slots. MD81a and MD93 proved the Untamed versions on
  the bike, but the quad never took them. I routed the two pieces the quad models:
  - the trick bound (`< 49` on Untamed's step) in `Ride::wreck_entries`
  - the preload stop in `Ride::enter`

## What the original does

- Out of bounds (`./kit fn 0x004a4f20 -o atv`; Untamed 0x004c8b40):
  - The test returns 0 unless the edge is armed. It asks the box test (unchanged) whether the body is
    within the margin of the world box's edge.
  - If so, the quad is thrown toward the box's middle at the body's height. The speed is min(2073.6,
    width/2) / 7.72, then 155.4 is added to `y`. The drag is halved.
  - The spin is three `rand` draws times 2⁻³¹ times 0.5, 0.4 and 0.7. Untamed's `w` is the zeroed local's
    0; Unleashed stores 1.0.
- Untamed's world keeps its box pointer at +0x42c (Unleashed +0x43c). The harness found this when it
  faulted.

## How it was proved

- Function oracle: `crates/game-sim/tests/untamed_atv/throw.rs` (`untamed_quad_throw_is_the_originals`).
  - It runs 4000 generated quads through SLUS-21701's out-of-bounds test in the interpreter, off
    context/untamed/ram/nat_or.bin, with `rand` hooked to the port's draws.
  - It compares against `quad::Ride::throw` on a spawned quad after `Ride::untamed`: thrown or not,
    velocity, angular velocity and drag, all bit for bit.
  - Each outcome has at least 200 cases: kept, thrown, and thrown only by the wider band.
- Mutant check: with Untamed's margin back to 150 and `w` back to 1, the test fails on case 0.
- The trick bound and the preload stop are not proved on the quad (see below). Their functions are proved
  on the bike by MD81a (`untamed_step.rs`) and MD93 (`untamed_reworked.rs`).

## Not verified / not 1:1

- The same-code verdicts (tire reader, put-back, unwreck, remote setter, part, ray swaps) are static
  only, with no oracle. That matches MD81b's method; the fingerprints match and the fields were read by eye.
- The quad's wreck-test bound (`< 49`) and the preload stop on entry have no quad-level test. The bike
  oracles prove the Untamed functions; the quad side is a one-line route each.
- The rest of Untamed's wreck entry is not on the quad: the hold word, entry flag and count, end stunts
  and clip, physics off. The headless quad models none of these, so they are left out as on Unleashed's.
- The vehicle put-back under the quad's put-back is MD93's rework. Routing it is MD168a's job (noted on
  MD168a).
- Animate (bounds +8) is not ported. game-render's quad pose compares stunt ids with Unleashed's 46, so on
  an Untamed quad its riding-pose check is 8 off. That is presentation only; follow-up filed.
- Node pose: the linkage aims take the general builder instead of the fast path. Last-bit differences are
  possible. Not checked, since nothing on the sim reads them.
- Net unpack: the filter fraction is no longer stored. Whether anything else reads the stored word in
  Untamed was not traced. Online only.
- Animate's dead branches (kinds 0 and 4, never reached) differ in their arguments; not followed.

## Next

- MD168a: give the quad on Untamed's step the vehicle put-back (`handling::untamed::put_back`).
- The game-render riding-pose follow-up (plan V).
