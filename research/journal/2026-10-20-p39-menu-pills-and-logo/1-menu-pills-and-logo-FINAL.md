# 1 — menu pills and logo (FINAL)

Task P39. 2026-10-20.

## What I did

Found the cause before changing anything, by rendering `openmx --screen NAME --shot` at 640x448,
1024x448 and 448x640. The painter scaled the 640x448 layout by width and height separately, so any
window that was not 640:448 stretched everything. At 1024x448 the 256-wide highlight became ~410 px
long and the logo came out flattened. On Settings/Controls the column was 1.75× wide and drawn
*after* the logo, so it darkened the logo ("behind the menu"). At 640x448 the main menu looked like
the original's: the original's own highlight is a fixed 256 wide (`context/shots/c1-title.png` in the
main checkout: "Career" on a 256 bar).

All of it is in `crates/openmx/src/menu.rs`, in the drawing half only (the menu state machine is
untouched):

- **One scale, anchors.** `Painter::new` takes `s = min(W/640, H/448)`, so the frame is the layout
  plus extra room on one side. Each part counts its layout numbers from an `Anchor`:
  - top left: logo, column, list, settings arrows, track band;
  - bottom left: title, ticker, the black ticker bar;
  - bottom right: the controller legend;
  - centre: the keyboard.

  Full-frame parts use `whole()`: background, pause dimming, column height, track masks. The radial
  light stays round and centred across the frame. The track still is cropped to the band's shape
  instead of stretched.
- **Pills sized to their label.** `Painter::pill` measures the label in the face it is drawn in
  (oblique 22) and draws `mainmenu_highlight` (and the shadow, on the lists) `PILL_PAD` = 12 layout
  px past each end. It is drawn as three slices: each end is half the texture's height wide, kept
  unstretched, and only the middle stretches, so a short pill like the keyboard's "a" keeps round
  ends. The same pill is used on the main, pause, settings and controls lists and on the keyboard's
  keys and commands (P36 used fixed 36/92/256/400-wide bars there). The settings arrows now sit 4 px
  either side of the pill instead of at fixed x.
- **The column** is `mainmenu.lyt`'s 256 on every list screen; the 1.75× column is gone. Settings and
  Controls rows are centred on the main menu's line (x 185), as the original's items are.
- **The logo** is drawn after the column and the list, and nothing is drawn over it. Its
  `mainmenu.lyt` box (512x64 at 20,25) ends at y 89 and the first pill starts at y 95, so with one
  scale nothing overlaps it at any shape.
- `Art::parts` draws like `Art::draw` and also returns where the logo, each pill with its label, and
  each plain label went, in frame pixels, so the test checks the layout without reading pixels back.

From opentennis: nothing taken. Its menus are Bevy UI, where flex layout sizes a pill to its text; this
shell paints a software frame, so the measuring is done here with the `.cel` font's advances.

## What the original does

Front end, not ported (AGENT.md: the menus are ours to design). The original's main menu runs at a
fixed 640x448: a 256-wide highlight bar that does not follow the label, and the logo drawn over
the column. This port keeps the logo's position and its draw order (over the column). The pill width
is the port's choice, which the task asked for.

## How it was proved

- `crates/openmx/tests/menu.rs` `every_pill_fits_its_label_and_nothing_covers_the_logo`:
  - windows: 640x448, 960x540 (16:9), 1260x540 (21:9), 360x640 (portrait);
  - every screen, twelve cursor positions each (Down through the lists; Right/Down across the
    keyboard's keys to its commands).
  - Asserted for every pill: its width is the label's measured width + 2·`PILL_PAD`·scale, ±1 px,
    and it is centred on the label, ±1 px. The logo's box overlaps no pill, no pill's label and no
    plain label. The logo is 8:1 (unstretched) at every shape. Each list screen has a logo, and
    every list screen and the keyboard draw exactly one pill.
  - Then `pills == 240` (5 screens × 12 × 4 shapes), which also proves it did not skip for lack of
    a disc. `./kit check -- -p openmx --test menu`: 7 tests passed. The full `./kit check` at done.
- By eye: `--screen {main,settings,controls,keyboard,tracks,pause} --shot` at 640x448, 1024x448
  and 448x640 (18 shots, in the session's scratchpad, not committed). Pills hug their labels with
  round ends, the logo is clear and unstretched, the legend stays in the bottom-right corner, and
  the keyboard is centred.

## Not verified / not 1:1

- **The pill padding (12 layout px a side) is chosen, not read.** `mainmenu.lyt` gives the
  highlight only as a fixed 256x32 box; it has no padding rule, because the original's bar does not
  follow its label. The 3-slice end width (half the texture's height) is also my choice. It looks
  right by eye; it was not measured against the texture's alpha.
- **The label box's height is the 'M' glyph's height.** It is used only for the overlap test, not
  for drawing. Descenders and the oblique face's lean past its advance are not in it. So "overlaps
  no item" is checked against advance-width × capital-height boxes, not against inked pixels.
- **The logo's box is the whole 512x64 texture**, transparent margins included. That is stricter
  than the visible logo, so the no-overlap check is conservative. It is not checked against the
  logo's alpha.
- **Settings/Controls are now on the 256 column, centred at x 185.** A long rider name or Controls
  line can run past the column's right edge and onto the background. Controls' small lines ("Pad
  /dev/input/…") are long; at x 185 a long device path is clipped at the frame's left edge, where at
  P36's x 242 it was clipped a little later. Not fixed, since the pad line is outside this task.
- **The keyboard's typed-name box stays a fixed 400x32.** It is an input field, not a pill, so it
  does not follow its text.
- **Portrait windows get the layout at the width's scale** with extra room at the bottom (the column
  runs to the bottom, and the title, ticker and legend stay at the bottom). This is my layout choice;
  the original has no portrait mode to compare against.
- **The shell's window loop was not driven live.** Resizing a window mid-menu was not tried. The
  shots are fixed `--size` runs, and the painter reads the frame size every frame, so a resize
  should follow.

## Next

none — the task is done.
