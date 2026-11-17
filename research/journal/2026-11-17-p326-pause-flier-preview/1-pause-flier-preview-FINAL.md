# 1 — pause flier preview (FINAL)

Task P326. 2026-11-17.

## What I did

- **The popup this task names is gone.** P341 (11-13) replaced the pause's Switch To Machine picker
  with the pause **Garage** page: tiles on the left, a cream picture panel on the right. That panel
  showed the tile's baked still. I read the task as "turn the picked flier skin in that panel",
  which is what the picker became.
- **Menu** (`menu_tiles.rs`, `menu_screens.rs`):
  - `Menu::pause_flier`: the lit tile's flier, in the skin the pauser has picked (`roster::flier_of`),
    only on `Screen::PauseGarage`.
  - `Menu::pause_flier_stage`: the panel's inside in frame pixels (`screens::pause_garage_stage`).
    The panel's box moved into `pause_garage_picture`/`pause_garage_inside`, same numbers as before.
  - On a flier tile the panel skips the baked still, since the shell draws the model there.
    Bike, quad and car tiles keep their stills.
- **garage_view.rs**: `PauseFlier`, a second `Stage` dressing the skin off-thread just like the
  Garage's Fliers tab does.
  - Software path: draws the stage's own dress (`Stage::draw_soft`).
  - GPU path: `Stage::draw_gpu` calls `forget_assets`, which would wipe the ride. So instead
    `take_into` copies the dress's model and textures into the ride's `Assets` (`take_in`, under
    `pause-flier:`), uploads that one model, and draws it with `render_over` off the ride's assets.
    On a new skin, or once the page closes, the old copy is let go (`let_go`, `drop_owner`).
  - `garage_view::pause_flier` picks the first paused view whose page has a flier, the same way
    `trick_view::shown` does.
- **main.rs** (the ride loop): show, take, pose before the frame; draw inside the Tricks page's
  closure (GPU) or after its software draw. It turns at the Garage's speed, timed by the clock
  rather than by frames. A shot stands it at turn 0.
- **Shot plumbing** (so the check can be taken):
  - `--screen pause-garage` now starts a ride under the page, as `pause` and `tricks` already do.
  - `--menu-row NAME` puts the cursor on that tile once the ride's tiles are known (`point_at_row`
    reads the pause Garage's rows).
- opentennis: not looked at. It has no 3D machine preview over a paused game, and the Garage stage
  already here is the thing to reuse.

## What the original does

Nothing to port. MX Unleashed's pause has no garage and no model preview. This is front-end work.

## How it was proved

- **Shots** (private headless weston, 1280x720, debug build, free06 from the default settings):
  - `--screen pause-garage --menu-row Biplane --shot … --shot-frame 2 --seconds 150`: the red
    biplane stands in the panel over the paused ride (GPU, llvmpipe).
  - The same with `--menu-row Helicopter`: the blue Kiowa.
  - The same with `--renderer software`: the biplane, drawn the same way.
- `garage_view::tests::the_pause_flier_takes_turns_in_the_rides_assets` (runs on the disc; it
  printed no skip):
  - The disc's two planes go into a ride's `Assets` one after the other, after the ride's own
    texture.
  - The pose names models that are in the ride's assets.
  - The first skin's textures are blanked when the second shows.
  - Nothing is left once the page closes, and the ride's own texture is untouched.
- `./kit check -- -p openmx --test menu`: 299 passed. `garage_view` unit tests: 3 passed.

## Not verified / not 1:1

- **Turning not watched.** Every shot stands at turn 0. The turn comes from a clock, and I didn't
  watch it spin in a live window.
- **A skin change was not shot.** L1/R1 on the tile re-dresses the stage. The unit test covers the
  swap in the assets, but no GPU frame was taken after a swap, so `drop_owner` followed by a
  re-upload under the same name was never seen drawn.
- **A pack skin was not shot.** Only the disc's MX Unleashed fliers were. A pack skin dresses through
  the same `dress_flier` as the Garage's Fliers tab (its `_ui` model), but it wasn't drawn over a ride.
- **Split screen: one flier only.** If two views have the pause Garage open on flier tiles at once,
  only the first view's flier turns. The other view's panel stays empty: the still is hidden and
  nothing is drawn there. Not tried.
- **Empty panel while dressing.** For the moment it takes to dress the skin, the panel shows nothing,
  because the still is hidden on flier tiles.
- **Online and phones not run.** The draw is in the shared `main.rs` (web and Android build it too),
  but only the desktop was run.
- **Release build not run.** The shots were taken with the debug build.

## Next

Nothing for P326.
