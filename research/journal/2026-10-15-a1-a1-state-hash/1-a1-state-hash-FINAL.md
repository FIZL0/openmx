# 1 — a1 state hash (FINAL)

Task A1. 2026-10-15.

## What I did

`crates/game-sim/src/hash.rs`: one walk over the simulation state, read by two sinks.

A state type describes itself once:

```rust
impl hash::State for Sim {
    fn walk(&self, w: &mut Walk<'_>) {
        w.u64("ticks", self.ticks);
        w.u32("rng", self.rng);
        w.field("ball", &self.ball);
    }
}
```

and that single description feeds both modes, so the thing that *detects* a divergence and the thing
that *explains* it cannot drift apart:

- **hash mode** — `hash::of(&sim) -> u64`. FNV-1a over little-endian bytes, allocation-free apart from
  one small path vector, cheap enough to call every tick. `hash::trace(&mut sim, &inputs)` is the
  per-tick list a fixture holds (A2's raw material).
- **dump mode** — `hash::Dump::of(&sim)` is every leaf with its path in walk order
  (`ball.vel[1] = 3 (0x40400000)`), and `hash::first_difference(&a, &b)` is the one-line answer to
  "which field first differed":

  ```text
  ball.vel[1] 2.8499999 (0x40366666) vs 2.85 (0x40366667)
  ```

  with `Diff::index()` (position in walk order) and `Diff::Shape` for the case where the two states are
  not the same shape at all — an old recorded dump against a newer build — so that is reported as a
  shape change instead of being blamed on a field.

The walker's API is what the first real subsystem will need: `u8/u16/u32/u64`, `i8/i16/i32/i64`, `bool`,
`f` (an EE FPU scalar), `vf` (a VU0 vector, as four lanes `name[0..3]`, including a `w` the simulation
ignores), `blob` (bytes, hashed in full, summarised in a dump by length and digest so a 2 KiB buffer
does not make a dump unreadable), `field`/`group` for nesting and `list` for a slice — whose length is
hashed *before* the entries, so a list that grew cannot hash the same as one whose new entries are zero.

`State` is implemented here for `Sim`, `physics::Ball`, `rng::Rng` (its 64-bit state word, not the last
number it returned) and `Input` (not simulation state, but hashed so a fixture can prove it is feeding
the frames it recorded rather than frames shifted by one).

Two existing functions now delegate to the walk instead of keeping a second copy of the same field list:
`Sim::hash` is `hash::of(self)` and `physics::Ball::hash` is `hash::of_with_seed(self, seed)`. Both
produce byte-for-byte the same stream as before, so the change is invisible to any hash.

**One fix in `lib.rs` worth noticing:** `state_hash`'s multiplier was `0x1000_0000_01b3`, one hex digit
longer than FNV-64's prime (`0x100000001b3`), so the function was not the FNV-1a its doc comment claimed
— it was a structurally identical fold with a 2⁴⁴ + 0x1b3 multiplier. Fixed to
`0x0000_0100_0000_01b3` (written full-width so a missing digit cannot hide again). The reason this is
worth doing rather than documenting: a standard algorithm means a capture tool or a script *outside*
this crate can reproduce a state hash from any FNV-1a implementation, and the published vectors become a
proof. Done now because it is free now — no fixture in the project records a hash yet. It moves the
only committed hash constant, `Sim`'s hundred-tick value, from `0x481b_a2a2_d10e_b072` to
`0x7bbf_5ca2_d10e_b072` (both test sites updated; the low 32 bits are unchanged, as the changed bit of
the multiplier predicts).

## What the original does

Nothing — this task has no counterpart in the game. The state hash is the port's own instrument: the
original PS2 code never hashed its state, and there is no address to compare against. That is why the
proofs below are properties rather than an oracle run, and it is the one task in the plan where
"compared against the original" is not the standard to hold it to.

What *is* borrowed from the original's shape: floats are hashed as raw patterns (the EE FPU has no NaN
and flushes denormals, but VU lanes carry whatever bits were loaded, so the pattern is the state), and
vectors are walked in register order `x y z w`.

## How it was proved

`crates/game-sim/tests/hash.rs`, 16 tests, plus a doctest on the module. Five properties:

1. **A known algorithm over a known byte stream.** `the_fold_is_fnv1a` checks `state_hash` against the
   published FNV-1a 64 vectors (`""` → `0xcbf29ce484222325`, `"a"` → `0xaf63dc4c8601ec8c`, `"foobar"` →
   `0x85944171f73967e8`) and that folding in two pieces equals folding in one.
   `the_hash_is_that_fold_over_the_states_bytes_in_order` builds the 48-byte stream of a 7-tick state by
   hand — ticks (8 LE bytes), the RNG word (4), the ball's two vectors as four lane patterns each, the
   spin — and folds it with an independent FNV-1a written out in the test file. So the *encoding* of the
   state is pinned by something other than the code that produces it.
2. **Platform independence, as far as one machine can show it.** Integer-only by construction, and
   `a_hundred_ticks_is_a_known_constant` pins `0x7bbf_5ca2_d10e_b072` as a literal, which is the value
   A3 will compare on a second architecture. `a_trace_is_one_hash_per_tick_and_repeats_exactly` runs
   120 ticks twice (same hashes), from a different seed (different from tick 1), and checks the 120
   hashes are all distinct, so a divergence cannot hide behind a stuck value.
3. **Sensitivity.** `every_leaf_of_the_state_reaches_the_hash` flips one bit of each leaf of a live
   state in turn — the loop is over the dump, so it grows with the state type — and requires the hash to
   move every time. `negative_zero_is_not_zero_and_a_nan_payload_survives` covers `0.0` vs `-0.0`, two
   NaN payloads, and an integer's width being part of the hash.
4. **The dump agrees with the hash.** `a_dump_hashes_to_the_same_value_as_the_state` checks
   `Dump::hash() == hash::of()` on all 30 ticks of a run; `a_dump_names_every_field_in_walk_order`
   asserts the exact path list and the rendered text.
5. **The first difference is the first one.** `the_first_differing_field_is_the_one_reported` plants two
   divergences (a one-mantissa-bit drift in `ball.vel[1]` and one in `ball.spin`) and requires the
   earlier field, with both bit patterns and walk index 7;
   `an_earlier_field_is_reported_before_a_later_one` checks it from either side;
   `a_shape_change_is_reported_as_a_shape_change` covers a shortened and a renamed dump.

Also `groups_lists_and_blobs_walk_the_way_they_are_documented` (a test-local `Race`/`Rider` state, since
the simulation has no list yet: paths, the blob digest, and a list whose length is hashed with it),
`a_path_reads_like_rust`, `input_frames_hash_distinctly`, `the_rngs_state_word_is_what_is_hashed`.

`./kit check`: 98 tests pass. `cargo clippy -p game-sim --all-targets` is clean for both new files
(the module denies `clippy::float_arithmetic`; the remaining warnings in the crate are pre-existing, in
`archive.rs`).

## Not verified / not 1:1

- **Nothing here is compared against the original, and nothing can be.** The game has no state hash;
  this is the port's instrument. What is proved is internal consistency plus one external anchor (the
  FNV-1a vectors). The hash becomes meaningful as a 1:1 claim only once A2's fixtures carry hashes
  recorded from real play.
- **Platform independence is argued, not measured.** Only x86-64 was run. The construction is
  integer-only with explicit little-endian bytes, which is the whole of the argument; the measurement is
  task A3 (x86-64 vs aarch64, and wasm32 if the port goes to a browser), and the literal constant in
  `a_hundred_ticks_is_a_known_constant` is what it should compare.
- **A field in `Sim` that is missing from its walk is still only caught indirectly.** Rust has no
  reflection and this crate forbids `unsafe`, so there is no way to assert "every byte of the struct was
  visited". Two tripwires stand in for it: the exact path list, and `size_of::<Sim>() == 48` with a
  comment saying what to do when it fires. The size one can also fire innocently (a compiler that lays
  the struct out differently, or a change to `F`/`Vf`), in which case the fix is to check the walk and
  update the number — it is a prompt, not a proof.
- **`state_hash` still treats a seed of `0` as "start from the offset basis".** So if a running hash
  lands exactly on zero mid-walk it restarts instead of continuing with zero. Probability 2⁻⁶⁴ per leaf,
  and it applies identically to both sides of any comparison, so it cannot produce a false *match* other
  than by collision. Left alone because fixing it would move the committed constant a second time for no
  behavioural gain; documented in the module.
- **FNV-1a is not a cryptographic hash**, and two states 2⁻⁶⁴ apart can collide. Accepted deliberately:
  the dump mode is the answer to "did the hash lie", and a collision costs one re-run rather than a
  wrong conclusion. Nothing in the project should treat a matching hash as proof of a matching state
  without a dump behind it.
- **`Walk::blob` summarises in a dump.** A blob's dump line names its length and its own digest, not its
  bytes, so a difference *inside* a blob is reported as "this blob differs" rather than "byte 37
  differs". If a subsystem ends up with a large byte buffer whose contents need field-level diffing, it
  should walk the bytes as leaves instead. Not exercised by the simulation yet (no blob in `Sim`).
- **Performance is not measured.** The hash allocates one small `Vec` for the path per walk and is
  plainly cheap, but no benchmark exists; if a per-tick hash ever shows up in a profile, the path vector
  can be hoisted out of `Walk::new` by the caller.

## Next

A2 (the replay harness) is the consumer: `hash::trace` gives it the per-tick list, and
`hash::first_difference` is the one-line failure its brief asks for — compare hashes per tick, then dump
the differing tick's two states. A3 is the platform proof, and the literal constant is what it compares.
