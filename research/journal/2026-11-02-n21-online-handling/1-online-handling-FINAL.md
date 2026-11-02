# Online handling — FINAL

## What changed

- `openmx-net` `Room::handling` (serde default, skipped when empty, so an older build's room is the
  original's) and `Handshake::{handling, handling_hash}`. The hash is `Disc::tricks` of that id: a
  trick pack's hash already covers its `mx_*.phy` files, which is all the handling reads from it.
  `check` compares it after the tricks: "the handling differs (host: …, you: …)", or the pack missing /
  not the same files.
- The shell: `lobby::room_handling` / `name_handling` put the Handling row's pack in a hosted room
  (as `name_tricks` does), `Network::pump` calls it; the lobby start sets `Args::handling` and
  `Args::room_handling` from the room; `ride_handling_pack` takes the room's pack first; both lobby seat
  loops (start and drop-in) read each seat's bike through `room_handling_phy` (the `Set::file()` member,
  as `lay_room_tricks` uses), then lay the tricks — the same order as an offline ride. Open to Online
  (N10) names the handling the ride was built on.
- The Handling row's help no longer says online is always Original.

## How it was checked

- `cargo test -p openmx-net`: `handshake::tests::every_seat_rides_the_hosts_handling` (agreement, the
  three refusals, older room/handshake JSON reads as the original's) and the existing tricks tests.
- `cargo test -p openmx --test lobby` (64 pass): `every_seat_of_an_online_ride_has_the_hosts_handling`
  runs `name_handling` (Original, mods off, a riding room untouched, MX vs. ATV) and the handshake on the
  shell's own `DiscFiles` over real pack folders: the same files ride, a changed `mx_race125p.phy` or no
  pack is refused.
- `cargo test -p openmx --test menu` (230 pass), `cargo build -p openmx --bin openmx`.

## Not verified / not 1:1

- No two-machine ride was run on MX vs. ATV's handling; the lobby test stops at the handshake. That a
  seat's bike is read from the pack on a lobby ride is by reading the code path, not a run.
- `--online` (no lobby, no handshake) stays the original's handling, as it does the original's tricks.
- Not an original-game behaviour: MX Unleashed has no handling setting, so there is nothing to match.
