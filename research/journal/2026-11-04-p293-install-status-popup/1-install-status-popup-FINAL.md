# 1 — install status popup (FINAL)

Task P293. 2026-11-04.

## What I did

- New `crates/openmx/src/menu_transfers.rs`, mounted inside `menu.rs` as `mod transfers` so it can reach Menu's
  private fields. It holds opentennis's transfers.rs model, cut down to openmx's shop:
  - **Batch counting.** `Transfers` keeps `all`/`up_all`, and `Shop::queued(n)`/`up_queued(n)` start a new batch
    when the asked items are all that is in flight. Done = all − in flight. Downloads in flight are store cards
    with `progress.is_some()`. Uploads in flight are `locals` in `Phase::Busy`.
  - **The strip.** `Shop::strips()` builds one `Strip` per direction: "Downloading 3/5: name", or "1-3/5: name +2"
    when several items are going at once. Its bar share counts a going upload as half done.
  - **When to ask.** `Menu::store_installed(key, ok)` is called by the shell on every install event. A good
    install marks the batch fresh and the mods changed. Once nothing is in flight, the ask is due.
    `calm()` limits where the popup may come up: Main/Online/Settings/Mods/Modifiers/Original/Store, no pause,
    no loading/listing/quit/leave/store-arm/other ask open. So the popup comes up on the screen that is up,
    or on the next calm one.
  - **Reload / Later.** The popup reads "Load the new mods?" with the rows Reload / Later. Later leaves
    `changed` set, and the Reload Mods pill then sits where the strip was (`reload_button()`), pressed
    with Select. Free Ride / Local / Online on Main then asks "Reload now / Play without them" once
    (`play_ask`). "Play without them" sets `without`, so that batch doesn't ask again. Either answer
    then goes on into the pick it was asked from.
- `menu.rs`: `walk` gives `walk_transfers` the first look at the pad. Main's play rows go through `play_ask`.
- `menu_store.rs`: `Shop.t`, `StoreAsk::Reload`, and queued() calls on Download All, a single install, Upload All
  and a single upload.
- `menu_screens.rs`: `Pen::transfers` draws the pill (dark rounded box, fitted line, PEACH bar), the Reload
  Mods button, and the popup with its hints. It draws on every screen except over the pause.
- `main.rs`: an install no longer rescans the mods folder at once. It calls `store_installed`, and the rescan
  waits for `StoreAsk::Reload`. Removals still rescan at once. The rescan calls `mods_reloaded()`.
  `--screen store` with `OPENMX_TRANSFERS=1|ask|done|play` fakes a batch for screenshots.

## What the original does

Nothing: the store is openmx's own. The model is opentennis's (M136 587a6689 pill on every screen, M141
4bbd99ab reload popup on a calm screen, B601 5f0207bf don't ask while queued, transfers.rs).

## How it was proved

- `cargo check -p openmx --bins --tests` is clean apart from an unused-method warning (`mods_waiting`).
- `cargo test -p openmx --test menu --test store_menu`: 263 + 126 passed, 0 failed. That includes:
  - `store_downloads_show_on_every_screen_and_end_in_the_reload_popup`. Select on the store queues two
    items. Back to Main shows "Downloading 1/2: a/nat01" with more sprites drawn than without a batch.
    Nothing asks while one item is still queued or over the track pick. The popup comes up on Main
    once both are in. Later leaves the button up, Free Ride asks with PLAY_ROWS, and "Reload now"
    sends `StoreAsk::Reload` and opens Tracks.
  - `reload_mods_waits_on_select_and_play_without_them_asks_once`.
  - Unit tests `a_download_batch_counts_down`, `an_upload_batch_counts_down`, `a_failed_batch_does_not_ask`.
- Screenshots on a private weston (llvmpipe), viewed: the upload pill over the store ("Uploading 2/3:
  my_track") and the Reload Mods button on the Mods page. They are kept in the session scratchpad only.

## Not verified / not 1:1

- Not driven end to end against the live store. Every batch here is a test fixture or a faked
  `OPENMX_TRANSFERS` sample.
- In openmx, "Reload" re-reads the mods folder (tracks, music, trick packs). opentennis restarts instead.
- A track installed from the store no longer shows in the pick until Reload (or Play → Reload now).
  Removals stay live, as before.
- The Reload Mods button is Select-only, because the front-end menus take no mouse clicks. It is hidden on
  the Store, where Select is Download All.
- Neither the pill nor the popup is drawn over the pause menu.
- The popup's hint bar draws over the screen's own (dimmed) hints, not in place of them.
- The Main-screen popup and Play-ask screenshots never wrote a png within 240 s on llvmpipe, so they
  were only checked by the menu tests.

## Next

Nothing.
