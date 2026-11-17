# 1 — drag cards on web and ios (FINAL)

Task F38. 2026-11-16.

## What I did

- Found the plumbing the task asks for already on main, landed by P319 (505bddd02) for the Tricks camera's drag:
  - web/index.html posts `{ mouse: [kind, x, y] }` for pointer down/move/up/cancel (moves only while pressed,
    with pointer capture); web/worker.js queues them and hands them over as `mouse()`.
  - web_host.rs decodes them into `MouseEvent`s scaled to the canvas, as it does the clicks.
  - ios_gpu.rs feeds every touch through touch_pad.rs, whose `take_drags` gives Down/Move/Up for a finger that
    landed on the menus (pad hidden, or off its buttons and stick); `mouse()` returns them in the kept-insets
    room, the same space as the taps.
  - main.rs's `drag_cards` reads `window.mouse()` on both the shell menu and the pause's Controllers screen, and
    screen.rs routes the GPU window's (web, iOS, Android) `mouse()` there. So menu_drag.rs's `Menu::mouse` gets
    the drags on every platform.
- Nothing was left to wire, so the change is a test: touch_pad.rs `a_finger_on_the_menus_drags` — a menu finger
  gives Down, Moves, Up (Cancel too lets go) and still clicks once; fingers on a button or the stick drag
  nothing; with the pad hidden any finger drags.
- opentennis: not looked at; this was plumbing already in place here.

## What the original does

Not applicable: the menus are ours (front-end).

## How it was proved

- Unit test `a_finger_on_the_menus_drags` (crates/openmx/src/touch_pad.rs) for the iPhone path's touch → drag.
- Read-through of the web path (index.html → worker.js → web_host.rs → screen.rs → main.rs `drag_cards`); the
  drag logic itself is P214's tests in tests/menu.rs.

## Not verified / not 1:1

- A card was not dragged by hand in a real browser: no headless browser test drives a pointer drag on the
  Controllers screen; the web path is proved by reading, not running.
- Not dragged on an iPhone: no phone here in an unattended run; only touch_pad.rs's part is tested, not
  winit's touch delivery or ios_gpu.rs's inset shift.
- A tap on the Controllers screen is both a click and a drag Down, as on the desktop (gpuwin.rs does the same);
  not checked that the click does nothing unwanted there on a phone.

## Next

None.
