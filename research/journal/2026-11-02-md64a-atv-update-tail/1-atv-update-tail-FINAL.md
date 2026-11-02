# 1 — atv update tail (FINAL)

Task MD64a. 2026-11-02.

## What I did

- Read the PC's `pc_quad_update` first (`./kit fn 0x005d8350 -o pc`), then SLUS-21104's
  `atv_quad_update` (`./kit fn atv_quad_update -o atv`, and `./kit dis 0x004ba9f0 --len 2408 -p atv` for
  the float order, the `max.s`, the `cvt.w.s` and the clamps, which the decompile garbles). The branches
  are below. The PC has the same clauses in the same order. Its rumble strength goes through x87 `ftol`,
  and Ghidra loses the "slow" flag there (`unaff_ESI`). The PS2 is the reference.
- `crates/game-sim/src/quad.rs`:
  - `update(t: &mut Tail, q, body, dt, m: &mut impl Moto)`, and its helpers `one_pair` and
    `rumble_strength`.
  - New types: `Tail` (what the update reads and keeps beyond `Quad`), `Surface` (one entry of the
    hazard table), `Player` and `Race` (what it reads of the two objects at +0x680 and +0x684).
  - Nine new `Call`s: BikeVehicleUpdate, HazardSurface, Rumble, StartStunt, EndStunt, CountdownOut,
    CountdownListener, CountdownWatcher and Post.
  - The tail's state lives in `Tail`, not in `Quad`/`Wheel`, so MD64's generator and literals are
    unchanged. Its per-wheel fields (surface, sunk, squeeze) are arrays in LF/RF/LR/RR order.
- `crates/game-sim/tests/atv_quad.rs`:
  - A tenth region, `player` (0x1e0), because the update writes the player's countdown.
  - `poke_tail`, `Gen::tail`, `tail_paths`, and the test. MD64's nine tests still pass with the bigger
    snapshot.
- research/oracle.txt: an `# MD64a` field block (offsets as comments only) and three new rows:
  - atv_vehicle_hazard_clock (quad vt +0x45c);
  - atv_name_bit;
  - atv_rumble_on.

  The bike update, stunt start/end and the post were already named (MD37, MD40, MD63).
- Nothing in opentennis matches this game-specific function, so I took nothing from it.

## What the original does

Recipe: `./kit fn atv_quad_update -o atv`. `dt` is in f12, the quad in a0.

1. **No vehicle state** (+0xbc4 null): it does the bike's vehicle update and returns.
2. **The rider is looked up** with the state's vt +0xac and the "MotocrossRider" name bit. If the rider's
   +0x378 → +0x14 is set, it returns 1 at once and does not run the bike's update.
3. **Two reads before the bike's update:**
   - slow = contact speed (+0xc38) < the stunt speed (+0x174c);
   - the scene node's address.
4. **The bike's vehicle update.**
5. **Hazard surfaces.** Each wheel has a surface byte (+0x110). It indexes a table at +0xdf4, 0xc per
   surface: a hazard word, then a time.
   - +0xc20 = any wheel's hazard word ≠ 0 (stored 0/1).
   - sunk = +0xc20 && any wheel's +0xd0.
   - Then vt +0x45c(quad, dt, time, sunk), where time = max.s over the four times (RR/LR, then RF, then LF).
     The callee is the vehicle's hazard clock: −1 means at once, a positive time counts down, then
     vt +0x3a8.
6. **The request word.** If +0x31c and +0x320 are both set, +0x31c = 0. Then +0x320 = +0x31c.
7. **Rumble.** It only runs if speed_len (+0x32c) > 10, the state's +0x28 == 1 and +0xc80 is clear.
   - s = max.s of the four wheels' +0xf0.
   - s > 0.95: hard, strength = cvt.w(255 × (0.45 × s)), raised to 95 if it is under 96.
   - s > 0.73: soft, strength = cvt.w(255 × (0.5 × s)), raised to 40 if it is under 41.
   - Otherwise the strength is 0.
   - It rumbles when strength > 0, the state's pad (+0x5c) is set and rumble is on for it
     (`atv_rumble_on`). The call is pad vt +0x4c(pad, soft, strength, 0.2).
8. **One-pair clocks.**
   - Wrecked (+0xc80): both clocks are zeroed.
   - Rear pair alone, not slow: the wheelie clock (+0x1744) += dt. Past 1:
     - if its time (+0x704) ≤ 0, start the stunt (vt +0x4b8, id +0x13c4, 1);
     - time += dt, keeping the best (+0x714);
     - with player and race both set, distance (+0x70c) += the player's +0x18, keeping the best (+0x71c).
   - Front pair alone, not slow: the same for the stoppie (+0x1748, +0x708, +0x13c8, +0x718, +0x710,
     +0x720).
   - Otherwise, if all four are down, or slow on the ground, or airborne with hang time > 0.1:
     - end the stunt (vt +0x4ac, −1) when either time is > 0;
     - zero both clocks, both times and both distances.
   - In any other case nothing changes.
9. **The player (+0x680), when set:**
   - The ride clock: +0x6f8 += dt, unless flag 0x200 is set.
   - The top speed: +0x6fc = the max of itself and the player's +0x108.
   - The distance forward: +0x724 = (0 + d) + f·dt (adda/madd), where f is never below 0.
     - With the front pair down, f is the contact speed.
     - Otherwise f is the body velocity · the node's +0x90 row with y zeroed, normalised by VU rsqrt.
       The +0x80 row is used instead when +0x94 is ≤ −1 or ≥ 1.
   - The word counter: +0x6c8 += 1 when +0x6cc was set and the player's +0x40 is now 0. Then
     +0x6cc = +0x40.
   - The countdown. If the player's +0xc4 > 0, the race (+0x684) is set and its +0x29c ≤ 0:
     - +0xc4 -= dt;
     - at ≤ 0, it calls the +0x680 object's vt +0x68, then each player listener's, then each race
       watcher's vt +0x7c.
     - The watcher list's first link is at +4 and every later one at +0x18.
10. **Last:**
    - In mode 1, the second countdown: if +0x175c is set, it goes off once +0x1760 < 0, and otherwise
      +0x1760 -= dt.
    - +0x1750 += dt.
    - It posts (8, 4, +0xd20) and returns 1.

## How it was proved

`the_quads_update_is_the_originals` in `crates/game-sim/tests/atv_quad.rs` runs 10,000 generated states.
Each one runs the original in the interpreter off `atv-r-spawn.ee.bin`. Every callee is hooked:
- logged as a `Call`: the bike update, the hazard clock, stunt start and end, rumble, the countdown and its
  listeners and watchers, and the post;
- answered without logging: the name lookup, the rider lookup and the rumble query.

The hooks also check each callee's arguments. The test then compares every word of the quad, body, world,
state, record, the four wheels and the player against the port's, and compares the call lists. The
original may write nothing else.

Every one of 34 paths was seen at least 30 times. The rarest were wheelie/stoppie starts (59, 67) and runs
(72, 73).
- The three hazard cases.
- The rumble: hard, soft, no pad or off, too little squeeze, and gated.
- The wheelie and the stoppie: clock only, starts, runs.
- Down with and without a stunt ended; left as it is; wrecked.
- Hang time exactly 0.1; speed exactly the stunt speed.
- The distance three ways.
- The ride clock held; the word falling.
- The countdown running, run out, and run out with listeners and ≥ 2 watchers.
- The second countdown both ways; no player; no vehicle state; rider held.

Run: `./kit check -- -p game-sim --test atv_quad`, 10 passed. Run directly with `--nocapture`, the test
printed its tally, not a skip.

Mutations, to check that the test can fail. Each changed one line of the port; all were restored.
- These failed, as they should:
  - stunt distance without the race check;
  - hang `<=` → `>=`;
  - sunk without the hazard;
  - slow `<` → `<=` (it survived until I added the speed = stunt-speed case);
  - no clamp on a backward distance;
  - the first watcher skipped;
  - LF's surface time left out;
  - stunt start at `< 0`;
  - second countdown at `<= 0`.
- These survived, both expected:
  - The dot product's two terms swapped. (0 + a) + b and (0 + b) + a are the same, as in MD64's full-lock
    sum.
  - "slow" read after the bike's update. The hooked bike update changes nothing, so the oracle cannot
    tell when it was read. The port reads it before, as the asm does.

## Not verified / not 1:1

- **Nothing the update calls is ported.** That covers the bike's vehicle update, the hazard clock, the
  stunt start and end, the pad rumble, the countdown's object, its listeners and watchers, and the post.
  They are `quad::Call`s, so their order and arguments are proved, not what they do. The rumble-on query
  and the rider's hold are fields (`Tail::rumble_on`, `Tail::rider_held`), not calls.
- **The return value is not modelled.** The original returns 1, or the bike update's own value when
  there is no vehicle state. `update` returns nothing. Nothing traced reads the value.
- **No capture or ride replay.** The quad's ride needs MD63's bike steps and a wheel port (MD37b).
- **Only the atv_ (Race) image** was used, as in MD64.
- **Names guessed from use, not from strings:**
  - the hazard table (its callee's −1 = at once and time-out are traced; that it means water or out of
    bounds is not);
  - `wheel_sunk` (+0xd0);
  - `wheel_squeeze` (+0xf0, MX Unleashed's compression offset);
  - the request word (+0x31c/+0x320);
  - `Player`, `Race` and all their fields (the countdown, the clock, word, step, speed);
  - the second countdown and the clock;
  - "top speed", "ride time" and "distance".
- **The inputs are generated, not captured.**
  - Ranges: surfaces 0..15 only (the byte could index past 16); squeeze 0.3..1.1 with 0.95 and 0.73
    exact; speeds as MD64's generator.
  - NaNs and huge values are not covered.
  - The rumble clamps (95, 40) are never reached by a squeeze in range. A strength under 96 needs a
    squeeze ≤ 0.836, which is not > 0.95. They are ported from the asm but never exercised.
  - The listener/watcher lists are at most 3 long.
- **Untested boundaries:**
  - The `|row.y| ≥ 1` row choice is tested at exactly ±1 and ±1.2.
  - The zero flattened row (rsqrt of 0) is generated now and then but not tallied.

## Next

Nothing in MD64a's scope is left. MD64b (the tire `.phy` reader) is still open. A Moto that runs the
bike's vehicle update and the tail's virtuals is MD37b's business.
