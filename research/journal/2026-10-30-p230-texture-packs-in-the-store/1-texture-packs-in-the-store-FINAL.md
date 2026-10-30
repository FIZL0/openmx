# 1 — texture packs in the store (FINAL)

Task P230. 2026-10-30.

## What I did

- `modstore.rs`: `Kind::Textures` (word `textures`, folder `mods/textures/<id>/`, tab 3 "Texture Packs").
  `FOLDERS` replaces the three hard-coded folder lists in `stamps`, `own_ids` and `up::locals`.
  `parse_check` reads a staged pack through the new `textures::pack_at`.
- `textures.rs`: `pack_at(dir, folder)` reads one pack the way `packs` does and checks it as hostile input:
  `mod.json` says textures, at least one key-named `textures/**/<name>-<key>.png`, and every such file has
  a PNG header that reads. `listed(mods, order, off)` lists every pack top first with name, texture count
  and on/off, for Pack Order. `textures.rs`'s walk (`mod_dirs`, depth 3) already finds
  `mods/textures/<id>`, named `textures/<id>` in `texture_packs`.
- `menu_store.rs`: STORE_TABS gets "Texture Packs" (a grid like the others: install, update, remove,
  report, Download All). It also gets a page, "Pack Order", first among the pages (before Upload and
  My Mods). This is opentennis's Installed → Texture packs, since openmx's store has no Installed view.
  It lists every pack in the mods folder top first, store or hand-made. Cross turns one on or off;
  Left/Right move it up/down (opentennis has △/□, but here △ is Back and □ is Remove). Start rescans.
  Each change goes straight to `texture_packs` (every pack, top first, so a pack installed later goes on
  top as opentennis's `on_top` does) and `texture_packs_off`, then `changed` saves settings.txt. The shell
  fills the list on `StoreAsk::Packs` (`StoreFront::packs` in main.rs).
- `menu_screens.rs`: Pack Order's rows: name, On/Off, place, count, folder. The online note
  (`PACKS_ONLINE`, opentennis's words: "only you see a texture pack; the lobby doesn't share it") shows
  under the Texture Packs grid and on Pack Order. In the browser the note says packs draw only on the
  desktop.
- `modstore_up.rs`: a pack of the player's own in `mods/textures/<id>/` is one of their Upload mods. It
  is packed and checked like any mod, with `parse_check` going through `pack_at`. Its preview is its
  own `preview.png`, or else its biggest texture file.
- Tests: `modstore_tests.rs` `a_texture_pack_round_trips` uses the fake store. It lists the pack on its
  tab, installs it into `mods/textures/hd_pack`, then checks that `textures::scan` finds both keys from
  the pack (a bare `<key>.png` is not key-named) and that `listed` and the off line work. It also checks
  that a pack with no key-named texture, with a non-PNG, or with another kind's mod.json is refused and
  leaves nothing; removing the pack takes it out of the scan. `modstore_up_tests.rs` `a_texture_pack_uploads`
  covers the local listing, packing and the preview picture, and refuses an empty pack.
  `tests/store_menu.rs` walks the new tab and page and adds `pack_order_switches_and_moves_packs`
  (switch, move, clamp, the settings round trip, nothing under `--mods off`).

Taken from opentennis: `Kind::Textures` and its word, the online note's text, the kind tab, the
installed-pack order list writing `texture_packs`/`texture_packs_off`, and the refusal of a pack with no
key-named textures and of broken pictures (`modfs` refuses a pack whose PNG doesn't decode). Changed:
see below.

## What the original does

Nothing: mods and the store are additions. With no pack installed, nothing changes.

## How it was proved

Unit tests above (`./kit check -- -p openmx --bin openmx -- modstore`: 22 passed; `--test store_menu`
passed). No emulator: there is nothing of the original's here.

## Not verified / not 1:1

- **No separate, bigger size cap for packs.** opentennis has MAX_PACK = 2 GB because its other mods
  stop at 1 GB. openmx's caps are already above that for every kind: MAX_TOTAL is 3.75 GB, MAX_ZIP is
  the 4 GB zip32 limit, and MAX_FILE is 1 GB. A pack can't go past the zip32 offsets without zip64,
  which neither reader writes or reads.
- **Install checks each texture's PNG header, not the whole picture.** opentennis decodes every PNG.
  A multi-GB pack of thousands of pictures would take a long time to decode at install, so a picture
  whose header reads but whose data is broken installs. It is then skipped when a ride asks for it,
  as any pack's broken file is (`read_png` → None).
- **No disc-copy check.** opentennis refuses a pack that ships the disc's textures unchanged
  (`texhash::disc_prints`). openmx has no such check, and the user dropped the own-disc share check for
  every kind on 2026-10-29 (b8589939). Not ported.
- **No covers/look tags and no before/after preview.** opentennis's `covers`/`look` tags and its
  disc-vs-pack before/after preview aren't ported. A pack's preview is its own `preview.png` or its
  biggest texture.
- **Only `mods/textures/<id>/` counts as an uploadable pack.** A hand-made pack sitting elsewhere
  (e.g. `mods/<pack>/`) still draws and still shows on Pack Order, but it isn't offered for upload.
  That matches how the other kinds are found (`mods/<kind folder>/<id>`).
- **The browser stays as P203 leaves it.** Texture Packs lists and installs into OPFS like any kind, but
  the page's rides don't read packs, and Pack Order there lists nothing (no OPFS walk). The tab's note
  says so.
- **Nothing drawn or screenshotted.** The new tab and page are checked by the menu tests only. Their
  layout (rows and the note line's y) was not looked at on screen, and no menu `--shot` was taken.

## Next

Done.
