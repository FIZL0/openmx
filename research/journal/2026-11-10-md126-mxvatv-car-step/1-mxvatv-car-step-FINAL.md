# 1 — mxvatv car step (FINAL)

Task MD126. 2026-11-10.

## What I did

- `atv_car::Pieces` answers `Call::Controls(dt)` with `controls` (crates/game-sim/src/atv_car.rs): the
  steer (d-pad, wheel or stick, the tables, the rear share), the brake and gas slews, the hold, reverse,
  the clutch and the hand brake with its fade. The engine's reverse switch is a new call,
  `Call::Reverse(car::Reverse)`, handed to `rest` (the engine is MD125's).
- `Keys` gained the `[Joystick]` rate limits, the three steer tables, the rear-steer points, squared
  steering, both yaw rate limits and the hand brake's three keys. `State` gained the controls' words: driven,
  wheel, d-pad, start penalty, the gas pedal, steer input, steer, rear steer, engaged clutch, the fade, the
  rears' friction, the hold, the vehicle's reverse and throttle switches, and the engine's reversing flag.
  The current yaw rate limit stays `Car::yaw_rate_limit`.
- tests/mxvatv_car_frame.rs: these words are read, poked, generated (`generate_controls`) and compared on
  both sides. The reverse switch is hooked and logged on the player's engine. `controls_tags` tags the
  branches. There are two new tests: `controls_are_mx_unleasheds` and `all_six_pieces_are_mx_unleasheds`.
  The rear tires' friction words are now in the "written but not compared" blocks.
- research/oracle.txt `# MD126`: the function, clause by clause, with its words.
- Nothing taken from opentennis: it has no car physics.

## What the original does

`./kit fn -p atv 0x004ca450` (vt +0x34c), with `./kit dis -p atv 0x004ca450 0x004cab00` for the order.
It is MX Unleashed's `car_controls` (`crate::car::Controls::step`) on SLUS-21104's vehicle. These are the
differences:

- **Start penalty.** It first runs a check that clears the gas pedal while the vehicle's start-penalty
  clock (`*(car+0x680)+0x1d0`) is above 0.
- **No driver.** The cleared list is the same as MXU's, plus the gas pedal and the brake pedal. The engine
  is always put out of reverse.
- **Wheel.** A wheel is the vehicle state's word +0x54.
- **Hold.** A hold flag (+0xd7c) keeps both brakes full and the gas at 0. It is cleared by any gas.
- **Reverse and throttle switches.** Reverse needs the vehicle's reverse switch (+0xdd8). The gas slew is
  replaced by 0 while the vehicle's throttle switch (+0xdd4) is off.
- **Hand brake.** It writes the rear brake word (+0x1134), scaled with the measured speed (+0x32c) from 0
  to 10.
- **Inline remaps.** Every remap is inline, using `adda.s 0`/`madd.s`. The steer tables' and the rear
  steer's flat range answers `out_lo`: the `sub.s` in that branch's delay slot writes a register the code
  drops. That is unlike MD106a's listeners. The fade's flat range answers `1 - 0`, which is also 1.

## How it was proved

Function oracle in the frame: SLUS-21104's world step over MD39a's buggy image, with the controls' hook
removed so the original's own code runs them. The port runs `Pieces::answer`. Each case pokes generated
keys and state on both sides.

- `controls_are_mx_unleasheds`: 3,000 frames.
- `all_six_pieces_are_mx_unleasheds`: 3,000 frames with travel, anti-slide, preload, launch and aero
  unhooked as well.
- The whole file is green: 11 tests, including MD106a's five pieces, MD106b's own calls and the all-hooked frame. (2026-11-11: the first session's work, saved on wip/MD126, was re-applied onto main after MD106b and rerun green.)

Every compared word is equal, and so are the call logs, `Reverse` included. Each of the 22 controls
branches is seen at least 110 times in each run: start penalty; no driver; d-pad, d-pad centred, wheel,
stick, stick squared; low or high segment, each flat or not; rear steer flat; held; into reverse; out of
reverse; brake eases on; brake let off; no throttle; hand brake; fading; fading on a flat range; faded.

Mutation check: with the steer tables' flat range answering `out_lo - in_lo` (MD106a's remap),
`controls_are_mx_unleasheds` goes red at case 0.

## Not verified / not 1:1

- **The engine's reverse switch is not ported.** It is handed out as `Call::Reverse` and hooked in the
  test, which checks only that it is called with the right argument, in the right order. Its body (engine
  +0xbc/+0xc0/+0xd8/+0xdc, the gear set) belongs to the engine piece, MD125.
- **The reversing flag is only an input.** `State::reversing` is not updated by `Call::Reverse` in the
  port: whoever answers the call has to keep it.
- **The start penalty is a bool.** `State::start_penalty` stands for the vehicle's clock being above 0.
  Nothing here models the clock itself, and nothing proves who runs it.
- **The idle buggy runs SLUS-21104's controls unchecked.** With the hook removed, the image's second
  buggy also runs the original controls on its own words. Those words are not compared, and its reverse
  switch is hooked and skipped.
- **The PC exe was not read.** `research/mxvatv-pc.md` has no address for the car's controls, and the PS2
  function was ported clause by clause from its disassembly instead.
- **Keys are read off the image.** They are not built by the .ini loader: as in MD106a, building them from
  files is left to whoever wires `Pieces` into a ride.
- **Only the buggy** was run (MD39a's image), not the baja or the truck.

## Next

MD127 (contact sort and probes), MD124 (tire step), MD125 (engine step and shift; it should answer
`Call::Reverse` and keep `State::reversing`), then MD106c.
