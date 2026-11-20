# 3 — the Kiowa strikes props, ported (FINAL)

Task S471. 2026-11-20.

## What I did

- Ported the Kiowa striking knockable props, the way the car's `Machine::step_props_in` does it.
- game-sim `kiowa.rs` (only new code; the old paths are unchanged):
  - `Collision::search_among`: the heli's search with the blades first, then the props after the world
    (`flight::knocks`, `Scene::knocking_gathered`). Prop entries are named `KNOCK_FROM + i`, as the plane's.
    With no props it is `search_knocking`.
  - `Collision::strike_props`: a copy of the car's `Machine::knocked`. The struck props are woken and
    searched in the pass (`PropSet::strike_in`), their rays meet its hull (kiowaf.col,
    `prop_wreck::struck_on`), and the mirrors are filed on its record after the blades' entries.
  - `KiowaProps`, `Blades::others`, `Blades::with_others_among`: the resolve meets the blades, then the
    struck props' bodies.
- Shell `vehicle.rs`:
  - New `KiowaRide::step_traffic`: `frame_held`, then the heli's frame among the props, then the props'
    step with the heli as their one striker, then `frame_late`. `drive_met` calls it when the Kiowa is up.
  - `step_through` is now `step_among(.., None)`. `KiowaAround` carries the struck props.
  - No props, not flown, or on MX vs. ATV's record: the old path (traffic frame, then `step_through`).
- Test `kiowa_capture.rs`: `a_tail_strike_on_a_prop_replays_bit_exact` replays C43 from C14's image
  among free09's 62 props, in the same call order as the shell.

## What the capture showed

- Walk order: the heli's search with the props after the world, then the blades, then the woken
  prop's search and its rays on the hull, then the heli's world step, then the props' step. The capture
  agrees bit for bit with this order.
- C43's rows are read a tick late from row 1: its 1205 holds C14's 1206 (two ticks after the image).
  A row's tick is its index plus one, and the tick after `t` ticks reads row `t - 1`'s pad.
- Row 1613 is a stale read: it equals 1612 in every column but the stamps and `pad_b`. Its tick ran.
  - Taking it as a phase slip (no tick) parts on 1614 (collective 0.4675 against 0.4629).
  - Taking it as stale replays to the end.
- The egg's inertia comes out right from the disc. My first reader matched `egg.psg` by name end and
  picked another model's box (inverse inertia 1.6e-3 against 8.4e-3). Reading by whole name fixed it.

## How it was checked

- `a_tail_strike_on_a_prop_replays_bit_exact`: 1205..=1663 bit-exact (one stale row, 1613, not
  compared). On every row:
  - the heli: `compare` and its whole body;
  - the egg's whole body;
  - both records, entry by entry (kind, from, on body, normal, point, both levers, surface, the side
    named).
- Records hold entries on 53 rows. The heli strikes the egg on 1579..=1592, the egg wakes on 1579.
- On 1579 the heli's record is [k0 on the egg, 16 k1 mirrors]. The egg's is [the mirror, 2 on the
  ground, 16 k0 on the heli], as C43's note says.
- `cargo test --release -p game-sim --test kiowa_capture`: 12 passed, 0 failed. Six of them skip:
  C17's and C24's captures are gone from every slot since 11-05.
- `cargo build --release -p openmx` builds. The openmx tests that fly the Kiowa are listed in the commit.

## Not verified / not 1:1

- The shell's `KiowaRide::step_traffic` has no replay of its own. The test drives the same game-sim
  calls in the same order.
- The test's resolve meets the props only. The shell's goes through `Blades::with_others_among`
  (blades, then props). No capture has a blade out and a prop struck in the same frame.
- Thrown blades do not strike props, and props do not search the blades.
- The props' strikers in the heli's frame are the heli alone. Riders' bikes are not added, as with the car.
- MX vs. ATV's record path (`step_on_record`) does not strike props.
- One strike, from behind (the tail), one prop (the egg). No nose strike, no rotor strike, no sign.
- The stale row 1613 is inferred from the replay, not from the recorder's timing.
- Without kiowaf.col (no blades built in the shell) the props' rays do not meet the hull.

## Next

- Nothing filed. A capture with blades out among props would test `with_others_among`'s order.
