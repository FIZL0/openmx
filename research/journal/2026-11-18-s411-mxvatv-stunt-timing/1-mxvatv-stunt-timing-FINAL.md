# 1 — mxvatv stunt timing (FINAL)

Task S411. 2026-11-18.

## Notes as I go

- State 4's own `.p2s` (s9's, md5 73389cd1…) is gone with ps2-decomp-kit-slots. Only its RAM
  (`state04.ee.bin`) is left. The nearest state is s1-slots copy 1's `SLUS-21104 (95BB1901).01.p2s.backup`:
  same session (bike 0x00e714e0, vsync 13441, 15 ft along from state 4), but paused on the quit dialog.
  Copied to `context/mxvatv-ps2/s411/` as `start.p2s` (md5 b181ce77…). The tapes take NO and resume first.
- The card's nodes come from the manager's pool of 100 at 0x012ad6c0 (0x28 each), the same in both images.
- c15's flip-1 (state 4) already shows the backflip's clip (`+0x1754`) and the bank clock (`+0x14f4`):
  the clip starts at row 94 with node +0x84 (the up row's height) first <= 0 and +0x94 > 0, goes 2 at row
  110 with +0x94 < 0. The flip lands at 270° (row 125), a wreck. The bank clock is 0.2 from row 95 (in
  the air, stunt 41) and runs down on the ground from row 125.

## Captures

`research/s411/card.toml` records every frame: the bike (0x00e714e0, 2 × 3072 bytes), its node
(`+0xe4 ->`), the card (`+0x680 ->`), the manager and the node pool. `research/s411/summary.py` prints
one line per row. Tapes are made from `research/s411/*.pad` by c15's `tape.py`, played by
`research/s411/play.sh` in copy 2 (KIT_CONFIG=context/c32-atv-kit.toml). Copy 2's ini NominalScalar
went 0.25 → 0.12 for the captures, because the recorder missed rows under load (load average about 220); put back to 0.25 after.

- `flip2` (flip.pad): a backflip off the first lip, comes round, lands, banks 11332.
- `spin3` (spin3.pad): a right 360 (armed tick 130, fired 138, stick back from 148), banks 16998.
- `flip`, `spin`, `spin2`: earlier tries — a backflip with gaps at its transitions, a 360 cut by a lip
  touch, and a 360 that landed mid-clip and wrecked. Not used by the test.

## What the original does (flip2, spin3)

- **Backflip clip, one frame late.** The clip changes at row k on the node's heights of row k−1:
  - start at 13753 (row 146) from row 145's up −0.1137 / forward 0.9868; node 41 opens with 2333
    (2000 + one frame at 10000/s);
  - 1 → 2 at row 157 from row 156's forward −0.0104;
  - 2 → 0 at 13809 (row 172) from row 171's up 0.604 / forward −0.7887 (over −0.79); node finished
    (0xd, 11332) in the air.
  The rule is the port's `watch_flip`. Only the phase differed: the port read the end-of-tick pose.
- **Backflip only under its torque.** The clip moves on only while the flip's torque is on (bike
  `+0x1194`). In spin3 the bike pitches past level in the 360 with the flip words 0, and no backflip
  starts. The port's watch started one there.
- **Bank clock.** Set to 0.2 when the stunt word at the start of the frame is a stunt (so 0 on the
  start row, 0.2 the row after). It counts down by dt on every frame on the ground, even one that set
  it. The word stays 41/45 past the clip until the move's words drop — on the second frame down
  (13813; first frame down 13811). So it counts .1667 .1667 .1333 … 0 and banks at 13825, the sixth
  frame after the word goes. spin3 banks 16998 at 13825 the same way. The port pinned it on "running
  now" and counted only when nothing ran: one frame early.
- **360 held start.** spin3's clip goes 0 → 2 in one frame at 13743 (row 114). Node 45 opens held
  that frame with 2499 (2000 + one frame at 15000/s). The clip goes back to 0 at 13803 and the node
  finishes (0xd, 16998) in the air. The port already starts it on the clip's frame.

## Port changes (score.rs, rider.rs)

- `AtvRows` gains `heights` (start-of-frame up/forward heights, the rider's `riding_from` transform),
  `move_on` (the flip's or 360's move still on), `flip_torque` and `front_torque`.
- `Atv` gains `held`: the stunt word the next frame starts with. Walked in its hash and wire.
- The bank clock: pinned on `held`, counted on every frame down.
- The backflip watch steps only under the backflip's torque (or on the ground / a wreck, to forget).
- The port's own front flip watch steps only under the front flip's torque. Without that it started a
  front flip node in the back half of both the backflip and the 360 (the original scores one node).

## Test

`tests/mxvatv_stunt_score.rs`: `the_backflip_calls_come_when_the_originals_do` and
`the_three_sixty_starts_held_on_its_clip`. Each checks the original's own timing on the capture, then
runs the port's card on the rows (the 360 node off its clip, `atv_frame`, `atv_grow`) and compares
nodes, combo flags, total, combo points and bank clock bit for bit on every row. Missed frames are run
on the next row's words, and the test asserts nothing changed inside them. A row read late (the same
words as the next row: flip2 row 161) holds the next frame. Both skip without the captures.

## Gate

`./kit check --all`: everything green except two.
- `ride.rs` `md25_mx_vs_atv_rides_as_before`: the pack-ride hash fold moved, as it should (`Atv::held`
  is hashed and the bank and watches changed). Re-pinned.
- `replay.rs` `the_dallas_rail_rides_replay_from_their_save_states`: it fails reading the shared
  `context/capture/s89/across.csv` (row 33: frame_counter 11734 after 11732). Another session rewrote
  the file at 10:17 today. It fails before any simulation runs, so it is not this task's.

## Not verified / not 1:1

- The start state is not state 4: its `.p2s` is gone. It is the same session 15 ft along.
- The pack ride's physics are MX Unleashed's, so when the move ends (and the bank starts) on a port
  ride is the port's own; the test feeds the original's move words.
- The bank clock's pose exception (the stunt word at the riding pose) is not modelled: never seen in
  these rides.
- The backflip that ends with its clip running (the mid-clip wreck), and the 360 that lands mid-clip
  (spin2), are not compared: spin2 has too many missed rows at the landing. Filed as S472.
- The node row's heights are taken as the frame's end; it fits every transition in both rides.
- The front flip is the port's own; its torque gate follows the backflip's, with no capture.
- Wheelies and stoppies are out of scope.
