# 1 — mxatrack tracks (READY)

Task MD259. 2026-11-21.

## What I did

- `crates/stitch/src/mxa.rs`: our own reader for MXA Track Studio's `.mxatrack`, written from the format
  (context/mxa read as reference only; no code taken from it).
  - The package is a zip: `manifest.json`, `track.json` (the track document), `collision.msc`, and optional
    editor files we don't need. Stored and deflated entries are read, with CRCs checked.
  - Checked as MXA checks: manifest version 1 in feet, the collision's SHA-256 against the manifest,
    the document's ranges (schema 1–8, size 32–32768 ft, resolution 65–8193, environment, surface 0–15),
    and the collision's header, grid, bounds, surface mask, padding and mesh chunks to the exact end.
  - The ground ridden is the collision's baked heights. They become a terrain via trnwrite.
  - `write_mod` writes `tracks/mxa_<id>/` (mod.json, `.scn`, `.ps2.trn`) through a staged `.part` folder.
  - `package` writes a test `.mxatrack` from a `Track`. Every test uses it.
- `crates/openmx/src/track_mods.rs`: `import_mxa` makes each dropped-in `tracks/*.mxatrack` a track mod.
  `scan` (the front-end list) and `find` (`--track`, lobby) call it.
  - mod.json keeps a stamp (file name, size, time), so a file already made is not read again.
    A file saved over is made again.
  - A second file with the same track id is left out with a line while the first file is there.
- Placement: MXA's frame has 0 in the middle of the track and no axis flip. So x + width/2,
  z + depth/2, and y lifted by the lowest height when that is below 0. The start is the
  collision's spawn moved the same way. Its direction is the spawn direction flattened.
- A grid with cells that aren't square, or one finer than 4097 samples a side, is sampled again on
  square cells at the finer spacing (or coarser, to fit), blending between samples.
- Glossary: "MXA track".

## What the original does

MXA Track Studio bakes the track's ground into `collision.msc`: a header with bounds and spawn, a
height grid with x varying fastest, an optional surface byte per sample, then mesh chunks (the ground
in 64-cell chunks, then props). Sample (i, j) sits at (−w/2 + i·w/(N−1), h, −d/2 + j·d/(R−1)).

## How it was proved

- `crates/stitch/tests/mxa.rs` (4 tests):
  - A package read back gives the same track, with and without surfaces and with extra document fields.
  - 9 kinds of broken package are refused, each with its reason.
  - A 257×257 track written as a mod reads back through `stitch::openmx::track`. Every sample's height
    is within one height step (top/65535). The surface is right. The start is mapped and lifted.
  - Resampling: rows on the grid keep their heights, rows between are blended, and 8193 samples go
    down to 4097.
- `track_mods.rs` unit test: made once, not read again, a duplicate and a broken file each left out
  with a line, made again after being saved over.
- `crates/openmx/tests/mxa_track.rs`: only the `.mxatrack` file goes in `mods/tracks/`. Then
  `openmx --headless --track mxa_md259ride` makes the mod and rides 150 ticks on the gas. The bike
  starts at the mapped start, rides more than 100 ft along +x, stays on the package's ground
  (−1 to +12 ft) and never goes down. It ran here with the disc (not skipped).

## Not verified / not 1:1

- **No real `.mxatrack` tested.** None exists on this machine, and Track Studio isn't installed.
  Every test package is made by our own `package`. So a misreading of the format would be made the
  same way on both sides. Final check: a track the person makes in MXA.
- **Surface ids.** MXA's surface number 0–15 is used as the MXU surface byte unchanged. Nobody has
  checked that the two numberings mean the same ground.
- **Ground texture not drawn.** The terrain is drawn grey. The mod's `todo` says so.
- **Props not laid out.** Prop meshes in the collision are checked for shape but not placed. The
  mod's `todo` says so.
- **Route, spawn yaw from the document.** The start is the collision's baked spawn, not recomputed
  from `spawnX/Z/Yaw`. The route is ignored.
- **Resample edge.** When a side isn't a whole number of samples at the new spacing, up to one
  spacing is dropped at the far edge.
- **Small tracks.** MXA allows 32 ft. A map under about 300 ft is all inside the edge's 150-ft
  band, so the bike is thrown every tick (as with any small map).
- **Stadium and venue.** `venueId` and the Stadium environment are read but not used. A Stadium
  track rides as a plain freeride, with no stadium around it.
- **Bounds slack.** The collision's bounds must match ±width/2 and ±depth/2 within 0.01 ft, and
  heights must sit inside the bounds within the same slack. A guess at MXA's float rounding, untried
  on a real file.
- **Repeat lines and leftovers.** A broken or clashing file is read and logged again at every start.
  Deleting an `.mxatrack` leaves its mod in place. Two MXA ids that differ only in punctuation share a
  mod id, and the second is left out.
- **First list after a drop is slower.** The front end's list builds the terrain the first time it
  sees a new file (once).
- **v1/v2 chunk rule.** In collision versions 1 and 2 the ground chunks come first. We check every
  chunk's structure, not that ground count.

## Next

The person: make a track in MXA Track Studio, save the `.mxatrack`, and put it in `mods/tracks/`
(or in context/mxa for an agent). Then ride `mxa_<id>` (the front end lists it, or
`openmx --track mxa_<id>`). If it is left out, the log line says why. The texture, props and stadium
are MD260.
