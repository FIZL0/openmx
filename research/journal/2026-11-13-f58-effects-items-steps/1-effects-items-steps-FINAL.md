# 1 — effects items steps (FINAL)

Task F58. 2026-11-13.

## What I did

- Ported the effects items' steps (vt `+0x14`) to `crates/game-sim/src/handling/effects.rs`:
  `item_step` dispatches on a new `Kind` (`Still`, `Gauge`, `Ramp`, `Swing`, `Spots`, `Linked`),
  supplied by a new `Steps: Items` trait. Everything goes through ps2-float (FPU `adda`/`madd`/`msub`,
  `min`/`max`, VU `vmulx`/`vsub` lanes). Raw `lwc1`/`swc1` and `lq`/`sq` copies stay raw words, and `.xyz`
  results keep the source quad's raw `w`.
- New hand-out `Call::Move { object, dt }`, the object's vt `+0x98`. The Spots and Linked kinds use it
  after setting the object's spot (`+0x30`) and the spot it came from (`+0x40`). `Call` lost `Eq` (it
  now holds an `F`).
- The object's vt `+0x98` turned out to be `0x0050ceb0`, the first emitter kind's step, which F57
  ported. PartWalk answers `Call::Move` with `emitter::step` and books "effects item emitter steps".
  So the 18 flip-1 steps that draw rand are now Rust through to the draw; `rand` itself is still
  handed on, as in F57.
- `tests/mxvatv_bike_ride.rs`:
  - New table `FX_KINDS` maps the six vtables to their kinds.
  - PartWalk's `Out::Step` runs `effects::item_step` for those kinds and books "effects item steps".
    An item of any other vtable is still handed to the original.
  - `fx_object_call` passes floats (dt for `+0x98`).
  - `walk_served` now also asserts that no `+0x14` was handed on. The flip test also asserts that the
    port stepped items and that some moved their object.
  - New oracle test `the_effects_items_steps_are_the_originals`.
  - Flip test: asserts that items moved their emitters on the port and that no `+0x98` was handed on.
- research/oracle.txt `# F58`: every address.
- opentennis: nothing to take. It has no counterpart of these vehicle effect items.

## What the original does

Each step is short and was read whole in the disassembly (`./kit dis <addr> <end> -p atv`), not
only in Ghidra's C. The vtables come from `./kit dis 0x003d1650 0x003d1678 -p atv` etc.

- **Still** (`./kit dis 0x004ff170 0x004ff178 -p atv`): `jr ra`. This is the Clocked class's step.
- **Gauge** (`./kit fn 0x004fa980 -p atv`):
  - level = `0.5 * a * (span - b) / span`. `a` and `b` are the words the table's `+0x18` and `+0x20`
    point at, and span is the item's `+0x18`.
  - If level ≤ 0, douse.
  - Otherwise object `+0x68` = item `+0x1c * (0.5 + level)`, then light.
- **Ramp** (`./kit fn 0x004faf20 -p atv`):
  - Latch (item `+0x90`): the entry's first word sets it; with that clear, the second word clears it.
  - If unlatched, or the entry's `+0x10` word is ≤ the floor (`+0x28`), douse.
  - Otherwise ramp = `adda(0, +0x3c).madd(+0x40, w)`.
  - With `+0x30` set there are two branches:
    - above the knee (`+0x2c`): the ramp gains `w * +0x44`, and the definition's rates and spans are
      the item's own times `+0x34`/`+0x38`;
    - otherwise: raw copies, including two quads, and the definition's `+0xc4` = `+0xc0 - +0xbc`.
  - In both, definition `+0x150` = `+0x140 - +0x130` (`.xyz`).
  - Then counts `+0x110`/`+0x114` = item `+0x48`/`+0x4c` times the ramp, then light.
- **Swing** (`./kit fn 0x004fb600 -p atv`):
  - Swings when the table's gate is open (level `+0x18` ≠ 0, gate `+0x1c` = 0) and the entry's
    `+0x14` word is set, or when the entry's swing (`+0x20`) is outside the ±`+0x30` band.
  - A swing sets:
    - object angle `+0x60` = `-(+0x2c) * swing`;
    - x spans from half of |angle|;
    - y spans from `clamp(1 - (1 + table swing) * +0x44, 0, 1)` (a `msub`, `min.s`, `max.s`);
    - counts: raw copies, then `madd` by half of the clamp;
    - fade `+0x28` = 0.25;
    - the definition's `+0x28` bit 0x200, by object `+0xb1` bit 0 when `+0x70` is set;
    - then light.
  - Inside the band, the fade runs down by dt. While it is above 0, the counts scale by fade/0.25 and
    it lights; at 0 it is cleared and the object doused.
- **Spots** (`./kit fn 0x004fdba0 -p atv`):
  - The part comes from `*(item +0x28)`; with none, nothing happens.
  - If the item's mask meets the part's flags, douse. The flags are built as an entry's, from part
    `+0x3c`/`+0x40` and the table's bit.
  - Otherwise light, then take motion = part `+0x1f0 * dt` (`.xyz`, w raw).
  - For each spot (count `+0xf0`, re-read every pass; array `+0xf8`, 0x90 bytes apart) whose pick byte
    (item `+0x18` + spot `+0x80`) is not 0xff:
    - if the pick differs from the definition's `+0x40`, pick (`./kit fn 0x0050a920`);
    - object `+0x40` = spot `+0x40` − motion, object `+0x30` = spot `+0x40`;
    - object vt `+0x98(dt)`.
- **Linked** (`./kit fn 0x004fe010 -p atv`):
  - Light or douse by the item's mask against the table's bit alone.
  - Then walk the table's parts (count `+0x10`, read once; array `+0x14`). A part is skipped when it is
    absent, when its bit in the links word (item `+0x18`) is clear, when its pick (item `+0x1c` + part
    byte `+0x3c`) is 0xff, or when the mask meets its flags (`+0x18`/`+0x1c`). Every other part moves
    the object as in Spots.
  - The motion here is `*(part +0x80) +0x80 * dt`, read *after* the pick.
- None of the six calls rand. The 18 flip-1 Linked steps that F31b5n saw draw rand do it inside their
  object's vt `+0x98`, the emitter step `0x0050ceb0` (F57's `emitter::step`).

## How it was proved

- **Function oracle `the_effects_items_steps_are_the_originals`, 20 000 cases.**
  - Each case is an item in scratch RAM with one of the six real vtables. It has its effects listener
    and table, an entry whose words point into a pool of floats and zeros, two objects with
    definitions, the spots' part with its spots, and eight linked-part slots (some empty).
  - The floats are junk-ish: ±0, 0.5, ±1, raw junk bits (Inf/NaN/denormals) and values in ±3. Picks
    mix 0xff and 0..3 against definition picks of 0..3. dt is 1/30, 0, or a value in −0.05..0.5.
  - Each case runs SLUS-21104's step and the port's.
  - The object calls are hooked on both sides with the same reactions, so order and re-reads are
    tested:
    - a pick moves the item to its second object and changes the linked motion;
    - a light changes the spots' motion;
    - a move decrements the spots count and flips link bits.
  - Result: the calls match in order and arguments (dt bits included), and the whole 0x11c0-byte
    layout matches.
  - Every kind lit and doused. Spots and Linked picked and moved, and Still made no call.
  - Four mutants each fail it, one at a time, restored with cp + touch:
    - Ramp's knee ramp without its `w * +0x44`;
    - Swing's fade counts without the /0.25;
    - Swing's spin bit inverted;
    - Ramp's raw branch without the definition's `+0xc4`.
- **Every-port replays.** `flip_replays_with_every_port_at_once` and `ride_replays_with_every_port_at_once`
  pass with the steps in Rust: C15 matches word for word every frame, and the RAM is the handing run's
  apart from the known race-progress and cue words.
  - flip-1: 4357 item steps ran on the port, and 554 of the emitter steps they reached ran on F57's
    port. No item virtual (`+8`, `+0x14`, `+0x18`, `+0x1c`) and no `+0x98` was handed on.
  - ride-2: 4834 item steps and 52 emitter steps ran on the port.
- Gate: `./kit check --all` in `./kit done`.

## Not verified / not 1:1

- **Some object calls are still the interpreter's:** light (`0x0050aaa0`), douse (`0x0050ab00`), halt and
  the pick (`0x0050a920`) are the emitters' code (F59 and beyond). The move's emitter step is F57's
  port, but its own hand-outs (`rand`, the other kind F57a, …) are still the original's. In the
  oracle all object calls are stand-ins, so it proves only that the steps make them as the original
  does.
- **`0x003d1740`'s step (`0x004fd0c0`, 700 bytes) is not ported.** No C15 item steps with it. PartWalk
  hands it to the original if one ever does. Filed as a follow-up.
- **The kind map is test-side**, as F56's class map is (`FX_KINDS` in tests/mxvatv_bike_ride.rs).
  Nothing in the shipped frame calls `effects::item_step` yet.
- **dt with junk bits is not in the oracle.** The original hands `$f12` to vt `+0x98` raw (`mov.s`),
  while the port hands the operand-read `F`. They differ only for an Inf/NaN/denormal dt, which the
  frame never makes. This is not 1:1 for such a dt.
- **Unaligned quads:** as in F57, the port reads and writes quads at the exact address, where
  `lq`/`sq`/`lqc2` round down to 16. The game's spots, parts and definitions are aligned (the replays
  match RAM), and the oracle's are too.
- **Branch reach in the replays:** I did not count which inner branches C15 reaches (Ramp's knee and its
  raw-copy branch, Swing's fade). The oracle covers them (see the mutants).
- **Not read for this task:** the PC exe. research/mxvatv-pc.md has nothing on these classes. The PS2
  code was read whole, and the oracle runs against the PS2 ELF.

## Next

(none)
