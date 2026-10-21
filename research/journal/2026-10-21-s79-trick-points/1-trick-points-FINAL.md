# 1 — trick points (FINAL)

Task S79. 2026-10-21.

## What I did

- Ported the original's score card into `crates/game-sim/src/score.rs`: one `Score` per rider, a
  field of `Rider` (`rider.rs`), so the wire snapshot (`snapshot.rs`) and the state hash carry it.
  The hash only walks it under `score` once it differs from a fresh card. This keeps a fresh state's
  909 leaves; `Sim` grows to 10256 bytes (pinned in `tests/hash.rs`).
- `Tricks::update` (`trick.rs`) now returns a `Clocked`: did the clock restart, which trick started,
  and did it stop. These are the three places the original's stunt clocks call the rider's score
  hooks. The rider feeds them to the card right after the trick update, as the original does.
- Wired the wreck entry, the put-back chord, and the verdict reset to the card's wreck and abort.
  At the end of the rider's frame it runs the scoring section (`Score::frame`) and then the
  manager's tick (`Score::grow`).
- Captured recording2's playback with the card in it (`watch/s79-score.toml`,
  `play_capture.sh` here). The capture is `context/capture/s79/recording2-score.csv`, 1894 rows,
  and is not committed.
- Tests in `crates/game-sim/tests/score.rs`, below.
- The riders' platform vectors (`RIDERS_*` in `tests/platforms.rs`) moved from tick 192 on, where
  the run's first trick scores. They were regenerated with `print_the_riders_vectors` and spliced in.

## What the original does

The card is `*(rider+0xf9c)` and the manager `*(rider+0xfa0)`; oracle.txt's S79 section has every
offset. Read the functions with `./kit fn 0x266e00`, `0x266fb0`, `0x267620`, `0x267820`, `0x267a30`,
`0x267a90`, `0x280b30`, `0x280d00`, `0x280d70`, `0x27b6e0`, and the scoring section of `0x27c890`.

- **The trick-start hook** (rider vtable +0x10c, called from the stunt clocks only for ids < 24, and
  for the backflip by the rider's tick) is gated on `(!front || !rear) && !wreck`. The backflip, 24,
  bypasses the gate. The hook opens a combo if none is open.
  - A trick pushes a node with base 0, rate 2500/s, mult 1, granularity 10, flags 0xc (round down,
    timed), then holds it (growing).
  - The backflip pushes base 10000. Then the combo's points gain the head times the node count,
    and the head is finished at once with mult = node count.
- **The trick-stop hook** (+0x110) finishes the head with mult = node count, ok, and un-holds it.
  The stunt clocks call it on a restart over a running trick, and on every frame past the
  trick's duration.
- **The abort hook** (+0x114): the head is finished not ok (it stops counting), the combo is closed
  not ok, and the node is un-held.
- **Finish**, ok: points × mult, rounded down to the node's granularity, which here is 10. Not ok,
  the node loses its valid bit, and with it its place in every sum.
- **The combo end** sums the valid nodes. Ok, it adds the sum × scale to the total. Either way it
  zeroes the combo's points, rounds the total down to 1, and frees the nodes.
- **The manager's tick** (once a frame, after every rider) does three things:
  - the combo clock += dt;
  - while the head is scoring and growing: held += dt, and points = (base + held × rate) × mult,
    rounded;
  - the combo's points = the sum of the valid nodes.
- **The rider's tick, scoring section.** "Slow" is the previous frame's contact speed (+0x438)
  < 5.0. When not wrecked:
  - **Wheelie** (rear only, not slow): after 1 s, a node with id 25, base 0, rate 1000.
  - **Stoppie** (front only, not slow): the same with id 26.
  - **Both wheels down, or slow and not airborne:** the head is finished, a completed backflip (stage 3)
    scores 24, the combo is **banked** (combo end, ok), and the wheel clocks are zeroed.
  - **In the air with hang time > 0.1:** a running wheelie/stoppie node is finished, and the clocks
    are zeroed.
  - Then a completed flip with nothing scoring scores 24 in the air, and the four-stage flip watch
    runs on the chassis' up-y and forward-y.
- **A wreck.** `jump_record_end` (0x27b6e0) is called from both the wreck entry and the put-back
  request. If a node is scoring, it is finished *not ok* and un-held. An open combo is closed *not
  ok*. Nothing is added to the total, and the combo's points (what a HUD would read) go to 0.
  - This answers the user note of 2026-10-21. Any landing that enters the wreck state counts as a
    crash: the original has no separate "bad landing" test in the scorer, only the rider's wreck.
  - **The whole combo is lost**, every node of it, not just the last trick's. The HUD-facing word
    (+0xe0) is zeroed in the same frame. Nothing keeps the lost points for display.
- **Freeride's mode (3)** in the manager's mode handler scores nothing more.

## How it was proved

- **Function oracle, 10 000 operations**: `the_card_scores_like_the_original`. It runs 20 runs ×
  500 random operations (start, stop, abort, wreck, grow, one-wheel, bank, push, finish, hold,
  close).
  - Each operation runs in the interpreter on the human's own card in
    `first_recording.p2m2_SaveState.eeMemory.bin`, and on the port beside it.
  - Every card field and node is compared bit for bit after each operation. All match, with up to
    12 nodes in a combo.
- **A PINE capture compared row by row**: `recording2s_points`, against recording2's playback
  (1894 rows).
  - The port is driven by the captured wheels, air, hang time, speed, wreck, attitude and trick
    clocks.
  - 1881 rows match the port after their frame, 7 match it between the rider's and the manager's
    tick, 2 before the frame (lag runs), and 3 match neither:
    - 2543 is read inside the trick-start hook, between push and hold;
    - 2595 is read inside the combo end, before the nodes are freed;
    - 2593 is the trick's stop, derived a row early from the trick clock.
  - The banks are asserted by frame and total: (2595, 24160) (2716, 27900) (3417, 55060)
    (3452, 55190) (3869, 60020) (4034, 64350).
  - So are the wrecks: at **3562 a trick landed into a wreck with 3330 points on the card, which
    never reach the total**.
- **No data needed**: `a_trick_landed_into_a_wreck_scores_nothing` rides the same trick twice. The
  clean landing banks it; the landing into a wreck leaves the total and the combo's points at 0.
- `./kit check` passes. The only failure seen was the known UDP port flake.

## Not verified / not 1:1

- **The freestyle time limit and target are not ported.** The brief asks for them; I did not find
  them. The manager's mode handler has event modes, and 3 (freeride, the only one captured) scores
  nothing extra. Mode 5's "timed" node flag (8) and the other modes were not read. Filed as a
  follow-up.
- **The node pool is per rider here.** The original has one pool of 100 nodes shared by all riders,
  and a push with the pool empty does nothing. They differ only when the riders' open nodes together
  pass 100.
- **The networked card branch (card flag 0x1) is not ported.** The original skips its combo start and
  so on for a remote rider's card. Here every rider scores on its own sim.
- **Rider words not ported:** the timer at +0x1408 (never seen set), and the stats beside the card
  (+0x131c..+0x1338, +0x133c: best wheelie and the like). They move no points.
- **The per-trick bonus table at +0x140c is assumed zero.** It was zero for all 26 ids in every state
  read. If anything ever sets it, a trick's rate would differ.
- **Whether the HUD shows the lost points is not checked on screen.** The card's combo word is zeroed
  on the wreck frame, and nothing keeps the lost amount. Whatever the HUD's listeners do (they are
  not ported) is P68's to look at.
- **The capture test's events come from the captured trick columns,** not from the port's own
  `Tricks`. The rider-level wiring (`Clocked` → the hooks, slow from the previous frame, the sense
  built from `ride`) is checked only by the riders' run now moving its hash at its first trick. No
  test compares a whole `Rider` ride's points with the original tick for tick.
- **The automatic put-back after a wreck** relies on the card already being empty (the wreck closed
  it). The chord's put-back calls the wreck path like `ride_reset_request` does; the verdict reset
  calls abort. Both are read from the decompilation, not captured.
- **3 capture rows match no phase** (above). They are reads inside a function or a row early, not
  differences in points.

## Next

Done here. Follow-up filed in the plan: the freestyle event's time limit and target, and its
modes' extra scoring (mode 5's timed nodes). P68 can read `Rider::score` (`total`, `combo_points`,
`nodes`) for the HUD.
