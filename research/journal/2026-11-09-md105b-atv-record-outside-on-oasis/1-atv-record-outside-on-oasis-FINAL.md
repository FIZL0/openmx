# 1 — atv record outside on oasis (FINAL)

Task MD105b. 2026-11-09. Slot s3.

## What I did

- **`atv_contact::Held::refill`** (`crates/game-sim/src/atv_contact.rs`): this is MD98's refill. The
  contacts that `handling::record::pass` files in a frame become the held record's entries, exactly as
  the pass leaves them in the RAM list: kind, ray, normal, point and surface word, with both arms
  `record::BLANK`. Each entry keeps the relative velocity its list slot already had, because the
  resolution writes that value before it reads it. If any contact names another record or a mover, the
  refill answers `false` and leaves the list empty, because the held record cannot resolve those.
- **`tests/atv_quad/sim_frame.rs`, new mode `Pieces::Port`**. On top of MD105's port record
  (clear/pass), it adds two things:
  - **The world step is the port's.** `WorldStep::step` reads the ride off the machine with
    `image::read_ride`, gives the frame's `Held` to `quad::Steps`, and runs `quad::frame`:
    - The port's terrain answers the ground.
    - The sweeps and the gather go to `atv_search::Oasis` over Desert Oasis, kept from one frame to the next.
    - The only calls handed on are `PartMove`, plus `GravityRecord` when nothing is held.
    - Then the original's step runs on the machine. Every word `image::compare` reads must be equal, and
      so must every word of the rigid body and every word the resolution writes (`record::held_words`).
  - **The record's outside is answered by the port's Desert Oasis**:
    - **List check** (`WorldStep::list`): when a frame starts, the record's nearby list must equal the
      port's list (`Oasis::list`), entry for entry, with the same distances and centre. The port's own
      step keeps that list through `quad::frame`'s collision sync, so the check passes every frame.
      The check also maps each port collidable to the image's.
    - **Part search** (`WorldStep::search`): `atv_search::search` with `atv_part::PartRays`, over the
      port's list, on the port world's objects.
      - The riding part is the world's own probe; the wreck part is added on first use.
      - The new `OasisParts` supplies the shapes: the props' meshes, and each plant's parts stood at
        the plant (a trunk mesh, or nothing for a sphere).
    - **Sweep** (`WorldStep::sweep`): `atv_sweep::set` gives the segment and its bounds. The search then
      runs with `OasisSweep`, which is `atv_sweep::Sweep`'s clear plus MX Unleashed's
      `sweep_probe::pair` on the prop's mesh.
    - **Node points** (`WorldStep::node_point`): the port's scene, `atv_moto::Scene::tip(0, v)`, on the
      scene `read_ride` read before the pass.
    - After each port answer the original's call still runs, so RAM gets its side effects (see below),
      and its answer must equal the port's word for word: the hits bit for bit, the sweep's centre,
      reach and hit bit, and the node point. The record's pass uses the port's answer.
- New tests:
  - `the_palm_strikes_contacts_push_the_ports_quad`: the 300-frame palm ride in `Pieces::Port`.
  - `a_spoiled_held_contact_turns_the_ports_step_red`: a mutation check. On frame 89 it flips one bit
    in each lane of the first held contact's normal, in the port's `Held` only. The port's step must
    then differ from the original's on that frame.
- In `oasis.rs`, `collidable` and `PROBE_AT` became `pub(super)`. Nothing else in that file changed.

Nothing was taken from opentennis: it has no vehicle physics. The glue copies this repo's F46c/F46d
helpers from `tests/mxvatv_bike_ride.rs` (`tree_in_ram`, `part_record_in_ram`).

## What the original does

On the palm ride, here is what the record's outside is asked each frame (counted with a temporary
tally over the 300 frames):

- **One sweep a frame**, on the record's sweep probe (`+0x204`) over the nearby list. Its pair does
  run: the palm and the other prop mesh come inside the probe's reach 239 times. It never hits.
- **One part search a frame**, with the active part: the riding part, or the wreck part from frame 96
  to the put-back and again after frame 256.
  - The list holds the two prop meshes (category `0x600`) and up to six plant groups (species 0, 4, 7
    and 10).
  - The riding part searches the groups' parts.
  - In all, 18 hits on the palm. The rest of the contacts come from the record's heightfield rays.
- **A node point per heightfield ray** after the wreck: 1,126 over the ride. Each one carries a point
  through the quad's scene node, as the scene's `tip` does.
- **The damping setters are never called** on this ride. The quad's record is a body record (`+0xe4`
  is set), so the node rotate, node position, node up, chassis velocity and node move calls never run
  either.
- The record's nearby list is the quad's list (quad `+0xc4`), filled by the PostProcess's collision
  sync. The port's `quad::frame` gathers the same list, entry for entry.

## How it was proved

Ran `./kit check -- -p game-sim --test atv_quad -- sim_frame`: all 5 tests pass. A temporary failing
assert confirmed that the port's outside answered 300 part searches, 18 hits, 300 sweeps, 239 sweep
pairs and 1,126 node points, and that the port's step resolved held contacts on 73 frames.

- On each of the 300 frames:
  - The port's world step, holding the port pass's contacts, equals the original's step on every word
    `image::compare` reads, every body word and every resolution word.
  - All of RAM outside the stack equals the original's whole frame (MD105's comparison and exclusions).
  - Each port answer from Desert Oasis equals the original's answer bit for bit.
- The mutation test shows that the port's step really reads the held contacts. A one-bit change in one
  held normal makes it differ from the original's step on frame 89.

The oracle is the SLUS-21104 interpreter, as in MD105.

## Not verified / not 1:1

- **The original's outside calls still run in `Pieces::Port`, for their RAM side effects.** The port
  answers the pass, but RAM changes still come from the original's calls:
  - the part place (the probe stood at the node, and its database entry moved);
  - the searches' searched/hit flags on each collidable, their globals and the probe's record;
  - the plant parts stood at each plant;
  - the sweep's set and place, with its database move;
  - the scene node's world frame update.
  The port's answer is checked against the original's, but these writes are not Rust's. Filed as a
  follow-up.
- **The part place is the original's.** The port's search reads the part's frame (`+0x130`), its reach,
  flags and filter, its ray tree and segments, and its record's list shape from RAM, as the original's
  place left them.
- **The sweep's pair is MX Unleashed's, not the ATV's.** `atv_sweep` does not port the ATV's pair.
  `OasisSweep` uses `sweep_probe::pair`, and on this ride its 239 pairs all miss, as the original's do.
  Only the miss (the hit bit) is checked: no hit's normal, point or struck object has been compared.
  Filed as a follow-up.
- **The plants' spheres write no contact, and the port does not test them.** `OasisParts` returns
  `Shape::Never` for a sphere part. The original's sphere test can raise the probe's hit bit. The pass
  reads only the hits, so the answer is the same, but the port's hit bit is not compared. Trunks
  (MC02's species 7 and 8) have no height, so nothing strikes them, and no trunk hit is proved.
- **The port's state is re-read from RAM every frame.** The ride, the body and the record are read off
  the machine before each step, and the scene before each pass. The port's world (`Oasis`: its
  collidables' flags, its list) persists from frame to frame. Each probe's collidable is re-read from
  RAM at each search. After each frame's equality check, the machine keeps the interpreter's step RAM,
  not the port's.
- **The quad's control-off word (`+0xc80`) is a mode in RAM.** The wreck writes 8. The port keeps only
  whether it is set (`control_off`). `image::compare` skips that word when it holds more than 1, and the
  test asserts the bool instead.
- **The held entries' first relative velocity** comes from the list slot before the pass, as the
  original's list keeps it. The resolution writes it before reading it, so no answer depends on it.
- **The damping setters, node move/rotate/position/up and chassis velocity are never called on this
  ride.** In `Pieces::Port` they still go to the original, and they are not proved.
- **Contacts with other records or movers** make the refill decline: the held list stays empty and the
  original resolves them. The palm ride has none.
- **The tail is still the interpreter's** (MD105a).
- **The interpreter is the oracle, not PCSX2**, as in MD105. **Only this ride**: the atv_ quad on
  Desert Oasis's palm.

## Next

MD105a, then MD105's all-Rust chain. The two follow-ups filed here (MD118, MD119): the outside's RAM writes in Rust,
so the original's calls stop running in `Pieces::Port`; and the ATV's own sweep pair, checked on a hit.
