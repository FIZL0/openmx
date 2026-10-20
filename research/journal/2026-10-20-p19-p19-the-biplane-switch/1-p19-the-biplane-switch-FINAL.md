# 1 — p19 the biplane switch (FINAL)

Task P19. 2026-10-20.

## What I did

- **New module `crates/openmx/src/vehicle.rs`.** It holds `Kind` (bike or biplane), `Spawn` (the
  `[Biplane<n>]` section of `machine.txt`), `Plane` (the plane's spawn, transform and velocity) and
  `Garage` (which machine has the controller, plus `swap` and `step`). It uses only std and `game_sim`, so
  `tests/ride.rs` compiles it in by path, as it already does with `pad.rs`.
- **`--vehicle bike|biplane` in the shell.** The default is `bike`. `World::simulate` now calls
  `garage.step(...)`. With the bike up, that is `sim.step` and nothing else.
- **M swaps machines.** The swap reacts to M going down, not to it being held, and happens between
  frames. It is not read while `--play` rides a tape, because tapes hold no swaps.
- **The plane's files**, all read from the disc at runtime:
  - Its spawn comes from `tracks\<track>\machine.txt` in **`machine.res`**. The file is not in the track's
    own archives.
  - Its camera comes from `appstart\biplanecpresets.ini`. Despite the `AppStart\` in its name, the disc
    ships it in each freeride track's **second archive** (`free06_2.res`, also the `_ss_2` ones), not in
    `appstart.res`.
  - Its model `biplane\anim\biplane.psg` comes from `machine.res` and draws through the shared loader
    with its own `pitts_*` textures.
- **The plane's camera** is a second `Race` camera on those five presets. It starts on `Flight Cam 1`,
  picked by name with P14's `Preset::find`. It snaps whenever a swap hands it over. L1 walks whichever
  camera is up.
- **With the plane up:**
  - the bike and the rider are not drawn, and their animation is not stepped;
  - the plane is drawn wherever it is (it is drawn while parked too);
  - the camera follows the plane;
  - the once-a-second line reads `(biplane)`.
- **The overlay** (`--hud`) has a `vehicle bike|biplane` line, in the empty fifth row of its first
  column.
- **`window::key::M`** (X11 keycode 58) is added.

On free06 the session prints `biplane: parked by tracks\free06\machine.txt ... facing 180 degrees`,
`5 camera presets`, `model loaded`, and `the dirt under the biplane is at 135.0 feet`. That height is
exactly the file's own Y, so the parked plane stands on the ground. A 320x224 `--shot` shows the plane,
textured, from behind and above, and the overlay shows `VEHICLE BIPLANE` and `CAMERA FLIG…`.

## What the original does

The source is R8's map (`research/oracle.txt`, the R8 block, `machine_swap` and the entries around it).
I read one more function myself: the pause menu's "Switch To Machine: %s" item, `./kit fn 28fcf0`.
- When the item is activated (event 0xc), it takes the selected machine out of the list and calls the swap
  `./kit fn 210b90` directly. It does no distance test and no speed test.
- The 30 ft range and the 40 mph / wrecked gate belong only to the prompt path: `./kit fn 3a8330`, then
  `./kit fn 3a7210`.
- The shell has neither a menu nor a prompt. Its M key is therefore the menu's path: the swap, with no
  gate.

What the swap does (R8):
- **The machine left** loses the controller. If it is the bike, its model is hidden, and no transform is
  written.
- **The machine taken** gets the controller and its own camera. A biplane is also unwrecked and reset to
  its spawn transform.

The placement `./kit fn 20f6e0` reads `[Biplane<n>]`'s `Position` and `Direction`. It also reads
`SetGroundHeight`: the string at `0x42edb0` is read with a default of 1. That key only changes whether
the position's `w` is passed through or forced to 1 before the placement call.

## How it was proved

- **A default session is the shell as it was.** I built the pre-P19 HEAD in a scratch worktree and rode
  the human's `first_recording.p2m2` as a tape (a copy, played with `--play ... --record`) on both builds:
  - both printed `ride hash 0xe781e509c443dec8` over the same 1811 frames, with the same wreck line;
  - the two tapes they re-recorded are byte-identical (`cmp`).

  In `tests/ride.rs`, `a_default_session_rides_the_bike_exactly_as_before` rides the whole 4020-frame
  minute twice: once through `Sim::step`, once through a `Garage` with the bike up. It asserts that
  `Sim::hash` is equal on every frame and that the parked plane never moves.
- **A session started in the plane.** `a_biplane_session_starts_in_the_plane_and_leaves_the_bike_alone`
  runs five seconds of the minute's throttle with the plane up. It asserts:
  - the plane is up, at its spawn;
  - the ride's hash is unchanged on every frame;
  - `sim.ticks` stays 0;
  - the pad still reached the input manager.
- **A swap mid-session.** `a_swap_mid_session_swaps` follows this sequence:
  - It rides the bike for 4 s. The bike must be off the line and rolling.
  - It swaps while the bike is moving. The plane is reset to its spawn, and over 3 s the bike's hash does
    not change.
  - It swaps back. The bike's first frame is within 5 ft of where it was left, so it was not reset to the
    start line, and the bike moves on from there.
  - A second swap into the plane resets a plane that had been moved.

  Unit tests in `vehicle.rs` cover the `machine.txt` parse, the swap's reset and refusal, and the words
  `--vehicle` accepts.
- **The plane flies:** `a_biplane_session_flies` is `#[ignore]`d with S34's name on it. Run with
  `--ignored`, it fails as it should: "the plane moved 0.0 ft on twenty-two seconds of throttle".
- `./kit check`: all tests pass. `./kit audit`: ok.

## Not verified / not 1:1

- **The plane does not fly.** S34's flight, S33's `biplane.phy` loader and I6's pad-to-plane mapping are
  not on main, so `Plane::step` is an empty hook. A plane that is up stands where it was parked. The
  controls with the plane up are not I6's: the pad is polled into the input manager, but nothing reads
  it for the plane. **What S34 has to wire in:**
  - replace `Plane::step`'s body (`crates/openmx/src/vehicle.rs`) with the flight step, fed I6's controls
    and the `world` it is handed;
  - give `Plane` the attitude, an honest `on_ground` and the speed the camera's `Ridden` reads (`main.rs`
    builds it from `plane.at`, `plane.facing`, world up, `plane.vel`);
  - draw the plane with its attitude rather than `M4::placement(at, facing, up)`;
  - un-ignore `a_biplane_session_flies`.

  I6 binds the plane's controls; if the shell needs a per-machine `Controls`, `Garage::step` is where
  the choice goes.
- **Which plane camera the original starts on.** I picked `Flight Cam 1` because it is the file's first
  record. I did not read the camera's preset index off the biplane recording's save state.
- **The plane camera's speed reference and reverse speed.** These are the bike camera's 117.3328 and 5,
  on the assumption that one constructor builds both cameras. I did not read them for the plane's camera.
- **The camera snaps on a swap.** The original runs `0x00200f00(app, player, 0)` after the swap, and I did
  not read it. Whether it snaps the new camera or lags it in is unknown.
- **The bike while switched out is frozen.** R8 left open whether the original keeps simulating it (a
  bike left rolling might roll on) or freezes it. The port freezes it; nothing steps it.
- **The pause menu's own gating.** `./kit fn 28fcf0` has no gate, but I did not read whether the menu
  only offers the item in some states. Prompt-only conditions such as wrecked or above 40 mph do not
  stop the shell's M key.
- **No prompt.** The original's freeride prompt within 30 ft of a driverless plane
  (`STR_SWITCH_BIPLANE`) is not ported. Its answer button is unknown (R8), and the shell has no front
  end to put a prompt in.
- **The key was not pressed live.** This desktop does not deliver keystrokes to the window (see the
  project notes), so M was exercised only through `Garage::swap` in the tests, never in a running
  window. The `--vehicle biplane` session was run live (4 s, with a shot).
- **A swap is not on tapes.** `--record` writes pad reports only, so a recorded session with a swap in
  it plays back as a bike session. For that reason M is ignored during `--play`.
- **`SetGroundHeight` is not applied.** I did not read what the placement does with it beyond the `w`
  lane. On free06 the file's Y is the ground height (135.0), so it makes no difference there.
- **The plane's drawing is V21's.** The model is drawn as the shared loader makes it:
  - no pilot (the rider is simply not drawn with the plane up);
  - no prop cone;
  - none of `biplane2`/`_ss`;
  - no emitters.
- **The second player's plane** (`[Biplane2]`) is never read. The shell is one player.
- **A hazard I did not touch.** The bike camera's lookup `read_suffix("cpresets.ini")` also matches
  `appstart\biplanecpresets.ini`, which lives in the same `free06_2.res`. Today it finds the track's
  file first (11 presets, `Race Cam 1`), but that depends on directory order. I left it alone because
  the bike must not move.

## Next

Nothing for P19. S34 picks up the four items under "The plane does not fly".
