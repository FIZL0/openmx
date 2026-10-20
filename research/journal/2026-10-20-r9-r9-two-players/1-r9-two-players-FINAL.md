# 1 — r9 two players (FINAL)

Task R9. 2026-10-20.

## What I did

Read the two-player game out of the decompilation (`./kit fn` / `./kit xref` / `./kit dis`) and the disc
(the camera ini, the UI layers, the track archives, read with a scratch reader built on
`game_render::disc`). I did not start the emulator: there is no two-player save state to load, and
getting into a split screen from the menu needs two pads, which the kit's virtual pad can't provide
(`/dev/uinput` has been root-only since 10-19). All the names and addresses are in `research/oracle.txt`'s
new **R9** section. I also corrected S7's camera_load_presets note. `plan/HUMAN.md` now asks for two
two-player states, in slots 6 and 7.

opentennis: I took nothing from it. This task is research on the original, and opentennis has no split
screen to compare against (it has seats and pad hot-plug, which are P20's business, not this task's).

## What the original does

**Layouts.** The engine has three split modes, held in the settings' viewport-mode object: 0 is one
view, 1 is *horizontal* (two full-width views, one above the other) and 2 is *vertical* (two full-height
views side by side). The boot config's `ViewportMode = Horizontal | Vertical` sets them, and so does the
front end. `./kit fn 0020aaa0` gives every human player a viewport and gives each one the frame
buffer's rectangle for the mode. It also tells each player's camera which mode it is in, and that
choice is what picks the preset copy.

**The four suffixes.** These are not viewport indices; S7's note was wrong about that. The camera's
split mode and the display's aspect together pick the suffix:

| | mode 0 (1P) | mode 1 (horizontal) | mode 2 (vertical) | anything else |
|---|---|---|---|---|
| 4:3 | `Preset%d` | `h` | `v` | `q` |
| 16:9 | `w` | `w` | `x` | `z` |

The disc's own cpresets.ini header says the same thing: "1 Player, 2P horiz, 2P vert, 1P wide (16:9),
2P vert wide". So `w` and `x` are the widescreen copies, not quarters. The `q` and `z` slots show the
loader was built for a four-way split, but no file on the disc has those sections, nothing sets a mode
above 2, and only the vertical split has a widescreen copy of its own.

**What the presets change.** Preset1h has AngleX 30 (65 in 1P), FollowingDistance 16 (13), FOV 80 (85)
and LookAhead 15, and is pitched flatter and pulled further back for a wide, short view. Preset1v keeps
65/13 and narrows the FOV to 60, with CameraSwing 2.5 and LookSwing 2.0.

**short_viewport is the split screen's.** In the race camera's aim, the 0.33 × AngleX pitch trim
applies only when the display is not 16:9 and the *view's* own height over width is under 0.55. At
640×448 that is only the horizontal split's 640×224 (0.35). Full screen is 0.7 and the vertical split
is 1.4. There is one more gate, a word on the scene at +0x78, which I could not name. **For V22:** the
port computes this from the window (`crates/openmx/src/main.rs:1350`), so a single-player window wider
than 1.82:1 trims when the original never would. It should come from the view's layout (true only for
a 4:3 horizontal split), not from the pixel shape.

**Pads.** Human player *i* is created with pad index *i* and bound to the input manager's device *i*.
That is fixed at spawn and there is no reassignment code on that path. AI riders get pad −1.

**Where the second rider spawns.** Not found. The scene has a single
`DefaultStartPosition`/`DefaultStartDirection` (`./kit fn 001a6f50`). The spawn loop
(`./kit fn 00207b20`) creates the players and places no one. The scene's gate sections
(`./kit fn 002af2e0`) carry trigger boxes, not lanes, and the race's start must come out of the binary
`.tdf` track file, which I didn't decode. The slot-7 state asked for in `plan/HUMAN.md` answers this
directly. What I did find: in freeride, two players means *no AI* (NumberOfRacers is 4 with one player,
otherwise the player count). A race keeps its AI, NumberOfRacers − humans of them.

**HUD per viewport.** The UI layer format has a Viewport bitmask for viewports 1–4, all, or full screen,
but no in-game layer on the disc uses it. Instead, `mxingame.lod` has MultiplayerHorizontal and
MultiplayerVertical groups whose layers come in per-player copies (…1, …2) laid out in full-screen
coordinates: the info text, the racing overlays (time, lap, position, countdown, flags), the stunt score,
the speedometer and, in freestyle, the radar, start menu and point list. One map, the alert, the hit
lights and a split-screen separator bar are shared. The final-standings menu, the demo and ghost layers
are left out. The oracle lists every layer.

**Cut back with two views up:**
- Freeride loads a separate `_ss` cut of the area: no ruts, no crow, no loop sound, no detail particle
  set, its own terrain and fewer props.
- Shadows reach 65 ft (fading from 55) instead of 100 ft (fading from 60).
- Outside supercross, the fog's far distance becomes MaximumFogDistance × 0.61.
- A freeride has no AI riders.
- The vehicles have `_ss` models, but I didn't confirm that the split is what picks them.

**Two riders touching.** This is the contact clause S37 already ported (`air::hit_wreck`), and nothing
in it is specific to two players. When a bike's ray sweep hits another actor, it writes its own entry
and a mirrored one into the other actor's record. The wreck test reads only its own entries. If the head
ray hits a rider whose chassis is more than JoustWreckHeight (2.5 on the 250 bikes) higher, that is
reason 7 for the rider underneath, thrown with the other bike's velocity. The rider on top rides on.

## How it was proved

It wasn't proved. This is static reading of the decompilation and the disc files, with nothing run.
The cpresets.ini mapping is the strongest finding: the loader's branches and the disc file's own
comment header agree.

## Not verified / not 1:1

- **None of it has been seen running.** There is no two-player save state, and reaching one needs two
  pads, while the kit's virtual pad has been unusable since 10-19. `plan/HUMAN.md` asks for slots 6 and 7.
- **The second rider's start position, in freeride and in a race.** I didn't find it. The spawn loop
  places no one, the scene has a single default start, and the gate sections hold no lanes. The race
  start is probably in the binary `.tdf`, which I didn't decode.
- **The scene word at +0x78** that also disables the short_viewport trim: its meaning wasn't found.
- **How the front end sets the split mode** (SplitScreenOptions / STR_DYN_SPLIT_SCREEN, read at
  0x003b98e0): not traced. Only the boot config's ViewportMode key was read.
- **Pad reassignment:** only the spawn path was read. A front-end swap of the input manager's device
  slots before spawn would not show there.
- **q / z:** I didn't run the loader against a missing section. "Left as it was" comes from reading the
  skip.
- **The fog × 0.61:** I read the write but didn't look at how the renderer uses the far value. "Draw
  distance" is an inference from the field's name.
- **The `_ss` vehicle models:** I didn't read what picks the skin index.
- **The actor query** (0x0011a150 / 0x0011a9d0) that finds another bike for the ray sweep: not read. So
  whether two humans' bikes can see each other there, the way AI bikes can, is assumed, not shown.
- **The HUD's per-player positions:** I listed the layers by name but didn't read their coordinates.

## Next

Once slots 6 and 7 exist: load each with `./kit state ram` and read the two players' chassis positions,
each view's rectangle and its camera's cam+0x174, and confirm the mode, the preset suffix and the
spawn spacing. Then decode the `.tdf` start segment if the race spacing doesn't come out of the RAM.
