# 1 — untamed tire on object (FINAL)

Task MD233. 2026-11-17.

## What I did

- Found the Untamed throttle tests red on main. Since MD131, the port's tire step asks "what does this
  tire stand on" (`Call::StandsOn`) itself. It no longer hands the whole `Call::TireStep` out. The
  harness only caught `TireStep`, so the ask fell through as a stray hand-out, and frame 161 failed.
- Compared Untamed's tire step with SLUS-21104's, part by part. The contact velocity is the same code:
  only registers and stack slots differ. It reads the same tire and object offsets. So the port's
  `contact_velocity` already fits. No port code changed.
- `crates/game-sim/tests/mxvatv_car_frame/untamed.rs`:
  - hooks Untamed's contact velocity on the truck's four tires
  - when a tire's stands-on words are set, keeps what they point at (read before the call) and the
    object's world centre of mass the call writes back (`+0x90`, read after it)
  - answers the port's `StandsOn` with the original's (in order, by its words)
  - checks each `ObjectCofg` the port writes, bit for bit, against the original's
  - asserts every one the original read was asked by the port
- The throttle tests now pin the frames with a tire on a prop: 161, 258, 259, 260 and 268. They are
  compared like every other frame.
- Nothing taken from ../opentennis: it has no car tire on a loose object.

## What the original does

- Untamed's tire step (`./kit dis 0x004d8320 -p game` on the Untamed scratch config, see the
  disassemble-Untamed note) is SLUS-21104's (`0x004b5ca0`) at the same tire offsets:
  - The contact velocity `0x004d7dd0` against `0x004b5740`: the same loads and stores at the same
    offsets. The diff is registers and the frame size only.
  - It reads tire `+0x19c` (the object). With its body (`+0xe4`) it writes obj `+0x90` = the body's
    quaternion turning obj `+0x80`. The ground's velocity is the body's, plus its spin at the ground
    point. That is `# MD131`'s reading.
- The tire step does not push the object's body. Every body call in its three parts (`jalr` through
  `tire +0x1ac`, vt `+0x4c`, `+0x84`, `+0x80`, `+0x78`) goes to the truck's own body.
  - The brief guessed that the push goes into the object. That is not the tire step's work.
  - The props are pushed by the record resolve, which MD161a already answers.
- The suspension `0x004d8370` and roll `0x004d8930` use the same offsets as SLUS-21104's (one fewer
  reload of `+0x1ac`). The forces part `0x004d8a70` too, through `0x004d9050`, where my window ran on
  into the next function.
- On MD160's throttle drive, the stood-on object is a prop with a rigid body (`0x00c774b0` in the
  spawn image's run). The tire takes it from the sort hit's object word (collidable `+0xbc`).

## How it was proved

- **Bit-exact replay** beside SLUS-21701's frame in the interpreter, through `frame_untamed` and
  through `Ride`:
  - throttle: all 310 frames, 663,930 words. Before, five frames were skipped.
  - 20 stands-on asks, each answered with the original's. 20 written-back centres of mass, each equal
    to the original's bit for bit.
- The other Untamed drives are unchanged: steer, jump and wreck. All 12 untamed tests pass.
- Only a test file changed, so every existing replay hash is unchanged.

## Not verified / not 1:1

- **Only an object on a rigid body is met.** The harness asserts a body. A tire on an object over a
  chassis, or on a mover, would fail the test rather than be answered. MD160's drives never do either.
  The port's reading of those branches is MD131's, proved on SLUS-21104 only. The code is the same in
  Untamed, but no Untamed frame covers it.
- **The object's words are read when the original's contact velocity runs**, mid world step, not from
  the port's own world. The port has no Untamed prop bodies of its own here; the harness feeds them.
- **The props' own words after the frame are still not compared**, as in MD161a. The tire step does
  not write them, apart from the centre of mass, which is checked.
- **The "push into the object's body" from the brief does not exist in the tire step.** I read all
  three parts' body calls. No live check beyond the replay, which matches on every word.

## Next

Land it (`kit done`).
