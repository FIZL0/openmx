# 1 — track mods (FINAL)

Task MD2. 2026-10-22.

## What I did

- **A folder reads as an archive** (`crates/game-render/src/disc.rs`). `Res` now holds a
  `Box<dyn Members>`: a trait with `entries()` and `bytes(entry)`. The disc's archive is one
  implementation, the same bytes and the same inflate as before. A loose folder is the other
  (`Res::folder`). `Res::files` takes the same thing as (path, bytes) pairs, for the browser's
  storage. `read` / `find_suffix` / `read_suffix` didn't change, so every loader that takes `&[Res]`
  (scene, terrain, collision, props, splat, sky, models, sounds) takes a mod folder without a
  signature change. Folder names are `\`-joined and sorted, so they match the archive's lookups.
  Lookups are case- and slash-insensitive. Two files differing only in case refuse the folder, as the
  standard says.
- **The mods folder** (`crates/openmx/src/track_mods.rs`, new). `scan(mods)` walks
  `mods/tracks/*/` in name order and checks each `mod.json` against openmx-mods'
  `standard/OPENMX-TRACK-STANDARD.md`:
  - `standard` 1; a higher one is left out with a line.
  - `kind`: `track`. Any other kind is skipped without a line, since a music pack is not broken.
  - `id`: equals the folder name, `[a-z0-9_]`, 1–40 characters, `<prefix>_<map>`.
  - Required: `name`, `source.game`, `version` as major.minor.patch, `author`, `permission`, and
    `todo` as a list of strings.
  - Optional `spawn` with `from`.

  The check also requires exactly one `.scn` that parses, and a terrain (named, then with the
  `.ps2` tag) that `Terrain::parse` reads. Each refusal is one line,
  `mods: tracks/<folder> left out: <why>`, and the mod is not listed. A `.col` that fails is not
  checked up front: `Props::load` already leaves out a mesh it can't read, as on the disc. The JSON
  reader is a small one of its own (no serde in the shell's deps); it keeps numbers as written so a
  `spawn` converts through the scene's own float reading.
- **The ride** (`main.rs`):
  - `--track <id>` finds the disc's archive first and a mod second, so a mod never shadows a disc
    track. Its id must have an underscore, which no disc archive name does.
  - A mod's folder stands in for the track's `.res`. Its second archive (models, the bike's
    sounds) is `free06_2.res`, as the standard says a mod has none of its own.
  - The scene's stem replaces `args.track` wherever a member name is built from it.
  - `spawn` from `mod.json` overrides the scene's start.
  - `--mods DIR|off` picks the folder for a command-line ride. Without it the ride reads
    Settings: `mods` on means `<store>/mods`, off means none.
  - A mod that's broken, or asked for with mods off, is an error exit with the reason, not a panic.
  - A headless tape ride now also prints `end: x, y, z` after its hash, so a test can see where
    it went.
- **The pick and Settings** (`menu.rs`):
  - `Menu::set_mod_tracks` lists `(id, name)` after the disc's five, and the pick draws a mod as
    a black band (no stills) with its name and "Freeride n of m".
  - A Settings row `Mods  On/Off` (saved as `mods = off`, only when off) sits above Camera.
  - `settings.track` may hold a mod id. A track the pick no longer lists opens on the first.
  - The front end scans when it opens, and again when the row changes, so turning the row off
    empties the pick at once and reads nothing.
  - An online room stays on disc tracks (MD6 is mods online).
- `.gitignore`: `/mods/`.

Taken from opentennis: the shape of `hst-base/src/mods_court.rs`. That means `list` / `find` by id,
a broken mod left out with its reason logged, the mods listed after the disc's, and a flag that
plays one directly. Its code is Bevy- and serde-based, so I took the structure only.

Decisions, unattended:
- The trait sits under `Res` instead of changing every loader to a new type. That was fewer lines
  in files other tasks touch, so the merges stay clean.
- The `--mods` flag wins over the Settings row. A test or script names its folder and isn't steered
  by whoever's settings.txt is beside the disc.
- A mod's second archive is free06's. The standard says a mod rides on a disc freeride's.

## What the original does

Nothing. The original has no mods. The folder layout is openmx-mods' standard, and every file in a
mod is read by the same readers as the disc's.

## How it was proved

`crates/openmx/tests/track_mods.rs` writes a mod into the target's tmp dir. It has a flat two-level
PLN v7 terrain of 256×256 samples at 4 ft, a CRLF scene naming `MDFLAT.trn`, and one wall `.col`
(two triangles, one node with two leaves) placed by `[Object1] COL=mdwall.col`. Beside it are
eleven broken mods and a music pack:
- `a_mod_folder_lists_and_the_broken_ones_are_one_line_each`: the scan lists exactly `md2_flat`,
  and each broken one gets exactly one line. The broken ones are bad JSON, no mod.json, no .scn,
  two .scn, no terrain, a truncated terrain, id ≠ folder, standard 2, a bad version, a bad spawn,
  and two files differing in case. The music pack and a loose README say nothing. `find` returns
  the mod, a broken mod's reason, or an error for an absent or `../` id.
- `a_mod_folder_reads_as_an_archive`: names, case- and slash-insensitive reads, and the scene,
  terrain and collision parsers on the folder's bytes. The same via `Res::files`, and its case
  clash refused.
- `a_track_mod_loads_and_rides_and_a_broken_one_does_not_crash` (needs the disc): the shell
  headless rides `--track md2_flat --mods <dir>` on a 150-frame full-throttle tape:
  - It prints the mod, `ground 1020 x 1020 feet in 256 x 256 samples`,
    `solid: 1 of 1 collision meshes`, and starts at 512, 5, 300.
  - It ends at 513, 0, 591: 291 ft up the flat in 5 s, on the ground.
  - The same tape gives the same hash twice.
  - A broken mod by name and a mod with `--mods off` both exit with a reason, and neither panics.
- `a_disc_ride_is_the_same_with_or_without_mods` (needs the disc): free06 on the same tape hashes
  the same with a folder full of mods, with `--mods off` and with an empty folder. It never prints
  a `mods:` line.
- Against HEAD (517539a, built in a detached scratch worktree with its own target dir): the same 150-frame throttle tape hashes `0xa3168823af054951` on free06 and `0x47b82bbe87280bb3` on
  free09 on both HEAD's binary and this branch's (no mods folder beside the disc).
- `crates/openmx/tests/menu.rs`, `the_pick_lists_mod_tracks_after_the_discs_and_the_mods_row_is_saved`:
  - With no mods the pick is TRACKS exactly.
  - A mod lists after them and Left wraps to it.
  - Cross rides `Act::Ride("md2_flat")`, and the settings track round-trips.
  - With the folder emptied, the pick opens on the first track.
  - The Mods row by pad turns off, is written as `mods = off`, and reads back.

  The P103 walk in `the_vsync_and_frame_limit_rows_change_by_pad_and_are_saved` takes one more Down
  for the new row.

## Not verified / not 1:1

- **No real mod has been ridden.** openmx-mods has no converted MX vs. ATV track in this layout yet.
  The test mod is generated, and it is two-level. A one-level terrain (MX vs. ATV's supercross and
  freestyle maps, `TerrainWidth=1`) loads, but the port's `Ground::new` sizes it at 16 samples, not
  the standard's 256, so its rider falls off the map at once. I filed that as **MD12** rather than
  change game-sim's world.rs in this task.
- **wasm / browser storage is not wired.** `Res::files` exists and is tested in memory, but the
  web shell has no track pick or settings, and nothing hands it the browser's files yet.
- **No previews.** A mod shows in the pick as a black band with its name. The standard's preview
  image isn't read.
- **A mod's own sounds are not played.** Track emitters are read from the disc's second archive
  (free06's), and a mod's `.aud` is not looked for by suffix. The bike's sounds are the disc's.
- **The second archive is always free06's.** A mod whose scene names a model only another
  freeride's `_2.res` holds would miss it. Missing props are left out, not fatal.
- **Online is disc tracks only.** Rooms and `--peer` rides don't offer or check mods; that is
  MD6's job.
- **Only the first problem per mod is reported.** A mod with several faults gets one line naming
  the first.
- **No unit test of `Res::folder` in game-render itself.** It is tested from crates/openmx's
  integration test, which is where the brief put the tests.

## Next

Landed. MD12 (one-level terrain size) is the first thing a real MX vs. ATV supercross mod will need.
