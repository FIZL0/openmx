# 1 — v2 side by side (FINAL)

Task V2. 2026-10-15.

The same moment, twice: one frame the original drew, and the port's frame of the same bike on the same
dirt seen from the same camera, both out of one save state. The pair and the differences are below; the
two tools that make another pair are committed, because the task says to do this *every time a subsystem
lands*, not once at the end.

```sh
./kit state ram context/recordings/first_recording.p2m2_SaveState.p2s --out context/states/human-ram.bin
python3 research/shots/moment.py context/states/human-ram.bin -o context/shots/v2-moment.txt
./target/release/openmx --track free09 --bike cr125_faisst --size 640x480 \
    --moment context/shots/v2-moment.txt context/shots/v2-port.png
python3 research/shots/side_by_side.py context/shots/v2-original.png context/shots/v2-port.png \
    context/shots/v2-side-by-side.png
```

In `context/shots/` (git-ignored, as every game-derived image is):

| file | what it is |
| --- | --- |
| `v2-original.png` | the frame the original drew at the moment the state was saved — PCSX2 keeps it inside the `.p2s` |
| `v2-original-live.png` | the same state re-loaded into kit copy 5 and screenshotted, 936x702 |
| `v2-port.png` | the port's frame of that moment, 640x480 |
| `v2-side-by-side.png` | the two at full size, original left, port right |
| `v2-side-by-side-small.png` | the same pair halved, which is the one to actually look at |
| `v2-bike-*.png` | the same frame with each of the four 125 liveries the state has resident (see the livery gap below) |
| `v2-moment.txt` | the moment itself: the numbers the port was posed from |

## What I did

### The moment

The state is the human's own: `first_recording.p2m2_SaveState.p2s`, the save state PCSX2 wrote beside
I4's input recording, copied in from the main checkout (`context/` does not travel between worktrees).
It is 2,773 frames into a **Free09** freeride session with the bike standing still on the spawn, which is
the most useful possible moment for this comparison — nothing is mid-animation, so a difference in the
picture is a difference in the renderer and not a difference of one frame's timing.

`research/shots/moment.py` reads a moment out of the state's EE RAM and writes it as nine plain numbers.
What it walks:

- `0x004e1e98` the GameManager (R6) → `+0x30` the player manager and `+0x34` the actor manager, each
  `{count, head, tail}` at `+0x30`; a list node is `{item, prev, next}`.
- Of the nine actors, four are type 4, the rideable vehicle (R6), and each carries the player that rides
  it at `+0x20`. The vehicle of the **first** player in the player manager is the human's: its velocity
  at `+0x130` is (0.0001, −0.013, 0.483) where the other three are doing 20–40 ft/s, and the original's
  own speedometer in the frame reads 00000.0.
- That actor's `+0x180` is its 4x4 world transform — rows x, y, z, position, the position at `+0x1b0`
  with w = 1 — which is the pose the port puts the bike in.

### The camera, which is the half of this that had to be found

A side-by-side where only the bike's pose matches is not the same moment, it is the same place. The
frame's camera is a global at **`0x0046a540`**: a 4x4 with the eye in row 3, immediately followed at
`0x0046a580` by the view-projection of that frame *relative to the eye*. Decomposed, that second matrix
is the whole camera:

```
columns          direction                     scale
0  right     ( 1.0000,  0.0000,  0.0013)       1.0913
1  up        (-0.0004,  0.9494,  0.3141)       1.4551
3  forward   (-0.0012, -0.3141,  0.9494)       1.0
```

The scales are `1/tan(fov/2)`, so the camera is **85.00 degrees across and 68.997 down** — and
1.4551/1.0913 is 1.33333, i.e. the game projects its 640x448 buffer for a 4:3 screen. 85 degrees is
exactly the `FOV` of `cpresets.ini`'s first follow camera, which V1 read; the eye is 11.1 feet behind the
bike and 7.0 above it, 13.1 away, which is that preset's `FollowingDistance = 13`. So this global is the
chase camera after the game's own filtering, which is the thing the port cannot compute (S7 is unported)
and the reason the pair is posed from RAM rather than from the preset.

How I know it is that frame's camera and not a leftover: projecting the bike's own origin through it puts
it at pixel **(318, 305)** of 640x448 and the bike's seat (3.5 feet up) at **(318, 220)** — dead centre,
horizontally, which is where the original's screenshot has them.

### The port's side

`crates/openmx/src/moment.rs` is new and additive: `--moment FILE OUT.png` poses the shell from those
numbers and draws exactly one frame — no window, no pad, no simulation, no `ride.rs`. The bike's
placement is `M4::placement(pos, look, up)` fed with rows 2 and 1 of the original's own transform, which
is the same convention the renderer builds, so the matrix the port draws the bike on is the matrix the
original had it on. The camera is the moment's eye, direction and vertical field of view.

The port is handed **numbers, never addresses**: every address above lives in the Python script, in
`research/oracle.txt` and in this entry.

**The frame is 640x480, not the shell's usual 640x448, and that is the point.** The renderer's projection
divides the horizontal by `width/height`; at 640x448 that is 1.4286 and the port's picture is 3% wider
than the original's. At 640x480 it is 4:3 — the aspect the original's own projection was built with — and
the two frames become the same projection of the same scene, which is what makes them comparable pixel by
pixel. It is also exactly the size PCSX2 writes its screenshots at.

## What the original does

Nothing was decompiled for this task; V2 is a comparison, not a port. Two things were read out of live
RAM and are worth keeping:

- `0x0046a540` the render camera (eye in row 3) and `0x0046a580` the frame's view-projection relative to
  that eye. Both are in `research/oracle.txt` under `# V2`.
- A freeride area runs four vehicles even with one human: the state has five players, four type-4
  vehicles, and three of them are moving. The port draws one (S14's opponent is not wired into the
  shell).

## How it was proved

By eye, which is what the task asks for, with the geometry checked by arithmetic first:

- **The same moment**: both images come from one `.p2s`. The left one is the frame PCSX2 kept inside it,
  so it is that frame and not a frame near it. Re-loading the same state into kit copy 5 and reading the
  bike back over PINE gives (4309.478, 207.999, 3321.682) against the state's (4309.564, 208.030,
  3319.623) — the same bike, crept 2.1 feet down the hill in the three seconds the emulator ran before
  the screenshot, which is why the committed pair uses the state's own frame and `v2-original-live.png`
  is only a cross-check.
- **The same camera**: the projection check above, (318, 305) and (318, 220).
- **The same scale**: at 1:1 the bike occupies the same pixels in both frames (`v2-side-by-side.png`,
  centre), and the one prop both frames draw — the hanging billboard on the horizon — lands within a few
  pixels of its place in the original.
- `./kit check`: 355 tests pass, three of them new, over the moment file's reader.

## The differences, which are the deliverable

Original left, port right, in `v2-side-by-side-small.png`. What the port does not draw, in the order a
viewer notices it:

1. **No rider.** The bike is drawn, the rider on it is not: the rider model, its skeleton and its
   animation set are nobody's task yet. In the original the rider hides most of the bike, which is also
   why the livery below could not be settled by eye.
2. **No HUD.** Score plate, compass, speedometer and the `RACE` tower in the original's right third are
   the front end's; the shell has none.
3. **The dirt has no texture.** The port's ground is one flat brown lit by the terrain's own normals;
   the original's is a tiled dirt texture with a detail map and tyre tracks over it. V1 says the same:
   which tile a cell takes is not decoded.
4. **The ground stops.** The port draws terrain to 44 samples around the rider (`GROUND_RADIUS`), which
   on Free09's 9-foot grid is about 130 yards, so the far hills end in a hard edge against the fog
   colour. The original's terrain runs to the horizon through its own level of detail.
5. **No sky and no haze.** The original has a sky with cloud and a haze that greys the distance; the port
   fills with the scene's fog colour, so its horizon is flat grey and its middle distance is too clear.
6. **No shadows.** The original casts the bike and rider onto the dirt; the port has no shadow pass.
7. **Brighter, warmer dirt.** The port's ground sits several shades lighter and more orange than the
   original's, which is Lambert against the scene's sun and ambient with none of the GS's blend modes.
8. **Two trees and the tall banner structure are missing**, though the hanging billboard beside them is
   drawn in the right place. Both look like billboard or alpha-tested props (the state has
   `free09_billboard_back`/`_wood` textures resident); whether the port fails to load those models or
   draws them invisibly, I did not chase — V2 lists differences, it does not fix them.
9. **The near ridges differ in shape.** The dune the bike faces is in both and in the same place; its
   crest line is softer in the port, which is the terrain's own normals being used for lighting but the
   cell interpolation between samples being the shell's (P1 says so).

What matches, which is the useful half: the place, the pose, the scale, the camera, the field of view,
the horizon line, the hanging billboard's position, and the fence posts across the middle distance.

## Not verified / not 1:1

- **The livery is a guess.** The state has four 125 bikes resident — `cr125_faisst`, `cr125_thq_01`,
  `kx125_rs_01`, `yz125_renner`, one per rider in the level — and nothing I could find in RAM says which
  of the four belongs to the first player: the vehicle object holds no pointer to its own build name
  within 0x400 bytes either side of the actor record. The pair is drawn with `cr125_faisst`; the other
  three are in `context/shots/v2-bike-*.png` so the next person can look. The bike in the original is
  nine tenths hidden by the rider, so by eye does not settle it either.
- **The port's rendering is not compared numerically.** No per-pixel difference, no tolerance, no
  threshold: AGENT.md says rendering is matched by eye and explicitly says not to chase the last pixels,
  so this entry lists differences in words and the images are the evidence.
- **Only one moment.** One state, one track, one pose — a bike standing still. Nothing here says the port
  draws a moving bike, a jump or another track the same way. The two tools exist so the next subsystem
  can be shown the same way rather than argued about.
- **The camera's roll is dropped.** The original's camera is rolled 0.07 degrees in this frame (its right
  axis is (1, 0, 0.0013)); the renderer's `look_at` carries a fixed up axis, so the port's is level. At a
  tenth of a degree this is below a pixel at the edge of the frame.
- **`0x0046a540` is named from its contents, not from code.** I did not find the function that writes it;
  it is the camera *of the frame in this state* because the frame's own bike projects through it to the
  pixels the original's screenshot has. Another activity (a replay, a split screen) may keep its camera
  somewhere else, and `moment.py` would then quietly describe the wrong view.
- **The live screenshot drifts.** `v2-original-live.png` is three seconds of emulator after the load, so
  the bike has rolled 2.1 feet; it is a cross-check that the state loads and shoots in kit copy 5, not a
  second copy of the moment. PCSX2 cannot be stepped a single frame from the kit, so a drift-free live
  shot would need the emulator paused, which is a hotkey nobody here sends.
- **The original's 640x480 is PCSX2's.** The game draws 640x448 and PCSX2 writes 480 lines; the port is
  rendered at 640x480 so the *projection* matches, but the original's lines have been resampled by the
  emulator on the way out and the port's have not. Vertical detail in the left image is therefore the
  emulator's interpolation, not the game's.
