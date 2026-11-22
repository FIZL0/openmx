# 1 — props put back switch (FINAL)

Task S475. 2026-11-21.

The person picked (sheet S475, 2026-11-21): "Add it as a switch: props reset on poof and at the start
line, off in original mode". This entry builds that switch. The live proof that the original has no such
reset is the earlier entry, research/journal/2026-11-21-s475-poof-keeps-knocked-props/.

## What I did

- **Props Put Back** (GLOSSARY.md), the port's own, off by default:
  - `PropSet::put_back_with_rider`: the switch. A constant of the ride, like `bike_frame`: not hashed or
    sent, and a snapshot decode keeps this machine's (it writes only the collections and listeners).
  - `PropSet::put_back_due`: a human rider poofed or went through the lap line this tick. Set where the
    frames end, used up by the props' step (`PropSet::step_with` / `step`), which first puts every prop
    back when the switch is on. Off, the mark is only cleared.
  - `PropSet::put_back_all` / `Collection::put_back_all`: each collection as the loader leaves it. Every
    body asleep on its placed spot (`Collection::reset`), countdown stopped, unfiled, every category; no
    listeners, lists or countdown; the world node's listener list empty.
- Where the mark is set (`Sim::props_put_back_on`, humans only: a rider with no AI levers):
  - a poof: the rider frame's `respawned` (the L1+R1 chord and the wreck put-back alike), in
    `Sim::step`, `step_all_posed`, the flown riders' step and the split-screen seat step
    (`rider_contact.rs`).
  - the plane's put-back: player 1's L1+R1 on the plane (`reset_chord` says it put the plane back).
  - the lap line: the race frame's `LineNotice::Lap` (a counted lap) or `Started` (a ghost race's first
    crossing) on a human's card.
- The shell: `--props-put-back` turns it on (help text too). Forced off online, where both machines must
  step the props alike.
- The Settings screen: a Props Put Back row under its own Props heading on the Game Modifiers page
  (the world's, `props_put_back = on` in the settings file), taken as the next ride starts. Original
  mode On turns it off, and on it reads Mixed; the host's All Players reset turns it off. Dimmed online.
  Test: `tests/menu.rs` `the_props_put_back_row_toggles_and_survives_a_restart`.
- Test: `tests/prop_set.rs` `a_poof_puts_knocked_props_back_with_the_switch_on`: a swung sign, the L1+R1
  poof; on, the sign is back on its spot, asleep, in that tick and stays; off, it swings on.
- Nothing from ../opentennis: it has no knockable props.

## What the original does

Nothing here: the original never puts props back on a poof or at the line (the earlier entry). This is an
addition, layered on top, off by default.

## How it was proved

The switch is the port's own, so there is nothing to prove against the original. Off is the original: the
mark is cleared and nothing else changes, so every replay and prop test runs as before (the gate). On is
checked by the new test above.

## Not verified / not 1:1

- **Not 1:1 by design:** the switch is not in the original. Off is the original.
- **The row takes at the next ride:** toggled under the pause, it waits for the next ride start (P394).
- **"The start line" is the race's line only:** free ride has no start line the game knows, so there only a
  poof puts props back.
- **Not on a tape:** a recorded tape does not carry the switch (Backwards Bail does), so a tape ridden with
  it on replays with it off. Part of the follow-up.
- **The lap line path is not run in a test:** it needs a race on sx01 off the disc and a rider driven
  through the line. The mark it sets goes the same way as the poof's, which is tested.
- **Online is off:** the switch is not agreed between the two machines, so the shell forces it off.
- **The plane's put-back is not run in a test:** it sets the same mark as the bike's poof. Only player 1
  flies a plane.
- **Puts back every prop, not only the knocked ones:** an untouched prop is already on its spot, so this
  changes nothing for it.
- **Not seen in the app:** no pad on this machine to knock a prop and poof by hand.

## Next

P394: the tape line, and the row taking under the pause.
