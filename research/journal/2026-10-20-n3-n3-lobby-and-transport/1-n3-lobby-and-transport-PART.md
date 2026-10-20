# 1 — n3 lobby and transport (PART)

Task N3. 2026-10-20.

## What I did

- **Where it went, and why not where the brief said.** The brief names `crates/openmx/src/net` and
  `crates/openmx/tests/netplay.rs`. When I started, N2 (s3, `task/N2`) was in flight writing exactly those paths
  (ggrs in `crates/openmx`), and P36 (s1, `task/P36`) was writing `crates/openmx/src/menu.rs`; both are `[ ]` in
  plan/TODO.md. To keep the merges clean the transport is a new crate, **`crates/openmx-net`** (library + a terminal
  lobby binary), and its test is **`crates/openmx-net/tests/netplay.rs`**. Folding it into `crates/openmx/src/net`
  once N2 lands is a move, not a rewrite: nothing in it depends on the shell.
- **Taken from opentennis** (`/home/rycs/repos/opentennis/crates/hst-base/src/{net.rs,handshake.rs,chat.rs}`,
  `tools/pkarr-relay`), and what changed:
  - Transport (`src/net.rs`): iroh 1.3 endpoints (QUIC, hole punching, relay fallback: n0's, your own, or none),
    one bi stream per guest with length-prefixed JSON messages, datagrams framed with the peer they are for/from,
    the host relaying both between guests (a star). Same `Online`/`online.txt` keys (`name`, `relay`,
    `relay_url`, `private`, `code_server`), same Cmd/Event shape on its own tokio thread.
    *Changed:* no browser/WebRTC path, no mods/packs/emotes/devices/rejoin/countdown/board slots/LAG; ping kept
    (2 s, shown per seat). `relay_only` (clear the IP transports) and `code_servers`/`Board::Local` are test hooks
    opentennis doesn't have.
  - Room codes (`src/codes.rs`): opentennis's four-character codes, ticket in a TXT record signed by a key derived
    from the code, written to the DHT (BEP 44) and to pkarr relays (PUT/GET by z-base-32 key), the freshest wins,
    a taken code is re-rolled. *Changed:* the key's seed and the TXT name are openmx's (`openmx-lobby-code/1/…`,
    `_omx`), so opentennis and openmx codes never collide; there is no openmx code server: online.txt's
    `code_server` first, then https://relay.pkarr.org. **On a LAN the code is found in the host's mDNS listing
    first**, so a LAN game needs no internet at all (opentennis needs the relay for codes).
  - Lobby board (`src/board.rs`): BEP 5 announce on a fixed infohash with the lobby number as the port, the
    listing as a BEP 44 item under a shared key salted by that number and signed again by the endpoint key; LAN
    listings in the mDNS user data (≤ 245 bytes, test checks a 24-emoji name fits). Quickplay = busiest open lobby
    of the same build with room, newest on a tie (`room::quick_pick`), else host.
  - Lobby state (`src/room.rs`): up to 8 seats, a peer may seat several local players (all or none), picks are
    rider + bike set, ready per guest (cleared on any change), names, ping, private, the code, riding.
    `back_to_lobby` when a peer drops mid-ride or the host ends it.
  - Handshake (`src/handshake.rs`): *changed from opentennis* — there it is exchanged at join; here the host's
    Start sends the line-up with its seed, every guest answers a `Handshake` computed from that line-up **and its own
    disc** (`Disc` trait: a hash per track and per bike set, FNV-1a 64 over the file the shell would load), the host
    compares each with its own: build, track (name and hash), every seat's bike set (name and hash, and "isn't on
    your disc"), wrecks, biplane. A mismatch refuses that peer with what differs and leaves the rest in the lobby;
    all equal → `Go` with the seed on every peer. The build mismatch is also refused at Hello.
  - Chat (`src/chat.rs`): opentennis's clean (control and bidi out, 80 chars) and rate limit (4 per 8 s), through
    the host with the sender's lobby name; never touches the sim.
  - `tools/pkarr-relay/`: opentennis's docker-compose + nginx, comments re-pointed (no openmx domain).
- **Terminal lobby** `cargo run -p openmx-net -- host|join CODE|find|quick` (flags in `src/main.rs`'s header). Tried
  by hand on this machine: host + join by code on the LAN with relays, board and code servers off: joined, chat
  line arrived, ready shown, ping 0 ms.
- **Lobby screens: not done** (P36's menu isn't in main yet). Blocked on it; see Next.

## What the original does

Nothing to compare: the original's online play (GameSpy-era) is not what we copy; the brief and AGENT.md say
menus and netcode are ours, from opentennis. No decompilation was read for this task.

## How it was proved

`crates/openmx-net/tests/netplay.rs` (`./kit check -- -p openmx-net --test netplay`, passing 3/3, three runs):
- `a_host_and_a_joiner_find_each_other_by_room_code_on_the_lan`: relays off, board off, no code servers — the code
  can only resolve from the host's mDNS listing. Join, ready, Start: both get `Start` with one seed; a ride message
  goes guest → host; the guest leaves mid-ride → host gets `Dropped` and is back in the lobby (riding false, seed 0).
- `a_host_and_a_joiner_find_each_other_by_room_code_through_a_local_relay`: an in-process `iroh-relay` on plain
  HTTP, both endpoints relay-only (no IP transports: every packet goes through it), no LAN, the code through a local
  pkarr-relay stand-in (signature over `3:seqi…e1:v…:` verified, an older seq refused). Join with another bike,
  chat both ways (the bidi override cleaned out), ready, Start → both ride.
- `a_joiner_whose_bike_set_differs_is_refused`: same relay setup; the guest's disc hashes race125p differently:
  the guest gets `Failed("Refused: Can't ride: rider 1's bike set race125p isn't the same on your disc as on the
  host's.")`, the host "Not started: Guesty: …" and a lobby with only itself, seed 0.
- Unit tests: the handshake refuses each sim-affecting difference and ignores the seed; FNV-1a reference vectors;
  seats/quickplay/listing rules; online.txt round trip; datagram framing; code parts; chat cleaning; listing seal.
`./kit check` (whole workspace): TESTS PASSED.

## Not verified / not 1:1

- **The real internet is untested**: no hole punch between two NATs, no n0 public relay, no mainline DHT board, no
  relay.pkarr.org round trip. The tests use mDNS on one machine and a local relay/pkarr stand-in; nothing reached
  the internet (on purpose: tests must not publish). Quickplay and Find over the DHT have no test at all (a
  `n0_mainline` testnet would do it; not written — the brief's test didn't ask, and time went to the two joins).
- **mDNS on one host only**: both peers were on this machine; two machines on a real LAN were not tried.
- **Lobby screens not built** (host, join by code, quickplay, LAN, players, ready, back to lobby): P36's front end
  (`crates/openmx/src/menu.rs`, in flight in s1) isn't in main. The Cmd/Event API is what they would drive.
- **Not wired into the shell**: nothing in `crates/openmx` uses this yet; the `Disc` the shell should pass (hashes of
  the track and `.phy` files it loads) does not exist; the tests use a fake one. The ride's traffic (`Cmd::Send`,
  `Cmd::Datagram`) is not N2's rollback yet.
- **Where it lives differs from the brief**: `crates/openmx-net/` and `crates/openmx-net/tests/netplay.rs`, not
  `crates/openmx/src/net` and `crates/openmx/tests/netplay.rs` (to stay off N2's and P36's files).
- **Which settings are sim-affecting is my call**: track, each seat's bike set, wrecks, biplane, build. The rider
  (model/gear) is assumed cosmetic and not compared; if the rider's mass or anything else in it feeds the sim, the
  handshake misses it. `--phy` overrides are not represented (the bike set name + hash stand for the file).
- **Merge conflicts expected**: the root `Cargo.toml` members line (N2 adds `exclude` right after it) and
  `Cargo.lock` (N2 adds ggrs/bincode). Both are mechanical to resolve.
- Datagram size (`DATAGRAM_MAX` 1100) is a guess at iroh's path MTU less the header; not measured.

## Next

1. When P36 lands: the lobby screens in `menu.rs` driving `openmx_net::Net` (Host / Join by code / Quickplay /
   LAN list from `Event::Found`, seats with rider+bike, Ready, Start for the host, chat line, the `Dropped` message
   on the way back to the lobby).
2. When N2 lands: move `crates/openmx-net/src` to `crates/openmx/src/net` if wanted, give the shell a `Disc` that
   hashes the files it loads, and carry N2's inputs over `Cmd::Send`/`Cmd::Datagram` after `Event::Start` (seed from
   the room).
3. A DHT testnet test for Find/Quickplay.
