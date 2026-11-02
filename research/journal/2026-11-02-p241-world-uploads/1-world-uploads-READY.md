# 1 — world uploads (READY)

Task P241. 2026-11-02.

## What I did

- **Worlds are listed as worlds.** Browse cards carry `world` (`menu_store.rs` `Card::world`). The card's
  line reads "World · 316 MB · …" (`menu_screens.rs`). Upload's and My Mods' rows say "World" rather than
  the tab's word (`main.rs` `store_kind`). The maintainer's `index.json` already kept `meta.kind`. A test now
  checks that it lists `"kind": "world"`.
- **The disc-built worlds don't go up.** `locals()` skips a world whose id `disc_worlds::is_disc_world`
  names (world_mxu, world_mxu_free, world_mxu_nat): each player's game builds these from their own disc on
  the first ride. `<mods>/cache/stitch` was never listed, because `locals()` reads only
  `tracks|music|tricks|textures`. A test now pins that too. Own-disc mods such as world_mxvatv and
  world_untamed are listed like any track, which is what 8753a7be (the user's) chose.
- **Caps first, in plain words.** `files_of` already measured every file from its metadata before
  packing, and `package` runs before any request. The messages now say what the cap is: "too big for the
  store: its files come to more than 3.8 GB, the most a store mod may hold", "one file of a store mod is
  1.0 GB at most", "more than 65000 files…". Because a mod's files are held under `MAX_TOTAL`
  (0xf000_0000), its zip stays under `MAX_ZIP` (no zip64).
- **Progress on long uploads.** `pack_telling` and `package_telling` send "Packing <name>: a / b MB" and
  "Checking <name>: a / b MB" every 16 MB. The LFS upload already sent "Uploading mod.zip: a / b MB".
- **A cut upload part is sent again.** Each LFS multipart part gets 3 tries on a send error or a 5xx,
  with the line "part n cut short, sending it again". After that the error says "Upload again to retry". A
  second Upload is a clean retry: the zip is deterministic, so the oid is the same, and the batch API skips
  an object it already has.
- **A cut download carries on (desktop and iPhone).** The download goes to
  `mods/.download-<author>-<id>-<sha16>.zip`. When it is cut short (the body ends early), it asks again
  with `Range: bytes=<have>-`, up to 3 tries, with the line "…: carrying on from N MB". After 3 cuts the
  partial is kept, and the next Install hashes it and carries on from there. Other cases:
  - A CDN that answers a Range with 200 makes it start again from 0.
  - A body that ends whole but short of the listed size is refused, not resumed. This keeps the old
    "less than listed" refusal.
  - The first try sends no Range (B242).
  - A partial of an older zip of the same mod is deleted.
- **Browser.** The browser's Free Ride rides no mod tracks yet (std::fs; P217 / P237). So it lists worlds
  and won't download one: Install says `WEB_WORLDS`, Download All passes worlds by, and the Tracks tab
  shows a line saying so when a world is shown. The **iPhone** app is the native shell (std::fs), so it
  installs and rides worlds through the same code as the desktop. I couldn't run it here.

opentennis (`hst-base/src/mods_online.rs`) has no resume, part retry or multipart, so there was nothing
to follow.

## What the original does

Nothing: the mod store is the port's own.

## How it was proved

- `cargo test -p openmx --bin openmx -- modstore`: 29 passed. The new tests are:
  - `modstore::up::tests::a_made_world_goes_up_and_comes_back_a_world`: a generated two-tile world with
    a join goes up through the fake Hub in LFS parts, with one part refused (503) and sent again.
    `meta.json` says World. `index_now` lists it as `"world"`. Another player has a track installed
    under the id of one of the world's tiles (`shared_tile`). Their Browse lists the world on Tracks as
    a World of the zip's size, with `Have::No`, and installs it into `mods/tracks/my_world`. There,
    `worlds::find` reads 2 tiles + 1 join from the world's own folder, `worlds::listed` (Free Ride)
    lists it, and their own `shared_tile` track still reads its own scene.
  - `…::the_disc_built_worlds_are_not_offered_for_upload`: world_mxu, world_mxu_free and a stitch cache
    are in the folder, and only `my_world` is offered.
  - `…::a_world_over_the_caps_is_refused_before_upload`: four sparse 1 GiB files make the upload fail
    with "too big for the store … 3.8 GB". The fake Hub was asked nothing and no `.upload-*.zip` was
    left. One file over 1 GiB gets its own message.
  - `modstore::tests::a_cut_download_carries_on_from_where_it_stopped`: on the download fake (now with
    Range, cuts and a Range-ignoring mode):
    - cut twice, it is installed in one Install with Ranges `[None, 1000, 2000]`;
    - cut 3 times, the 1500-byte partial is kept and the next Install asks `bytes=1500-`;
    - with a CDN that ignores Range, it starts again and installs the right bytes.
    - No partial is left in any case.
- `cargo test -p openmx --test store_menu`: 111 passed, including the new
  `download_all_passes_worlds_by_in_the_browser`.
- `tools/web.sh --check` builds the wasm shell. This caught and fixed a missing wasm `package_telling`.
- **By hand, real data, against the fake Hub on localhost.** This was a throwaway ignored test, not
  committed. world_untamed (781 MB, 9,916 files, 23 tiles) was reflinked into a scratch mods folder.
  - Upload packed it to a **316 MB zip** in 53 s (packing, the parser check, then LFS multipart in 16 MB
    parts) and opened PR #1 on the fake.
  - A fresh second mods folder listed it as `World`, tab 0, 331,376,852 bytes ("316 MB"), and installed
    it in 24 s. `worlds::find` read 23 tiles and 1,127 pieces, and `worlds::listed` showed it.
  - Then `target/release/openmx --track world_untamed --mods <that fresh folder> --headless --frames 300`
    rode it for 10 s on its ground (untamed_nat_sdca), with props drawn. The scratch folders are deleted.

## Not verified / not 1:1

- **No world on the real store, so there is no PR link.** I didn't upload world_untamed to
  huggingface.co/datasets/FIZL0/openmx-mods. It is 316 MB of files from the MX vs. ATV Untamed disc
  (`"permission": "own-disc"`). A public upload can't be taken back: the LFS objects stay in the PR's ref
  even after it is closed. Also, openmx-mods' standard still says own-disc means "never shared", although
  8753a7be dropped the game's check. I left the one public act to the user (plan/HUMAN.md, "A world on
  the store (P241)"). Everything up to the Hub's HTTP API is proved against the fake, which follows the
  real API (batch, multipart `chunk_size` + numbered part URLs, completion POST, verify, preupload,
  NDJSON commit).
- The real Hub's multipart thresholds and part size, and its CDN's Range answers for `resolve/` redirects,
  are untested. The code handles both 206 and a 200 to a Range.
- Not run on an iPhone. It shares the native code, so it should install and ride like the desktop, but I
  haven't seen it.
- The browser doesn't resume or download worlds. It refuses them in plain words instead (P217 / P237 are
  the follow-ups that would let it ride them).
- A single-part LFS PUT still reads the whole zip into memory. This is P221, already filed. Worlds go up in
  multipart, so they don't take this path.
- Upload's "is up (317 MB)" rounds up and the card's "316 MB" rounds down. This was there before P241 and
  I left it.

## Next

For the human: see plan/HUMAN.md, "A world on the store (P241)". If you want a world on the public store,
upload world_untamed from the game, or with
`cargo run --release -p openmx -- --mod-store-admin upload ~/repos/openmx/mods world_untamed`. Then
install it on a fresh mods folder and paste the PR link here.
