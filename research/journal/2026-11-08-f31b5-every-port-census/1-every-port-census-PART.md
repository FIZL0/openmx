# 1 — every port at once (PART)

Task F31b5. 2026-11-08.

## What I did

- **`replay_whole`** (`crates/game-sim/tests/mxvatv_bike_ride.rs`) runs [`replay_pure`] with F46's
  four names Rust's too. That puts every landed port on at once:
  - F31b5g/h: the steps hand nothing on;
  - F46: the database moves, the gather, the sweep and the part search;
  - F43/F44/F45/F47: their ports, already in `port_tail`'s update rig.

  Every row is checked word for word against C15, and the whole RAM against the handing run's after
  every frame. `replay_pure` now takes the `IN_RUST` names, and they apply to its second run only.
- **Two tests**, `ride_replays_with_every_port_at_once` and `flip_replays_with_every_port_at_once`.
  Ride-2: 459 rows, no wreck, RAM alike. Flip-1: 465 rows, wrecked on row 125, put back on row 193,
  RAM alike except the dropped cue's word. Both assert that the only calls still handed on by name
  are F54's: the part stand (432 / 161) and the hit listeners (282 / 110).
- **An interpreter census.** `ps2_oracle::Ee` gained `ran`, a running count of instructions across
  all calls (`steps` resets on every call, nested ones too). `posts_frame` books the count per part
  of the frame into `INTERPRETED`. `TreeInRam`, `TailInRam::out` and `UpdateInRam::out` book their
  own sub-parts, which nest inside "the tail".
- **Filed** the uncovered parts as F31b5i..m (below).
- Nothing taken from opentennis: it has no interpreter-backed frame to compare against.

## What the census says

Interpreted instructions over the whole replay, every port on (ride-2 / flip-1):

| part | ride-2 | flip-1 | who |
|---|---|---|---|
| the rider leaf (the manager's walk steps it) | 37.2M | 46.3M | F31b5i (on S278a/b's ports) |
| the original vehicle update, run beside its port as a check | 12.3M | 13.9M | none: a check, drops with the interpreter |
| the update's part walk (`Out::Walk`) | 4.4M | 3.2M | F31b5j |
| the bike's world step (F54's part stand and hit listeners, list accessors) | 2.5M | 2.0M | F54, F31b5m |
| the wreck fade's global record search (`Out::Search`), kind clear | — | 2.8M | F31b5l |
| the tail's per-frame message post (the event post) | 1.46M | 1.34M | F31b5l (F48 for the wreck posts' listener) |
| the net send queue (`Out::NetQueue`, inside `Out::Network`) | 1.25M | 1.22M | F31b5k |
| the manager's own work, the first other leaf | 0.34M | 0.34M | F31b5m |
| the other records' clear and pass, the sim list accessors | 0.19M | 0.08M | F31b5m |
| rumble, prepare, wreck test leftovers | <0.03M | <0.07M | small; F31b5l/m to sweep up |

## How it was proved

- `./kit check -- -p game-sim --test mxvatv_bike_ride -- every_port_at_once nothing_handed_on`:
  4 tests passed.
- The census numbers come from running the built test binary with `--nocapture` (about 43 s for
  both). The table is the `INTERPRETED` printout.
- Gate: `./kit check --all` in `./kit done`.

## Not verified / not 1:1

- **The acceptance is still open**: neither replay runs without the interpreter. The census above
  lists what still runs there. It waits on F31b5i, F31b5j, F31b5k, F31b5l, F31b5m and F54. After
  those land, the frame's state still has to move out of RAM into Rust-held structures; that last
  step stays F31b5's own.
- **The census is attributed by region, not by function.** A region's count includes everything
  nested in it, e.g. "out Parts" includes "the update's Walk". The table gives the innermost
  attribution I measured. The world step's 2.5M is not split between F54's two calls and the list
  accessors.
- **"The original vehicle update beside the port" is my reading of `TreeInRam`**: it runs the
  original and then the port, and compares their RAM. The port's results are the ones kept, so a pure
  frame drops the original's run. No test runs the frame without it yet.
- **Rumble, prepare and the wreck test's leftovers** (under 0.1M) are not given a task of their own.
  I folded them into F31b5l/m's sweep-up by judgement. I did not check which calls they are.
- **The other five riders' PreProcess** (part of the task's line) are the "other records' clear and
  pass". F31b5d showed them inert on state 4; dropping them from the frame is F31b5m.

## Next

When F31b5i..m and F54 land, the frame with every port on should show nothing in `INTERPRETED`
except the vehicle update's check. Then build the frame with no `Ee`, starting from `replay_whole`.
