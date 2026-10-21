# 1 — the originals overlay (FINAL)

Task V26. 2026-10-20.

## What I did

- `crates/openmx/src/overlay.rs` (new): the original's own HUD over each view, drawn from
  `fstyleui.res`'s art and laid out by its layouts.
  - **Speedometer.** One player gets the analog dial `speedometer_analog` with its needle, the
    needle's pin, the odometer plate, the dim `00000.0` and the miles over it. Two or more players get
    the digital readout alone (`yearlingdashboldoblique_55`), because the original forces digital at
    two players.
  - **Radar.** The `freestyle_radar` rose turns so the rider's heading is up. The yellow arrow stays
    fixed in the middle. Every other rider is an 8x8 cyan `map_icon_hit_freestyle` at their bearing
    and distance.
  - **Pure functions** `speed_text`, `needle_degrees`, `odometer_text`, `radar_mark` and `radar_turn`
    take plain numbers, so they can be tested without a disc. `Art::draw` returns a `Drawn` saying
    what it put where.
- **Layout per view.** The parts keep full-screen size (the window's `min(w/640, h/448)`). The radar
  counts from the view's bottom-left and the speedometer from its bottom-right. A view smaller than
  311x150 layout px shrinks them to fit; no layout V22 makes does that today.
  - This follows the original's own two-player layouts, which keep the parts full size in a
    half-screen view (`freestyleradarhorizontal1`: rose at 87,155; `speedometerhorizontal1`: readout
    at 588,155).
  - It also works for V24's any-aspect windows: the parts go to the corners and nothing stretches.
- **The `--overlay original|debug|none` setting.** It is also P36's `settings.txt` key `overlay` and
  the Settings screen row, which now cycles Original/Debug/None. The default is `original`.
  - Old files still load: `on` reads as debug, `off` as original. `--hud` is kept as
    `--overlay debug`.
  - H still toggles the debug HUD. While it is up, the original's HUD steps aside; with `none`,
    nothing draws until H.
- `menu.rs`: added the `Overlay` enum. Its `Font`, `Glyph` fields, `span`, `blend` and `bilinear`
  are now `pub(crate)` so the new module can reuse them; nothing was moved.
- `main.rs`:
  - `mod overlay`;
  - the art is loaded once per ride (a disc without it rides on with no HUD);
  - `radar_span = ground.spacing * 192`;
  - a per-rig `Odometer`;
  - the draw call after the debug HUD, in each view's own frame;
  - the args and usage text.
- `tests/menu.rs`: the two `overlay: true` became `Overlay::Debug`. The test's Cross on the row now
  cycles Original→Debug, so it lands on the same state as before.
- `tests/overlay.rs` (new, 6 tests).
- `research/oracle.txt`: the V26 block of addresses.

## What the original does

- **Speedometer.**
  - `./kit fn 2ac400`: mph = speed × 0.68182, floored at 0 and capped at 105 by bit selects, before
    the needle is posed.
  - `./kit fn 2acbc0`: the needle's angle is `132 − 2.64·mph` degrees. So 0 mph is at the dial's lower
    left, 50 straight up and 100 at the lower right, which matches the art.
  - `./kit fn 2ac580`: the readout is `"%.0f"` of the same mph. The odometer is the statistics
    tracker's distance × 0.00018939393, printed `"%07.1f"`, or `00000.0` under a tenth of a mile.
  - The layer: `speedometer1.lyr` with `speedometer.lyt`'s positions. The needle is the 3-D model
    `ui\geom\speedometer_needle.psg` (tip 40.4, tail 12.1, half-widths 0.7 and 3.6, pivot at the
    origin), drawn 55 px across at 536,357, which is texel (56,56) of the dial: its pivot.
- **Radar.**
  - `./kit fn 35d3e0`: the world it covers is `W = max(terrain+0x538, +0x53c) × 192`, and those are
    the terrain's sample spacings (see `biplane.rs`'s `map_extent`). The rose turns by `2π − heading`,
    with heading `acos(f.z/|f.xz|)` reflected for `f.x < 0`.
  - `./kit fn 35e4d0`: each other rider sits at `r = min(d, W/2)·104/W` along `a = bearing(rel) −
    bearing(f)`, at `(87 + r·sin a, 363 − r·cos a)`. Humans are coloured `00 ff ff ff`, AI `00 00 ff
    ff`.
  - The rose's ring has a radius of 52 texels, which is exactly `W/2·104/W`.

## How it was proved

- **Looked right** in headless shots, by eye against the disc's own art:
  - 640x448, one player at speed under a hand-written gas tape (`--play`): the needle sits on the
    dial's marks and the odometer digits lie over their ghost;
  - 1024x448: the parts stay in the corners;
  - four players: digital readouts and a radar in every view.
  - The shots are not in the repo (game data).
- **`tests/overlay.rs`**, run through `./kit check -- -p openmx --test overlay`, has 6 tests. The two
  disc tests were proved to run (not skip) by a temporary panic after their asserts, which reported 8
  marks pixel-checked; the panic was then removed.
  - `the_speedometer_reads_the_rides_speed`: a stock-bike `Sim` launched on a flat floor. Over 37
    batches, the speed the HUD is handed is within 5% of the distance the bike actually covers a
    frame. The text is the whole mph, and the needle sits on the dial's 0/50/100 marks and stops at
    105.
  - `one_rider_is_the_arrow_in_the_middle`: one player. A bike steered right turns the rose by
    exactly `2π − heading`, and the rider itself sits in the middle.
  - `a_rider_steered_right_shows_on_the_right`: the radar's right is the rider's right. A bike steered
    right on the pad comes out right of the radar's middle, and one steered left comes out left, both
    ahead. This is what pins the handedness, rather than assuming +x is right.
  - `four_riders_are_marked_where_they_are`: four riders ridden apart. In every view, all three
    others' marks match a second derivation from dot products with the viewer's own forward and
    right, to 0.01 px. Riders out of reach sit on the ring.
  - `the_views_draw_the_riders` (disc): the real art drawn.
    - One player: the needle at the ride's speed, the odometer text, no marks.
    - Four 320x224 views: each readout at its rider's speed; each view's marks at the derived
      positions in the view's pixels, and cyan in the frame where they are clear of the arrow.
  - `a_wide_window_puts_the_parts_in_its_corners` (disc): 1280x448, 640x896 and 1920x1080 put the
    arrow at the bottom-left at the window's scale and the dial in the bottom-right.

## Not verified / not 1:1

- **The speed pointer.** I did not find in RAM what `**(iface+0x1c)` points at. In `rec-start` the
  candidate block held rider+0x88c (0.0) at +0x1c and rider+0x860 (the chassis' +0x34 speed) at +0x20
  and +0x34. The port shows `chassis.speed` (the camera target's speed, the plane's when it is up). If
  the original reads another number there (for example the 0.8× capture copy), the readout is off by
  that much. Reason: ran out of the timebox.
- **No side-by-side with the original.** It was drawn by eye against the art, not compared to a
  screenshot of the original's HUD; the emulator was not started. Screenshots from this desktop are
  unreliable (the lock screen), and a matched-state capture of the HUD would be a task in itself.
- **The needle is a 2-D tapered bar, not the 3-D model.** It is in the model's proportions, in a flat
  orange (255,153,0) worked out from vertex 127 × colour scale (2.0,1.2,0.0), unlit.
  - The pin is the pin texture on a 10 px disc; its mesh is not round in the files I read, and the
    size is by eye.
  - "55 px" is read as 1 model unit = 1 px, from the model's bounding radius of 27.3 (55 across).
- **Right justification** puts the text's last character at X. This is inferred: it is the only rule
  that lays the odometer's miles (X 573) over its ghost (X 501) in a 12 px monospace font. The digital
  readout follows the same rule unchecked.
- **The odometer** adds up the rider's 3-D path a frame at a time, from the start of the ride,
  skipping jumps over 100 ft. The original's statistics tracker (+0x78) was not read: what it sums,
  when it resets, and whether it carries over between sessions are all unknown.
- **Analog or digital for one player.** The original's option (value 1 = digital) was not traced to a
  default. The port shows analog for one player because the layer lists it; there is no
  digital-for-one setting.
- **Split-screen layouts.** The original's own `…vertical1/2` and `…horizontal1/2` layouts are not
  used. Each view uses the full-screen layout from its corners at full size, which is close to them but
  not their exact numbers (e.g. horizontal1's readout is 69 lines above a half-view's bottom; here it
  is 119 − the font's height). The original never had four views, so four has no reference.
- **Radar forward.** The port uses the target's forward row, as the camera gets it. The original
  switches to the normalised velocity under a flag I did not identify (possibly the airborne one).
- **Mark colours.** Every rider in the port is a player, so every mark is the human cyan. The
  byte-order reading (RGBA, giving cyan and blue) is mine, and AI blue is never drawn.
- **Draw order.** Rose, then marks, then arrow. The layer's component list puts Player Icon first;
  the original's depth order was not read.
- **Not ported:** the radar's gate and activity icons (free ride has none in the port), the race map
  (`racingmap.lyt`, race mode), the PreloadMeter flash, the metric setting, and the rose's alpha /
  blend mode beyond the texture's own alpha.
- **The plane** (seat 0 with the biplane up): the readout and radar take the plane's position,
  forward and speed from `chase::ridden`. The original's plane HUD was not looked at.

## Next

- Pin `**(iface+0x1c)` under PINE while riding (read the speedometer's object from the layer and
  compare its digit string to rider+0x860 at speed).
- Compare one PCSX2 frame of the free ride HUD with the port's at the same aspect.
