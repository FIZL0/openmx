# 3 — wip landed (PART)

Task MD123. 2026-11-11.

## What I did

- Session 2 (file 2) did its work but ended before `./kit done`, so the runner saved it as `wip/MD123`. That
  snapshot was taken from an older main, so its copies of `game-render/src/kiowa.rs`, `openmx/src/*`,
  `openmx/tests/mxvatv_fliers.rs` and `tests/mxvatv_bike_ride.rs` would have undone MD109's later
  changes. I took only MD123's own files: `quad.rs`, `tests/atv_quad.rs`, `atv_quad/{frame,mxu,oasis}.rs`
  and this journal. Main hasn't changed any of them since the wip's base, so they apply as they are.
- The wip hand-edited `plan/TODO.md` to add MD123a. I filed it with `./kit plan add MD123` instead, using
  the same text.
- MD129 (the quad's model-side load inputs, "before MD123") is still open. So a quad made only from its
  files, which the task's test asks for, still can't be built. MD123 stays blocked `--after MD129,MD123a`.

## How it was proved

- `./kit check -- -p game-sim --test atv_quad`: 106 tests passed.
- `mxu::the_spawned_quad_rides_free09_headless` run alone with `--nocapture`, to rule out a silent skip:
  it printed `493.41888 ft gone, top 106.229095 ft/s, all four down 266 of 360 frames, lowest 0.4826355 ft
  over the ground, turned 2.0993645 rad on the stick; handed on {"GravityRecord", "PartMove"}`. Those are
  the numbers file 2 recorded.
- The Oasis chain (`frame::`) passes through `quad::Ride`.

## Not verified / not 1:1

- Everything in file 2's list still stands: no quad from its files (MD129), no capture can prove the
  free09 ride, MXU surface numbers not remapped, `OnWorld`'s contacts not the original's, the calls
  dropped in `rest`, no tail, record, wreck or put-back yet (MD123a), the `Bike` clone a frame not timed,
  and the bounds' floor kept from the image.

## Next

MD123a, then MD123 itself once MD129 lands (see file 2's Next).
