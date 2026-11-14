# 1 — speed step (FINAL)

Task S314a. 2026-11-13.

## What I did

- Ported the brain's vt+0xa0 speed step, the adaptive difficulty's cap on the target speed of a rider ahead
  of the race's leader, as `ai::drive::speed_step` (with `drive::Leader` and `drive::NODE_FREE_SPEED`).
  It takes the brain's `Alod`, the rider's route-node flags, its card's progress and the leading card. It is
  still a `drive::Calls::slow` callout from the drivers' tail. The callout's doc now points at it, and the
  drivers' tests keep hooking it at its real address as before.
- Proved it in `crates/game-sim/tests/ai/drive.rs`, in two tests named `the_speed_step_matches_*`.
- Nothing taken from opentennis: it has no race AI to take a speed cap from.

## What the original does

`./kit fn 0x00388120` (the slot) and `./kit fn 0x00286960` (the cap). Use `./kit dis 0x00286960 --len 304`
for the exact compare order. The steps:

- The slot answers 0 when the brain has no route node (`+0x224`), or when the node's flag word (`+4`) has
  bit 4 set. Otherwise it answers what the cap answers.
- The cap answers 0 for a null speed pointer (the slot always passes `+0x1e0`), and 1 with no race or no
  leading card (race `+0xa4`).
- The base is the leader's `+0x108` word, raised to ALODSpeed (`c.olt`, so a tie keeps the leader's bits).
  The gap is the rider's card progress (`+0xd4` → `+0x1c`) minus the leader's.
- If the gap is ≤ 0, nothing changes.
- If the gap is within ALODWindow:
  - within ALODLeadWindow, nothing changes;
  - otherwise the cap is `base + (5 + gap·((5+base)·(upper−1))/window)`, in that operation order, and only
    a speed over the cap is cut to it.
- If the gap is past ALODWindow, a speed over `base·upper` comes down to `base·upper` or to `speed·lower`,
  whichever is more. A tie goes to `speed·lower`.
- If a stored cap is under ALODSpeed, the raw ALODSpeed word is stored instead.

## How it was proved

- **Function oracle, 10,000 generated calls.** The slot runs in the interpreter, going through to the cap,
  against the port. Both the answer and the raw speed word must match. The inputs are aimed at each branch
  and come out of the run as:
  - not stepped: 1225
  - no leader: 1006
  - behind or level: 974
  - within the lead window: 1479
  - between the windows, left alone: 1645
  - between the windows, capped: 1100
  - past the window, left alone: 1285
  - cut to `base·upper`: 650
  - cut to `speed·lower`: 636
  - stopped at ALODSpeed: 785

  The inputs also land exactly on the windows and caps, and include −0 progress and denormal ALODSpeed
  words. The first denormal case showed that `Ee::f32_at` flushes a denormal, so the test compares
  `u32_at`.
- **sx01's race image (S282c), the three brains as they stand.** The answer and the speed match the port.
  Two brains get stopped at ALODSpeed (65.85) and one gets cut to `speed·lower`. None of the three has
  SLOWS (0x400) up in that image, so the drivers would not have run the step on that tick; the test calls
  it directly.

## Not verified / not 1:1

- **No live capture of SLOWS doing anything.** No image or capture here has a brain with SLOWS (0x400) up.
  The sx01 brains have it down, and I did not look for which race kind or difficulty raises it. The
  function is proved; when the game runs it is not.
- **No product caller yet.** Only the tests implement `drive::Calls`, so nothing in the game calls
  `speed_step` yet. Whoever wires the drivers to real callouts has to read the node flags, the leading card
  and the brain's `Alod` into it.
- **The leader's `+0x108` word is not identified.** The step reads it as a speed, and `race::LapRecord`
  copies it as `word`. On the sx01 image it is 0.00089, so there ALODSpeed always wins.
- **Null pointers.** The port can't take a null speed pointer, which the slot never passes. It also takes
  the rider's own card progress as given, while the original reads the card unchecked once a leader exists.
- **Not read in the PC exe.** `pc.classes.tsv` has no VehicleAI or MXVehicleAI class, so the slot has no
  name there, and I did not search the exe for it by its constants.
- **Node flag bit 4's meaning.** It is named only by what it does here (`NODE_FREE_SPEED`); nothing found
  says what sets it.

## Next

Nothing for this task.
