# 1 — whole frame closed (FINAL)

Task F31b5. 2026-11-17. Follows `../2026-11-14-f31b5-whole-frame-in-rust/2-ports-and-what-is-left-READY.md`.

## What I did

- Checked the follow-ups the last entry named: F31b5q, r, s, t, u, v and F67 are all ticked on main.
  Their switches (`batch send`, `heap`, `wreck check`, `wreck hand-outs`, `wreck sound`, `wreck post`,
  `wreck camera`, `wreck listeners dropped`) were already in `replay_in_pure_rust`'s list.
- Ran `ride_replays_in_pure_rust` and `flip_replays_in_pure_rust` with `--include-ignored`: both pass.
- Took the `#[ignore]` off `ride_replays_in_pure_rust` (flip's was already gone), so the gate runs both.
- Fixed the stale doc line on `replay_in_pure_rust`: the wreck posts are no longer the original's (F67).
- No code under `src/` changed.

## What the original does

Nothing new this session. The frame's parts are in the earlier F31b5 entries and its follow-ups'.

## How it was proved

- Bit-exact replay, `crates/game-sim/tests/mxvatv_bike_ride.rs`:
  - `ride_replays_in_pure_rust`: C15 ride-2, all 459 rows match, no wreck, interpreter total 0.
  - `flip_replays_in_pure_rust`: C15 flip-1, all 465 rows match, wreck state changes on rows 125 and
    193 only, interpreter total 0.
  - Both also hold the read watch (the frame reads no byte where its RAM parts from the handing run's)
    and the bike and its blocks equal to the handing run's (inside `replay_pure`).
- Not a skip: a temporary `assert_eq!(total, 999)` in both made them fail with `left: 0`, after the
  log line "the whole frame in Rust, the interpreter ran 0 instructions" for each ride. Put back.
  The heap's port served new/delete/realloc 7/7/9 (ride-2) and 23/21/16 (flip-1).
  Run time for the two: 63 s.
- The other five riders' PreProcess: `the_other_riders_pre_process_leave_the_bike_alone` (F31b5v's
  journal, `../2026-11-17-f31b5v-other-riders-leave-the-bike/`).
- Original rides keep every replay hash: the gate (`./kit check --all` inside `./kit done`).

## Not verified / not 1:1

- **The frame runs over a RAM image held by `Ee`, not Rust structures.** No MIPS instruction runs,
  but the ports read and write the original's memory layout. The seed (`handling::seed`) and the
  terrain (`handling::ground` on Nat05's .trn) are the ports' own; the rest of the world (items,
  emitters, heap, spatial database) is state 4's RAM.
- **Only C15 ride-2 and flip-1.** Other tracks, bikes and rider counts are not replayed.
- Things the follow-ups left that these two rides cannot see (each written in its own journal):
  - the noise build leaves the 2-D/3-D gradient tables as the state had them (F31b5, 11-14);
  - dropped rumble: pad output is compared nowhere (11-14, F31b5t);
  - dropped listeners (other listeners, part-walk models, wreck listeners but the camera controller)
    are shown harmless only by the read watch on these two rides (F31b5m, F31b5j, F67);
  - unported paths that never run here: the stance with id ≥ 0, `Kind(false)`, trail items with no
    `+4`, multi-block stop loops (F31b5t); sound play paths other than the wreck cue's (F31b5u).
- The five riders' PreProcess check watches only the bike's words (F31b5v).

## Next

Nothing; `./kit done` closes F31b5.
