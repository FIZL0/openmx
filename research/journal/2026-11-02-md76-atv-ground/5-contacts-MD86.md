# MD86 — the quad's contacts swept in the port

2026-11-03. Task: `./kit plan MD86`.

`quad::Steps` now answers `Call::Contacts` itself when it is given a sweep (`Steps::sweep`, the
`quad::Sweep` trait). Desert Oasis's sweep is `atv_search::Oasis`. It takes the node's frame, gathers
the quad's nearby list (`atv_world::gather`), runs the search over MC02's world and its plants
(`atv_search::collide` with `Plants`), and reads the record out (`atv_search::contacts`). Then
`quad::wheel_contacts` lays the result on the four wheels. With no sweep (`()`), `Call::Contacts` goes to
`rest` as before, so every older test is unchanged apart from writing `sweep: ()`.

`tests/atv_quad/oasis.rs` runs the original's Force body off MD37a's `atv-r-spawn.ee.bin`, with the
distributor (`atv_quad_wheel_contacts`) and the search unhooked. The quad, the body and the wheels
(`snap_all`), the hit bit, the record's count, the rear node (`+0x143c`) and the listed calls all equal the
port's on 763 frames:

- 211 frames are MD37a's Desert Oasis rows (`race-*`);
- 72 are blended toward rows that carry a swept contact;
- 480 are aimed so that each wheel's ray crosses each of the 120 meshes.

Every wheel is swept and the rear contact is kept, and 17 frames have two contacts or more. A second test
runs every ported Force step with the contacts at once: same frames, all equal.

## What vt +0xc8 is

oracle.txt called the search's vt `+0xc8` the "record clear". It is not. It runs `atv_node_frame` on the
probe's node (probe `+0x174`), which updates the node's world first and copies it from `+0x70`. Then it
calls vt `+0xcc` to stand the probe at that frame. It is now named `atv_search_node_stand`. The port does
the same thing: it updates the scene (`Scene::update(0)`), sweeps at `nodes[0].world`, and keeps the
updated scene, as the original's write does.

## The gather is not the distributor's

The distributor does not fill the nearby list. The PostProcess's collision sync gathers it round the
active probe (quad `+0xd8`), after the parts move. That is why the trigger probe's frame in an image lags
its node: the snapshot falls between the sync and the next distributor. Until MD75c2 answers the Gather
call, `Oasis` gathers in the sweep, with its gather probe at the node's position. The test does the same
on the original's side (gather probe centre at the node, then the original's own gather), so the list
both sides search is the one the sync would leave.

## Small things

- **A plant's sphere can set the hit bit (`+0x126` bit 5) with no contact.** The port does not test plant
  spheres, so it reports no hit. 35 frames do this. Only the distributor reads the bit, and only to skip
  the lay-out, which does nothing with 0 contacts anyway. The test allows exactly that case and counts it.
- **`rear_node` (`atv_moto::Parts::rear_node`, quad `+0x143c`).** The port keeps it as an index into
  its own world's collidables, while the original keeps the address. The test maps one to the other by
  the mesh's centre bits and radius.
- **Owner and vehicle are 0** on every Desert Oasis contact, so no listener is notified.
  `Steps::contacts` debug-asserts this.
- **Placing a captured orientation.** `Scene::place` leaves the node's world rows stale. Without
  refreshing the bike's `right_y`/`up_y` from them, the backflip's clip (quad `+0x1754`) read different
  rows in the full Force run.

## Context files

This slot lacked `MC02.esb` and `capture/md37a`. I copied them in from s3 and s2 (`context/`,
git-ignored).

## Checked

- `./kit check -- -p game-sim --test atv_quad --test atv_search --lib`: 96 tests passed.
- `--test atv_gather --test atv_ground --test atv_wheel --test mxvatv_ride --test mxvatv_step --test
  mxvatv_air`: 35 tests passed.
