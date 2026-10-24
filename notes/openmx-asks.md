# What openmx asks of openmx-mods

The other direction from openmx-mods' own `notes/openmx-asks.md` (what a mod needs the port to change):
here, what the port would like a mod to carry. openmx-mods reads this file through its `openmx`
symlink; an agent there takes an entry into its `plan/TODO.md` and says here when it lands. Each entry:
which task asked, what to add, and what the port does without it.

## Open

- **MD18, each world piece's terrain shape in the world's `mod.json`** (standard §3). To lay a world
  out, `worlds::find` needs of every tile and join its terrain's feet a sample, its tree's levels and,
  for a join, how many root cells its tree has across and along. Today it opens each terrain and reads
  its header and its tree's root (at most 1,089 + 97 + 1,024 bytes a file; see openmx's
  `research/journal/2026-10-23-md18-cheap-mod-scan/`). With them in `mod.json` it would read nothing
  but `mod.json` and the tiles' scenes, which the ride keeps. Asked: an optional `terrain` object on
  each entry of `tiles` and `joins`,

  ```json
  {"tile": "nat13", "at": [0.0, 0.0], "keep": [0, 0, 1279, 1279],
   "terrain": {"spacing": 4.5, "levels": 2, "cells": [5, 5]}}
  ```

  - `spacing`: the `.trn` header's scale (the float after the palette), written as the shortest text
    that reads back to the same `f32` bits (Untamed's `Nat_OR` is `4.499999`, not `4.5`);
  - `levels`: the header's tree depth;
  - `cells`: the root node's children table, `[across, along]`: one more than the highest column and
    row whose child offset is not 0 (at least 1 each). For a tile the port takes these from the scene's
    `TerrainWidth`/`TerrainBreadth` as now; a join has no scene, so for a join it is the only place.

  `stitch` writes every number already (it generates the joins and copies the tiles' terrains), so it
  costs a world's build nothing. The port would trust them only after checking them against the file
  once at the ride's load, and would keep reading the header when they are missing. Without them
  nothing breaks: a world's find stays a few kilobytes a piece.
