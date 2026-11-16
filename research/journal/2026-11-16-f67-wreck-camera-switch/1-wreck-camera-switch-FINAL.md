# 1 — wreck camera switch (FINAL)

Task F67. 2026-11-16.

## What I did

- New `handling::camera`: the camera controller's kind-8 receive (ids 0x15, 7, 6, 0xd, 0xc, 0xb), its
  camera switch, both camera classes' leave/enter, the second class's spread, the connector's sorted list
  (unlist, add, connect, drop, own) and the interfaces' offers, connects, disconnects and takes.
- `attach::insert_with`: `insert` with the overlap pass and the kept temp as flags. `insert` calls it with
  the old behaviour, so nothing else changes.
- Test glue in `mxvatv_bike_ride.rs`: `CameraInRam` serves each virtual call by its port. Heap pushes go
  through the frame's heap hooks with the original file and line. The re-insert goes through `attach`.
- Two new switches: "wreck camera" (the receive on the port) and "wreck listeners dropped" (the other five
  listeners skipped). The second is under the read watch.
- `flip_replays_in_pure_rust` is no longer ignored.

## What the original does

- The chain and every address are in research/oracle.txt `# F67`. Start with `./kit fn 0x491270` (the
  receive), then `./kit fn 0x490b30` (the switch).
- A wreck post leaves the current camera (its list entry unlisted, its interfaces disconnected) and enters
  the new one. Entering re-inserts the camera's attachment in the spatial database and adds it to the
  sorted list. The second camera class also pushes its target and spot arrays.

## How it was proved

- A bit-exact replay fixture: `flip_replays_in_pure_rust` (flip-1, all 465 rows, wrecked on row 125 and
  put back on row 193). RAM is compared each row, and the read watch checks that no byte the frame reads
  parts. The interpreter now runs 0 instructions on every frame (8,630 before, 7,146 of them the camera).

## Not verified / not 1:1

- Camera class A with a part at +0xbc: not ported, the glue hands it to the original. flip-1 never takes it.
- Camera class B entered with a flag and an offset equal to (0, 0, 0, 1): not ported, same reason.
- The rider interface's connect past its two early outs: not ported, same reason.
- The switch's +0x34 join (`./kit fn 0x4f75f0`): left to the original, same reason.
- Any virtual the glue does not know falls back to the original under the census label `camera <addr>`.
  None ran on flip-1, but other wrecks may reach them.
- The five other listeners (hud, race progress, speed flag, menu list, picker) are dropped, not ported.
  The read watch proves the frame never reads what they write, on flip-1 only. The menu list writes
  0x851ff4 on id 0xb, and that word parts between runs without any read.
- Only flip-1 is proved. No function oracle over generated inputs.

## Next

- Port the uncovered arms above when a fixture reaches them (the census names them).
