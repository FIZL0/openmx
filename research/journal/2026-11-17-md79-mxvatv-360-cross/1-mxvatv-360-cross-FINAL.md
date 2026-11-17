# 1 — mxvatv 360 cross (FINAL)

Task MD79. 2026-11-17.

## What I did

- Went to read the 360's fade-in/out from the stunt record (`record+0x50/+0x54 × record[1]`), as MD78 asked.
- Found the record is not a fade. It is the 360's score. R17 had already shown this for the same call: the
  stunt start pushes a node on the bike's score card, base = max(PointValue × scale, 10),
  rate = max(HoldPointValue × scale, 10). MD78 read the PC version of that call as an animation layer.
- Read the 360 rows from state04's RAM image (`context/mxvatv-ps2/state04.ee.bin`): ids 44/45, each
  `+0x50` = 2000, `+0x54` = 15000, scale 1.0. Taken as milliseconds that would be a 2 s cross in and a 15 s
  cross out over a 1.47 s clip, which cannot be right.
- So there is no fade to read here. I kept MD78's 0.25 s. I only fixed the comment in
  `crates/game-sim/src/vehicle_anim.rs` (the 360's played-once arm) so it no longer suggests the record holds the cross.
- Filed R43 (low): find where SLUS-21104 really starts the 360 clip, and with what blend.
- Nothing taken from ../opentennis: it has no stunt animation.

## What the original does

- The 360 step (`./kit fn 0x004e53d0 -o atv`) calls the bike's vt +0x4b8 with the 360 id
  (`+0x13bc` left / `+0x13c0` right) once the spin passes 35° (0.6108652 rad). It sets the clip state with
  vt +0x540, which only stores its argument at `+0x1758` (`0x004e7a40`, `jr ra; sw a1, 0x1758(a0)`).
- vt +0x4b8 is `./kit fn 0x004e6370 -o atv`. It looks up the stunt table row (`./kit fn 0x00268c90 -o atv`,
  12-byte entries at manager +0x18, id < 0x2e). It reads `+0x50`/`+0x54` × the entry's float `+4`
  (`lwc1`, so a float, not an int), floors each at 10 with `max.s`, and hands both to the score-node push
  (`./kit fn 0x0046aae0 -o atv`, node words +0xc base, +0x10 rate). No animation call is made.
- The stunt id also goes to `+0x1528`. Two copiers (`0x004ba690`, `0x004d9f30`) write it into a byte at
  `+0x904` when it is < 0x29 or a 360/backflip id. I did not find who plays the clip from it.

## How it was proved

- Save-state RAM image: the bike at 0x00e714e0 (vtable 0x003d0450, as in `tests/mxvatv_stunt_score.rs`),
  manager `+0x14f0` = 0x912090. Rows 41/44/45 were read with a short script: 2000/10000, 2000/15000, 2000/15000.
- What the call does with them is R17's function oracle
  (`crates/game-sim/tests/mxvatv_stunt_score.rs::the_backflip_reads_its_points_off_the_stunt_table`,
  all 46 rows, base and rate bit for bit).
- No product behaviour changed (comment only), so no new test.

## Not verified / not 1:1

- **The 360's cross in is still the chosen 0.25 s.** The record the task named holds score numbers, not a
  fade, so there was nothing to take. The original's real cross is unread. Filed as R43.
- **The 360's cross out is unchanged.** Same reason. MD78's note about it still stands.
- **Who plays the 360 clip in the original was not found.** The 360 step's own calls are a score push and a
  word store. The stunt id goes to `+0x1528` and the `+0x904` byte. I did not trace that byte's readers
  (it seems to be copied as part of a block). Watching it live needs the rider's animation RAM mapped, and it is not.
- **Only state04's table was read.** Other machines load the same ini rows. Only the numbers could differ,
  and they are score numbers either way.

## Next

Nothing for MD79. R43 finds the real blend.
