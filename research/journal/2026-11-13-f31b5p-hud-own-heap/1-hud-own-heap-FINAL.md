# 1 — hud own heap (FINAL)

Task F31b5p. 2026-11-13.

## What I did

- **The HUD on a heap of its own, in the handing run.** The original's heap manager keeps eight heaps.
  An allocation takes the current one (the manager's `+0x18` id), and a free goes to the one its block's
  header names. So the test makes a second heap with the original's own maker (`hud_heap`,
  tests/mxvatv_bike_ride.rs), over RAM nothing touches: `0x01f80000..0x01fb0000`, between the boot heap's
  end and the calls' stack. It hooks the HUD's receive (the frame post's listener) in the handing run (`a`)
  to run with that heap current. The heap is made before run `b` copies `a`'s RAM, so both runs have it.
  During the receive, a free of a block from another heap is left undone: the HUD's own blocks from before
  the image stay in the frame heap, as they would with no HUD at all.
- **No frame-post tell at all.** `replay_whole` now runs `"frame post tells dropped"` (all eleven) plus
  `"hud on its own heap"`, with `replay_pure`'s read watch on. `three_served` asserts that no tell is left
  told.
- **RAM parting, classified.** `ride/flip_replays_with_every_port_at_once` drop `HUD_WORDS` and the HUD's
  heap range (`but_the_huds`, which asserts that each is among the parts and the heap is used). The rest
  must be exactly the race progress's words, plus the ride's re-reported race word and the flip's cue word.
- Took nothing from opentennis (it has no interpreter replays of this kind).

## What the original does

research/oracle.txt `# F31b5p`: the manager's layout, the alloc and free routing, the heap maker
(`./kit fn 0x1b5d90 -p atv`), and the slot init (`./kit fn 0x1b6030 -p atv`). The maker takes its lock object
from the C library's `new` (hooked to hand out the reserved range's first bytes) and a time stamp off RCNT0
(hooked, stamped 0). On C15 the HUD's receive allocates on row 59 only: 124 blocks, 120 freed. It also frees
two frame-heap blocks (string objects of class vt `0x3bb380`) that it held.

## How it was proved

- Replays (C15), tests/mxvatv_bike_ride.rs `ride/flip_replays_with_every_port_at_once`: every row is C15's
  word for word (459/465), and the bike and its blocks are the handing run's after every frame. The read
  watch is on: the port run's frame reads no byte where the RAM parts from the handing run's. Wrecked on
  125 and put back on 193. The census holds no frame-post tell.

## Not verified / not 1:1

- The handing run itself is no longer the original's frame on the HUD's heap traffic. Its HUD allocates
  from a second heap the game never makes, and two frees of the frame heap's blocks during the HUD's receive
  are left undone. Rows are still checked against C15, so the frame's outcome is proven. The heap layout
  under the HUD is not.
- The HUD is not ported to presentation. Its tell is dropped from the pure frame, and the HUD heap
  arrangement exists only in the test harness. The live game does nothing of this.
- The heap's lock object and time stamp are stand-ins (the lock in the reserved range, the stamp 0). They
  are the HUD heap's alone and nothing in the frame reads them (read watch).
- Two of `HUD_WORDS` (`0x15d7674..0x15d767c`, `0x169f064`) are called the HUD's for three reasons: they
  part on the row its receive allocates on, they hold the two blocks it frees, and the frame never reads
  them. No write watch named the writer. `0x42e200` is the construction tally of class `0x3bb380`: only
  constructors read it, to bump it.
- The every-port proof covers C15's two replays only. A HUD that frees a frame-heap block another part of
  the frame later reallocates would only show on other captures (the read watch would catch it there).

## Next

None for this line. Porting the HUD to presentation is its own work, if anyone wants its gauges drawn from the port.
