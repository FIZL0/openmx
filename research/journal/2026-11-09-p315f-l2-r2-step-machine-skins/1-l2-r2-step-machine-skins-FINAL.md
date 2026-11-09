# 1 — l2 r2 step machine skins (FINAL)

Task P315f. 2026-11-09.

## The decision (unattended run, nobody to ask)

The task asked me to settle with the user whether the Fliers rows take L1/R1 (and the Garage's tabs move
to another pair) or left/right stays. Nobody was there to answer, so I decided: **L1/R1 stay the tabs, and
L2/R2 step the shown machine's skin on the Machines tab, beside left/right on the row.**

Why:
- L1/R1 are the tab switch on every tabbed page: Settings, the Garage, the mod store, the Free Ride strip's
  player. Each page draws the same L1/R1 tab strip (`Pen::tabs`), and Settings' main-menu card says "L1 / R1
  between tabs". If only the Garage moved its tabs to another pair, it would be the one page that works
  differently.
- The user asked for bumpers ("click bumpers to change skin") so the skin can be stepped without walking to
  the row first. L2/R2 give exactly that: from any row of the Machines tab they step whatever the stage is
  showing (the flier on a flier row, the bike's paint off them). That is what the stage's arrows already
  step (`Menu::step_garage_machine`), so the pad, the touch arrows and the mouse all do the same thing. L2/R2
  had no use in the menus before.
- Left/right on the row still step it as they did, so nothing that already worked changes.

If the user wants the true bumpers (L1/R1) for skins instead, the change is small: swap the two checks at
the top of `walk_page`'s tab handling, and change the Garage's tab strip and hint to L2/R2. Noted under Next.

## What I did

- `Press` gets `l2`/`r2`, read from the pad's L2/R2 bits (`press_of`) and merged in `Press::or`. The
  keyboard should reach them through its default L2/R2 keys (S/W), like any other control.
- `walk_page`: on the Garage's Machines tab, L2/R2 call `step_garage_machine(∓1)` before the tab check. On
  the other tabs and pages they do nothing.
- The Machines tab's hint row gets an "L2 R2 Skin" chip (`Btn::L2R2`, drawn like the L1R1 chip and as
  glyphs in the other button styles). The flier rows' about text mentions L2 and R2.
- The sound cue for a side press counts L2/R2.
- Test: `tests/menu.rs` `l2_and_r2_step_the_machines_tabs_skin_and_l1_r1_stay_the_tabs`. R2/L2 step the
  plane's skin on its row and the bike's paint from the Bike Class row (the cursor's row stays as it was),
  L1/R1 still switch tabs, and L2/R2 do nothing on the Gear tab.

Nothing taken from opentennis: its menus have no skin picker with a stage.

## What the original does

Not relevant: the menus are ours to design (AGENT.md). MX Unleashed's garage has no flier picker.

## How it was proved

The menu test above, run against the disc's catalogue (it ran, no skip line). `./kit check --all` at done.

## Not verified / not 1:1

- No menu shot of the Machines tab with the new "L2 R2 Skin" chip: only the menu test covers it. Nobody
  looked at the chip's layout in the hint row on a narrow (split-screen) frame. The hint row shrinks its
  hints to fit, as it does for the other chips.
- The user never confirmed the decision (L2/R2 for skins, L1/R1 stay the tabs). See "The decision".
- Switch To Machine in the pause menu has no picker yet (the pause row still just swaps), so L2/R2 there
  were not wired. They will need wiring when that picker lands.
- The keyboard's L2/R2 keys (S/W by default) were not pressed in a running shell: the test drives
  the pad report only. The keyboard's keys become the same report bits as everything else.
- No pad in use here (uinput is root-only), so a real pad's analogue trigger was not checked to give one
  step per pull. The menu reads L2/R2 from the report's digital bits, the same as every other button, and
  the reader only acts on the moment a button goes down, so a held trigger steps once.

## Next

If the user would rather have L1/R1 step skins: on the Garage only, check L1/R1 for `step_garage_machine`
and L2/R2 for the tabs, and give the Garage's tab strip and hint the L2/R2 words.
