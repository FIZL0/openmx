# 1 — nothing handed on (FINAL)

Task F31b5g. 2026-11-05.

## What I did

`quad::Steps` hands five calls on to `StepsInRam::hand_on` (tests/mxvatv_bike_ride.rs). Each is now
answered in Rust or dropped with a reason.

- **The gravity's record call** (the record's vt `+0x88`, given (record, 1)). This call is the record's
  resolution, which MD75e1 had already ported as `atv_contact::Held::resolve`. What was missing was a
  reader: `quad::Steps` answers the gravity itself when `bike.contacts` holds the record, and
  `steps::read` leaves that field `None`. I added the reader and its writer to
  `handling/record.rs`:
  - `record::held` reads the record's collider, its contact list and the whole body. It answers
    `None` if any contact is with another record or a mover, and the call is then handed on as
    before.
  - `record::held_words` writes back what the resolution changes: the record's normal speed, its
    world centre and its relative velocity; each contact's two arms and its relative velocity; and the
    whole body.

  `StepsInRam::call` now fills `contacts` on every `Call::Gravity`. It writes `held_words` back before
  the listener's view of the body. This happens in every rig that uses `StepsInRam`, so the gravity's
  record call is no longer handed on anywhere (1836 calls on ride-2, 1860 on flip-1). The existing
  F31b5a and F31b5b replays drop it from their handed-on lists. F31b5b's run still checks the whole
  RAM against F31b5a's, so the resolution is bit-exact over the replays too.
- **The stunt start** (vt `+0x4b8`). Ported as `update::stunt_start(m, id, arg)`.
- **The stunt end** (vt `+0x4ac`). Ported as `update::trick_end(m, id)`.
  - Both are on the `Updates` trait, because F45 also needs them for `update::trick`.
  - The player's run calls they make go out as new `Out`s: `RunOpen`, `RunScore`, `RunClose`,
    `RunTally` and `RunEnd`.
  - The run's stunt mark is the existing `Out::Mark`, the same function F42 found.
  - The start's nested trick ends go out as `Out::TrickEnd`.
- **The wreck** (vt `+0x368`). This is `wreck::enter`, run through `WreckInRam`, after asserting that
  the slot is the wreck entry.
- **The preload cue** (`post(owner, bike+0x674, 8, 14, +0xd20, 0)`). Dropped. F31b5f already showed
  that the frame is bit-exact with every event post dropped. The only listener of the bike's that
  writes the bike, the camera controller, ignores id 14. The new replay confirms this (below).
- `StepsInRam` and `PortSteps` have a new `pure` flag, which `posts_frame` threads through. With it
  set, `StepsInRam::in_rust` answers the start, end and wreck in Rust and drops the cue. A small
  `Updates` (`RunDropped`) drops the run calls. Calls answered this way are counted under `"= "`, and
  calls handed on under `"> "` as before.
- Filed **F31b5h**: port the run's scoring calls. Added a note on **F45** that the start and end ports
  exist.
- Nothing taken from opentennis; it has no counterpart.

## What the original does

Addresses and the full pseudo-code are in research/oracle.txt `# F31b5g`. To read them yourself:
`./kit dis -p atv 4e6370 --len 0x1f0` and `./kit dis -p atv 4e4db0 --len 0xa0`.

- Stunt start:
  - The bike's five kept stunts are `+0x13b8..+0x13c8`, and the last two are "late".
  - The start does nothing (and answers 1) when the bike is airborne or held and the id is not a kept
    stunt.
  - Otherwise it sets the next stunt `+0x1528` to the id, unless the id is kept and either the next
    stunt is under 41 or the id is a late one.
  - It zeroes the clip's three clocks and sets `+0x1650` to 1.
  - With a run and a player:
    - For a non-kept stunt, it first ends each of the player's running stunts (vt `+0x4ac`).
    - It opens the run if the player's `+8` bit 5 is clear.
    - It scores the stunt with the table entry's two scores times its scale, each at least 10.
    - It sets the stunt mark.
- Trick end:
  - It writes nothing itself.
  - With a run, and a player whose `+8` bit 5 is set: an id of −1 closes the run, then tallies the
    player's `+0xcc` count as a float. Any other id ends that one stunt.
  - The −1 test is a 64-bit `beq` against a sign-extended −1. A caller has to pass the id the way `lw`
    leaves it, which the oracle does.

## How it was proved

- **Function oracles**, each 10 000 cases on state 4's RAM image (tests/mxvatv_bike_ride.rs):
  - `the_gravitys_record_call_is_the_records_resolution`. The original's `+0x88` against
    `held → resolve → held_words`. Every word of the record, its list (up to the capacity of 14) and
    the body is compared, and the original's writes are checked for anything else off the stack
    (`Ee::written`). Paths: no contacts 1634, list full 1399, some 6967, damped 6217, off any surface
    6734, velocity changed 6360, rates changed 4181.
  - `the_stunt_start_is_the_originals`. The original runs with the run calls and its nested trick end
    hooked to log. The answer, every word of the bike, the player and the stunt table, and the call
    log must match, and nothing else may be written. Paths: not started 3359, kept 5527 (next written
    1636, next kept 3891), not kept 1114, no run or player 2060, running stunts ended 393, list stopped
    short 463, run opened 2378, run already open 2203, a score at the floor of 10: 4050.
  - `the_trick_end_is_the_originals`. The answer and the call log (including `$f12`) must match, and
    neither side may write anything. Paths: no run or player 3035, run not open 3481, closed and
    tallied 1739, one ended 1745.
- **Replays**, `ride_replays_bit_exact_with_nothing_handed_on` and
  `flip_replays_bit_exact_with_nothing_handed_on`. These run C15's ride-2 and flip-1 twice in
  lockstep through `posts_frame`, once handing on and once pure.
  - Every row of the pure run matches C15 word for word, along with the wreck state (flip-1 wrecks on
    row 125 and is put back on row 193).
  - After every frame, the bike's 0x1800 bytes and its physics blocks equal the handing run's.
  - The steps hand nothing on: ride-2 answers nothing in Rust, and flip-1 answers `PreloadCue` once and
    `StartStunt` once.
  - A probe run printed where the two RAMs came apart (listed below). I checked that both tests really
    ran rather than skipping.
- The F31b5a and F31b5b replays (`*_on_quad_steps`, `*_with_rust_callees`) still pass with the
  gravity's record call in Rust.

## Not verified / not 1:1

- **The stunt end and the wreck in the pure frame are never reached.** Neither replay has the steps
  ask for `EndStunt` or `Wreck` (flip-1's wreck comes from the wreck test, not the steps). `trick_end`
  rests on its oracle, and `wreck::enter` rests on its earlier oracles (MD63c/B35). Their wiring in
  `in_rust` (picking the end's id, `WreckInRam`'s dt) is not exercised by any replay.
- **The run's scoring is dropped, not ported** (F31b5h). On flip-1 row 94 the dropped calls write the
  run (`+0x3c`), the player's `+8` flags and `+0xc8` list, and three other objects. None of these is the
  bike's, and the bike stays equal. But the player's `+8` bit 5 is the run-open flag that both the start
  and the end branch on. A later stunt in a pure frame would make different run calls than the
  original, though the bike's words do not depend on that branch.
- **The cue dropped.** Its one write off the stack on flip-1 row 72 (0x00851fec) is not the bike's. It
  is not ported. Whatever it drives (a sound, a camera cue) is not in the pure frame.
- **RAM also differs from row 125 on** (three ranges, oracle.txt), after the wreck. I did not trace
  which dropped call these knock on from. The bike, the blocks and every row stay exact.
- **The record is held only with no other record and no mover.** Otherwise `held` answers `None` and
  the call is handed on. I did not count whether either replay's record ever has contacts. The
  oracle's generator draws kind-0 contacts only, because a kind-1 contact is always with another
  record.
- **The stunt start is not proved for ids of 46 and over.** The original reads its table entry
  through address 0 there, and the image's word at 0 is not a pointer. The game's ids are under 46.
- `hand_on`'s original-side end still passes its id zero-extended (an F31b5a line). That is only right
  for ids that are not −1. It is never reached on the replays.

## Next

Nothing for F31b5g. F31b5h ports the run's calls.
