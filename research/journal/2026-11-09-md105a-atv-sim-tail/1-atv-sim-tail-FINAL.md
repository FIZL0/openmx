# 1 — atv sim tail (FINAL)

Task MD105a. 2026-11-09. Slot s3.

## What I did

- New `crates/game-sim/tests/atv_quad/sim_tail.rs`, mounted from `sim_frame.rs`. Its `port_tail` stands in for the
  original's tail in `frame_in_pieces` when the new `tail` flag is set. It runs:
  - the walk, `tail::walk`. A group whose step is the walk recurses through the port. The quad's listener
    (quad `+0xc`) runs `port_update`. Any other listener runs the original's step.
  - the quad's update, `quad::update`, on the quad in RAM (`UpdateInRam`). The rider hold and the rumble's on
    come from the original's lookups, as the update reads them before anything else. Under the update:
    - the bike's vehicle update is `tail::inner` on the quad (`TailInRam`, the bike's `tail::read` /
      `tail::words` map; the quad is the same base class);
    - the hazard clock is `tail::stuck`.
  - under the inner update:
    - the wreck test is `wreck::wreck_test` (`WreckInRam`, F31b4b's glue for the quad), and under it the entry,
      unwreck and part toggle (`wreck::enter`, `unwreck`, `part`);
    - the ray swaps are `quad::swap_rays`;
    - the put-back is `quad::put_back` over the new `wreck::put_back_base`, with the wheels' resets
      `wreck::wheel_reset`;
    - the prepare is `quad::animate`.

    Every other call is the original's in the interpreter. Each port asserts that the quad's vt slot holds the
    function it stands for.
- `crates/game-sim/src/quad.rs`: the `Moto` trait gets `tail_out` / `tail_in` (default no-ops), and
  `update` makes every call through `tail_call`, which hands its `Tail` out before the call and takes it
  back after. The original reads each word where it uses it: a put-back inside the bike's update sets the
  request word, and the update's request clear has to see it. The `body_in` / `body_out` hooks (MD75e)
  were the precedent. `one_pair` reads the stunt and clock before its `StartStunt`, so `t` is not borrowed
  across the call. The order of operations is unchanged.
- `crates/game-sim/src/handling/wreck.rs`: `put_back_base` is split out of `put_back` (the vehicle's
  put-back after the quad's override has run its own head). `put_back` calls it, so its behaviour is
  unchanged.
- `animate.rs`: `read_anim` and `Table` are now `pub(super)` so the glue can share them.
- New test `the_quads_tail_is_the_ports_through_the_palm_strike_wreck_and_put_back`: the palm ride with the
  port's record and the port's tail, against the original's whole frame.
- Addresses: research/oracle.txt `# MD105a`.

## What the original does

- The tail (`./kit fn 0x00128af0`) walks the sim's listener tree. The quad's listener steps through a thunk
  (`./kit dis 0x004bf9a0 -p atv`: `j 0x4ba9f0`) to the quad's update, once a frame.
- The update (`./kit fn 0x004ba9f0`): with a vehicle state, it looks up the rider (a name lookup, then the
  state's vt `+0xac`), and a held rider skips everything. Otherwise it runs the bike's vehicle update
  (`0x004e1b90`, the same function as F31b4a's), then the quad's own tail (MD64a's map).
- On the palm ride, the inner update runs fade, wreck test, lean, prepare (animate), ride, sync,
  snapshot, network and parts every frame, and the hazard clock every frame. The wreck test enters
  the wreck on frames 96 and 256 (each entry swaps to the wreck rays and toggles the part). The put-back
  timer runs the unwreck on frame 164: swap back, then the quad's put-back with 4 wheel resets and the
  vehicle's base put-back. The rumble fires on 4 frames.

## How it was proved

`./kit check -- -p game-sim --test atv_quad -- the_quads_tail_is_the_ports` passes. This is the palm ride
(300 frames) with the record's two virtuals (MD105) and the whole tail in Rust. Every RAM word outside
the interpreter's stack and the ground ring is equal to the original's whole frame after every frame, and
`check_palm` holds: first contact 89, swaps [96, 164, 256], wrecks 0→1→2, put back at the spawn.

- Temporary counters, since removed, confirmed which ports ran over the ride:
  - every frame (300 times): update, inner, stuck, wreck test, animate;
  - the wreck entry twice, the part toggle twice, the unwreck once;
  - the swap 3 times;
  - the put-back once, with its base once and 4 wheel resets;
  - the rumble on 4 frames.
- Mutations, temporary:
  - Flipping the low bit of the put-back's settle word goes red on frame 164 in that one word
    (quad `+0x141c`).
  - Making `tail_in` keep the stale `Tail` goes red on frame 164 at quad `+0x320` (request_was). The hook
    is needed: the put-back inside the bike's update sets the request, and the update's clear must see it.
- `./kit check -- -p game-sim --test atv_quad`: 70 passed. That includes `update`'s and the put-back's own
  oracles (MD64a, MD65), because `quad.rs`'s `update` and `wreck.rs` changed shape. The countdown fix (`p`
  taken back after the calls) landed after this run, and `./kit done`'s `./kit check --all` covers it.

## Not verified / not 1:1

- **Stale `f12`**: as in F31b4a, the calls the original makes with a stale `f12` get dt (or 150 / 1.0
  where the original loads them) from the glue. Everything matches bit for bit on this ride, so no callee
  that ran here reads it.
- **Some port writes land only at the end.** The put-back's `PutBack` words (settle, request, player
  respawn, the flag byte after the base) and the swap's ray words are written when the port returns, not
  where the original stores them. The animation driver's five words are written the same way. Only the
  `Quad` words those ports keep are written before each call. A callee in between that read one of those
  words would see the old value. On this ride none does: RAM is equal after every frame. But the order
  is not checked inside a frame.
- **Only one ride and only some paths.** The palm ride takes no tricks (wheelie/stoppie never start), no
  countdown or run-over, no fade over, no network interpolation (vehicle state not 3/4), and has no player
  respawn point. Those paths of `quad::update` / `quad::animate` are covered only by their own oracles
  (MD64a, MD65c), not by this chain.
- **The countdown's lists are counted before the calls.** `update` walks `p.listeners` and `race.watchers`
  as counted when the update began, while the original follows the links as it calls. A listener that
  cut or grew its list mid-countdown would differ. The ride never runs the countdown. After the countdown
  calls, `update` takes the player back from `t` (`tail_in`'s re-read), so its final `t.player = Some(p)`
  no longer puts back the copy from before the calls.
- **Outside is still the interpreter's**: the bike vehicle's own slots (lean, ride, sync, snapshot, fade,
  network, parts), the ground reset, the clips, the node pose, the scene calls, and every other listener's
  step.
- **Left out of the compare**: the stack, the ground ring, and the two stack-pointer globals, as in MD105.
- **The interpreter is the oracle, not PCSX2**, as in MD105.

## Next

MD105a is done. Next is MD105b (the record's world search and node calls in Rust, its contacts fed to `quad::frame`). Then MD105's
all-Rust chain: `quad::frame` plus this tail, word for word against the whole frame.
