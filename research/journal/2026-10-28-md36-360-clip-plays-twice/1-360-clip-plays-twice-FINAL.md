# 1 — 360 clip plays twice (FINAL)

Task MD36. 2026-10-28.

## What I did

- Found why V69's 360 on whip_tricks.rs's flip ramp plays `threesixtyR` twice: neither of the brief's
  two guesses. The original has no latch, and the second play doesn't come from turning past 395°. The
  turn the clip reads is `min(|start − h|, |start + 2π − h|)`, and it only folds a heading that
  wrapped *downwards*. This ramp's bike leaves the ground at 352.1° and the 360 turns it up through 0.
  So the measure reads ~350° the moment it crosses: the clip starts 10.3° into the turn, comes round at
  ~327° (35° short of a whole turn), and starts again when the heading crosses 0 the second time, at
  372.4°. It lands at 429.5° with the clip playing. The PC exe measures it the same way (below), so
  the double play is the original's own behaviour. No code change was made for it.
- What the exe did show the port getting wrong: the clip state's transitions. Ported now as
  `air::spin_clip`, a pure function lifted out of `three_sixty`, with `air::spin_turned` for the measure
  and a doc that states the wrap quirk:
  - 0 → 1 when the turn is **over** 35° (unchanged);
  - 1 → 2 when **at or over** (it was "over");
  - 2 → 0 (came round) when **under** (it was "at or under");
  - **1 stays 1** when back under 35°. The port used to drop it to 0, which also skipped the mid-clip
    landing wreck. The exe leaves state 1 alone there (`else if state == 2`).
- Tests: `crates/game-sim/tests/spin_clip.rs` checks each transition and the boundaries, including no
  latch, plus the one-way fold (352° → 2.4° reads 349.6°; 8° → 350° reads 18°).
  `whip_tricks.rs` `a_360_taking_off_under_two_pi_plays_its_clip_twice` rides V69's tape and asserts:
  the take-off heading is under 2π − 35° turning up; the body plays the clip on exactly the frames the
  sim's clip state is set; there are exactly two starts; at each start the measure is over 35° but the
  true unwrapped turn is within 35° of a whole number of turns (the wrap starts it, not the turn).
  `Seen` gained the sim heading (the node's forward row), the clip state, the yaw torque and the
  take-off heading.
- Nothing taken from opentennis: it has no trick clips.

## What the original does

MX vs. ATV's 360 step on the PC exe: `./kit fn 0x00600790 -o pc` (Ghidra drops the x87 measure: the
clip compares read an `fStack_40` that is never assigned). The measure comes from the disassembly,
0x600851..0x6008b6: `fld start; fadd 2π (0x40c90fdb); fsub h; and 0x7fffffff` and `fld start; fsub h;
and 0x7fffffff`, then the lesser. `h` comes from `./kit fn 0x00564d20 -o pc`: the forward flattened
(y = 0), normalised, acos of z, and `(π − a) + π` when x < 0. That's the port's `spin_heading`, word for
word. The clip block:

    if state == 0:      if 0.6108652 <  turned: play(dir < 1 ? L : R) and, if it played, state = 1
    elif 0.6108652 <= turned: if state == 1: state = 2
    elif state == 2:    stop(clip), state = 0

The end of the move (landing, touching, yaw torque under MinY) wrecks with reason 0xb/0xf when the
state isn't 0. openmx-mods `notes/tricks.md` (M20) reads the same measure and rule off the PS2 ELF
(SLUS_211.04, steps 3 and 4). Untamed's `spin2` capture shows the re-entry live: the clip stopped, went
past 35° again, and the bike landed in state 2 and wrecked. M21's Untamed `spinpull` (path 436°, landed
101° off, wreck) shows the original turning well past 395° when the air is long enough. The port's
~205°/s on this 2.1 s flight sits inside M21's captured 190–230°/s.

## How it was proved

- The clip's transitions: against the PC exe's branches, by reading the code (the exe isn't an oracle
  for numbers, and nothing here is numeric beyond the 35° constant the port already had). Each branch
  has its own assertion in `spin_clip.rs`.
- The measure: the exe's instructions, plus the M20 notes off the PS2 ELF. `spin_turned` is the same
  arithmetic the port already ran; it just has a name and a test now.
- The double play: `whip_tricks.rs` above, on the port's own ride.

## Not verified / not 1:1

- **The PS2 ELF wasn't re-read for the state-1 and boundary clauses.** SLUS_211.04's Ghidra project
  isn't on this disk (openmx-mods `extracted/mxvatv/` holds only `Env`). They come from the PC exe,
  which `notes/tricks.md` says runs the PS2's same step. The M20 notes describe the 2 → 0 rule the same
  way, but never mention state 1 falling back.
- **No capture of the original on this ramp.** The ramp is a generated test lip, not a disc track, so
  "the original plays it twice here" is inferred from the shared measure, not filmed. Untamed's
  `spin2` is the only capture showing a second start.
- **The play-success clause isn't ported.** The exe only enters state 1 when its play call answers
  non-zero. The port enters it regardless, and only `rider.rs`'s `spin_state` checks the library. A
  pack bike without the clip would wreck in the port and not in the original. Filed as **MD59**.
- The edge-case change (state 1 holding under 35°, the `>=` boundary) can't be reached by any captured
  ride: at ~6°/tick a turn can't cross back under 35° the substep after crossing over it. No capture
  exercises it.

## Next

Done.
