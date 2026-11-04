# 1 — wreck bike in the plane world (FINAL)

Task F32. 2026-11-03.

## What I did

- **`wreck_plane::InWorld`** (new): a downed wreck bike as a body of a flown plane's dynamics world.
  It has listeners like its own world's (`wreck_bike::Lists`):
  - preforce: clears the shock peak;
  - force: the part's gravity, then its list resolved against the props it struck, then the planes'
    bodies, each the copy nearest the bike (`Flight::flown`, written back with `Flight::pushed`);
  - post: sets its transform from its body.
- **`flight::Met::downed`** (new field): `search_downed_by` files a copy of each searched wreck bike there,
  in the met of the first plane it was filed with. Before the copy is filed, its entries on the hulls
  (`PLANES + j`) are renamed to their places on its resolve, after the props it struck.
- **`Flight::frame_struck`**, the lone plane's frame (`frame_met`, `frame_props`):
  - the `Listeners` count the downed bikes as bodies `1..` of every list;
  - each body is `Met::wrecks`' body, the one the plane's resolve already pushed.
  - So each substep goes: the plane's force, then each wreck bike's force, then the plane's integrate,
    then each wreck bike's.
- **`frames_together_props`**: the same for `Together`, with the bikes after the fleet. A bike's planes
  are mapped from the search's order to fleet places. Callers build the search's planes from the fleet's
  planes that have a met, so that is the mapping. After the step, each bike goes back to the met it came
  from.
- **`step_downed_by`**: takes the bike back whole when the frame stepped it. The bike's body comes from
  `Met::wrecks` as before. If no frame ran (`InWorld::stepped` unset), it steps the bike after the frame
  as before (`WreckBike::step_flown`). The callers in the Sim lane, the shell and online are unchanged.
- **`tests/c16_fly.rs`**:
  - new test `f32_the_plane_and_wreck_bike_are_the_images`;
  - `JOUSTS` and `STRETCHES` re-pinned;
  - the joust's printout now names the tick where the plane parts.
- `research/f30/index.md`: an "F32: done" section.
- opentennis has no flown planes or wreck bodies, so nothing was taken from it.

## What the original does

research/f30/index.md, "The tick": the level's collision node lists the plane, another object, the wreck
bike and one more, all in one dynamics world. In each substep the resolves run plane, other, wreck, and
then all bodies integrate. `f30_the_originals_tick_137856` runs that update on image 137856 in the
oracle and gets image 137857 back.

## How it was proved

A save-state capture. `f32_the_plane_and_wreck_bike_are_the_images` runs `fly_two` from 137845 and
compares four ticks with the images, all bit for bit:

- the port's plane body after tick `t` (velocity and position) against image `t + 1`, for
  137854..137857;
- the wreck bike's body once P2 is down, against `*(P2 + 0x7a8) + 0x110`.

Before F32 the plane parted on tick 137856: the port had (-28.609848, -0.08526538, 3.6421182) and the
image has (-28.616682, -0.05351289, 3.6458206).

Pins against C25's capture (`held`):

| Run | Before | Now |
|---|---|---|
| Joust from 137810 | 46 | 53 |
| Joust from 137845 | 11 | 18, parts on tick 137863 |
| Joust from 137910 | 1 | 100 of 100 |
| Tape stretch 137910..138010 | 1 | 100 of 100 |

Every other event and stretch is unchanged. `row_137911_is_the_wreck_bikes_first_substep` and
`planes_flown_together_meet_p2s_wreck_bike` still pass. The second checks that `frames_together` with a far
plane matches the lone frame, so the `Together` path is exercised.

## Not verified / not 1:1

- **The joust from 137845 still parts on tick 137863.** The images stop at 137858, so I did not find
  what differs there. Filed as a follow-up.
- **Two planes over one wreck bike** are not checked against the original, because no capture has one.
  The order (every plane's resolve, then the bike's) follows the single-plane case and the node's list
  order.
- **The fallback for a plane whose frame never ran** (the shell's `tables` missing) keeps the old order,
  step after. It is not 1:1, and play never takes it.
- **The props a wreck bike struck** are pushed as the bike's own copies, as before F32, not as the plane
  frame's bodies. A prop struck by both the plane and the bike would get two copies. No capture has one.

## Next

none
