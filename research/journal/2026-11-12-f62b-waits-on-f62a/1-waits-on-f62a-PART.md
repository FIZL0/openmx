# 1 — waits on f62a (PART)

Task F62b. 2026-11-12.

## What I did

- Nothing ported yet. F62b sits directly on F62a: vt +0x88 (the clip advance and crossfade) is called only
  from the rider's idle layer (vt +0xd4), and F62a is the task that ports that layer as `Call::Idle` in
  `rider_leaf`, leaving vt +0x88 as an outside call for F62b to serve.
- F62a is finished and pushed (`origin/task/F62a`, commit "F62a: port the rider's idle layer ..."), but the
  runner has not merged it to main yet. It changes the same files as this task (`handling/anim.rs`,
  `handling/rider.rs`, `tests/mxvatv_bike_ride.rs`).
- Doing F62b now would mean either merging F62a's branch into this one (the rules say only the runner
  merges) or hooking vt +0x88 inside the *interpreted* idle layer in the test. That second harness would
  be thrown away as soon as F62a lands, and it would conflict with F62a's edits to the same `rider_leaf`
  code. Also, the acceptance line ("census 0 but rand/heap/cue") can't be met until F62a's port is on main.
- So I blocked the task with `--after F62a`, which reopens it as soon as F62a merges.

## What the original does

Not studied yet in this session. The brief says vt +0x88 is ~1.9 KB with two 2-2.4 KB callees, plus the wreck
states' clip start (vt +0x90 under it), and only flip-1's idle layer reaches it (68 and 2 calls).

## How it was proved

Not proved: nothing was ported.

## Not verified / not 1:1

- Everything in the task: vt +0x88, its two callees and the wreck states' vt +0x90 are still the original's,
  run in the interpreter. The task is blocked until F62a lands on main.

## Next

Once F62a is on main: read F62a's journal (research/journal/2026-11-12-f62a-rider-idle-layer) and
research/oracle.txt `# F62`/`# F62a`. Then `./kit fn -p atv` on the rider controller's vt +0x88 and its
callees, port them into handling/anim.rs, and serve the outside clip-advance call in `rider_leaf`'s idle
arm. Prove it with `flip_replays_with_the_rider_leaf_on_the_ports`, with the tail's rider census left at
rand/heap/cue only.
