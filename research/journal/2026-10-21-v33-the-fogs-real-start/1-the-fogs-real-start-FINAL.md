# 1 — the fog's real start (FINAL)

Task V33. 2026-10-21.

## What I did

- `scene.rs`: `[Fog]`'s `Visibility` and `Haziness` are parsed now (`Scene::fog_visibility`,
  `fog_haziness`; the original's defaults 1 and 0.5), and `fog_range(min, max, visibility, haziness)` /
  `Scene::fog_range()` give the original's fog start and end. All four fog numbers go through
  `game_sim::phy::number` (the original's `strtod` + truncating double-to-float, ported in the sim
  already), because `Haziness=0.60` is 0x3f199999 in the original, not Rust's 0x3f19999a, and that is
  2 ulp at the fog's start.
- `world.rs`: `Lighting::of` takes `fog_near`/`fog_far` from `fog_range()` instead of the raw minimum and
  maximum, so `Graphics::Original`, its far clip, `Full`'s haze (which starts at the original's end), the
  software rasteriser, the ground and the props all follow. The printout in `openmx` (`the disc's fog
  a..b`) now prints the worked-out range.
- Test fixtures that built a `Scene` by hand or relied on 25..1000 being drawn as 25..1000 are updated
  (gpu.rs `fogged()` is free07's whole `[Fog]` now, 280.8..1000; its known scene fogs 6..70).
- `research/oracle.txt`: named fog_new, fog_apply, fog_tick, fog_to_view, renderer_set_fog,
  scene_fog_read beside the existing fog_set.
- Nothing taken from opentennis: it has no scene fog of this kind.

## What the original does

- The scene reader (`./kit fn 0x001a8a50`) reads `[Fog]` into the scene's fog record: ColorRGB,
  `Visibility` (default 1.0), `Haziness` (0.5), `MinimumFogDistance` (64), `MaximumFogDistance` (768),
  each through `0x0014b790` = strtod + double_to_float.
- The fog object's constructor (`./kit fn 0x0012f4d0`, called from `0x001a4dec`) is handed
  (Visibility, Haziness, Max, Min, **64.0**, object, colour); the 64.0 is an immediate (`lui 0x4280`),
  also passed by every split-screen reset in viewport_arrange (`fog_set`, `0x0012f940`).
- `./kit fn 0x0012f6a0` (fog_apply) clamps both fractions to 0..1 and works out
  `end = near + vis*(far-near)` (+0x38) and `start = (1-haz)*(end-64)` (+0x34).
  In recording2's save state the object (at scene+0x420) holds vis 1.0, haz 0x3f199999, far 1000,
  near 10, 64 → start 374.40002 (0x43bb3334), end 1000: exactly the renderer's pair. So the "near"
  that moved was never moved: `MinimumFogDistance` only matters when `Visibility` < 1 (nat08 alone,
  0.5 → its fog ends at 375).
- `./kit dis 0x0012fb00 --len 0x50` (fog_to_view): sets the view's far clip (`0x00112aa0`) to the end
  and calls the display object's vtable +0x3c with (start, end).
- `./kit dis 0x00179ea0 --len 0x40` (renderer_set_fog): +0x48 start, +0x4c end, +0x7c = 255/(start-end).
  That is a linear ramp, GS FOG 255 (clear) at the start, 0 (all fog colour) at the end; the per-model
  setup uploads +0x4c and +0x7c for VU1. A model whose nearest point is nearer than the start
  (display+0x48 against distance+radius) is drawn with fog off — same picture as a ramp clamped at 0.
- fog_tick (`0x0012f970`) eases colour and both fractions towards a target when a blend time is given;
  only the zone path of `0x001a6460` (FogColorRGB/FogVisibility/FogHaziness, under a flag at app+0x36c)
  uses it. No `.scn` on the disc has any of those zone keys (surveyed with a throwaway test).

## How it was proved

- `crates/game-render/tests/fog.rs` `free09s_fog_is_the_save_states`: free09's `.scn` read off the disc,
  its fractions bit-equal to the save state's fog object (0x3f800000, 0x3f199999), `fog_range()` bit-equal
  to the renderer's pair in the RAM image of `recording2.p2m2_SaveState.p2s` (display_object +0x48/+0x4c),
  and the view's far clip equal to the end. It ran (it failed twice on its own asserts, at 2 ulp and at
  the truncated fraction, before passing). Needs the RAM image: `./kit state ram` of recording2's `.p2s`
  (copied from the main checkout's context/recordings).
- `scene.rs` `the_fog_starts_where_the_haziness_puts_it`: the same free09 numbers without the disc, bit
  for bit; nat08's `Visibility` 0.5; the clamp (above 1, below 0, NaN); missing keys' defaults.
- `gpu.rs` `the_original_is_the_discs_fog_and_far_clip` on free07's `[Fog]`: 280.8..1000.
- `./kit check`: passed.

## Not verified / not 1:1

- **One save state.** The formula is shown bit-exact on free09 only (Visibility 1). nat08's Visibility 0.5
  path (end = min + 0.5·(max-min)) is read from the disassembly, not checked against a nat08 state —
  no save state on nat08 exists here.
- **The fog's shape on screen.** Linear from start to end, opaque at the end, is read from
  renderer_set_fog's 255/(start-end); the VU1 microprogram that uses it was not read, so whether the
  original measures the eye's distance or the view depth, and how the GS's 8-bit fog quantises it, is not
  checked. The port keeps V32's radial distance. Rendering is match-by-eye; no screenshot was compared.
- **Zone fog** (fog_tick's eased blend, the zone keys) is not ported: no disc scene uses it, and
  `Graphics::Original` takes the scene's fog only.
- **Split screen's 0.61** (two views outside supercross scale `MaximumFogDistance` before fog_set) is still
  not wired in the port; `fog_range`'s docs say to pass `0.61·max` when it is.
- **A scene with no `[Fog]` at all** keeps the port's old 5..2000 fallbacks for the distances; what the
  original's record holds then is not checked (every scene on the disc has the section).

## Next

Nothing for V33.
