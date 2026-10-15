# 1 — a2 replay harness (FINAL)

Task A2. 2026-10-15.

## What I did

`crates/game-sim/src/replay.rs` (new; the inline stub in `lib.rs` becomes `pub mod replay;`). A fixture
goes in, the simulation runs, and the **first** tick that disagrees comes out as one line:

```text
tick 413: ball.vel[1] -58.94952 (0xc26bcc4f) vs -58.949516 (0xc26bcc4e)
```

Expected (the recording) first, the port's value second, both as decimal *and* pattern, because the
decimal is what a human compares and the pattern is what actually differed. That is the whole brief: a
failure to read in one glance with a tick number to re-run to, never a diff of two files.

**The fixture** (`Fixture`) is: a name, a seed, an optional `start: Option<Sim>` for a run that begins
from a mid-session state, one `Input` per frame, and the two kinds of expectation the task names:

- `hashes: Vec<u64>` — one `Sim::hash` per frame (A1's walk). Cheap, covers every leaf of the state,
  says nothing about *which* leaf moved. This is what the port's own run gives, and — the hash being
  integer-only — what makes a fixture a cross-platform test for free (A3).
- `watches: Vec<Watch>` — a named path through A1's walk (`ball.vel[1]`) with the values it held on the
  ticks it was sampled, sparse or dense. This is what a capture of the *original* gives, and it is the
  kind that names the field in the failure line.

Where a tick has both, the watch is reported: `state hash a vs b` is a true statement that helps nobody.
`verify` dumps the state only on the ticks that have watch samples, so a hash-only fixture allocates
nothing per tick.

**Tick numbering**, since this is where a replay harness is usually wrong: tick 0 is the state the
fixture starts from, before any input; tick *k* is the state after the *k*-th input frame; `hashes[k-1]`
is tick *k*'s hash. Numbering is relative to the fixture, not to `Sim::ticks`, so a fixture that starts
mid-session still reads `tick 1` for its first frame. `Watch::shifted(by)` is the knob for a capture
whose sample point is not where the port's tick ends, and the test
`an_off_by_one_in_alignment_fails_at_the_first_tick` shows what that mistake looks like (it fails at
tick 0/1 and never stops), so it is recognised rather than investigated.

**Five failure reasons, and three of them are the fixture's fault, not the port's** — told apart on
purpose, because a fixture recorded by an older build is not a regression: `Field` (the ordinary one),
`Hash`, `NoSuchField` (a path that is not in the walk), `WrongKind` (a float recorded against an integer
field: the watch is aimed at the wrong field) and `Unreached` (samples past the last input frame, samples
out of tick order, more hashes than frames — every way a fixture can quietly check less than it claims).

**Values compare exactly, with no tolerance, and the module does no float arithmetic** (it denies
`clippy::float_arithmetic` like the rest of the crate). Floats are compared as bit patterns; `0.0` and
`-0.0` are different states. Integer *width* and signedness may differ between a recording and the port
— that is the recorder's choice of type, not the game's — but the value may not. For the rendering-ish
numbers that legitimately need a tolerance there is `./kit record --diff --tol`, which is not a
simulation proof and does not pretend to be.

**Making a fixture.** Two sources, and the difference matters:

- `replay::record(name, seed, &inputs, &paths)` runs the port and writes down what it did: a
  **regression** fixture. Worth having on every `./kit check`; proves nothing about the original. It
  refuses a path that is not in the walk rather than recording a watch with nothing in it.
- `Capture::parse(csv)` reads a `./kit record` CSV of the **original** and `cap.float/uint/int/bool/blob
  (column, path)` turn its columns into watches: an **acceptance** fixture, the only kind that proves the
  port is right. The frame counter maps to ticks relative to the first row, so `every = N` and
  `when`-gated rows come through as sparse samples, and an empty cell (a pointer chain that went null)
  is "not sampled", never zero. Cells stay text until a reader is chosen, because the recorder writes a
  float, a decimal integer, a `fmt = "hex"` column and a `bytes:N` blob into the same CSV and only the
  person who wrote the watch spec knows which is which — guessing would read `3` as an integer and
  silently turn a bit-exact float comparison into nothing.
- `Fixture::to_text` / `Fixture::parse`: a line-based fixture file (`format`/`fixture`/`seed`/`frame`/
  `watch`), round-tripping byte for byte, floats as patterns so the round trip is exact. Unknown
  keywords and malformed numbers are errors **with the line number**. Fixture files hold values read out
  of the game, so they are game data: they live in the git-ignored `context/fixtures/`, never in the
  repository.
- `replay::state_at(&f, tick)` re-runs a fixture to the tick a bare `Hash` divergence was reported on,
  which is how that becomes a field name via A1's `hash::first_difference`.

## What the original does

Nothing — like A1, this task has no counterpart in the game. The harness is the port's own instrument;
there is no address to compare it against, which is why the proofs below are properties and one real
capture rather than an oracle run.

What *is* taken from the original's shape: floats compared as raw patterns (A1's reasoning), vectors
addressed by lane in register order, and one tick = one frame with no `dt`.

## How it was proved

`crates/game-sim/tests/replay.rs`, 21 tests (20 run on every `./kit check`, 1 behind `--ignored`), plus
two doctests (`Capture`, which runs, and `state_at`, compiled only). `./kit check`: 120 tests pass. Seven properties, each of them a way a replay
harness can appear to work while checking nothing:

1. **Agreement passes, disagreement fails at the right tick.** A 600-frame self-recorded fixture
   (3,004 checks: a hash per frame and four fields per tick) replays exactly; a one-bit change to `hashes[412]` is reported at tick 413; three
   changed hashes report the *earliest*, at tick 8.
2. **The failure is one line.** `a_watched_field_fails_as_one_readable_line` asserts the exact string,
   that it is a single line, and that the tick and the path come first.
3. **A named field beats a bare hash** when both cover a tick.
4. **Nothing passes vacuously**: a watch on `player.vel.x` (a path this state does not have yet) is
   `NoSuchField` at the tick it was first sampled, not a skip; an integer against `ball.spin` is
   `WrongKind`; samples past the run, samples out of order and a hash list longer than the run are all
   `Unreached`. Widths and signedness cross, values and float patterns do not (`-0.0` ≠ `0.0`).
   A perturbed start state is caught at tick 0 when the field is watched and at tick 1 by the first
   hash when it is not — including `ball.pos[3]`, a lane the simulation never reads.
5. **The fixture file round-trips** to the same bytes, every input, hash and float pattern intact, and
   still replays; nine malformed files each report the right line and reason. The hand-written fixture
   test doubles as the argument for patterns over decimals: the port is at `2.6999998 (0x402ccccc)` and
   `2.7` reads as `0x402ccccd`, one bit away.
6. **A `./kit record` CSV reads as watches** — including sparse `every = 4` rows, an empty cell, and a
   `fmt = "hex"` column — and a single wrong cell fails at that row's tick naming the field. Five
   malformed CSVs report the right line.
7. **Off-by-one alignment is recognisable** and `Watch::shifted` undoes it.

**Against the real recorder, not against its documentation.** I started PCSX2 copy 4, found the frame
counter with `./kit probe vsync` (0x47a9d8; saved to this worktree's `kit.toml`) and recorded 120 samples
of it plus two neighbouring words with a throwaway spec — `./kit record … --out
context/capture/boot-counter.csv`, 120 frames, 0 torn reads. `every_capture_in_context_parses` reads
every `*.csv` in `context/capture/`, checks the frame counter rises and asserts **every column is
readable by one of the typed readers**; on that file it reports `vsync:uint near_f32:float
near_flags:uint`. So the reader agrees with what the tool actually writes (`0.0`, `0x20`, a `frame`
column first), which is the thing I could not have got right from the format document alone. The CSV is
game data and stays in the git-ignored `context/`; the test prints what it found and passes when the
directory is empty, which is the only honest thing a test can do about a file it may not ship.

`replaying_every_fixture_in_context` does the same for `context/fixtures/*.fixture`: when a recording of
real play lands there (S8), that test is the port's verdict, with the slow ones in
`context/fixtures/slow/` behind `--ignored`.

## Not verified / not 1:1

- **No watch value has ever been compared against the original's own numbers.** The capture I recorded
  proves the *format* path (columns → watches → ticks), not the simulation: the port's state is still
  A1's example ball, so there is no ported field for a real column to be compared against. The
  acceptance comparison is S8's job, and `replaying_every_fixture_in_context` is the hook it drops into.
  Until then every passing replay test in this file is a regression test of the port against itself.
- **The capture I recorded is of the boot sequence, not of play.** This copy of the emulator only had
  save-state slot 9 (scratch) in it, and `kit.toml`'s `[states] 5 = "a self-running demo/attract loop"`
  slot was not present, so the capture is 120 frames of the intro: a rising frame counter, a constant
  float and a constant flag word. Enough to pin the CSV reader, not enough to be interesting. It also
  shows `120 counter gap(s)` — the recorder was sampling every other frame by request (`every = 2`), and
  I did not chase whether the gap count is confused by that, since the rows themselves are sound.
- **`Fixture::start` cannot be written to a fixture file.** A fixture that starts from a mid-session
  state has to be built in code, because `Sim` has no reader (only A1's walk, which is write-only). When
  something can build a `Sim` from the original's RAM image, the format needs a `state` line and a
  version bump; `FORMAT_VERSION` exists for that.
- **The CSV reader cannot know a column is unusable.** A `scale` or `fmt` key in the watch spec makes a
  column non-bit-exact, and the scaling happened before the CSV was written, so choosing honest columns
  is still on the person writing the spec. Documented on `Capture`, not enforceable here.
- **NaN payloads from a capture do not survive.** A float cell is parsed with the standard library's
  parser, so a recorded `nan` comes back as the host's quiet NaN pattern and not the game's. A NaN that
  matters must be written as a pattern (`0x7fc00001`) in a fixture file, which the format supports.
- **Nothing here is proved on a second architecture.** The hash is integer-only and the harness does no
  arithmetic at all, so the fixtures should replay identically on aarch64 and wasm32 — but that is A3's
  test, not a claim this task checked.
- **`Input` is still the example's `buttons`/`stick`.** The fixture file's `frame` line is exactly those
  three numbers, so it will change shape when I1/I3 give the port the game's real pad layout. The
  parser rejects what it does not understand, so an old fixture will fail loudly rather than quietly.

## Next

Nothing outstanding for A2. The two hooks the rest of the plan uses: drop a `*.fixture` into
`context/fixtures/` (S8's acceptance run, I3's recorded pad sessions), and point watches at the paths a
ported subsystem adds to A1's walk.
