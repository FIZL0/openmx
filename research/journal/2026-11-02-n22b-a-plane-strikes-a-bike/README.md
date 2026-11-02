# N22b — a plane strikes a bike

2026-11-02. Task: `./kit plan N22b`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

In split and online, a player's plane struck the other seats' bikes (S225), but its resolve met them
as static boxes. In the original, a riding bike's frame collidable belongs to its collision record.
So the plane's ray hit names that record: the plane's entry (kind 0) has the bike as its other, and
its resolve impulses the bike's chassis when the contact closes fast. The bike's list takes the
mirror (kind 1, naming the plane) ahead of its own entries, because the node searches the planes
first. That is ported now (`Flight::search` with `Met::bikes`/`named`, `flight_search::Scene::owned`,
`rider_contact::met_plane`/`struck_first`/`pushed_riders`, wired in `main.rs` and `net/rides.rs`). It
is proved bit for bit on s93's 5074 image: the plane's entries, P2's whole list, every resolve call
with both riders' chassis, and the tick on the capture's row. Details in
`1-a-plane-strikes-a-bike-FINAL.md`.
