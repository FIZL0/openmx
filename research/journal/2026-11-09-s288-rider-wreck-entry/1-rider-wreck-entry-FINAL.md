# 1 — rider wreck entry (FINAL)

Task S288. 2026-11-09.

## What I did

- Ported MX vs. ATV's rider wreck entry as `game_sim::handling::rider::wreck_entry`. rider::modes calls it
  going into mode 2, or when the wreck kind is 13 again. Three helpers came with it: `body_mass` and
  `body_inertia` (the bodies' own virtuals the entry calls) and `keep` (add a collidable to a sphere's
  ignore list).
- New `rider::Call` variants: `BodyMass`, `BodyInertia`, `Realloc` (the ignore list growing) and
  `WreckCue` (the call the entry ends on, still the original's).
- Oracle `the_riders_wreck_entry_is_the_originals` in `crates/game-sim/tests/mxvatv_rider_modes.rs`.
  `Port` there now serves body mass/inertia with the port and answers realloc with a fixed buffer.
- The bike-ride rider leaf (`crates/game-sim/tests/mxvatv_bike_ride.rs`, `LeafRider`) serves the wreck
  entry and the bodies' mass and inertia with the port. Realloc and the cue stay interpreted. The
  served-call census of `flip_replays_with_the_rider_leaf_on_the_ports` was updated:
  - "= wreck entry" 1, "= body mass" 18, "= body inertia" 18
  - body target 19 → 37, pose and settle 2 → 3, switch 1463 → 1482
  - rand 5 and wreck cue 1 are new
  - flip-1 stays bit-exact: same wreck rows, same RAM difference.
- Follow-up S300: port the wreck cue.
- Nothing was taken from opentennis; it has no counterpart.

## What the original does

Recipe: `./kit fn 0x004ed700 -p atv`. Jump table: `./kit dis -p atv 0x004ed700`, around the kind switch.
research/oracle.txt # S288 has the full word list. In words:

1. Pose the rider (still) and settle its ragdoll world.
2. Copy two of the rider's settings into the world.
3. Switch seven parts off and the aim force on.
4. Switch every ragdoll part on, and make each part's sphere ignore the bike's collidable. The list
   grows by one through the heap's realloc when it is full.
5. For each pose-list entry, reset that body's velocity word, copy its two coefficient rows, and set
   three damping lanes 2·f·√(m·k).
6. Give each holder's body the pose list's mass, inertia and target.
7. Clear the wreck arm's latches.
8. Then, by the record's wreck kind:
   - the aimed kinds (1–5, 8, 11, 12) set the two holds (5 at random), the holder the aim force
     follows, and a randomised aim and two clocks from a per-kind block of the rider's words; 5 and 12
     first kick the bodies from the record;
   - 6 and 7 give every body the record's velocity;
   - 13 tumbles every body: a random spin, and the record's velocity scaled 1.02/1.01/1.02 plus
     spin × offset;
   - 15 drops every body;
   - the rest do nothing.
9. Finally, tell the rider's `+0x8f4` object, which reads the kind itself.

Ghidra gets two things wrong. Both are in oracle.txt, and the port follows the machine code:

- The aim's w lane is the raw block word, not 1.0.
- The first switch call in kinds 1, 3 and 8 takes a1 from `$5`, which is still 1. Ghidra shows no
  argument.

## How it was proved

- **Function oracle**: 20 000 generated cases on state 4's rider RAM image, in
  `crates/game-sim/tests/mxvatv_rider_modes.rs` (`the_riders_wreck_entry_is_the_originals`). The
  original runs whole in the interpreter. Pose, settle, switch, rand, realloc and the cue are hooked and
  logged; body mass, inertia and target run as the port.
  - Every written word and the call log are equal.
  - Each kind ran about 1 100 times. Kind 5's four hold pairs ran 127 / 241 / 249 / 487 times.
  - The list grew 11 988 times and found a free slot 7 274 times. An inertia lane was ≤ 0 14 931 times.
  - A mutant (aim w forced to 1.0) fails the test.
- **Bit-exact replay**: C15 flip-1 with the rider leaf on the ports stays bit-exact with the port
  serving the wreck entry.

## Not verified / not 1:1

- **The wreck cue is not ported.** `rider::Call::WreckCue` is still the original's, interpreted in the
  bike-ride leaf and hooked in the oracle. It is follow-up S300.
- **Realloc is not modelled.** The oracle hooks the heap's realloc and returns a fixed buffer, so a real
  move-and-copy of the ignore list is not exercised. The port only calls it and stores what it returns.
  In the flip-1 replay no list grew, so no realloc ran.
- **Body mass, inertia and target are not checked on their own.** The oracle runs them as the port
  rather than the original's virtuals (vt +0x18, +0x60, +0x68), so they are proven only through their
  effect on the entry's written words. That covers every word they write, but each has no separate
  oracle. `body_target` was already proven in S285.
- **The PC executable was not consulted.** The port was read from the PS2 function and proven against
  it, so the PS2 is the reference either way.
- **One kind live.** No new PINE capture was made. Live evidence is the one flip-1 wreck in C15; only
  the oracle checks the other kinds.

## Next

Done. The task lands with `./kit done S288`.
