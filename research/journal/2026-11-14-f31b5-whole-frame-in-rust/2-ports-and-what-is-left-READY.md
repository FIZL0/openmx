# 2 — the pause, the object switches and the noise in Rust; what is left (READY)

Task F31b5. 2026-11-14, after the usage limit reset. Blocked on F31b5q..v, which hold what is left.

## What I did

- **The pause lookups** (`handling::tail::{name_bit, state_item, paused}`): the name registry walk
  (0x214b30, a2 0) and the state's item search (vt +0xac, 0x507710). The name strings come from the
  caller, so src holds no address. Oracle `the_pause_lookups_are_the_originals`: 12,000 generated
  registries and lists (short, long and 0x80+ names, aligned and unaligned copies), every path ≥100
  times. Passes. Switch `"paused"`.
- **Rumble** for the tail's ready check: dropped (`"rumble dropped"`). It is pad output only; the read
  watch stays clean. The wreck's own rumble is not dropped (it is in port_wreck's hand-outs, F31b5t).
- **The particle object switches** (`handling::effects::{object_light, object_douse, object_halt,
  object_pick}`): 0x50aaa0, 0x50ab00, 0x4e9f00 and the pick 0x50a920. Oracle
  `the_particle_object_switches_are_the_originals`: 16,000 cases, every word of the object and its
  player compared, all 8 pick paths (own rows × table × rate). Passes. Switch `"object switches"`.
- **The noise build**: the pure run builds with `SmoothNoise::build` and writes the perm/g1 tables and the
  generator back. The 2-D and 3-D tables stay unwritten; the read watch shows the flip never reads them.
  Switch `"noise build"`.

Result (`in_pure_rust`, both still bit-exact on every row, bike as the handing run's, read watch clean):

| | before this stretch | now |
|---|---|---|
| ride-2 | 757,675 | 546,207 |
| flip-1 | 591,831 | 245,874 |

## Tried and backed out

- Dropping light, douse and halt without porting them: the frame reads the object's +0xac/+0xb0.
- Dropping the pick: the read watch trips on flip row 244. The ground ring diffs seen then came from the
  particles settling differently.
- Dropping the emitter other-kind send (0x514e40): the read watch trips on ride-2 row 34 at 0x6737ac.
  The send runs a batch pool. It needs a port (F31b5q).

## What is left (the plan's follow-ups)

- F31b5q: the emitter other-kind send, 401k (ride-2) and 119k (flip-1).
- F31b5r: the atv heap. Realloc 0x1b7470 in the record pass (120k and 54k); new/free 0x1b7620/0x1b76d0
  in out Prepare (~4k).
- F31b5s: the per-frame wreck check 0x4a4f20 (~21k a ride).
- F31b5t: the wreck's hand-outs on the flip (~44k).
- F31b5u: the rider leaf's rand, cue, sound and wreck-entry heap; out Fade (~2k).
- F31b5v: the other five riders' PreProcess shown not to touch the bike. Not started.

When F31b5q..u land, adding their switches to `replay_in_pure_rust` and removing the `#[ignore]` on
`ride/flip_replays_in_pure_rust` closes the acceptance.

## Not verified / not 1:1

- The acceptance is not met: the frame still interprets the calls above.
- The frame runs over a RAM image held by `Ee`, not Rust structures.
- The noise build leaves the 2-D/3-D gradient tables as the state had them. Only the read watch on these
  two rides says nothing reads them.
- Dropped rumble: pad output is not compared anywhere.
- The five other riders' PreProcess claim (F31b5v) is not shown.
