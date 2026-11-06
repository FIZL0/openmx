# 1 — rider wreck arm

Task S285. 2026-11-06.

## What I did

- `crates/game-sim/src/handling/rider.rs`: `wreck_arm` and its parts (`aim`, `let_go`,
  `release_bike`, `ground_y`, `clauses`), plus three callees as their own functions:
  - `release` (`0x001156f0`: a collidable taken off a shape's ignore list);
  - `pose_targets` (`0x004eedc0`: the bodies' pose targets set or cleared);
  - `body_target` (the bodies' vt `+0x68`, `0x00493b00`: `+0x150` from a quad).

  `modes` now calls `wreck_arm`. It takes the terrain's map (`Option<&track::Terrain>`), the same way
  `bone::solve` does, because the ground query is the ported `atv_ground::query` read through
  `ground::read_terrain`.
- `Call::WreckArm` is gone. Two new calls: `Call::Gather` (the nearby gather, a broad-phase call made
  the way `bone::Scene` makes its searches) and `Call::BodyTarget` (a virtual). The aim force's
  switch-off reuses `Call::Switch`.
- `tests/mxvatv_rider_modes.rs`: the S278b oracle now runs the arm. It loads Nat05's map, draws the
  arm's words, and tallies each arm path.
- research/oracle.txt `# S285`.
- Nothing from opentennis: it has no ragdoll rider.

## What the original does

Read with `./kit dis 0x004efe10 0x004f0280 -p atv`. Ghidra cuts the function at the ground query.

- **Aim**: while `+0x640` runs, `+0x660` is `+0x650`. It is taken as it is (`+0x62c`), or turned by
  the main body's orientation (`+0x630`), or else by the record's. The clock then runs down by dt.
  Once the clock has run out, an aim force (`+0x5bc`, the rider world's force whose `+0xc` points at
  `+0x660`) that is still on is aimed at (0,0,0,1) and switched off.
- **Let go**: while `+0x644` runs it runs down. Once it has run out, both holds `+0x550`/`+0x554` are
  switched off if either is on.
- The **gather** (`0x001d6a60(+0x5b8, part 0's sphere, 3)`), then S278's touched walk.
- **Release**: once, when the main body is more than radius + 1 from the record's `+0x1dc` collidable,
  each part's sphere drops it from its ignore list.
- **Ground**: MD76's query on a copy of the main body's position; only its `y` is read.
- **Clauses** on the flag's clock t:
  - (t 0, both clocks out, not high, more than `+0x850` over the ground): targets set, high on, slow off;
  - else (t > 0 and high): targets cleared, high off;
  - else (t past `+0x854`, or speed under `+0x858`, and not slow): slow on, targets set, and the main
    body's target `+0x840`.
- **Fade**: `+0x5d8` runs down while above 0.

## How it was proved

- **Oracle `the_riders_mode_machine_is_the_originals`** (`tests/mxvatv_rider_modes.rs`): 60 000 drawn
  frames of state 4's rider, 24 973 of them in mode 2. In the original, nothing in the arm is hooked
  except the gather and the two on/off virtuals, which only log. The release, `0x004eedc0`, the bodies'
  vt `+0x68` and the ground query all run in the interpreter. The query runs on Nat05's map on both
  sides. The main body is placed over real ground (heights from the port's query at 64 spots near state
  4's rider), at `+0x850` over it exactly one time in five. The draws also cover:
  - the clocks at 0, −0 and dt;
  - spheres' ignore lists holding the collidable or not, with counts 0–4 and −1;
  - the collidable at exactly radius + 1;
  - wreck kind 13 among the kinds.

  Every word written outside the stack and the query's ring matches, and so do the calls. Path counts,
  read off a forced failure:

  | Path | Count |
  |---|---|
  | aim as is / by body / by record | 3901 / 3964 / 3821 |
  | force off | 6657 |
  | let go | 10236 |
  | released (off a list) | 4931 (4926) |
  | high set | 1826 |
  | high cleared | 5146 |
  | slow set, late | 886 |
  | slow set, slow | 2749 |
  | targets set with kind 13 | 906 |
  | fade ran | 16707 |
- Two mutants each fail the oracle: the height test `<=` changed to `<`, and the kind-13 offsets
  swapped.
- `./kit check --all` (in `./kit done`).

## Not verified / not 1:1

- **The nearby gather is still a call** (`Call::Gather`), logged and not run. It is ported on Rust data
  (`atv_world`, MD83) but not over the rider's RAM list. Its effect on this frame is unchecked: the arm
  reads nothing it writes, but the rider world's step does.
- **The two on/off virtuals** (the aim force's slot, `0x0049bd80`, and the holds', `0x0049bd30`) are
  calls. They are logged, not run, and not ported.
- **The ground query's ring cache is not kept**: the original writes its ring of read cells and the
  terrain's `+0x9e0`/`+0x9e4`, and the port does not. The compare leaves those words out. The query's
  answer does not depend on them (MD76).
- **`BodyTarget` is not in the call compare**: the original's vt `+0x68` runs as it is (a hook would
  replace it), so the port's calls are checked only through the `+0x150` words they write. A call that
  rewrites a value already there would go unseen. The pose list is drawn fresh each frame, which makes
  that unlikely. The test checks that every body's vt `+0x68` is `0x00493b00`. Other riders' body
  vtables were not looked at.
- **One map and one rider**: the ground is Nat05's, the rider state 4's. A terrain with a zero cell
  reader (`+0x94`), where the query writes nothing, is ported but never drawn.
- **No live capture or replay**: `modes` is still not wired into a ride path. Its other callees are
  still open: S286 (the pose and settle virtuals), S287 (the riding reset) and S288 (the wreck entry).
- **The PC exe was not read** for this stretch. The rider class's function was not located in it.
  Every branch was read off the PS2 disassembly and proved by the oracle.

## Next

Filed S289 for the calls that are still left to the interpreter (the two on/off virtuals and the gather).

S286–S288, then wire `modes` (with the map) into the rider's frame beside S278a's world (S280).
