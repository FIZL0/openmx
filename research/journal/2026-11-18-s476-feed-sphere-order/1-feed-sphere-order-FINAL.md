# 1 — feed sphere order (FINAL)

Task S476. 2026-11-18.

## What I did

- Found the order the original hands the rider's nine bone collisions to the dust feed. It is not the
  rider file's order.
- `game_sim::dust::balls_of` now builds the balls in that order (`FEED_BONES`, bones
  `[17, 14, 0, 2, 3, 10, 6, 16, 13]`). Before, it used the file order `[14, 17, 0, 2, 3, 6, 10, 13, 16]`.
- The masks read the same sets either way (foot dust 0x3 = the two feet; Rider Dust/Chunks 0x1fc =
  the other seven). But the order inside a set changes which sphere emits first, so the first emit's
  "just on" zero draws, and the emitter's final points, landed on a different sphere.
- Checked held (off) feet: the original feeds them anyway, with their flags clear. The port already
  clears every contact's flags each tick, on or off, so an off foot gives the foot dust nothing in both.
- New test `s476_feed_spheres_in_slot_order` in `tests/ai/wreck_dust.rs`.
- Did not build the Sim's own wreck beside the original's draws. Filed as S483.

## What the original does

- The rider body actor keeps nine named collision slots. `./kit dis 0x0024a088 0x0024a1a8` shows the
  vehicle builder filling them by each collision's bone. A bone with no collision leaves its slot 0.
- Every frame the rider body (`./kit dis 0x0024e2e0 0x0024e380`) copies the nine slots into the feed's
  list, in slot order. Whether the collision is on is not looked at.
- The actor's own collision list is the file's order, the same as the port's `Records::collisions`.
- Full slot table in research/oracle.txt, S476.
- `./kit fn 0x0027c2d0` zeroes all nine pointers on one branch (a network/snapshot path). Not followed.

## How it was proved

- Save-state capture `capture/s138/s325-nose76-2604.ee.bin`, all four riders:
  - each feed's nine spheres, read as bones through the actor's parts, are `FEED_BONES`;
  - each actor's list, read the same way, is the disc's `mrider.phy` collision order;
  - the port's `balls_of` on a ragdoll with that file order gives `FEED_BONES`.
- Then 45 of the original's whole frames from that image (through the wreck at 2615): every frame the
  same nine pointers are written; feet were off 283 rider-frames, never with ground or scene set; the
  wrecking rider's feet came on at frame 10 and touched at 11–12 and 30–42.
- `./kit check -- -p game-sim --test ai wreck_dust` and `… race4` pass.

## Not verified / not 1:1

- The Sim's own wreck beside the original's per-tick collision-entry draws: not run. It needs the
  ride and rider body seeded off the image (replay.rs-only helpers today) and the ragdoll drifts
  0.1–0.6 ft (R24), so the draws would not match bit for bit. Filed as S483.
- A held foot that was touching when it was turned off: not seen in this capture (no foot touched and
  was then held). The port clears the flags every tick so it would hand over nothing; the original was
  only seen handing over clear flags.
- The zeroing branch in `0x0027c2d0`: not followed; the port never empties the list.
- An off foot's stale point and zone are handed over in both, but only the flags were compared.

## Next

- S483: the end-to-end Sim wreck run.
