# 1 — v22 split screen (FINAL)

Task V22. 2026-10-20.

## What I did

- `--players N` (1 to 8) and `--split h|v` in the shell. The frame is cut into one view per player
  (`crates/openmx/src/split.rs`): one player is the full screen, two are the original's own two layouts
  (stacked for `h`, the default; side by side for `v`), three and four are quarters and five to eight
  are four columns by two rows — those two the port's own.
- Each player is a `Seat` in the shell's `World` (pad reports, own `Sim`, own `Garage`, own Start
  freeze), stepped in seat order inside the one `Engine::step`, all riding the one `Place`. Each has a
  `Rig` on the drawing side: own body and chassis animation (loaded again per player, as the first
  player's are), own `Race` camera (a clone of the first's after the preset pick), own `Hud`, and own
  copies of the two skinned models (`<name>@k`) because a skinned model's triangles are its pose.
- Every view is drawn by its own player's camera into a `Frame` of the view's own size and then put
  into the window's frame; the full screen's one view is drawn straight into the frame as before.
  `short_viewport` (h/w < 0.55) and `vertical_fov`'s aspect read the view's own extents, and the
  body's LOD reads the focal length of the view drawing it (re-skinned only when the level differs
  from the last view's). Every view draws every rider; spare views of a layout (3 of 4, 5–7 of 8)
  are black.
- Camera presets per layout: `cpresets.ini` and `biplanecpresets.ini` are read through the layout's
  own copy (`Preset{n}h`, `Preset{n}v`, `Preset{n}q`), with `--camera` / P14's pick-by-name applied on
  that set; a file with no copy for the layout falls back to the full screen's and says so. free06
  has all 11 `h` and all 11 `v` copies and both files have no `q`.
- The other players start beside the first, along the start line's right vector, alternating right
  and left, 6 ft apart, each set on the ground under them (the original's own second spawn is R9's).
- Pads: the first player has the host pad and the keyboard as always; each other player gets the next
  joystick device nobody has (`Pads::open_another`) or an idle pad. The tape, M's machine swap and
  the track's plane are the first player's; while a tape is ridden the others ride `Report::idle()`.
- `--headless` (no window, no host pad; draws for `--shot`) and `--frames N` (quit after N), so a
  test can run the shell itself.
- The end of a split session prints one line per player: ticks, position, view rectangle, camera
  position and triangles a frame. The fps line appends ", N views" when there is more than one; with
  one player it is unchanged.
- opentennis has no split screen, so nothing was taken from it.

## What the original does

- **Viewport setup** — `./kit fn 0x0020aaa0`: the layout word at `param+0x78` is 0 (one viewport, the
  whole screen), 1 (two viewports, full width and `h >> 1` tall, the second at `y = h >> 1`) or 2 (two,
  `w >> 1` wide and full height, the second at `x = w >> 1`). It loops over at most two viewports and
  sets each one's near plane (2.4). Nothing in it lays out more than two.
- **Which preset copy a camera reads** — `./kit fn 0x001fbf50` (the camera preset loader): it formats
  the section name by the camera's viewport index (`+0x174`). If the display class word at
  `0x004211a0` is not 3: index 0 → `Preset%d`, 1 → `Preset%dh`, 2 → `Preset%dv`, any other →
  `Preset%dq`. If it is 3: 0 or 1 → `w`, 2 → `x`, any other → `z` (format strings at
  `0x0042d380`–`0x0042d3e0`). The index is the layout number: `./kit fn 0x00200960` with
  `(cam, layout, 1)` reaches `./kit fn 0x0021f6e0`, which writes `+0x174 = layout` and reloads the
  presets.
- **The display class** — written by the video-mode setter `./kit fn 0x00312410`: modes 1 and 2 → 2,
  modes 3, 4 and 5 → 3. So `w`/`x`/`z` are **not** quarters: they are the same three layouts' presets
  for the other display modes (`w` for both full screen and stacked, `x` for side by side). The brief's
  "four as quarters, the original's `w`/`x` if R9 says so" therefore resolves to the port's own
  quarters; the shell is not one of those display modes and reads the first set. `q` is what any
  index past 2 reads, so the port's own layouts (index 3 and up, conceptually) read `q`.
- So `h` is the stacked split ("split horizontally": two wide strips) and `v` the side-by-side one —
  the opposite of what one might guess from "vertical" alone, and the reason `--split h` stacks.

## How it was proved

- **One player draws exactly as today**: a 90-frame idle tape recorded headless, then played at
  640x448 with `--shot-frame 80` by this branch (headless) and by HEAD (built in a scratch worktree,
  windowed on :0). The two PNGs have the same sha256 (`4f7f501f7a516f51…`) and the ride hash is the
  same (`0xaf4de72f5f10bd74`).
- **Four players step and draw four views**: `a_four_player_session_steps_and_draws_four_views` in
  `crates/openmx/tests/ride.rs` runs the built shell (`--headless --players 4 --size 320x224 --frames
  60`) and checks its last lines: every player stepped ≥ 60 ticks, each view is a 160x112 quarter at
  its own corner and drew triangles, each camera is 1–40 ft from its own rider and at the same offset
  behind each of the four (within 3 ft), and the four riders are in four places. It ran (its shot
  under `target/tmp/` was fresh) — not a skip.
- `split.rs`'s unit tests: the original's two halves at 640x448 are the exact rectangles its setup
  computes; every layout tiles the frame exactly once at even and odd sizes; each layout reads the
  loader's suffix; `put` lands a view at its place.
- **By eye** (`--shot`, headless, 640x448, frame 100; files under the git-ignored `context/v22/`):
  - `one.png` (with `--hud`): as before.
  - `two-h.png`: two 640x224 strips, each its own HUD with its own speed (1.9 and 1.5 mph), each
    camera behind its own rider with the other rider beside.
  - `two-v.png`: two 320x448 halves, the same.
  - `four.png`: four 320x224 quarters, each behind its own rider; with `--hud` the full-size debug
    overlay covers most of a quarter (see below).
- **fps at the window's size** (the tiling WM sets the window to 936x1018, so the frame is 936x702;
  `--seconds 8`, steady-state lines; the machine was shared with the other slots, load average ~15 on
  32 threads):

  | views | fps | triangles a frame |
  |---|---|---|
  | 1 | 28–31 | ~21 000 |
  | 2 (stacked) | 22–24 | ~51 000 |
  | 4 (quarters) | 13–14 | ~145 000 |
  | 8 (eighths) | 10–10.6 | ~378 000 |

  Four views do **not** fit in 60 Hz: ~13 fps at 936x702 (12.5 at 640x448 headless). One view does not
  either on this loaded machine (~30). Nothing the simulation reads was lowered; the engine's clock
  makes a slow frame a longer step, as it always has. The triangles grow as players squared, because
  every view draws every rider's body and bike: that and the per-view ground pass are where a cut-back
  (V23) would start.

## Not verified / not 1:1

- **The original's second-rider spawn** — not read (R9 is open). The 6 ft side-by-side offsets along
  the start line are the port's own.
- **The default two-player split** — which of `h`/`v` the original starts a two-player game in, and
  how the player picks it, was not read; `h` is the default only because it is layout 1.
- **The original's per-viewport HUD and its cut-backs** (draw distance, LOD, effects, AI) with two
  views up — not read (R9). The shell's HUD is its own debug overlay drawn at full pixel size into each
  view, so in quarters and eighths it covers most of the view; making it fit is V23's.
- **The `w`/`x`/`z` copies** are not used: they belong to display class 3 (video modes 3–5), which the
  shell does not model. That the class word is only ever 2 or 3, and which mode the retail boot sets,
  was not checked against a running game.
- **`q` copies** — free06's two camera files have none, so the quarters and eighths ride the full
  screen's presets; no track was searched for one that has them. That the port's own layouts read the
  loader's "any other index" copy is a choice, not something the original does with them.
- **Odd extents** — the original halves by a shift (one pixel short on an odd extent); here the last
  view takes the remainder. Same on every even size, which every PS2 mode and `fit_frame` is.
- **Riders do not touch each other** — reason 7 (`Joust`) and any rider-rider contact are not ported;
  seats are stepped in order and never see each other.
- **Extra players' pads** were only tried as idle pads: no second joystick is attached here, so
  `Pads::open_another` picking a second device is untested on hardware.
- **Tapes carry the first player only**; with a tape the others ride idle. The track's plane, M's swap
  and the keyboard are the first player's alone.
- **No capture or save state of the original's split screen** was compared; the views are checked by
  eye and by the shell's own numbers only.
- **`near` per viewport** — the original's setup sets 2.4 for each of its viewports, which is the
  shell's `NEAR_CLIP` already; every view uses it. Whether anything else the setup writes per viewport
  (beyond extents and near) matters was not followed.

## Next

- R9 should read the second spawn, the default split, the per-viewport HUD and the cut-backs; V23 makes
  eight (and four) readable and fast.
