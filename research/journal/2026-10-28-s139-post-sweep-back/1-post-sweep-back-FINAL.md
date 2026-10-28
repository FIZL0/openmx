# 1 — post sweep back (FINAL)

Task S139. 2026-10-28.

## What I did

- Found the port already done: S125 (599ebbf1) ported collision_record_sweep_back for the riding chassis
  as `contact_search::sweep_back_chassis` / `Ride::sweep_back`, run by the rider's tick before the ground
  and scenery contacts. S125's journal said no capture compared it with the original, so S139 is that proof.
- `research/s139/sweep.toml`: S138's whole-bike watch plus the record's swept segment (bike +0x7d0 →
  +0xd0 from, +0xe0 to), the sweep probe's flags (bike +0x7cc → +0x116) and its contact (push, point).
  `research/s138_hits.py` takes `--watch`.
- `research/s139_shove.py` + `research/s139/shove.sh`: load recording2's state, teleport, pause over AT-SPI,
  write a world velocity into the chassis, save, and record from that state. `research/s139/post.sh`:
  S138's wall.sh with the new watch.
- Captures (context/capture/s138, s2 only): post, shove200, shove400, fly300 (head on, never swept),
  side1 and sidepost (thrown west at -200, clamped to 135: the sweep strikes nine and six ticks).
- Test `the_sweep_back_puts_a_fast_bike_back_out_of_a_post_as_the_original_does` (game-sim
  tests/replay.rs): side1, sidepost and post ridden by the port from the original's state with its pad.

## What the original does

See the S139 block in research/oracle.txt (`./kit fn 0x00270390`). The sweep runs from where the node's
offset point (0, 3, 0) was, velocity × 2 × dt back, to where it is; a struck probe whose push is not
facing the motion (0.7074) puts the node at point + push × 2 − offset. The body rays reach only ±1.3 ft
sideways but −3.8..3.6 ft along the bike, and the riding speed cap is 135 ft/s (4.5 ft a tick), so the
sweep only matters sideways: head on, a ray always meets a post first.

## How it was proved

Save-state captures over PINE (research/s139/shove.sh, post.sh), three runs, compared tick by tick in
the test above (31 + 16 + 10 ticks):
- every one of the original's 15 struck put-backs (sw_to) is met by the port's sweep of the same or
  previous tick within 0.05 ft per lane (they match to print precision);
- the port's chassis keeps to the original's cofg within 0.05 ft while both ride;
- the first wreck is the original's reason within a tick (side1 11 at 2484 both; sidepost 6 at 2480
  against 2481; post 1 at 2571 both);
- with the rider's sweep_back call disabled the test fails: side1's bike tunnels through the wall
  (4668 against 4670.6 on the first tick, 28 ft off by tick 2478) and sidepost wrecks reason 2.

## Not verified / not 1:1

- sidepost wrecks reason 6 (the head ray) one tick early in the port (row 2480 against 2481), with the
  path and every put-back equal before it. Not found: which pose the original's body rays search from
  on a swept tick (its position set only marks the node dirty, then 0x00119be0 runs on the rays). Filed
  as S202; the test allows the tick.
- The capture's sweep lanes are read at the top of a tick, so some rows hold the next tick's values
  (sidepost 2474..2476, post 2567). The port's tick-2473 and 2475 sweeps in sidepost are in no row; the
  test only holds such sweeps to fall among the original's strikes, and skips a chassis row that
  repeats the next one.
- The refusal branch (both sides moving and facing past 0.7074) and a moving struck object are never
  hit: free09's props are static. Not captured.
- Only the riding chassis on free09's garage shed; no tree trunk, fence or other machine was swept.
- The captures and RAM images live only in s2's context/; the test prints and passes elsewhere.

## Next

None; S202 for the one-tick wreck.
