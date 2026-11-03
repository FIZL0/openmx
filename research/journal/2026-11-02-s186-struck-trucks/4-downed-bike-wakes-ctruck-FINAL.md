# 4 — a downed bike's wreck wakes ctruck, and the mid-tick stand-down (FINAL)

Task S252a. 2026-11-03.

## What I did

- **The captures.** `research/s252a/drop.py` takes s89-along (copy 3, slot 40), runs it until
  ctruck (0x01cc7630) is a set lead short of a path clock, and then teleports the bike onto the
  truck's line. The teleport is research/r14_windmill_bones.py's: everything near the rider moves by
  one delta. Then it drops the bike hard enough to wreck it. `research/s252a/strike.py` records
  `watch/s252a-downed.toml` from that state, one row a tick, and saves states around the wake.
  `research/s252a/ram.py` unpacks them. There are three runs:
  - `bike`: the wreck bike lies still 11 ft ahead of the truck. The truck wakes on 12031.
  - `roll`: the wreck rolls down into the truck at about 10 ft/s. It wakes on 12000.
  - `doll`: the ragdoll is in the way and the wreck is 29 ft off. It wakes on 11962, struck by a
    bone.
  The commands are in the test's comment, and the addresses are in oracle.txt S252a.
- **The wreck body's address.** The first watch file read the wreck bike at 0x0195c6a0. Its
  +0x60/+0x70 are about a foot from the body's, and its +0x170 is heap text. Pinned to those rows,
  the port's wreck was upside down and its rays started underground, so it never struck. The real
  body is the rider's +0x7a8 -> +0x110 = 0x0196f020, the same address in every state made from
  s89-along. I fixed the watch file and strike.py and recorded bike and roll again. The game is
  deterministic from the saved state, so they woke on the same frames as before. doll.csv is still
  the old recording, and its wreck columns are wrong.
- **The test.** `tests/replay.rs`
  `a_downed_bikes_wreck_wakes_the_truck_driving_into_it_as_the_original_does` builds a `Sim` whose
  rider is down with reason 5 and no ragdoll. Its wreck bike is the state's body, among free06's
  cars and trucks from the state (`s252_traffic`). It runs each of bike and roll three ways:
  - **Pinned:** the wreck body is put on the capture's rows. The company truck wakes on the
    original's tick (4 after the state) and stops at the original's point within 1e-3. Its path
    point matches the capture on every tick before.
  - **Free:** the port's own wreck. It also wakes the truck on tick 4, at the same point.
  - **Lifted:** the wreck pinned 100 ft up. The truck drives on.
  So S252's wiring (the wreck bike's search marks the `Prop`, then `Traffic::heard`) holds to both
  captures. No port code changed.
- **The stand-down** (from the wake images, oracle.txt S252a):
  - On the wake tick, the wreck object's last-struck slot (+0x1fc) names the truck's path mesh,
    but its contact list keeps no entry for the mesh. The listener woke the truck and stood the mesh
    down in the middle of the wreck's search, before that contact was kept.
  - Later in the same tick, the woken truck's own object (empty the tick before) searched. Its list
    has its ground entries plus kind-0 entries against the wreck object (3 in bike, 4 in roll). The
    wreck's list has the matching kind-1 entries.
  - So a second search that tick meets the truck's new body and not the mesh. No entry names the
    f.col itself. The m.col/f.col pair is newly filed in three collidable lists.

## How it was checked

- `cargo test --release -p game-sim --test replay a_downed_bikes_wreck` passes, with both runs
  printing `Some((4, true, ..))` pinned and free and `None` lifted. S252's plane test still passes.
- `./kit audit` reports only two leaks, both already in HEAD's `plan/DONE.md` (lines 519 and 533).

## Not verified / not 1:1

- **The ragdoll's strike has no port test.** Only the doll run's capture and states exist, and its
  wreck columns read the wrong body. Filed as S272.
- **The port doesn't check the woken truck's own search on the wake tick.** That search meets the
  wreck bike in the original. The test holds the wake tick and point only, not the truck body's
  contacts or push afterwards; that is S251's ground.
- **Untested:** whether the port drops the contact on the struck mesh within the striking search,
  as the original does.
- **Free runs' wreck bodies:** the free runs wake on the same tick, but the wreck body's own path is
  not compared to the capture.
