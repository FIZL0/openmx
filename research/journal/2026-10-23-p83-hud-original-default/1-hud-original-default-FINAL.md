# 1 — hud original default (FINAL)

Task P83. 2026-10-23.

## What I did

- **Decision: "Original" in the brief now means the default HUD, which is P121's ride HUD.** P83 was
  filed 2026-10-21, when Original was the default overlay. P121 (user, 2026-10-22, the later word)
  made `Overlay::Hud` the default and first choice. So every launch starts on the default (Hud), and a
  settings file saying `debug` falls back to that default rather than to Original. The point of P83,
  never landing on the debug HUD, holds either way.
- Why a launch could land on debug: settings.txt's `overlay = debug` (the Settings row offered it) went
  through `main.rs`'s front-end ride path (`a.hud = overlay == Debug`), and since P121 `overlay = on`
  was already read as a non-debug HUD. A direct launch with no `--overlay` was already on the default.
- `menu.rs`: `Overlay::CHOICES = [Hud, Original, None]`. `Settings::parse` takes only those, so
  `overlay = debug` keeps the default. `Overlay::ALL`/`from_word` still know `debug` for `--overlay`.
- `menu_pages.rs`: the Display tab's HUD row (the pause menu reaches the same Settings page) offers and
  cycles `CHOICES`, and its help and tips drop Debug.
- `main.rs`: a new `Args::debug_hud`, set only by `--hud` / `--overlay debug`. F3/H toggles the debug
  HUD only when it is set. A settings change no longer sets `rig.hud.on` from the overlay (it was always
  false now). It only clears it when the launch was not `--hud`. Docs and `--help` updated. I added no
  separate `--debug` flag, because the brief gave it as an option and `--hud` already covers this.

Nothing taken from opentennis; it has no equivalent debug-HUD switch.

## What the original does

Nothing to port: the debug HUD is the port's own furniture, and the menus are ours to design.

## How it was proved

- `tests/menu.rs` `the_settings_never_start_or_offer_the_debug_hud`: a fresh Settings, an empty file,
  `on`/`off`/`debug` from before P121, and `debug` in a file marked as written since P121 all read as Hud.
  It also saves and loads through `menu::save`/`load`, checks that the row's choices are exactly HUD,
  Original, None, and pads it round both ways and on Cross without ever reaching Debug.
- `tests/hud.rs` `only_a_hud_launch_starts_on_the_debug_hud`: the real shell, software renderer, from the
  disc (it ran: its three PNGs were written). A plain launch matches `--overlay hud`'s frame, and `--hud`'s
  frame differs by more than 1000 pixels. I looked at that frame and it shows the debug panel.
- I updated the tests that used `Overlay::Debug` as a saved setting (menu.rs round trips, the P118
  old-file test, the P126 Original switch, ride_hud.rs's P121 test) to use None or Hud.

## Not verified / not 1:1

- **F3/H gating not driven by a test.** The shell runs headless in tests and no key reaches it there,
  so "F3 does nothing without `--hud`" is only proved by reading the one-line `&& args.debug_hud` guard.
- **No front-end launch test against settings.txt saying debug.** The shell reads settings.txt from
  beside the user's disc (`menu::store_dir`), and a test must not write there. The parse/load test
  covers the file side. The main.rs side follows because `Settings::overlay` can no longer be Debug.
- **Default is Hud, not Original:** the brief's wording is superseded by P121, as decided above.

## Next

Nothing.
