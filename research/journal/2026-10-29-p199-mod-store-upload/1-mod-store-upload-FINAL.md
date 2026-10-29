# 1 — mod store upload (FINAL)

Task P199. 2026-10-29.

## What I did

- `src/modstore_up.rs` (a child of `modstore`): the upload side of P198's store.
  - **Sign in**: Mods → Upload to the Store (or R1 past Music) shows the rules first (`menu_store.rs`
    `STORE_RULES`, Cross agrees, once a session). Then *Get a token* opens
    `huggingface.co/settings/tokens/new?tokenType=write` and *Paste token* reads the clipboard
    (`wl-paste`/`xclip`/`xsel`, `pbpaste`, PowerShell; in the browser, the last text pasted into the page),
    finds the `hf_…` token in it, asks `whoami-v2`, refuses a read-only token, and keeps `name\ntoken` in
    `hf-token.txt` beside settings.txt, created 0600 (an older file tightened before the token goes in). The
    browser keeps it in localStorage `openmx.hf-token` (`web_host::token`/`token_keep`, worker.js, index.html),
    left out of Export settings. Paste, because the on-screen keyboard has no `_` or capitals.
  - **Upload my mod**: the player's own mods are every folder under mods/tracks, tricks, music with a
    `mod.json` of that kind and no store.json (the store's installs are not theirs). `own-disc` and a missing
    `permission` are refused with the reason on the row. Packing walks the folder with Install's own rules
    (plain names, no links, the store's extensions, the caps), streams a zip to `mods/.upload-<id>.zip`
    (deflate unless it doesn't shrink), reads it back with `ZipDir` and runs `check_mod_json` and the shell's
    parsers (`parse_check`) on it, so what goes up is what Install takes. A track/world's preview still goes
    up as a 480×270 lossless webp. Commit: preupload → LFS batch (multipart parts read from the file, or one
    PUT) → verify → NDJSON commit of `mods/<id>/mod.zip`, `meta.json`, `preview.webp`.
  - **Re-upload onto the player's own PR**: the player's open PRs are read once; a mod already on one of
    them is updated in place (same zip → skipped, older version → skipped); otherwise it goes onto their
    newest open PR, else a new PR (`create_pr=1`). So a player has one PR, one commit a mod.
  - **Upload all**: every uploadable row in turn, in the background, each row's status line
    (Waiting, Checking, Uploading mod.zip: n / m MB, then Done, Skipped or Failed).
  - **My public mods**: their open PRs' mods and their mods on main; Cross arms, Cross again takes down: a
    mod sharing a PR is removed from it (`deletedFolder` commit), a PR's last mod closes the PR, a merged mod
    becomes a PR deleting it (for a maintainer to merge).
  - `openmx --mod-store-admin [--token F] [--store URL] card README.md | index | upload MODS [ID…]` for the
    maintainer (default token `~/.config/openmx/hf-token.txt`).
- **The dataset card**: `research/p199/dataset-README.md` (what the store is, the rules, the layout,
  meta.json and index.json, what openMX checks), committed to FIZL0/openmx-mods main with the stored token:
  https://huggingface.co/datasets/FIZL0/openmx-mods/commit/372ab22f1bd7404e5db35f4ac2a708895bf80e84 —
  read back byte for byte, and the Hub parsed its YAML (license other/per-mod, tags).
- **Seeding: not done, on purpose.** All 62 mods in ~/repos/openmx/mods say `"permission": "own-disc"`
  (the elev_* worlds only for MX vs. ATV's sky; openmx-mods' notes/elevation.md says built with
  `--sky none` they are public-domain). Putting them on a public store would break the card's own rule 3
  and the standard's meaning of own-disc, so plan/HUMAN.md § Seeding the mod store says how to rebuild
  shareable ones and upload them.
- Follow-ups: P220 (Upload in the browser), P221 (stream the single-part LFS PUT).

## What the original does

Nothing: the store is the port's own.

## How it was proved

- `src/modstore_up_tests.rs` (7 tests) against a fake Hugging Face on localhost that takes preupload, LFS
  (one PUT under 600 bytes, 256-byte parts above, joined and sha-checked on completion), verify, NDJSON
  commits (main, `create_pr=1`, an open PR only by its author), PR status and the listings P198 reads:
  upload → another player's Browse lists it as `tester/a_music` on refs/pr/1 and installs it byte for byte
  → a new version updates the copy on PR #1 → a second mod joins PR #1 → the same zip is skipped → Upload all
  refuses own-disc and skips the rest → My Mods lists both → take-down of another's refused, one taken out
  of the PR, the last closes it → on main, take-down opens a deleting PR. Also: token parsing, the 0600 file
  (and a 0644 one tightened), read-only token refused and never kept, own-disc/no-permission refused, store
  installs not offered, pack round trip through `ZipDir`, `.exe` and links refused, preview webp at 480×270,
  the re-upload plan, base64, the maintainer's index parsed back by P198's `parts`.
- `tests/store_menu.rs`: the five tabs round on L1/R1; `the_upload_and_my_mods_pages_ask_only_when_they_may`
  walks the rules, sign-in rows, Upload/Upload all, a refused row, My Mods' arm-then-confirm take-down.
- `tests/web.rs` `the_shell_builds_for_the_browser`: the browser build with the three new host calls.
- Live: the card commit above (the real commit API, a regular file on main).

## Not verified / not 1:1

- No live upload, re-upload or take-down against the real Hugging Face: that would publish a test mod on
  the public store. The commit path for a regular file (the card) is proved live; LFS (preupload's
  `uploadMode`, the batch's multipart `chunk_size`/completion shape, verify) only against the fake, written
  from huggingface_hub's protocol.
- Whether a fine-grained token (the user's is one) has write on FIZL0/openmx-mods for another player is the
  token's own scopes: sign-in only refuses role `read`; a fine-grained token lacking the repo's write fails
  at the commit with Hugging Face's message.
- The browser: sign-in/My Mods/take-down are wired (localStorage, paste listener, worker host calls) and
  build, but not driven in headless chromium, and CORS on whoami-v2, commit and discussions status from the
  page is unchecked. Upload itself is desktop-only (P220).
- Clipboard: `wl-paste` is what this desktop has; xclip/xsel, pbpaste and PowerShell paths are untested.
- The single-part LFS PUT holds the whole zip in memory (P221); multipart reads parts from the file.
- The new pages take the pad and keyboard; mouse clicks on their rows do nothing.
- No `index.json` written: Browse falls back to the crawl with none, and an index would go stale with
  every PR until a maintainer reruns `--mod-store-admin index`.
- The store is not seeded (see above; plan/HUMAN.md).

## Next

Nothing for an agent: the human's seeding step is in plan/HUMAN.md.
