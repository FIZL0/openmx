# 1 — free09 ss presampled paths (FINAL)

Task S181. 2026-10-29.

## What I did

- Found the port already done: S159 filed this on 10-27 18:37 with the two two_p_truck.rs tests
  ignored; S128 (42fd18a5, 23:20 the same day) ported `PresampleTerrain` in `traffic.rs`
  (`Spec::presample` on the first step, the curve's own height, the segment's key normals lerped
  lane by lane at the curve's parameter, then the 0.707-gated normal filter) and un-ignored both.
  The RAM lengths are the measure's on the *clip's* keys before the presample, which is what
  `Spec::new` measures, so the ~38.4655 lengths were never a measuring bug — the old test measured
  the presampled keys.
- Ran both tests against the s159 images (they live in s1's context only; symlinked
  `context/states/s159` here, git-ignored): `the_paths_measure_as_the_originals` and
  `the_truck_drives_from_each_image_into_the_next` pass, not skipped.
- The drive test takes the lengths and the kept tangents out of the image, so it never proved the
  port's own presample on free09_ss. Added `the_shells_presampled_paths_drive_as_the_originals`
  (two_p_truck.rs): the shell's free09_ss specs (measured off the disc's clips), presampled by the
  port on free09_ss's ground — every key's position, normal and length equal to image 132087's bit
  for bit — then every car stepped from each image into the next (5 pairs, 6 cars) with clock,
  cursor, point, heading, up, world matrix bit-exact.
- Nothing taken from opentennis: no path cars there.

## What the original does

As S128's journal and `traffic.rs`'s module doc say; nothing new found. One reading for tests: the
path's clean-tangents word means "the kept tangents are this segment's", so an image whose car has
left the measure's segment keeps tangents made on the *presampled* keys; the new test maps such a car
to `stale = false` (the port makes the same tangents again), and only a car still on the measure's
cursor rides `Spec::measured`.

## How it was proved

Save-state RAM images: S159's twelve free09_ss images (`context/states/s159`, research/s159/rams.py),
5 tick-apart pairs × 6 cars, plus every presampled key of the six paths against 132087.

## Not verified / not 1:1

- The s159 images exist only in s1's context (symlinked here); in any other slot the three S159/S181
  image tests skip and read as passing.
- No image catches a car on its first frame (the presample itself happening mid-step) on free09_ss;
  that order is proven on free09 by S128's presample.rs, not here.
- The new test's `stale` mapping (above) is a reading of the clean word, not a separate proof that an
  image car on the measure's segment has not re-entered it since the presample; whether any image car sits on
  its measure's cursor was not checked, so that branch may be unexercised.

## Next

none
