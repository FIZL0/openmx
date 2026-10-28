# 1 — bird trigger shape (FINAL)

Task V75. 2026-10-28.

## What I did

- `creature.rs`: a trigger's `Box` is **half extents** about `Position` (V44 already used it that way;
  now proved). `Trigger::struck(a, b)` is the original's segment-against-box leaf, ported op for op on
  `ps2_float` (`segment_touches_box`: VU transform and sums, FPU `mula`/`msub`/`madd` for the three cross
  axes, inclusive). `holds(p)` is a segment of no length.
- What touches a trigger is the riding bike's **body rays** (`bikem.col`, eleven segments) at the chassis
  transform, not the rider's position point. New `creature::Rays`; `Creatures::step` takes one per seat;
  `main.rs` `creature_rays` puts `bike_col.rays.segments` through `ride.transform()` (a seat with no bike
  collision falls back to its position as a point).
- The trigger frame: up and look normalised, right = up × look, as the reader's basis call builds it with
  squaring off. Every bird trigger on the disc has look ⟂ up, so it is a rotation (tested).
- Follow-ups filed: V103 (V71 fx and V73 rain triggers still touch with the rider point), V104 (path cars
  strike triggers too; IgnoreTrigger / EnterTest / ExitTest / percent's contact-last-frame clause).
- Nothing in opentennis bears on this.

## What the original does

Recipes: `./kit fn 0x001c69b0` (trigger reader), `./kit fn 0x001e7b10`, `./kit fn 0x001e8050`,
`./kit fn 0x001e7f70`, `./kit dis 0x00106590 --len 0x200`, `./kit fn 0x001ebe70 --limit 400`,
`./kit fn 0x0011a9d0`. Details are in research/oracle.txt under "V75".

- The reader makes a sphere when `Radius > 0`, else a box shape (kind 6). The box set stores the centre as
  the zero vector and `Box` as read. Its bounding box is centre M·c, extent |R|·h; its radius is |h|;
  box-vs-box takes (c, h, M). So `Box` is half the size along each axis, about `Position`.
- The frame is the basis call on (Position, LookVector, UpVector, orthogonalise 0): row 1 the normalised
  up, row 2 the normalised look, row 0 their cross.
- A trigger fires only in the collision pass: the mover's pair test against the struck shape, then the
  struck object's contact callback (the trigger's: OneTimeOnly, RandomTriggerPercent, the actions).
  Nothing polls; the trigger's update only checks exits for actors already inside.
- Against a kind-6 box only the ray class (rays put into box space, then the ray tree, its node test and
  the segment-against-box leaf) and the mesh class have a pair test. Spheres, the kind-5 class and the
  hierarchy return 0. Of a riding bike: the body rays (the record's collide query) can hit; the sweep and
  the camera are spheres; the second ray probe ignores category 0x100, which is the category every trigger
  box is given (the word the reader copies reads 0x100 in every save state). So the body rays fire it.
- The leaf: both ends through the four rows (origin row summed first), less the centre; midpoint m and
  half length d; a miss if |m| − h − |d| > 0 on any box axis, else if any of the three cross axes
  (|m_i d_j − m_j d_i| > h_i |d_j| + h_j |d_i|) separates. Touching counts.

## How it was proved

- **Function oracle**, 40,000 generated inputs: `crates/game-render/tests/trigger_box.rs` runs the
  original leaf (`segment_box_touch` in oracle.txt) on the boot ELF and compares it with
  `creature::segment_touches_box`. Inputs: random rotated rows with segments about the box, long segments
  across it, points, and points exactly on a face, edge or corner or one ulp off. All 40,000 agree
  (21,146 touching). Changing the first axis test from `> 0` to `>= 0` fails it at case 114 (a face
  point), so the boundary cases bite.
- **Disc**: `tests/creature.rs` `every_bird_trigger_is_a_square_box`. The disc has four bird triggers:
  nat02 vulture_10/12/02 and nat19 vultures. All are boxes, look ⟂ up, percent 100, not one-time.
  vulture_10_trigger: at (3044.71, 173.61, 3086.15), Box (65.5, 29.75, 80.75), look (0.4956, 0, 0.8686).
  The test checks that 0.99 of the half along each axis is inside and 1.01 is outside.
- Who touches it: by reading the code (above), not by a run.

## Not verified / not 1:1

- **No PINE capture of a scare.** The task asked for one. No save state anywhere here is on a nat track
  (S98: "No nat track has been loaded in PCSX2"), the free tracks have no trigger boxes (every .p2s RAM
  image scanned for the trigger callback: none), and reaching nat02 means driving the menus blind. The
  shape and who strikes it rest on the function oracle and the code reading.
- The rays' placement (`main.rs` `creature_rays`: model segments × `ride.transform()`) and the point into
  box frame (`Trigger::struck`'s dot products) are host f32, not the original's VU transform. A ray that
  grazes a face may land an ulp the other way.
- The ray tree's node test (box against a tree node's bounds) is not ported. The port tests every one of
  the eleven segments against the leaf, so it can only touch where the original's tree also reaches the
  leaf, if the node test is conservative (assumed, not proved).
- Which rays the original tests while wrecked (or in the wreck ragdoll) is not checked. The port uses the
  body rays at `ride.transform()` in every state.
- The sphere trigger's segment test (`Radius > 0`) is not proved. No bird trigger uses one.
- IgnoreTrigger, EnterTest/ExitTest, and the percent roll's "contact last frame" clause are not ported
  (V104). Path cars firing triggers is not ported (V104).
- The V71 fx and V73 rain triggers still touch with the rider's point (V103).

## Next

None for V75; V103 and V104 carry the rest.
