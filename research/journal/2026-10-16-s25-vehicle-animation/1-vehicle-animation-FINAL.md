# 1 — vehicle animation (FINAL)

Task S25. 2026-10-16.

## What I did

Ported the vehicle's own animation state machine — the only live caller of what S20 ported — into a
module of its own, so the animation caller finally has the caller it was written for.

* `crates/game-sim/src/vehicle_anim.rs`, new, 828 lines. One frame of the machine, in the original's
  order: `Driver::frame`. With it come the pieces the frame needs and nothing else owns —
  `Library`, the vehicle's own sixty-entry clip table and the loader that fills it off the stunt
  names; `Lag`, the two first-order lags that carry the pose grid's two axes; `Params` and `Request`,
  the fields the frame reads off the vehicle and off the request block the vehicle points at;
  `Blend`, the five shapes a frame can hand `motion.rs` (free, one, two, four and the pair), with
  `run` driving the clock through `motion::Player`; `Place` and `Play`, what the frame put where.
* `crates/game-sim/tests/vehicle_anim.rs`, new, 1220 lines, three proofs — the state machine, the
  clip table and the sixth entry point. All three bit-exact against the retail image.
* `crates/game-sim/src/lib.rs` declares the module. `chassis::turn` and `motion::clamp01` /
  `motion::remap` became `pub(crate)`: three words, no bodies touched.
* `research/oracle.txt` gains an S25 block — the vehicle's field map, the request block's, the frame
  order, both tables arm by arm, and the two mode-four writers with the reason Ghidra drops them. It
  also records five names the task needed (`transform_set_position`, `motion_blend_pair`,
  `vehicle_load_clips`, `vehicle_extra_at`, `vehicle_add_anim`) and corrects one claim of the S20
  note.
* `PLAN.md`'s code map gains the module.

## What the original does

The frame is `./kit fn 0x002492b0` — 2548 bytes, `(dt in f12, vehicle in a0)`. Its two tables are the
data at `0x00431a80` (the fade rate each state restarts with, by state) and `0x00431990` (the sixty
jump-table arms). The clip loader is `./kit fn 0x002483a0`, the extra-record lookup
`./kit fn 0x0024eee0`, and the sixth entry point of S20's animation caller is
`./kit fn 0x00147640`.

The frame is thirteen steps and then a dispatch:

1. **Placement.** Skipped entirely when the request is not held *and* both of the vehicle's two
   "placing itself" flags are set — that is the only way out. Otherwise the position is either the
   request's base quadword as it stands, or the request's offset vector turned by the request's
   quaternion (`chassis::turn`, the same routine the chassis leans with) and added to the base,
   keeping the turned vector's `w`. Then the node is given that position and the request's rotation.
2. **The restart test** is `want != state`, *or* the request's sequence number is at least two and
   differs from a byte on the vehicle that this frame never writes. So on a vehicle whose byte stays
   zero — which is every vehicle this frame can see — any sequence of two or more restarts the state
   every frame. I ported the test, not the consequence.
3. **The rate switch**, only on a restart, and only to pick which of the vehicle's five fade-rate
   fields becomes the live one. Two of its arms (the two reverse states) also zero the reverse clock,
   and one group (the ready poses) takes zero instead of the ready rate when the request says cut.
   An out-of-range want falls to the default rate — the switch's own default arm, not a guess.
4. On a restart: remember the previous state, rewind the clock, raise the started flag, set the ramp
   to one and zero all three weights.
5. If the request is held, the live rate and the ramp are both zeroed — a held request is a hard cut.
6. The state becomes the want.
7. The ramp walks down by rate times dt.
8. The from-table flag is cleared (two arms put it back).
9. The holding flag drops when the ramp has run out, unless the state is the air one.
10. If held, each lag whose target is not `FLT_MAX` is snapped to its target instead of stepped.
11. Both lags step — mix first, then phase. The step is `value += min(dt, tau)/tau * (target - value)`.
12. **The arm.** If the vehicle's extra index is not negative the frame takes an added record instead
    of the table: a record holds up to two clips, and whether both, only the first, only the second or
    neither is there picks mode two, one, one or zero. The second-only case uses the *mix* lag where
    the first-only case uses the phase lag — the same weight register, loaded from the other lag.
    Otherwise the sixty-entry table:
    * **0..23, the gestures.** From-table goes back up, mode becomes one, and the clip is this
      state's. If the previous state was *not* a gesture the blend is a plain one-clip play at the
      request's progress. If it was, this is the cross: on a restart the cross is zeroed and both
      cross clocks take the start phase; otherwise the cross length is the cap or half the outgoing
      clip's duration, whichever is less, and the two clocks are advanced by the request's progress
      scaled by each clip's duration. Either way the ramp is zeroed, both clocks are clamped, and the
      mode becomes **five** — the sixth entry point, which nothing else in the image calls.
    * **24..29, 31, 32, 36, 37** have no arm at all: they fall to mode zero, which only advances the
      clock.
    * **30, the pose grid.** The phase lag is the first weight. Within a row the mix lag below a half
      picks the row's second and first clips and doubles itself; above, the third and second and
      doubles the remainder. Then the request's band value picks the row: below the first threshold,
      the stopped row paired against the still row; up to the second, the still row alone; past the
      third, the riding row alone; between the second and the third, the still row paired against the
      riding row. The two paired cases are the **only two writers of mode four** in the image, and
      each one's `addiu` sits in a branch delay slot, which is why the decompiler drops both blocks.
      Their third weight is `motion::remap` of the band across the two thresholds, skipped — left at
      one — when the two thresholds are equal.
    * **33, 34, reverse.** A positive band zeroes the reverse clock; otherwise the clock runs
      backwards by dt times three hundredths of the band and is wrapped by `fmodf` at one. Two clips,
      the clock as the first weight and the phase lag as the second.
    * **35, air.** The same within-row pick as the pose grid on its own three clips, phase lag first,
      mode two.
    * **38, backflip.** One clip, weight a hard tenth.
    * **39, no-hander.** One clip, from-table back up, phase lag as the weight.
    * **40..59, the wreck-and-hold group.** On a restart it calls the engine's play with the live rate
      as the fade and this state's clip — **unconditionally, without looking at whether the slot holds
      one**. A null handle in the clock is what "nothing is playing" is, so the arm is not a no-op for
      an empty slot; it clears the clock. Mode zero either way.
13. **The tail** clamps the three weights and the ramp into 0..1 and dispatches on the mode: five to
    the pair, four to the three-clip blend (which takes four clips), two to two, one to one, zero to
    the plain advance.

**The clip table** (`./kit fn 0x002483a0`) is two passes over the stunt table into the first
twenty-four slots — the stunt's own name, then, for the slots still empty, the name again — followed
by thirty-six fixed names into the rest. Each name is the vehicle's prefix and the bare name joined
and truncated to thirty-one bytes, which is where the loader's buffer ends; 1917 of 2000 generated
vehicles hit that truncation. Unlike the rider's loader there is no per-vehicle gate on a stunt and no
`bike_` infix.

**The sixth entry point** (`./kit fn 0x00147640`) scales each clip's duration by its own phase, slerps
the pose of the first clip onto the second by the mix — so mix zero is the second clip and one is the
first — and writes the clock. It has no zero test on the window, so it fires the empty window every
frame the clip does not wrap.

## How it was proved

Three function oracles in `crates/game-sim/tests/vehicle_anim.rs`, each against the retail image
through `ps2-oracle`:

* **the state machine** — 24,000 generated frames over 62 states (all sixty plus two out-of-range
  wants), bit-exact. Compared per frame: the position and rotation the frame put on the node (as
  bits), whether it rewound, what it played, and the whole call the tail made — mode, every weight,
  the ramp and all four clip handles. The generator is asserted to have covered every state more than
  a hundred times and every mode the machine can write: 19,635 restarts, 15,967 placements, 5,043
  crosses out of a gesture, 3,013 frames off an added record, modes `[11293, 5338, 1961, 0, 365,
  5043]`, and the two mode-four writers 196 and 169 times.
* **the clip table** — 2,000 generated vehicles, all sixty slots each, exact. 12,289 slots left empty,
  62,956 pose clips found, 1,917 vehicles with a truncated name.
* **the sixth entry point** — 6,000 generated frames, bit-exact on the clock it writes and the window
  it opens. 2,988 landed on a new clip, 1,338 wrapped, 1,783 fired the empty window.

`./kit check`: 460 tests pass.

## Not verified / not 1:1

- **A negative previous state.** The gesture cross indexes the clip table with the previous state
  without a range test, so a negative one reads the words in front of the table; the port answers
  "no clip". Excluded from the generator — there is no path in the frame that writes a negative
  state, and reproducing the read would mean modelling whatever lies at `vehicle+0x900` minus a few
  words.
- **An extra index past the end of the container.** The record lookup trusts the index; the port
  answers an empty record. Excluded for the same reason — the index is a track's to set and the frame
  never derives it.
- **A gesture cross whose outgoing clip is missing.** The cross divides by that clip's duration
  through a handle it never null-tests. The generator forces both of the two slots present, so the
  original's read through a null handle is untested and the port's `F::ZERO` divide is unreached.
- **A blend whose first clip handle is null.** The original dereferences it; the port does nothing and
  leaves the clock alone. Asserted at the end of the third proof as the port's behaviour, not the
  original's.
- **`Play::duration`.** The play the wreck-and-hold arm makes passes zero for the keep-phase flag, so
  the engine never reads the clip's duration out of it. The field is carried because the engine's play
  takes it; nothing downstream of this frame can observe what it holds.
- **Everything in the sixth entry point past the clock.** The channel sampling and the quaternion
  blend it does after writing the clock are the renderer's half, out of this crate's scope, and are
  not ported — the proof compares the clock and the window only.
- **The restart test's unwritten byte** (step 2 above) is ported as the original computes it. I have
  not found the writer of `vehicle+0x8fc`; if one exists in a track script the port reads the same
  field, but I cannot say the field is always zero, only that this frame never sets it.

## Next

Nothing outstanding. `motion.rs`'s sixth entry point now has its caller; the module is a leaf — no
other crate calls it yet, and wiring a vehicle actor onto it is a later task's job.
