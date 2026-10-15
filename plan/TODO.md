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

- [ ] **F5** (after F3) The matrix and quaternion helpers the game uses for bones and cameras, built on F1–F3 with the same op order as the original (the order changes the last bits) → crates/game-sim/src/math.rs | t: crates/game-sim/tests/math.rs | j: f5-matrix

### Phase 5 — the first proven function, and the harness

### Phase 6 — input, the game's own way


### Phase 7 — one subsystem, end to end

Pick the subsystem this game is *about* (the thing the player does every second) and take it all the way:
ported, oracle-checked, replay-proved. One finished subsystem teaches more about the codebase than five
half-ported ones, and it is the first time the port is worth running.

- [ ] **S2** (after S1) Actor lifetime: how actors are created, freed and iterated (order matters: it decides RNG draw order and collision resolution order) → crates/game-sim/src/actor.rs | t: crates/game-sim/tests/actor.rs | j: s2-actors
- [ ] **S3** (after S2) Movement: the player's (or main actor's) acceleration, friction, maximum speed and turning, in the original's arithmetic and its update order. Prove each piece with the oracle, then the whole with a short replay fixture → crates/game-sim/src/move.rs | t: crates/game-sim/tests/move.rs | j: s3-movement — the dirt bikes ground physics from its `bike\*.phy` set: throttle, brake, lean and steering, with the suspension and wheel contact that feed S10. The freeroam critical path runs through here
- [ ] **S4** (after S1) Collision against the world: the original's shape test and its sweep, including how it resolves a hit and how many iterations it allows. Port the test it uses, not a better one — a more accurate collision is a different game → crates/game-sim/src/collide.rs | t: crates/game-sim/tests/collide.rs | j: s4-collision
- [ ] **S5** (after S1) The state machine of the main actor (idle, move, the game's own actions): states, transitions, the frame counts, and which transitions can happen on the same frame → crates/game-sim/src/state.rs | t: crates/game-sim/tests/state.rs | j: s5-states
- [ ] **S6** (after S5) Animation driving: which motion plays per state, its frame advance and blending, to the frame. Looks are flexible, timing is not: a motion that is one frame out moves the action frames with it → crates/game-sim/src/motion.rs | t: crates/game-sim/tests/motion.rs | j: s6-motion
- [ ] **S7** (after S1) The camera for normal play: its target, its lag and its limits. It does not have to be bit-exact unless it feeds the simulation — check whether it does before deciding → crates/game-sim/src/camera.rs | t: crates/game-sim/tests/camera.rs | j: s7-camera
- [~] **S8** (after A2) The acceptance run: a full recorded sequence of real play (several minutes, from a human recording) replays tick-exact against the port. This is the point where the port stops being a collection of functions. Expect it to fail first on the smallest thing — that is what A1's field-level diff is for → crates/game-sim/tests/replay.rs · context/fixtures/ | t: crates/game-sim/tests/replay.rs | j: s8-acceptance — retargeted at freeroam: the acceptance run is a freeride recording — ride around, jump, trick, land, crash — not a race. S8 stays the moment the port stops being a collection of functions BLOCKED: no recording of real play exists to replay: plan/HUMAN.md 'One longer recording of free riding' and slot 4 are both open, and nothing under context/ here or in the main checkout holds one — and the port has no bike to replay it against (S1-S5, S10, S11 open; Sim is still A1's example ball, so today's verdict is NoSuchField bike.pos[0] at tick 0). Unblocked by R7 (the FreeRide boot path makes the recording repeatable from a save state: watch/acceptance.toml --state 4 -n 9000 is then the whole job) or by the human item; and before the port's tick is written, settle whether the original's measured dt (three values in 300 frames) makes a fixed-step tick-exact replay possible at all — see the dt paragraph in research/journal/2026-10-15-s8-s8-acceptance/1-s8-acceptance-READY.md.

### Phase 8 — showing it

- [ ] **V1** (after D5) Load the game's models and textures well enough to draw one actor and the world it stands in, from the user's disc at runtime. Rendering only has to look right, so stop at "recognisably the same scene" and move on → crates/game-sim/src/ (new render crate) | t: a screenshot comparison by eye against `./kit shot` | j: v1-first-draw
- [ ] **P1** (after V1) The playable shell: a binary that opens a window, reads a real pad through I1s mapping, runs the ported simulation at the games own 60Hz and draws it with V1s renderer, dropping you into the freeride area from D6. No emulator, no fixture, no menu — press start and ride. This is the one task that turns a pile of proved functions into something a person can play, and it is the measure of priority 1 → crates/openmx/src/main.rs (new bin crate) | t: `cargo run -p openmx` rides for 60 s with no panic and `./kit check` stays green | j: p1-playable-shell
- [ ] **V2** (after V1) Side-by-side screenshots: the same moment in the original and in the port, from the same save state, in `context/shots/`, with the differences listed. Do this once a subsystem lands, not once at the end → context/shots/ | t: none (by eye) | j: v2-side-by-side

### Stretch

Worth doing, not worth blocking on. Nothing here is picked up by an unattended run.

- [ ] **K1** A `./kit probe` recipe per kind of value you have had to hunt for (a counter, a score, a position, a flag) — written down as recipes rather than rediscovered → docs/PROVING.md | t: none | j: k1-probe-recipes
- [ ] **A4** A fuzzer over the replay harness: random input fixtures, looking for a tick where the port and a recording diverge in a way the handwritten fixtures never hit → crates/game-sim/tests/fuzz.rs | t: crates/game-sim/tests/fuzz.rs | j: a4-fuzz
- [ ] **S9** The rest of the game modes from R4 (menus, loading, results): the front end does not have to match the original, so design it as you like — but the sim-facing parts of each mode (what it seeds, what it carries between matches) do → crates/game-sim/src/mode.rs | t: crates/game-sim/tests/mode.rs | j: s9-modes — stays stretch by decision: freeroam boots through StartMode=FreeRide, which skips the front end entirely, so nothing on the critical path waits on a menu
- [ ] **V3** Widescreen and high resolution as a layer on top: with it off, the game must draw exactly as the original did → render crate | t: none (by eye) | j: v3-widescreen
- [ ] **S12** Opponent AI and the race activities: the AI riders, the start gate, lap counting and results — the `Supercross`/`Nationals`/`Ghost` activity classes from R4. Deliberately out of the freeroam path: `NumberOfRacers=1` with `AllAI` off keeps every one of them out of a freeride world, so none of this is needed to ride. Do not start it until P1 rides → crates/game-sim/src/ai.rs | t: crates/game-sim/tests/ai.rs | j: s12-opponent-ai — nothing here starts automatically: the runner skips the whole Stretch section (ps2kit/runner.py), so AI is only picked up when a human asks for it
- [ ] **S13** Supercross and the race activities: the `Supercross`/`Nationals` activity class (0x002f9b70), the start gate and its drop, lap and checkpoint counting, the results the race hands back, and the `Tracks\sx01.res` track data. Freeroam does not touch any of it — a freeride world is created with `NumberOfRacers=1` and `AllAI` off — so this waits until P1 rides. Nothing here starts automatically: the runner skips the whole Stretch section (ps2kit/runner.py), so a human picks this up deliberately or not at all → crates/game-sim/src/race.rs | t: crates/game-sim/tests/race.rs | j: s13-supercross
