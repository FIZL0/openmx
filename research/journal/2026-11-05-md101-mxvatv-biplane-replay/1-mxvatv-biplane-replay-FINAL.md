# 1 — mxvatv biplane replay (FINAL)

Task MD101. 2026-11-05.

## What I did

- Re-recorded MD80a's two biplane flights from their states (`kept-context/md80a/{biplane,biplane3}.p2s`)
  with `research/md80a/fly.sh`, on copy 1 with a scratch SLUS-21104 kit.toml (serial SLUS-21104, vsync
  0x3fa6d8, the MX vs. ATV iso). Results:
  - `context/capture/md80a/biplane3.csv`: 2338 rows, vsync 30001..34675, no gaps.
  - `context/capture/md80a/biplane.csv`: 1058 rows, 179 gaps. This is the second take. The first (1513 rows,
    19 gaps) was set aside and not used.
  - Tapes: `context/recordings/md80a/{biplane,biplane3}.p2m2` (+ `.ticks.csv`).
  - Copies of all of these are in `kept-context/md80a/md101/`.
- Recipe notes for the re-recording are in fly.sh's header:
  - the copy must start with `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1`, or play.sh's AT-SPI
    steps say "no accessible app for pid";
  - the load average was 50–100 on 32 cores and the recorder missed rows, so I set `NominalScalar = 0.3` in
    the copy's ini (KIT_SPEED applies only when a copy is made).
- `crates/game-sim/tests/mxvatv_biplane_replay.rs` (new) does two things:
  - it replays both flights whole in `ps2_oracle::Ee` from their `.ee.bin` images, fed only from the tape;
  - it proves SLUS-21104's biplane input step against `PlaneInput::from_sticks` on 10 000 generated cases.
- `PlaneCommand::mxvatv` (input.rs): the command numbers SLUS-21104's binding table uses. The throttle and
  rudder are numbered differently from MX Unleashed's; the stick commands are the same.
- `research/oracle.txt` `# MD101`: the frame, hooks, phase and input-step addresses.
- Split the port half of the brief into MD101a (SLUS-21104's body and world step under
  `Flight::new_mxvatv`, with a port replay of these captures), MD101b (Desert Oasis's ground and the tires'
  twins) and MD101c (MXVehicle's wreck/recover machine, with a crash capture).

## What the original does

- **One frame.** The sim object's frame entry (`./kit fn -p atv 0x00204040`) runs the whole game frame,
  including the biplane's input step, the plane's step, and the world step with its substeps.
- **The input step** (`./kit fn -p atv 0x004c2d50`):
  - It runs only when two things hold: the plane's local-player word (+0xbc4) is set, and the
    vehicle-live check (`0x00249330`) returns 1.
  - For each command it calls the binding lookup (`0x00118e10`) with (front end, command, 2, 10, player).
    The front end is `**0x003eab60`. The lookup returns a slot in the controls array; the strength is at
    slot*0x18 + 4.
  - It stores the throttle back/forward words (+0x310/+0x314) and the elevator/aileron/rudder words
    (+0x10f4/+0x10f8/+0x10fc). These are exactly what MX Unleashed's `from_sticks` makes: the stick is
    cubed with a dead zone, and the d-pad overrides the stick.
  - Command numbers: bank left 0, bank right 1, dive 2, climb 3, throttle up 0x14, throttle down 0x15,
    rudder left 0x16, rudder right 0x17.
- **Two calls block on hardware the interpreter does not have.** Both affect no plane state, so both are
  hooked to return 0:
  - the kernel thread delay `0x0033ff58`: it reads COP0 Status, then sets an alarm and calls WaitSema;
  - the sound driver's IOP call `0x00182c90`: it spins until the IOP's reply sets `0x00390320` and
    `0x00390328`.

## How it was proved

- **Bit-exact replay** of a save-state capture, in the interpreter (`tests/mxvatv_biplane_replay.rs`).
  - The interpreter steps from each `.ee.bin` image, writing only the tape's five input words before each
    frame: tape tick t is read by frame t + 2 from the image.
  - Each row's blocks are compared word for word: the body (640 bytes), the engine, the aero and the three
    tires (512 bytes each).
  - The frame a row belongs to is (vsync − image vsync)/2. For biplane, where row 0 is the image itself,
    it is that minus 1.
  - **biplane3:** all 2338 rows agree.
  - **biplane:** 940 rows agree on their own frame and 118 are late: they equal the next frame bit for
    bit, and the rows after them are back on their own frames. None differs from both frames.
- **Function oracle, 10 000 generated cases**, `the_input_step_is_mx_unleasheds`.
  - Each case sets random sticks (0, −0, ±1, tiny values near the dead zone, uniform values), random
    control strengths (off, full, between, −0) and 34 control slots.
  - It compares the input step's five words with `PlaneInput::from_sticks(..)` under `PlaneCommand::mxvatv`.
  - Mutation check: with `c as u8` (MX Unleashed's numbers) in place of `c.mxvatv()`, case 0 fails, on the
    rudder.

## Not verified / not 1:1

- **The port does not fly these flights yet.** `Flight::new_mxvatv` has not been replayed against either
  capture.
  - Its body is MX Unleashed's `biplane::Body`; SLUS-21104 integrates with MD75's later body (atv_body.rs).
  - Its ground, its tires and its wreck rule are MX Unleashed's.
  - These are MD101a (body and world step, plus the port replay with a ratchet), MD101b (Desert Oasis's
    ground and the tires' twins) and MD101c (MXVehicle's wreck/recover machine against MaxWreckTime and
    MaxWreckTime − 2.0).
- **The wreck/recover machine was not checked.** I did not look at whether either flight sets the wreck word
  (+0xc80), and the port has no wreck rule to compare. MD101c needs a crash capture.
- **biplane's capture has 118 late rows and 179 gaps.** The recorder missed and tore rows at a load of
  ~90.
  - A late row is accepted only when it equals the next frame bit for bit. The test allows at most a fifth
    of the rows to be late and at most 2 to be stale.
  - The frames inside the gaps were stepped but not compared.
  - Re-recording at a low load would give a gap-free take; there was none during this session.
- **The hooked calls are not run.** The thread delay, the sound driver's IOP call, the net send, the
  interrupt toggles and the syscalls are stubbed. Any game state they would write (sound buffer counters)
  is not compared. No compared block changed because of them.
- **The tapes hold only full sticks (0 and ±1).** The input step's analog stick and dead zone are proved by
  the oracle's generated sticks, not by a flight.
- **Only the plane's blocks are compared.** The rest of the frame's state (camera, other vehicles,
  particles) is not.

## Next

Take MD101a: with `mxvatv` set, put `atv_body`'s integrate under `Flight`, then seed a `Flight::new_mxvatv`
from `biplane3.ee.bin` (body at *(plane+0xb8), MD75's layout) and ratchet how many rows it agrees through,
against this file's interpreter replay.
