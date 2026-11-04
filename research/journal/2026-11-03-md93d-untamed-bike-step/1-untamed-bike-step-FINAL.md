# 1 — untamed bike step (FINAL)

Task MD93d. 2026-11-03.

## What I did

- **Classified the two functions** MD81a had left open. I diffed them with `research/md81a/sdiff.py`, with
  three things masked: the frame size, any base register in a memory operand, and the bike register, which
  is `$s0` in SLUS-21104 and `$s1` in SLUS-21701. I then read each callee pair. The addresses and field map
  are in research/oracle.txt `# MD93d`.
- **The no-hander is scheduling only.**
  - `$f5`/`$f6` are swapped.
  - One `mul`/`adda`/`madd` block is reordered; it computes the same `len²` sum.
  - Its fields, acos table, compare constant and callees moved.
  - It has no port, and none is needed: the port has no SLUS-21104 no-hander either.
- **The air state differs in one branch.** When the in-world test fails on the bike's origin:
  - **SLUS-21104** clears the wreck reason and two more words, posts the out-of-world event (8, 13), and
    calls the put-back (vt `+0x3a8`, handed 1, 1). That put-back snaps the bike to the ground.
  - **SLUS-21701** clears nothing, posts the same event under its moved tag, and calls the bike's unwreck
    (vt `+0x36c`, handed 1). The unwreck resets the stunt words, ends the stunt state and calls the
    put-back handed 0, so there is **no ground snap**. It also runs the base unwreck.
  - The other 171 differing lines MD81a counted are scheduling. The bike register moved, `$s0` caches the
    body's orientation pointer, and the frame grew 16 B.
  - The callees are the same code on moved fields: balance clear, quaternion axes, in-world test, event
    post, vehicle post and node position. The quaternion-to-euler conversion swaps its out-of-line `b`
    stubs for `bc1fl`/`bc1tl` with the `mov.s` in the delay slot, which gives the same clamps.
- **`handling::untamed::air_state`** (`crates/game-sim/src/handling/untamed.rs`) is the port.
  - It works over a new `untamed::AirState`, holding the bike's words, its body as `quad::Body`, the
    centre of gravity, both wheels' touching words and contact velocities, and the collision record's
    tipped word.
  - It reports its calls as `untamed::AirCall`, the MD93c `UpdateCall` style: the callback is handed the
    state and may change it. The callback's answer is read only for `InWorld`, where it means "inside".
  - Its math is SLUS-21104's quad air state's (`quad::air_state`) on the bike's fields: the speed, the
    local velocity and the axes, the euler angles, the world angular velocity, ground and airborne, hang
    time, contact velocity, crossup with authority, and tipped. The origin is computed on VU0, as the
    original does.
- **Not wired into the ride.** SLUS-21104's bike air state is not ported as a function either: the ride's
  air step is `air::contact` (SLUS-20767's), and its put-back is the port's own. `air_state` would take that
  place once a ride runs SLUS-21104's bike PostProcess.
- **No crate code was moved or renamed.** I added `AirCall`, `OUT_OF_WORLD_EVENT`, `AirState`, `air_state`
  and a private `origin` at the end of `untamed.rs`, plus one bullet in its module doc.

## How it was checked

- **`untamed_air_state_is_the_originals`** (`tests/untamed_reworked.rs`) runs 20,000 generated states on
  `nat_or.bin`, with the bike, body, vehicle state, two wheels and the collision record laid out at
  Untamed's offsets.
  - **Hooks.** Seven callees are hooked: balance clear, no-hander, in-world test, post, unwreck (whatever vt
    `+0x36c` holds), vehicle post and node position. Each hook checks `a0`: the bike, the post queue word,
    or the node. The post's hook also checks `a1` = bike+0x698, 8, 13 and 0, and the unwreck's checks
    `a1` = 1. Each logs `dt`, the origin handed to the in-world test, and the post's tag.
  - **Hooks change state.** Each hook also changes the state the way the port's callback does:
    - the balance clear sets the wheels' touching words;
    - the no-hander sets the velocity and the flipping word;
    - the in-world test sets the tag and answers inside or outside;
    - the unwreck writes the crossup and the node position, which are written over later;
    - the vehicle post sets the hang time;
    - the node writes its position.

    So every read after a call is checked to come after it.
  - **Not hooked.** The quaternion axes and euler angles are left to run, so the port's
    `math::quat_axes`/`quat_to_euler` are checked on Untamed's copies too.
  - **Comparison.** Every word of the six regions must match by bits, and the original may write nothing
    outside them and the stack. The call lists must be equal.
  - **Coverage.** Each path reaches at least 100 states: no state, remote, balancing, not balancing,
    contact from the front, the rear or the body, takes off, stays up, crossup, no crossup, full authority,
    no authority time, authority ramp, up exactly at a quarter, tipped, upright, out of the world, and in
    the world.
  - **Arc cosine table.** The table and its scale in RAM are compared against `AcosTable::build()`.
- **Hand mutations, each red and then reverted (12):**
  - balancing tested without the wreck reason;
  - the hang time restarted every airborne frame;
  - the rear contact taking the front's velocity;
  - the crossup speed 10.5;
  - the sign from `c.x`;
  - tipped on `<`;
  - the tag read before the in-world test;
  - no kept node copy;
  - unwreck before the post;
  - the heading's `w` zeroed;
  - the last-airborne word from on-ground;
  - only kind 3 remote.
- **Regressions:** `untamed_reworked` (9), `untamed_step` (2) and `atv_quad` (60) pass with their images
  present. `./kit audit` shows only the 4 leaks already on main.

## Not verified / not 1:1

- **No capture.** No ride was recorded and the air state was not compared on one. The proof is a function
  oracle on generated state, with every callee but the axes and euler angles hooked.
- **No callee is ported here.** That covers the balance clear, the no-hander, the in-world test, the
  post, the unwreck, the vehicle post and the node.
  - The unwreck's effect, a put-back without the ground snap, is read from the asm, not run.
  - The port's ride has no Untamed out-of-world behaviour.
- **Not wired into the ride.** Nothing runs `air_state`, so the out-of-world change does nothing on screen.
- **Generated states are not ride states.** Quaternions are random unit ones plus a few hand-placed edge
  cases. Wheel words are 0, 1 or random. NaN and Inf speeds or authorities were not fed.
- **The no-hander's verdict rests on the diff and a read of the asm.** It was not run against SLUS-21104's.
- **The PC exe was not read**: `-o pc` is not configured in this slot.
