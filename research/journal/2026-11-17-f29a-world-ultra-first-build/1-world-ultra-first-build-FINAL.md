# 1 — world_ultra's first ground build

Task F29a. 2026-11-17.

## What I did

- **The ground's first build runs on 8 threads at most (`worlds.rs`, `FIRST_BUILDERS`).** It was one
  builder per core (32 here). Each builder holds a piece's whole chunk set until it is handed over (a
  coarse piece's before it is decimated), so 32 in flight put world_ultra's peak ~250 MB higher.
  8 is the Deck's thread count, so the Deck loads as before.
- Building coarse pieces without the full chunk set first (the brief's other option) was not taken:
  `coarse` reads cells across neighbouring chunks, so it would need a second chunk builder that must
  give the same chunks as decimating. The cap gets most of the gain with a one-line change.
- The in-ride builder was one thread already; it is unchanged.

opentennis has no streamed ground to take from.

## How it was measured

Release build, `--headless --mods ~/repos/openmx/mods --track world_ultra --timings --size 640x360
--frames 30`, with a temporary `F29A=<n>` switch for the cap (since removed), runs alternated.

Load 7–9 (the quietest window in ~45 min; it never went under 6):

| world_ultra, 32 cores | 32 builders (before) | 8 builders (F29a) |
|---|---|---|
| VmHWM at `load:` | 1676, 1716, 1619 MB | 1350, 1424, 1445 MB |
| ground's first build | 1.72, 1.72, 1.65 s | 2.36, 2.23, 2.08 s |
| first frame after launch | 6.08, 5.99, 5.70 s | 6.66, 6.32, 5.94 s |
| CPU to the first frame | 25.1–26.0 s | 20.0–22.2 s |

Other caps, at load 15–21: 4 builders 1220–1323 MB but the build 3.3–4.3 s; 12 and 16 builders
1508–1540 MB. 8 is the knee: most of the memory for ~0.5 s of build.

## How it was proved

Not a simulation change: the pieces built are the same (each from its own folder, handed over in the
same order), only fewer at once.

- `./kit check -- -p openmx --test world_ultra_load --test world_streaming --test world_first_frame
  --test wrapped_world --test world_look`: 22 tests passed.
- `cargo test -p openmx --test world_ultra_load -- --nocapture`: both world_ultra tests ran, none
  skipped; peak 1389 MB.
- The full gate in `./kit done`.

## Not verified / not 1:1

- **A truly quiet window.** The timings are at load 7–9, not under 6: none came in ~45 minutes. The
  three pairs agree with each other and with the pairs at load 15–21 (+0.4–0.6 s of build).
- **The Deck.** Not measured (no `ssh deck` in runs). With 8 threads it took 8 builders before too,
  so nothing should change there.
- **`world_ultra_load.rs`'s peak bound** stays 2000 MB: that file is not on this task's line. It
  could come down to ~1700 MB.
- **Pictures.** Not compared: the pieces and their order are unchanged, only how many build at once.

## Next

none
