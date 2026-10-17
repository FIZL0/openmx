# 1 — v19 a wreck that settles (FINAL)

Task V19. 2026-10-17.

## What I did

`rider_anim::Body::settled` had no writer. The wreck arm of the body's selection reads it to decide
whether a body off the bike flails or holds a limb, and S27 found the only thing in the original that
writes it — the body actor's *other* vtable slot, its **mode machine** — and deferred it (V18's entry
named it: `./kit fn 0x0024c7e0`). So a wrecked body in the port flailed for ever. This task ports that
slot, as far as the port has fields for it.

- **`rider_anim::Body::mode`, a second entry point.** `Mode` (five values), `Thrown` (what the wreck in
  progress tells it), `Machine` (the state behind it, on `Body` as `machine`), and three methods:
  `mode` (the chain and the entries), `enter_wreck` (the entry's flags and its five draws) and
  `wreck_frame` (the clocks, the roll and the settle test). It is a separate call and not a step of
  `Body::frame` because that is what it is in the original: two slots of one actor's vtable, run from
  the actor's update. Every existing caller's signature is untouched.
- **The ride answers it.** `Rider::body_frame` builds a `Thrown` off `ride::Crash` — the throw the
  wreck's reason picked, whether the rider point has reached the dirt, its velocity and its height —
  reads the rider's own `[Wrecks]` off the loaded wreck files, and calls `body.mode(..)` before the
  selection. A rider with no wreck files under them is handed `None` and never settles, which is the
  same rider S26 left lying there until the hand chord.
- **Three stand-in sensors, and they are the one inexact thing here.** The original's settle test reads
  a ragdoll: a bone array whose every bone has a contact flag, and a main bone with a height and a
  velocity. This port's wrecked rider is a single point, so `touching` is that point having landed, and
  the velocity and the height are its own. The *test* run on them is the original's, instruction for
  instruction, including where the square root is taken.
- **Nothing moved in the hash.** `machine` is on the body, which is deliberately outside `State::walk`,
  so every replay hash is unchanged (leaf count still 750) and only the size tripwire in
  `crates/game-sim/tests/hash.rs` had to be told: 6608 → 6648.

## What the original does

`./kit fn 0x0024c7e0` — 3592 bytes, `dt` in `f12` and the actor in `a0`, and `./kit xref` finds one
caller, the body actor's update (`./kit fn 0x0024e640`). Most of it is renderer work this port has no
answer for; what this task ported is its decision:

- **Five modes**, into `+0x47c`, with the previous one in `+0x480`: the request's own placement flag
  (`+0xc`) means someone else is placing the body; a wreck reason in the request (`+0x104`) means it is
  off the bike; the step's `from_table`/`holding` (`+0x680`/`+0x684`, and a backflip in the request
  raises the second) mean a pose off the clip table is in charge; otherwise riding, unless the
  start-line flag is up or a look-around idle is playing, which is the gate.
- **The entries**, as a chain of `else if`s with a quirk in it: the arm that cleans a wreck up is the
  chain's last `else`, so a body leaving the wreck for the held or gate modes is cleaned up by neither
  — those two match first — and one leaving it for riding or a pose is cleaned up *instead* of running
  that mode's own entry. The port keeps the chain, and the quirk, as written.
- **The wreck's entry** (`./kit fn 0x0024ad90`) draws five numbers out of the throw the reason picked —
  the spin's three lanes, then `ForceTime ± ForceTimeDelta`, then `HandHoldTime ± HandHoldTimeDelta`,
  each `base + delta * (2u - 1)` — in that order, off the generator the whole program shares.
- **The wreck's frame** runs the two clocks down, accumulates the roll while any bone is touching and
  throws it away when none is, and then the test: *not still being thrown about* (the roll is running,
  or either clock is, or it was flung, or the rise across the ragdoll's step is no more than
  `RiderFlailHeight`), and then *landed rather than just flung*, and then the two limits off the rider
  file — the roll has lasted `RiderRollTime`, **or** the speed has dropped below `RiderRollSpeed`. Either
  one, once, and `+0x6b4` goes up and the bones take the stiffer of the two damping records stored
  beside them. The rider's whole `[Wrecks]` block lives on this actor at `+0x700`, with
  `RiderFlailHeight`, `RiderRollTime` and `RiderRollSpeed` at `+0x8d0`, `+0x8d4` and `+0x8d8` — which is
  exactly `game_sim::phy::Wrecks`. The field map, the mode table and the settle test are written down in
  `research/oracle.txt`'s V19 block, with the ten callees named so a test can hook them.

## How it was proved

- **The wreck arm, frame by frame against the original.** New test
  `the_wreck_arm_of_the_mode_machine_matches_frame_by_frame` in `crates/game-sim/tests/rider_anim.rs`
  (through `./kit check`): the real function, with its ten callees hooked away so that only the mode
  chain and the wreck's clocks, roll and settle test run, over **6000 generated frames** — the step, the
  three clocks, both flags, the contact flag, the rise and the three limits all varied, and the limits
  sometimes negative so that the "flung off the ground again" arm is reachable from a test. The
  ragdoll's hook is not a no-op: it moves the main bone by the rise the case asks for, which is how a
  condition the original measures across its own step is driven from outside. Compared per frame: the
  mode, the previous mode, the settle flag, the flung flag, the contact flag, all three clocks, the
  height the next frame measures its rise against, and which of the two damping sets the frame asked
  for — the last off a recording hook, because in the original that is inside the bones and not a
  field. All 6000 matched: **997 settled, 220 flung, 2981 rolling, 1443 already at rest**.
- **A wreck that settles, through a whole rider.** New test
  `a_wrecked_body_settles_and_then_holds_still`: a `Rider` with wreck files under it, dropped from 30 ft
  onto flat ground at the human recording's own `dt` (0.03333), with nobody touching the pad and
  nothing but the mode machine writing `settled`. It wrecks on the vertical limit (reason 5) at tick 46,
  the thrown rider reaches the dirt at 69, and at **91** — 22 ticks later, which is `RiderRollTime`'s 23
  frames of roll less the one the landing tick itself contributed — the body settles with nothing having
  written the flag, grabs state 43 (one of the five held limbs) and keeps it for every later wrecked
  frame. The machine puts the rider back at 130, 84 ticks after the wreck, which is the number S26's own
  fixture measures against the capture's 91 — so the body's settle lands inside the recovery the
  recording holds. After the respawn the flag, the flung flag and the roll are all clear again, by the
  chain's own leaving-the-wreck arm.
- **The whole suite.** `./kit check` → TESTS PASSED (quoting the verdict, not the count), and
  `./kit audit` clean.

## Not verified / not 1:1

- **The three sensors are stand-ins, and here is what each stands in for.** `touching` is
  `ride::Crash::landed` — the rider point having reached the ground — where the original scans the
  ragdoll's bone array for *any* bone whose contact flag is set. `vel` and `height` are that same point's,
  where the original reads the main bone's. So a port body "lands" once and stays landed, while a real
  ragdoll can bounce a limb clear and reset its own roll; and the speed the limits are tested against is
  one point's and not a bone's.
- **The rise is measured across the frame, not across the ragdoll step.** The original takes the main
  bone's height either side of its own ragdoll step, inside one call. This port's rider point is stepped
  by `Crash::step`, a call earlier, so `Machine::height` remembers last frame's height and the rise the
  test reads is what that step did. The oracle proof hands both sides the same rise, so what is proved is
  the *test*; that the two rises are the same number on a live wreck is not proved.
- **The whole renderer half of the function is not ported**, and nothing below reads a port field, so
  none of it is proved either way: the head and look dummies and their two eased weights
  (`+0x5d8`/`+0x5dc`/`+0x618`/`+0x61c`, `+0x638`/`+0x63c`), the thrown body's orientation quaternion
  (`+0x6d0` into `+0x6e0`, and the two flags `+0x6a8`/`+0x6ac` that choose which frame it is taken in),
  the bike-collision detachment (`+0x6b0` and the per-bone `./kit fn 0x0011aed0`), the ragdoll's own step
  (`./kit fn 0x001dc960`), the per-bone damping records and the bone loop inside
  `./kit fn 0x0024c310`, the two other modes' entries (`./kit fn 0x0024d5f0`, `./kit fn 0x0024c490`),
  the placement-flag block (`+0x2a8`/`+0x2ac`/`+0x49c`/`+0x494`/`+0x498` and the scene's own mode), the
  `+0x644`/`+0x648` clock, and the IK tail that copies fourteen words of the request into the actor.
- **`Machine::places_itself`, `spin` and `stiff` are kept and read by nobody.** They are the fields the
  original writes at the points the port transcribes, kept so that the draws stay in order
  (`spin` costs three of the entry's five) and so that the arms that switch the damping sets are not
  no-ops. `places_itself` in particular is *not* wired into the step's placement: the port tells the
  step `true` either way.
- **The order of the two vtable slots is the port's choice.** `Rider::body_frame` runs the mode machine
  and then the selection, because the mode machine is what writes the flag the selection reads. Which
  slot the original's actor update calls first is not established — `./kit fn 0x0024e640` calls the mode
  machine at `0x0024e7fc`, and the selection is reached through a different slot of the same vtable.
- **The entry's five draws are in the original's order, but their *values* are not compared.** The
  oracle proof drives frames of the wreck and never its entry (the entry is hooked), so what is proved
  about `enter_wreck` is that it is a transcription, not that it produces the same five numbers as the
  original off the same generator state.
- **`RiderRollSpeed` is compared in feet per second.** The port's loader scales that key by the same
  mph→ft/s factor as the rest of the file; the oracle proof sets the field directly on both sides, so
  the scaling is inherited from S26 and not re-proved here.
- **Nothing here was compared against the live game.** No PINE recording, no save state: the original's
  own code is the oracle, and the behavioural test's tick counts are the port's own. The one number that
  touches the human recording is the recovery's 84 ticks, which S26 already measured against its 91.
- **The names this proof calls the original by are in a committed file, its `[symbols]` twins are not.** The test resolves
  `rider_body_mode` and its callees out of `research/oracle.txt` (committed, V19 block), so it runs
  anywhere the disc is; the `[symbols]` entries added to this worktree's `kit.toml` for `./kit fn
  @name` are not committed and want reconciling after the run.

## Next

Nothing in this task is half-done. What it leaves on the table: the ragdoll itself (a wrecked rider is
one point here, which is what makes the three sensors stand-ins), and the request block at `+0x488`,
whose writer is still unfound — V18 left that one too.
