# 1 — the plane's rays strike a ball, the balls' strike the plane (FINAL)

Task S241. 2026-11-02.

## What I did

- Capture (PCSX2 copy 2, research/s241 scripts, states in s2's `context/s241/hit0-*.p2s`, RAM in
  `context/states/s241-hit0-*`): `aim.py PID 1 0 25 20 2 0 8` puts biplane.p2m2's plane 25 ft short of
  ball 0 on free06 through the game's own reset (it reads the placement at +0xe0 before copying the spawn
  over it, so both are written), lets it settle and rolls it at 20 ft/s; `run.py PID` unpauses PCSX2
  through AT-SPI (no pad server to do it); `strike.py hit0 2 0 --frames 120` records
  watch/s241-strike.toml into context/capture/s241/hit0.csv and saves states around the strike;
  `lists.py` prints the node's filed objects and every list. Findings in the oracle's S241 block: the
  plane's rays hit the asleep ball's f.col, its list names the ball, the ball's list takes the mirror
  first, the ball wakes on 11046; the plane's world resolves before the props'; later the balls' own
  rays hit the plane's hull (11061).
- `flight_search::Scene::knocking` / `Knock`: every prop with an f.col and a collision object is in the
  plane's search after the world, a collidable of the props' category, its record after the bikes'.
- `Flight::frame_props` (`frame_met` now goes through the same `frame_struck`, with no props): the search
  with the props in it, the struck props woken and their mirrors filed at the head of their lists
  (`PropSet::strike`), the props' rays on the hull (`prop_wreck::struck_on`, `struck_by_props`'s body
  made general over the struck mesh and its category), the plane's world step with the struck bodies as
  its resolve's others (`Side::Body`), and the pushes written back (`PropSet::pushed`).
- `Sim::step`'s plane lane: `frame_props` (when there are props), then the props' step with the plane as
  striker 0, then the traffic's late step (it stepped the props first, with no strikers).
  openmx `Plane::step_traffic` does the same for `Garage::step_among` and `fly_lane_met`.
- tests/prop_plane.rs (states + disc + MX.PSR): the plane's entries on the ball and the ball's mirrors
  from hit0-11046 / -11047 (one each) and -11048 (none), every searched field; the plane's and ball 0's
  first-substep resolves inside the original's node update on hit0-11046 (both bodies and the arms); the
  balls' hull entries and the plane's mirrors on hit0-11061 (balls 0 and 4); and `Sim::step` from
  hit0-11042 (which holds row 11043) waking ball 0 on the third tick, 11046 as in the original.

## Not verified / not 1:1

- No row-by-row replay of hit0: the Sim test seeds only the plane's body (its tires and surfaces are the
  port's fresh ones) over free06's terrain without the scenery, so it shows the wake tick, not the rows.
- The props' candidates are appended after the plane's gathered list with a plain distance (length of
  the reach centre less the prop's centre, less its radius), not the original's broad-phase gather; the
  hit0 lists match, but the order among several props struck on one ray is not proved.
- The hull pass runs the filed props in collection order, not the node's filing order; in 11061 they
  agree (ball 0 then ball 4). Its mirrors go after the plane's own entries and the bikes' mirrors.
- Ball-against-ball entries (in the original's 11061 lists) are not ported.
- A fast strike that crashes the plane is not captured; the crash test on a prop's category is the
  existing one.
- Split-screen/online seats sharing one prop set (S197c, `among.props`): a seat flying the plane still
  steps its own (empty while shared) set, so it does not strike the shared props: S257. Ball-ball: S258.
- openmx's biplane_particles `a_plane_flown_into_the_ground_puffs_where_it_hits` fails, the same on HEAD
  in a scratch worktree (no props in it): F9.
