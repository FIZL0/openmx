# 1 — disc worlds (FINAL)

Task P232. 2026-10-30.

## What I did

- **The stitcher moved in.** openmx-mods' `tools/stitch` (lib.rs, build.rs, check.rs, jsonw.rs,
  openmx.rs) is `crates/stitch`, and `tools/trnwrite` (the join .trn writer and `openmx_view`) is
  `crates/trnwrite`, copied as they were but for:
  - mod.json's JSON is the shell's: `track_mods`' `Json` and its parser moved to `stitch::json`
    (`track_mods` re-exports it), with `f64()` (what modcheck's `num()` was) and `arr()` added.
  - `check::check` no longer runs modcheck (openmx-mods' CLI runs it beside); `ultra.rs`, the CLI and
    the MX vs. ATV / Untamed disc imports stay in openmx-mods, as the brief allowed.
  - New entry points, nothing old changed: `Tile::from_parts` (a tile from its files' bytes, so a disc
    map's archives are a source; `Tile::load` now reads the folder and calls it), `build::build_tiles`
    (`build` loads tiles and calls it; `copy_tiles` false writes only mod.json and joins/),
    `Params::from_spec` (a `worlds/<id>.json`), and `stitch::VERSION` (1; the cache key, bump when the
    joins change). `AUTHOR` stays "openmx-mods tools/stitch 0.1" so the world files are byte-identical
    to what openmx-mods writes. Two comments' retail addresses (the shootoff test, the position query)
    became words, for `./kit audit`.
  - The world specs `world_mxu.json` and `world_mxu_free.json` are in `crates/stitch/worlds/`.
  - Tests moved: holes, join, middle_spawn, prune, ridge, rock, spec_todo, steep, and the first test of
    openmx_worlds; trnwrite's grids, and its disc test now reads every `.trn` of the configured disc's
    `Tracks\*.res` (ignored, as before).
- **The shell builds the worlds** (`crates/openmx/src/disc_worlds.rs`): tile `mxu_<track>` is the disc's
  `Tracks\<track>.res` + `_2.res`, read as `Origin::Disc` pieces (`worlds::read` takes a `DiscTile`
  opener; `worlds::find` is `read` with none). The joins are built with `stitch::build::build_tiles` the
  first time and kept under the mods folder, `cache/stitch/<id>/<key>/` (key: sha256 of
  `stitch::VERSION`, the spec, and every tile's scene and terrain bytes; another key's folder is
  removed when a new one is built). Under `--mods DIR` or the mods folder beside the settings, with the
  Mods setting off too (they are the disc's, not a mod's).
- **The pick**: `menu::SPLICED` ("Every free ride", `free_all`) is gone; `menu::DISC_WORLDS` lists
  world_mxu_free and world_mxu after the five towns, in MX Unleashed's filter, with the five towns'
  stills turning on their cards. A saved `track = free_all` and `--track free_all` ride world_mxu_free.
  An installed `mods/tracks/<id>` world of either id is ridden in its place and has no row of its own.
  The browser lists neither (`DISC_WORLDS` is empty on wasm32) until P203's OPFS view.
- `src/spliced.rs` (P71's made strip joins) and `tests/spliced_maps.rs` (its layout's rides) are removed;
  `tests/spliced_pick.rs` now rides world_mxu_free from a settings file that says `free_all`.

## What the original does

Nothing: the original has no worlds. This is the port's own (MD8/MD29) built the way openmx-mods built it.

## How it was proved

- **Byte-identical to openmx-mods.** Built off the disc by the shell with an empty mods folder:
  world_mxu_free (5 tiles, 38 joins + 14 seam joins, ~10 s release) and world_mxu (27 tiles, 1222
  joins + 328 seam joins, 1 min 52 s release, wall clock while other slots ran): `diff -rq` of joins/
  and `cmp` of mod.json against `~/repos/openmx-mods/mods/tracks/world_mxu{,_free}` — identical, both.
- `tests/disc_worlds.rs`: on an empty mods folder the first ride builds world_mxu_free, the second
  builds nothing and says the same world/wrap/holes/seams lines (`--seams` walks every piece's edges:
  216 edges, no gaps), the built mod.json and every join file equal openmx-mods' installed world, and
  riding that installed world gives the same lines. Its reference is the main checkout's
  `mods/tracks/world_mxu_free`, byte-equal to openmx-mods' (checked), so the comparison ran in the
  gate rather than skipping. world_mxu's test is the same, ignored (minutes, 850 MB).
- `./kit check`: stitch + trnwrite 24 passed; openmx menu, spliced_pick, middle_spawn, lib 258 passed;
  disc_worlds with `--include-ignored` passed (the kit printed "26 tests", a shared count, so the
  world_mxu run inside it is not proven by that line; its build and comparison by hand above is).
- `tests/menu.rs`: both rows, their order, Cross rides them, settings keep them, `free_all` maps to
  world_mxu_free, an installed world of their id is not a second row.
- Shot: `context/shots/p232/join_free06_free10.png` (slot s2), world_mxu_free from 5950,330,3000
  looking east over the join between Dallas (free06) and Rochester (free10).

## Not verified / not 1:1

- **The web**: no disc worlds in the browser yet (needs P203's OPFS view of the disc); filed as a follow-up.
- **trnwrite's disc test** reads only MX Unleashed's `.trn` files (the configured disc); MX vs. ATV's
  and Untamed's are left to openmx-mods' copy of the test.
- **openmx_worlds' ultra and disc-import tests** were not moved (ultra.rs stays in openmx-mods).
- **`check::check` without modcheck**: the moved report no longer includes modcheck's findings.
- **world_mxu's ignored test** may not have run inside `./kit check` (see above; its build was run and
  compared by hand); its first ride takes ~2 min with no progress bar beyond the one log line, on the loading
  screen.
- **The cache is never cleared** of a world whose spec is removed (only other keys of the same id).
- openmx-mods' T13 (point its stitch at these crates) is theirs to do; until then the two copies can
  drift, and only the byte comparison above would catch it.

## Next

Nothing for this task. P233 (filed, after P203): the browser's disc worlds. openmx-mods' T13: point its
stitch at crates/stitch and crates/trnwrite.
