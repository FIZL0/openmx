# 1 — c5 pad (FINAL)

Task C5. 2026-10-15.

## What I did

`./kit pad` reaches the game, and the three places a press lives in RAM are now named: libpad's buffer,
the game's own per-control state, and the one-frame event queue the front end reacts to. The proof is a
300-frame capture in which a scripted press moves the menu selection, with the whole chain — raw button
word → control state → event → selection — in every row, and 25 checks over it that all pass.

The short version of the finding the rest of the port needs: **the game keeps no "pressed this frame"
bitmask, and no per-pad bitmask either.** It keeps 16 per-control entries (held, strength, ms-since-
release, ms-held, each timer with the previous frame's value beside it) and raises an *event* on the
transition, consumed inside the same tick. "Pressed this frame" is "held, and the held timer reads 0".

Committed:

- `watch/pad.toml` — the watch spec: libpad's two buffer halves and their stamps, the event queue, all
  16 controls of pad 0, and the two entries of the dialog the presses move.
- `watch/pads/menu-down-up.pad` — down/up once a second, the input the capture is clocked against.
- `docs/PCSX2.md` — a new "Where a press lands in RAM" section under the virtual controller: the buffer
  layout, the 16 button bits, the half-stamp trap, the control map and the event queue.

Not committed (game data lives in the git-ignored `context/`):

- `context/captures/pad-menu.csv` + `.json` — the capture, 300 rows, frames 13138..13437, no gaps.
- `context/captures/check-pad.py` — the 25 checks, re-runnable on any capture from this spec.
- `context/shots/c5-dialog-before.png`, `c5-dialog-after-down.png` — the same dialog before and after one
  scripted `press down`: the highlight is on *No*, then on *Yes*.

Local-only settings (`kit.toml` is git-ignored, so the next agent in another worktree must redo these —
they are listed in full under "How it was proved" so they can be pasted back):
`[states] 3` now describes the Training dialog, and 21 new `[symbols]`.

## What the original does

One frame reads the pad once, before the simulation. `@engine_tick` (`./kit fn 0x00131100`) calls the
input manager's Update through `app+0x104`'s vtable slot 0x10 — `./kit fn 0x00169c40` — and that is the
only pad read in the frame. Update zeroes the event queue, polls each device, and dispatches the queue.

**The device.** `./kit fn 0x00130410` makes the manager (0x67c bytes, `Game.cpp`) with two pads, and
`./kit fn 0x00169ac0` makes one 0x738-byte `PSXControl` per pad (`PSXControl.cpp`) and then opens all
eight port/slot pairs with `scePadPortOpen`, buffers 0x100 apart starting at 0x47a980. The manager keeps
the devices at +0xc and +0x10 and their count at +0x664.

**The poll.** `./kit fn 0x0016ab10` is `PSXControl::Poll` (device vtable +0x6c). It:

1. copies last frame's held flags into a stack array and clears all 16 (`+0x328 + 0x18*n`);
2. `scePadRead` into a 32-byte stack buffer; a zero return or a non-zero status byte means "no pad", and
   every axis is forced to its centre and the frame ends there;
3. computes the button word once: `(buf[2] << 8 | buf[3]) ^ 0xffff` — libpad's word is active low;
4. walks the 16-entry map at device+0x14 (0x10 bytes each: flags byte, pressure-byte index at +4, u16
   mask at +8). Flags bit 1 means analogue: the control is held when its pressure byte is above 0x19,
   with strength `(byte + 1) * 0.00390625`. Otherwise it is held when its mask meets the button word,
   with strength 1.0;
5. compares each control against last frame's flag. Unchanged: the matching timer (`+0x334` held,
   `+0x330` released) takes `dt * 1000` and its old value is kept beside it (`+0x33c`, `+0x338`).
   Changed: the *other* timer pair is zeroed and `./kit fn 0x0011d860` appends an event.

So the press edge is zero-width: it exists as the event, and as the fact that the held timer reads 0.
The timers are accumulated as float and stored as int, which is why an untouched control's
"ms since release" sticks at a large garbage value — `(float)665190464 + 33` is `665190464`.

**The event.** `./kit fn 0x0011d860` writes 0x18 bytes at manager+0x48 + 0x18*n and bumps the count at
manager+0x44: control index, type 2, device id, device slot, 1 for a press / 0 for a release. At the end
of the same Update `./kit fn 0x0011dab0` walks the queue and calls app dispatch slot 0x50 for a press and
0x4c for a release, handing over the event and the control's own state. Nothing survives to the next
frame but the timers.

A second reader, `./kit fn 0x0016b1d0`, reads the same pad into a 16-deep ring of (axis, timestamp)
samples at device+0x20 — a finer-grained history for the sim. `./kit fn 0x0011d3d0` is the single-axis
read with the dead zone and the clamp curve, which is I2's job, not this one.

The 16 controls, in order, with the bit and the pressure byte each comes from (the map, read live):

| # | button | mask | pressure byte | | # | button | mask | pressure byte |
|---|---|---|---|---|---|---|---|---|
| 0 | triangle | 0x0010 | 12 | | 8 | select | 0x0100 | — (digital) |
| 1 | circle | 0x0020 | 13 | | 9 | start | 0x0800 | — |
| 2 | cross | 0x0040 | 14 | | 10 | R3 | 0x0400 | — |
| 3 | square | 0x0080 | 15 | | 11 | L3 | 0x0200 | — |
| 4 | L1 | 0x0004 | 16 | | 12 | up | 0x1000 | 10 |
| 5 | R1 | 0x0008 | 17 | | 13 | right | 0x2000 | 8 |
| 6 | L2 | 0x0001 | 18 | | 14 | down | 0x4000 | 11 |
| 7 | R2 | 0x0002 | 19 | | 15 | left | 0x8000 | 9 |

## How it was proved

Three measurements on copy 2, all from save state 3 (the Training dialog: two entries, so every press
moves the selection and the move is visible).

**1. The pad reaches the game, by eye.** `press down 120` between two `./kit shot`s moves the highlight
from *No* to *Yes* (`context/shots/c5-dialog-before.png`, `c5-dialog-after-down.png`), and the two words
that mark the selected entry swap in the same moment.

**2. The button word, one button at a time.** 15 of the 16 bits were measured directly: load state 3,
hold one button, read libpad's buffer, release, reload. Every bit came out as the standard libpad layout,
and every one matched the mask the game's own control map holds for that button — two independent
sources agreeing. Select is the exception: `./kit pad` refuses to send it (it is PCSX2's hotkey
modifier), so its 0x0100 is from the map only. A second sweep held each button and read the control
states: each one lit exactly the control the map predicts, with strength 1.0.

**3. The capture.** `./kit record watch/pad.toml --state 3 --pad watch/pads/menu-down-up.pad -n 300`
→ `context/captures/pad-menu.csv`, 300 rows, frames 13138..13437, `skipped 0, gaps 0, suspect false`,
29.1 rows/s against a 30 Hz tick. `context/captures/check-pad.py` runs 25 checks and all 25 pass:

- one row per frame, and `frame_counter` equals the poll column in all 300;
- the status byte is 0 and the mode byte 0x79 (DualShock 2 with pressures) in all 300; the sticks never
  leave 0x7f; no button but up and down ever appears in the word;
- `ctl12.held` / `ctl14.held` equal "the up/down bit is clear" in all 300 rows, and equal
  "pressure byte > 0x19" in all 300, and the strength equals `(pressure + 1) / 256` wherever held;
- 14 transitions (7 presses, 7 releases). Every transition frame carries exactly one event, naming that
  control and that direction; no other frame carries an event at all;
- the held timer is 0 on every press frame, the release timer is 0 on every release frame, and the held
  timer grows by exactly 33 a frame while held;
- exactly one of the dialog's two entries is selected in every row, and the pair swaps exactly 7 times —
  once per press, on the press frame itself, never on a release.

**The half-stamp trap, which cost the most time here.** libpad's port buffer is two 0x80-byte halves and
`scePadRead` takes the half whose stamp at +0x58 is the larger. A PINE row is sampled after the tick, and
by then the IOP has refreshed the *other* half: `stamp_a - stamp_b` is 1 in all 300 rows, and the control
state agrees with the **older** half in 300 of 300 rows and with the newer one in 297. The two halves
carry different data in 6 of the 300 rows — the frames where a press began or ended between two field
refreshes. The stamps are also the two words earlier tasks met: C4's `@field_counter` is half A's stamp,
and the word `./kit probe vsync` picks for this game is half B's.

**Why the heap addresses are usable as symbols.** The manager and both `PSXControl`s are heap objects,
but they land at the same addresses in two unrelated boots: C4's slot-9 RAM image
(`context/states/*.09.eeMemory.bin`, taken in another session on another copy) has the manager pointer at
app+0x104 reading 0x678550, the devices at 0x678be0 / 0x679440 and the map at 0x679330 — the same values
this session's live read gives. They are allocated at a fixed point in boot from a deterministic pool.

The `[symbols]` to paste into another worktree's `kit.toml`:

```toml
pad_buffers          = "0x0047a980"   pad0_buttons        = "0x0047a982"
pad0_stamp_a         = "0x0047a9d8"   pad0_stamp_b        = "0x0047aa58"
pad_port_table       = "0x004e9390"   input_manager_ptr   = "0x00479a94"
input_manager        = "0x00678550"   input_event_count   = "0x00678594"
input_events         = "0x00678598"   pad0_control        = "0x00678be0"
pad1_control         = "0x00679440"   pad0_control_map    = "0x00679330"
pad0_control_state   = "0x00678f08"   pad1_control_state  = "0x00679768"
input_tick           = "0x00169c40"   pad_poll            = "0x0016ab10"
pad_poll_history     = "0x0016b1d0"   pad_axis_read       = "0x0011d3d0"
input_queue_event    = "0x0011d860"   input_dispatch      = "0x0011dab0"
input_create_pads    = "0x00169ac0"   input_manager_create= "0x00130410"
pad_control_ctor     = "0x0016a870"
```

## Not verified / not 1:1

- **Select's bit (0x0100) was not measured.** `./kit pad` refuses to send select, so that one bit comes
  from the game's own control map (entry 8) and from the standard libpad layout, not from a live press.
  Every other bit was measured twice, so the risk is small, but it is not zero.
- **Only one pad was exercised.** The game creates two `PSXControl`s and opens all eight port/slot pairs;
  everything here was measured on port 0 slot 0. `@pad1_control` and `@pad1_control_state` are the
  addresses the second device sits at, not verified against a second controller, and nothing here says
  what happens to the event queue when both pads press in the same frame.
- **No multitap, and no disconnect.** The "no pad" branch of the poll (status byte non-zero, or
  `scePadRead` returning 0) was never taken: the virtual pad is always present. The `+0x6dc` connected
  flag read 1 throughout, so the code that centres every axis when a pad goes away is read-only
  knowledge.
- **The analogue sticks are untouched by this task.** They sat at 0x7f in every row of every measurement.
  The dead zone, the clamp curve and the 16-deep sample ring are I2's, and nothing here proves them.
- **The pressure threshold was proved only at the extremes.** The virtual pad reports 0 or 255, so
  "held above 0x19" was exercised with 0 and 255 and never near the boundary; the exact comparison
  (`> 0x19`, not `>=`) is from the decompilation alone.
- **The press-to-selection link is a correlation, not a traced call.** The capture shows the selection
  flipping on the frame of the press event, seven times out of seven, and `@input_dispatch` calling app
  slots 0x50 / 0x4c explains it — but I did not follow the dispatch into the front end to see which page
  object consumes it. The two words the capture watches (0xc20358 / 0xc20438) are the dialog's entries in
  *that save state*, found by `./kit probe find`, and nothing names the object they belong to.
- **Menu auto-repeat was never triggered.** The presses are 120 ms (3–4 frames); a held direction may
  well repeat through a timer somewhere in the UI, and this capture cannot see it.
- **`events` is only ever 0 or 1 in this capture.** Nothing proves the queue's behaviour when several
  controls change in one frame, and I did not find its capacity.
- **The timer arithmetic is float, and only the 30 Hz case was seen.** `dt * 1000` came out as exactly 33
  per frame in all 300 rows because the engine's `dt` is clamped to 0.03333 (C4). What the timers do when
  the cap is off is not covered.
- **`./kit done` ran with `--skip-audit`, and here is why.** `./kit audit` fails on this tree, but on
  neither of C5's files: both leaks are in `plan/TODO.md` (lines 31 and 91, `0x00330460` and
  `0x002f9b70`), committed by another task before this branch existed (`git log -1 -- plan/TODO.md` →
  c6e068d) and untouched by me. `plan/TODO.md` is the file the runner and every parallel agent edit, so
  fixing it from this worktree would have bought a merge conflict for a problem that is not C5's. It
  needs doing — R7 and S13 should name those two activity classes instead of numbering them — by whoever
  owns those lines. Nothing C5 commits contains a retail address: `./kit audit` reports no leak in
  `docs/PCSX2.md`, `watch/pad.toml`, `watch/pads/menu-down-up.pad` or this journal, and the addresses in
  this entry are in `kit.toml`, which is git-ignored.
- **The frame counter is not the pad's clock.** The capture holds one row per *engine* frame; libpad
  refreshes per field, twice as often. Everything above is stated at engine-frame resolution.

## Next

I1 (the port's input layer) can be written against this: 16 controls in the order above, held + strength
+ the four timers per control, the transition rule (zero the other timer pair, raise an event), and the
rule that a press is visible for exactly one frame. The one thing I1 must decide that this task could not
answer is what a port does with the event *queue* when more than one control changes in a frame — the
original appends in control order, device by device, but no capture here shows two at once.
