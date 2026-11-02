# 1 — flip scores at start (FINAL)

Task S234. 2026-11-01.

## What I did

- **`Score::flip_at_start`** (`crates/game-sim/src/score.rs`) is a port's-own switch, the user's ask,
  set on the app's riders beside P243's lockout (`crates/openmx/src/main.rs`). With it up, a flip is
  put on the line the frame the bike starts turning it over. That frame is the rising edge of
  `Air::flipping`, which `Sense::flipping` carries in as `score::BACKFLIP` or `score::FRONTFLIP`. The
  flip's worth is what the watch would have paid: 10 000 for the backflip, the pack's front-flip
  points for the front flip, times the combo's length.
  - **Once per rotation.** `Score::shown` keeps the flip that was paid at its start. When that
    flip's watch goes from 2 to 3, the watch goes back to 0 and pays nothing.
  - **A trick still growing keeps growing.** The flip node goes in under the head, not on top of it,
    so the trick is not finished early.
  - **Landing and crashing.** A landing banks the flip whether or not the watch ever came round. A
    crash loses it with the rest of the combo, and with the lockout, a crash within a second of the
    landing loses it too.
  - **A rotation the bike makes on its own still scores on the original's watch.** That is a
    tumble with no flip gesture, so `flipping` never goes up.
  - **With the switch down, every call is the original's.** `turning` is only tracked while the
    switch is up, so original rides hash and replay exactly as before.
- **`Rider::frame`** reports the flip as `Frame::started` on the frame it goes on the line. The app
  prints that as `trick: BackFlip` or `trick: Frontflip`.
- **`ride_hud`'s dead `BackFlip + ` prefix** is not shown for a flip that is already on the line.
- **Pins moved.** `Sim` is 11984 bytes (`tests/hash.rs`). The riders' wire is 13619 bytes with a new
  digest (`tests/platforms.rs`): three bytes per card, four riders. `tests/score.rs`'s capture sense
  gained `..Sense::default()`.

## What the original does

- **The original never scores a flip at its start.** In the rider's tick (`./kit fn 0x27c890`),
  the trick-start hook is called with 0x18, the backflip, in only two places, and both need the
  watch word `+0x1400 == 3`:
  - on the landing branch;
  - in the air, when nothing is scoring.
- **The watch.** It goes to 1 when the up axis is below level with the nose up, to 2 when the bike
  is upside down with the nose down, and to 3 when it is the right way up again with the forward
  axis no lower than −0.79. S79 ported this bit-exact.
- **The HUD listener.** The original tells it (vtable +0x94, event 6) when the watch moves. That is
  the HUD's `BackFlip + <trick>` prefix while a trick runs inside a flip, and it shows no points.
- **C8's film agrees.** `context/capture/c8/run2.mp4`, frames sampled at 2 per second from 3 s on,
  shows no popup while the bike is upside down. "BackFlip 10000" appears only once the bike is
  round.
- **Why a landed flip can score nothing.** The watch only reaches 3 while airborne, and the
  landing resets a watch still at 2. So a flip that touches down more than ~52° nose down, or on the
  very tick it would have come round, never scores, even though it went all the way round. That is
  the user's "it waits for a specific point".

So the brief's "port that" and the user's ask differ: the original waits for the rotation. I went
with the user's ask, which the brief's test line also asks for, and put it behind a switch. The
original's behaviour stays the default, and `tests/score.rs` still proves it.

## How it was proved

- **New `crates/game-sim/tests/flip_points.rs`, 7 tests, no data needed.**
  - **The card, driven by a synthetic pitch sweep at 10° a tick:**
    - the flip is on the line the first tick `flipping` is up;
    - a flip landed at 300° (60° nose down) banks 10 000 with the switch, and the original's card
      banks 0 on the same input;
    - a full 360 banks 10 000 exactly once, and the original also pays 10 000 on its watch;
    - a double flip pays 10 000 + 2 × 10 000;
    - a crash loses the flip;
    - a running trick keeps growing with the flip under it;
    - the front flip shows at its start, once, with the pack's points.
  - **A whole ride (`Sim`):** the stand-in bike, flat out off a ramp onto ground 60 ft down, with the
    backflip gesture on the lip.
    - The node is on the line on tick 121, the first tick `Air::flipping` is up, and `Frame::started`
      says BackFlip that tick.
    - The bike turns ~510° and lands clean, and the total is 10 000.
    - With the switch down, the card has nothing on that tick.
- **Existing suites, all pass:** `cargo test --release -p game-sim --test score --test hash --test
  snapshot --test crash_lockout --test platforms --test trick --test event --test replay`, and `-p
  openmx --test ride_hud`. That includes S79's function oracle and the recording2 capture, so the
  switch-down path is unchanged.
- The full gate is run by `agent-runner done`.

## Not verified / not 1:1

- **Not 1:1, on purpose.** The original scores a flip when it comes round, not when it starts. The
  app turns the switch on; any ride without it is the original's.
- **A flip started and landed short with no wreck** keeps its points. The original would not pay
  it. With the disc's wreck angles, a flip that is not round lands as a wreck, which loses it.
- **Not compared against a PCSX2 capture tick for tick,** because the change is not the original's
  behaviour. C8's film was only used to see when the original shows the flip.
- **Not seen in the app by eye.** The HUD chain reads the score nodes, so the flip's name should pop
  up at the start, but I did not ride it in the shell.
- **The ridden test's flip turns ~510°.** It uses the test's own air numbers, and its wreck limits
  are past a full turn. It shows the start and the bank, not a tidy 360.

## Next

Done.
