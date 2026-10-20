# 1 — s53 the ragdoll in the ride (PART)

Task S53. 2026-10-20.

## What I did

- **Found the task's two prerequisites unmerged.** The brief says "after S51 and S52". At the start of
  this session neither was on main. S51 (the step) was running in slot s1 with no commits yet. S52 (the
  contact and the settle test) was running in slot s4 with one WIP commit, and had uncommitted edits to
  `ride.rs`'s `Crash`, `rider.rs` and `rider_anim.rs`. Those are the files this task names.
  Without the step nothing moves the bones. Without S52's pose copy the bones have no pose to draw.
  Without both, the settle tick cannot be compared with the capture. Editing `Crash` and `Rider`
  alongside S52 would also give two merges that conflict.
- **So this session did the one piece that stands alone: the wreck sound's draw.** It is new
  `crates/game-sim/src/wreck_sound.rs`, proved bit-exact. The rest of the task is blocked on S51 and
  S52 being merged (see Next).
- `wreck_sound.rs`:
  - `List::pick` is the engine's no-repeat pick (`./kit fn 1b5820`). A list of fewer than two sounds
    answers 0 and draws nothing. Otherwise it draws `rng.index(count)`, the same three instructions as
    `Rng::index` (scale × rand first, then × count; checked in `./kit dis 1b5820`). If that equals the
    last pick it moves on one, mod count.
  - `Emitter::wreck` is the wreck entry's last call (`./kit fn 33e9c0`), down to the pick. Reasons
    1–7 and 12 pick from the crash list, 8–11 and 14 from the fall list, and the rest pick nothing.
    Nothing is picked either while the emitter's status virtual says it is playing.
  - `Emitter::retail()` is two lists of five, nothing picked. That is what first_recording's live
    emitter holds.
- New oracle.txt names: `sound_pick` and `emitter_busy`. Also a field map of the emitter under
  `wreck_sound`, whose note no longer says "Not ported".

## What the original does

- The wreck entry (`./kit fn 24ad90`) ends with `wreck_sound(actor+0xa74, 0)`. This runs for every
  reason; the switch only decides what comes before it.
- `wreck_sound` returns at once in three cases:
  - its second argument is non-zero (never, from this caller);
  - the emitter's vt +0x90 (`emitter_busy`, `./kit fn 1b6440`) answers non-zero, meaning a sound of
    its own is still playing;
  - the switch on the request's wreck reason (`emitter+0x240`, which is the actor's own request
    `+0x488`, the one the wreck entry's tests write the reason into) does not know the reason.
- Otherwise it calls a stop virtual if `+0x29` bit 0 is clear, then picks from the reason's list.
  After the pick, the play is gated by `+0x1f0` bit 1 and by another status call. That part is audio
  and draws nothing.
- Draws per wreck, as the original makes them at the entry:
  - reason 5: 2 hand draws + 5 throw draws + 1 sound;
  - reasons 1–4, 8, 11, 12: 5 + 1;
  - reasons 6 and 7: 1 (the sound only);
  - reason 13: 3 for the cartwheel and no sound;
  - reasons 9, 10, 14: 1 (the sound, no throw);
  - reason 0 and 15+: 0.

  This counts only an idle emitter.
- **Looked at in the captures, not proved.** Counting the LCG steps between capture rows around each
  captured wreck entry gives:
  - the first recording's two reason-5 entries: +10 and +9 over the row before;
  - recording2's reason 8: +5;
  - reason 12: +7;
  - reason 6: +0.

  The background varies from row to row (2 to 12 draws), and each row covers two frames, so these
  neither confirm nor refute the counts above. Reason 6's +0 hints that the emitter was busy, or that
  the entry fell in a different row. The script is not committed: it is a dozen lines over the
  `rng_state` and `wreck_reason` columns.

## How it was proved

`crates/game-sim/tests/wreck_sound.rs`, on first_recording's save state:

- `the_emitter_is_the_retail_one`: the live emitter's two lists are 5 and 5 with nothing picked.
  Its request is the actor's own `+0x488`, and its vt +0x90 is `emitter_busy`.
- `the_pick_matches_the_original`: a function oracle on `sound_pick` over **10 000** cases:
  - list sizes negative, 0..7, 5 and up to 2^20;
  - last picks of −1, random, or inside the list;
  - random generator states.

  Compared: the answer, the stored last pick and the generator's state. The cases include more than
  1 000 short lists, more than 5 000 drawing ones and more than 300 that moved on.
- `the_wreck_sound_matches_the_original`: a function oracle on `wreck_sound` over **10 000** cases:
  - reasons −1..16, each more than 100 times with the list it should pick from;
  - the status virtual hooked to answer 0, 1 or 2;
  - both lists' sizes and last picks generated;
  - `+0x29` bit 0 random, and the stop virtual hooked and counted (it ran more than 1 000 times);
  - the play gate's bit cleared.

  Compared: both lists and the generator's state.
- `a_retail_wreck_draws_once`: a retail emitter draws exactly once for a known reason, and not at
  all for 0, 13 and 15.
- A planted mutant (move on by two instead of one) failed both oracle tests.

`./kit check -- -p game-sim --test wreck_sound`: `TESTS PASSED: 4 tests`.

## Not verified / not 1:1

- **Almost all of S53 is not done:**
  - the ragdoll is not run from `Rider` (the wreck entry, the riding entry, the damping switch);
  - Crash's rider point is not replaced;
  - the shell does not draw the bones;
  - `hit_of` is not reconciled;
  - nothing is held against the captured wrecks;
  - there is no reason-5 settle-tick test.

  All of these need S51's step and S52's contact, pose copy and settle test. Neither was merged, and
  S52 was editing the same files.
- **The sound's draw is not wired into the ride.** Where it goes depends on where the wreck entry's
  draws are made. Today the throw's draws are made by `rider_anim`'s wreck-mode stand-in
  (`enter_wreck`, through the idle pool's generator, and only when a shell runs `body_frame`). S52
  is replacing that stand-in. The sound must draw right after the entry's own draws, out of the same
  generator, so adding it to `Rider::frame` now would draw in the wrong order.
- **The emitter's busy status is audio state the port cannot know.** `Emitter::wreck` takes it as an
  argument. A port with no sound system will pass `false`, which is right only if the rider's own
  emitter is idle at the wreck. What plays on that emitter, and so whether it can be busy at a wreck,
  is not established. The captures' draw counts do not settle it either (What the original does).
- **The play path after the pick is not modelled.** The stop virtual, the play gate (`+0x1f0` bit 1),
  the second status call, the play and the two flags it sets are audio. The oracle clears the gate's
  bit, so that path never runs in the proof. Reading the decompilation, it calls nothing that draws,
  but that is unproven.
- **The two list names are mine.** "crash" (reasons 1–7, 12) and "fall" (8–11, 14) follow the reasons'
  meanings. The sounds the lists hold were not read.
- **Only first_recording's rider's emitter was read.** The other riders' list sizes were not.

## Next

S53 is blocked on S51 and S52. Once both are on main, in this order:

1. **Replace the stand-ins.** Replace `rider_anim::Body::enter_wreck`'s five throw draws (or whatever
   S52 leaves there) with `Ragdoll::wreck_enter`, then `Emitter::wreck(reason, false, rng)` on the
   same generator. Put a `wreck_sound::Emitter::retail()` on `Rider` beside the ragdoll.
2. **Reconcile `ride::hit_of`** with `Ragdoll::wreck_enter`'s switch: no throw for 6, 7 and 13, and
   nothing at all for 0, 9, 10 and 14+. `Crash`'s hand-hold time should come from the ragdoll's
   drawn `launch.hand_hold`.
3. Draw the bones in `crates/openmx/src/main.rs` while the rider is down.
4. Write the reason-5 settle test against acceptance-freeride's two windows (ticks 3259–3349 and
   4167–4190).
