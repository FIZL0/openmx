# 2 — p38 join and leave, wired (FINAL)

Task P38. 2026-10-29.

## What I found

Entry 1 blocked on P35 (seats) and P36 (front end). By the time this run picked P38 up, the wiring the
block asked for had been done by later tasks, each building on `join.rs`:

- **P43** put `join::Join` on the main menu as Local Game's `Screen::Join`, keyed by P35's pad seat
  number, with `Menu::present` fed every frame from main.rs (`seat_dev`), so a pulled pad's seat goes
  `Gone` and comes back `Taken`; `menu_screens.rs` draws a gone card as "Gone: plug it back in".
- **P51/P171** made Start and the views follow the count; `Party::layout()` is V22's split for 1–4
  (`menu::layouts`), quarters/eighths past two.
- **P76** joins and leaves a running ride (Cross to drop in, the pause's Leave Session).
- **P210** added the Controllers screen (`join::Assign`, main menu and pause) over `pad.rs`'s
  `Seats::assigned`/`assign`, where a waiting seat is still its pad's.

So of the brief, two things were left: `tests/menu.rs` had join and move by pad input but no **leave**
and no **seat kept across an unplug and back**; and on the Controllers screen another device's
Left/Right could **swap a gone pad off its seat** (`Assign::move_to` swaps whoever is in the column),
so the pad would not come back to its own seat.

## What I did

- `join.rs`, `Assign::press`: Left/Right step to the nearest column that way that is free to step into
  (`free_to`: no seat, an empty seat, or one whose device is plugged in), skipping a seat kept for an
  unplugged device. Swaps with plugged devices stay as P210 made them.
- `tests/menu.rs`:
  - `pads_join_move_and_leave_and_an_unplugged_one_keeps_its_seat` — Local Game by pad input through
    `front_frame`/`tap_device`: Cross joins, Right/Left move to the nearest open seat (past a taken
    one, never swapping), Circle leaves and the screen stays; pad 2 out → `Gone(2)`, still a view,
    labelled "- gone"; two other pads' Cross fill the open seats around it, a Right steps past it; pad 2
    back → `Taken(2)` in its own seat; Start goes on with three riders.
  - `a_pad_that_drops_mid_ride_is_shown_gone_and_comes_back_to_its_own_seat` — P210's Controllers rig
    (real fifo pads through `pad::Seats`): pad B unplugged mid-ride, 3P waits; the Controllers screen
    has it on 3P, unplugged, not a loose card; pad A's hat right goes past it to 4P and left past it to
    2P; B plugged back is on 3P and its Cross rides 3P.

**Taken from opentennis**: nothing new this entry (entry 1 and P210 took `assign.rs`). opentennis frees
an unplugged device's seat; here it is kept, as P35's `PadSlots` keeps it.

## What the original does

Not read: a front-end addition ("Menus and front-end: yours to design").

## How it was proved

`./kit check -- -p openmx --test menu -- pads_join_move_and_leave`: passed; `-- a_pad_that_drops`:
passed. The skip is what the second test's "onto 4P" assertion checks: without it `move_to` swaps A
and B, putting B on 2P (reasoned from `move_to`, not run as a mutation).

## Not verified / not 1:1

- **Eight on one screen**: the join screen is `ON_SCREEN` (4) seats; the stretch to eight is
  `join::MOST` and the Controllers screen's, not exercised on the join screen here — the brief ties it
  to the stretch task, which this run did not do.
- **The split screen following a mid-ride join/leave** is P76's and its tests'; not re-tested here.
- **Not seen by eye or with a real pad**: the tests drive fake fifo pads and button words; no physical
  controller was unplugged in a running shell from this slot.
- **The skip rule's mutation** was reasoned, not run (see above).
- **The gate's first two runs were red on tests this task does not touch**: `original_mode` with no display exported (rerun with `DISPLAY=:0`), then `web.rs::the_browser_rides_on_the_gpu` (webgpu page `state: failed` at load ~50, passed alone), filed as K10.
