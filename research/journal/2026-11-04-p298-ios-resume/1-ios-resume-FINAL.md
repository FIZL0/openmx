# 1 — ios resume (FINAL)

Task P298. 2026-11-04.

## What I did

The ask: the iPhone app relaunches after a while in the background instead of resuming. iOS already
suspends an app in memory and resumes it as it was, so a relaunch means iOS ended the suspended app
(most likely for its memory). Two parts, both done in part:

- **Going away** (`openmx-ios/src/sys.rs`, `watch_lifecycle`, called from `ios.rs` beside
  `watch_focus`): WillResignActive, DidEnterBackground, WillEnterForeground, DidBecomeActive and
  DidReceiveMemoryWarning observed on the main queue. Going away counts one (`away()`), which the
  shell's thread answers on its next frame (`answered_away`); DidEnterBackground waits for that
  answer on the main thread for up to 2 s of iOS's ~5 s, so the save is on disk before the app is
  suspended, and logs `saved after N ms` with the memory. Memory warnings are logged with the memory
  (opentennis's B538). Coming back counts one (`take_back()`).
- **The shell's answer** (`main.rs`): in a ride, 1P's pause opens and the resume point is written; in
  the menus, the screen is written and the menu's GPU models (Garage) and pictures are forgotten
  (`Renderer::forget_assets`, rebuilt when next drawn); while a ride loads, the ride being loaded.
  Coming back to the same process takes the file away again, so it only exists while the app is away.
- **The resume point** (new `src/resume.rs`, `resume.txt` beside `settings.txt`): a menu screen by its
  `--screen` word, or a ride: the track, the first seat's pick (bike, set, rider, look) and its whole
  state in the resync's codec (`net::rides::seat_bytes` → `game_sim::snapshot::encode`), hex. Written to
  a side file and renamed. Stamped with the build id; another build's state is dropped (the codec may
  read it otherwise) and the ride starts from the spawn.
- **The offer** (`menu.rs` `Offer`, drawn by `menu_screens.rs` with the Quit popup's `popup`): at launch
  a point found is offered on the main menu once the splash has 1P — "Resume the ride on free06?",
  Resume / Not Now (Circle is Not Now). Resume on a screen opens it (`Menu::open`); on a ride it rides
  the track on the saved pick and, once the ride is built, puts the state over the first seat
  (`from_seat_bytes`, as a resync does a joiner's) and opens the pause. The point is spent either way;
  ended again before it was answered, the offer is saved again.
- Which screens a point keeps (`Screen::resumed_as`): the screen itself where it opens alone; Online for
  Games/Lobby (the room is gone after a relaunch); Garage for the name keyboard; none for the splash,
  the main menu, the join/split/assign screens and anything over a ride (Pause, Tricks, Audio). An online ride saves the
  Online screen; a split screen, a played tape or a seat on a machine other than the bike saves the
  track from its start, no state.

**From opentennis:** it has no save-and-resume (its B567 found iOS resumes a suspended app for hours, and
only re-ran its update check on foreground). Taken: the memory warning log (B538,
`hst-base/src/ios.rs`'s `watch_memory_warnings`), in this crate's hand-made block literal as P233's
`watch_focus` has it rather than objc2's `RcBlock`.

**Followed up:** P298a (give the ride's GPU world back in the background, if the jetsam report says it is
memory); plan/HUMAN.md "Why iOS ended the app in the background, and does resume work (P298)".

## What the original does

Nothing to port: the PS2 game has no suspend. This is the app's own front end.

## How it was proved

`crates/openmx/src/resume.rs`'s tests (`./kit check -- -p openmx --bin openmx -- resume::`, 4 passed):

- `a_ride_point_restores_the_same_ride`: a seat ridden 240 ticks (gas, bars swept) saved through the
  file, read back to the same `Ride`, put over a new seat with `from_seat_bytes`: the same `Sim::hash`,
  and the two ride on 60 more ticks with equal hashes. Read by another build: the ride without its
  state, said.
- `a_screen_point_restores_the_same_screen`: every screen the menu can show, mapped by `resumed_as`,
  through the file and `Menu::open`, shows that screen again (at least 8 kept).
- `the_main_menu_offers_the_point_until_it_is_answered`: the popup takes Up/Down/Cross/Circle on the
  main menu, answers once, and moves nothing under it.
- `a_file_with_no_place_is_no_point`.

`tools/ios-build.sh check -p openmx --bin openmx` builds for aarch64-apple-ios with the new observers.

## Not verified / not 1:1

- **Not run on an iPhone.** No device here: the notifications' order, the 2 s wait in
  DidEnterBackground, the save landing before suspension and the offer after a relaunch are unchecked
  on hardware. plan/HUMAN.md's P298 item says how to check.
- **Why iOS ended the app is not known.** The task asks to check the device log / jetsam report; there is
  no device here. Memory is assumed (the task's own guess); plan/HUMAN.md has the steps.
- **A ride gives no memory back** in the background: its world, models and textures stay on the GPU,
  and the streamed world pieces and audio buffers stay too. Only the menus' GPU models and pictures are
  freed. Re-uploading a ride's assets mid-ride is not something the ride loop does today; P298a.
- **The shell's glue in `main.rs` is not run by any test**: off iOS `resume::away()` is never `Some`, so
  the save in the ride loop and loading frame, the launch's offer and the state put over the first seat
  of a built ride are only compiled. The parts they call (file, codec, menu offer, screens) are tested.
- **A resumed ride is not the same ride in everything the shell holds outside `Sim`:** the camera starts
  over at the bike (a cut), the HUD's clocks and messages, the tire tracks, particles and the music's
  place start fresh, and the seat's machine is the bike (only a bike seat saves its state).
- **Only the screen is kept, not its state**: the Settings tab, the cursor row, a dropdown, a
  scroll or a typed name in progress is not kept.
- **Only 1P's pause opens** on going away; in a split screen the other seats are not paused (iOS
  suspends the process, so they do not ride on while it is away, but they ride on once it is back).
- **Swiping the app away** in the app switcher is iOS ending it too: the next launch offers to resume
  rather than starting clean. The offer can be refused.
- **An online ride is not resumed** (the room is gone); the Online screen is offered instead.

## Next

The human's two checks in plan/HUMAN.md (jetsam report, a resume on the phone), then P298a if memory.
