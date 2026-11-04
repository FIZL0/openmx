# F30 — the plane's tick after P2's wreck 7 in C16

The question was why the port's plane after tick 137856 misses row 137858. It is the first tick after
P2's wreck 7, which begins on tick 137855.

- Original: (-28.616682, -0.0535129, 3.6458206).
- Port: (-28.609364, -0.0847827, 3.6421926).

The brief suspected the wreck bike's seed in `Crash::begin`. The seed is not the cause: the port
steps the wreck bike after the plane, where the original steps the two together.

## Images

`context/states/s272/s272-137854..137858.ram` are kit copy 1 savestates taken off
`context/recordings/c16/fly.p2m2` with `research/c10/play_at.sh` (slots 60–64), converted with
`research/s272/rams.py` pointed at copy 1's savestates. They are git-ignored. When they are missing,
the probes below skip.

As with the other s272 images, image `f` holds the state after tick `f − 1`. Image 137857 is the
original's state after tick 137856, and its plane velocity is row 137858's, to every digit.

## The seed

`tests/c16_fly.rs` `f30_wreck_bike_images_against_the_port` compares image 137856's wreck part with
the port's `Crash` after tick 137855 (`fly_two` from 137845). The wreck part is
`*(rider + 0x7a8)`: body at `+0x110`, object at `+0x120`.

- **Body:** equal on every field the original's layout holds (`read_body`).
- **Node rows:** a few ulps off. The port built them as `quat_to_mat4` of the wreck quat
  (`wreck_bike::node_of`). The original holds the look-at of the wreck axes, which is how
  `Ride::place_wreck` (S263) builds them. `Crash::begin` now sets them that way, and they match
  bit for bit.

  This barely moves tick 137856: the port's plane velocity y goes from -0.08526486 to -0.08526538.
- **Contact lists after tick 137856:** both match the original's on every printed digit.
  - The wreck bike's list: two terrain entries, then the kind-0 entry on the plane.
  - The plane's list: one kind-1 mirror on the wreck bike.

## The tick

`f30_the_originals_tick_137856` runs the original's collision node update (`0x0026f800`, `dt` taken
from image 137857's `frame_dt`) on image 137856 in the oracle, logging each contact resolve
(`0x00270dc0`). The node lists four objects: the plane, another object (body 0x1a8c7d0, empty list,
probably plane 1), the wreck bike, and one more.

The resolves run plane, other, wreck in each of the two substeps. All bodies are in one dynamics
world, so the wreck bike's resolve runs after the plane's in every substep before both integrate. The
oracle's plane and wreck bodies after the call are image 137857's bit for bit: velocity, position,
rates, quat, last_force and last_moment.

The port (S276, `wreck_plane::search_downed` / `step_downed`, and `frames_together_props`) runs the
plane's whole frame with the wreck body as a mirror side. It then steps the wreck bike's whole world
against the plane's post-frame body. Making the port match means stepping the downed wreck bikes as
bodies of the plane's world: their force listener after the planes', and their integrate in the same
substep. That is F32.

## F32: done

The wreck bike is now a body of the plane's world (`wreck_plane::InWorld`, `flight::Met::downed`).
`search_downed_by` files a copy of each searched wreck bike in its first plane's met. The plane's frame
(`Flight::frame_met`/`frame_props`, and `frames_together_props` for planes flown together) counts it among
its bodies. Each substep its force listener (gravity, its list resolved against the props it struck, then
the planes' bodies) runs after the planes', and its body is integrated with theirs. Its preforce clears
its shock peak, and its post sets its transform. `step_downed_by` hands the stepped bike back. A frame that
never ran leaves the bike to the old step after.

`tests/c16_fly.rs` `f32_the_plane_and_wreck_bike_are_the_images` pins it: after ticks 137854..137857, the
port's plane body is image `t + 1`'s bit for bit, velocity and position, and so is the wreck bike's. Row
137858 is (-28.616682, -0.05351289, 3.6458206). The joust from 137845 now holds 18 ticks (11 before) and
parts on tick 137863. From 137810 it holds 53 (46 before). From 137910 the plane holds all 100 (1
before), and so does the tape's 137910 stretch.
