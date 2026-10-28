# 1 — sun flares (FINAL)

Task V72. 2026-10-28.

## What I did

- `game-render/src/sun_flare.rs`: reads a `.toy`'s `[SunFlares]` count and `SunFlare1..n` sections with
  the original reader's defaults, and builds each flare's picture as an additive camera-facing quad
  (`glow::Glow`) at the sun's screen point, `2·flareScreenSize` px of a 448-line frame, alpha
  `flareAlpha/128` times the share of the sun not hidden.
- `openmx/src/main.rs`: a disc track loads its flares and `tex\sunflareNN.pst` pictures into the glow
  list, and each view draws the flares whose sun point is in view, on both renderers. Hiding is five
  rays over the original's sample box, marched against the sim's ground, once a sim tick per view.
- Tests: six unit tests in `sun_flare.rs`; `game-render/tests/sun_flares.rs` reads every track
  archive's `.toy` off the disc (56: the 51 tracks and five freeride `_ss` scenes; 43 flares drawn).
- The brief's "57 scenes" counted to 56 `.toy`s here; the zero-flare list is as the brief says, and
  free10_ss has none too.

## What the original does

See research/oracle.txt "V72". The reader sets defaults, then overrides from the section; with
useToyFlareSettings on it overwrites the `.psm` OcclusionFlare filter's sample size, colour, alpha and
size. The disc uses `FlareRGBA`, which nothing reads, so every flare is colour 128 (white). Each frame
the sun point is the camera minus the light direction times 20 ft; if it clips into the frustum a z-buffer
sample box of occlusionSampleSize px is queued, and the flare sprite drawn centred on the point.

## How it was proved

Looked right by eye only: `--shot-pose` shots of free06 toward the sun on the GPU and software
renderers show the flare in the sky; a pose behind a hill shows none. The disc survey test checks the
reading on all 56 `.toy`s. No side-by-side against the original.

## Not verified / not 1:1

- No comparison with the original's picture: no save state here looks toward the sun (all are free09 /
  free06 follow cams), and no capture was made.
- Hiding is the port's own: rays against the sim ground, not the original's z-buffer sample. Props
  without collision do not hide the sun, and the 15 % ray steps can skip a thin ridge.
- Hiding is worked out once a sim tick, not every frame (cost: 2.7 ms a frame uncached).
- `Saturation` and SaturationMin/MaxColor (per-zone screen tint) are not drawn: their consumer was
  not found.
- MinScale, MaxScale, Width, Height, VertexColor, RetrieveMaterial are read by the original but not
  used here (MaxScale is unread by its init anyway); the `+0x40` centredness value was not found in the
  PS2 draw, so no fade toward the screen edge.
- The blend is taken as additive from the picture (white on black, opaque alpha); the GS blend
  register was not decoded.
- Size assumes 640×448 with square sprite pixels; the 2× sprite size of flareScreenSize is read from the
  draw, not measured on screen.
- useToyFlareSettings = 0 (keep the `.psm`'s values) is not ported; no disc section uses it.
- The sample box is not clamped at the screen edge as the z-sample would be.
- World mods get no flares.
- Key matching is case-insensitive as ini.rs does; the original's case rule not checked.
- Nothing was taken from opentennis.

## Next

Nothing.
