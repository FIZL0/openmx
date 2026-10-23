# P118 — menu redo

2026-10-22. Task: `./kit plan P118`.

The front end is redrawn from `docs/menu-concept.html`: the orange backdrop, the dark column, the
logo, the maroon pill with its white rim and hard shadow, the slanted title over the black band, and
the angled hint tab, all in vector ink (`menu_art.rs`: distance-field type slanted 11°, stadiums and
boxes measured in frame pixels) that the CPU rasteriser and the GPU sprite pass draw alike. Main is
Free Ride, Local Game, Online, Garage, Mods, Settings, Quit with a side panel. Settings has five tabs
on L1/R1 (Ride, Display, Audio, Controls, Camera), the Garage three (Rider, Bike, Gear, each with a
Player row first), and every option row changes in place, dims with a reason when it does not apply,
and has its side panel. The state machine stays pure and pad-driven (`menu.rs` + `menu_pages.rs`; the
painter is `menu_screens.rs`). `tests/menu.rs` walks every screen and tab by presses alone, presses
every row of every page, wraps L1/R1, reads a pre-redo settings.txt/controls.txt as the same settings
(and writes them back word for word), and measures label edges at 1080p and 4K on both paths. It
came out done; details and what is not 1:1 in `1-menu-redo-FINAL.md`.
