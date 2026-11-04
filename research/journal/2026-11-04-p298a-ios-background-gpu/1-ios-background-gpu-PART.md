# 1 — ios background gpu (PART)

Task P298a. 2026-11-04.

## What I did

- Nothing in the product. The task only applies "if plan/HUMAN.md's P298 jetsam report says memory".
  That report (HUMAN.md, "Why iOS ended the app in the background, and does resume work (P298)", step 1)
  is still the bare request P298 wrote today. Nobody has filled in a JetsamEvent reason, the
  `openmx-log.txt` "saved after N ms, memory … MB" line, or any memory-warning lines. There is no device
  here to get them from.
- I decided not to build it anyway as a guess. HUMAN.md and P298's journal both say P298a is moot if
  iOS ended the app for some reason other than memory. The change also has a cost: a stall on every
  return to the app while the whole ride's world uploads again, plus a ride-loop path that no test can
  run off iOS (`resume::away()` is never `Some` there). Without the report there is nothing to weigh
  that cost against.
- Blocked the task on the human's report.

## What the original does

Nothing to port: the PS2 game has no suspend.

## How it was proved

Not applicable: no change.

## Not verified / not 1:1

- **Why iOS ended the app in the background is unknown.** This needs the device's JetsamEvent report
  (plan/HUMAN.md P298 step 1). No device here.
- **A ride still keeps its GPU world, streamed pieces and decoded audio in the background**, unchanged
  from P298.

## Next

What the human must do: plan/HUMAN.md's P298 step 1. If the JetsamEvent for openmx says
`per-process-limit`, `highwater` or `vm-pageshortage`, reopen P298a. The work is: in the ride loop, call
`game_render::gpu::Renderer::forget_assets` on DidEnterBackground, upload the ride's assets again on
WillEnterForeground before the next frame (MD97 already holds back ground uploads while the app is away
and rebuilds held pieces on return, so reuse that hook), and drop decoded audio clips the mixer can read
again. If openmx is not in the report, close P298a as moot.
