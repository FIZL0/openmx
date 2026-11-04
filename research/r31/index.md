# R31 — why C16's joust from 137845 parts on tick 137863

After F32 the joust from 137845 (`tests/c16_fly.rs` `fly_two`, `JOUSTS`) held 18 ticks and parted on
tick 137863. The question was what differs there.

## Images

`context/states/s272/s272-137859..137866.ram` are kit copy 1 savestates (slots 65–72) taken off
`context/recordings/c16/fly.p2m2`. They were made with `research/c10/play_at.sh` at NominalScalar 0.05,
then converted with `research/s272/rams.py` pointed at copy 1's savestates
(`openmx-slots/pcsx2/s1/PCSX2/savestates`). Every save landed on the tick asked for. As with the other
s272 images, image `f` holds the state after tick `f − 1`. They are git-ignored, and the probe skips
without them.

## The probe

`r31_the_joust_against_images_to_137866` (run it with `--ignored`) runs `fly_two` from 137845 to 137866.
After each tick `t` it compares the following with image `t + 1`:

- the port's plane body and P2's wreck bike body, field by field;
- both contact lists, entry by entry.

| After tick | Plane | Wreck bike | Wreck list (orig/port) | Plane list (orig/port) |
|---|---|---|---|---|
| 137854 | bit for bit | (not down yet in the port) | | |
| 137855..137862 | bit for bit | bit for bit | 0/0, then 3/3 | 2/2, then 1/1 |
| 137863 | parts | parts | **4/3** | **2/1** |

Up to tick 137862 every body field matches: velocity, position, quat, rates, forces, moments and
accelerations. Every list entry matches too.

## What differs on 137863

On tick 137863 the original's **plane files a strike of its own on the wreck bike**. The port's plane
does not. These are the original's lists after tick 137863:

- **Plane:**
  - entry 0: kind 0, on the plane at (-7.5, 2.3962276, -1.6266332), normal (0.7411895, 0.011839539,
    0.6711917), point (4588.3716, 137.42819, 3515.7632), other the wreck bike's collision object;
  - entry 1: the kind-1 mirror of the wreck bike's own strike on the hull (the port's entry 0).
- **Wreck bike:**
  - entry 0: that strike's kind-1 mirror, the normal turned round;
  - entries 1–3: its own two terrain entries and its strike on the plane (the port's 0–2, bit for bit).

The plane's point is the far end of **plane ray 33**, (0, 4.4718924, -4.662307) → (-7.5, 2.3962276,
-1.6266332), a wingtip ray (x = -7.5) in `biplanem.col`.

The ray strikes the **wreck part's mesh collidable**. In image 137864 the wreck part's collision object
(`*(P2 + 0x7a8) + 0x120`) is the record of two collidables:

- 0xdc3040: its ray probe (shape kind 2, category 0x8);
- 0xdc38d0: a mesh (shape kind 0, class 2), with
  - category 0x9a058, which is `wreck_bike::COLLIDABLE`;
  - flags 0x94 (0x04, can be struck);
  - its +0x120 matrix equal to the wreck node's rows;
  - radius 4.303495;
  - mesh root box centre (-1.74e-5, 3.0165892, -0.0311873) and half (1.3144319, 1.563439, 3.7878752).

That root box is `wreckbikef.col`'s, and also `bike\col\bikef.col`'s (both are 8-vertex boxes with the
same root). Two words beside the plane's collision object, 0x105fae0 and 0x106030c, name the collidable,
so it is in the plane's search.

The order follows from the collision node's list, which goes plane, other, wreck (research/f30):

1. The plane searches first. Its ray's entry goes on its list, and the mirror goes at the head of the
   wreck bike's list.
2. The wreck bike searches. Its own entries follow the mirror, and its strike's mirror goes on the plane's
   list after the plane's entry.

## Why the port misses it

The port's plane meets the other riders through `rider_contact::plane_bikes` → `flight_search::Bikes`.
Those are the riding bikes' `bikef.col` boxes, and a wrecked rider has `in_world` false, so a downed
rider's bike is in no part of the plane's search.

`wreck_plane::search_downed` runs the other direction only: the wreck bike's rays against the plane's
hull. The wreck bike's hull (`wreckbikef.col` on its record's node, `wreck_bike::COLLIDABLE`) is already a
collidable in the props' worlds (`prop_world::Wreck`, S243). It is not one in the flown plane's world.

Until tick 137863 no plane ray reached the wreck bike, so the lists matched. On 137863 ray 33's end goes
inside the wreck bike's box.

## Follow-up

F35: put each downed wreck bike's hull in the flown plane's search world. Its entries and mirrors go in
the order above. Proof: this probe bit for bit on 137863, and the joust's pin moving past 18.

Landed in F35 (journal 2026-11-03-f35-plane-rays-strike-the-wreck-hull): the probe is bit for bit
through image 137866, and both joust runs hold the plane to the end. Planes flown together: F36.
