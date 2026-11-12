# 1 — ship every player build (FINAL)

Task P312. 2026-11-11.

## What I did

- `ship` meant something only on Android (P308, `android.rs`). Now `main.rs`'s `dev_env`, the first call in
  `main` (desktop and iOS; the page has no environment, Android keeps its own drop in `android_main`), removes
  every `OPENMX_*` variable but `OPENMX_TEXTURE_RELOAD` when built with `ship`, and says which it ignored on
  stderr, as opentennis's `dev_env` (B492) does. The keep list is Android's: the other variables the shell
  reads (`OPENMX_BUTTONS`, `_LATEST`, `_UPDATED`, `_UPDATE_API`, `_PAD_ROOT`, `_TRANSFERS`, `_WORLDS`,
  `_RIDING_RAGDOLL`, and game-render's `OPENMX_THREADS`) are developer fakes and fixtures. `OPENMX_COMMIT`,
  `_VERSION`, `_BUILD` are compile-time (`option_env!`/`env!`), untouched.
- `std::env::remove_var` is unsafe in edition 2024, so the crate's lint went from
  `forbid(unsafe_code)` off Android to `deny(unsafe_code)` everywhere (it was deny on Android already), with
  one `#[allow(unsafe_code)]` on that block.
- tools/build.sh: `--features ship,native-sim`, `--exact-sim` → `--features ship` (so `build.sh features`,
  which pages.yml's beta Linux job uses, carries it too); `android` passes `--exact-sim` by a flag of its own
  now that the exact features are no longer empty.
- tools/ios-ipa.sh and tools/web.sh: the same two feature sets. `web.sh --check` (the tests' page) stays the
  dev build, no `ship`; web.sh's out dir now keys on `native-sim` rather than on any features, so
  `--exact-sim` still builds in `target/web` as before.
- tools/release.sh: before anything builds (and in `--dry-run`), refuses when tools/build.sh, apk.sh,
  ios-ipa.sh or web.sh has no `features="--features …ship…"` line — opentennis's grep, web.sh added.
- crates/openmx/tests/player_build.rs pins the new feature lines and the release's check.

## What the original does

Nothing: a port-only feature (the shell's developer variables), no game behaviour.

## How it was proved

- `cargo build -p openmx --bin openmx --features ship` and
  `OPENMX_BUTTONS=xbox OPENMX_TEXTURE_RELOAD=1 OPENMX_UPDATE_API=x openmx --version` printed
  `openmx: a player's build, ignoring OPENMX_BUTTONS OPENMX_UPDATE_API` (texture reload kept).
- `cargo check --features ship,native-sim` builds; `tools/build.sh features [--exact-sim]` prints
  `--features ship,native-sim` / `--features ship`; the release grep matches all four scripts.
- `./kit check --all` (player_build.rs among it).

## Not verified / not 1:1

- No player build was run end to end: not the Linux one (Steam Runtime docker), Windows (MinGW), the iPhone IPA,
  the page (`tools/web.sh` without `--check`) or the APK. `ship` is an empty cargo feature and `dev_env`'s
  only cfg-split is wasm32/android, so they should compile as the host build did, but no wasm32 or iOS
  compile of the new code was done here.
- tools/release.sh itself was not run (no release, by the task's rules); its new loop was checked by running
  the same grep over the four files by hand.
- On iOS `dev_env`'s "ignoring" line goes to stderr before the log moves to Documents, so it does not reach
  the phone's log (no iPhone here to see; iOS apps get no environment from a player anyway).

## Next

Nothing.
