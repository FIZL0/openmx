# 1 — split screen toy (FINAL)

Task V105. 2026-11-04.

## What I did

- `crates/game-render/src/disc.rs`: `Res::with_member(name, bytes)` returns the same archive with one
  member's bytes swapped for others. The member keeps its place and name, and gets its new size; every
  other member is read from the archive as before. Unit test
  `a_swapped_member_reads_its_new_bytes_and_the_rest_their_own`.
- `crates/openmx/src/main.rs`: `split_screen_toy(disc, track, stem)` swaps the track archive's
  `<stem>.toy` for `<stem>_ss.toy` out of `<stem>_ss.res`. A track with no twin on the disc comes back
  unchanged. `ride` calls it when the load-time layout (`Layout::for_players(players, split)`) is not
  Full, the ride is offline, the track is the disc's (not a mod), and the ride is not a disc world. Every `.toy` reader goes
  through the track archive, so all of them get the twin's file without changes of their own: the
  traffic, the animated toys, the props, the emitters, the triggers, the weather, the sounds, the creatures
  and the flares.
- Nothing was taken from opentennis: it has no per-layout track files.

## What the disc has

I compared each `.toy` with its `_ss` twin, CR stripped:

- free06, free07 and free10: identical, byte for byte. The brief's "free10_ss differs from free10" is not
  true of the `.toy`. The V76 note "one free10_ss toy" writing False is SH_windsock, which writes False in
  both files.
- free09: only the five `[AnimatedToyN]` `AnimateWhenNotVisible` lines differ, False in the base file and
  True in the twin.
- free11: Car1–Car3 `TimeScale` 0.3/0.3/0.3 in the base file, 0.5/0.5/0.2 in the twin. That is
  simulation state: the cars' clocks.
- The supercross and national tracks have no `_ss` archive.

## What the original does

`research/oracle.txt`, "What two views cut back": `world_load` (`./kit fn 0x00205c40`), with two views
(mode != 0) in a free area (league 2), loads the track as `"%s_SS"`, which is `Tracks\freeNN_ss.res` plus
`_ss_2.res`. So in that case its `.toy` is the twin's.

## How it was proved

- Disc test `split_screen_toy::two_views_read_the_twins_toy` (openmx bin), run with
  `./kit check -- -p openmx --bin openmx split_screen_toy`, 1 test passed:
  - free09 through `Animated::load`: unseen `[false; 5]` alone, `[true; 5]` split;
  - free11's first three cars through `game_render::toy::cars`: 0.29999998 ×3 alone, and 0.5, 0.5,
    0.19999999 split;
  - sx01, which has no twin: its own file, unchanged.

  Its first run went red on the free11 values (I had written 0.3, and the reader truncates to
  0.29999998), which shows it runs on the disc and is not skipped.
- `./kit check -- -p game-render --lib disc::`: 4 passed.
- Release smoke run, `openmx --track free09 --players N --headless --frames 30`:
  - with 1 player, no swap;
  - with 2 players, it prints `split screen: tracks\free09\free09.toy read from Tracks\free09_ss.res`,
    5 toys on clips and side-by-side views.

## Not verified / not 1:1

- **Only the `.toy` is the twin's.** The original's split screen loads the whole `_ss` track. In free06_ss
  that means no rut meshes or rut texture, no crow, its own `.trn` and a smaller `.scn` (fewer props). The
  port still rides the base track's terrain, scene, props and `_2.res` everywhere else. That is the larger
  gap, and I did not file it again here (V76/R7 notes already record it).
- **Which views decide it.** The original's test is "two views in a free area". The port uses the
  load-time layout. A seat that joins mid-ride (P76) keeps the file the ride loaded, and so does a split
  screen that drops to one view. The original reloads nothing either, but this was not checked against it.
- **Online** rides (`--online` or the lobby) always read the base file, even with two local views.
  The cars and toys are the session's, rolled back alike on every machine, so no single machine's layout
  may pick their file. A two-view machine online therefore runs free11's cars at the base file's speeds,
  where the original's split screen would run them at the twin's.
- **No live capture** of a two-view free09/free11 ride in PCSX2 was taken. The claim that the original reads the twin's
  `.toy` there rests on the `%s_SS` load in `research/oracle.txt`, not on a RAM read of the loaded toys.

## Next

Done.
