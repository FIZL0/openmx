# 1 — plane rays strike the wreck hull (FINAL)

Task F35. 2026-11-03.

## What I did

- `flight_search::Scene::wrecking(wrecks, first)`: the downed wreck bikes' hulls in the plane's search.
  Each one is a collidable after the world's and the knocks' (S241). Its category is
  `wreck_bike::COLLIDABLE` and its collides-with `wreck_bike::COLLIDES_WITH`, and it is enabled. It is a
  nearby candidate after the gathered ones, as a knock is. A hit names record `first + j`, which takes the
  mirror, and is kept as struck as `WRECKED + j` (1 << 20). `category()` gives `WRECKED` ids the wreck's
  category, which has `VEHICLE`, so the plane's crash test spares it. The narrow phase is the knocks'
  (`pair_knock` and the new `pair_wreck` share `pair_placed`).
- `wreck_plane::Hull` (mesh, the node's frame at the copy nearest the plane, body velocity, record
  capacity) and `Met::hulls`, which runs parallel to `Met::wrecks`. `search_downed_by` fills them. The
  mesh is the rider's ragdoll kit's `bike_mesh`, which is also where the props' worlds take it
  (`Sim::wreck_hull`). If the rider has no kit, it is the plane's own `Flight::wreck`
  (`Flight::with_wreck_hull`, a new field). The shell (`main.rs`'s `flight_for`) gives the plane
  `wreckbikef.col`, and snapshot.rs keeps the field local, like `bike` and `hull`.
- `Flight::search_wrecked` (`search` is now a wrapper around it). The hulls get records after the knocks'.
  The plane's entries on hull `i` are renamed `WRECK_FROM + j`, and the hulls' records' mirrors are
  handed back. In `frame_struck`, the `WRECK_FROM` rename now takes kind 0 as well as kind 1, so the
  plane's resolve impulses the wreck body. The mirrors go at the head of the in-world wreck bike's list
  (`InWorld` with `place == j`): they name the plane (the first plane after its struck props), with
  their point at the copy nearest the bike, then the list is cut to capacity, and the touching flags are
  set. So the order is the original's: the plane searches first, then the wreck bike files its own
  entries after the mirror, and its mirrors go on the plane's list after the plane's entry.
- `tests/c16_fly.rs`: `fly_by` and `fly_two_by` give the plane the disc's `wreckbikef.col`
  (`wreck_hull`, parsed as the shell's kit parses it). JOUSTS moves from (53, 17), (18, 55) to
  (90, 17), (55, 55): the plane now holds to the end from 137810 and from 137845.

## What the original does

Read off images s272-137859..137866 (research/r31). The wreck part's collision object holds a mesh
collidable that is on the plane's node and in the plane's nearby list. On tick 137863 the plane's ray 33
end goes inside it: the plane files a kind-0 entry, and the mirror goes at the head of the wreck bike's
list, before the wreck bike's own search.

## How it was proved

- The save-state images: `r31_the_joust_against_images_to_137866` (run with `--ignored`). After every
  tick from 137855 to 137865, the plane body and the wreck bike body match images 137856..137866 bit for
  bit on every field. Both lists match the images entry for entry (kind, on, normal, point, other):
  4/4 and 2/2 from 137863 on. Before F35 both bodies parted on 137863, with lists of 4/3 and 2/1.
- The capture: `the_plane_jousts_p2_as_the_original` against C25's rows. The plane holds 90 of 90
  ticks from 137810 (was 53) and 55 of 55 from 137845 (was 18). P2's counts don't change.
- Every other c16_fly test still passes, including the whole tape's STRETCHES and
  `planes_flown_together_meet_p2s_wreck_bike`, with the hull in the Sim lane's plane too.
  `./kit check -- -p game-sim` result is in the commit message.

## Not verified / not 1:1

- Planes flown together (`flight::frames_together`) still have no wreck hulls in their search. Only a
  lone plane's frame (`frame_met` / `frame_props`, which covers the Sim's plane lane, split screen and
  online lone planes) has them. Filed as F36.
- The hull's nearby-list candidate is appended after the gathered ones, at its distance now, as a
  knock's is. The original gathers it with the broad phase. Its place among the candidates and its
  gather-time distance were not checked. They matter only when two collidables tie on one ray, or for
  the sweep's nearest hit.
- No capture checks the plane's sweep probe against the hull: no swept hit on a wreck happens in C16's
  joust.
- The shell's riders with a ragdoll kit take the kit's mesh, which is the same file, `wreckbikef.col`.
  It was not run against a capture in the shell. The test riders have no kit and take the plane's.
- A plane's mirrors on a wreck whose `InWorld` is in another plane's met are dropped. That can only
  happen with several planes (F36).

## Next

Nothing for F35. F36 does the planes flown together.
