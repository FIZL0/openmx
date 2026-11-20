# F11 — the iPhone's HUD and hints clear of its own touch pad

2026-11-19. Task: `./kit plan F11`.

Already done by P304 (merged 2026-11-05), filed before it landed. No code change.

- `ios_gpu.rs` poll sets `menu::touch_room` while `ios::TOUCH.shown`, to the room inside the kept
  insets: the same edges `touch_pad.rs` lays its pad out from. So the ride HUD
  (`ride_hud::sprites_clear_of`) and the front end's hints and band line (`menu_screens.rs`, `Room::of`)
  stand clear of the iPhone's pad as they do of the page's.
- The one gap in that mapping: `touch_pad.rs` sizes its `u` from the whole window, `touch_room` from the
  room inside the insets. On a landscape iPhone both are height-bound (`0.125 × height`; the bottom
  inset is not kept), so they match. `tests/ride_hud.rs`'s `every_touch_control` asserts this for the
  iPhone 15 Pro case (2556x1179, 177 each side). They would only differ if the side insets pushed the
  room narrower than 1.79:1, which no iPhone does.
- P323's mph under the fps line, top right, is laid out round the same `Room`.

Checked: `cargo test --release -p openmx --test ride_hud touch_pad` (3 pass, including P304's iPhone case
and P323's) and `--test menu touch_pad` (hints and band, title).
