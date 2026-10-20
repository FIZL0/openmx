# 1 — s30 the humans tricks (FINAL)

Task S30. 2026-10-20.

## What I did

- Captured recording2's playback with C6's watch spec into `context/capture/acceptance/recording2-tricks.csv`
  (1894 rows, counters 2374..4267). The test takes C6's `recording2-freeride.csv` first if it is there and
  has the trick columns; both are the same spec.
- `the_humans_tricks` (crates/game-sim/tests/trick.rs) runs the tape through `Gesture::read`,
  `Tricks::request`/`idle`, `Tricks::update` and `Tricks::landing`, driven by the capture's `on_ground`,
  `wreck_reason` and `fresh` columns, and compares nine trick columns on every row.
- Two fixes in `trick.rs`, both found by the comparison:
  1. **A wreck leaves the direction alone.** The original's gesture reader, while the rider is wrecking,
     zeroes the stick, the group and the armed flag and nothing else: the direction word (and the look,
     and the d-pad-axis flags) keep the last driven frame's value. The port cleared them. Visible on
     recording2 as `trick_dir` holding through rows 641-763 and 1750-1828.
  2. **The stunt clocks truncate.** `single()` parsed the table's durations with Rust's round-to-nearest;
     the loader reads them through the INI reader's float-with-default getter, which is `strtod` and then the
     EE's double-to-float routine — the same two steps as a `.phy` number (`phy::number`), and that routine
     truncates. `1.33` came out one ulp high, so a trick's elapsed clamp sat one bit above the original's on
     its last tick. It now calls `phy::number`. Every clock in the table moves with it where the decimal is not exact — the no-hander's `0.33` hold
     is one bit lower too, and S11's table test now pins that.
- Opened **P20** for the two ways `rider.rs` drives the machine differently from the original (below).
- Listed every gesture the human made and proved the shell's keyboard makes each one: no P task for the
  keyboard was needed.

### The capture recipe

Paused PCSX2 (copy 5), opened recording2 with Play through the accessibility bus (the Qt menu item presses
fine there), which leaves the emulator paused on the save state's counter 2373; `./kit record` with the
start gate, quarter speed (`NominalScalar` 0.25 — restored to 1.0 after), `KIT_FRAME_DELAY=0.04`,
`KIT_FRAME_STALL=120`, and the vsync address the playback's build uses set in kit.toml for the run and
restored after. Half speed left stale rows; quarter did not.

### What the capture showed about the caller

The trick machine itself matched once these were modelled; each is the order the test calls it in:

- **The pad is field `2k - 2`.** Row k's pad bytes (buttons and both sticks) equal the recording's video
  field 2k-2 on all 1893 rows that have one — `TickPhase::First`, tick k-1. (S8's first recording read the
  second field; this playback read the first. Which field a playback reads is the playback's, not the game's.)
- **The request runs on the gesture read the tick before.** On every trick, `trick_req` turns over one row
  after the group and direction appear (169, 277, 926, 1148, 1379, 1553). So per tick: request/idle, the
  clocks, the landing test, and the pad last.
- **The stick is the pad's own.** Through `input::Manager` and `Controls::from_pad`, but not the
  `[Joystick]` rate limit (`ride::Held`): with it, `trick_dir` leaves the original's on many more rows.
- **A wreck keeps the trick.** Row 1188 lands slot 0 unsafe (elapsed 1.367 < safe 1.5): wreck 12, and
  `trick_running` stays 1 through the wreck until row 1212, when the rider is back on without a respawn and
  the trick is cleared. A respawn (`fresh` 0→1, rows 764 and 1829) still reads as wrecked on its own tick.

`rider.rs` does neither of the last two: it reads the gesture before the request on the same tick (every
trick one tick early) and resets the tricks when a wreck starts. Not changed here — the brief's files are
trick.rs and its test — and filed as P20.

### The index

| row requested | gesture | slot | stunt | first safe | over |
|---|---|---|---|---|---|
| 169 | group 1 up | 0 | Taunt | 214 | 219 |
| 277 | group 1 left | 2 | cancan | 316 | 322 |
| 926 | group 1 down | 1 | lazyboy | 1008 | 1012 |
| 1148 | group 1 up | 0 | Taunt | never — wreck 12 at 1188 | 1212 (back on) |
| 1379 | group 4 right | 15 | stalelin | 1429 | 1437 |
| 1553 | group 1 left | 2 | cancan | 1599 | 1605 |

Nothing queued (no gesture arrived while a trick was in its overlap window). No backflip. The idle request
asks for the wheelie (slot 25) for one tick on 19 rows (87, 137, 220, …, 1711); the port does too, on the
same rows. Landing: the trick clause says wreck 12 at row 1188 and nowhere else; the original's other two
wrecks (reason 8 at 641, reason 6 at 1750) are not the trick's, and the clause says nothing there.

### The gestures and the keyboard

Every (group, direction) the gesture held with a group down, and on how many ticks:

| group | buttons | direction | ticks | slot |
|---|---|---|---|---|
| 1 | Circle | up | 20 | 0 |
| 1 | Circle | down | 2 | 1 |
| 1 | Circle | left | 50 | 2 |
| 1 | Circle | right | 1 | 3 |
| 4 | Circle + R1 | up | 13 | 12 |
| 4 | Circle + R1 | down | 67 | 13 |
| 4 | Circle + R1 | right | 24 | 15 |

The d-pad was never down. The keyboard (`keyboard()`, crates/openmx/src/pad.rs) binds no d-pad, but its
arrows are the left stick ramped to the stop (0 or 255) and Circle (C), Triangle (V) and R1 (D) are keys of
their own, so the test builds each gesture's report the way the keyboard would end up sending it — the
group's buttons and the stick at the stop — and reads it through the same input layer and `Gesture::read`:
all seven name the same group and direction. **The keyboard can make every gesture the human made; no P
task opened for it.**

## What the original does

The gesture reader (`./kit fn 0x250fb0`), when the rider's wreck word is clear, takes the stick, lets each
d-pad axis override it, classifies the stick into angle, reach and direction, and decodes the three trick
buttons into the group (0 arms the next trick). When the wreck word is set it zeroes the stick, the two
other analogue words beside it, the group and the armed flag — and leaves the direction. The stunt table's
clocks come in through `./kit fn 0x14b790` (strtod, then the truncating double-to-float).

## How it was proved

A PINE recording compared frame by frame: `the_humans_tricks` over 1894 rows of recording2's playback, all
nine trick columns (`trick_group`, `trick_dir`, `trick_armed`, `trick_req`, `trick_id`, `trick_count`,
`trick_running`, `trick_safe`, `trick_elapsed` by bits) on every row, with one difference allowed by name
(below). The pad alignment is asserted on every row; the six trick slots are pinned in order; the landing
verdict is asserted on every touchdown and wreck entry; the keyboard check runs every gesture through the
input layer. Before the two trick.rs fixes the elapsed column was off by a bit on the last tick of every
cancan and the direction differed through both long wrecks.

## Not verified / not 1:1

- **Twelve `trick_dir` rows are not explained** (115, 135, 242, 543, 562, 568, 853, 888, 1291, 1317, 1338,
  1605). On each the stick is moving through about half way (the port's reach 0.52–0.56, just over the 0.5
  ring) and the original reads no direction where the port reads one. No trick button is down on any of
  them, so no slot differs and no trick is named differently. Ruled out: the joystick rate limit at any
  rate from 4/s to 30/s, the previous tick's stick, the tick's other field, the mean of the two fields.
  The capture has no stick column, so what the original's reader was handed on those ticks is not known;
  the stick is passed into the reader by its caller (`./kit fn 0x27ff90`'s dispatcher), which was not
  traced. The test allows exactly this shape (original dir none, group 0, stick bytes moving) and at most
  twelve of them.
- **`contact` is driven as false.** The capture has no collision-list column; the landing clause's contact
  input was never exercised against the original here.
- **Not captured, so not compared**: the queue's contents (only its effect on `trick_req`), the hold and
  wiggle clocks, the progress word, the no-hander fields. The landing's attitude clauses are the
  bike-in-the-air task's and were not exercised.
- **The stunt map is assumed to be player 0's default** (`stunts.selection(0)`), as the freeride boot
  leaves it; the capture has no column for the map.
- **Malformed table text** is still an error in the port where the original's `strtod` would read 0. The
  disc's table has none.
- **The keyboard check is the report the keyboard ends at, not the keyboard.** It does not drive
  `keyboard()`'s ramp in the shell: whether a human gets the stick past half way (about four ticks of
  ramp) while holding the group, and whether three keys at once ghost on a given keyboard, are not tested.
- **One recording.** Groups 2, 3, 5 and 6, the backflip, the queue and the overlap take-over were not in it.
- **Two captures of the same playback differ** in `rng_state` from row 2 and in the bike's position at about
  1e-3. A playback is not deterministic in the columns C6 and
  S32 will read. Not chased (not this task's).
- **`rider.rs`'s order and wreck reset differ from the original** (above): filed as P20, not fixed here.

## Next

P20: put the order this test uses into `rider.rs`.
