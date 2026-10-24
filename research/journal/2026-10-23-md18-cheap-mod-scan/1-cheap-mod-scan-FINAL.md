# 1 — cheap mod scan (FINAL)

Task MD18. 2026-10-23.

## What I did

- **The menu's scan reads `mod.json` only.** `track_mods::scan` (run at every start, over every mod) now
  checks each `mod.json` as before (standard, kind, id = folder, name, source.game, version, author,
  permission, todo, the spawn's numbers through the scene's own conversion) and nothing else: no folder
  walk, no scene, no terrain. Its list is a new `track_mods::Listed { id, name, dir }`, since a mod's
  `stem` comes from its folder's listing. `track_mods::find` (what a ride asks) adds the folder's listing
  for its one `.scn` (still no file read but `mod.json`); the scene and terrain are first read by the
  ride's own load, which now says which file is wrong (`MDFLAT.trn: 1 blocks of 2129 from 77 overrun the
  file`, `<scene>: …`) — the front end shows it as "Could not ride".
- **A world's find reads each terrain's header and its tree's root alone.** game-sim's `track.rs` gains
  `TerrainHead` (`parse` of the file's first ≤ `TerrainHead::MOST` = 1,089 bytes: scale, levels, blocks,
  root offset; `fits(len)` = `Terrain::parse`'s "the tree fits the file" check) and `TerrainNode::read`
  (one node out of the bytes at it; `Terrain::node` now calls it, same behaviour) with
  `TerrainNode::BYTES`/`TABLE_BYTES`. `worlds::shape_of_file` opens a terrain, reads its header, seeks to
  the root node (97 bytes) and to its children's table (1,024 bytes), and makes of them what `shape()`
  made of the whole file. Tiles find their terrain's path through `track_mods::terrain_path` (the same
  lookup as `terrain_bytes`, without the read); joins through `join_terrain_path`.
- **What find read is kept.** `WorldPiece::scene_bytes` holds each tile's scene file as find read it
  (before the `drop`); the ride's home scene (`main.rs`) and each tile's dressing (`world_dress::build`)
  use it instead of reading the file again. The parsed `Scene` was already kept and is what the
  simulation's `Files` loads collision from.
- **Asked of openmx-mods** in a new `notes/openmx-asks.md` (this repo's; openmx-mods reads it through its
  `openmx` symlink): an optional `terrain: {spacing, levels, cells}` on every `tiles`/`joins` entry of a
  world's `mod.json` — the header's scale (as the shortest text giving the same f32 bits), its tree
  levels, and the root table's used columns/rows. With those, find would read `mod.json` and the scenes
  only (the scenes are kept for the ride, so not a second read). I put it in this repo rather than
  appending to openmx-mods' own `notes/openmx-asks.md`: that file is the other direction ("what the port
  needs from openmx"), its main checkout runs its own agents, and the plan named the file as one this
  task creates here.

Nothing to take from opentennis: it has no mod folder.

## What the original does

Nothing: mods are an addition. With no mods folder, or the setting off, no code here runs.

## How it was proved

- `crates/openmx/src/worlds.rs` `a_worlds_find_reads_each_terrains_header_and_root_alone`: a generated
  world (two tiles, one with a palette and a texture page, at 9 and 4.5 ft; a 3×1-cell join at 6 ft).
  Linux's own count of bytes the thread read (`/proc/thread-self/io` `rchar`, every `read` whoever makes
  it) across `find`: 7,557 bytes, against an allowance of the mod.json + scene files whole plus
  `MOST + 97 + 1024` per terrain = 7,836 (+512 for the counter's own read); the terrains are 112,464 bytes.
  Each piece's spacing, levels and (join) root cells equal what the old whole-file `shape()` makes of the
  file; the kept scene bytes equal the files; a terrain cut short of its tree is refused by find.
- `crates/game-sim/src/track.rs` `a_terrains_header_reads_without_the_rest_of_it`: `TerrainHead` from the
  first `MOST` bytes equals `Terrain::parse`'s fields, `TerrainNode::read` equals `Terrain::root()`,
  `fits` and a too-short header.
- `crates/openmx/tests/track_mods.rs`: `mods_with_terrains_cut_to_their_headers_all_list_and_a_ride_says_why_not`
  (three mods with terrains cut to their headers: the scan lists all three and logs nothing; a headless
  ride on one fails naming `MDFLAT.trn` and the overrun). The MD2 scan test now splits its broken mods:
  the six with a bad `mod.json` are still one line each and left out; the five with bad files
  (no/two scenes, case clash, no/bad terrain) are listed, `find` refuses the three its listing sees, and
  the ride refuses the other two (the existing `md2_noterrain` ride check).

## Not verified / not 1:1

- **A terrain whose blocks or texture pages do not fill the space in front of its tree is no longer
  refused before a world rides** (find checks the header, the root and that the root fits the file, not
  the blocks). The home tile's terrain is still parsed whole by the ride (an error, "Could not ride");
  any other tile's is parsed when first loaded and, if it fails, is logged `world: <tile> has no ground`
  and the ride goes on with a hole there. No test rides such a world.
- **Not every byte is read once.** A world's `mod.json` is read by `is_world` and again by `find`; each
  terrain's header and root (~2 KB) are read by find and again inside the ride's whole read of the file;
  the pick reads each track mod's `mod.json` twice (the scan, then `source_games` for MD11's filter). All
  are kilobytes; the whole terrains and scenes are what is read once now.
- **A track mod with a broken scene or terrain is listed in the pick** and fails only when picked
  ("Could not ride: …"), where before it was left out with a log line. That is what the task asked; the
  front end's message for it was not looked at on screen.
- **The 52 installed free-ride mods were not timed** before and after; the proof is the byte count on a
  generated world and the scan reading no file but `mod.json` by construction.
- **openmx-mods has not answered the ask**; `find` reads the headers until it does, and nothing reads a
  `terrain` field yet.

## Next

Done, unless openmx-mods adds the `terrain` field: then a follow-up has `find` take it (checked against
the file at the ride's load).
