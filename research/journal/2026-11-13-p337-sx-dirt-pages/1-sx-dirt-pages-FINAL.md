# 1 — sx dirt pages (FINAL)

Task P337. 2026-11-13.

## What I did

- **Which platform, which change.** None of the platforms: the release (`cargo build --release -p openmx
  --features ship,native-sim`, tools/build.sh's features) and the dev build drew sx01 the same, one flat
  dirt colour (shots compared side by side). No change since v0.12.0 dropped a texture: the shell's
  ground paint was `Splat::load`, else the terrain's pages **only for a mod track** (MD4's
  `match &modded { Some(_) if ground.paint.is_none() => … }`), identical in v0.12.0's main.rs. The
  disc's supercross tracks have no `terrainscreenspacemesh.psm` (no splat), so a lone disc sx track
  always fell to `GroundPaint::Bare` (`ground::DIRT`). **So did every national**: nat01 has no splat
  either (only the freeride tracks have one), and the pre-fix release build drew Seneca Butte as flat
  tan; it now prints `ground: 32 terrain pages on 6400 of 6400 leaves, with a detail map` and draws
  its own green/brown ground, the paint the MX World already gave that tile. What changed in v0.13.0 is that Race (P313)
  rides those stadiums alone; before, a player met them only inside the MX World, whose tiles try the
  pages first (`worlds::map_paint`, MD9) and so did draw sx dirt. That is the "dropped since v0.12.0"
  the user saw.
- **Fix.** `game_render::gpu::track_paint` (new, beside `GroundPaint`): a lone track's splat, else its
  `.trn` pages under its detail map (`Pages::load_detail` → sx01_d.pst, `detail_multiplier` from the
  scene), else bare — for every track, disc or mod. `TrackPaint::paint()` gives the `GroundPaint`.
  openmx's `Ground::dressed` calls it (replacing `painted` + `paged`, which only main.rs used), and
  main.rs's load calls `dressed` and keeps the `ground: N terrain pages …` line. sx01 now prints
  `ground: 16 terrain pages on 256 of 256 leaves, with a detail map`.
- Nothing taken from opentennis (it has no terrain pages).

## What the original does

The original draws a supercross floor from the `.trn`'s paletted pages with `<track>_d.pst` blended over
them (MD4's reading, the same as MX vs. ATV's maps). No new reverse engineering: this task only routes
the disc's sx tracks through the reader MD4 already has.

## How it was proved

- Looked right beside a screenshot: PCSX2 copy 3 on `context/states/c31-sx01-race-start.p2s` (shot via
  AT-SPI, research/c28/shot.py; the copy must start with `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
  QT_ACCESSIBILITY=1`) shows grainy brown dirt with dark ruts; the port's `--track sx01 --headless
  --frames 40 --shot` before the fix was one flat tan, after it the same grainy rutted dirt.
- Test `crates/game-render/tests/dirt_textures.rs` (disc; skips without it):
  `every_supercross_track_paints_its_dirt_from_its_pages` — every `sxNN.res` on the disc gets pages on
  every leaf and a detail map from `track_paint` (24 tracks, sx01–sx24), nat01 too, and free09 keeps
  its splat;
  `dirt_textures_load` — a headless GPU frame of sx01's pages over a flat chunk is textured (pixel
  spread above 4) where the bare paint is one colour (below 1).

## Not verified / not 1:1

- The fixed dirt was compared with the original **by eye** only (one view at the sx01 start). The port's
  dirt reads a little darker and greyer than the original's; rendering is matched by eye by policy
  (AGENT.md), so not chased.
- Nationals: nat01 was looked at in the port only (before: flat tan, after: textured ground), not
  beside the original; the change follows the world's own tile paint (MD9), which already drew it.
  Other nationals not shot.
- Supercross tracks other than sx01 were not looked at on screen; the test proves only that each loads
  its pages on every leaf and a detail map.
- The user's report did not name a platform. Only the Linux desktop release build was run; web, iOS and
  Android share main.rs's load (web mounts it, android_so.rs is a link to it), so they take the same fix,
  but none was run.
- Software renderer: it paints `Ground::pages` through `Pages::paint` (MD4) and so should draw the sx
  dirt too; not shot.

## Next

None for P337. Music, bots, gate drop and the hit event from the same report are P339, P313/S-tasks and P338.
