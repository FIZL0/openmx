# 1 — s8 acceptance (READY)

Task S8. 2026-10-15.

**S8 is not done: there is no recording of real play to replay, and nothing in the port to replay it
against.** What landed is everything around that hole — the recipe for the recording, the reader that
turns one into a fixture, the scale the harness has to work at, and three measurements that would each
have made the first real acceptance run wrong. The hole itself is one human item (`plan/HUMAN.md`) and
one open task (R7); see *Next*.

## What I did

**`watch/acceptance.toml`** (new) — the recipe. One row per frame carrying *both halves* of a frame,
because a replay needs both and a capture of one is worthless:

- **the input**: the sixteen controls the game's own input manager believes are held (`@pad0_control_state`,
  C5), plus both halves of libpad's port-0 buffer and both stamps, so the row can be checked against
  itself and the sticks can be read;
- **the state**: the player's vehicle actor — velocity (+0x130) and the 4×4 world transform whose row 3
  is the position — walked down R6's chain from the GameManager singleton;
- **the frame**: the game's own tick counter, the `dt` it was handed, and the C library's 64-bit LCG
  state (R5), which is the seed a replay has to start from.

It is `every = 1` and says so twice, because a capture with a gap in it cannot be replayed at all.

**`crates/game-sim/tests/replay.rs`** (+~700 lines, appended; nothing above was touched) — the reader and
its proofs. `acceptance_fixture(&Capture)` turns a capture of that spec into a complete `Fixture`:
inputs from the control columns, the seed from the RNG column, watches from the bike columns. A2's
`Capture` only makes watches, so **the input side is new**: a capture of the original could not drive the
port before this.

Thirteen tests, and the three findings below are each one of them. Plus the scale the word "acceptance"
implies: `several_minutes_replay_exactly` is 9,000 ticks (five minutes at the game's 30 Hz) rather than
A2's 600, `a_divergence_deep_in_a_long_run_names_its_tick_and_field` perturbs one bit at tick 8,765 and
checks it is reported there and by name, and `a_long_fixture_round_trips_as_a_file` round-trips 9,000
`frame` lines and 36,000 float patterns byte for byte.

`the_acceptance_run` (`--ignored`) is where the recording lands: it finds either
`context/capture/acceptance-freeride.csv` or `context/fixtures/slow/acceptance-freeride.fixture` on its
own, builds the fixture, replays it, and panics with A2's one-line divergence. With neither present it
prints the three steps that make one and passes — the same thing `replaying_every_fixture_in_context`
does about an empty directory, and for the same reason: a test cannot fail over a file the repository is
not allowed to hold. **The absence of the recording is recorded here and in `plan/HUMAN.md`, not as a red
test.**

**`watch/pads/all-buttons.pad`** (new) — every button `./kit pad` can send that the front end ignores,
then the left stick to each extreme. For proving an input decode, not for playing: start and cross are
deliberately absent so nothing loads mid-capture (a load stalls the tick counter, and a capture with a
stalled tick is not replayable).

## What the original does

Nothing new was decompiled. What this task *measured* about the original, all three of which were
assumptions I had written into the reader before the first capture contradicted them:

1. **The simulation ticks at half the video rate, and the recorder's `frame` column is the video one.**
   `frame` rises by 2 on every row while the game's own counter (`@frame_counter`, app+0x374) rises by 1:
   300 of 300 rows, every delta exactly 2 and 1, in two captures. So A2's `Capture::tick_of` — which
   knows only about `frame` — reports **twice** the tick for this game, and a fixture built on it would
   have compared the original's tick 1 against the port's tick 2 for the whole run. Every watch is
   re-ticked off the game's counter instead (`ticks_of`), and
   `the_ticks_come_from_the_games_counter_not_the_recorders_frame` pins both the right answer and the
   trap.
2. **`dt` is measured, not fixed.** Three distinct values in 300 frames of the front end
   (0.03283332288265228, 0.03286665305495262, 0.033330000936985016), and R6 saw 0.041532 in a level. The
   game asks how long the frame took; it does not assume. A fixed-step port cannot reproduce that, so the
   reader reports the spread in the fixture's name rather than refusing the capture — what to do about it
   is a decision about the port's tick (S1), not about this reader. This is the single most important
   thing in this entry and it is expanded in *Not verified* below.
3. **The game's controls are not libpad's bits, and the half the game read is the older one.** The input
   manager's control *k* comes from the wire bit in C5's control map, which is not the identity (control
   0 is triangle, 0x0010; control 6 is L2, 0x0001). I had written the identity into the decode on the
   strength of "12 = up, 14 = down" in `watch/pad.toml`, and the second capture failed it immediately.
   Eight of the sixteen entries were re-measured here and agree with C5's live read of the map exactly:

   | control | 0 | 3 | 4 | 5 | 6 | 7 | 12 | 14 |
   |---|---|---|---|---|---|---|---|---|
   | wire bit | 4 (triangle) | 7 (square) | 2 (L1) | 3 (R1) | 0 (L2) | 1 (R2) | 12 (up) | 14 (down) |

   The fixture's buttons are indexed by *control*, not by wire bit, because the control is what the
   simulation reads; `CONTROL_MASK` exists only to cross-check against the wire.

## How it was proved

`./kit check`: **162 tests pass** (was 150 before this task; 13 new in this file, of which the acceptance
run itself is `--ignored`; both of the file's `--ignored` tests run clean). Two live captures, both from save state 9 (C3's main menu) on emulator copy 5,
`./kit record watch/acceptance.toml`, 300 rows each, `skipped 0 torn read(s)`:

- `context/capture/s8-input-chain.csv` — `--pad watch/pads/menu-down-up.pad`, frames 5764..6362;
- `context/capture/s8-input-buttons.csv` — `--pad watch/pads/all-buttons.pad`, frames 5766..6364.

`every_input_capture_in_context_agrees_with_the_wire` reads every capture in `context/capture/` that has
the acceptance spec's input columns and checks the two independent sources of each frame against each
other — the game's control state, and libpad's button word from the half the game read. On both
captures: **300 of 300 rows agree with the older half**, and 293 / 289 with the newer one (the rows where
a press began or ended between the halves being refreshed). Taking the newer half would be right 97% of
the time, which is the kind of wrong that survives every short test and desyncs a five-minute replay.
The same test reads both captures as inputs (299 each, one per tick after the first row) and reports the
`dt` spread.

**The stick decode, against what was commanded.** `all-buttons.pad`'s five stick positions came back as
exactly five decoded values: `stick l -1 0` → `(-127, 0)`, `l 1 0` → `(127, 0)`, `l 0 -1` → `(0, -127)`,
`l 0 1` → `(0, 127)`, centre → `(0, 0)`. So bytes [6] and [7] of the buffer half are LX and LY and the
centre is 0x7f, measured rather than assumed. RX/RY never left 0x7f.

**The reader, end to end, against a simulation.** `a_capture_of_real_play_becomes_a_fixture` builds a
capture in the recorder's exact shape (46 columns, the vsync counter rising by 2, the measured `dt`
values cycling, both buffer halves with `stamp_a = stamp_b + 1`, the bike columns filled from the port's
own run) and checks every decoded input value, then *aims the same capture at lanes the port does have*
and replays it tick-exact. That is the whole pipeline — columns → inputs + seed + watches → `verify` —
proved against a running simulation rather than against my reading of the format.

Four ways a capture can look like an acceptance recording and not be one are each refused with the frame
it happened on: a chain that resolved to a non-vehicle actor, an actor nobody is riding, a chain that
went null (one empty cell), and an unreadable `dt`. A missing frame is refused by the tick counter
(`frame_counter 2280 follows 2278`). `the_watch_spec_and_this_reader_agree` reads
`watch/acceptance.toml` and asserts it still declares every column the code reads — the only thing
connecting the spec to the reader, and a rename in either would otherwise surface minutes into a
recording session.

## Not verified / not 1:1

- **The acceptance run itself has not happened.** There is no recording of real play anywhere on this
  machine (`context/` in this worktree and in the main checkout both have no `recordings/`,
  `fixtures/` or pre-existing `capture/`), and the human item for it in `plan/HUMAN.md` ("One longer
  recording of free riding") is still open, as is slot 4. Everything in this entry is the machinery
  around it. **The port has never been compared against the original's play, by this task or any
  other.**
- **…and it could not be, because the port has no bike.** `Sim` is still A1's example ball: S1–S5, S10
  and S11 are all open, so every `bike.*` path in `BIKE` is a path the state does not have.
  `the_acceptance_fixture_names_the_field_the_port_is_missing` asserts exactly that — today's verdict is
  `NoSuchField bike.pos[0]` at tick 0 — and it will fail when S3 lands, which is the point: that failure
  is the moment the acceptance run becomes a real comparison, and whoever lands the ported movement owns
  updating it. The eight path names in `BIKE` are my choice, not a ported state's: they are what the
  actor is *expected* to walk as.
- **A measured `dt` may make a tick-exact replay impossible, and I did not settle it.** The original
  integrates with the frame time it measured (finding 2 above); the port is fixed-step by design
  (`lib.rs` rule 1: "one tick = one frame with no `dt`"). If the simulation really consumes app+0x344,
  then no fixed-step port can reproduce a human's recording bit-exactly, and the acceptance run has to
  carry `dt` as part of its input (an `Input` field, or a per-tick list) — which changes `Sim::tick`'s
  signature and is S1's decision, not mine. What I have is two observations (the front end here, R6's
  level) that the value varies; what I do *not* have is the decompiled read of app+0x344 inside the
  physics step, which is what would turn this from a worry into a fact. **Nobody should write the port's
  tick without reading this paragraph.** Start at `./kit fn 0x00318c30` (the type-4 actor's
  constructor) and `./kit xref 0x00479cd4`.
- **The input decode is not proved to reach the simulation.** `Sim::tick` ignores its `Input`, so
  `a_capture_of_real_play_becomes_a_fixture`'s replay would pass with the inputs shifted, reversed or
  empty. What is proved is the decode itself (against libpad's own word, 600 rows) and the *watch*
  alignment (against a real run). The input alignment — row *k*'s controls are the input of tick *k*,
  so row 0's are dropped — is reasoned from "the recorder samples after the tick" and is **not
  measured**. It is the first suspect if a real acceptance run diverges at tick 1 and never recovers;
  `Watch::shifted` and A2's `an_off_by_one_in_alignment_fails_at_the_first_tick` are the knob and the
  signature.
- **Eight of the sixteen controls were re-measured, not all sixteen.** Circle, cross, select, start, R3,
  L3, right and left come from C5's live read of the control map only (select cannot be sent at all).
  They are in `CONTROL_MASK` on C5's authority.
- **No capture of a *level* was made.** Both of mine are of the front end, so the bike columns of
  `watch/acceptance.toml` have never resolved: the spec's actor chain is R6's, unchanged and
  untested-by-me, and its five `-> 0x8` hops were right for a one-player Freestyle session and will be
  wrong for another mode. The spec and `acceptance_fixture` both check `bike_type == 4` and
  `bike_player != 0` so that a wrong hop count is a sentence rather than a replay of the wrong actor,
  but *that check has never run against a filled cell*. I did not try to reach a level: R6 did it
  unattended from the slot-9 menu state but did not commit the pad script, so it would have been blind
  menu navigation, and the capture would have been Freestyle on a monster truck rather than the freeride
  recording S8 asks for — against a port with nothing to compare.
- **The fixture format cannot hold the original's whole RNG state.** `@rand_state` is 64 bits (R5) and
  `Fixture::seed` is 32, so `acceptance_fixture` keeps the low half and prints the whole thing in the
  fixture's name. A mid-session recording therefore cannot seed a replay exactly. The fix is the same
  `FORMAT_VERSION` bump A2 wants for `Fixture::start`.
- **`dt` is not watched and neither is the RNG state.** Both are recorded in the capture; neither is
  compared per tick, because the port's `rng` is a `u32` stub rather than the 64-bit LCG and a watch
  against it would fail for a reason that has nothing to do with the game. A diverging draw count is the
  commonest cause of a desync, so this is the first watch to add once `Sim` holds `rng::Rng`.
- **The captures themselves do not survive this worktree.** `context/` is per-worktree and git-ignored,
  so `s8-input-chain.csv` and `s8-input-buttons.csv` (with their provenance sidecars) are gone when this
  worktree is removed. The numbers measured from them are in this entry; the files are not evidence
  anybody else can re-open. `watch/pads/all-buttons.pad` and `watch/acceptance.toml` are committed, so
  both are reproducible in ten seconds each.
- **Not run on a second architecture.** The reader does no float arithmetic (it compares patterns and
  parses text) so A3's conclusion should carry, but these tests have only run on x86-64.
- **The long-run tests use the port's example ball**, which free-falls: nothing lands, nothing saturates
  and nothing wraps in 9,000 ticks. They prove the harness at length, not the harness against a
  *realistic* five minutes — a ported simulation with a state machine in it may well find something
  these do not.

## Next

For the next session, in this order:

1. **The recording does not exist. Get one.** Either the human item in `plan/HUMAN.md` (slot 4 + "One
   longer recording of free riding"), or **R7** — the FreeRide boot path, which makes a freeride area
   reachable with no menus and nobody playing, and which is what turns the acceptance run from a
   once-ever human artefact into something an agent can re-record whenever the port changes. R7 first is
   the better order: a repeatable recording is worth more than an irreproducible one, and
   `watch/acceptance.toml --state 4 -n 9000 -o context/capture/acceptance-freeride.csv` is then the
   whole job. Write the capture into a shared `context/`, not a worktree's, or it dies with the session.
2. **Settle the `dt` question** (second item in *Not verified*) before writing the port's tick. It
   decides whether a tick-exact acceptance run is possible at all.
3. Then S1→S3: the first ported movement makes
   `the_acceptance_fixture_names_the_field_the_port_is_missing` fail, and from that point
   `the_acceptance_run` is the port's verdict with no code to write.
