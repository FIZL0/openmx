# 1 — mod store browse install (PART)

Task P198. 2026-10-29.

## What I did

- **The store** (`crates/openmx/src/modstore.rs`, native and wasm): the public HF dataset `FIZL0/openmx-mods`,
  each mod `mods/<id>/{mod.zip,meta.json,preview.webp}`. `main` is Verified; open PRs are listed under their
  author's name. The listing is three requests (M116): `index.json` (P199's tooling writes it), the open PRs, and
  their heads. `main`'s mods come from the index. A PR's mods come from it only while the PR's head is the commit
  the index read; any other PR, or every rev when there is no index, is walked tree by tree. It is
  kept for an hour in `mods-store.json` beside the settings. Start reads it fresh. A 429 or 5xx, or no network,
  leaves the last listing up with a note (M116). Keys are `<author>/<id>` (B248).
- **Install**:
  - the zip streams in with progress every MB (B204), hashed as it comes, its size and sha256 checked against
    meta.json;
  - then the data-only zip checks. From the central directory: plain paths only (no `..`, roots, backslashes,
    drive letters, device names, control characters), no links or special files, no case-blind duplicates,
    no encryption, deflate or store only, the extension allowlist, an entry cap, and mod.json at the top or under
    one wrapping folder. While unpacking: the local header must match, inflate is capped at the declared size so
    a bomb stops at its own claim, then the size and CRC are checked, and a running total is kept;
  - mod.json must agree with meta.json on id and kind and be a standard version we read;
  - natively, the shell's own parsers (track_mods::find, worlds::find, trick_packs, music_packs) then run on a
    stage `mods/.store-<id>/`. Only then is it swapped into `mods/<kind>/<id>/` with `store.json` (zip sha,
    rev, verified, version). The old copy is moved aside and put back if the swap fails;
  - a folder of that id that the user made (no store.json) is never overwritten or removed.
- **Rescan without a restart**: Installed/Removed trigger the shell's existing mods rescan (`main.rs` loop), so a
  track lands in Free Ride's list straight away (desktop).
- **Remove, Update, Report**: Update is Cross on a card whose installed sha differs. Report opens
  `huggingface.co/datasets/FIZL0/openmx-mods/discussions/new?title=Report: <name> (<id>)&description=Mod id: <id>…`.
- **The opentennis bugs, built in**:
  - B204: streamed with progress;
  - B242: no Range is ever sent, and the redirect is followed;
  - B358: the browser writes through a sync access handle in OPFS `store/`, read back in 4 MB pieces for the sha
    and unpack;
  - M116: as above;
  - B248: keys, not names;
  - B259: previews cached by zip sha, so an update's new zip gets a new picture;
  - B339: a queue, 3 at a time natively and 2 in the browser;
  - B372/B373: a picture grid, 4 wide, with Tracks / Trick Packs / Music tabs on L1/R1.
- **The menu**:
  - Settings → Mods → Browse the Store opens `Screen::Store` (`menu_store.rs`, drawn in `menu_screens.rs`);
  - each card shows the preview, name, author · source game, size · Verified / Not reviewed, and Installed /
    Update / In your folder / a progress bar;
  - the menu holds only plain view types, and `main.rs`'s `StoreFront` translates between them and the store, so
    the 17 test files that mount menu.rs need nothing new.
- **New flag**: `--mod-store URL` points at another store (tests). `--screen store` now asks for the listing.
- **Browser**: seven host calls in `web_host.rs` / `web/worker.js` (`store_fetch/got/read/close/put/drop`,
  `open_url`). index.html opens only `https://huggingface.co/` addresses that the worker asks for.

## What the original does

Nothing. The original has no mods. This copies opentennis's M17 store shape.

## How it was proved

- `src/modstore_tests.rs` (13 tests):
  - about 34 hand-made bad zips, each refused;
  - a bad-mod set through the fake store: a wrong sha, size ±1, no scene, a bad version, a mod bringing its own
    store.json, and the user's own folder. Each is refused whole, and the mods folder's tree is unchanged
    afterwards;
  - a seeded fuzz of 20,000 mutated inputs (xorshift 0x0198_f00d) over the zip reader, the meta/mod.json readers,
    the listing parsers and the preview decoder, with no panic;
  - a fake HF on localhost (std TcpListener, mod.zip behind a 302 to `/cdn/…`). It lists, keeps the listing,
    installs through the redirect (then `track_mods::find` finds it), installs a music pack from a PR, updates
    with a new preview (B259), removes, and handles busy (429) and the parallel cap.
- `tests/store_menu.rs` (new): the grid's arrows, tabs, Cross / Square / Triangle / Start / Circle, the asks, and
  `--mods off`.
- `tests/web.rs::the_browser_installs_from_the_store` + `tests/web/store.mjs`: headless chromium, the real disc
  picked, `?software&args=--screen store --mod-store <loopback>/store`, Space (Cross) on the card. The 2 MB track
  arrives in OPFS `mods/tracks/web_store_track/` with all four files, and store.json names the zip's sha. The
  download is gone from OPFS `store/`. Two zip GETs (the store, then the CDN), none with Range. The screenshot I
  looked at by hand showed the card "Installed" and the note "Installed Web Store Track".
- `the_shell_builds_for_the_browser`: HOST_CALLS now 23, matching the glue.

## Not verified / not 1:1

- **Browser track select does not see a browser install yet.** track_mods, worlds and game-sim read std::fs. The
  wasm shell's `crate::files` is an in-memory map of OPFS `files/`, not `mods/`. So in the browser the mod is
  installed (in OPFS `mods/`) but Free Ride does not list it. Filed as P217.
- **The shell's hostile-input parsers do not run in the browser** (the stage is native-only), only the zip and
  mod.json checks. Same reason, also P217.
- **No live install against huggingface.co.** The store is empty until P199 seeds it. The API shapes (tree,
  discussions, refs, resolve) are written from HF's documented responses, not from a live reply.
- **CORS from the worker to huggingface.co** (API and resolve and the CDN redirect) is unverified. The browser
  test serves the fake from the page's own origin.
- **The Report URL's prefill** (`discussions/new?title=&description=`) is HF's documented form, but untested on
  the site. Opening it natively (`xdg-open`) and in the page (`window.open`) was not exercised.
- **"No restart" on desktop** is the existing rescan path triggered by Installed/Removed. The tests prove the
  install lands where `track_mods::find` reads. No test drives the shell's loop through an install into Free
  Ride's list.
- **index.json's format** is this side's reader only. P199's writer must match it:
  `{version: 1, pulls: {<n>: {commit}}, entries: [Entry]}`. Each entry is re-checked through meta_of.
  - `main`'s entries are trusted as the index has them, with no check of `main`'s head. A mod merged after the
    index was built is missing until the index is rebuilt.
- **Previews**: a PNG preview decodes natively only (wasm decodes webp only). The browser test had no preview (404
  → the empty cover).

## Next

`./kit done P198`. Follow-ups filed:
- P217: the browser's Free Ride reads OPFS `mods/`, and the parsers run in the browser.
- P218: a live store check once P199 seeds the dataset.
- P219: the 4K GPU timing test fails under load. It reddened this task's first gate at load ~40 and passed on a rerun.

`tests/menu.rs::the_mods_page_lists_music_packs_beside_the_track_packs` pinned the Mods page's rows, so it now
expects the new Browse the Store row.
