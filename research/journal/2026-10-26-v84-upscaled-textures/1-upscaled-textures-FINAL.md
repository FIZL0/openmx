# 1 — upscaled textures (FINAL)

Task V84. 2026-10-26.

## What I did

Taken from opentennis (`/home/rycs/repos/opentennis`), as AGENT.md asks:

- **`tools/chainner/upscale.chn`**: copied byte for byte (`cmp` is clean). No folders and no model are set in it.
- **`tools/upscale_mods.py`**: kept as it was, with the same options, output wording, batches of 500, flat
  numbered temp names, dropped preview nodes, `MIOPEN_FIND_MODE=FAST`, `--no-sandbox` last, the process
  group killed after, `sources.json` SHA-1s, `--check` / `--prune`, and the `Lazy.from_coroutine` note.
  What changed is where the images come from:
  - opentennis pulls PNGs out of a mod's `.glb`s. Here each mod's folder is dumped once a run with
    `openmx --dump-textures TMP --from MOD`, so the port's own decoder reads the `.pst`s (no Python
    re-implementation, as the brief asks).
  - An upscale is named by the dump's relative path (`Tex/Gate-<key>.png`), not `<glb stem>/<name>.png`,
    and `sources.json` is keyed by that path.
  - Texture packs are skipped as well as `"upscale": false`, since they are upscales already.
  - `--openmx` names the binary (default `target/release/openmx`).
  - `--prune` walks subfolders, since a mod's folders nest.
- **The lookup order** (opentennis's `textures.rs` / `mods_textures.rs` / texhash's `Overrides`):
  - Packs first. A pack nobody ordered goes on top, then settings.txt's `texture_packs` order;
    `texture_packs_off` turns packs off. A pack with no key-named files gets one log line.
  - Then key-named files in `mods/texture-replacements/`, matched by the part after the last `-`. The first
    file of a key wins, in name order.
  - Then mods' `textures/upscaled/`, then the disc.
  - The same startup line shape: `textures: N from P texture packs, M key-named in
    mods/texture-replacements, U mods' own upscales`.
- **The docs**: opentennis's "Texture replacements" section was reworded for this game into
  `docs/TEXTURES.md`, which is linked from README.md's table.

What changed for MX Unleashed:

- **The key.** opentennis keys its own textures by the source image. Here it is FNV-1a 64 over the
  decoded picture's width, height and RGBA, as 16 lowercase hex digits (`game_render::replace::key`).
  - It is stable across runs and builds and does not depend on where a member sits in an archive.
  - A texture two archives carry is one key.
  - game-render stays free of dependencies, which is why there is no SHA crate.
  - The dump of the disc writes 10 786 textures in 91 s, to 10 682 distinct paths. A path carries the key,
    so the 104 repeats are the same picture under the same name, written twice.
- **Where it plugs in.** The renderer has no Bevy asset server, so the source is one process-wide
  `RwLock` in `game_render::replace`, asked when a texture is decoded:
  - `world::decode_texture`, for every model's material: props, bikes, riders, the biplane and machines.
  - splat's ground layers, the detail map, the painted road (`ground_mesh`), and a mod map's terrain
    pages (`Pages::of`, through `swap_all` so the page array keeps one size).
  - In the shell, the start zones' art (`portal.rs`) and the tire tread.
  - `world::decode_disc_texture` is the unreplaced decode, which the dump uses.
- **Tiling (opentennis B222)** holds by construction. UVs are never touched, and the GPU's mips are built
  from whatever Texture it is handed (`gpu.rs` `mips`), so a replacement gets its own mips.
- **Not swapped, on purpose:**
  - HUD and menu sprites and fonts (`overlay.rs`, `menu.rs`, `dress.rs`), which are cut by pixel rectangles.
  - Particle sheets (atlas rectangles in pixels).
  - The ground's shadow tiles, joined pixel row to pixel row.
  - A world's baked splat pages (`world_dress.rs`), which are baked from the layers. They do pick up
    replaced layers, but are not keyed themselves, because the bake depends on the light.
- **Setting**: `Departure::Textures`, "Upscaled Textures", Upscaled / Original, on by default. It is
  written to settings.txt only when it is off (`textures = original`), and the Original switch turns it
  off. There is also `--textures upscaled|original` for one ride. The row says the software renderer
  draws them too, slower.
- **The mods folder** is the one beside the settings, or `--mods DIR`'s.

## What the original does

Nothing to port: presentation only, a departure from the disc's textures behind a switch.

## How it was proved

- `crates/openmx/tests/textures.rs` uses hand-made `.pst`s and no disc. Each test:
  - **`a_key_named_file_is_drawn_with_the_setting_on_and_keeps_its_tiling`**:
    - A 4×4 checker is replaced by a 16×16 of four quadrants, through `decode_texture`.
    - A UV 2 tiles over reads what it reads at the base.
    - Off, or with no mods folder, it is the disc's.
    - Unkeyed and unmatched files change nothing.
  - **`a_texture_pack_beats_the_key_named_files_in_settings_order`**: two packs (one nested,
    `packs/hd_b`) and a key-named file, all for the same texture, with the order, the off list and an
    empty pack.
  - **`a_track_mods_own_upscale_replaces_its_pst_and_the_dump_keys_it_the_same_twice`**: the shell binary's
    `--dump-textures --from` on a mod with a `.pst` and an `.atm` gives the same names twice. The
    upscale under that name is drawn when the `.pst` is read from the mod's folder as a ride reads it.
  - **`upscale_mods_check_lists_what_a_mod_still_needs`**: `tools/upscale_mods.py --check` exits 1 listing
    both textures. It exits 0 once stand-in upscales and a `sources.json` are in place (no chaiNNer).
    A wrong SHA-1 is stale; a changed `.pst` is missing and leaves two unused; `--prune` deletes them.
- `crates/openmx/tests/menu.rs`:
  - `upscaled_textures_is_a_graphics_departure_and_the_pack_lines_round_trip`.
  - The settings file round trip now carries the three fields.
  - P128's Graphics-tab list now has the row.
- `game_render::replace`'s unit tests cover the key and the swap, including refusing a broken
  replacement and `swap_all`'s resample.
- **By eye**, on a real ride (`--headless --frames 2 --track free06 --shot`):
  - With free06's 83 textures replaced by magenta at twice the size, the ground and props drew magenta:
    171 611 magenta pixels of 286 720. With `--textures original` there were 655.
  - With the 406 Bike, Rider and biketext textures replaced by green, the bike and rider drew green.

## Not verified / not 1:1

- **Layer 2, a PCSX2 texture pack, is not read** (follow-up V85). PCSX2 names a dumped texture by a hash
  of the GS upload: TEX0, the texture's bytes as uploaded and its CLUT. opentennis's texhash reproduces
  that for its game. For this game, nothing has checked whether the GS upload matches what the `.pst`
  holds. The mip levels, the swizzled CLUT order (`pst::unswizzle`) and the 4-bit textures could each
  move the hash. Without a PCSX2 dump of this game to compare against, porting it would be a guess, so
  V84's brief leaves it.
- **No live reload** (follow-up V86). `OPENMX_TEXTURE_RELOAD` is not read. The folders are walked when
  a ride loads, so a changed file is drawn from the next ride. Mid-ride it needs a re-upload by key in
  both renderers.
- **No real upscale was run**: chaiNNer is not installed here (the brief says the tests must not need
  it). The chain, the headless run and the `Lazy` fix are opentennis's, unchanged, and are untested
  on this machine.
- **The software renderer** samples a big replacement at its full size with no mips, so far textures
  alias more than the disc's. It is slower with 4× textures too. This was not measured.
- **GPU memory**: 4× replacements are 16× the texels. Nothing caps it; a full disc's worth was not
  loaded.
- **A combined world (MD8)** draws each tile's mod upscales through the same lookup. No world test
  covers it.
