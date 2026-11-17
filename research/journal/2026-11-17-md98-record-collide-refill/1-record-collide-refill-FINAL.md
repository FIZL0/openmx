# 1 — record collide refill (FINAL)

Task MD98. 2026-11-17.

## What I did

- Found the brief's main ask already landed:
  - MD105 (11-09): `handling::record`'s clear and pass run the ATV's record bit-exact through the palm
    strike, wreck and put-back.
  - MD105b (11-09): `atv_contact::Held::refill` takes `record::pass`'s contacts each frame, and the
    pass's world search, sweep and node points are the port's (Desert Oasis).
  - MD123a: `quad::Ride::record_on` runs clear, pass and the refill before the steps every frame, so
    `quad::Bike::contacts` is no longer only laid from an image.
- Ran the brief's tests (`./kit check -- -p game-sim --test atv_quad`): 121 pass. The images they read
  (`context/mxvatv-ps2/md37a/atv-r-spawn.ee.bin`, `mc02.res`) are on disk, so the palm tests ran, not skipped.
- The piece left is contacts with another record (other quads). Filed it, no code changed:
  - C38: a PCSX2 image of two quads touching (an ATV race start, steered into a neighbour).
  - MD98a (after C38): `Held` keeps the other record and body and `resolve` answers it, proved on C38's image.
- ../opentennis: nothing to take (no vehicle contact there).

## What the original does

The record's pass (vt `+0x7c`) files a kind 0 contact for each ray hit; when the struck object has a record
(entry `+0x84`), it also files a kind 1 contact, normal turned round, on that other record naming this one.
The resolution (vt `+0x88`) then pushes both bodies. No MX Unleashed world the port rides has another
record, so `record_on` always refills; `Held::refill` refuses (returns `false`) a list naming one.

## How it was proved

Already proved by MD105b: `tests/atv_quad/sim_frame.rs`'s `the_palm_strikes_contacts_push_the_ports_quad`,
300 frames off `atv-r-spawn.ee.bin` (a PCSX2 save-state RAM image), every word of the port's step equal to
the original's through the palm strike, wreck and put-back. Re-run green here.

## Not verified / not 1:1

- Contacts with another record (quad vs. quad) are not ported or proved: no captured image has two quads
  touching. `Held::refill` still refuses such a list and the gravity's call is handed on. Filed as C38 → MD98a.
- Contacts with a mover (entry `+0x88`) are not ported: no world the port rides has one, and no capture shows
  one. Left inside MD98a's scope only if C38's image has one; otherwise unfiled, since nothing can reach it.

## Next

C38, then MD98a.
