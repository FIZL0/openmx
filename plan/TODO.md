# TODO — open tasks, in order

The rules are in `PLAN.md`; this is only the backlog. Read it with `./kit plan` (headings and ids),
`./kit plan next` (the next task to take) and `./kit plan brief <ID>` (everything you need for one task:
the line, its long form, its journal and the files, with line counts and recent commits). Never open this
file whole, and never edit a checkbox by hand — `./kit plan tick <ID>`, `./kit plan block <ID> "<why>"`,
`./kit plan unblock <ID>`, `./kit plan move <ID> …`, `./kit plan add <PREFIX> "<text>"`.

One line is one task, finishable in one session:

    `- [ ] **ID** what it has to do → files to open | t: tests it must add | j: journal slug`

`→ files` is what the task touches — the runner uses it to keep two agents off the same code, so keep it
honest and specific. `t:` names the test that will prove it; a task with nothing under `t:` either is not a
port task or is not thought through yet. `j:` is the journal directory under `research/journal/`.
`- [x]` done (it moves to `plan/DONE.md`), `- [~]` blocked with the reason on the line, `(after X)` waits
for X, `(in X)` is a subtask of X.

**Ids are permanent.** They end up in journal directories, commit messages, test names and code comments.
If a task comes back, reopen it under its own id; never renumber, and never give an old id to new work.

Prefixes: `D` disc · `R` decompilation · `C` the frame and captures · `F` arithmetic · `O` oracle ·
`A` acceptance harness · `I` input · `S` simulation · `V` rendering · `K` the kit itself.

### Next (in this order)

The first four phases are tooling and feel like a detour from the port. They are the difference between a
port that converges and one that is 90 % done forever: every task after them is proved with what they
build. Do them in order; inside a later phase, tasks that touch different files can go in any order.

### Phase 2 — a decompilation you can read


### Phase 3 — the frame, and your first capture

### Phase 4 — the arithmetic, before any ported math

### Phase 5 — the first proven function, and the harness

### Phase 6 — input, the game's own way


### Phase 7 — one subsystem, end to end

Pick the subsystem this game is *about* (the thing the player does every second) and take it all the way:
ported, oracle-checked, replay-proved. One finished subsystem teaches more about the codebase than five
half-ported ones, and it is the first time the port is worth running.

- [~] **S8** (after A2) The acceptance run: a full recorded sequence of real play (several minutes, from a human recording) replays tick-exact against the port. This is the point where the port stops being a collection of functions. Expect it to fail first on the smallest thing — that is what A1's field-level diff is for → crates/game-sim/tests/replay.rs · context/fixtures/ | t: crates/game-sim/tests/replay.rs | j: s8-acceptance — retargeted at freeroam: the acceptance run is a freeride recording — ride around, jump, trick, land, crash — not a race. S8 stays the moment the port stops being a collection of functions — the recording exists: `context/recordings/first_recording.p2m2` (3621 frames, ~60.4 s, two pads, PCSX2 2.9.127) with its own `_SaveState.p2s`, on Free09 with a Race125 bike — so the plan/HUMAN.md item is done and this is no longer waiting on a person. It is still waiting on the port rather than on the human: `Sim` is A1s example ball (crates/game-sim/src/lib.rs:70, the only `impl Game` is tick.rs:778), and S2-S5 and S7 are open, so there is no bike to replay against yet. Do I4 (the tape reader) and S14 (where the NPC contact bounds the window) first, and take D7 before trusting any physics comparison — the run is a 125, not the 250 the port assumes — the GUI playback is done (2026-10-16, by the agent over the desktop's accessibility bus, not by a person): the human's minute is captured off the original — 1,811 ticks, 1,449 with a control held, two crashes — the tick phase I4 left open is measured (the game reads the second field of each tick), S14's wreck window is measured (ticks 3259-3349 and 4167-4190, reason 5, so the tick-exact window ends at 3258), and the_acceptance_run builds a 1,810-frame fixture with 14,488 checks from it on every ./kit check. The captures, the reader, the refusals and the verdict are in and green BLOCKED: no longer a person — the port. The verdict is NoSuchField 'bike.pos[0]' at tick 0: the capture is good and there is no bike in the port to compare it against. S3 (movement) and D7 (the 125cc parameter set) land first, and the window stops at the first wreck (tick 3258) until reason 5 and Bike\Wreckbike.phy are ported. Nothing here needs an agent in the meantime — what it waits on has an id now: P5, the replay seam. S3 and D7 have both landed, so NoSuchField is the last thing between this capture and a verdict with a tick number in it BLOCKED: P5 landed, so the verdict is no longer NoSuchField — it is `tick 0: bike.pos[0] 4309.563 (0x4586ac81) vs 0`, and that is the start state, not the physics: the capture is frames 2774..4584 of a mid-session ride while Fixture::initial() falls back to Sim::new(seed) at stock::SPAWN because Fixture::start is "not expressible in the text format yet" (crates/game-sim/tests/replay.rs:101-106). Unblocked by seeding Fixture::start from the capture's first row, or by giving the text format a start state; decide there whether acceptance_verdict panics or reports, which P5 left to S8. The three acceptance captures are parked in context/capture/acceptance/ so main is green in every checkout — put them back (cd context/capture && for f in acceptance/acceptance-*; do mv "$f" .; done) before reading a verdict

### Phase 8 — showing it

### Phase 9 — it has to feel like the game

The port draws a track, loads a 125 off the disc, runs the ported physics and poses a rider. Ridden, it
does not feel like MX Unleashed yet: the keyboard barely registers, the bike will not stand still, and
nothing on the bike itself moves. Everything here is measured in the shell with a person at the controls
or with `--trace`, and every one of them is something you can feel in the first ten seconds of riding.

Two rules carried from the phases above: a number that reaches the simulation is still proved against the
original (oracle or capture), and a number that only reaches the picture matches by eye. These tasks are
ordered by how much of the ride they fix, not by size.

### Stretch

Worth doing, not worth blocking on. Nothing here is picked up by an unattended run.

- [ ] **K1** A `./kit probe` recipe per kind of value you have had to hunt for (a counter, a score, a position, a flag) — written down as recipes rather than rediscovered → docs/PROVING.md | t: none | j: k1-probe-recipes
- [ ] **A4** A fuzzer over the replay harness: random input fixtures, looking for a tick where the port and a recording diverge in a way the handwritten fixtures never hit → crates/game-sim/tests/fuzz.rs | t: crates/game-sim/tests/fuzz.rs | j: a4-fuzz
- [ ] **S9** The rest of the game modes from R4 (menus, loading, results): the front end does not have to match the original, so design it as you like — but the sim-facing parts of each mode (what it seeds, what it carries between matches) do → crates/game-sim/src/mode.rs | t: crates/game-sim/tests/mode.rs | j: s9-modes — stays stretch by decision: freeroam boots through StartMode=FreeRide, which skips the front end entirely, so nothing on the critical path waits on a menu
- [ ] **V3** Widescreen and high resolution as a layer on top: with it off, the game must draw exactly as the original did → render crate | t: none (by eye) | j: v3-widescreen
- [ ] **S12** Opponent AI and the race activities: the AI riders, the start gate, lap counting and results — the `Supercross`/`Nationals`/`Ghost` activity classes from R4. Deliberately out of the freeroam path: `NumberOfRacers=1` with `AllAI` off keeps every one of them out of a freeride world, so none of this is needed to ride. Do not start it until P1 rides → crates/game-sim/src/ai.rs | t: crates/game-sim/tests/ai.rs | j: s12-opponent-ai — nothing here starts automatically: the runner skips the whole Stretch section (ps2kit/runner.py), so AI is only picked up when a human asks for it
- [ ] **S13** Supercross and the race activities: the `Supercross`/`Nationals` activity class (`activity_supercross` in research/oracle.txt), the start gate and its drop, lap and checkpoint counting, the results the race hands back, and the `Tracks\sx01.res` track data. Freeroam does not touch any of it — a freeride world is created with `NumberOfRacers=1` and `AllAI` off — so this waits until P1 rides. Nothing here starts automatically: the runner skips the whole Stretch section (ps2kit/runner.py), so a human picks this up deliberately or not at all → crates/game-sim/src/race.rs | t: crates/game-sim/tests/race.rs | j: s13-supercross
- [ ] **K2** Port the kits merge fixer down into ps2kit/runner.py, replacing the long-lived tmux master. Upstream in ps2-decomp-kit: linux 50c040f (merge df47d27), windows 73b6e08 (merge a9dec98). `fix_merge()` is one `claude -p` per failed merge, synchronous, under the MERGE lock the runner already holds, prompted with the conflicted files, the branch and the last 12 lines of notes/master.md. That deletes everything here that exists only to keep a pane alive: OUTBOX, LOCK, MARK, master_idle/master_busy, the 90-minute stale lock, the starvation bypass, pane adoption, and `enter()` — whose bracketed-paste-and-count-the-Enters delivery is the fragile part of the current design and the reason the one-shot is worth having. It also works with no tmux, which today degrades to LEFT FOR A HUMAN. New config: `[run] fix_merges` (default true), `fix_timeout` (1200s, then the merge is left for a human), `fix_model` (empty = KIT_MODEL or opus), and `[git] push_merge`; `fix_cmd()` sits beside `agent_cmd()` and honours KIT_AGENT and KIT_LEAN the same way. **Set `push_merge = false`** — it defaults true upstream, and openmx deliberately does not publish the shared branch (AGENT.md, PLAN.md and the master prompt all say so; 008f58e). kit.toml is untracked here, so that key is a hand edit to the local file, and not while a run is going. The fixer does not run the tests: the runner runs the gate itself once the fixer commits. Verify with upstreams `test_runner_fixer` in tests/test_plan.py, which drives a real two-sided conflict through a stub agent, plus ./kit check → ps2kit/runner.py | t: tests/test_plan.py | j: k2-port-merge-fixer — two corrections found while checking repos-c3s claim that the fixer is unreachable on the sequential path. It is unreachable here too, and for the same reason: openmx `sequential()` runs in the main checkout on the current branch, commits straight onto it, and never calls `merge()` or `finish()`, so there is no conflict for a fixer to reach and the port needs no `has_master`-style flag. Which means this task line was wrong to say the port buys a no-tmux path: there is nothing to degrade from. Second, `finish()`s `has_master=False` branch (the LEFT FOR A HUMAN log) has no caller — both call sites are on the parallel path and pass four args — so it is dead code; the docstring that described it is fixed, and the port should drop the parameter rather than carry it down
- [ ] **V6** the dirt and the sky: the `.trn` ground texture's tile indexing and per-quad UVs — V1 left the 1 MiB block's layout unresolved and repeats one tile every 24 feet — plus the finer detail texture the real dirt carries, and the scene's own sky cube instead of the fog colour -> crates/game-render/src/terrain.rs | t: none (by eye, against V2's side-by-side) | j: v6-dirt-and-sky
- [ ] **K3** the (after X) gate reads only single-id dependencies: the runner's regex wants one run of non-space characters before the closing bracket, so a line gated on a list — (after S17, S18, S19) — matches nothing and the task starts whenever its files are free, which is how the shell rewire began an hour before the controllers it needs were in the branch; make the gate read every id in the list, and say in the module docstring which forms it accepts
