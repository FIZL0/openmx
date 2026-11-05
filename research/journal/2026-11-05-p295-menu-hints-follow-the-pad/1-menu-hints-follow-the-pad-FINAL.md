# 1 — menu hints follow the pad (FINAL)

Task P295. 2026-11-05.

## What I did

- Checked P292 (50002c02) first: it did not pin anything. It only taught `pads::style_of` the
  iPhone's touch pad name; `Menu::note_dev` already tracked the last device.
- The cause was one level down, in `menu.rs`'s `Readers::read_with`. Past the splash, 1P's pad and
  the keyboard are read as one joint press (P109, so a push a desktop reports on both is one press),
  and that joint press was always put on the **first hosted device in the report list**. main.rs
  (which the browser and iOS hosts run too) lists the keyboard first, so every push of 1P's pad, button or
  stick, reached `step_devices` as `Dev::Keys`. `note_dev` then set `last_dev = Keys` and the hints
  drew key caps. That covers all three reports: desktop, a browser with a controller (the browser
  has a keyboard device too), and the iPhone with a controller. P292's test missed it because it
  called `step_devices` directly, without the readers.
- Fix: `Readers` keeps the device whose push the joint press is (a new fourth field), and that
  device carries the joint press. Only a real joint press can change it, so the keyboard reporting
  the pad's push a frame later (P109's echo) gives no press and leaves the pad. On a same-frame push
  from both, the pad wins, because it is the source when a desktop maps a pad to keys. `Readers::more`
  hands the frame's further presses to that same device. web_host.rs, ios.rs and pads.rs needed no
  change: they have no device path of their own.
- Outside the task, to get the gate green: `disc_worlds`'s `world_mxu_free` test was red on this base
  (and on main). MD95 (be8e5269) changed world_mxu_free's spec, and MD61 (8794b3b1), made beside it,
  refreshed `crates/stitch/worlds/world_mxu_free.start.json` for the old spec. After the merge, the kept file's
  key (35d5…) was no longer the build's (9750…), so the first ride built from cold. I copied the
  start the cold build kept over the file, as the test's message says. Only the key changed, the
  start itself is the same, and the test passes.
- From opentennis: the idea that the legend follows the device that last gave input
  (`hst-play/src/play/legend.rs`, `dev_style`/`keys_style`, with the touch pad drawing PlayStation's
  buttons). openmx already has that structure (`note_dev` → `hint_dev` → `style_of`), so I copied no code.
  The change only makes sure the press reaching the menu names the device that made it.

## What the original does

Not applicable: the menus are ours (AGENT.md, "Menus and front-end: yours to design").

## How it was proved

- New `crates/openmx/tests/menu.rs` test
  `a_pad_press_after_a_key_press_switches_the_hints_to_the_pads_style_and_back`. It drives the
  keyboard and an Xbox pad through `Readers::read_hosted` as main.rs does: 1P claimed by the pad, then
  key → Keys, pad d-pad → Xbox, key → Keys, pad stick → Xbox, both on one frame → Xbox, and the
  keyboard echoing the pad's push a frame behind stays Xbox. Each push also has to move the cursor.
  With the `menu.rs` fix reverse-applied it fails at "the pad's d-pad after a key" (Keys, not Xbox),
  the user's symptom. With the fix it passes.
- `a_push_both_the_pad_and_the_keyboard_report_is_one_press` (P109) and
  `a_touch_driven_seats_hints_are_playstation` (P292) still pass.

## Not verified / not 1:1

- Not run on a phone (browser or the iOS build) or with a real pad on the desktop. There is no pad or
  phone on this unattended machine. The fix is in the shared shell path (`main.rs` → `Readers`),
  which the test runs, but the hosts' own device lists were only checked by reading
  (web_host.rs and ios.rs build no `Dev`s themselves).
- When the touch pad and the keyboard both count as hosts, the touch pad now carries the joint press
  when it pressed, so the hints show PlayStation glyphs after a touch press, as the brief asks. This
  is covered by P292's direct test only, not through `Readers`.
- On a frame where the pad and a key both start a push, the hints follow the pad. That is a choice,
  made for the desktop that maps a pad to keys, not anything measured.
- The first `./kit done` run also went red in `online_car`'s
  `two_online_shells_see_the_guest_switch_into_the_buggy_and_back`: the two machines' buggies drifted
  about 1 ft apart. It passes when run alone, and nothing in it goes through menu input readers, so I
  did not fix it here. Filed as N25 (critical).

## Next

Nothing.
