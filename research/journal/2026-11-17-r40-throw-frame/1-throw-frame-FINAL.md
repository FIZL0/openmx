# 1 — throw frame (FINAL)

Task R40. 2026-11-17.

## What I did

- Found R24's leftover y residual in R14's south-bones replay. It was the frame the throw's pull is
  turned by, as the brief guessed.
- How it showed:
  - A probe printed the bones' momentum gap to the csv, per tick (mass-weighted, all 18 bones).
  - During the throw (ticks 3–9) the gap grew by about 0.94 a tick. It was a vector at right angles
    to the pull, about a tenth of its size, and it turned with the pull.
  - That is the pull turned about 6° too far: a quarter of a tick of the bike's spin (~24° a tick).
- The cause:
  - The port turned the pull by the wreck bike body's quaternion as the step left it.
  - The original turns it by the request's quaternion, which is `mat4_to_quat` of the bike node's
    world matrix.
  - The wreck bike's node is what its post listener wrote: the pose before the frame's last substep
    (`wreck_bike::node_of`, which reads `last_quat`). Four substeps a tick, so a quarter tick behind.
- The fix: `crates/game-sim/src/ragdoll_ride.rs` hands the wreck arm `request_quat(node)`, the
  quaternion of the wreck bike's node rows.
- With it, the momentum gap is flat from tick 3 to the end of the throw (to 0.01). The y gap went
  from −5.41 to −0.96. All of that −0.96 is already there on the riding ticks before the wreck
  (tick 2: −1.19).
- The test `the_windmill_throws_the_bones_with_the_originals_draws` now has:
  - tighter bounds: the worst bone < 0.15 ft to tick 11 and < 0.6 ft to tick 23; mean dv x < 0.2,
    y < 0.35, z < 0.3;
  - a new assert that the mean y gap moves < 0.05 ft/s between tick 3 and tick 9. Before the fix it
    moved ~0.6.
- Nothing came from opentennis: it has no ragdoll throw.

## What the original does

- The throw clock block is `./kit dis 0x0024cf98 0x0024d118`. While the force time `+0x6bc` > 0:
  - `+0x6a8` set: the pull `+0x6e0` = the force `+0x6d0` as it is;
  - else `+0x6ac` set: turned by the main bone's quaternion (bone body `+0x170`);
  - else: turned by `[actor+0x488]+0x190`. That is the request's quaternion.
- The request fill (S111, oracle.txt) writes `+0x190` as `mat4_to_quat` of node `*(rider+0x18)`'s
  world matrix. While wrecked, that node is the wreck bike's (S65: the request's frame is a copy of
  the wreck bike's).

## How it was proved

- A save-state capture: `context/capture/r14/south-bones.csv` and its before image (R24's
  re-recording), replayed by `tests/replay.rs the_windmill_throws_the_bones_with_the_originals_draws`.
- The bones' momentum gap during the throw, before → after the fix:
  - t3 −1.30 → −0.96 (y);
  - t9 −5.41 → −0.96 (y).
  - After the fix it does not move at all over ticks 3–23.
- The worst bone off the csv (ft):

  | tick | before | after |
  |---|---|---|
  | 11 | 0.30 | 0.11 |
  | 23 | 0.71 | 0.54 |

- The mean dv y is −0.12 to −0.14 on every wreck tick. It was down to −0.74.

## Not verified / not 1:1

- **The request quaternion's bits are not compared.** The capture has no `+0x190` or `+0x6e0`
  column. The proof is the bones' momentum gap going flat through the throw, not a bit check of the
  pull.
- **The node → quaternion trip uses the port's `mat4_to_quat`.** I took it to be the same routine
  the request fill uses, since S111 proved the riding request's `+0x190` with it. I did not check
  it on a wreck row.
- **The rest of the wreck frame still mixes two poses.** `Down::enter_from` and
  `Down::step_posed_among` build `frame` from the body's current quaternion and the node's origin
  (pose before the last substep). The request's rows are the node's. Switching both to the node's
  rows changed nothing on this capture: reason 4 holds no hands, and the off-bike test and bike
  search did not fire differently. So it is unproven, and I left it alone (S424).
- **The ~0.13 ft/s y gap that is left is from before the wreck**, on the riding ticks (t1 −0.33,
  t2 −1.19 momentum). The likely cause is the skeleton held at the image's pose (R24's note). Not
  chased here.
- **The growth after tick 19** (worst 0.22 → 0.54 ft by t23) was not looked into.
- **Other reasons' throws** (2, 3, 8, 11, 12 also turn by the bike) have no capture here. They
  take the same change; it is checked only by the existing tests staying green.

## Next

- S424: the wreck frame, rows and origin both from the node.
