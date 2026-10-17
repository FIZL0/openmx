# 1 — the clutch that never comes out (FINAL)

Task P11. 2026-10-17.

## What I did

The brief's defect — `ride.clutch_in` is a latch nothing clears, so one frame at rest off the
throttle and the bike never moves again — **was already half-paid when I arrived.** P8 (`77103cb`,
merged since the brief was written) added `ride.clutch_in = held.clutch;` one line above the idle
rule, which copies the rider's lever over the latched field every frame. Measured on P10's own
fixture, the bike no longer reaches 0.0 ft/s with the engine at 7474 rpm; it reaches 18.6 ft/s.
So the task was not "find the latch" but "finish it", and there were two things left.

**1. The port keeps two copies of a word the original keeps one of, and the idle rule wrote the
wrong one.** `crates/game-sim/src/ride.rs`. The original has exactly two clutch floats on the bike:
the rider's lever and the input layer's rate-limited copy of it, and the second one is both what the
engine reads through its own pointer *and* what the idle rule stamps a 1 into. Because that one word
persists, the input layer's rate limit is also what lets the clutch back **out**: next frame it walks
the word from 1 back down to the lever at the file's `ClutchOutRateLimit`. There is no "let the
clutch out" rule anywhere, and there does not need to be one — the rider feeds it out over a quarter
of a second on a bike whose file says 4, which is the answer to the brief's question of *where*.

The port had split that word in two: `Held::clutch`, which the input layer walks, and
`mv::Ride::clutch_in`, which the idle rule wrote and the engine read. Nothing walked `clutch_in`, so
before P8 it latched, and after P8 it snapped from fully in to fully out in a single frame — a
dumped clutch, not a fed one. The idle rule now writes `Held::clutch`, the one word the input layer
owns, and `clutch_in` is set from it immediately afterwards as the copy the engine reads. Two
writers, one word, exactly as the original.

`Step::clutch` moves with it, from `held.clutch` to the rider's raw lever `c.clutch`. That is what
the original's launch-ramp function actually reads (`+0x644`, not `+0x65c`; see below), and keeping
the two apart is what stops the idle rule's 1 from rearming or stalling that ramp now that it lands
in `Held`. On every fixture in the tree the two are the same value — nobody's rider touches the
clutch lever — so this moved nothing by itself; I checked, by reverting it alone and watching the
canonical vectors fail the same way.

**2. P10's test rode a bike with a 1.6 ft wheelbase.** `crates/game-sim/tests/ride.rs`. The test took
its wheel mounts from `mounts(&p)` where `p` is the *patched* bike, and section (8)'s `launch_bike`
patch moves `BodyTireForcePosition` to a tuning arm — a key this crate reads both as the drive
force's arm and as the wheel mounts. So the test's bike had its wheels hung 0.3 ft *below* the body
origin, 1.6 ft apart, and wrecked at 18.6 ft/s with reason 11 at frame 31 whatever the clutch did.
It then *froze*: the test built a `Rider` with no wreck body, and `Rider::frame` steps neither the
ride nor a ragdoll in that state, which is why every field in a trace of it is bit-identical from
frame 31 to frame 600 and why the failure read as "the bike will not move". The mounts now come off
the unpatched file, the way `ride_the_minute` does and for the reason its comment gives, and the
rider carries the wreck body so that a wreck is an assertion failure rather than a frozen rider.

With both: 65.7 ft/s after ten seconds of throttle, from one frame or from a full second at rest,
and no wreck.

**Tests.** `it_pulls_away_after_a_frame_off_the_throttle` is un-ignored and now asserts four things
rather than one — the clutch is fully in while the bike sits there, the first frame on the throttle
leaves it between 0.9 and 1.0 (fed, not dropped), it is out 15 frames in (the file's quarter second,
±2), the bike reaches more than 20 ft/s, and nothing wrecks. The same test now exists through the
pad in `crates/openmx/tests/ride.rs`, which the brief asked for: that file had no `#[ignore]` left to
take off, because this leg was never one of the minute's — the minute's script has `down cross` on
frame zero and so can never see the defect. An idle pad for a frame, then `down cross`, is the state
a session actually starts in, and it is worth a test of its own in the file whose failure would be a
player's complaint.

**The canonical vectors moved**, and the three `HUNDRED_TICKS` constants with them. The bike at rest
off the throttle now holds its rate-limited clutch at 1 instead of 0, which is a hashed field, and
the canonical run's random button bytes press the clutch, so its physics moved too. Regenerated with
`print_the_vectors` and pasted back by constant (`HASHES`, `TRACE_DIGEST`, `POS_Y`, `VEL_Y`,
`FINAL_DUMP_HEAD`, `FINAL_DUMP_DIGEST`, `FIXTURE_TEXT_DIGEST`); `FINAL_DUMP_LEAVES` and
`EVERY_LEAF_HASH` did not move, which is the check that the state *type* did not change — only what
the state does.

## What the original does

- `./kit fn 0x00257a00` — the ground step. One write to the bike's rate-limited clutch and no
  clearing of it, at the same place the port's idle rule sits: `if (speed < 7.3333 && throttle == 0)
  clutch = 1.0`, with the shift decision and then the launch-ramp call after it. The port's
  `mv::IDLE_SPEED` is that literal.
- The input phase (the function that walks every command toward the pad at the `[Joystick]` rates;
  it is the one containing `if (clutch_rate_limited < lever) …` just above the ground step's caller,
  and `grep -n "+ 0x65c) =" context/decomp/boot.c` finds its three non-vtable writers) walks the
  rate-limited clutch toward the **lever** at `ClutchInRateLimit` or `ClutchOutRateLimit` depending
  on direction, every frame, in place. That is the only thing in the game that lets the clutch out,
  and it works on the idle rule's 1 because they are the same word.
- `./kit fn 0x002572f0` — the launch ramp (the port's `mv::clutch`). Its two conditions read the
  **lever** and the raw throttle, not the rate-limited pair, so the idle rule's write does not reach
  it. The port now matches that for the clutch.
- P13's entry already read `./kit fn 0x00246fd0` (the engine) and settled that `engine_update` and
  the idle rule's condition are not defects; nothing here contradicts that, and neither of them
  changed.

## How it was proved

- **Against the original's code**: the three functions above, read for the two writers of the
  rate-limited clutch and for which of the four clutch-ish floats each of them touches. The claim
  this task rests on is structural — one word, two writers, the rate limit doubles as the release —
  and it is readable in the decompilation.
- **Against the port's own statement**: `./kit check` — 550 tests, all pass, with
  `it_pulls_away_after_a_frame_off_the_throttle` live in both crates for the first time. Measured on
  the game-sim fixture, from one frame and from one second at rest: clutch 1.0 → 0.9333 on the
  first throttle frame, 0.8667, 0.8, 0.7333 … and 0 fifteen frames in (`ClutchOutRateLimit` 4, a
  frame of 1/60 — a quarter of a second to the bit), top speed 65.68 and 65.58 ft/s, no wrecks. The
  same shape through the pad in `openmx`.
- **A control that the test is not vacuous**: the pre-fix run of the same test, traced frame by
  frame, is bit-identical from frame 31 onward — that is what caught the frozen-rider fixture bug
  rather than an assertion being tuned until it passed.
- **No emulator was run.** Nothing here needed a frame of video or a PINE session: the defect is a
  structural one in the port, the original's side of it is three functions of decompilation, and the
  bike it is measured on is the tree's hand-written stand-in rather than the disc's.

## Not verified / not 1:1

- **`ClutchOutRateLimit` is not the disc's number.** The quarter second the clutch is fed out over is
  `4` from `crates/game-sim/src/stock.rs`, the hand-written stand-in bike, and the two new tests
  assert against that. The *shape* — the release is the input layer's rate limit and not a rule of
  its own — is the original's; the duration on a real bike is whatever `bike\race*.phy` says.
- **The regenerated `platforms.rs` vectors are proved on x86-64 only.** The cross-architecture test
  (`the_pinned_vectors_hold_on_another_architecture`) cannot run in this worktree: its script's
  first-run fetch of an upstream toolchain into `/tmp/openmx-cross` returns `curl: (22) … 404`, so
  it fails on the download and never reaches a vector. The test is `#[ignore]`d, so `./kit check` is
  unaffected, but nobody has yet shown that the new numbers are the same on aarch64 or wasm32.
- **`Step::gas` is still the rate-limited throttle where the original's launch ramp reads the raw
  lever.** `./kit fn 0x002572f0`'s first condition is on the raw throttle word; the port passes
  `held.gas`. I moved `Step::clutch` to the lever because the fix required it and because it changes
  nothing on any fixture, and left `Step::gas` alone because two of its three readers (the donut and
  the low-speed upright controller) are not this task's and the change is not free there. It is a
  real divergence, worth a task.
- **The original's throttle release is instantaneous and the port's is rate-limited.** In the input
  phase the throttle is walked up at its rate but assigned straight down (`else { throttle = lever;
  }`), where the port's `toward` is symmetric. This decides exactly when `gas_rate_limited == 0` and
  so when the idle rule arms — which is this task's own trigger — but it is an input-layer defect
  rather than a clutch one, and fixing it blind would move every fixture in the tree. Also worth a
  task.
- **Nothing was compared against a recording.** No capture of the original shows a rider pulling
  away from a standstill, so the quarter-second feed is asserted against the file's own key and not
  against the disc. The human's acceptance recording starts rolling.
- **The wheel mounts / drive-force arm tangle is still open.** P8's journal asks the question and
  this task did not answer it; it only stopped one more test from reading the key the wrong way
  round. Three test harnesses now work around it by reading the mounts off an unpatched copy of the
  same text.

## Next

Nothing is owed for P11 itself. The two input-layer divergences in *Not verified* are the natural
follow-ups, and the second (the throttle's one-sided rate limit) touches the same idle rule.
