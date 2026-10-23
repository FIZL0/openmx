# 1 — loading screen (FINAL)

Task P127. 2026-10-23.

## What I did

- **Where the time went.** world_mxu_free from the user's mods folder, release, headless: 39 s to load,
  38.9 s of it in `worlds::chunks` (the drawn ground's chunks, CPU), `set_ground_painted` ~0.3 s, the
  model upload a fraction of a second. So the freeze was CPU work on the window's thread, not the GPU.
- **The build on a worker** (`src/loading.rs`, `ride()` in `main.rs`). Everything `ride()` did before
  it touched the window — modifiers, disc, archives, scene, tiles and simulation ground, props, models,
  riders, biplane, traffic, the spawn — is now one closure, `build`, unchanged code un-indented inside
  it (so the diff and other agents' merges stay small), which returns its ~55 locals as a tuple that
  `ride()` destructures under the same names. Under the front end it runs on a scoped thread
  (`loading::on_worker`) while the window's thread draws the loading screen; with no front end it runs
  in place as before (`OPENMX_THREADED_LOAD=1` puts it on the worker there too, for the hash test).
  The thread is *scoped* because the simulation's ground and the drawn ground borrow the terrain's
  bytes: `ride()` owns two `OnceLock`s (the bytes, the simulation's parse) that the build fills, so
  what it hands back may borrow them. When the GPU will want them, the build makes the ground chunks
  too (`worlds::chunks_counted` ticks the bar per chunk: "Ground 812 of 4160").
- Made `Send` for it: `disc::Image` (the web's `HostImage` is two `u64`s, still fine) and
  `tiles::Source` (game-sim's own test `Memory` moved from `Rc<Cell>` to `Arc<AtomicUsize>`).
- **On the window's thread**, between loading frames: sound, the other riders' rigs (a frame each),
  then the GPU — the ground in one call, the models `UPLOAD_SLICE` (16) at a time with a frame
  between (`Renderer::upload_models`, `upload_all`'s own order). Release: 0.66 s for 180 models and
  4160 chunks.
- **The loading screen** (`menu_screens.rs` `Pen::loading`, `Menu::loading`): the menu's backdrop and
  logo, the track's still framed as the pick shows it with the pack's tag, "Loading" over the track's
  name, a bar in the menu's glass and lit colours filled by stage (each of scene, tiles, props, models,
  ground, upload an equal share, counted stages filling theirs), the stage line under it and a percent,
  the title "Loading", Circle — Cancel, and the ticker. Drawn by sprites on the GPU window, by the
  software path otherwise.
- **Cancel**: Circle on any menu pad or Escape (on its edge). The flag is read by the build at its next
  stage or ground chunk, `on_worker` joins the thread before it returns, and the front end is put back
  on the pick it rode from (`Menu::back_to_pick`: the screens the ride was picked from are kept in
  `ride_stack`), cursor on the track. Online, a cancel abandons the lobby's ride (`Network::abandon`).
  A failed load (or a panic in the build) comes back as the error it always was: "Could not ride …".
- **Test hooks**: `OPENMX_LOAD_HOLD=<stage>[:<seconds>]` holds the build at a stage;
  `OPENMX_LOAD_CANCEL=<frames>` presses Back after that many loading frames.
- **Online**: nothing new was needed, and a test now says so. The shell makes a seat's `Netplay` (its
  ggrs session) only after its ride is built, ggrs runs no tick until every peer has synchronised, and
  `recover` gives a never-heard peer `LOADING` (60 s) before it counts as away. So every seat's tick 0
  waits for the slowest load.
- **Split screen** was already one `ride()` for every seat, so one load.
- opentennis: its P50 (`hst-play/src/preload.rs`) is the same idea in bevy — the loader on the async
  pool from the moment the menu asks, the loading screen up meanwhile, "same loader, same data: only
  where and when it runs changes". Nothing to copy code-wise (bevy vs. our own loop).

## What the original does

Not a port of the original's loading screen: the original shows its own still and tip screens while
the PS2 streams; this is the port's front end, in its menu's style, as the brief asks.

## How it was proved

- `tests/loading.rs` (9 tests): the worker with no disc — a load held at a stage keeps the waiting side
  turning frames (30+ while held) and a cancel returns with `loading::workers() == 0`; a finished load
  hands back what borrows memory outside it, the bar only fills, and ends at 5/6 ("Ground 10 of 10");
  a failed or panicking build is an error and its thread is gone; the hook parses. From the disc:
  the loading screen draws (`target/tmp/p127-loading/loading.png`, looked at: Dallas, MX Unleashed
  tag, bar at 20 %), its bar is what moves between 20 % and 80 %, and it is not the pick. The shell:
  a load from the front end held 4 s at the models keeps drawing (≥ 1 frame a second asked of a debug
  build under the gate) and the upload draws frames between slices; a load held until cancelled goes
  back to `Some(Tracks)` with "the load's thread is gone (0 running)" and no ride; a ride built on the
  worker has the plain build's ride hash and end position (free06, 120 ticks of full throttle).
- `tests/netplay.rs` `the_ride_starts_only_once_a_slow_loading_peer_is_in`: `g` silent from the first
  round for 20 s (its load), the host stays on tick 0 throughout and never calls it away; once in, the
  two ride 200+ ticks in sync from tick 0, no restart. (The harness gained `run_asleep`.)
- By hand, release, a real X window on :0: world_mxu_free from the front end — **3045 frames drawn over
  the 50.7 s build** (60 a second, the display's rate) and 3059 to the ride's first frame.

## Not verified / not 1:1

- **The desktop's "Not Responding" itself** was not observed either way: no compositor check here. What
  is proved is that the event loop is polled and a frame presented every 16 ms all through the load.
- **The ground's upload is one call** (`set_ground_painted`, ~0.3 s for 4160 chunks in release): one
  frame of ~0.3–0.7 s with no events polled. Short of the desktop's threshold, but not sliced.
- **Sound and the other riders' rigs** run on the window's thread with a loading frame between each
  rig; how long one rig takes was not timed.
- **Online timing**: the harness models a loading peer as one that has sent nothing; in the shell its
  ggrs session is made after the load, while the test's session exists from round 0 and only stays
  unpolled. A load past `LOADING` (60 s) still counts the peer as away, as before P127 — 50 s for
  world_mxu_free here is close; online rides disc tracks only (no worlds) for now.
- **Not tested with a real pad's Circle**: the cancel tests use the hook; Circle goes through the same
  `read_hosted` presses as the menus (`p.back`), Escape through `window.pressed`, neither driven here.
- **An online cancel** (`Network::abandon`) is not covered by a test.
- **The GPU window's loading screen** was seen running (frame counts) but not looked at as a picture;
  the picture looked at is the software draw. The sprite path is the menu's own (`Art::sprites`).

## Next

Nothing for this task. If world loads grow past 60 s, online would need `LOADING` raised or a loaded handshake.
