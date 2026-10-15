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

- [ ] **R7** The FreeRide boot path: StartMode `FreeRide` sets app+0x408=2 and event+4=0 and goes straight into a loaded world with no front end at all (R4), so drive the original into a rideable freeride area from a cold boot by writing those two words — no menus, no human save state. Name the freeride track `.res` files, the `HitMaker`/`HitFreeride` activity class R4 left unnamed (0x00330460), and the `NumberOfRacers`/`AllAI` knobs that keep opponents out. This is what makes every later gameplay capture repeatable without a person playing → kit.toml `[symbols]` · research/oracle.txt | t: a scripted boot reaches a freeride area and `./kit shot` shows the bike on the ground | j: r7-freeride-boot
- [ ] **D6** The freeride area on the disc: one freeride track from the `.res` list R7 names, read with D5s archive reader — the ground geometry, the collision shapes the sim actually tests against, the spawn point, and which `bike\*.phy` parameter set freeride defaults to. S4 cannot be proved against anything until the real collision data parses → crates/game-sim/src/track.rs | t: crates/game-sim/tests/track.rs | j: d6-freeride-track
### Phase 2 — a decompilation you can read


### Phase 3 — the frame, and your first capture

- [ ] **C2** (after C1) The vsync counter: `./kit probe vsync` finds a word in RAM that goes up by one per frame; confirm it stalls when the game pauses and put it in `[emu] vsync_addr`. Every capture and every timing comparison is counted in this one number → kit.toml `[emu]` | t: `./kit pine watch <addr>` shows +1 per frame for 300 frames | j: c1-emulator
- [ ] **C5** (after C1) The pad: `./kit pad` presses a button on the running game and the game reacts. Then find where the game reads the pad in RAM (the bitmask per pad, and whether it keeps "pressed this frame" separately) → kit.toml `[symbols]` · docs/PCSX2.md | t: a scripted button press changes the menu selection in a capture | j: c5-pad

### Phase 4 — the arithmetic, before any ported math

- [ ] **F3** VU0 macro mode: the ops the game actually uses (`vmul`, `vadd`, `vmadd`, `vmula`, `vdiv`, `vsqrt`, `vrsqrt`, the clipping and min/max ones), with VU0's own rounding and its flag behaviour. Take the list from the disassembly rather than implementing the whole ISA → crates/ps2-float/src/vu.rs | t: crates/ps2-float/tests/vu.rs | j: f3-vu0
- [ ] **F4** (after O1) The game's own transcendentals: whatever `sinf`, `cosf`, `atan2f`, `sqrtf` and `powf` the disc links. They are rarely the standard library's, and a port that calls the host's `sin` will differ in the last bits of every angle. Port the disc's implementations and check them against the oracle over the full input range → crates/ps2-float/src/libm.rs (new) · research/oracle.txt | t: crates/ps2-float/tests/libm.rs | j: f4-libm
- [ ] **F5** (after F3) The matrix and quaternion helpers the game uses for bones and cameras, built on F1–F3 with the same op order as the original (the order changes the last bits) → crates/game-sim/src/math.rs | t: crates/game-sim/tests/math.rs | j: f5-matrix

### Phase 5 — the first proven function, and the harness

- [ ] **O1** (after R1) The oracle, against one trivial function: `./kit oracle` interprets the real code from a RAM dump and calls it with your arguments. Prove it on something with an obvious answer first (a clamp, a distance, a table lookup) so the interpreter is a tool rather than a suspect. Add any unknown opcode it panics on → crates/ps2-oracle/src/ · research/oracle.txt | t: crates/ps2-oracle/tests/basics.rs | j: o1-oracle
- [ ] **O2** (after O1) Oracle coverage for the arithmetic of F1–F4: run each ported op against the real code over ≥10k generated inputs, including the awkward ones (zero, denormals, the largest magnitudes, equal exponents, exponent differences of 24 and over) → crates/ps2-float/tests/ · research/oracle.txt | t: crates/ps2-float/tests/oracle.rs | j: o2-float-oracle
- [ ] **A2** (after A1) The replay harness: a fixture of recorded inputs plus the initial state, run through the simulation, compared tick by tick against the captured hashes or watch values. Make a one-line failure (`tick 413: player.vel.x 1.5 vs 1.500001`), not a diff of two files → crates/game-sim/src/replay.rs · crates/game-sim/tests/replay.rs | t: crates/game-sim/tests/replay.rs | j: a2-replay-harness

### Phase 6 — input, the game's own way

- [ ] **I1** (after C5) The input layer: pads mapped to the game's own bitmask, with "held", "pressed this frame" and "released" exactly as the original keeps them (including the frame it latches them, which decides whether a press on the frame of a state change counts) → crates/game-sim/src/input.rs | t: crates/game-sim/tests/input.rs | j: i1-input
- [ ] **I2** (after I1) Analogue sticks: the original's dead zone, clamping and any curve, in its arithmetic. An approximated stick curve quietly changes every movement in the game → crates/game-sim/src/input.rs | t: crates/game-sim/tests/input.rs (oracle) | j: i1-input
- [ ] **I3** (after I1) Input recording and playback in the port: a fixture of pad frames drives a run, so a human-played sequence from the real game can be replayed against the port (this is what A2's fixtures are made of) → crates/game-sim/src/replay.rs | t: crates/game-sim/tests/replay.rs | j: i3-input-fixtures

### Phase 7 — one subsystem, end to end

Pick the subsystem this game is *about* (the thing the player does every second) and take it all the way:
ported, oracle-checked, replay-proved. One finished subsystem teaches more about the codebase than five
half-ported ones, and it is the first time the port is worth running.

- [ ] **S1** (after R3) The fixed update: the tick order from R3, as the top-level step of the simulation — what updates before what, where input is read, where the RNG is drawn from, what happens on the frames the game is loading or paused → crates/game-sim/src/lib.rs | t: crates/game-sim/tests/tick.rs | j: s1-fixed-update
- [ ] **S2** (after S1) Actor lifetime: how actors are created, freed and iterated (order matters: it decides RNG draw order and collision resolution order) → crates/game-sim/src/actor.rs | t: crates/game-sim/tests/actor.rs | j: s2-actors
- [ ] **S3** (after S2) Movement: the player's (or main actor's) acceleration, friction, maximum speed and turning, in the original's arithmetic and its update order. Prove each piece with the oracle, then the whole with a short replay fixture → crates/game-sim/src/move.rs | t: crates/game-sim/tests/move.rs | j: s3-movement — the dirt bikes ground physics from its `bike\*.phy` set: throttle, brake, lean and steering, with the suspension and wheel contact that feed S10. The freeroam critical path runs through here
- [ ] **S4** (after S1) Collision against the world: the original's shape test and its sweep, including how it resolves a hit and how many iterations it allows. Port the test it uses, not a better one — a more accurate collision is a different game → crates/game-sim/src/collide.rs | t: crates/game-sim/tests/collide.rs | j: s4-collision
- [ ] **S5** (after S1) The state machine of the main actor (idle, move, the game's own actions): states, transitions, the frame counts, and which transitions can happen on the same frame → crates/game-sim/src/state.rs | t: crates/game-sim/tests/state.rs | j: s5-states
- [ ] **S10** The bike in the air: where the sim decides the wheels left the ground, the rotation the stick commands while airborne, the lean and preload carried off the lip, and the landing test — the angle it tolerates, what counts as a clean landing and what throws the rider. Half of how riding around feels is this task, so it is priority-1 work, not polish → crates/game-sim/src/air.rs | t: crates/game-sim/tests/air.rs | j: s10-air
- [ ] **S11** Tricks: the trick table — which stick-and-button gesture starts which trick, the frame windows that decide whether it counts, the rotation each one applies, and what makes a landing a crash instead. Tricks are the point of freeroam, so they are ported and proved like the rest of the simulation, not approximated by eye → crates/game-sim/src/trick.rs | t: crates/game-sim/tests/trick.rs | j: s11-tricks
- [ ] **S6** (after S5) Animation driving: which motion plays per state, its frame advance and blending, to the frame. Looks are flexible, timing is not: a motion that is one frame out moves the action frames with it → crates/game-sim/src/motion.rs | t: crates/game-sim/tests/motion.rs | j: s6-motion
- [ ] **S7** (after S1) The camera for normal play: its target, its lag and its limits. It does not have to be bit-exact unless it feeds the simulation — check whether it does before deciding → crates/game-sim/src/camera.rs | t: crates/game-sim/tests/camera.rs | j: s7-camera
- [ ] **S8** (after A2) The acceptance run: a full recorded sequence of real play (several minutes, from a human recording) replays tick-exact against the port. This is the point where the port stops being a collection of functions. Expect it to fail first on the smallest thing — that is what A1's field-level diff is for → crates/game-sim/tests/replay.rs · context/fixtures/ | t: crates/game-sim/tests/replay.rs | j: s8-acceptance — retargeted at freeroam: the acceptance run is a freeride recording — ride around, jump, trick, land, crash — not a race. S8 stays the moment the port stops being a collection of functions

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
- [ ] **A3** (after A2) Make the hash platform-independent and prove it: the same fixture hashes identically on x86-64 and on aarch64 (and on wasm32 if this port will run in a browser). Transcendentals go through the game's own `libm`, never the host's → crates/game-sim/tests/platforms.rs | t: crates/game-sim/tests/platforms.rs | j: a3-platforms — moved to stretch: a hash that agrees across architectures matters when the port ships, not while one bike is being made to ride
- [ ] **S13** Supercross and the race activities: the `Supercross`/`Nationals` activity class (0x002f9b70), the start gate and its drop, lap and checkpoint counting, the results the race hands back, and the `Tracks\sx01.res` track data. Freeroam does not touch any of it — a freeride world is created with `NumberOfRacers=1` and `AllAI` off — so this waits until P1 rides. Nothing here starts automatically: the runner skips the whole Stretch section (ps2kit/runner.py), so a human picks this up deliberately or not at all → crates/game-sim/src/race.rs | t: crates/game-sim/tests/race.rs | j: s13-supercross
