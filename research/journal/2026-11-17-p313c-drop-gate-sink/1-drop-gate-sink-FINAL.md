# 1 — drop gate sink (FINAL)

Task P313c. 2026-11-17.

## What I did

- Read every disc track's `.toy` (a scratch crate over `game_sim::archive` on the iso's MX.PSR). The tracks
  whose `[DropGate]` sets `MoveGateDown` TRUE: nat09, nat12, nat13, nat16 (`True`), nat19, nat20, sx17,
  sx18. All of them have `MoveTime=0.25`. Every other track is FALSE, and the free tracks have no `[DropGate]`.
- Their words (GateName, NumberSoultreePieces S, NumberCollisionPieces C):

  | track | GateName | S | C | collision pieces the scene has |
  |---|---|---|---|---|
  | nat09 | Nat09_dropgate01 | 1 | 0 | — |
  | nat12 | Nat19_Dropgate01 | 2 | 1 | wants 03, has only 02: none |
  | nat13 | Nat03dropgate | 1 | 1 | wants Nat03dropgate02.col: none |
  | nat16 | Nat19_Dropgate01 | 2 | 2 | 03, 04 |
  | nat19 | Nat19_Dropgate01 | 2 | 1 | wants 03, has only 02: none |
  | nat20 | Nat20_dropgate01 | 2 | 1 | 03 |
  | sx17, sx18 | THQsxdropgate01 | 2 | 2 | 03, 04 |

- `crates/game-sim/src/drop_gate.rs` (new): the names (`soultree_names`, `collision_names`, `gathered`)
  and `Sink`: `bind` (the trigger, the `[LowerGate]` spot's path distance to the line), `tick` (the card
  walk and the height step, F bit-exact), `stand` (the gathered props to the identity).
- `Race::sink` (boxed, on the wire, walked as `sink` only when present). `Sim::race_frame` ticks it after
  the race manager and stands the pieces through `World::moving()`.
- `Props::load_indexed`: `Props::load` plus where each scene collision entry went, so a skipped file does
  not shift the pieces' indices.
- Shell: `race_start::SinkWords` (the toy words), `gate_sink` (built only when MoveGateDown is set, and only
  on a disc track, not a world's tiles), `Start::sink` handed to the race. `GateSwing` takes the other
  soultree pieces out of the scene and draws all of them lowered by the race's `Sink::y`
  (`GateSwing::pieces` / `actors`, both render paths). The unused `GateSwing::actor` went.

## What the original does

Recipes: `./kit fn 0x002af2e0` (the reader), `0x002ff2e0` (init), `0x002ff3d0` (bind), `0x002ff540` (the
pieces by name), `0x002ff900` (update), `./kit dis 0x001edc90` (the mesh collidable's vt +0xa8). The whole
reading is in research/oracle.txt under "P313c".

- The bind finds the path spot nearest `[LowerGate]`'s TriggerPosition and measures from it to the line.
- With MoveGateDown, the gate hangs the scene props named after GateName under a group node of its own, and
  lists the `.col` objects numbered after them.
- Each tick, while armed, the update walks the cards in list order. The first card whose kept spot is
  farther from the line than the trigger, and whose spot is nearer, starts the sink and disarms.
- In the sink, the group's height steps `y = (-100 - y) * (t / MoveTime)` from where it is (not a blend):
  at 30 Hz with 0.25 s that is −0, −13.3, −23.1, −30.8, −36.9, −42.0, −46.4, −50.1, then −100 on the
  ninth tick.
- Each sink tick tells every listed collision mesh to take its node's matrix. A static scene collision
  object has no node (its load passes 0), so the matrix is the identity: the meshes go to the world origin.
- RaiseGate's counter (+0xb4/+0xbc/+0xc4) only clears its own flag. Nothing reads it, so it is not ported.

## How it was proved

Static reading only. Unit tests: `crates/game-sim/tests/drop_gate.rs` (6: names, gathering, the trigger on a
built straight path, the height sequence with its −0, a gate that does not sink, the identity stand) and
`race_start.rs` (3 new: the words on nat19's and sx17's text, the pieces taken and lowered, the props'
indices past a skipped file). No live run: none of the eight tracks has a save state or a capture.

## Not verified / not 1:1

- **No live proof.** No save state or capture exists on any sinking track, so nothing here was compared with
  the running game. Filed as C40.
- **Order in the tick.** The gate is a scene object. Whether its update runs before or after the race
  manager's in the same frame was not traced. The port runs it after the manager (and after the laps), so a
  card's distances are this tick's. If the original runs it first, the sink starts one tick later.
- **The [LowerGate] object's allocation** was not found. Only the free when there is no [DropGate] was seen.
  The port assumes the object exists whenever [DropGate] does.
- **nat12, nat16, nat19** write the trigger as `Position=`, which the reader does not know, so their trigger
  is the default origin. Ported as read: the spot is the path spot nearest (0,0,0), whatever that is.
- **The group composition** (a child's world = its local, kept from its old world, times the group's) is
  assumed from 0x001b3220 keeping the local transform. The render adds y to the pieces' translation.
- **Before the drop.** The trigger runs on whatever the cards hold, including before the drop. If the grid
  stands a card past the trigger, the sink would fire at once. That is what the reading says; not checked.
- **A track with no paths** (or a dry route pool) leaves the spot null; the original measures from a null
  path. The port counts that distance 0.
- **Rollback.** The props stood at the identity are world state, not on the wire. A netplay rollback to
  before the trigger leaves them at the identity until the trigger fires again. Harmless in practice (they
  only leave), but not 1:1 with a fresh run.
- **Render floats.** The drawn pieces use host f32 for the turn and the y add. The sim's y is bit-exact.
- **MoveGateDown false** builds no sink in the port. The original still walks the trigger there, but with
  `on` false it does nothing visible.

## Next

Done here. The follow-up (C40) is a live proof on one of the eight tracks (sx17 has two collision pieces that
match): a race save state, then a capture of the group's y and the pieces' collidable matrices as the first
rider passes the trigger.
