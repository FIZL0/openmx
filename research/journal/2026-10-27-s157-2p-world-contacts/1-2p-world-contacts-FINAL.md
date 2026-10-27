# 1 — 2p world contacts (FINAL)

Task S157. 2026-10-27.

## What I did

- `crates/game-sim/tests/two_p_world.rs` (new): every world contact C10 indexed in the 2p recording,
  replayed from C10's RAM images (`context/states/c10/c10-<frame>.ram`) through the interpreter, the
  original against the port, both riders. Six tests, all on the disc and the images; none skips here
  (checked by raising two counts once: both failed).
- The port's wreck test now wrecks a Race500P where the original does (`rider.rs`, `track.rs`):
  - `track::fifty_reach(name)`: the original's constructor flag, "Race50" or "Pro50" in the physics
    file's name — which the 500s' names hold too, so a 500 takes the 50cc reach (4) in the hit clause.
  - `Rider::fifty` (default false, not sent or hashed: a constant like `bike_col`, put back from this
    machine on a snapshot load) is handed to `air::hit_wreck`, which before took `false` always. The
    shell (`main.rs`) sets it for every local rider from its bike's `.phy` name.
  - The body rays' own scenery entries (`rider_contact::scenery`'s, via `mover_contact::wreck_contacts`)
    go on the wreck test's list, after the ground's, the head-ray sweep's and the cars'. Before, the
    port's list held only the head ray, swept at the frame's end, which is all a 125/250 can wreck
    on; a shoulder ray (end y 4.58) on a trunk wrecked nothing.
- `hash.rs`'s Sim size pin: 11536 → 11544 for the new field.
- `research/oracle.txt`: S157 block (the node in the 2p images, the riders' order, +0x588 on Race500P).

## What the original does

C10's index lists four world contacts; all are a riding bike's own body rays (kind 0, other 0):

| image → tick | rider | what | ray end y | wreck after the tick |
|---|---|---|---|---|
| 130412 → 130413 | P2 | trunk, normal (-0.196, 0, -0.981) | 1.8583021 (nose) | none |
| 130416 → 130417 | P2 | the same trunk | 4.580028 (ray 0/1, shoulder) | 6 |
| 130895 → 130896 | P2 | another trunk, normal (-0.555, 0, 0.832) | 4.580028 | 6 |
| 131482 → 131483 | P1 | the ground search, bike on its side | 1.8583021 | none |

C10 called P2's three "walls"; they are tree trunks: the port's props alone find nothing there, its
ecosystem's trunks find the original's entries to the bit. Nothing else in the ride touches the world
(C10's index): no crate, sign, beach ball, wall or other prop.

The level's collision node in the 2p state is `0x0061e630` (objects: two others, then the riders'
records in either order; a wrecked rider's record is replaced by its wreck bike's). At 60 Hz each riding
record resolves once a tick, first substep only (`./kit fn 0x00270dc0` is the resolve). Both bikes are
`Bike\Race500P.phy`, whose name the constructor's strstr matches against "Race50": bike `+0x588` = 1 on
both (read in c10-130416), so the hit clause's reach is 4 (oracle.txt S37) and the shoulder rays at
4.58 wreck on the scenery.

## How it was proved

Save-state images, C10's: 7 images (130412, 130416, 130417, 130418, 130895, 130896, 131482, plus the
next images 130419, 130897, 131483 as targets), `crates/game-sim/tests/two_p_world.rs`:

- `the_original_ticks_each_image_into_the_next`: the node's tick from 6 images lands on the next one —
  both riders' lists and wreck reasons, and the hit rider's chassis (centre, force, moment, shock, twist,
  vel, rates) bit for bit; both riders' chassis on 130416, 130895 and 131482.
- `the_original_meets_the_world_where_c10_indexed_it`: each of the 4 contacts on its tick, on the named
  rider, one entry, kind 0, on the scenery, its ray's reach; the other rider untouched; the wreck reasons.
  Also both riders' `+0x588` = 1.
- `the_port_finds_the_world_contacts_the_original_does`: the port's `rider_contact::ground` + `scenery`
  over its own free09_ss (terrain, 275 props, the `.esb`'s trunks) from the image's record node: every
  field of every entry bit for bit (from, end, normal, point, arms, surface, other, mover), 4 of 4.
- `the_port_resolves_the_world_contacts_as_the_original`: the port's `Bump::resolve` (Race500P's
  collider) against the original's call: velocity, rates, shock, twist, force, torque and arms bit for
  bit, 4 of 4.
- `the_tick_with_the_ports_resolve_wrecks_as_the_original`: the node's tick with the port's resolve in
  the original's place equals the original's tick, both riders, lists, wreck reasons; the hit rider
  lands on the next image where there is one.
- `the_ports_wreck_test_wrecks_where_the_original_does`: `air::hit_wreck` on the port's own contacts
  with the Race500P reach gives Scenery (6) on the two shoulder hits and none on the nose and ground
  hits; with the 125's reach it gives none on all four.
- `./kit check --all` green but for the size pin (fixed, rerun alone green) and the two lobby load
  flakes, which passed on the gate's own rerun. S138's walls/nose verdicts and roof_landing unchanged.

## Not verified / not 1:1

- **No crates, signs, beach ball or walls**: the task named them, but the 2p ride never touches one
  (C10's index: four contacts, all trunks or the ground). Nothing here proves the port against them.
- **The port's rider tick was not run from the images.** What is proved is the port's pieces — search,
  resolve, hit clause — each against the original's call on the same RAM. The composed `Rider::frame`
  (its ordering of the wreck list: ground, head-ray sweep, cars, then the body rays' scenery entries)
  was not ridden from a 2p image; the order matters only when an actor entry precedes a scenery one,
  which no 2p contact has. Seeding a whole `Rider` from a 2p image is not something the tests can do yet.
- **The list order of the port's wreck test is not the record's.** The original reads its record's
  one list in order; the port's list puts the head-ray sweep (taken at the frame's end) before the body
  rays' scenery entries. A head ray on scenery is a wreck either way; a lighter actor listed before a
  scenery hit could end the search differently. Not met in any capture.
- **The head-ray sweep and the body rays can both report the head ray** on the same scenery. Harmless
  for the verdict (scenery wrecks either way), but the port's list is not the original's count.
- **`fifty` is set where the shell builds its riders** (`main.rs`, `new_rider` for every seat, from
  `bike_words`, the `.phy` member each seat rides). Online, a snapshot load keeps each machine's own
  value as it does `bike_col`; a lobby or online seat whose bike is swapped after the build (e.g.
  `bike_words[0]` rewritten on a lobby drop-in) keeps the value of the bike it was built on. Not traced
  through a networked ride.
- **P1 in the node-only tick**: from 130417, 130418 and 130896 P1's centre/vel/rates/force/moment differ
  from the next image by a few ulps: the node alone runs on P1's held controls of the last frame, the
  frame's input step that moves them is not run. P2 (down) and both lists match.
- **The images and the capture are one row apart on the wreck**: the image after tick 130417 already
  holds P2's reason 6, the capture's row is 130418. The capture is a separate playback read while it
  runs; not chased further.
- `UTILITYSHACK1.col` (a hierarchy file) is not read by the port, as before; no 2p contact is near it.

## Next

Landed. Follow-up S174: a whole-rider
replay from a 2p image (seed both `Rider`s off RAM and step `Sim`) to prove the composed wreck order.
