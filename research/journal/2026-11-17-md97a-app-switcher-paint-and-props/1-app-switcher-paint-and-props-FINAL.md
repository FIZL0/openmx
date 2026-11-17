# 1 — app switcher paint and props (FINAL)

Task MD97a. 2026-11-17.

## What I did

MD97 rebuilt the ground after the iPhone app was away. This does the same for the paint and the props.

- **Paint (`paint_stream.rs`).** New `PaintStream::write_again`. Every map shown, or half written, is read
  again from its folder. It is then written over its own room in its page array. A new state, `Again`,
  means "shown, and being read again". The old pages stay drawn till the new layers land, as
  `build_again` keeps the old ground.
  - Out of reach while read again: the map is let go at once and goes to `Asked`. Its read is let go as
    it comes, so it is never asked twice.
  - A read that fails the second time leaves the map shown as it was (log: `not written again`).
  - The background give-back (P307) turns an `Again` map into `Asked` too.
- **Props (`main.rs`, the ride loop).** On the same "back in front" moment, every dressed tile's models and
  textures are let go (`drop_owner`) and queued again. They go up a few a frame on the existing MD28 path:
  levels on a thread, then models, then `add_props`. The tile's props and cars are not drawn till theirs
  are up.
- **Dress uploads wait while away.** The per-frame model and texture uploads now skip while
  `gpuwin::in_front()` is false. The dress stream's `update` and `world_dress::take` still run, so the
  collision gets the same tiles at the same time as before. Only the GPU part waits.
- The "back in front" block no longer needs a ground stream to run. `ground_away` is now updated whenever
  the app comes back, not only when there is a ground stream. Log lines: `world paint: back in front at
  tick N, M maps written again`, `world props: back in front at tick N, M tiles put up again`.
- The mid-ride dress upload start became a small helper, `dress_uploads`, used by both the new tile path
  and the redo. The `Levels` type moved beside it.
- The brief named `worlds.rs`, `world_dress.rs` and `gpuwin.rs`. None needed a change: `build_again`
  and `in_front` are already there, and the ride loop that calls them is in `main.rs`.
- opentennis: nothing to take (MD97 found it has no background GPU handling).

## What the original does

Nothing to port: this is the app's GPU streaming, not the game.

## How it was proved

- New GPU unit test `paint_stream::tests::the_maps_shown_are_written_again_after_the_app_was_away`
  (crates/openmx bin). On a headless renderer, a two-map world is shown from the load. Then one map's page
  is overwritten black, standing in for a refused write. The test checks:
  - `write_again` asks for both maps once. Both stay drawn while asked.
  - With the reads answered, both are written in one frame. The frame is byte-identical to before the
    spoil.
  - The stream then rests: no asks over three more frames.
  - Read again while out of reach: let go, then `Far` once the read comes, no layers left taken. Back
    in reach, each map is asked exactly once.
  - It ran on the GPU here (no "skipped" line).

## Not verified / not 1:1

- **Not run on the iPhone.** No device here. Same as MD97: that iOS refuses these uploads is inferred, not
  logged.
- **The props redo has no test.** It is plain shell-loop code: `drop_owner`, then the existing MD28 upload
  queue. A test would need the whole ride loop. It was only built, not driven. That `drop_owner` followed
  by an upload puts a model back is the renderer's existing behaviour (the same as a tile let go and dressed
  again mid-ride).
- **The far pages and the leaf image are not rewritten.** The paint redo rewrites only a map's own pages
  and (through `world_paint_show`) its leaves. If the far pages written at the load were refused, they stay
  wrong. Fixing that needs a new `Renderer` call in game-render, outside this task's files. The load waits
  for the front before writing them (MD97), so only a write in the moment the app leaves can be hit.
- **The props blink out after the app comes back.** A tile's props and cars go undrawn from the redo till
  their models are up again: a few frames per tile, tiles one after another. The ground and the paint keep
  their old copy drawn instead. Keeping the old props would need the renderer to replace models in place.
  I judged a short pop-in after the app switcher fine.
- **The load's other uploads (scene, sky, riders, stand-in) are still not redone.** Not asked for here
  (MD97's note).
