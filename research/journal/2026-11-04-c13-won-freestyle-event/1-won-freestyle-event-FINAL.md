# 1 — won freestyle event (FINAL)

Task C13. 2026-11-04.

## What I did

- Built a fresh zone state, because C9's slot 9 had been overwritten:
  - loaded first_recording's state into copy 3's slot 8;
  - ran `research/s76_tele.py <pid> 3400 193 3348 2 9`;
  - saved slot 9 at frame 2922, with the "Would you like to begin a Stunt Competition?" prompt up.

  It has no brake hold, because `/dev/uinput` is root-only again. The bike rolls about 0.8 ft/s.
- Pressed the buttons with a generated input recording rather than the pad (`capture/make_tape.py`):
  - Circle on fields 40..79;
  - Cross on 150..189, 250..289, 350..389 and 450..489;
  - nothing otherwise.

  The tape and its state are `context/c13/c13.p2m2` and `c13.p2m2_SaveState.p2s`.
- Wrote `capture/record_poke.py`. It runs `./kit record`'s own `record()` with one PINE write, on
  the recorder's connection. Once the event clock (manager +0xf8) reaches AT with one player on the
  list, it writes VALUE over the card's total (card +0xdc).
- `capture/play.sh` runs one capture: C6's playback sequence plus 12 screenshots over AT-SPI (one a
  second) after the write. The spec is `capture/c13-event.toml`: C9's fields plus three:
  - `p0_warn` and `p0_warn2`: the warning HUD's `+0xb0 → +0x54 / +0x58`;
  - `p0_ann_queued`: the player entry's announcer `+0x64 → +0xac`.
- The captures are in `context/capture/c13/` (slot s3 only; git-ignored). Each has a `.poke` line and
  the screenshots are in `shots/`:

  | run | write | clock at the write | frames | torn | gaps |
  |---|---|---|---|---|---|
  | run1 | 123456.0 | 117.0125 | 4600 | 0 | 1 |
  | run2 | 100000.5 | 60.0260 | 4600 | 1 | 9 |

  run2's largest gap is 12 ticks, at clock 70.6.
- Added `a_won_live_event_runs_like_the_port` and its helper `follow` to `tests/event.rs`. C9's test
  is unchanged.
- Added a C13 block to `research/oracle.txt`, in comment lines only.

## What the original does

**The warning's RAM trace.**
- The activity's frame (`./kit fn 0x3ddf70`) calls `0x366680(hud, on)` on the player entry's +0x58
  HUD.
- That function calls the vtable of `*(hud + 0xb0)` (`./kit dis 0x366680`). Slot 0x50 is
  `sw a1, 0x58(a0)` and slot 0x54 is `sw a1, 0x54(a0)`. Both get `on`.
- So `*(hud+0xb0)+0x54` is 1 while the warning is up, and +0x58 is the same word. The two are equal
  on every row.
- With one rider, the call runs only while the latch is clear. It passes
  `on = !(left == 0 || 5 <= left || target <= total)`, as S92 ported.

**The latch.** On the first frame whose total reaches the target, with the latch clear and one human,
the frame does two things:
- calls `0x361730(0, 3.0, announcer, 5, player, 0)`, which queues a message. Announcer +0xac counts
  the queue and goes 0 → 1 on that row.
- sets the entry's +0xc0 = 1.

The same frame has already called the warning off. "Objective Complete!" was on screen right after
the write in both runs.

**The end** (`./kit fn 0x3dd870`):
- Clears the latch (+0xc0 = 0, +0x720 = 0, +0xcc = 1) before the verdict. run1 reads one row inside
  the end's tick: latch 0, ended flags set, rider still on the list, clock not yet stepped.
- A Win needs:
  - the total **above** the target, with the rider the leader and alone; or
  - with two riders, being the leader.

  It is announced as message 6; a loss is message 7. The queue went up at the end in both runs.
- On a win it sets the UI string `STR_FREESTYLE_STUNT_WIN` to "You Win!" (to "Tie!" on a tie), and
  sends "activity win" or "activity lose" through the activity's +0x20.
- Outside a career it then shows "To complete this Stunt Activity you must be in a career!". The
  first_recording state is not in a career. run1's shots show "You Win!" under that dialog.

**The event's card is the freeride card itself.** The manager's +0x34 card is 0x1c833c0, and C9's
`free_total` is its +0xdc. The write therefore shows in both columns. This is how the join "puts
the total aside" and the end puts it back.

**The HUD's SCORE stays 0** after the write, while the card's total is past the target. It showed
123456 only after the end. The HUD reads a points word of its own, which the write does not touch.

## How it was proved

A PINE recording, compared row by row against `Competition`. The spec and scripts are in `capture/`.
Floats are read as u32 and compared bit for bit, over 15 words: C9's 14 and the warning.

The rider sits still, so each row's total is either 0 or the written value; the test asserts this.
Because of that, the port's tick t can read `0` before some tick p and the written value from p on.
The test fixes p as the tick after the one that made the clock of the first row showing the written
total. That row's latch is still 0, and the frame runs before the clock steps. With p fixed:
- every row up to the end must be the port after some tick, never going back, at most 30 ticks
  ahead (C9's limit was 4; run2 lost 12 ticks at once). The clock moves every tick, so a row can
  match only one tick;
- a row read mid-tick may mix tick k's words with tick k+1's.

Results:

| run | rows | event ticks | p | p's that also fit | rows on a tick | read mid-tick | warning up |
|---|---|---|---|---|---|---|---|
| run1 | 3758 | 3703 | 3613 | 3612, 3614 | 3758 | 0 | 60 ticks (rows 115.05 → 117.01) |
| run2 | 3737 | 3703 | 1903 | 1902, 1904 | 3737 | 0 | 0 |

The other p's fit because of the gap rows: a row two ticks on hides which tick latched.

Then the test checks:
- tick p sets the latch and takes the warning down;
- the warning before the write is the row's;
- the announcer queue rises on the latch's row and on the end's;
- on the row where the rider is gone, the manager's flags and clock are the port's ended tick;
- `finish` gives **Win**, and the card ends at the original's freeride total (123456.0, 100000.5).

Mutation: `WARNING = 4.0` in event.rs fails run1 at frame 6560, the warning's first row.

`./kit check -- -p game-sim --test event -- live_event`: 2 passed (C9's test and this one).

## Not verified / not 1:1

- **Score from riding over the target is not covered.** The total was written over PINE. Each
  capture has only two totals, so the test never checks a total that changes from tick to tick, or
  one that crosses the target between two PINE reads.
- **Which tick latched is fixed by reasoning, not by the rows alone.** The rows also fit one tick
  either side, because gap rows two ticks on hide the latch's tick. The test takes the tick after
  the write row's clock, as explained above.
- **The announcer's messages are only counted.** Their ids (5 reached, 6 win) come from the
  decompilation. The capture shows only the queue count rising on those rows, plus the on-screen
  text. The port has no announcer: the shell is meant to speak from `Seat::reached` and the verdict.
- **The end's latch clear is not in the port.** `Competition::finish` leaves `Seat::reached` set; the
  original clears entry +0xc0 (and +0x720, +0xcc) in the end. Nothing after the end reads it, since the
  seat goes with the rider, so the comparison stops at that row.
- **The win's UI string and "activity win" were not read from RAM.** They come from the
  decompilation, and "You Win!" was seen on screen.
- **The career dialog after a win is not in the port.** The state is not in a career, so the
  original shows "you must be in a career" (and gives no track unlock). The port's `finish` says
  nothing about careers. The shell decides what a win unlocks.
- **The profile's points** (`finish`'s second value: 123456, and 100000 for 100000.5) were not read.
  The profile's address is still unknown, as in C9.
- **The HUD's own score** (0 until the end) is a different word from the card's total. It was not
  traced.
- **Tie and two riders** are still not captured live; this was one rider each time.
- **The heap addresses are first_recording's**, as in C9. The zone state was re-made with s76_tele.py,
  since C9's slot 9 is gone. Rebuild it as described under "What I did".

## Next

Done.

To capture again:
1. Have copy N running with `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1 WAYLAND_DISPLAY=wayland-1`.
2. Make the zone state and the tape (see "What I did").
3. Run `./kit pcsx2 with -- bash capture/play.sh <pid> context/c13/c13.p2m2 context/capture/c13/runN.csv 4600 <AT> <VALUE>`.
