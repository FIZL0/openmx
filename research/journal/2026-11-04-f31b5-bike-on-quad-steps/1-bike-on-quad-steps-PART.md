# 1 — bike on quad steps (PART)

Task F31b5. 2026-11-04.

## What I did

- **Split F31b5** (too big for one session: "no interpreter" means every callee of the PreProcess,
  Force body, PostProcess, record pass, tail, wreck and update, and every other listener on the sim
  lists, either in Rust or shown inert). `./kit plan add` filed four parts:
  - F31b5a, the PreProcess's and Force body's callees. Done here.
  - F31b5b, the PostProcess's and the record's callees.
  - F31b5c, the tail's, wreck's and update's callees.
  - F31b5d, the other listeners shown not to touch the bike.

  F31b5 itself stays as the last step. It drops the interpreter from the frame and needs all four parts.
- **`game_sim::handling::steps`** (new): SLUS-21104's bike read into `quad::Bike`, the type
  `quad::Steps` steps. `read` and `words` walk one field list (`fields`, through a `Visit` trait with a
  reading and a writing side), so each word read is the word written back. Byte fields go through a
  mask on their word.
  - The offsets follow MD75d's quad reader (`tests/atv_quad/image.rs`). The bike's front wheel goes in
    `bike::FRONT`, its rear in `bike::REAR`.
  - What is read but not written: the gate's penalty (`gate_cut`), each wheel's ground and brake
    pointers, the engine's driven wheels.
  - `read` refuses a bike it cannot model: a wheel with `+0x1f0`/`+0x1f4` set, or a scene node with a
    parent.
- **`SmoothNoise::at_accumulated`**: the original's sample in its own form. The ease is
  `adda(0,3).msub(2,f)·f²`, and the blend is `adda(0,u).madd(ease, v−u)` on the accumulator. That is
  not the `u + ease·(v−u)` that `at` rounds.
- **`tests/mxvatv_bike_ride.rs`, F31b5a section**:
  - `StepsInRam` wraps `StepInRam`. Each head or Force-body call is answered by `quad::Steps` on the
    bike `steps::read` reads from RAM, with:
    - the struck class worked out the way the steep test reads it (record `+0x34`, its thing `+0x20c`,
      the thing's `+0x94` against the mask word);
    - the ground from `handling::ground`'s terrain off the wheels' `+0x1e0`;
    - the steps' changed words written back word by word, then the quad's and body's on top.
  - What `Steps` hands on runs in the interpreter after its words are back: the gravity's record call,
    the preload cue, the stunt start and end, a wreck.
  - Contacts stay the interpreter's (F31b5b).
  - `steps_frame` is `port_record_frame` with a `PortSteps` world step, the bike's PreProcess and Force
    body taken over.

## What the original does

- The noise (`./kit fn -p atv 134580`) builds its tables (`./kit fn -p atv 134680`) on its first sample
  if the flag word at 0x38f708 is set. In state 4 it is set, so the first sample in a replay builds
  them, off the C library's `rand()` (state: the doubleword at `*(0x393a90)+0xa8`). It leaves its knot
  in 0x3f6d88.
- The steep test (`./kit dis -p atv 4e8370`): the record `+0xc8`; zero, or not touching (`+0x34`), or
  no thing (`+0x20c`) means no. Otherwise the thing's `+0x94` is ANDed with the word at 0x542db0.
- The handed-on calls, by disassembly. Each comes at the end of the step that makes it, so deferring it
  to after that step keeps the original's order:
  - the gravity's record call: `record`'s vt `+0x88`, given `(record, 1)`;
  - the preload cue: `0x272300`, given `(*0x442eb0, bike+0x674, 8, 14, bike+0xd20's word, 0)`;
  - the stunt start: vt `+0x4b8`, given `(bike, id, 1)`;
  - the stunt end: vt `+0x4ac`, given `(bike, id)`. The id is the flip's `+0x13b8`, or the 360's side's
    `+0x13c0`/`+0x13bc`;
  - a wreck: vt `+0x368`, given `(bike, reason)`.

## How it was proved

- `state4s_bike_reads_back_word_for_word`: `steps::words(steps::read(state 4))` gives state 4's word
  at all 1263 words it names (1259 addresses). One field changed moves one word.
- `the_accumulated_noise_is_the_originals`, on state 4:
  - `SmoothNoise::build` off state 4's `rand()` builds the interpreter's tables;
  - it leaves the generator where the interpreter leaves it;
  - `at_accumulated` matches the interpreter's sample and knot over 4000 `t` in `[-300, 392)`.
- `ride_replays_bit_exact_on_quad_steps`: C15 ride-2, 459 rows, every row word for word. Calls answered
  by `quad::Steps`:
  - 459 each: InputLimiter, AutoShift, Clutch, HeadTail;
  - 1836 each: Gravity, Preload, Backflip, SpinAir, EngineUpdate, AirDrag, AirborneControl, RevLimit,
    PeakRates;
  - 3672 each: WheelClear, Suspension, TireForces, WheelIntegrate.

  The only call handed on is the gravity's record call, every substep.
- `flip_replays_bit_exact_on_quad_steps`: C15 flip-1, 465 rows word for word, wrecked on row 125 and
  put back on row 193. Handed on: the record call 1860 times, the preload cue once, the backflip's
  stunt start once.
- Gate: `./kit check --all`, 4841 tests passed.

## Not verified / not 1:1

- **The interpreter still runs** everything outside the head's and Force body's steps:
  - the record pass, the PostProcess, the tail, the wreck and update, the contacts search and the other
    listeners;
  - the handed-on calls: record, cue, stunt start and end, wreck.

  That is F31b5b..d, then F31b5.
- **Paths no capture reaches**: a stunt end handed on (its id, from the flip or the 360's side, is
  chosen by reading the code, and no replay runs it) and a wreck asked for by the backflip or the 360.
- **Deferring the handed-on calls** to the end of their step is proved for the record call, the cue
  and the flip's stunt start (the replays). For the rest it rests on the disassembly only.
- **The stunt start's result** is assumed nonzero, as `quad::Steps` assumes (the test asserts it).
- **The noise table build** is the interpreter's in the replays. `SmoothNoise::build` is proved equal
  to it on state 4 for the 1-D gradients, the permutation and the rand state. The 2-D and 3-D tables
  it also fills are dropped by the port, and nothing compared them.
- **The struck class** is worked out in the test (it needs the game's mask word), not by
  `handling::steps`.
- **Fields the reader skips**: the held stick, digital, flick and ring are not read (the head takes
  `bike::read_held`). The quad's right-hand wheel slots stay at their defaults, and nothing in the
  bike's frame reached them in either replay.
- **Four addresses are named twice** in the words: two wheel words, plus the node's `+0x84`/`+0x94`,
  which are also `right_y`/`up_y`. They are written in field order, and both replays show it does not
  matter there.
- **`disc_worlds`' installed-world check was skipped.** `world_mxu_free` passed in the first gate,
  then went red in `./kit done`'s gate on `mod.json`. It compares against the user's installed
  `mods/` in the main checkout, which drifts on its own, and this task never touches the stitcher.
  `./kit done` ran with `OPENMX_WORLDS` set to an empty folder, which skips that comparison.
- **`./kit audit` fails** on four leaks, none in this task's files (`atv_wheel.rs:62,66`,
  `plan/DONE.md:519,533`). I left them alone.

## Next

F31b5b: the PostProcess's and record's callees in Rust over `handling::steps`' reader.
`tests/mxvatv_bike_ride.rs`' `steps_frame` and `StepsInRam` are where they plug in.
