# 1 — riding ragdoll at rest (FINAL)

Task S255. 2026-11-02.

## What I did

- Read what the original does with a riding body that has come to rest: nothing special (below). It solves
  every tick, all three passes, whatever the bike and the bones are doing.
- Checked whether the port could skip the step *exactly*: if a tick's whole input (the bones, the contacts,
  the drive, the bike box and frame) were bit-identical to the last one, the output would be too, and the
  step could be skipped without changing a bit. I logged that input for both riders every tick over 600
  ticks of `openmx --headless --players 2` on free06, free07, free09 and free11. It never repeated, on any
  of them. The bike does not hold still bit for bit. On free06 the headless riders are not at rest at all:
  they roll down the start hill at up to 40 ft/s. On free09's flat they creep (0.4 ft/s at tick 270), and
  on free07 too. A rest test that fires has to be a lossy one, so I did not build one (proposal below).
- Made the exact solve cheaper instead. Per step, timed inside the shell (temporary `Instant` counters,
  since removed), 0.40 ms went: forces 0.04, integrate 0.014, constraints (the holds and joints) 0.33, bone
  collisions and bike box 0.011. A stack sample of the step (eu-stack, 1689 samples) puts 81% in
  `Pin::two_body`. That is all ps2-float four-lane VU ops, and the op bodies `vu::madd4`/`msub4`/`sub4` were
  **out-of-line calls** (`.llvm.*` symbols). F15a made `Vf`'s methods `#[inline]`, but LLVM declined
  `add4`/`sub4`/`mul4`/`madd4`/`msub4` behind them. They are now `#[inline(always)]`. Nothing else changed:
  the same IEEE operations in the same order, bit for bit by construction.
- Tried and dropped: `#[inline]` on `chassis::turn`, `biplane::unturn` and the ragdoll's
  `cross`/`sumw`/`prod`. The step got slower (0.26 against 0.19 ms on the same load), so it is not in. A
  `-C target-cpu=x86-64-v3` build gave nothing measurable either (0.45 against 0.47 ms a step, inside
  the noise). The lane width is not what costs.
- Added `bench_riding_step` (ignored) to `crates/game-sim/tests/ragdoll_riding.rs`: 2000 riding steps
  (`Ragdoll::step`: the forces, the integrate, the holds and the joints, no collisions) from the actor
  in S109's animandpump RAM image, best of 7 rounds (`BENCH_ROUNDS`).

## What the original does

- `rider_physics_update` (`./kit fn 0x0030f6e0`, oracle.txt): `placed && active && !repose` →
  `dynamics_world_step` on the actor's world. There is no speed test, no sleep counter and no "settled"
  test in the riding branch. The settle test and its damping switch belong to the wreck's arm.
- `dynamics_world_step` (`./kit fn 0x001f29a0`, read again for this task): the preforce listeners; then
  `n = (int)(dt·rate)+1` substeps, each running every force listener, every body with a non-zero first word
  (integrate, then end of substep), and the whole constraint list `world+0` times; then the post
  listeners. No body or world sleep anywhere in it. The riding world is three passes at rate 25
  (`RIDE_WORLD`), so at 30 Hz it is one substep of three passes over 4 holds, 2 + 7 bone collisions,
  17 joints and the bike box.
- The props do have a sleep (`prop_world.rs`, the parts' `SpeedThreshold` etc.), and so does the chassis
  (`+0x1b4`). The rider's ragdoll world has none: the original just runs it, on a VU0 that does a `vmadd`
  in a cycle.

## How it was proved

- Exactness: `cargo test --release -p game-sim --test ragdoll_riding`: 7 passed, among them
  `the_riding_tick_is_the_originals_from_each_row` (all 1203 of run1's riding ticks bit for bit against the
  original) and `the_wreck_enters_from_the_riding_bodies`. `cargo test --release -p ps2-float`: every suite
  passes (20 + 7 + 10 + 19 + 6), including the four-lane-against-scalar checks over every class of input
  and `results_are_identical_on_every_platform`. `cargo test -p game-sim --no-fail-fast`: 1061 passed, 1 failed (the_humans_tricks, the known red on C6's capture in s1), 20 ignored.
- Speed, `bench_riding_step`, the two builds' test binaries run in turn, four pairs, load average 10–15:
  base 0.314 / 0.301 / 0.300 / 0.299 ms a step, inlined 0.250 / 0.239 / 0.241 / 0.242 ms: **−20%**.
- Speed in the shell, `OPENMX_THREADS=1 openmx --headless --track free06 --players 2 --timings --frames 600`,
  four interleaved pairs (session `sim` ms a frame), base → inlined: 1.92 → 1.31, 1.59 → 1.18,
  1.09 → 1.00, 1.04 → 0.97. The inlined build was faster in every pair. The two quietest pairs read 7–8%
  off sim. Inside the shell (my temporary counters) the riding step went from 0.40 to 0.28 ms.

## Not verified / not 1:1

- Not measured on the Steam Deck: there is no Deck reachable from this machine. All the numbers above are
  this desktop's (a shared machine, load 10–15). The Deck runs the same generic x86-64 build
  (`tools/build.sh`), so the inlining should carry over, but how much is unmeasured.
- The `openmx` crate's tests were not run (the change is in ps2-float's codegen only, and the game-sim
  suite covers the replay and hash vectors).
- No rest check was built, so the riding body still costs the full solve at rest. The split screen on the
  Deck is cheaper by about a fifth of that solve, not cured. The rest check is proposed below and needs the
  user's ruling, because it cannot be both exact and ever fire.

## Proposal: a rest check (not built, needs a ruling)

The only rest check that fires is a lossy one. That breaks AGENTS.md's bit-exact rule for the riding body,
which is sent and hashed online and compared tick for tick against the original in
`the_riding_tick_is_the_originals_from_each_row`. A shape that should keep riding-ragdoll-stays-off's
springiness, because it only engages where nothing springs:

- **When:** the bike is grounded with its speed under `STAND_SPEED` (the stand-up speed the pose damper
  already switches at) and its rates under a small bound, the request has no hold or mode change, and on the
  last *full* step no bone moved more than ~1e-4 ft against the bike's frame.
- **What:** step the world with one constraint pass in place of three, or carry the bones rigidly with the
  bike's frame and skip the world. Any input past the bounds (throttle, a lean, a landing, the bike moving)
  goes back to the full step that same tick, so sway and settle on riding and landings stay the original's.
- **Proof it would need:** watched on screen standing and pulling away (the ruling's demand), and the
  replay and hash tests re-pinned with the lossy path named.

## Next

- If the user wants more than the inlining, rule on the rest check (filed as F22, `later`).
- Exact options still open, each a ps2-float change and none tried here: every four-lane op re-clamps its
  operands (`vu_bits4`) even when they came straight out of another VU op, and the chop's narrow, widen and
  compare could be cheaper. A wider target was measured and gave nothing.
