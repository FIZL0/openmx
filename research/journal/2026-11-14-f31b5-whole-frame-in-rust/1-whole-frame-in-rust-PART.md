# 1 — the whole frame in Rust (PART, superseded by 2)

Task F31b5. 2026-11-14. Cut off by a usage limit; work in progress, the session resumes here.

## What I did

- `replay_in_pure_rust` (tests/mxvatv_bike_ride.rs): `replay_pure` with every port at once: replay_whole's,
  the rider leaf, the wheel contacts, the other listeners dropped, the part walk's models dropped. A new
  switch, `"no original beside"`, runs the ported vehicle update without the original beside it.
  `FRAME_RAN` counts every instruction the second run's frames interpret.
- Two tests, `ride/flip_replays_in_pure_rust`, assert that count is 0. They are `#[ignore]`d until it is.
- Result (run by hand): both replays are C15's on every row (flip wrecked 125, put back 193), the bike
  the handing run's, read watch clean. The interpreter ran 757,675 instructions over ride-2 (1.65k a
  frame) and 591,831 over flip-1 (60M before, in the 11-08 census).

## What is left (call site in the test file → original function, calls / instructions, ride-2 · flip-1)

- `posts_record`'s record pass → 0x1b7470: 9 / 120k · 3 / 54k (a heap call?)
- the tail's `paused()` lookup → 0x214b30 (name bit): 459 / 139k · 465 / 140k; the state's vt +0xac → 0x507710: 459 / 8k
- rumble → 0x51a250 (ready), 0x1540b0: 24 calls, ~1k. Pad output: drop.
- `port_wreck` (line ~4148) → 0x4a4f20 every frame (20k); on the flip also 0x272300 (post), 0x4e4e50,
  0x4e19a0, 0x1143e0, 0x18d540, 0x194da0/e00, 0x266c90, 0x4fa760, 0x128040/80, 0x4f7590.
- the part walk's effects items (line ~5325) → 0x514e40 (emitter other kind send, 401k · 119k),
  0x50ab00, 0x50aaa0, 0x4e9f00, 0x50a920 (item object virtuals).
- line ~5572 → 0x348fa0, 0x348ff0 (0 instructions: hooked).
- line ~5955 → 0x1b7620, 0x1b76d0 (heap, ~4k).
- `port_update`'s Lean on the flip (line ~7921) → 0x134680: 1 call, 140k.
- the rider leaf (line ~18805): rand 0x3561a8, cue 0x197ef0/0x1985d0, sound 0x14ea30/0x14ede0.

Next: port or drop each (pad and sound output can go under the read watch), then turn the two tests on.

## Not verified / not 1:1

- The acceptance is still open: the frame interprets the calls listed above.
- The frame still runs over a RAM image held by `Ee`, not over Rust structures.
- The gate was not run this session.
