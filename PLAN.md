# PLAN — how work gets done here

`AGENT.md` has the rules. This file has the mechanics: how a task goes from the list to a commit, and
where everything lives so you never have to search for it.

## Continue

"Continue" is one task, start to finish:

```sh
./kit plan next                 # the first open task
./kit plan brief T7             # its line, its own file, its journal, the files it names
                                # ... do the work ...
./kit check                     # tests
./kit journal new T7 walker     # the entry, including what you could not verify
./kit done T7 "port the walker" # tests again, tick the task, commit, push
```

Then stop and report what you did and what you did not verify. One task per iteration. Don't merge two
tasks because they look related, and don't skip ahead to a more interesting one.

If a task turns out to be bigger than one sitting, split it before starting: write `plan/<ID>.md` files
for the parts, `./kit plan add` them, then take the first.

## The task list is a tool, not a file

Don't read `plan/TODO.md` and don't edit its checkboxes by hand. It is a plain-markdown file so a human
can skim it, but every change goes through the command, which keeps the format consistent and the archive
in step:

| Command | Does |
|---|---|
| `./kit plan` | the index: sections and how many are open in each |
| `./kit plan next` | the first open task |
| `./kit plan T7` | that task: its own file if it has one, else its line |
| `./kit plan brief T7` | everything you need to start it, in one call |
| `./kit plan tick T7` | done → moves it to `plan/DONE.md` |
| `./kit plan block T7 "<why + what unblocks it>"` | blocked, with the reason recorded |
| `./kit plan unblock T7 [note]` | un-blocked |
| `./kit plan add T "text" [@ where]` | a new task, next free ID in that prefix |
| `./kit plan move T7 top\|before T3\|after T3\|in <section>` | reprioritise |
| `./kit plan note T7 "<text>"` | add a line to its file |
| `./kit plan newid T` | the next free ID in a prefix |
| `./kit plan archive` | sweep finished lines into `plan/DONE.md` (after merges or hand edits) |

`brief` is the one that matters. It assembles the task's line, its plan file, its journal directory (the
newest entry pasted in, capped), and each file the task names with its length and last few commits. One
call instead of eight, and you start with the same context every time.

Why a tool and not a file: a task list you read is a task list you read *all of*, every turn, for the
rest of the session. The list is a few hundred lines; `./kit plan next` is three.

## The journal

Every task gets an entry in `research/journal/YYYY-MM-DD-<ID>-<slug>/`. This is the project's memory: what you
found, what the original does, which addresses you used, what you measured, and — the important part —
what you could not check.

- One README per task. Newest iteration file names whose move it is: `-FINAL` (done), `-PART` (an agent
  continues), `-READY` (needs the human).
- `./kit journal new <ID> <slug>` creates the directory and stubs with the required sections;
  `./kit journal state <ID>` answers "whose move is it" and `./kit journal finish` closes an entry.
- Captures, screenshots and dumps go in `context/`, **never** in the journal — the journal is committed.
- The last section is always `## Not verified / not 1:1`, with a reason per item, or `none`. If this
  section is missing, `./kit done` tells you so.

The journal is why a project like this can survive being picked up months later, or by someone else, or
by an agent with no memory of yesterday. Write it for that reader.

## Wrap-up

```sh
./kit done T7 "port the walker"                  # tests → tick → commit, one call
./kit done T7 "partial: tables read" --blocked "needs a save state in the second stage"
```

It runs the test command, runs the float lint if the port has one, refuses to commit if the tests fail,
ticks or blocks the task, commits everything including the journal entry, and pushes the branch it
committed on. One call, so a wrap-up cannot be half-done — the state where the code is committed but the
task is still open is the one that confuses the next session most, and the state where it is committed but
only on this disk is the one that loses work outright.

Pushing your own branch is nobody's manual step: `./kit done` pushes the branch you are on when it is a
task's own (an agent's `task/<ID>` from its worktree, a topic branch here), and a `./kit run` pushes the
branch of any task it could not merge itself, before handing it to the master. Publishing the **shared** branch stays a
decision somebody makes: neither a wrap-up nor a run pushes it, and a wrap-up in the main checkout prints
the one-line `git push` instead. A merged `task/<ID>` is dropped from the remote, but only once the remote
actually holds the merge — before that, the branch is the only copy of that work the remote has. A remote
that has moved is merged in first, never rebased, because the run's worktree branches are based on those
commits. A push that fails is a logged line and nothing more. Turn it off with `[git] push = false`.

## Code map

Keep this current. It exists so nobody greps the repo to find where something lives.

### The kit

`ps2kit/` — the tooling. All of it generic; see `docs/INTERNALS.md` for the contract.

- `config.py` reads `kit.toml` (the only game-specific file) · `platform_.py` the whole OS surface
  (the only OS-specific file) · `cli.py` dispatch
- static: `disc.py` ISO 9660 · `elf.py` PS2 ELFs and overlay ELFs · `ghidra.py` headless analysis ·
  `decomp.py` read the dumps one function at a time
- live: `pine.py` PCSX2 memory · `pad.py` virtual controller · `emu.py` launch/stop/screenshot/states ·
  `probe.py` find addresses · `record.py` declarative captures
- workflow: `plan.py` · `journal.py` · `checks.py` · `runner.py` unattended runs · `oracle.py` ·
  `setup.py` setup and doctor · `hooks/` the context guards

`watch/` — capture specs (`*.toml`), one per thing you record. `research/` — your own scripts and notes,
committed, no game data. `context/` — everything game-derived, git-ignored.

### The port

`crates/` by default, but this is yours to shape. The three that come with the kit, and what this port has
added beside them:

- `ps2-float` — the PS2's arithmetic: EE FPU add/sub/mul/div/sqrt/madd, VU0 macro ops, the game's own
  transcendentals. Everything in the simulation goes through this.
- `ps2-oracle` — a small R5900 interpreter that runs one function from the game's own code so a test can
  compare your port against it over thousands of inputs.
- `game-sim` — where your port goes. Starts nearly empty, with the replay harness and the state hash,
  because those two are what keep everything after this honest.
  - `hash.rs` — the state hash: one walk over the whole state, folded into 64 bits for a per-tick
    comparison or dumped field by field to name the first field that differs (A1). Every state type
    describes itself once, with `impl hash::State`.
  - `input.rs` — the input layer: the pad read once a frame before the simulation, mapped into the
    game's own bitmask and kept as sixteen controls a pad — held, strength, the press and release
    timers — plus the one-frame event queue the front end reacts to. The original keeps no "pressed
    this frame" mask, so the edge is the event and "held with the timer at 0" (I1). The same poll's
    six analogue axes: the report byte each one reads, the inversion one of them gets, and the dead
    zone of ±0.2 that is the only shaping there is — no clamp and no curve (I2).
  - `replay.rs` — the replay harness: a fixture of inputs and expectations replayed tick by tick (A2),
    plus the pad tape — a recorded session as the report bytes and measured `dt` of each frame, which
    `Playback` plays back through the input layer so a human-played sequence drives the port (I3).
  - `rng.rs` — the C library's 64-bit LCG the whole game calls, plus the random float and the uniform
    list index its call sites wrap it in. Proved bit-exact in `tests/rng.rs` (R5).
  - `track.rs` — one track as the disc stores it: the `.scn` scene (spawn, terrain layout, every model and
    collision placement), the `.col` shapes the simulation tests against in their own bounding-volume tree,
    and the `.trn` quantised heightfield with the game's own height and surface query. Proved against
    free06 in `tests/track.rs` (D6).
  - `tick.rs` — the fixed update: one frame as the original runs it. The mode mailbox, the Timer 0
    clock, the thirty-entry `dt` average with its 30 fps clamp, the pad latch, the markers around the
    simulation, and the three different ways a frame can not simulate. `Engine::step` is the top-level
    step everything else hangs off. Proved against `engine_tick` itself in `tests/tick.rs` (S1).
  - `actor.rs` — actor lifetime: the two intrusive lists a level's live objects hang in, the interned
    name an actor's "kind" actually is, the three steps of creating, registering and freeing one, and
    the master list's order — which is registration order, and so is the order the per-frame dispatches
    and the two "nearest actor" searches resolve in. Proved against the manager, the record's
    constructor and both destructors in `tests/actor.rs` (S2).
  - `air.rs` — the bike in the air: where the sim decides the wheels left the ground, the hang time and
    the crossup angle, the preload and the backflip taken off a lip, what the stick still commands while
    airborne, and the test a landing has to pass for the rider to stay on. Carries the game's own
    double-precision arc cosine and the startup table built from it. Proved against the original over
    20,000 generated substeps in `tests/air.rs` (S10).
  - `trick.rs` — tricks: the 57-stunt table the disc ships and each player's 24-slot gesture map, the
    stick-and-button gesture that names one, the hold/safe/duration clocks that decide whether a trick
    counts, and the trick's own half of the landing test. Includes the engine's shared arc-cosine
    table, which the stick classifier reads. Proved against the disc and the oracle in
    `tests/trick.rs` (S11).
  - `collide.rs` — collision against the world: the triangle test the original sweeps a bundle of probe
    segments through a collision mesh's tree with, the box rejections that keep a probe out of a branch,
    how a hit resolves into the blocked travel a caller takes off the motion, and the two limits the
    sweep allows itself (16 probes, 256 levels). Proved against the original's sweep over 10,000
    generated scenes in `tests/collide.rs` (S4).
  - `state.rs` — the main actor's state machine: riding, down after a wreck, waiting, put back. The
    reasons a rider comes off, the settle window and the wait that decide when it goes back on, the pad
    chord beside them, which transitions share a frame, and the fact that a down rider's physics does
    not step at all. Proved against the original machine, its entry and its put-back in
    `tests/state.rs` (S5).
  - `move.rs` — movement on the ground: the per-frame step that turns the throttle, the brake and the
    rider's lean into forces on the chassis. Where the mass acts and how far the stick moves it, the
    steering lock fading out with speed, the donut's roll, the suspension rates, the gearbox and the
    clutch's launch ramp, the drag that sets the top speed, the rev limiter — and the substep loop
    that is the order the whole ground frame runs in. The twenty-one callees it drives (suspension,
    tires, roll and pitch, the torque curve) are not ported yet, so the step returns the ordered list
    of them instead, which is what makes the order provable. Proved against the original over 2000
    generated frames and a 240-frame ride in `tests/move.rs` (S3).
- `game-render` — the drawing side: the disc's own models and textures, on screen. Host `f32`, no
  bit-exactness, nothing the simulation depends on.
  - `disc.rs` ISO 9660 → `MX.PSR` → a `.res` member, read from the user's disc at runtime ·
    `psg.rs` `.psg` geometry: nodes, materials, LOD groups, VIF streams → triangle strips ·
    `pst.rs` `.pst` textures: a 256-colour palette in the GS's order, plus its mip chain ·
    `terrain.rs` `.trn` ground: a patched height field, its palette and its ground texture
  - `ini.rs` the plain-text format the disc's data files share · `scene.rs` `.scn` scenes, `.psm`
    materials, `.sub` bike builds and the `cpresets.ini` follow cameras · `world.rs` a track in
    memory: models, materials and textures matched by name, and drawn
  - `raster.rs` a depth-buffered, perspective-correct, textured triangle rasteriser · `png.rs` a PNG
    writer · `mat.rs` vectors and 4x4 transforms in the files' own convention ·
    `bin/first-draw.rs` the whole chain: disc in, one frame of a track out

As you add modules, list them here with one line each, the way `ps2kit/` is listed above. A code map that
is a sentence per file is worth more than a directory listing, and it is the cheapest context in the
project.

## Unattended runs

```sh
./kit run once                  # one task, then stop
./kit run 3                     # three agents at a time, each in its own worktree
./kit run 3 --mode d            # ... picking tasks that touch different files
```

Each agent gets a git worktree on a `task/<ID>` branch, so three can build and test without fighting over
one target directory. Tasks are picked to avoid working on the same files, and each branch is merged when
its session ends.

A merge that is not append-only text goes to the **master**: one more long-lived session, in the main
checkout and its own tmux window, handed those branches one at a time. It exists because a run skips tasks
whose branch is still waiting to be merged — so without it a few conflicts stop the critical path until
somebody wakes up. It resolves, runs the tests, ticks the task, drops the branch, and writes a line in
`context/notes/master.md` saying what it decided. It does not publish the shared branch: that stays the
decision above. See `docs/WORKFLOW.md` for what to expect from a long run and how to pick up the pieces
afterwards.

## When blocked

Stalled means: a tool hangs, the emulator will not cooperate, the same fix fails twice, something needs a
human, or twenty minutes pass with nothing to show. Then: stash the experiments, write what happened in
the journal, `./kit done <ID> "<what is solid>" --blocked "<why + what unblocks it>"`, and take the next
open task — preferably in a different section, since the obstacle is often shared within one.

Never end a run while open tasks remain. At the end, list the blockers. Human-only items go in
`plan/HUMAN.md` — never into the code as a guess.
