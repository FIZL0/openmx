# 1 — remove resume offer (FINAL)

Task P320. 2026-11-06.

## What I did

User, 2026-11-05: "the resume ride should be removed, it usually doesnt work". I took out part (2) of
P298, the saved point and the main menu's offer, and kept part (1), the pause and the memory give-back
while the app is in the background.

- `crates/openmx/src/resume.rs` is deleted: the `resume.txt` point (screen or ride + seat state in hex),
  its save/load/clear and its four tests (screen round-trip, ride round-trip bit-exact, the offer popup,
  a file with no place).
- Its three lifecycle wrappers that part (1) still needs, `away`, `answered` and `backgrounds`, moved to a
  new `crates/openmx/src/away.rs` (`mod away`). `resume::back()` is gone: it only cleared the file.
- `main.rs` (`android_so.rs` is a symlink to it, so Android is the same file): `Front::resume` and its
  `Resume` struct; the launch's `resume: offering …`; the main loop's answer to the offer; the resumed
  ride's pick (`resume_pick`) and the state put over its first seat; `resume_point`,
  `ride_resume_point`, `keep_resume_point`; and every `resume::clear`. Going away still pauses the ride
  (`menu.pause()`), still gives the ride's models back on DidEnterBackground (P298a), still gives the
  menu's GPU assets back, and still answers (so iOS's DidEnterBackground wait ends at once). While a
  ride loads, going away is just answered.
- `menu.rs` / `menu_screens.rs`: the `Offer` popup, `OFFER_ROWS`, `Screen::resumed_as`, `Screen::name`
  (only the offer used it), `Menu::offer`, its press hook and its drawing. All were P298's (63257086).
- `crates/openmx-ios/src/sys.rs`: the `BACK` counter and `take_back` (only the file's clear read them);
  doc comments no longer say "saves". The DidEnterBackground log says `answered after N ms` where it
  said `saved after N ms`; the ride's give-back logs `away: gave back …` where it said `resume: …`.
  P307 is noted with the new strings. `ios.rs`'s comment on `watch_lifecycle` updated.
- `plan/HUMAN.md`: the P298 item's step 2 ("That resume works") is gone; step 1 (why iOS ended it)
  stays, with the new log line.
- Android never wrote a point (`resume::ON` was iOS-only, and `away()` is `None` off iOS), so on Android
  this removes dead code only.

opentennis: nothing taken; it has no resume offer to compare with.

## What the original does

Not applicable: front-end/platform behaviour, ours to design.

## How it was proved

- `away::tests::the_main_menu_after_a_launch_takes_its_own_first_press`: a fresh main menu's Cross (and
  Down, Cross) opens a row, so no popup sits over it. `Menu` no longer has an offer to put up at all, and
  the launch path has no code that reads `resume.txt`.
- `cargo build -p openmx` (desktop) and `tools/ios-build.sh check -p openmx --bin openmx`
  (aarch64-apple-ios) both build, with no new warnings.
- `./kit check --all` through `./kit done`.

## Not verified / not 1:1

- Not run on an iPhone or an Android device: that a relaunch after iOS ended the app goes to the front
  end with no prompt is shown by the code (nothing reads the file, no offer exists) and the menu test,
  not on a phone. No device is reachable from an unattended run.
- A `resume.txt` already left in a phone's Documents by an older build stays there, unread. It is a few
  bytes and does nothing; deleting it would mean keeping code that knows the file, which the task said
  to remove.

## Next

None.
