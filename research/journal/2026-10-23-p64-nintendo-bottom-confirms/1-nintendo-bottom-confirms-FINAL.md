# 1 — nintendo bottom confirms (FINAL)

Task P64. 2026-10-23.

## What I did

- **By position is the default.** `Wiring::default()` (pad.rs) and `Controls::default()` (menu.rs, now a
  hand-written `Default`) both have `nintendo_positions: true`. A Switch pad's B (js 0, `BTN_SOUTH`, the
  bottom one) reads as Cross and A (js 1, the right one) as Circle, exactly as on an Xbox pad or a
  DualSense. Everything downstream reads Cross: the splash's press start (`ok || start`, so + works too),
  confirm on every screen and the join screen, and in a ride whatever Cross is — under the original layout,
  the gas. Nothing in the menus or the ride needed changing, only the default.
- **Migration.** P53 wrote `nintendo = labels` into every `controls.txt` whether or not anyone picked it.
  That word now reads as positions. The menu's label choice is written as a new word, `by-label`, and only
  that word puts a pad on labels. So an old file migrates the next time it is loaded, and it is rewritten
  the next time anything is saved. `positions` keeps its spelling, so P53-era files that already said
  `positions` round-trip as they were. The file's comment block says so.
- **The row.** P53 had already added the Controls tab's row ("Nintendo Pads", By Label first). It is now
  **Nintendo A/B**, with **By Position** first (the default) and **By Label** second.
  `docs/menu-concept.html`'s row matches. Applying it at once was already in place: main.rs calls
  `seats.set_wiring(front.wiring())` on any changed menu frame, and `Front::wiring` copies
  `nintendo_positions` across. It is saved through `Controls::text` as before.
- **Tests.**
  - `a_nintendo_pad_s_bottom_button_confirms_until_its_row_says_labels` (tests/menu.rs) uses pad.rs's
    sysfs+fifo Pro Controller fixture and starts from a `controls.txt` that says `nintendo = labels`:
    - the file loads as positions;
    - on the splash, A asks to quit (back) and B takes 1P;
    - B confirms Settings and A backs out;
    - R1 and the hat reach the Controls tab's row, which reads By Position, and the hat's right flips it;
    - on the next frame (the test copies main.rs's set_wiring-on-change step) B backs out and A confirms;
    - the file says `by-label`;
    - a "restart" (new Seats, `menu::load`) keeps labels: B on the splash asks to quit, A takes it;
    - + takes the splash either way.
  - The existing P53 pad test now asserts B → Cross and A → Circle by default, with labels as the
    flipped case.
  - The ride-pause and splash-at-any-frame-rate tests press B, not A.
  - The file-format test also checks the migration and that `by-label` round-trips.

Nothing came from opentennis: it has no Nintendo label/position switch.

## What the original does

Nothing to port. The PS2 pad has a fixed Cross. This is the port's own front end and host-pad mapping.

## How it was proved

`./kit check` (the menu tests above, using fifo pad fixtures). No emulator was needed: the PS2 game never
sees a Nintendo pad.

## Not verified / not 1:1

- I never tested on a real Switch Pro Controller (none here, and the shell skips uinput pads). The js
  numbering is pad.rs's PRO_KEYS, read from hid-nintendo's own mapping table by P53. If a kernel or an
  8BitDo firmware numbers B differently, the default would be wrong on that device.
- The test's "applied right away" step copies main.rs's `if menu.changed() { seats.set_wiring(front.wiring()) }`.
  It does not run main.rs's loop, because the binary's `Front` cannot be built in tests. If that call
  were removed from main.rs, this test would not notice.
- Migration can override a real pick: a user who picked By Label in P53's row before today also had
  `labels` written, and they are moved to positions once. The task asked for this; the user has said three
  times that they want positions.

## Next

Nothing.
