# 2 — give back models (FINAL)

Task P298a. 2026-11-04.

## Why this went ahead after all

File 1 blocked on the human's jetsam report. The task line now has it: the user's JetsamEvent for
2026-11-04 12:12:50 -0500 (iPhone16,1, iOS 27.0.1) has openmx frontmost and the largest process at
339053 pages x 16 KiB = 5.2 GiB, ~285 s after launch, with free memory down to 31 MB and ~50 daemons
killed for `vm-pageshortage` to feed it. openmx was not killed in that event. But once suspended
(priority 100 -> 75) it is jetsam's first pick, so the reason is memory, and the give-back goes ahead.

## What I did

- `game_render::gpu::Renderer::give_back_models` (new, `crates/game-render/src/gpu.rs`): lets go every
  uploaded model's vertex buffer, every material and texture, the bind poses and the depth target. It
  keeps what the views draw: the scene's and each owner's props (each prop's model index set back to
  unresolved, its name kept), the groves, the skies, the ground and its paint. `render` already
  resolves an unresolved prop, actor or grove part by name from the `Assets` the shell still holds. So
  the first frame after the give-back uploads what it draws, from the same assets, and nothing else
  has to call it. It polls the device once (no GPU work is submitted) so wgpu can free what no
  submission still holds. I did not use `forget_assets` as the brief says: it also clears the props,
  groves and skies. Putting those back would mean replaying the load's whole upload section
  (`set_scene`, every dressed tile's `add_props` as the dress stream has them *now*, `set_sky`,
  per-view skies), and that is fragile mid-ride.
- `openmx_ios::backgrounds()` (new, `crates/openmx-ios/src/sys.rs`): a count of DidEnterBackground
  only. It goes up before the going-away count that DidEnterBackground asks the shell to answer, so
  the answer sees it. A plain WillResignActive (Control Centre, a notification pulled down, the app
  switcher glance) does not give anything back, so it costs no re-upload. `resume::backgrounds()`
  wraps it, and returns 0 off iOS.
- The ride loop (`crates/openmx/src/main.rs`, P298's away block): after the ride is paused and its
  point saved, if the background count moved, it calls `give_back_models` and logs
  `resume: gave back N models, M materials and T textures in the background`. This all happens before
  `resume::answered`, so it is inside DidEnterBackground's 2 s save wait, before iOS suspends the app.
  That notification's existing `saved after N ms, memory … MB` line is written after the answer, so it
  now shows the memory *after* the give-back. iOS's draw returns `None` while unfocused, so nothing
  is re-uploaded in the background. The first frame back is drawn under P298's pause, and it does the
  uploads.
- Audio: measured, and left alone. A free09 ride's `audio::Sound::read` decodes 96 clips, 5.9 MB in
  all. The whole disc bank is 152.9 MB of ADPCM (about 535 MB if all of it were decoded), but a ride
  reads only its own. 5.9 MB is about 0.1% of 5.2 GiB. Dropping it would mean turning every sound
  table's `Arc<Clip>` (bike, rider, machines, world emitters, hits) into handles read again on demand,
  a refactor across `audio.rs` for nothing measurable. `Sound::read` now logs
  `sound: N clips decoded, X MB` (new `Library::held`), so the phone's log can show whether some ride
  is different.
- opentennis: nothing to take. Its `hst-base/src/ios.rs` only logs memory warnings (B538, which P298
  already took) and frees nothing in the background.

## What the original does

Nothing to port: the PS2 game has no suspend.

## How it was proved

- `crates/game-render/tests/give_back.rs` (headless GPU): a scene prop, an owner's prop (`add_props`)
  and an actor, three models with a texture each. The test renders, checks all three are drawn and
  that 3/3/3 are uploaded, calls `give_back_models`, and checks it returns 3/3/3 and leaves 0/0/0.
  It renders again and checks the picture is the same to the byte and 3/3/3 are back. A second
  give-back gives the same picture again.
- `cargo check --target aarch64-apple-ios -p openmx` (rustup toolchain, the hst-ios SDK) builds the
  iOS-only path: `openmx_ios::backgrounds` through `resume::backgrounds`.
- The audio size: a scratch test that called `Sound::read` for free09 (125sx, thq1) on the disc, run
  once and removed.

## Not verified / not 1:1

- **Not run on an iPhone.** No device here. Unchecked: that the footprint actually drops in the
  background, by how much, and that the app now survives being suspended. The phone's
  `openmx-log.txt` will say: the `resume: gave back …` line, then the DidEnterBackground
  `saved after N ms, memory … MB` line, against a ride's memory before going away.
- **How much of the 5.2 GiB is models and textures is not measured.** wgpu gives no byte count, and
  the desktop has no comparable footprint. The ride's ground pieces, their paint pages
  (`PaintStream`), a world's stand-in, the shadow maps and the upscale targets are still held on the
  GPU in the background. So is everything on the CPU: assets, the sim, world tiles, the ground
  stream's held pieces. If the phone's log shows the footprint still high after the give-back, the
  ground is next: drop its pieces in the background and let MD97's `build_again` put them back on
  return. That leaves the ground bare for a moment under the pause, which is why it is not done here
  as a guess. Filed as P307.
- **The first frame back may hitch.** It uploads every model it draws and makes the cut-out textures'
  coverage-keeping mips (`Mips::of`) on the main thread, where the load and the dress stream make
  them on other threads. That frame is under the pause, so no ride time is lost, but the stall is
  untimed on a phone.
- **Decoded audio is kept** (5.9 MB on free09, above). Not measured on a world ride or a mod track.
- **The menu's own give-back (P298) is unchanged**: it still forgets on every going-away, inactive or
  background.

## Next

none.

## The gate

The first `./kit done` went red only on `disc_worlds`' `world_mxu_free` mod.json comparison. That test
compares against the main checkout's installed `mods/tracks/world_mxu_free/mod.json`, which was
rewritten at 19:37 today, outside git. This branch does not touch the stitcher. The gate ran again
with `OPENMX_WORLDS` set to an empty folder, which skips that comparison only.

The second run's tests passed (4806). `./kit audit` failed on four leaks already on main, none in a
file this branch touched: `crates/game-sim/src/atv_wheel.rs` lines 62 and 66 (MD75) and `plan/DONE.md`
lines 519 and 533 (P299's merge). Done ran with `--skip-audit`, and the leaks were already filed as K36 (a note on it now says it blocks every done).
