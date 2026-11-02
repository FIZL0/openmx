# 1 — kiowa (FINAL)

Task V113. 2026-11-02.

## What I did

- `crates/game-render/src/kiowa.rs` (new). `Kiowa::load` loads one skin out of `machine.res` and finds its
  rotor nodes (`RotorSpindle`, `Blade_Node01..04`, `TailRotor_Node`). It loads the four parts beside it
  and turns depth writes off on the blurs' blended surfaces. `Kiowa::found` picks up an already-loaded
  skin. `pose(model, &Rotor)` replaces the spindle's rotation with the angle about y and the tail's with
  the same angle about x, then composes down the parents as `biplane::pose` does. `parts` lists what
  hangs where: 4 × `bladeblur` while the blades are on, and `tailrotorblur`, or `tailrotorbent` once
  `wrecked`. `Rotor { angle, blades, wrecked }`, with `Rotor::between` for interpolation the short way
  round. There are 3 unit tests.
- `crates/game-render/tests/kiowa.rs` (new, disc). Every skin loads with its nodes and all textures. The
  parts load, and the blurs blend without writing depth. `kiowa2` is the `kiowa_body02` livery. A quarter
  turn keeps the blades on the rotor circle and the tail hub still: spindle at (0, 8.41, -0.61), tail
  rotor at (-1.24, 7.09, -23.53) in model space. `free09_2.res`'s `helocpresets.ini` reads as
  "Aero Cam 1", "Aero Cam 2", "Slot Cam", "Lookback".
- `crates/openmx/src/kiowa_draw.rs` (new) turns a `Kiowa` + `Rotor` + placement into GPU actors (model
  posed, parts unposed at their posed nodes, no shadows).
- `crates/openmx/src/main.rs`: at load, when the lot has a helicopter, the skins load from `machine.res`
  and a line reports the skins and the HeloCPresets count. The lot's `Machine::Helicopter` bays are drawn
  at `BAY_ALPHA` when the bays setting is on (not in overview), rotor at rest.

## What the original does

Read in Ghidra (`./kit fn <addr>`):

- `0x00390c70` (MX helicopter init) loads 4 × `bladebent` (+0xd2c..+0xd38, later thrown as rigid bodies
  +0xe48..+0xe54). It finds `RotorSpindle` (+0xd04), `Blade_Node01..04` (+0xd08..+0xd14) and
  `TailRotor_Node` (+0xd18). It loads 4 × `bladeblur` (+0xd1c..+0xd28), `tailrotorblur` (+0xd3c) and
  `tailrotorbent` (+0xd40).
- `0x003920b0` (blade spin): unless the blade blur node is hidden, the angle +0xe58 += 27·dt. The
  spindle's rotation is set to the angle about (0,1,0) (`0x426db0`) and the tail's to the same angle
  about (1,0,0) (`0x426da0`). Each `Blade_Node`'s world matrix is copied onto its `bladeblur`, and the
  tail node's onto `tailrotorblur`. The bent tail is placed on the tail node whenever it is shown.
- `0x00392b60` (wreck) throws the blades (vt+0x1b0) unless the reason is 13, hides the tail blur and
  shows the bent tail (hide `0x001b3850`, show `0x001b3870`). `0x00392c10` (unwreck) shows the 4 blurs
  and the tail blur again and hides the bent tail.
- There is no swap by speed: the blur is always the drawn blade. The "blur swap" in the brief is the
  wreck swap.
- Live on C14's state (copy 1, 2026-11-02): helicopter actor 0x007ccae0, angle +0xe58 read 2483.2 then
  2539.9 about 2 s later, which is ~27 rad/s. It is never wrapped. The blur node word +0xd3c stayed 0x40.
- HeloCPresets: `appstart\helocpresets.ini` is in free06/07/09/10/11 `_2.res` and `_ss_2.res`, in
  BiplaneCPresets' format. Aero Cam 1 is AngleY 180, AngleX 20, FollowingDistance 40, FOV 85.

## How it was proved

- Tests: `./kit check -- -p game-render --test kiowa` (2 passed, real disc) and
  `./kit check -- -p game-render --lib kiowa` (3 passed). `cargo check -p openmx --tests --bins` is clean.
  `./kit audit` shows nothing in these files.
- Looked right beside a screenshot. The PCSX2 side is C14's `flight.p2m2_SaveState.p2s` in copy 1's slot 8,
  started with `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1`, the screenshot taken by pressing
  System → Screenshot over AT-SPI (no uinput pad here, and grim sees the lock screen). The port side is
  `openmx --track free09 --headless --shot-pose 3063.6,145.2,4054,-90,-20,85`, Aero Cam 1 worked out by
  hand from the bay (40 ft behind, 20° down). Files are in `context/shots/v113/` (git-ignored): `orig.jpg`,
  `port.jpg` (as shipped, half alpha), `port1.jpg` (a throwaway opaque build) and `side.jpg` (opaque port
  over the original).
  The view is the same: the trees, the house, the hill on the left. The Kiowa is the same blue skin, at the
  same place and size on screen, with the tail boom, the tail fin and the tail rotor where the original
  has them. Opaque, the port's blurs show the same bright tip streaks as the original's.

## Not verified / not 1:1

- **The camera is not wired.** HeloCPresets is only read, counted and tested. Following a flown Kiowa with
  it belongs with P216, since nothing flies the Kiowa yet. A follow-up is filed.
- **The parked Kiowa is see-through and its rotor is at rest.** That is the port's convention for every
  bay (GPU only, `BAY_ALPHA`, no collision), so it does not look like the original's solid machine. In
  C14's state the Kiowa is the human's, so its rotor spins. Whether an unowned parked Kiowa spins was not
  checked. The blur's look hardly changes with the angle, so the stills cannot tell a still rotor from a
  spinning one.
- **The blade angle differs** between the two shots (rest vs. 2483 rad), so the streaks sit in different
  places.
- **The thrown blades (`bladebent` rigid bodies) are not drawn.** `parts` drops the blurs when `blades`
  is off and nothing else is drawn; S220a1 owns the thrown blades.
- No software-renderer path. The `_ss` skins load, but the parked draw uses skin by bay livery, as the
  biplane does. Split-screen skin choice was not checked.
- The opaque build shows a ground shadow under the Kiowa; the shipped half-alpha draw does not. That
  shadow was not compared.
- One place (free09's first bay), one camera, one time of day.

## Next

Done for V113. P216 should drive `Rotor` from `game_sim::kiowa::spin_blades` and draw the flown Kiowa
through `kiowa_draw::actors` at alpha 1. It should also follow the Kiowa with HeloCPresets
(`game_render::kiowa::PRESETS`).
