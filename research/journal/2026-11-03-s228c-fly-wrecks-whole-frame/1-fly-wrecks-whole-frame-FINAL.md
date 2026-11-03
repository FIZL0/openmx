# 1 — fly_and_collide's wrecks through the whole frame (FINAL)

Task S228c. 2026-11-03.

## The stall

S228 found that running the whole frame (crates/game-sim/tests/ragdoll_wrecks.rs `whole_frame`) from
context/states/s228/s228-<frame>.ram ran out of budget: call 0x133500 (the scene's simulate walk), pc
0x198bf0. That pc is in 0x198b90, named `iop_rpc_send` in research/oracle.txt. It is the sound driver's
double-buffered RPC to the IOP. iop_command_flush and fifteen voice calls (voice_start 0x19cde0 and the
rest) use it. Before a no-wait `sceSifCallRpc` on one buffer's client, it spins on
`sceSifCheckStatRpc` (0x3ff328, now named) until the other client's last call is back. That is
true while the packet's busy bit (+0x10 & 1) is set and its id matches.

fly_and_collide's images were saved with one of those calls in flight. Nothing in the interpreter
plays the IOP, so the bit never clears. C10's images never hit this: their packets were already
answered, and their runs never sent twice on the same client.

## The hook

`whole_frame` hooks `sceSifCheckStatRpc`. Called from inside `iop_rpc_send` (its return address in
the function's 344 bytes), it answers 0: the last call is back, as on hardware, where the IOP answers
well within a frame. Called from anywhere else, it runs as it is (`through`), so libpad's own waiters
are untouched. The rest of the send runs as recorded: the buffer swap, the counters, and
`sceSifCallRpc` with its DMA already stubbed (`SIF_SET_DMA`).

## The judging

The new test is `fly_and_collides_wrecks_are_the_originals`. It loads the fly capture through
`TwoP::load_from` (the same watch/2p.toml columns), and finds each wreck's image with
`image_ending_a_frame_in` (s228's images, up to 14 back). Then it runs `judge_whole` to the frame after
the wreck, with the same checks as S176's 2p test.

| wreck (row, reason) | image | held to | original's first wreck | port |
|---|---|---|---|---|
| 137251, 3 crossup | s228-137240 | 137252 | frame 137250, [3] (pitch -23.88, roll 18.55, hang 1.618) | [3] |
| 138436, 8 roll | s228-138425 | 138437 | frame 138435, [8] (roll -85.28) | [8] |
| 139446, 1 front | s228-139435 | 139447 | frame 139445, [1] (fore-aft g -56.64) | [1] |
| 139894, 6 scenery | s228-139883 | 139895 | frame 139893, [6] | [6] |

All of the 48 calls of the original's wreck test on P1's bike (12 a run) enter the same reasons in the
port, including the ten-plus riding calls before each wreck that enter none, and the held calls after
them. Each wreck is in the frame before the row that first shows it. That is the row read before its
own frame's simulation (`Shows::Before`), the same two-frame lag S228 saw between the port's wreck
tick and the first row showing it.

## How it was proved

- `./kit check -- -p game-sim --test ragdoll_wrecks -- fly_and_collides the_2p_wrecks_are_the_originals`
  passes. So does the whole ragdoll_wrecks file (16 tests). So S176's 46 2p events still hold with the
  hook in place.
- `./kit audit` reports no new leaks. The two it lists are in plan/DONE.md, from other tasks.

## Not verified / not 1:1

- **The IOP's answer is assumed, not replayed.** The sound RPC's reply buffer (recv, 0x20 bytes) is
  not filled the way the IOP would fill it. Nothing in these four runs reads it on the way to the wreck
  test: the runs hold the capture bit for bit. But a run long enough for a voice call to read a reply
  could part there.
- **Only P1's wrecks.** P2 idles for the whole tape, and both biplanes stand parked, so nothing else
  on fly_and_collide wrecks.
- **The ragdoll after each wreck** is not compared here. These runs stop a frame after the wreck's
  row.
