# 1 — peer udp presets (FINAL)

Task N15. 2026-10-21.

## What I did

- `--online`/`--peer` (N2's plain-UDP path) has no room or handshake to say each seat's controller
  preset (P79), so until now every seat there rode preset 0 and a machine on another preset printed
  that it was not sent. Now every datagram's head carries the sender's **card** after the build id
  (N7): `MAGIC`, id length, id, card length, card. The card has one byte per seat the sender
  holds, in seat order: its first seat's preset (`--preset` under `--controls original`, the same
  expression the lobby path uses), and 0 for its other seats, which have no pad.
- `net::udp::Udp` keeps each peer's last card. `Udp::presets(lineup, me)` gives every seat's preset
  once every peer in the line-up has been heard (an empty `-` seat gets 0). It refuses a card whose
  length does not match the seats `--seats` gives that peer (the machines were started with
  different line-ups), and a preset the original does not have.
- `Online::frame` (`take_presets`) builds each seat's table from those presets right after it
  receives and before the session runs. ggrs ticks only after its sync round trips with every peer,
  so every card has arrived by the first tick. If a tick ever ran first, the ride ends and says
  why, rather than desyncing silently. After that, a resync, restart or desync dump carries the
  presets in the ride's own bytes (N13's `Rides::encode`).
- `MAGIC` is now `omp`. A datagram that starts with the old `omx` comes from an N7–N14 build that
  sends no card. It has the same build id, so its session bytes would otherwise be read as a card.
  It is refused by name: "sends no controller presets (a build from before N15)".
- `Udp::meet` lets the tests name peers after binding every socket to port 0, which avoids the
  port race in the existing two-builds test (see the memory notes on "Address already in use").
- opentennis has no plain-UDP path (no `UdpSocket` in its crates), so nothing was taken from it.
  The design follows N13: each seat rides its own player's preset, and a mismatch is refused.

## What the original does

Nothing to port: the original has no online play. The preset tables are P79's.

## How it was proved

- `tests/netplay.rs` `every_seat_rides_its_own_preset_over_peer_udp`: three machines (host, g, k
  with two seats) run real `Netplay` sessions over localhost UDP with the `Udp` transport. Every
  seat starts on preset 0 and the cards are 3 / 2 / 1,0. Every machine ends with the seats on
  [3, 2, 1, 0]. No machine ticks before it has every preset. All three agree on the state hash
  and the pads before every settled tick (>150 of 240), with no desyncs. N13's
  `every_seat_rides_its_own_preset_and_says_it_in_bytes` already shows the same pads on preset 0
  give a different ride, so the hash agreement means the presets matched.
- `net::udp` unit tests: the card round-trips and is not passed on as session bytes. Presets wait
  for every peer. A wrong seat count and an out-of-range preset are refused. An N7-format datagram
  is refused by name. The old build-id tests were updated to the new head.
- End to end: two `target/release/openmx --headless --online` shells on 127.0.0.1, `--controls
  original --preset 3` and `--preset 1`. Both print `online: controller presets 3,1`, run 392 ticks
  with 0 desyncs and report the same state hash before tick 300.

## Not verified / not 1:1

- The headless shell run proves only that the presets arrive and the machines agree. `--online`
  pads are idle, so no button went through a preset table there (the hash equals the preset-0
  run). That the tables matter online is proved by the harness test with `mash` pads, not by the
  shell.
- An N7–N14 build talking to this one was not run (there is no old binary here). The refusal is
  proved with a hand-built N7-format datagram. An old build that receives this build's `omp`
  datagrams will say "sends no build id (a build from before N7)". The ride is refused, but the
  reason is inaccurate, and that build's code cannot be changed after the fact.
- A machine's second and later seats on `--online` have no pad (`--players` is refused there), so
  they ride preset 0 rather than a per-seat `--preset` list entry.

## Next

Nothing.
