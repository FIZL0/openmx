# 1 — stream the single put (FINAL)

Task P221. 2026-11-04.

## What I did

- `modstore_up.rs`: the LFS `basic` action (one PUT, used when the Hub gives no `chunk_size`, which
  today means mods under its multipart cutoff) no longer calls `Blob::all()`, which read the whole
  mod.zip into memory. A new `Blob::body()` returns a `reqwest::Body`: the bytes for `Blob::Bytes`, and
  for `Blob::File` the opened `tokio::fs::File`, which reqwest sends from disk a buffer at a time.
- The PUT now sets `Content-Length` itself. A streamed body has no length hint, and without the header
  hyper sends it chunked, which S3's presigned PUT refuses. `body()` also checks the file's size on
  disk against the size the Blob carries, the size the batch request announced, and refuses with "the
  zip changed while uploading" if they differ, so it never sends a length that doesn't match the bytes.
- Cargo: native `reqwest` gets the `stream` feature and native `tokio` gets `fs`. Neither adds a crate:
  `tokio-util`, `futures-util` and `wasm-streams` were already in reqwest's locked tree (another crate
  in the workspace turns `stream` on already).
- `Blob::all()` is still used for the commit's `regular` (non-LFS) files, which are small JSON and
  README files sent inline as base64.
- Nothing taken from opentennis: it has no LFS upload.

## What the original does

Nothing: this is the app's mod store, not game logic.

## How it was proved

- New test `modstore::up::tests::a_small_zip_goes_up_in_one_put_from_the_file`: a 500-byte file
  `Blob` goes through `lfs_put` to the fake Hub on localhost. The fake reads only `Content-Length`
  bytes and stores them. The test asserts the store holds exactly the file's bytes and that no
  multipart upload was used. A Blob whose size is not the file's is refused.
- Mutant: with the `Content-Length` header removed, the test goes red (the fake got an empty body and
  the verify step 404s). With the header back it passes. `./kit check -- -p openmx --bins --
  modstore::up`: 13 tests pass, including the existing upload/re-upload/take-down, world and
  multipart-retry tests.

## Not verified / not 1:1

- Not tried against the real Hugging Face / S3 endpoint: an upload would publish from this machine's
  FIZL0 token, which is not allowed here. That S3 takes an explicit Content-Length with a streamed body
  is from S3's documented PUT rules, not observed.
- The wasm32 (browser) build was not compiled: neither the system rustc nor the 1.99.1 rustup
  toolchain here has the wasm32 std. Its `body()` arm is `Bytes` → `Body` (reqwest's `From<Vec<u8>>`,
  which wasm also has) plus the same `File` refusal as `read()`. The browser's fetch drops a
  `Content-Length` header it is given, which does no harm there.
- Peak memory was not measured. That the file is no longer read whole follows from the code
  (reqwest's `From<tokio::fs::File>` is a `ReaderStream`), not from a profile.

## Next

The work is done and pushed on task/P221, but `./kit done` could not tick it. Main's gate is red at
36a51c7f on two tests this task doesn't touch:
- `tests/web.rs the_browser_installs_from_the_store`: "listed"/"pictured" stay null. It fails the same
  way on bare HEAD in a scratch worktree, run alone. Filed already as P294 (critical).
- `game-sim tests/kiowa_capture.rs the_rotors_probe_is_the_images`: the c14 kiowa image is missing in this
  slot (F33).
A third failure, `world_ultra_load`'s first frame at 35 s against a 20 s limit, passed when rerun alone
(load).
Once P294 and F33 land, rerun `./kit done P221 "<msg>"` to tick it. The code needs no change.
