# 1 — deck p2 join crash (FINAL)

Task P271. 2026-11-03.

## Getting the crash

The brief asked for the Deck's log or `coredumpctl` backtrace first. This was an unattended run with
nobody to ask, and I was not to ssh into the Deck, so I went straight to a desktop reproduction.

Driver: `pads.py` (this folder). It lays out `OPENMX_PAD_ROOT` (P171's stand-in pads: `dev/jsN`
fifos and `sys/jsN` entries) with pad 0 plugged in, starts the shell, then runs a script: `wait S`,
`plug N`, `press N cross|start|…`, `axis N A V`, `end`. The window goes to a private headless weston
(`weston --backend=headless --shell=kiosk --socket=p271-wl`, `WAYLAND_DISPLAY=p271-wl`). On the
desktop's own `:0` the window was throttled to 1 fps, and a 120 ms press fell between two frames.

1. Release build, `--screen tracks`, Cross on pad 0 rides free06, `--sim-thread on`. Pad 1 plugged
   in at 23 s, then Cross: `players: player 2 joined …; views: 2 players, side by side`, then at
   once `panicked at crates/openmx/src/main.rs:932:19: index out of bounds: the len is 1 but the
   index is 1`. That line is `RideHud::with`'s `let me = &self.ridden[v];`.
2. `--track free06` alone has no front end, so seats are never open and Cross on a free pad does
   nothing. A drop-in needs the front end (`--menu --screen pause` or `--screen tracks`).

## Why

The loop's tick has two halves. The first half (pads, menu, the join) runs on the loop. Under N23a
the simulation then goes to the worker while a picture is drawn, and the rest of the tick (cameras,
`ridden`, the bodies' frames) runs at the top of the next pass. The join reseats `world.seats`, the
rigs and the rects in the first half. The tick is sent off, and the picture drawn beside it reads
`ridden` and `cameras` from the tick before, which have one entry. Two rects mean `ridden[1]`.

`--sim-thread auto` sends a tick off when `world.seats.len() >= 2 && gauge.wants()`. The join tick
has two seats, so on hardware where a tick plus a picture overruns the frame time (the Deck at its
refresh rate) auto does exactly what `on` does. On this desktop the gauge never wanted it (no
"sim thread on" line under `--timings`), which is why it showed up on the Deck.

## The fix

- `main.rs`: a `reseated` flag, set when a local join or leave is applied. A reseated tick is not
  sent to the sim thread (`&& !reseated` beside `landing.is_none()`). It runs on the loop, its
  second half lays out the new count's `ridden` and cameras before the picture, and the next tick
  goes off as usual with every list already the new length.
- `main.rs`: building the joiner is fallible now. The lot is parked for the new count
  (`park_lot`, P87's two lines made a function), then the seat and rig are built in a closure whose
  `?`s (`disc.res` for the rider and the bike, `Garage::new`) come back as `Err`. Before, those `?`s
  returned from `ride()` and ended the ride. On an error the lot is parked back, the shell prints
  `players: player N could not join: …`, the pause note says `Player N could not join: …`, and
  nothing is reseated (`change` filtered out).
- `main.rs` + `split.rs`: `SMALLEST_FRAME = 8`. A window's frame is never smaller, so the eighths'
  views are 2 px or more. Found on the headless repro with no display: the window reported 0x0,
  the frame was 1x1, and the join split it into `0x1` views. That panicked in
  `raster::Setup::of`'s `clamp(0.0, w - 1.0)`, and with that guarded, in `sky.rs`'s
  `chunks(0)`. A minimised window may report the same size, so this is not only a test issue.
- `game-render/src/raster.rs`: `Setup::of` returns `None` for a zero-sized target, since it covers
  nothing.

Nothing taken from opentennis: its seats never ran a sim thread.

## How it was checked

- `tests/drop_in.rs` (new) runs the shell headless from the front end (`disc.iso --headless --menu
  --screen pause`) with two fifo pads. Pad 1 rides. In the riding cases its Start closes the pause
  first, and the test waits for a second line that is not `(frozen)`. After a second of ride, pad 2
  is plugged in (`2P: Pad 1`) and presses Cross. The test asserts `players: player 2 joined … views:
  2 players`, two more seconds of fps lines ending `, 2 views`, and the shell still up with no
  `panicked`. Four cases: riding and over the pause, each with `--sim-thread on` and `off`.
  `./kit check -- -p openmx --test drop_in`: 4 passed (each case's `target/tmp/p271-*` folder made,
  so none skipped). With the `!reseated` guard taken out, both `on` cases fail with the shell's
  `main.rs:939:19` index panic and both `off` cases pass.
- Hand runs, release build, all ending with the split up and no panic:
  - free06, `--screen tracks`, sim thread on, GPU window 640x360: joined, 12 s of 2 views.
  - free06 with `--tricks mxvatv_tricks`, `--sim-thread auto --timings`, 1280x800: joined, both
    players riding (513 and 375 ticks).
  - `world_mxu_free` (the disc world, joins built fresh, 5 tiles) with `--tricks mxvatv_tricks`,
    sim thread on, 1280x800: joined mid-ride, 12 s of 2 views.
  - Headless, no display (the 8x8 frame): joined over the pause and riding, sim thread on.
  - Start on pad 2 instead of Cross: it joins too, and its later Starts pause and unpause its own
    view.
- `./kit check -- -p openmx --test menu --test join --test drop_in --test loading`: join 5, loading
  50 and drop_in 4 pass. menu fails only `original_on_sets_every_sub_switch_and_one_turned_back_leaves_the_rest`
  ("Whole Map" vs "Farther Fog"). That failure predates this task: P264's commit records it failing
  the same on a clean HEAD (P268's fog change, P269).

## Not verified / not 1:1

- The Deck's own log and backtrace: not asked for (unattended run), and the Deck was not reached.
  The desktop repro gives the same symptom (a crash the moment player 2 joins), and the cause fits
  the Deck's default `--sim-thread auto` only if its gauge wanted the thread. That is inferred from
  the Deck's frame budget, not seen on a Deck.
- `--sim-thread auto` engaging: it never engaged on this desktop, so auto was only run on its
  non-threaded path. `on` is the threaded path auto takes.
- Real pads: stand-in fifo pads only. Hot-plug of a real USB or Bluetooth pad, and the Deck's
  built-in controls plus an external pad (Steam Input's virtual pads), were not run.
- The joiner-could-not-be-built path: no disc read failed here, so the note was never shown. It is
  checked only by reading the code.
- Online seats (N20 `sit`) were not touched or run. Online never uses the sim thread.
- Joins past two (three or four players) and a leave were not run live. A leave sets `reseated` the
  same way.
- A minimised GPU window was not tried. The frame floor was proved only on the headless 0x0 window.
