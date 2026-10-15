# 1 — i1 input (FINAL)

Task I1. 2026-10-15.

## What I did

`crates/game-sim/src/input.rs`: the port of the game's input layer, and nothing else in that crate but
its one `pub mod input;` line and one code-map line in `PLAN.md`.

- `Control` — the sixteen controls in the game's own map order, which is also the order events come out
  in. `CONTROL_MAP` is the binding of each to its bit, its pressure byte and its flags, lifted out of
  the game's own RAM (the game builds that table at runtime, so it is not in the image).
- `Report` — the 32 bytes `scePadRead` leaves behind, with the active-low big-endian word, the sticks
  and the twelve pressures. `Report::from_buttons` is the port's seam with a host pad: a bitmask in,
  the bytes the hardware would have produced out.
- `ControlState` — the six words the game keeps per control: `held`, `strength`, ms since the release,
  ms held, and each timer's previous-frame copy. `pressed()` is "held and the held timer reads 0", which
  is all the original leaves behind once the frame's events have been dispatched.
- `Event` / `EventQueue` — the one-frame queue: cleared at the top of the update, dispatched at the
  bottom of the *same* update, so a non-empty queue is an edge frame and nothing about an edge survives
  into the next one.
- `Pad::poll` — the original's own read, instruction for instruction in order: the absent-pad branch,
  the copy-and-clear of `held`, the status check, the sixteen-control read, then the second pass that
  compares against the copies, advances one timer, zeroes the other pair, and queues the event.
- `Manager::tick` — clear, poll every pad in device-list order, stamp. One call a frame, before the
  simulation, which is where the original's frame puts it.
- `add_ms` — the timer arithmetic, public because the oracle test proves it directly.
- `impl hash::State` for all five types, kept in `input.rs` rather than appended to `hash.rs` so that
  parallel tasks are not all editing the same tail of the same file.

`crates/game-sim/tests/input.rs`: 27 tests. `./kit check` → `TESTS PASSED: 156 tests`.

## What the original does

One frame is clock → `dt` → **input** → simulation → render, so a press is latched before the frame
that acts on it, and a frame that tears down a menu or starts a race polls the pad *after* the change —
which is why a button held across a transition is seen by the new state on its first frame. The answer
to "does a press on the frame of a state change count": yes, and it counts for the new state.

There is **no "pressed this frame" bitmask, and no per-pad bitmask at all**. One `PSXControl` per pad
under one manager; sixteen control records of 0x18 bytes each (`held`, `strength`, rel-ms, held-ms, and
the previous-frame copy of each timer at +8..+0x14); a 16×0x10 map built when the pad is bound (flags,
pressure index, u16 mask); and the manager's event queue, whose count is zeroed at the top of every
update.

Reading one control: flags bit 1 means analogue — take the pressure byte, call it down **strictly
above** 0x19, strength `(pressure + 1) / 256`, and never look at the bit. The other four (select,
start, R3, L3 — exactly the four a DualShock 2 sends no pressure for) test the u16 mask against the
button word with strength 1.0. The word itself is bytes 2–3 of the report, big-endian, active low, so
the poll computes `((b2 << 8) | b3) ^ 0xffff` once and tests against that.

The timers are the only record of *when* something changed, and they are integers with a float round
trip each frame: load the stored word, convert it as **unsigned** (`bltz`, then halve / `cvt.s.w` /
double), one `add.s` of `dt * 1000`, then convert back as unsigned (`c.ole.s` against 2³¹, and above it
subtract the bias and `or` the sign bit in by hand). Because the result is stored as an integer and
re-read next frame, the fraction is discarded every frame and never accumulates: at the 30 fps cap the
sequence is 0, 33, 66, 99 … a flat +33, where a float accumulator would give +33, +33, +34. I had
predicted the accumulating version; the recording says otherwise, and a port that accumulates drifts a
millisecond a frame — a frame of difference in a 33-frame menu repeat.

On a transition the *other* timer pair is zeroed (press → held-ms and its previous copy; release →
rel-ms and its previous copy) and an event is appended: control index, kind 2, the device's id, the
device's index, 1 for a press or 0 for a release, and a placeholder stamp the dispatcher overwrites
from the manager's time source. Reading the span's arguments (`move $5, $17` is the loop counter, `lw
$9, 804($16)` the index, `$8` the direction) is how the record's field order was settled.

Two branches raise **nothing**: a pad the library says is gone, and a read whose status byte is
non-zero. Both clear every `held` and `strength` and leave the four timers untouched, so a pad
unplugged mid-press loses its release event entirely and its release timer then counts on from
whatever it held before the press.

Recipes, research only — these addresses are in `research/oracle.txt` and this entry, never in the
port: `./kit fn 0x0016ab10` (the poll), `./kit dis 0x0016aec0 --len 0x128` (the timer update and the
event append), `./kit fn 0x0011d860` (the append: no bounds check), `./kit fn 0x0011dab0` (the
dispatcher: stamps, then app slot 0x50 for a press / 0x4c for a release), `./kit fn 0x00169c40` (the
update that zeroes the count), `./kit fn 0x0016a950` (`PSXControl::Open`: count 0x10, allocates and
zeroes the map), `./kit fn 0x001349f0` (the base constructor: zeroes only `held` and `strength`).

## How it was proved

**1. The whole poll against a recording of the real game — 219 frames, every word, zero mismatches.**
`context/capture/i1-input.csv` (220 rows, frames 2269–2488, `0 torn read(s), 0 counter gap(s)`),
recorded with `watch/input.toml` while `watch/pads/two-at-once.pad` drove the pad: two controls down in
one field, two up in one field, three down in one field, and a hold of over a second. The test replays
it row for row from the pad bytes in the same row and compares all sixteen control records (six words
each) and the events (six words each): `219 frames, 8 presses, 8 releases, 6 multi-event frames, 1155 ms
the longest hold`. The timers with uninitialised garbage in them are compared too, and they match every
row — which is a free proof of the unsigned round trip at values above 2³¹ (`ctl14.held_ms` sits at
2 722 105 685 and tracks exactly).

**2. Which half of libpad's double buffer the engine read.** The port must be fed the half whose stamp
is *behind*: validating the whole map against all 220 rows gives **0 mismatches of 3520 using the older
half and 17 using the newer one** (the halves differ in 9 rows, and `stamp_a − stamp_b` is 1 in every
row). The test picks the smaller stamp, so it states the rule rather than the accident.

**3. The timer arithmetic against the game's own instructions.** The update is not a function but a
thirty-instruction span inside the poll, so the test lifts those instructions into a bare interpreter,
appends its own `jr $ra` where the poll's branch went, and compares `add_ms` against them over 14
timers × 7 frame lengths = 98 calls — including both sides of 2²⁴ (where a single can no longer count
by ones) and both sides of 2³¹ (where the conversion back needs its bias), which the game would need
days of play to reach. Named `pad_timer_add_ms` in `research/oracle.txt`; the test skips with a sentence
on a checkout with no disc.

**4. The control map, off a RAM image** (`./kit state ram`, C4's slot 9 — no emulator, and safe while a
recorder is running): index, flags/pressure/mask — 0 triangle 03/12/0x0010, 1 circle 03/13/0x0020,
2 cross 03/14/0x0040, 3 square 03/15/0x0080, 4 L1 03/16/0x0004, 5 R1 03/17/0x0008, 6 L2 03/18/0x0001,
7 R2 03/19/0x0002, 8 select 01/–/0x0100, 9 start 01/–/0x0800, 10 R3 01/–/0x0400, 11 L3 01/–/0x0200,
12 up 03/10/0x1000, 13 right 03/8/0x2000, 14 down 03/11/0x4000, 15 left 03/9/0x8000. Both pads carry
device id −1 and indexes 0 and 1; pad 1's map is all zeros (never connected); all four timers of every
control are heap garbage. The masks are compared in the test against C5's word measured one button at a
time, so the two sources have to agree.

**5. The question C5 left for I1: what happens when several controls change in one frame.** They are
queued in **control index order, not press order**, because the poll reads all sixteen controls and only
then walks them again looking for changes. Frame 2392 of the recording carries three events with
`ev0` = control 4 (L1) and `ev1` = control 7 (R2) while the pad script pressed up (12), L1 (4), R2 (7)
in that order; frames 2275, 2287, 2308, 2320 and 2404 agree. Events carry kind 2, device −1, index 0 and
stamp 0 in every row. Row 0 also shows the queue's clear for what it is: `events = 0` beside a stale
`ev0.control = 10` left over from an earlier frame, which is why the hash walks only the events the
frame actually raised.

## Not verified / not 1:1

- **Select's bit (0x0100)** is from the game's own control map in RAM, not measured live: the kit's
  virtual pad refuses to send select (it is PCSX2's own hotkey modifier), so it is the one control of
  the sixteen no recording covers.
- **The four timers start at 0 in the port and are uninitialised in the original** — the base
  constructor zeroes only `held` and `strength`, so until a control's first transition its timers are
  heap garbage counting upward. Knowingly not 1:1, and harmless because the first transition of each
  control zeroes its own pair; the acceptance test therefore seeds the port from the recording's first
  row instead of pretending a fresh port could reproduce the garbage.
- **The original's event array length is unknown.** The append has no bounds check at all — it writes
  where the count says and bumps it — and nothing in the image gives the array's size. The port uses 32
  = 2 pads × 16 controls, which is the structural maximum (each control can change at most once a frame,
  and the two pad polls are the only callers), so the port cannot overflow where the original would not;
  but the number is not the original's.
- **Intermediate pressures are from the instructions, not from a measurement.** The virtual pad is
  digital, so every pressure byte in the recording is 0x00 or 0xff; `(pressure + 1) / 256` at the 254
  values in between, and the 0x19 threshold, are read off the code and unit-tested against the formula,
  not against hardware. A real DualShock 2 would settle it.
- **Only three `dt` values occur in the recording** (0.03333 — the cap — 0.03286665 and 0.03283332).
  Everything else about the timers is covered by the oracle run rather than by the game.
- **A zero-`dt` frame never occurred in any recording.** With the clock stopped, `pressed()` stays true
  while the queue holds the press for one frame only; the original has both readings and the same
  disagreement, and the module documents which is which, but the frame itself is untested against the
  game.
- **The two no-event branches are read, not recorded.** No capture has a pad unplugged mid-press or a
  non-zero status byte, so "the release event is lost with the pad" and "the timers are left untouched"
  come from the instructions and are pinned by unit tests only. (The absent-pad branch *is* exercised
  every frame of the recording in one direction: pad 1 was never plugged in, and the event count matches
  in all 219 rows, so it raised nothing.)
- **Pad 1 is otherwise unproven live** — its control map is all zeros in RAM and no recording has a
  second pad, so the port treating it identically to pad 0 is an assumption about the game's own code
  (one class, two instances), tested synthetically.
- **`held_mask` / `pressed_mask` / `released_mask` and `Report::from_buttons` have nothing to compare
  against.** The original never builds a bitmask out of its controls and never synthesises a report;
  these are the port's seams with a host pad, and they are exact only in the direction the mapping was
  measured in.
- **The pad's connection and mode negotiation is out of scope** (`./kit fn 0x0016b8c0`): the port takes
  `connected` from the platform layer. Nothing in the simulation depends on how a pad got into pressure
  mode, but a port that forgets to ask for mode 0x79 leaves twelve of the sixteen controls permanently
  released, which is worth knowing before someone debugs it.
- **The event stamp is 0 because this game's manager has no time source.** The dispatcher overwrites the
  placeholder with it on the way to the front end, so in every captured row it is 0; a build where that
  source is live is not covered. The port keeps the field and a `time_ms` that defaults to 0.

## Next

Nothing outstanding for I1. For whoever wires the front end: the queue is the thing to consume, in queue
order, within the frame — and `./kit fn 0x0011dab0` says which app slot each direction lands in.
