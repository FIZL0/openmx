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
`A` acceptance harness · `I` input · `S` simulation · `V` rendering · `K` the kit itself ·
`P` the playable shell and how it rides.

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

### Phase 8 — showing it

### Phase 9 — it has to feel like the game

The port draws a track, loads a 125 off the disc, runs the ported physics and poses a rider. Ridden, it
does not feel like MX Unleashed yet: the keyboard barely registers, the bike will not stand still, and
nothing on the bike itself moves. Everything here is measured in the shell with a person at the controls
or with `--trace`, and every one of them is something you can feel in the first ten seconds of riding.

Two rules carried from the phases above: a number that reaches the simulation is still proved against the
original (oracle or capture), and a number that only reaches the picture matches by eye. These tasks are
ordered by how much of the ride they fix, not by size.

### Phase 10 — drivable

Phase 9 made the bike stand still and the keyboard register. Ridden now, it still cannot be ridden:
open the throttle from the start line and the bike loops out and wrecks in six tenths of a second, and
the rider drawn on it stands bolt upright with its hands off the bars. Both are measured below, in the
shell, with the numbers a reader can reproduce from a `--trace` or a `--shot`.

The same two rules: a number that reaches the simulation is proved against the original, a number that
only reaches the picture matches by eye. Ordered by what stops you riding first.
- [~] **S29** the rider cannot bail. Nothing in the port is named bail, and `air::Wreck` (crates/game-sim/src/air.rs:237) has no voluntary reason: its nine are all things that happen to a rider. Reason 13 has no variant there either, and S26 only knows it by its spin ("the cannonball", three lanes off the game's `rand`), so check first whether a bail is reason 13. Find the original's bail from C6's bail windows: which button the pad columns hold on the tick before the reason goes non-zero, which function turns that press into a wreck and with which reason, whether it is allowed on the ground, in the air or both, and what the rider and Wreckbike do afterwards. Port it into S26's crash and recovery so that a bail is a wreck like the others, then bind it on the shell's keyboard (`keyboard()`, crates/openmx/src/pad.rs:572) and on a pad. Prove it against C6's windows the way S26 proved reason 5 (86 and 82 ticks against the capture's 91 and 24): the press to wreck latency and the length of the recovery. S26's reason-to-throw mapping is the port's own inference (`ride::hit_of`), so say in the journal whether a bail lands on a throw the file names → crates/game-sim/src/air.rs · crates/game-sim/src/ride.rs · crates/openmx/src/pad.rs | t: a_bail_wrecks_on_the_press in crates/game-sim/tests/ride.rs, at C6's latency and recovery length | j: s29-bail — no wait on C6 (user, 2026-10-20): if its capture of recording2 is not in context/capture/acceptance/ yet, take the columns this task needs yourself the same way, and leave C6 the full index BLOCKED: MX Unleashed has no bail to port (research/journal/2026-10-20-s29-bail/1-bail-READY.md). Unblocks when the user either closes S29 as matching the original, or names an invented bail (button, reason, ground/air) to add behind an off-by-default flag — plan/HUMAN.md has the ask
- [ ] **P24** a held turn at speed scrubs off half the speed, which the original's do not. P15: with the throttle pinned and the stick hard over, race125p in the shell comes round at 1.6 to 2.5 rad/s (the original's held-stick turns run 0.6 to 2.5, so the rate is in range) but goes from 76 to 35 ft/s in 1.33 s; the original's 1.1 s turn at 57 ft/s only goes from 61 to 52. P10's journal blames the roll rate the ground pose pins and P4's tire stand-in; find which, and wire it → crates/game-sim/src/mv.rs · crates/game-sim/src/ride.rs | t: crates/game-sim/tests/ride.rs — race125p in a one-second full-lock turn at 55 ft/s keeps 80% of its speed | j: p-a-turn-that-keeps-its-speed

### Phase 11 — the biplane

Freeride lets you swap the bike for a biplane (`STR_SWITCH_BIPLANE`), and the user wants it in the port (2026-10-20):
the controls, the flight, the animations, the model and its textures, behind a switch. The same two rules as the
bike: what reaches the simulation is proved against the original, what only reaches the picture matches by eye. With
the switch off the port rides exactly as it does now.
- [~] **S41** the biplane flies with its hands off and its body still (after S38, S39, I6). `Flight::frame` (crates/game-sim/src/flight.rs) runs only the controls' preforce half: wire the rest into it in the original's order — the tires' preforce, then n = (int)(dt*115)+1 substeps of collision force, autopilot, engine (with the smooth noise ported), aero and the two tire force listeners per wheel, the body integrator pair and the constraint list (S38, S39), then the measure (S38) that writes `attitude` and `air`, and the crash test (S39); replace `Sim::step`'s hands-off `Pad::default()` with I6's mapping of the recorded pad; then raise BIPLANE_AGREES_THROUGH in crates/game-sim/tests/replay.rs (0 since S40: the capture's plane creeps from tick 1) as far as it goes, up to the first crash at tick 693 → crates/game-sim/src/flight.rs · crates/game-sim/src/lib.rs · crates/game-sim/tests/replay.rs | t: the_biplane_acceptance_run | j: the-biplane-flies-under-its-own-power — split from S40 (2026-10-20): S40 made the lane, the fixture and the ratchet BLOCKED: S38 (body, world step, measure) and S39 (tires, collision force, crash test) are not merged — both are in progress in slots s1 and s3, editing biplane.rs. Unblocks when both land: then fill Flight::frame in the original's order, hold a SmoothNoise seeded from the save state's tables in biplane_start, and raise BIPLANE_AGREES_THROUGH (journal 2026-10-20-s41 Next)
- [ ] **S45** the biplane's broad phase. Port collision_search_gather (its re-sort gather_resort, the per-candidate distance collidable_distance) and the world's collidable tree it walks (world_tree_query_init/begin/next/end), filling world_search::Nearby (S44) the way the plane's record+0x10c search is filled: the refresh test against the moved centre, the bound it keeps, the capacity (+0x34), type-2 candidates through world_search::accepts with the nearby flag off and the bit 0x08, type-3 placed groups by their model's radius off the world manager. Then probe_collide_world (called with 0 by the contact search when +0x10c is null): the probe's box over the same tree and the second pass over moving objects. Find which call refills the plane's search each frame. Prove both with the oracle on biplane.p2m2_SaveState (the plane's search holds 26 meshes and 8 hierarchies there). (after S44)
- [ ] **S46** the biplane's ray probe narrow phase. The plane's ray probe (record+0x1f0, kind 2 at +0x68, 36 rays) answers world_search::Narrow (S44) through its class vtable (+0x38): +0x90 ray_probe_clear, +0xb0 ray_probe_prepare, +0x94 ray_probe_pair into ray_collide_pair — a tree against a tree (see oracle.txt's S4 block for its descent) — against a mesh (kind 0) and a hierarchy (kind 1), the only kinds in the plane's search. The result (+0x6c: +0x8c hits, +0x90 rays, +0x94 normals, +0x98 points, +0x9c objects) is what contact_search::Search::rays hands back. Prove it with the oracle over generated rays against the save state's own meshes. (after S44)
- [ ] **S47** the biplane's sweep probe narrow phase. The plane's sweep probe (record+0x1f4, kind 3 at +0x68) answers world_search::Narrow (S44): +0x90 sphere_probe_clear, +0xb0 sphere_probe_prepare, +0x94 sphere_probe_pair into sphere_collide_pair on S4's collide::mesh_sweep, against a mesh (kind 0) and a hierarchy (kind 1); its push-out direction (+0x20), point (+0x30) and hit bit are contact_search::Swept. Prove with the oracle on the save state's meshes. (after S44)
- [~] **S48** the biplane's world searches in the ride. Implement contact_search::Search on S44's world_search::search_nearby (and S45's probe_collide_world when the record has no nearby search) with S46's and S47's narrow phases over the level's collidables (the meshes, hierarchies and the other riders' bikes the save state's search holds), the plane's two ray-probe listeners (plane_probe_hit_listener: a struck category in the global mask goes to its owner's vt +0x124; plane_probe_sound_listener only plays sounds, presentation) and the struck bikes' bike_struck_listener, then call contact_search::collide before contact::resolve in Flight::frame's substeps and raise BIPLANE_AGREES_THROUGH as far as it goes. (after S41, S45, S46, S47) BLOCKED: needs S41 (Flight::frame's substeps), S45, S46 and S47 merged to main; re-run S48 once all four have landed

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
