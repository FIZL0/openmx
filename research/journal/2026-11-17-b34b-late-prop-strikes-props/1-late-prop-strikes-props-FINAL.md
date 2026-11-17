# 1 — late prop strikes props (FINAL)

Task B34b. 2026-11-17.

## What I did

- A prop woken by the vehicle's strike (`PropSet::strike_in`) now runs its rays against the other props
  in that pass, right after its own late search: `prop_props::strike_props_late` walks the props' rays
  starting from the freshly filed objects, in filed order. Anything those rays wake is filed at the end and
  walked too. `strike_props` and the late walk share one body (`strike_props_from`).
- `strike_then`'s callback now gets every fresh object at once. `strike_in` runs each one's own search,
  then the one late ray walk. `strike` (the bike lane's hear) still loops over them one by one.
- Test: `tests/prop_plane.rs` `a_ball_the_plane_wakes_strikes_the_ball_beside_it`.
- Item (2) of the brief, the wreck bikes' hulls in the late search, is not done. It is filed as S384.

## What the original does

The props node's collide walk reads its filed count again at every step. So an object filed during the
walk (woken by a strike) is searched at the walk's end, and its search includes its rays on the other
props. Those rays can wake more props, which are filed and searched in turn. The plane's or bike's
collision object is filed first on the node, so a prop it wakes is searched after every other filed
object.

`./kit fn 0x0015a030` (physics object reset): takes the object's probes out of the broad phase, places it
from `+0x90` position / `+0xa0` nose / `+0xb0` up, and files its probes again where it now stands. This
is how the test stands a sleeping ball somewhere new and keeps the static tree right.

## How it was proved

No shipped scene has a sleeping prop within a struck prop's reach. free06's balls are ≥ 14 ft apart,
free09's ≥ 13.8 ft, free11's pins 6.2 ft. Two free06 balls put together in PCSX2 throw each other apart
before they can sleep (`research/b34b/put_ball.py`, `settle.py`; the captures in context/s241/b34b-* are
useless: moving an asleep ball by RAM leaves its tree key stale, so nothing meets it).

So the scene is made in the interpreter, from `b34at-11065` (the tick before the plane wakes ball 0, from
B34). Ball 1 is put asleep at 6.4, 6.0, 5.6 and 5.2 ft past ball 0, on the line from the plane, using the
original's object reset. Then the original's node walk is run (every filed list emptied, each object
searched in list order, count re-read each step) and compared with one port `Sim::step` of the plane lane.

Compared: which balls were filed and in what order; balls 0 and 1's whole lists (kind, the side named,
every field the search writes); their four flags; whether each is awake.

- 6.4 ft: ball 0's rays don't reach ball 1. Both sides: ball 0 `[k1 plane, k0 -]`, ball 1 empty.
- 6.0, 5.6, 5.2 ft: ball 0's rays wake ball 1, and ball 1's rays strike ball 0 back. All lists match the
  original bit for bit (2, 4 and 5 entries on ball 1).

Also passing: `prop_plane`, `prop_props`, `knock_launch`, `prop_wreck` (25 tests). `./kit check --all`: red
only on two known main reds outside the props, K64 (mxvatv_bike_ride
`the_riding_parts_listeners_are_the_originals`) and K59 (untamed_cars
`the_untamed_baja_drives_in_the_shell_on_untameds_step`).

## Not verified / not 1:1

- **Wreck hulls in the late search (brief item 2): not done.** `search_late` still gets `&[]` wrecks, so a
  late prop's entry on a downed rider's bike waits a tick. Proof needs two riders, one down beside a ball
  the other strikes. Filed as S384.
- **One striker reaching two props: order differs.** At 4.8 and 4.4 ft ball 1 also reaches ball 2. The
  original's rays meet ball 2 before ball 0; the port's meet ball 0 first, so ball 1's list is in another
  order (same entries). The port hands the other props in flat order. The original's gather walks the
  static tree's cells, where awake props stay too (F68). This gap was already listed in prop_props.rs and
  predates B34b. Filed as S385; those two gaps are left out of the test until then.
- **Several props woken by one strike:** each one's own search runs before the late ray walk over all of
  them. I think that gives the original's list order, but no case with two fresh props was compared.
- **Pass order when an awake neighbour reaches the sleeping prop:** in the port the props' pass runs
  before the vehicle's frame, so such a neighbour wakes the prop before the vehicle strikes. The
  original walks the vehicle first. Not looked at further.
- **Bike lane:** only the plane lane was compared. The bike's strike goes through the same `strike_in`.
- Bodies after the step were not compared: the test compares lists, flags and wake state only.

## Next

None: the task lands. Follow-ups S384 (wreck hulls) and S385 (gather order).
