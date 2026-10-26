# 1 — hide the cursor in the ride (FINAL)

Task P167. 2026-10-26.

## What I did

- `crates/openmx/src/cursor.rs` (new): the decision, `shown(Showing, focused)`. `Showing::Front` (front-end
  screens and the loads) always shows it; `Showing::Ride(menu)` hides it only when the window has the focus
  and the front end's `Menu::riding()` holds — no 1P menu and no other seat's pause (P84) open. A ride
  without the front end (`--ride`) has no menu, so it hides whenever focused. The pointer is never read, so a
  moved mouse in a ride does not bring the cursor back.
- `main.rs`: the decision applied after every `poll()` in the three loops that own the window: the front
  end's, the loading screen's (both `Front`) and the ride's (`Ride(front's menu)`). Per frame, so a pause opened
  this frame shows the cursor on the next one.
- `screen.rs`: `Screen::focused()` and `Screen::show_cursor()`, which tells the window only on a change, and a
  `Drop` that shows the cursor again when the shell lets the window go (quit).
- `gpuwin.rs` (winit, X11 and Wayland, windowed and fullscreen): `Window::set_cursor_visible`, and the focus
  kept from `WindowEvent::Focused`.
- `window.rs` (the software renderer's own X11 client): the core protocol has no hidden cursor and this
  module makes no reply-bearing requests, so the hidden cursor is a 1×1 depth-1 pixmap cleared by a
  PolyFillRectangle with a foreground-0 GC, made into a cursor that is its own mask (CreatePixmap, CreateGC,
  PolyFillRectangle, CreateCursor, FreeGC, FreePixmap; ids `id_base | 3..5`), put on the window with
  ChangeWindowAttributes(CWCursor); shown is CWCursor = None, the parent's. FocusChange added to the event
  mask; FocusIn/FocusOut set the focus, ignoring detail NotifyPointer.
- Nothing taken from opentennis: it has no cursor handling (Bevy's window).

## What the original does

Nothing to port: the PS2 has no mouse cursor. This is shell behaviour the user asked for.

## How it was proved

- `crates/openmx/tests/cursor.rs`: a real `Menu` walked by pad reports — front end shown at either focus;
  main menu and track list shown; ride hidden in focus, shown out of it; bare ride (`None`) likewise; 1P's
  pause shown; Settings over the pause shown; resumed hidden; 2P's own pause with 1P riding shown.
- `window.rs` unit tests (compiled into tests/cursor.rs and tests/menu.rs): the bytes of the hide request
  sequence (opcodes, depth 1, foreground 0, cursor its own mask, ids distinct from window/gc in the client's
  range), show = CWCursor 0, the blank cursor made once; focus in/out/pointer-detail events.
- On the real display (:0 via Xwayland): `openmx --ride --renderer software --seconds 3` with a temporary
  trace (removed) logged the hide on the first ride frame and the show on quit, and the server returned no
  error to the hide sequence over the ride. `openmx --ride` (GPU, winit) ran and quit cleanly with the calls in.

## Not verified / not 1:1

- **Not seen by eye on a real window, either backend.** The session's desktop is locked (grim grabs the lock
  screen, and the app's own `--shot` never contains the cursor), so whether the pointer actually disappears
  over the picture was not looked at. The X11 path is proved only as far as "the server accepted the
  requests"; the winit path only as far as "the call is made" — winit's own `set_cursor_visible` is trusted.
- **Wayland native not exercised**: the GPU run here chose whatever winit picked with WAYLAND_DISPLAY set; it
  logged no backend, so whether the run was Wayland or X11 is not known.
- **Focus changes were not seen live**: the locked session sent the X window no FocusIn/FocusOut, so the
  lost-focus show is proved by the unit test on hand-built events and the decision test only.
- Online rides: the decision reads this machine's menu, so another machine's player pausing does not show
  the cursor here (their pause is not in our menu). That matches "no menu up" on this screen; noted in case
  it is wanted otherwise.

## Next

Done unless the human's look on a real window disagrees.
