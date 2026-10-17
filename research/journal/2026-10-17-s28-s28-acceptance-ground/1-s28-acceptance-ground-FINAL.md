# 1 — s28 acceptance ground (FINAL)

Task S28. 2026-10-17.

## What I did

The acceptance run had no ground under it. S8 left it diverging at tick 1 with the port's bike in free
fall towards `Flat`'s floor at y = 0 while the human's recording is a minute of Free09 at y ≈ 208, and
the brief named the missing piece: a `verify` that takes a world, and a capture that says which world.
Both are in now, and the bike is the Race 125 the loader builds for a freeride boot (D7) instead of
`stock`'s stand-in 250.

**The seam** (`crates/game-sim/src/replay.rs`, `crates/game-sim/src/lib.rs`):

- `Fixture::world: Option<Where>`, where `Where { track, area }` is the pair of names the disc uses:
  `Tracks\free09.res` and `tracks\free09\free09.scn`. A fixture carries the *names*, not the world —
  the world is tens of megabytes of disc data and nothing under `context/` is committable, so the
  fixture says what to load and the caller loads it.
- `replay::verify_in(&Fixture, &mut dyn World)` and `replay::state_at_in(...)`, which are the old
  `verify`/`state_at` with the world as an argument. `Sim::tick_in(input, world)` is the step they
  drive, beside the existing `Sim::tick`.
- `verify` keeps its old signature and its flat floor, and now **refuses** a fixture that names a
  world: `Reason::WrongWorld { want }` at tick 0, rather than silently free-falling 208 units. That is
  the bug this task removes, made unrepeatable.

**The naming** (`crates/game-sim/tests/replay.rs`): nothing in the capture says which level it was
recorded on — `watch/acceptance.toml` records the bike, and adding a column means re-recording the
minute, which needs the human and the GUI (PCSX2 playback is a menu). So the area is read off the ride
itself, in `recorded_on`: for each of the disc's five freeride areas, how the recorded bike's 1,811
positions sit against that area's heightfield. A ride stays above its ground and comes down on it, so
an area is out if it ever buries the bike by more than 2 units and out if the bike never comes within
2 units of its dirt. Exactly one area survives, and the test asserts that.

On the acceptance capture the first half alone is decisive:

| area | rows buried > 2 of 1,811 | deepest | closest above |
| --- | --- | --- | --- |
| free06 | 878 | 148.71 | — |
| free07 | 315 | 77.22 | — |
| **free09** | **0** | **1.53** | **≈ 0** |
| free10 | 1,146 | 84.26 | — |
| free11 | 197 | 18.07 | — |

On the shorter `acceptance-ride.csv` (900 rows) it is not: that ride stays over ground Free11 also has,
60-odd units below the wheels the whole way, and Free11 buries it nowhere. The touch-down half throws
Free11 out and leaves Free09 again. A third signal agrees with both and is not used: Free09's own start
line `[4310, 208, 3316]` is 3.67 units from the acceptance capture's first row, where the nearest of
the other four is 320.

The bike is `Bike\Race125P.phy`, found in the first bike archive that has it (`Bike\125sx.res`, 33,672
bytes) and read with one shared `rider::Tables` so the `.phy`'s curve and the state that rides it come
off the same table. `tests/phy.rs` already proves that file against the live bike in the recording word
for word and proves every copy on the disc identical, so this task only states which file and that it
parsed.

`in_the_world_of(cap, then)` holds the borrow chain — archive bytes → `Terrain` → `Ground` → `Place` —
in one stack frame and hands the callback the world, its name and the bike. Both real call sites use
it: `the_acceptance_run` and `every_acceptance_capture_is_the_original_at_a_step_the_port_can_match`.
Where the disc is not in the worktree, both print that there is no verdict to take rather than taking a
flat-floor one.

## What the original does

Two decisions of the original's loader are what this task leans on, and both were already read out of
the game by earlier tasks rather than by me:

- A freeride boot loads the Race 125 pair, `Bike\Race125P.phy` and `Race125A.phy` — D7, which is
  why `game_sim::track::FREERIDE_ENGINE` is `Engine::Cc125` and `FREERIDE_PRO` is false. The track's own
  second archive carries the 250 pair of each tier, which is what used to make the port call a freeride
  bike a 250; the loader asks for the pair the *event* names.
- An area is a `.scn` scene in `Tracks\<area>.res` naming a `.trn` heightfield (the disc stores the
  platform's `.ps2.trn`), a start line, and the collision shapes and models standing on it — D6 and S4.

What the original does *not* do is record which of them it is in anything the watch spec can reach,
which is why the area is inferred. Nothing in this task read new decompilation.

## How it was proved

- `./kit check` green: `TESTS PASSED` across the workspace with the acceptance tests exercising the new
  path against the real capture and the real disc in this worktree.
- The area: a measurement over all five freeride areas and all 1,811 recorded rows, in `recorded_on`
  and asserted by `the_acceptance_recording_was_made_on_free09_on_the_race_125` — the table above. The
  same measurement runs on `acceptance-ride.csv`'s 900 rows and picks Free09 there too.
- The run itself, over Free09 on the Race 125, against the human's recording tick by tick
  (`acceptance-freeride.csv`, 1,810 ticks at the 30 Hz clamp):

  | lane | parts at, over `Flat` (S8) | parts at, over Free09 (now) |
  | --- | --- | --- |
  | `bike.pos[0]` | 1 | **30** |
  | `bike.pos[1]` | 1 | 1 |
  | `bike.pos[2]` | 1 | 1 |
  | `bike.pos[3]` | never | never |
  | `bike.vel[*]` | 1 | 1 (w never) |

  and on `acceptance-ride.csv`, `bike.pos[0]` holds to tick 33.

**So `ACCEPTANCE_AGREES_THROUGH` stays 0.** The run's verdict is its worst lane and `bike.pos[1]` still
parts at tick 1, so there is nothing to ratchet; raising it would be a gate the port does not pass. What
the world bought is real and is in the gate's own note: `bike.pos[0]` holding thirty ticks of real play
where every lane used to part at the first one.

Why tick 1 still parts is measured, not guessed, and it is no longer the ground being absent — it is the
bike's ride height:

- The recorded body origin at tick 0 is `[4309.563, 208.02943, 3319.6428]`. `Ground::at` puts Free09's
  surface there at **208.52306**, i.e. 0.49 units *above* the origin, and the cell is flat (its four
  corners are 208.502…208.526, so no interpolation choice can account for a unit).
- The port's wheel mounts are at chassis-frame `[0, -0.25, ±0.75]`, so after one tick both mounts are
  at y ≈ 207.82 — three quarters of a unit underground — the suspension is pinned at its stop
  (compression 1.0 of a maximum 1.0) and the springs push with 186.4 and 193.5 units. The chassis is
  shoved *up*: `bike.pos[1]` 208.10735 against the original's 208.02884. S8's free fall is gone and an
  over-compression has taken its place.
- Over a flat floor at a sweep of heights, the height that reproduces the original's tick 1 is
  **207.10**, to within 0.0005. So the port's bike geometry wants the ground about 1.42 units lower than
  this port reads it, and one of two numbers is wrong by about that much.

The capture cannot say which, and both candidates are outside this task's files:

- **The mounts.** The port takes them off the `.phy`'s `body_tire_force_position`; the original reads
  the bike *model*'s own tire nodes (S22, and `crates/openmx/src/main.rs` does it that way for the
  shell). Mounts a unit and a half higher would fit.
- **The height.** `Ground::at`'s blend is this port's own arithmetic (its own module note says so). On
  Free09 the scene's 276 collision placements sit a median **66.5** units above that height with a
  cluster at **−1.98**, and the 571 models the same way — a bimodal spread on a field that is 3 × 3 root
  cells where `tests/track.rs` proves the reader on free06, whose placements sit a median under a unit
  from it. That is worth a reader's second look before anyone blames the mounts.

## Not verified / not 1:1

- **The area is inferred, not recorded.** Free09 is the only one of the disc's five freeride areas whose
  ground the whole recorded ride stays above and comes down on, and its start line is 3.67 units from
  the capture's first row — but no column in the capture names a track, so this is a measurement over
  the capture's geometry, not the game's own statement. Closing it means a `track`/`area` column in
  `watch/acceptance.toml` and a re-record, which needs the human and the GUI.
- **Tick 1 onwards is still not the original**, and the verdict is `bike.pos[1]` 208.10735 against
  208.02884 — see above. Unresolved between the wheel mounts (S22) and `Ground::at`'s height (D6); this
  task measured the discrepancy at about 1.42 units and did not pick a side.
- **The wheel mounts are the `.phy`'s `body_tire_force_position`**, not the bike model's tire nodes,
  because reading the model is S22's and the mount is handed to `Ride::new` from outside.
- **`Ground::at`'s bilinear blend is this port's own arithmetic.** The original's heightfield query is
  not reproduced instruction for instruction, and the free09 placement spread above suggests the reader
  deserves another look on a multi-root field.
- **Props are loaded but 2 of 276 collision meshes are missing** on Free09: `UTILITYSHACK1.col`
  (collision class 1 is not understood) and `Large Egg.col` (in no archive of the track). Neither is near
  the recorded ride's first ticks, but the sweep the ride runs against is 274 meshes, not 276.
- **`wrecks_enabled` is off** in the acceptance start state, where the human's recording was made with
  wrecks on and contains two crashes. The capture holds no wreck state to seed, so the choice is a
  guess either way; off keeps the replay riding past the point the port would otherwise throw the rider,
  and is written here rather than hidden. It is the state `Sim::new` leaves too.
- **The start state is only the eight lanes the capture holds** — the world transform and the body
  velocity. The suspension's compression and the wheels' spin (both zero at tick 0 here, and the
  compression is pinned at its stop by the end of tick 1), the engine's revs and gear, the air and trick
  state, the balance accumulators and the input layer's press timers all start at zero where the
  original's were a minute into a ride. The respawn point is set to where the capture starts.
- **The trick set is `Set::default()`**, not the disc's stunt table: `Sim::ride` takes one and nothing
  in this task reads the game's.
- **The RNG seed is 32 bits of the original's 64.** Unchanged by this task, and still the fixture
  format's largest gap.
- **The text fixture format carries neither a start state nor a world.** `Fixture::to_text`/`parse`
  round-trip everything else; a fixture file replayed from disk therefore still runs over `Flat` from the
  spawn, and `the_acceptance_run` says so where it takes that path.
- **`replay::record` has no world-taking variant.** It records over whatever world the caller already
  has only by way of `verify_in`'s step; recording a run over a track means driving `Sim::tick_in` by
  hand. Not needed by anything today.
- **One of the two captures is a ride with no human on the pad for part of it.** `acceptance-ride.csv`
  is kept and measured the same way; its own input is not the recording's.

## Next

The gate is waiting on one number: whether the wheel mounts or the heightfield height is wrong by ~1.42
units at `[4309.563, 3319.643]` on Free09. The cheapest discriminator is reading the Race 125's model
and printing its tire node positions in the chassis frame (S22's reader, which the shell already has in
`crates/openmx/src/main.rs`): mounts about 0.7…1.2 *above* the body origin mean the `.phy` is the wrong
source and S22 closes this; mounts near `[0, -0.25, ±0.75]` mean `Ground::at` is reading Free09 high and
D6 does. Either way, `ACCEPTANCE_AGREES_THROUGH` should then move well past 1 — `bike.pos[0]` already
holds thirty ticks with the ground under it.
