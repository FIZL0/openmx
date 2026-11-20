# 1 — dual stick lean (PART)

Task P386. 2026-11-20.

## What I did

- `pad::Sticks` (Single, Dual): a remap of a seat's report after its layout's (`Scheme::ride_tricks`) and
  before the ride sees it, the way the layouts work. Under Dual:
  - the left stick's y byte (the original's lean fore/aft) is the right stick's y;
  - the left stick's x byte (the original's steer *and* side lean — one field) is whichever of the left
    and right sticks' x is further from centre, the left on a tie;
  - the right stick's two bytes go to centre, so the camera's look-around (which reads them) does not turn;
  - R3, the camera reset under a single stick, is let go and put on the control the seat's table reads
    look-back off (L2 on the ride's side of every preset's table — Square's place under the new layout).
    That is dual sticks' own look-back button (MXA's LookBackDualStick idea); the layout's own look-back
    button keeps working.
- Settings: `dual_sticks` per seat, `sticks = single dual …` in settings.txt (written only when a seat is
  dual), a Sticks row ("Single Stick" / "Dual Stick") on the Controls tab after Preload Axis, per seat like
  Markers. menu.rs cannot name `crate::pad`, so it keeps a bool and main.rs picks `pad::Sticks`.
- main.rs: read live each frame like Markers and Preload Axis. Not in a biplane or the Kiowa (their stick
  is the flight stick), and only for a seat whose device is a game pad (`Seats::assigned`): the keyboard's
  lean keys write the left stick's y, so a keyboard seat keeps the single stick. No R3 look-back on a ride with MX vs. ATV's extra tricks (their L2/R2 are tricks).
- Online: the remap is applied to the local pad before it goes out (local players are world.seats[0..local]
  and their settings index is the same), so peers get the remapped report; nothing joins the handshake.
- opentennis: nothing like it there (a tennis game's sticks), nothing taken.

## What the original does

One stick: `Controls::through` reads the left stick's x and y (report bytes 6 and 7) as the ride's
`stick`, x both steering and leaning sideways. The right stick (bytes 4, 5) is the camera's look-around
only. Nothing in the sim changes; the remap only moves bytes.

## How it was proved

- `pad::tests::dual_sticks_lean_on_the_right_stick_as_the_single_stick_does`: nine bytes across each axis,
  through the ported poll (`Manager::tick`) and `Controls::from_pad` — the right stick under Dual gives the
  ride exactly the `stick` the left stick gives under Single; the camera axes stay centred; left x still
  steers; left y no longer leans; the further-out rule and its tie.
- `pad::tests::dual_sticks_look_back_on_r3_and_leave_the_online_bytes_alone`: R3 holds look-back through
  the shipped table; bytes 20.. (where online writes `net::SWITCH` and on) are untouched.
- `tests/menu.rs::each_seat_picks_single_or_dual_sticks_on_the_controls_tab`: the row, per seat, the file.

## Not verified / not 1:1

- MXA's own dual-stick behaviour is not checked: context/mxa (MxaBindings.cs, NOTES.md) is not on this
  machine (see MD259). The mapping here is my design from the task's words; in particular how MXA mixes
  steer and side lean, and which button its LookBackDualStick is, are unknown. R3 was picked because the
  right stick's click is freed once the stick no longer turns the camera.
- Steer and side lean share one input field in the original, so dual sticks cannot steer and side-lean
  independently: the further-out stick wins. A blend (sum, clamp) was not tried.
- Not played by hand on a real pad: no pad on this machine for an unattended run. The remap is covered
  through the poll in tests only.
- Online not run end to end with two machines on different Sticks settings; the claim that it works rests on
  the remap happening before the online path takes `world.seats[k].reports[0]` (main.rs).
- A Preload Axis on the right stick (P367) reads nothing under dual sticks, since the ride sees the right
  stick centred. Left as is.
- A seat with a pad *and* the keyboard folded into its report is the pad's (`Seats::assigned`), so there
  the keyboard's I/K lean is dropped under dual sticks. Not handled; dual sticks is a pad setting.
- The Original switch (Departures) does not touch Sticks: it is already the original's (Single) by default.

## Next

-
