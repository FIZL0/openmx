# 1 — gate override oracle (FINAL)

Task S298. 2026-11-13.

## What I did

- Added `the_supercross_loaders_gate_override_matches_over_ten_thousand_scenes` to
  `crates/game-sim/tests/race.rs`: it runs the supercross activity's whole loader (`./kit fn 0x2f9f40`)
  in Ee on a fake activity, scene, track and app, and compares the grid gate, the line and the start gate
  word for word against `Supercross::place_drop_gate` / `place_finish_line` on the same props.
- No port change: the S13e port matched on every case first time. A mutant (the line's cross product's
  operands swapped) fails the test, so the comparison bites.
- Nothing taken from opentennis: it has no race gates.

## What the original does

The loader asks the scene (its vtable slot `+0x120`, name, 1) for "dropgate" and then "finishgate"; when
the scene has no finish it asks the global prop list (`./kit fn 0x212af0`), whose holder's `+0xb0 → +0x30`
is the prop. With a drop prop: its bound (`./kit fn 0x1b44b0`) gives half width and height; the track's
grid gate (track `+0x58`, when there) takes the prop's position, 2×half width, height, across and
`across × up` (whole quads stored), then has its path spot looked up (`./kit fn 0x26b410`, track,
gate+0x40, gate+0x94, 0, 1, 0); the start gate (activity `+0x3dc`) gets flag 0x02, the position, width,
1000 high, across and look (`xyz` only), set back 4.5 along the look. With a finish prop and a track line
(`+0x5c`): the line takes the position (`xyz`), its bound, the across (`xyz`), its path spot, then the
path's direction at the spot's node (`./kit fn 0x26e630`) made square to the across and normalised with
`vrsqrt`.

## How it was proved

Function oracle, 10,000 generated scenes (seed `0x5298_0dd5_9a7e_0001`): drop prop present 7/8, grid and
line gates present 5/6 each, the finish from the scene, the fallback or nowhere; prop rows unit vectors
or track-sized spots with `Pick::stray` words (denormals, Inf/NaN, signed zeros, any bits) and any `w`;
gates pre-filled with random words in every lane so each untouched lane is checked too. Hooks: the
scene's lookup, the fallback, the bound, the path spot and the path direction. Besides the gate words,
the test checks what the loader asked: both names in order, the fallback only when the scene had no
finish, one bound per prop placed, the path-spot arguments for each moved track gate and the line's
spot node handed to the path direction. Branch counts of one run: drop 8764, grid 7334, line 7388
(1050 by the fallback), no finish 1165, finish but no line 1447.

## Not verified / not 1:1

- **The scene-name branch is not generated.** With app `+0x3cc` and its `+0xb4` set, the drop prop's
  name is formatted (`./kit fn 0x411130`) and lowercased (`./kit fn 0x413118`) instead of "dropgate";
  the test always leaves `+0x3cc` 0. The port takes a `Prop` and has no name lookup, so nothing in the
  port reads that branch; which name a scene uses is the caller's business and stays unproved.
- **The hooked functions are not proved here**: the prop lookups and the bound are unported (S13e's
  note stands); path_spot and path_ahead have their own S293/S282b oracles. The hooks write fixed
  answers, so a bound that also wrote other words would go unseen.
- **The path direction failing is not imitated.** The original's direction function leaves its out
  quad untouched on a last node / zero-length node, so the line's normal is then built from whatever
  was on the loader's stack. The hook always answers; the port takes `ahead` from its caller.
- **The start-gate and lane-object allocations are skipped** (activity `+0x3dc` and `+0x3ac` set
  beforehand): the gate constructor and the lane object are not part of the gate override.
- **The MX vs. ATV PC exe was not read** for this: the task proves an existing port against the PS2
  loader itself, every branch of which the generator reaches.

## Next

Nothing.
