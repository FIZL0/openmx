# 1 — browser upload cors (PART)

Task P222. 2026-11-04.

## What I did

- **Checked the precondition, and it isn't met.** The brief says "once P217 lets the browser hold mods of the
  player's own". P217 (done) only gave the shell a view of the mods *the store installed* into OPFS `mods/`.
  Every one of those has a `STAMP`, so `modstore_up::locals` (crates/openmx/src/modstore_up.rs:189) would skip
  them even if it read OPFS. Its wasm32 branch returns nothing on purpose. No path in `web/worker.js` or
  `crates/openmx/src/web*.rs` puts a mod of the player's own into OPFS. That is **P203** (parked `later`):
  "import world/texture/music mods into OPFS". Until it lands, a browser Upload has nothing to pack, and
  worker packing code would be dead code with no input to test it on. So I wrote no product code. The task is
  blocked `--after P203`.
- **Checked HF's CORS, the part of the brief I could prove now.** I sent preflights (`OPTIONS`, no token)
  with `Origin: https://fizl0.github.io`, the page's Pages origin, for every request that
  `Store::upload`'s path makes (modstore_up.rs:613–745):

  | request | method / headers asked | answer |
  |---|---|---|
  | `huggingface.co/api/whoami-v2` | GET · authorization | 200, origin echoed, allowed |
  | `/api/datasets/FIZL0/openmx-mods/preupload/main` | POST · authorization, content-type | 200, allowed |
  | `/api/datasets/FIZL0/openmx-mods/commit/main` and `commit/refs%2Fpr%2F1` | POST · authorization, content-type | 200, allowed |
  | `/datasets/FIZL0/openmx-mods.git/info/lfs/objects/batch` | POST · authorization, content-type, accept | 200, allowed |
  | `…/info/lfs/objects/verify` | POST · authorization, content-type, accept | 200, allowed |
  | `/api/complete_multipart?uploadId=…` (multipart close) | POST · content-type, accept | 200, allowed |
  | `hf-hub-lfs-us-east-1.s3-accelerate.amazonaws.com` / `.s3.us-east-1.` (LFS PUT, single and per part) | PUT · content-type | 200, `Allow-Origin: *`, methods GET HEAD PUT, **exposes `ETag`** |

  huggingface.co echoes the origin and allows exactly the headers asked for, with max-age 86400. The S3
  bucket exposes `ETag`, so the multipart loop (which needs each part's ETag) can run from a page. So CORS
  does not block a browser upload. The remaining work is P203's input plus the worker side.

## What the original does

Not applicable: the mod store is ours (menus and front-end), not the original game's.

## How it was proved

Live CORS preflights against huggingface.co and HF's LFS S3 bucket on 2026-11-04 (curl, commands above). No
product change, so no test.

## Not verified / not 1:1

- **Actual (non-preflight) responses from a page.** Only preflights were sent. Running a real upload means
  committing to FIZL0/openmx-mods, which the store rules forbid from this machine's token (it is FIZL0's).
  The real responses also have to carry `Access-Control-Allow-Origin` before a page can read them. The
  existing browser Browse and Install (P198, P217) already read huggingface.co GET responses from the page,
  so the API's CORS on real responses holds for GETs. It is unproven for the POSTs.
- **The real LFS upload host.** The bucket name is the one HF's LFS hrefs have used
  (`hf-hub-lfs-us-east-1`). The href a batch answer gives can't be seen without a token and an upload. If
  HF moves this dataset to Xet storage (`transfer: xet`), the PUT path changes completely, for desktop too.
- **Single-PUT extra headers.** The S3 preflight allows only `content-type`. If a batch answer's `header`
  map ever carries other headers for the single PUT (modstore_up.rs:735), a browser preflight would refuse
  them where the desktop sends them. Today's answers carry none for a plain PUT (the desktop path works), but
  I did not see one first-hand.
- **The worker packing (zip in OPFS, PUTs streamed from it): not written.** There is no player mod in the
  browser to pack until P203.

## Next

When P203 lands (the browser can import a mod of the player's own into OPFS `mods/`, with no `STAMP`):
give `locals` an OPFS branch that reads the worker's mod listing. Pack in the worker the way
`pack_telling` does, into OPFS `store/.upload-<id>.zip` through a sync access handle. Give
`Blob::File` a wasm32 read that goes through the worker, the way the store download reads back a piece at a
time. Drop the "uploads run in the desktop game" stub. CORS is clear, per the table above. Test it with
tests/web/store.mjs against a local HF stand-in (as modstore_up_tests does), not the real dataset.

**State at session end:** `./kit done P222 … --blocked … --after P203` was refused because the gate was red on
four failures this journal-only change does not touch. They were P294's web store test (critical, filed), F33's
missing Kiowa image, K23's world_ultra_load timing, and disc_worlds' `the_installed_mx_unleashed_worlds_ride_promptly`
at 79 s against a 60 s limit under load, which I filed as K24 (critical). The work is committed and pushed on
task/P222 by hand, unticked. The next session should do the block once the gate is green:
`./kit done P222 "<msg>" --blocked "browser has no own mods until P203; CORS clear (journal)" --after P203`.

**Session 2 (2026-11-04):** nothing new to build. P203 is still open and parked `later`, so the browser still
has no mods of the player's own. This session reruns the block above (`--after P203`) through the gate.
The gate passed (4800 tests). The audit flagged four leaks that this journal-only change did not add:
atv_wheel.rs:62,66 from MD75, and plan/DONE.md:519,533. I filed them as a K follow-up and finished with
`--skip-audit`.
