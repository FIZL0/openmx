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
- [ ] **S38** the biplane's body does not move. Port the rigid body the plane flies on (it is not the bike's chassis_integrate): the body-frame force and moment accumulators (research/oracle.txt body_force_local, body_moment_local), the integrator the world step calls per substep (vt+0x44 → 0x1f2340/0x1f20d0, then vt+0x30 0x1f77c0), gravity, the world step itself (dynamics_world_step: n = (int)(dt*115.0)+1 substeps, preforce listeners once per frame, force listeners per substep, the constraint list at world+0x80 iterated world+0 times — say whether any constraint touches the plane), and the plane's measure listener 0x352960 (airspeed, alpha/beta through the acos table (acos_table_ptr), body-frame velocity and rates, Euler angles (quat_to_euler), the grounded flag from the three wheels) that feeds S34's controls/autopilot/engine/aero their Attitude and Air. Oracle-proved per function on biplane.p2m2_SaveState like S34's tests → crates/game-sim/src/biplane.rs · crates/game-sim/tests/biplane.rs | j: the-biplanes-body — split from S34 (2026-10-20): S34 ported and proved the four force listeners; this is the library under them
- [ ] **S41** the biplane passes through what it hits. Port the collision object's contact resolution that biplane_collision_force calls every substep (research/oracle.txt collision_resolve, vtable slot +0x80 of the plane's collision object at plane+0x170, and its callees), which is where the plane's contacts with the scene, the surface the crash test reads (collision +0x1fc, flags +0x84 against plane+0x580) and the contact pointer (+0x40) come from; S39 ported biplane_collision_force with it handed in as a closure. Find on the way whether anything sets a tire's held contact (+0x84) or reaches water: S39 found neither. Prove it with the oracle like S39's proofs in crates/game-sim/tests/biplane.rs.
- [ ] **S40** a ride with the biplane is not a lane the harness can name. Wire S34's four listeners, S38's body and world step and S39's wheels into one per-tick plane update, make a plane ride an `impl Game` lane (as P5 did for the bike, without moving a bike hash), and replay it against C7's capture: the_biplane_acceptance_run in crates/game-sim/tests/replay.rs, a fixture over Free06 with its own ratchet starting at 0, printed on every `./kit check` the way S8 and S32 do it → crates/game-sim/src/biplane.rs · crates/game-sim/src/ride.rs · crates/game-sim/tests/replay.rs | t: the_biplane_acceptance_run | j: the-biplane-flies-a-ride — split from S34 (2026-10-20); needs C7's capture
- [ ] **I6** the biplane has no controls. Port the game's own mapping from the pad to the plane's eight controls (R8: throttle up/down, rudder left/right, climb, dive, bank left/right — which stick axis, which buttons, analog or digital, and the pressure bytes if it reads them) into the input layer, and check it against C7's capture: the recorded pad, put through the port's mapping, gives the control values the original read, tick for tick. Then bind them in the shell: on a pad the game's own layout, and on the keyboard a set that can fly — `keyboard()` (crates/openmx/src/pad.rs:572) is the bike's, and `KeyAxis`'s ramp (pad.rs:252) was tuned for a bike's stick, so say in the journal whether it suits an elevator and an aileron. The HUD's control list (`--hud`) shows the plane's eight when the plane is up → crates/game-sim/src/input.rs · crates/openmx/src/pad.rs · crates/openmx/src/hud.rs | t: the_biplane_reads_the_pad in crates/game-sim/tests/biplane.rs, against C7's capture | j: i6-biplane-controls — no wait on R8 (user, 2026-10-20): find the addresses and names this task needs in the decompilation yourself, and add them to research/oracle.txt for R8
- [ ] **S35** nothing on the biplane moves. Drive its animation from the flight state the way S25 and S27 drive the bike's and the rider's: the prop's spin from the engine (and the broken prop when it is crashed), the ailerons, elevator and rudder deflected by the controls they come from, and the pilot's clips — `biplane_roll_fwd`, `biplane_roll_bck` and whatever else R8 finds the plane asking for. Clip selection reaches the simulation's state, so prove it against C7's capture. How far a surface visibly swings only reaches the picture, so it matches by eye → crates/game-sim/src/vehicle_anim.rs · crates/game-sim/src/rider_anim.rs · crates/game-sim/src/biplane.rs | t: the_biplane_asks_for_the_originals_clips in crates/game-sim/tests/biplane.rs, against C7's capture | j: s35-the-biplane-moves — no wait on S34 (user, 2026-10-20): if the flight is not ported yet, build on what is there and say in the journal what S34 has to wire in
- [ ] **S36** the biplane cannot crash and come back. Port what R8 finds the original doing when the plane hits the ground, the water or a wall: the crash test's reason, the wreck (does it use `Wreckbike.phy` or something of its own), the broken prop, `Biplanewreckdust`, and the respawn: where, facing which way, on the ground or in the air, and after how long. Prove the timing against any crash in C7's capture the way S26 proved the bike's (the entry tick and the recovery length) → crates/game-sim/src/biplane.rs · crates/game-sim/src/ride.rs | t: the_biplane_crashes_and_respawns in crates/game-sim/tests/biplane.rs | j: s36-the-biplane-crashes — no wait on S34 (user, 2026-10-20): if the flight is not ported yet, build on what is there and say in the journal what S34 has to wire in

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
