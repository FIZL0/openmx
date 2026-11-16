# 1 — s12 ai in the sim (FINAL)

Task S331c. 2026-11-16.

## What I did

- **Built on S331b.** S331b (finished, pushed as `task/S331b`, not yet merged into main when this
  session started) already hands the race driver its AI net (`Racing::net`, `Lane::net`). It changes
  the same lines (`Pilot::think`'s arguments, `Sim::think`). So I merged `origin/task/S331b` into this
  branch (a fast-forward) and built on it, rather than block with `--after S331b` or leave the runner a
  conflict on the same lines. Merging this branch brings S331b's commit too, and S331b passed its own gate.
- **The route node** (`ai::route_node`, `Jump::of`, `HabitNode::of`, crates/game-sim/src/ai.rs). The
  trick plan's jump and the habits' node now come from the node the brain's own spot is on in its AI
  net (`Racing::net`). They are read where each step runs: the tail in `route`, `Driven::plan` inside
  the race driver (after the route follower has moved the spot), and `approach` (the habits). The
  `Around::jump` and `Around::node` fields are gone. Their three literals (ai.rs, tests/ai/drive.rs,
  tests/ai/sim.rs) lost the two `None`s.
- **The edge and the spot** (`ai::OnTrack`, `on_track_of`). This is the race track under the card: the
  laps' track paths, the same paths as an `ai::path::Net`, and the card's spot (`LapCard::spot`).
  - `OnTrack::edge` is `path::off_track` on the card's path from its first node, pushing along the
    card's node. It is `None` when the spot names no path or node.
  - `OnTrack::spot_point` is the track's point at the card's spot.
  - `Pilot::think` takes `track: Option<OnTrack>` and builds `Around::edge` (only with a card) and
    `Around::spot` from it.
- **In the Sim** (`Sim::think`, crates/game-sim/src/tick.rs): on a race with laps, the lane makes the
  race track's `path::Net` the first time a brain thinks (`Lane::edge`, kept in an `Arc`). Each think
  then hands `on_track_of(laps, net, player)`.
- Nothing taken from opentennis: it has no path nets or AI riders.
- Filed **S336** (the habits' keeper state is never fed) and **S337** (low: the free-ride path branch
  still reports `"path"` missing).

## What the original does

- **Route node.** `ai_trick_plan` reads the brain's `+0x224`: the node word of its follower spot
  (`+0x220` path, `+0x224` node, `+0x228` along) on its net (`+0xdc`, the AI file). It reads the node's
  `+4` flags (0x20 jump) and `+0x48` air time. `ai_habits` (`./kit fn 0x00280070`) reads the same
  `+0x224`: `+4` flags (8 pull-back, 0x40 take-off, 0x100 clutch), `+0x44` speed (below 0 → 150) and
  `+0x4c` gain.
- **Edge.** `ai_update_mx` (`./kit dis 0x00387ea4 0x00388088`) pushes only when all of these hold:
  flag 0x20000, race (`+0xd8`), card (`+0xd4`), the card's `+0x88` path, its `+0x8c` node and the
  race's `+0x68` net (the race file) are all set. Then it calls:
  1. `path_locate(-1, net, 0, ahead, path, out)`;
  2. the located segment's next and the one after (`+0x54`); any null → no push;
  3. `path_segment_holds` on both stretches; either holding → no push;
  4. `path_node_direction(net, card node, io = ahead)`.
- **Spot.** `ai_habits`' cross-up calls `path_spot_point(keeper+0x68, card+0x88, sp)`, where the
  keeper is rider `+0xfa0` and the card is rider `+0xf9c`, into a vf0 quad. With no keeper track it
  uses the brain's `+0x1d0` instead.

## How it was proved

`s331c_the_path_hooks_read_the_nets_as_the_race_image_does` (crates/game-sim/tests/ai/on_track.rs).
It runs on S282c's sx01 race image (three AI riders on the race driver), using the image's own two nets
read out of RAM:

- **Route node.**
  - For each brain, its `+0x220` spot gives, through `route_node`, the very node `+0x224` points at.
    `Jump::of` and `HabitNode::of` match that node's raw words.
  - Then every node of the 444-node AI net is checked by its spot.
- **Edge.**
  - For each brain, the card's `+0x88` path (its `+0x10` first node) and `+0x8c` node are the
    `Spot`'s path and node on the race file. The brain's race `+0x68` is the race file.
  - Then 10,000 random spots on the race net, with random points within 60 ft of any node point:
    `OnTrack::edge` against `ai_update_mx`'s stretch replayed call for call through the original's own
    `path_locate`, `path_segment_holds` ×2 and `path_node_direction`. 4,507 pushed and 5,493 held, all
    bit-exact.
- **Spot.**
  - In the same 10,000 cases, `OnTrack::spot_point` against `path_spot_point` into a vf0 quad, with
    along from −0.2 to 1.2: 2,816 clamped and 465 on a last node, all bit-exact.
  - Then each card's own `+0x88` spot.
- **Checks on the test itself.**
  - A temporary panic after the last assert fired, so the test runs to its end rather than skipping.
  - Mutant `node = n` (not `first + n`) fails at "case 4: the edge".
  - Mutant airtime from `+0x44` fails at "brain 0x8c9af0: the trick plan's and the habits' node".
- `./kit check -- -p game-sim --test ai`: all 57 tests passed before the new test was added. That
  includes S314b's joined race driver on the image, which now reads the derived node. Its rider never
  leaves the ground, so this does not prove the jump read.

## Not verified / not 1:1

- **No ride compared against a capture.** No PCSX2 recording of a supercross race with AI riders was
  compared tick by tick, so the trick plan's and habits' reaction to a real take-off node in a ride is
  not proved. The reads (which node, which words) are proved on the image; the trick plan and habits
  themselves were oracled by S12d and S312.
- **The spot's clamp is not written back.** The original's `path_spot_point` clamps `along` into the
  card's own `+0x90`. The port clamps a copy, and the lap card keeps its own spot. The laps' spot
  search keeps along within 0..1, so the clamp should never fire there. That was not checked over a
  ride.
- **A spot naming a node on another path** (S311's case) cannot be held by `race::Spot`, so
  `route_node` and the edge read node n of the spot's own path. This never happens in the sx01 image.
- **Keeper state.** The habits' cross-up skips when the keeper's `+0x64` is 3. The Sim never sets
  `Habits::keeper_state`, so it is always 0. Filed as S336.
- **The free-ride path branch** (`seek::Seek::Path`) still tells `"path"` missing. It has no node for a
  free-ride brain to read, since its spot never moves. Filed as S337.
- **The edge's net is made once per lane** (`Lane::edge`, from the first laps it sees). A lane kept
  across two races with different tracks would push against the first track. Today the shell spawns
  a new lane for each race.
- **Edge without a card's spot.** If the laps have a card for the player but its spot names no path
  or node, there is no edge, as in the original. `None` laps give no edge and no spot. The habits'
  fallback to the brain's `+0x1d0` then comes from `habits` itself (S312).

## Next

None for S331c. S336 feeds the keeper state.
