# 1 — native player build (FINAL)

Task P262. 2026-11-02.

## What I did

- `tools/build.sh`: the default is now `--features native-sim`; `--exact-sim` builds without it (`--native-sim`, F15's
  flag, still accepted and the same as the default). New `tools/build.sh features [--exact-sim]` prints the cargo
  arguments the choice makes, so pages.yml's Linux beta job (which builds without build.sh's docker step) and the test
  read one source.
- `tools/web.sh`: the release page builds native-sim; `--exact-sim` for the exact page; `--check` (what tests/web.rs
  and tests/netplay_browser.rs build) stays exact. The native page builds in `target/web-native`, its own directory,
  so a release build never swaps the pkg/ a running test is serving for a native one.
- `tools/ios-ipa.sh`: the app builds native-sim (`--exact-sim` to opt out); tools/ios-build.sh itself is unchanged
  (a developer's plain iPhone build stays exact, like `cargo run`).
- `.github/workflows/pages.yml`: the Linux beta job adds `$(bash tools/build.sh features)`; the Windows, web and iOS
  jobs already go through build.sh / web.sh / ios-ipa.sh defaults. The beta notes gain a line on the native sim.
  ci.yml untouched (exact).
- `tools/release.sh`: its notes say every download runs the native sim, what that means online, and where the
  exact build is. RELEASE_NOTES.md gets the player-facing bullet.
- The online build id already separates the two (F15's `sim_id`, ", native sim"):
  build_stamp.rs's existing `a_native_sim_builds_id_is_not_an_exact_ones` covers it. The menu corner and `--version`
  already say "native sim" (F15).
- Tapes (game-sim `PadTape`): a new `native_sim` field, written as a `sim native` line only on a native ride (an
  exact tape's bytes are unchanged, every older tape reads as exact, `.p2m2` is exact); `sim exact` also parses, any
  other word is an error. `Recorder::new` stamps `ps2_float::NATIVE`. `PadTape::sim_warning(native)` gives the line
  the shell prints on `--play` when the tape's sim is not this build's. Tapes are the only replays the shell writes.
- Cargo.toml's comment on `native-sim` now says the player builds turn it on and that it must not become a default.

## What the original does

Nothing: build plumbing.

## How it was proved

- `crates/openmx/tests/player_build.rs` (3 tests, gate): runs `tools/build.sh features` (default and `--native-sim`
  → `--features native-sim`, `--exact-sim` → nothing) and checks the linux/windows builds pass it on; checks web.sh,
  ios-ipa.sh, pages.yml and release.sh build the player builds native and web.sh `--check` exact; checks no
  Cargo.toml makes native a default, ci.yml never names it, gate.toml's only native build is F18's variant, and the
  test binary itself is exact (`!ps2_float::NATIVE`).
- `crates/game-sim/tests/replay.rs` `a_tape_names_its_sim_and_the_other_sim_warns`: the round trip, the exact
  file's unchanged bytes, the warning both ways, the recorder's stamp — run on the exact build and on the native
  one (`--features ps2-float/native --target-dir target/native`), both pass.
- `cargo check -p openmx --features native-sim` builds.

## Not verified / not 1:1

- No player build was actually run here: not tools/build.sh linux (docker), windows, web.sh's native page,
  ios-ipa.sh, or the Actions jobs; only `tools/build.sh features`, `bash -n`/`sh -n` of every edited script, a YAML
  parse of pages.yml, and the native `cargo check`. The first beta after the merge is the end-to-end check.
- The shell's `--play` warning line was not seen on a real run (needs a disc ride with a tape of the other sim);
  it is a println of `sim_warning`, which the replay test covers.
- Online play: an exact dev build (`cargo run`) no longer meets the downloads online — by design (F15's id), and
  the user's ruling is that peers share the native pool.

## Next

none
