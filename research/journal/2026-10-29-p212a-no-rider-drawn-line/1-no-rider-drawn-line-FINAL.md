# 1 — no rider drawn line (FINAL)

Task P212a. 2026-10-29.

## What I did

- `game-render` gpu.rs: `Drawn` gets `actors: Vec<Vec<usize>>`, the triangles each of a view's actors
  sent, in `View::actors`' order, and `Drawn::actor(v, i)` (0 for one it does not have). An actor is
  counted where its instance is queued: its level-of-detail group's runs, each `count / 3`, which is
  exactly what the solid, blended and ghost passes draw (each run once). An actor whose model never
  reached the GPU (`model_ids` has no id, or `None` for a model that failed to upload) or at alpha 0
  counts 0. Actors are never frustum-culled on the GPU, so those are the only ways a rider is left out
  there besides the shell not queueing it at all.
- openmx main.rs: the GPU path remembers seat 0's bike and body indices in `actors` (every view's list,
  marked or wrapped, keeps the shared list's order, so view 0 indexes them right). Each frame whose
  `Drawn` says the bike drew triangles counts toward `rider_frames.0`, and of those, the ones where the
  rider drew none — or the shell never queued a body (`shown.body` or `rig.body_model` `None`) — toward
  `.1`. The 1 s line ends ` (no rider drawn)` when every such frame lacked the rider and
  ` (no rider drawn in N of M frames)` when some did; only while seat 0 is riding (phase Riding, bike up,
  simulated, not waiting), the line's state word empty. `rider_left_out` builds the words.
- Filed P223 (after P212a): read the next phone's log for the cause, and wrote the user's step in
  plan/HUMAN.md ("A bike but no rider on the iPhone"). The task's second half — read the next phone's
  openmx-log-previous.txt — needs the user to launch on the phone and send the log; nobody is here to.
- opentennis: nothing to take, its renderer has no per-actor count.

## What the original does

Nothing to port: this is a diagnostic line in the shell.

## How it was proved

- `gpu::tests::each_actor_counts_its_own_triangles`: on the headless GPU (it ran, not skipped — checked
  with `--nocapture`, no "skipped" line), the known scene's board as three actors: solid counts every one
  of `Model::triangles(0)`, an unknown model and the board at alpha 0 count 0.
- `rider_left_out_line::the_line_says_when_the_rider_was_not_drawn` for the words.
- A real headless GPU ride on the default track (`openmx --headless --seconds 4`, RADV): the line ends in mph, no
  suffix. With a throwaway edit (not committed) naming the body actor's model "nothing", the same run
  printed `1s 46.7 fps, 215761 triangles a frame, 0.4 mph (no rider drawn)` every second.

## Not verified / not 1:1

- Not run on the iPhone (Metal): the cause of the 2026-10-28 launch is still unknown; P223 waits on the
  user's next phone log.
- The software renderer path does not count the rider: the task asked for the GPU path, the phone and
  every default launch use it.
- A rider drawn but wholly off-screen, or drawn with a transparent texture or a broken pose (all nodes
  at the origin, NaN matrices), still counts its triangles: the count is what was sent to the GPU, not
  what reached pixels. If P223's log shows triangles but the phone shows no rider, that is the next
  suspect (pose palette / skinning on Metal).
- In a split, view 0 shows every seat; with two riders on the same body model only seat 0's entry is read,
  so the count is seat 0's own.

## Next

P223 once the user sends the phone's logs.
