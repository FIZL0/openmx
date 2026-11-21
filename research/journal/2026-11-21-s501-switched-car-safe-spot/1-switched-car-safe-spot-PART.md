# 1 — switched car safe spot (PART)

Task S501. 2026-11-21.

## What I did

- `game-sim` `car::Store::record`: the original's safe-spot recorder for a car the human has switched
  into. It reuses the bike's put-back probes (`ride::open_sky`, `ride::clearance`, `ride::flat_unit`,
  made `pub(crate)`, not moved), which the car's run probe for probe.
- `car::Parked::settled_from`: the load's settle, put back once from a store carried over a Restart.
  `settled_after` (S500) calls it with none.
- Shell (`vehicle.rs`): `InPlay::record` writes into the parked copy's store (out of the Sim while
  driven, back in it with the store). `Garage::drive_met` calls it twice after the car up steps
  (the car post, the score manager), from the car's node origin and its third row.
  `InPlay::stores` / `InPlay::settled_from` and `main.rs` `restart_stores` carry each car's store
  across the pause menu's Restart (`Front::stores` → `Args::stores`).
- Glossary: "Safe spot".
- Tests: `game-sim/tests/car_spawn.rs` `a_switched_car_records_its_safe_spot_as_the_original`;
  `openmx` `in_play_tests::a_driven_truck_records_where_a_restart_puts_it`.

## What the original does

- Each frame the car's post (`./kit fn 0x003a04e0`) and the mode-3 score manager each call the
  recorder (`./kit fn 0x003a1a00`) on the switched car.
- No score card (car +0xd24 = 0): it returns at once. A parked or switched-out free-ride car has none.
- No activity and card flag 0x800000 clear (free ride): the probe is the root node's world place
  (`./kit fn 0x001b2870`) five feet down. The passed spot is not used; the passed nose is.
- Overhead test: one ray from 10 ft over the probe straight down to 10 ft under the ground, in a
  query 1000 wide (`./kit fn 0x003a10b0`, the car's copy of the bike's put-back query). A hit above
  the ground: nothing stored.
- Unless state +0x754 == 13: the clearance test (`./kit fn 0x003a1580`), the bike's 14-probe ring.
  Clear, with the ground normal's y over 0.8, it stores:
  - spot = ground point + 0.5 × normal (xyz; w the point's)
  - nose = the nose flattened (y = 0), made unit (rsqrt), then squared: side = nose × up,
    nose = up × side (w becomes up's)
  - up = the normal
- Switch-out: machine_swap (`./kit fn 0x00210b90`) gives the car left the spare-card slot's card
  (world +0xe0 + 4·player) and moves the leaving car's card there. C34's switched image has the
  slot at 0, so a car left in free ride has no card and records no more: its store keeps the last
  safe spot, and a Restart puts it there.
- s342's car: query reach +0xdf0 = 100 (the bike's), card +0x40 = 0 (gather kind 3, props and
  plants), card flags 0x180c06.

## How it was proved

- **Bit-exact against the original's own calls** in its whole frame
  (`car_spawn.rs` `a_switched_car_records_its_safe_spot_as_the_original`). Off
  `context/states/s342-baja-free07.ee.bin`, 8 frames (dt 1/30, pads as saved, `whole_frame` copied
  from `ragdoll_wrecks.rs`). The recorder is hooked; each call reads the root place the original's
  way, runs the call as itself, and runs the port's recorder on free07's world from the same place
  and nose. 16 calls, every one storing; spot, nose and up bit for bit (w lanes of nose and up too).
- **Shell**: `a_driven_truck_records_where_a_restart_puts_it` on a flat floor. The driven truck's
  store moves; it goes back to the Sim with the truck; a Restart from the carried stores rests the
  truck there, store squared once.

## Not verified / not 1:1

- **State +0x754 == 13 branch not ported**: the spot from root +0xb0, clearance with car +0x100 and
  flag 1. No image has that state, and its name is unknown.
- **Card flag 0x800000 and the activity branches not ported**: they use the passed spot instead of
  probing. Free ride's card has the flag clear, and a car with an activity is a machine racer, never
  the human's here.
- **The 1-probe clearance** (activity type 2 with card +0x40 set, radius 5) is not ported, for the
  same reason.
- **Switch-out from code and one image**: machine_swap read, C34's spare slot = 0; no image after a
  live switch out of a car. The port stops recording when the car is no longer up.
- **Query gather**: the port's car uses kind 3 and no checkpoints always (free ride's card). It
  gathers its list at its own first spot; the original's list was gathered earlier (+0xde0). Same
  answer on s342, unchecked elsewhere.
- **`World::keeps`** is applied to the spot, as for the bike (MD14/MD22): the port's own refusal of
  a spot with no ground under it. The original has no such test.
- **Frames run**: 8 coasting frames on one image, all storing. A refused spot (a roof, a steep or
  crowded spot) was not seen in the original's run; the port's refusals are the bike's, proved there
  (S66, S75, S83).
- **Where the inputs come from in the shell**: the port's car node (origin, third row) is taken as
  the original's root node place and +0xa0 row. S342 matched the nodes for drawing, not this call.
  The second call's nose in the original is the score manager's (mgr +0x120); on s342 it gave the
  same store as the first, and the shell passes the node row twice.
- **Online rides do not record**: online has no Restart.
- **Split screen**: each seat drives its own copy. A Restart takes the first seat's store that
  recorded this ride, else the first seat's. The original has one world; this choice is the port's.
- **Not seen live**: no Restart after driving in PCSX2 (no pad here).

## Next

- Nothing for S501. A live check (drive a car off, Restart, see where it stands) needs a pad.
