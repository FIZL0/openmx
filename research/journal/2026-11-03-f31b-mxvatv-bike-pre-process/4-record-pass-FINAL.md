# F31b4: the bike's collision record pass

2026-11-04. Task: `./kit plan F31b4` (F31b split, 4 of 5).

F31b4 covered three things: the collision record's pass, the frame's tail (the listeners' step, the
bike's vehicle update, the wreck test and entry, the put-back, the unwreck) and MD63e's mass properties.
That is more than one session. This session ported the first piece, the record pass. The rest is filed
as F31b4a (the tail and the vehicle update), F31b4b (the wreck, the put-back and the unwreck, with
MD63f's wreck gravity) and F31b4c (MD63e's mass properties).

The record pass is now `game_sim::handling::record`. Addresses and the call map are in
research/oracle.txt under `# F31b4`.

## What it does

The record is the object the sim keeps on its `+0x24` list (the bike's `+0xc8`). Before the world
steps, the sim runs every record's first virtual, then every record's second.

- `record::clear` (vt `+0x7c`): empties the contact list and the four touch flags. With a body and the
  damp switch, it hands the body back its rest damping through the body's two setters.
- `record::sweep_back` (vt `+0x80`): the segment runs from where the record's reference point was `lead`
  seconds of velocity ago to where it is now. The body's quaternion and position give that point, or, with
  no body, the node's turn and position, using the chassis' velocity. The sphere probe sweeps the segment.
  On a hit, the body and the node are put out along the hit's normal, then the active part is placed again.
  The exception is when what was struck has a record or is a kind-2 mover and the two fly apart: both
  faster than 1, and the cosine of their directions past `facing`.
- `record::pass` (vt `+0x84`) runs the sweep back first. Then, with an active part and the rays on, it
  runs two kinds of ray:
  - Heightfield rays: each of the part's rays that points down against the body's up. The node places
    the ray's end; inside the terrain's box, `ground::under` answers. If the end is under the ground,
    the record files a contact. The terrain service's answer is the ground buffer's point and normal,
    depth 0, and as surface the corner with the largest weight (`ground::surface`).
  - World rays: the part's search. Every hit raises "touched" and "on an object", even when its category
    is masked. Each unmasked hit files a contact (no surface, the struck record, the mover if its kind is
    2) and becomes the struck object. A struck record also gets a contact back, with the normal negated
    and `w` kept. If the struck record is the record itself, that contact goes on its own list.
- MX Unleashed's record ([`contact_search`]) is the same code at other offsets. This port keeps its own
  types (`Record`, `Contact`, `Hit`, `Struck`, `Swept`) so that its `read`/`words` match SLUS-21104's
  layout. The outside world is one trait, `Collides`: the node's calls, the probes and their searches,
  the terrain box and query, the struck object's velocity, and the other records.

## Checked

- `the_records_pass_is_the_originals`: 12 000 records. Each starts from state 4's bike record. Every word
  the two virtuals read is drawn at random:
  - the switches and the touch flags (0/1), the rest damping, the offset, lead, push and facing (0 now and
    then, signed), the old segment, body or chassis, the body's quaternion, velocity (slow a third of the
    time) and position;
  - the node's up and translation, the terrain box about the node or no terrain;
  - the list's count and capacity (−1, 0, 1, 2, 3, 14, 16), near-field or global, the mask, the part and
    probe or none;
  - the part's rays (up to 6), the probe's hit bit and its first normal, point and object (or none), the
    part's hits (up to 4);
  - the objects: no record, the other record, or the record itself; no mover, a kind-2 mover or another;
    a category;
  - the other record's list, capacity and body or chassis, and `dt`.

  Each record runs through SLUS-21104's 0x00204c30 then 0x002051f0, and through `clear` then `pass` via
  `RecordInRam`, whose calls are the original's callees. All of these come out word for word the same:
  the record (0x210 bytes), its list, the other record and its list, and the body. The calls outside the
  record are logged by hooks and match in order and arguments:
  - the body's two damping setters;
  - the node's turn, position, set and placement. The turn and placement answer a fixed map of the
    argument; the position answers the node's own `+0xa0` in `xyz` only;
  - the probe set, the placements, both searches (they answer nothing; the hits are laid out
    beforehand), the mover's velocity virtual (its `+0x10`), and the terrain query (a height near the
    point, with the normal, surfaces and weights hashed off it, ties among them).

  The contact filing and the terrain service run as the original's own code. So the service's depth 0,
  point, normal and largest-weight surface are checked against the original on random buffers.

  Path counts:

  | Path | Records |
  |---|---|
  | damped | 3921 |
  | sweep, body | 5304 |
  | sweep, chassis | 2603 |
  | pushed | 3786 |
  | pushed past a mover | 377 |
  | flying apart | 1469 |
  | no rays | 3237 |
  | heightfield queried | 1066 |
  | on the ground | 641 |
  | outside the box | 1596 |
  | on an object | 5245 |
  | list full | 1643 |
  | struck | 4865 |
  | handed itself a contact | 752 |
  | handed another a contact | 1543 |
  | filed a mover | 1574 |
  | touched, masked | 1297 |
- `ride_replays_bit_exact_on_the_ports_record_pass`,
  `flip_wrecks_and_is_put_back_bit_exact_on_the_ports_record_pass`: the frame is F31b3's hybrid frame
  (the port's head, Force body and PostProcess). The bike's record runs `clear` and `pass` every frame
  (asserted). Their callees run in the interpreter, and the port's terrain query is `ground::under` over
  Nat05's map. All 459 and 465 rows match word for word. The flip wrecks on row 125 and is put back on
  row 193. The flip's record makes 470 heightfield queries, all through `ground::under`; the ride makes 0.
- Mutations:
  - Raising the touch flags only for unmasked hits fails the oracle (record 53).
  - Handing back the normal un-negated fails the oracle (record 11).
  - Flipping the facing compare fails the oracle (record 3).
  - Filing no heightfield contacts fails the flip (row 126).
  - Keeping the list through the clear fails the flip (row 127).
  - Skipping the push, the world rays, or the struck word passes both replays: only the oracle covers
    them.
- `./kit check -- -p game-sim --test mxvatv_bike_ride --test mxvatv_step --test mxvatv_bike_ground`: 37
  passed. The oracle printed its path counts in `cargo test`, so it did not skip.
- `./kit audit`: the same 4 leaks as on main (atv_wheel.rs, plan/DONE.md). None are in these files.

## Not verified / not 1:1

- Neither ride makes the sweep push or the world rays file. Free ride on Nat05 has nothing near the
  bike, so those branches are covered by the oracle only, against hooked searches. The oracle's hit
  records, objects, movers and other record are laid out in scratch RAM, not by the game's own probes.
- The heightfield buffer's normal and surface word are uninitialized stack in the original. The port
  hands the query zeros. They are the same only if the query writes both, which it does for any point it
  can read a cell for. In the box, every tip is such a point; out of the map, `ground::under` leaves
  them, and the two would differ.
- The struck word (`+0x20c`) lies outside the captures' 256 record bytes, and nothing in the frame reads
  it. Only the oracle checks it.
- In the replays, the record's callees (the node's calls, the probes' set/place/search, the body's
  setters) are the original's. Their Rust ports belong to F31b5's pure-Rust frame. Only the bike's own
  record is ported; any other record on the list still runs in the interpreter.
- The port's `Collides` hands the body to the sweep separately. Its new position is not in RAM when the
  node's set and the part's placement run. In the original it is, but neither reads the body: the flip
  stays bit-exact, and the oracle's hooks read none of it.
- The tail, the wreck and put-back, and MD63e's mass properties are not done (F31b4a–c). Only the
  targeted test files were run, not the full replay suites.
