# 1 — button icons (FINAL)

Task P281. 2026-11-03.

## What I did

A **Button Icons** row on Settings → Display, after Aspect Ratio. Its choices are Auto, PlayStation,
Xbox, Switch and Generic, and Auto is the default. The help text is opentennis's. Settings.txt saves it
as `button_icons = ps|xbox|switch|generic`, and writes no line on Auto.

Taken from opentennis (`crates/hst-play/src/play/legend.rs`, `main_menu/options.rs`):
- the style set (Keys, PlayStation, Xbox, Switch, Deck, Generic), the setting's words and its help;
- Auto's pick by USB vendor: Sony is PlayStation, Nintendo is Switch, Valve is the Deck (Steam's virtual
  pad only with `SteamDeck=1`), and anything else is Xbox;
- the keyboard always drawing its own keys;
- the shoulder words: L1/R1, LB/RB, L/R; ZL/ZR, LT/RT;
- a forcing env var for shots, named `OPENMX_BUTTONS` here (theirs is `HST_LEGEND`).

Changed from opentennis:
- A pad with no ids (a browser's, an iPhone's) is also read by its name. "DualSense", "Pro Controller",
  "Steam Deck" and the like pick its family, and a browser's "Vendor: 054c" / "054c-0ce6-" in the name
  is parsed.
- The touch pad (P197) draws ✕ ○ □ △ on Auto.
- The PlayStation style is openmx's DualShock 2 look as it was, with SELECT/START. It is not the
  DualSense's CREATE/OPTIONS, because the original was played on the DualShock 2.

The code is in `crates/openmx/src/pads.rs`, mounted as `menu::pads` so no test crate needs a new mount.
- `style_of(setting, device)` gives a device's style: Keys for the keyboard, otherwise the setting's, or
  on Auto `identity_family` of the pad's `vvvv:pppp uniq phys name` identity.
- `Buttons` gives each control's glyph. A face button is addressed by where it sits: a Switch's bottom is
  B. By Label (`nintendo = by-label`) Cross is the right-hand A.

Whose buttons each place draws:
- **Menus, splash, join screen, settings.** The device that last pressed something, else the first pad
  plugged in, else the keyboard (`Menu::hint_dev`).
- **Pause.** In a split, each player's pause uses their own device (`Menu::seat_dev`). The shell writes
  `Menu::ride_devs` every ride frame from the pad seats (`args.pad_seat`).
- **Ride HUD.** The Accept / Hold to start pill uses each view's rider's device (`Shown::buttons`). The
  join cue uses the first plugged-in pad that nobody rides (`Menu::free_dev`, `Shown::join_buttons`).
- **Keyboard keycaps.** The bound keys, from `Seats::key_caps()` (`Bindings::key_caps`). A modifier is
  skipped when the line has another key, so L1 is Q beside E, not Shift. The menus' Circle is Esc.
- **Trick list.** Gestures are drawn as the ride's keys (Circle is F, not Esc).
- **Pause chip.** "*Cross* on a free controller joins" says the free pad's word (A, B, …).
- **Pad diagram** on Controls. It draws its face buttons in the pad's style, and in the PlayStation's
  for the keyboard.

How each style draws:
- Xbox: green, red, blue and yellow lettered discs.
- Switch and Deck: dark lettered discs.
- Generic: a disc with four dots, the meant one filled.
- Keyboard: a light keycap as wide as the key's name.
- Shoulders, Select and Start: the existing dark pill with the style's word.

## What the original does

Nothing to port. The original only knows the DualShock 2, and menus are ours to design (AGENT.md).

## How it was proved

- `tests/menu.rs`:
  - `button_icons_is_auto_by_default_and_saved_in_settings_txt` covers the default, the row's label,
    choices and help, stepping it by the pad, and saving and reloading each value.
  - `auto_button_icons_pick_each_pads_family` covers eight pad identities picking PlayStation, Switch,
    Deck and Xbox; the keyboard's caps; each split seat's own style; a fixed setting; and By Label.
  - `the_settings_file_reads_back_as_it_was_written` now also carries `button_icons`.
- `tests/ride_hud.rs`:
  - `the_hud_prompts_draw_each_styles_buttons` checks the join cue's words per style, and the Xbox A's
    green pixels drawn on the software path.
- Unit tests in `pads.rs` (3) and `pad.rs` (`the_hints_keycaps_are_the_bound_keys`).
- Shots: `openmx --menu --screen {splash,settings,pause} --shot`, with `OPENMX_BUTTONS` set to each of
  keys, ps, xbox, switch, deck and generic, on a private headless weston. They are kept in
  `context/shots/p281/` (git-ignored). I looked at them: the hint tab, the tab bar's shoulders and the
  splash's Start/Cross change per style, and the PlayStation shots match the look before.

## Not verified / not 1:1

- Not checked on real hardware. A real DualSense, Switch Pro, Xbox pad or Deck has not been plugged
  into this machine for this task; the vendor ids come from opentennis's table and the unit tests.
- The HUD prompts were only checked by the software-path test and the code. No ride shot with an offer
  pill or the join cue up was taken.
- In the menus (not in a ride), a second player's own device shows only on their pause. Every other
  menu screen shows the device that last pressed.
- The join screen's open seats draw the last-pressed device's Cross, not the free pad's.
- The pause chip's free-pad word came after the shots were taken, so no shot shows it. The
  pause shots still say "Cross".
- The Switch's "-" and "+" are ASCII. The font may lack U+2212 and was not checked for it.
- The web build's pads are only parsed by name/ids in the name. No browser was driven for this task.

## Next

Nothing. The task is done.
