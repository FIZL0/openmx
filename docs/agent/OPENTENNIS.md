# Taking it from opentennis

`../opentennis` (`~/repos/opentennis`) is the same owner's port of another PS2 game (Hot Shots Tennis, Rust and
Bevy) and is further along. Its code is ours, so copy from it freely rather than reinventing anything it already
has:

- the oracle and float layers it shares with this kit;
- the PCSX2 tools (`tools/record_*.py`, `play_p2m2.py`, `vpad.py`);
- local seats and pad hot-plug (`crates/hst-base/src/seats.rs`, `crates/hst-play/src/play/controls.rs`);
- the menus, settings files and on-screen keyboard (`crates/hst/src/play/main_menu/`,
  `crates/hst-base/src/lib.rs`'s `store`);
- the whole of online play: ggrs rollback (`crates/hst-play/src/play/rollback.rs`, `snapshot.rs`, `netplay.rs`,
  `resync.rs`, `recover.rs`) over iroh QUIC with room codes and LAN (`crates/hst-base/src/net.rs`,
  `handshake.rs`), and its design in `plan/M2.md`.

Its code map is `plan/CODEMAP.md` there. The menus here are in MX Unleashed's style, not tennis's: opentennis
gives the structure (screens, seats, settings, lobby), and the original disc gives the look.
