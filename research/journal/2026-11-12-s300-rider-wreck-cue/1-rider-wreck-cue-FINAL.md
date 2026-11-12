# 1 — rider wreck cue (FINAL)

Task S300. 2026-11-12.

## What I did

- Ported MX vs. ATV's wreck cue, the call the rider's wreck entry ends on, as
  `game_sim::handling::rider::wreck_cue`. The engine's no-repeat pick is `rider::list_pick`.
  The audio side stays as new `rider::Call`s: `CueBusy`, `CueWake`, `SoundBusy`, `SoundPlay`
  and `CuePlaying`. The rider's `+0x8f4` object is a sound emitter, the same one MX Unleashed has
  (`game_sim::wreck_sound`, S173). What counts for the simulation is the pick's rand draw.
- `tests/mxvatv_rider_modes.rs`:
  - New oracle `the_wreck_cue_is_the_originals`.
  - `hook_calls` now hooks the cue's five audio calls in all tests. They log, and the two
    "still playing" ones answer from `CUE_ANSWERS`, which is 0 unless a test sets it.
  - S288's test no longer only logs the cue. It runs the original cue as it is and serves the
    port's `Call::WreckCue` with `rider::wreck_cue`. It now also writes the drawn kind into the
    record's `+0x138`, the field the cue reads, and draws 9 rands instead of 8.
- `tests/mxvatv_bike_ride.rs` rider leaf: `WreckCue` is now served by `rider::wreck_cue`. Its
  audio calls run the original's code through the vtables. flip-1's census now shows one cue,
  one extra rand, and one sound played. Both replays stay bit-exact.
- Nothing taken from opentennis: tennis has no such emitter. I took the shape from
  `wreck_sound.rs` (the MXU port of the same engine code).

## What the original does

`./kit fn 0x004f7ca0 -p atv` and `./kit dis` give the full logic; research/oracle.txt `# S300`
lists the words and the vtables. In words:

1. If `a1` is not 0, it does nothing.
2. The emitter's vt `+0x98` asks whether the emitter is still playing. If it is, nothing happens.
3. Otherwise the record's wreck kind picks a list:
   - 1–7 and 12: the crash list.
   - 8–11 and 14: the fall list.
   - anything else: nothing.
4. If the emitter is not awake, it is woken.
5. A sound is picked. A list of two or more draws `trunc(n × rand × 2⁻³¹)`. If that equals the
   last pick it moves on by one, mod n. A shorter list picks 0 without a draw.
6. It then stops unless `+0x1f0` bit 1 is set and the last playing sound is not still playing.
7. Otherwise the pick is played with flags 4, flag bits are set at `+0x1f0` and `+0xe4`, and the
   pick becomes the emitter's playing sound.

The rand draw in step 5 happens before the bit-1 gate in step 6. So the draw depends on the
emitter's audio "busy" answer only, not on the gate.

## How it was proved

- **Function oracle, 12 000 drawn cases** (`the_wreck_cue_is_the_originals`, state 4's emitter).
  Every written word and the call log are equal. Every path was taken at least 312 times. Totals
  from one run:

  | path | cases |
  |---|---|
  | each kind | ~650 |
  | emitter busy | 2 963 |
  | woken | 3 215 |
  | drew | 4 136 |
  | a repeat moved on | 360 |
  | a negative pick | 312 |
  | a sound still plays | 615 |
  | played | 2 633 |

- **S288's 20 000 cases** (`the_riders_wreck_entry_is_the_originals`) now include the cue,
  original against port.
- **Replay:** C15 flip-1 with the rider leaf on the ports stays bit-exact through its one wreck
  (cue drew, sound played). ride-2 has no wreck.

## Not verified / not 1:1

- **The `a1 ≠ 0` arm is not ported** (it returns at once). The only caller, the wreck entry,
  passes 0.
- **The audio calls are not ported.** These are the emitter's busy, wake and set-playing calls
  and the sound's busy and play calls. They stay as `Call`s because they are audio, not
  simulation. Their answers are audio state, and the busy ones decide whether rand is drawn. In
  the oracle they are drawn words. In the replay they are the original's code against PCSX2's
  RAM. In the app no answerer exists yet: nothing in the shell runs MX vs. ATV's rider, and
  nothing else calls `rider::wreck_cue`. Whoever wires it must answer the busy calls the way
  `wreck_sound`'s clocks do for MXU.
- **I did not read the PC exe's version** (AGENT.md "Reading the PC exe first"). research/mxvatv-pc.md
  has no mapping for the wreck entry or the emitter, and the PS2 function is small enough to cover
  every branch directly.
- **Signed picks:** the oracle feeds negative rand answers and negative last picks. Retail rand is
  never negative, so those cases only check that the arithmetic matches.

## Next

Done.
