# 1 — a3 platforms (FINAL)

Task A3. 2026-10-15.

## What I did

`crates/game-sim/tests/platforms.rs` (new, and the only file of the port this task touched) plus a runner
for it, `crates/game-sim/tests/cross/platforms.sh` and `tests/cross/wasi-run.js`. A1 argued the hash was
platform-independent; this measures it.

**The method, in one sentence:** every expected value in `platforms.rs` is a *literal*, and that same
test binary is compiled for another architecture and run there — so two machines that both pass it agree
with each other, instead of each confirming its own answer.

What is pinned, and why that thing:

- **The canonical fixture.** Seed `0x4d58_554e`, 256 input frames built from an integer LCG (no file, no
  game data, so it is the same run on every machine and cannot be mistaken for a recording of the game).
  Pinned: `Sim::hash` at every 16th tick, an FNV-1a digest over all 256 hashes (so the ticks between the
  samples are covered too), the full `hash::Dump` text of the last state, and the fact that all 256
  hashes are distinct — a stuck hash would pass every comparison above it.
- **The same run through the replay harness**, as A2's `Fixture`: `ball.pos[1]` and `ball.spin` as
  pinned bit patterns on those 16 ticks, verified with `replay::verify`. Plus the negative: one pinned
  bit flipped must fail at that tick naming that field, so a pass is a comparison and not an empty list.
- **A fixture file's bytes.** `Fixture::to_text` of that fixture, FNV-1a'd to one constant: a recording
  written on one machine is the same bytes on another, or it cannot be shared. Round-trips and replays.
- **The byte encoding of every leaf kind.** A test-local `Every` state with one of each leaf the walker
  can emit, including the two lengths that could pick up the host's pointer width (a blob and a list),
  `-0.0`, and a NaN pattern in a VU lane. Its hash is checked against a byte stream built by hand in the
  test and folded with an FNV-1a written out there — so the *encoding* is pinned by something other than
  the code that produces it — and also against its own pinned constant and a pinned `path: kind width`
  listing, so a width that changes is read as a width and not as an unexplained hash.
  (The NaN lane taught me something worth writing down: `Vf::from_bits` puts every lane through VU0's
  operand rule, which has no NaN and clamps to MAX, so `0x7fc0_0001` is hashed as `0x7f7f_ffff`. That is
  the state the VU would actually hold, it is integer-only, and it is identical everywhere.)
- **The game's libm, bit for bit.** `sinf cosf tanf asinf acosf atanf atan2f expf logf powf sqrtf` over
  103 inputs chosen per function: the exact zeros and ones where fdlibm's special cases live, the branch
  boundaries of `atanf`'s reduction (7/16, 1.1875, 2.4375), the top of the exact argument reduction
  (200 ≈ 2⁷·π/2), and the magnitudes where the EE flushes to zero or saturates (`expf(-88)` is `0`,
  `logf(1.1754944e-38)` is `0xc2aeac4f`).

And two guards, because a value that can only be *detected* after the fact is worse than one that cannot
drift:

- `the_simulations_sources_call_no_host_transcendental` reads every `.rs` under `game-sim/src`,
  `ps2-float/src` and `ps2-oracle/src` (15 files), strips comments and string literals, and rejects
  `.sin( .cos( .powf( .ln( .mul_add(` and 20 more as method calls and as `f32::`/`f64::` paths. This is
  the gap `clippy::float_arithmetic` cannot close: that lint rejects *operators*, and the host's libm
  arrives as a method call. `sqrt`, `abs`, `max`, `min`, `clamp` and `lerp` are deliberately not on the
  list — they are also `F`'s own method names, which are the correct thing to call. `mul_add` is on it:
  it is a true FMA, which the EE does not have. The stripper has its own test (prose mentioning
  `f32::sin` must not read as a call; a lifetime `'a` must not read as a char literal; line numbers must
  still point at the right line).
- `the_simulation_has_no_dependency_that_could_reach_a_host_libm` holds those three crates to path
  dependencies on each other. An added registry dependency is how a host libm shim arrives.

`ps2-oracle` is in the scan list on purpose: its interpreter decides what "the original did" in every
oracle proof, so if *it* reached for the host's libm, the proofs would be as platform-dependent as a bad
port.

**The runner.** `tests/cross/platforms.sh` builds the simulation's tests for
`aarch64-unknown-linux-musl` and `wasm32-wasip1` and runs them under `qemu-aarch64-static` and node's
own WASI (`tests/cross/wasi-run.js`), one command, nothing installed into the system and nothing new in
the repository. It takes the target's standard library from this machine's toolchain when it has it
(a rustup user needs no download) and otherwise fetches the matching upstream release into
`$OPENMX_CROSS_DIR` (default `$TMPDIR/openmx-cross`) — which is also why it brings its own `rustc` and
`cargo`: a distribution's rustc cannot load an upstream `std`, because the version string is part of the
crate hash (Arch's `1.98.1 (Arch Linux rust 1:1.98.1-1)` rejected the upstream `compiler_builtins`
outright). musl links statically through `rust-lld` and the target's own `libc.a`, so no cross-gcc and no
cross-glibc are needed. A target whose runner is missing is reported as `skipped`, never as a pass, and
a run where nothing could be executed exits non-zero.

`the_pinned_vectors_hold_on_another_architecture` drives that script from a `#[ignore]`d test, so the
proof is one `cargo test --ignored` away but never part of `./kit check` (it builds the world twice).

## What the original does

Nothing — like A1 and A2, this task has no counterpart in the game. The hash and the fixtures are the
port's own instruments; there is no address to compare them against, which is why the proofs below are
properties, pinned vectors and two architectures rather than an oracle run.

What *is* about the original: the libm vectors are the values `ps2_float::libm` produces, which are
fdlibm's algorithms over the EE's chopping arithmetic. They are pinned here as the port's own
regression, **not** as a claim about this game's `libm.a` — that comparison is `./kit oracle` against the
game's own code, and `ps2-float`'s docs say to make it before trusting a transcendental.

## How it was proved

`crates/game-sim/tests/platforms.rs`: 11 tests on every `./kit check` (`TESTS PASSED: 139 tests`, up
from 128), 2 behind `--ignored` (the cross run and the vector generator). `cargo clippy -p game-sim
--all-targets` reports nothing for the new file; the remaining warnings in the crate are pre-existing in
`archive.rs` and `replay.rs`.

**The measurement, which is the point of the task.** `crates/game-sim/tests/cross/platforms.sh`:

```text
aarch64-unknown-linux-musl ok
wasm32-wasip1 ok
2 target(s), host is x86_64-unknown-linux-gnu
```

84 tests on each of those two architectures, the same 84 that pass on x86-64 and against the same
literals: `ps2-float`'s `fpu` (20), `vu` (6) and `libm` (7), and `game-sim`'s `hash` (16), `platforms`
(11), `replay` (20) and `rng` (4). So A1's hundred-tick constant `0x7bbf_5ca2_d10e_b072`, the canonical
fixture's 256 hashes, the fixture file's byte digest and every pinned transcendental are now *measured*
to be the same on three architectures (x86-64/glibc, aarch64/musl, wasm32/WASI), two of which have a
different register file and one of which has a 32-bit `usize`.

**A negative control, because a test that has never failed proves nothing.** I changed `hash::Walk::list`
to hash a list's length at `size_of::<usize>()` bytes instead of a fixed 8 — the most plausible real form
of this bug. x86-64: 11 tests pass, nothing noticed. wasm32: `platforms` fails. Then reverted (the
diff is empty; `hash.rs` is byte-identical to what A1 left). So the method detects the class of bug it
is for, and it detects it on the architecture where it matters.

**The host's libm, measured rather than asserted.** `the_games_libm_is_not_the_hosts` prints how many of
the pinned inputs the two disagree on: on glibc/x86-64 **48 of 103** (`sinf` 6/17, `tanf` 9/17, `asinf`
8/10, `logf` 5/8 …). It also passes on musl/aarch64 and on wasm32, i.e. all three hosts' libms differ
from the game's — which is the whole reason rule 2 of `game-sim`'s docs exists, now with a number
against it.

## Not verified / not 1:1

- **Nothing here is compared against the original, and nothing can be.** Same as A1 and A2: the game has
  no state hash and no fixture format. What is proved is that the port's instruments mean the same thing
  on every machine. Whether they measure the *right* thing is S8's job.
- **The libm vectors are the port's values, not the game's.** Pinned so they cannot drift and so they
  are identical across architectures; *not* proved against this game's own `libm.a`. Every one of them
  could be a uniform one ulp away from what MX Unleashed computes and these tests would still pass.
  `ps2-float/src/libm.rs`'s own header says the same thing and names the way to settle it
  (`cargo test -p ps2-oracle libm`, 10k inputs against the game's code); that run belongs to whichever
  task first ports a function that calls a transcendental. Until then the port has no transcendental
  call sites at all, so nothing in the simulation depends on the question yet.
- **Two of the three architectures were run under emulation, not on real hardware.** qemu-aarch64 and
  node's WASI are faithful about integer and IEEE arithmetic (and the hash is integer-only, which is the
  argument), but they are not an arm64 laptop or a browser. What a real machine could still add:
  a different libm behind `f32::sin` (which only `the_games_libm_is_not_the_hosts`'s printed count
  depends on, never an assertion), and compiler autovectorisation differences, which Rust may not apply
  to float arithmetic anyway. If an arm64 machine ever runs `./kit check` directly, that closes it.
- **wasm32 was proved under WASI, not in a browser.** `wasm32-wasip1` and `wasm32-unknown-unknown` share
  the same codegen and float semantics, so the arithmetic result carries over; a browser build would
  additionally need a test harness that reports back to the host (`wasm-bindgen-test`), which is not
  worth adding while PLAN.md says nothing about a browser port — the task line itself scopes wasm32 to
  "if this port will run in a browser". The 32-bit `usize` is the part that mattered, and that is
  measured.
- **The default `OPENMX_CROSS_DIR` is `$TMPDIR/openmx-cross`, and a first run there downloads ~250 MB**
  of upstream toolchain (once, matching `rustc -V`). A machine with the rustup targets installed skips
  it entirely; a machine with no network says so and skips the target rather than failing it. Nobody
  who runs `./kit check` is affected — the cross test is `#[ignore]`d.
- **The source scan is textual.** It strips comments and string literals with a hand-written scanner
  (tested), but it cannot see through a macro that builds a call from fragments, a trait method named
  `sin` on a non-float type (a false positive, which prints the file and line), or a host call reached
  through a dependency of a dependency — the last of which is what the path-dependencies-only test is
  for. It also scans `src/` only, not `tests/`: this test file itself calls `f32::sin` deliberately, to
  measure the distance to it.
- **`./kit check` does not run the cross targets**, by choice (two more builds of the workspace). The
  claim in this entry is true of the commit it is written on and is re-checkable in one command; if CI
  ever exists, `tests/cross/platforms.sh` is the line to add to it.
- **Not in PLAN.md's code map.** `tests/platforms.rs` and `tests/cross/` are documented in their own
  headers and in this entry, but I did not add a line to PLAN.md: other agents are editing it in
  parallel and A2 set the precedent of leaving the map to a quieter moment.

## Next

Nothing for this task. Two things it hands to later ones:

- When the state type grows, A1's constant and this file's vectors move together. Regenerate with
  `cargo test -p game-sim --test platforms -- --ignored --nocapture print_the_vectors` and paste the
  generated block in; the module header says what to check before believing the change.
- When the first ported function calls a transcendental, run the oracle against the game's own `libm.a`
  (`cargo test -p ps2-oracle libm`) and fix `ps2-float/src/libm.rs` to match it. The vectors here will
  move; that is the one change to them that is a *fix* and not a regression.
