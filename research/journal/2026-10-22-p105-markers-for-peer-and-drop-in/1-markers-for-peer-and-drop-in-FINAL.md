# 1 — markers for peer and drop in (FINAL)

Task P105. 2026-10-22.

## What I did

P99 carried a player's saved markers (`markers/<track>/<profile>.markers`) online only in a lobby pick. Two
online paths have no lobby pick, so they started every seat empty. Both paths now carry the markers:

- **`--peer` (plain UDP, `net/udp.rs`).** Each machine sends its player's file to every peer in pieces of at
  most 1 KB. Each piece is a `MARKS` datagram: `[len u32][index u16][bytes]`, where the bytes come from
  `saved_markers::to_wire` (the track's name, then `Markers::to_file`). The piece's head carries the build id and
  the card like any other datagram, and an empty file still sends one piece. `Udp::markers(&lineup, me)` returns
  `None` until every peer's pieces are all in. Once they are, it returns each seat's bytes: a peer's first seat
  gets its file, the rest get nothing. `Online::frame` offers this machine's pieces every 100 ms until the first
  tick. It hears and ticks nothing until every peer's markers have arrived, then puts `from_wire` on every seat.
  The result is that every machine starts every seat on the same slots, and a machine slow to load holds the
  others back the way the ggrs sync already does. MAGIC went from `omp` to `omq`. A datagram with the old magic
  gets a refusal message ("a build from before P105") instead of a silent hang.
- **Drop-in.** A joiner does not know the ride's track until the host's `Start`, and the room takes no new pick
  once the ride is under way. So the markers travel in the handshake. `Disc::markers(track, bike)` is a new
  default-empty hook. The guest's `Start` handling calls it for a drop-in and puts the answer in
  `Handshake::markers` (`#[serde(default)]`, never compared). The host writes that text into the joiner's
  seat pick, broadcasts the room, and only then raises `Arrived`. Online's `Arrived` handling reads the pick
  (`from_pick` on the room's track and the pick's bike) and builds the spawn with `rides::spawn_marked`. That
  spawn goes into the restart state every machine takes the join on, so every machine has the same slots.
  `DiscFiles` gets the settings folder and profile from the front end each frame (`bring_markers`), and its
  `markers` reads the file on the ride's bike. main.rs sets the drop-in's bike to the room's first seat's
  bike, as the ride does.
- `--online` from the command line (`--peer`) loads the profile `args.look.name` (or `1P`) on `args.track` and
  `bike_words[0]`, and hands it to `Online::saved_markers`.
- `tests/lobby.rs` mounts `saved_markers.rs` beside `online.rs`, which now calls it.

## What the original does

Nothing. Session markers are the port's own feature, so there is no oracle. Nothing from opentennis applied:
it has no saved per-player ride state to send at a join.

## How it was proved

- `crates/openmx/tests/markers_online.rs` (2 tests, via `./kit check -- -p openmx --test markers_online`):
  - The `--peer` test runs `host,g,k,k` over real localhost UDP. The host brings 2 slots, g none, and k 5
    (more than 4 pieces). k offers its pieces only from round 30. No machine ticks before round 30. Every
    machine starts the seats on the same slot signatures: host's, none, k's, none. Every settled tick has the
    same state hash and pads on all three. Up (a lone Up in the mash pads) rides seats 1 and 3 onto one of
    their slots in the settled ticks. Seats 2 and 4 never land on one.
  - The drop-in test runs host-only, then g drops in with 3 slots through `to_pick` → `from_pick` →
    `spawn_marked` → `admit`, over an in-memory wire with lag 2. Over 600 rounds, more than 300 ticks settle
    from the join with the same hashes and pads. Both machines end with g's seat holding its slots and the
    host's seat holding none. Up rides g's seat onto a slot, and the host's seat never lands on one.
- `crates/openmx-net/tests/drop_in_markers.rs` (3 tests, LAN mDNS, nothing off the machine):
  - A drop-in's disc is asked once, on the ride's track and bike (not the guest's own pick). The host's room
    holds the markers in the joiner's pick before `Arrived`, and the host's own pick is untouched. The guest's
    room shows them too.
  - A lobby `Start` asks for none.
  - Handshakes that differ only in markers still match.
- New `net::udp` unit tests: an old-magic peer is refused with the P105 message, and every seat gets its
  markers only once every piece is in, whatever order the pieces arrive in.

## Not verified / not 1:1

- **No live two-shell run** of either path. Every piece the shell calls is tested. The wiring in main.rs and
  lobby.rs (profile and folder per frame, `--peer` load) compiles but was not watched in a running shell. Reason:
  an unattended two-window drop-in with a placed marker needs a tape and a host. The headless test drives the
  same `Netplay`/`Udp`/`rides` code instead.
- **Old builds can no longer `--peer` with this one.** The magic bump is deliberate: an old build would tick
  without waiting for markers and desync. It gets a refusal message, not a ride.
- **A drop-in joiner with several local seats gets markers only on one seat.** The handshake carries one
  player's file (1P's profile), and the host puts it on the joiner's seat only when it has exactly one.
  Reason: a profile per extra seat would need a list in the handshake. That case is rare, so it is left for later.
- **On `--peer`, only each machine's first seat brings markers.** Same reason: one profile per machine.
- **Pieces are resent every 100 ms until the first tick.** Up to about 12 datagrams per peer per resend, with no
  ack. Reason: simple, and it ends at the first tick. On a very lossy link the start waits for a full set.
- **`--peer` assumes every machine names the same track and bike** (as it already did for the ride). A file for
  another track or bike reads as none on every machine alike, so it cannot desync. It just starts empty.
- **The joiner's file is rewritten with the same content** after its first placed slot, by P99's `Kept`
  signature. This is harmless and was not tested.
- **The room broadcast now carries a drop-in's ~24 KB of hex** in its pick, as P99's lobby picks already do,
  well under the 1 MB message cap.
- The two markers_online tests prove the markers are in the ride by the bike landing on a slot within one run,
  not by comparing against a run without markers. Reason: a machine samples its pads by the frame it is on,
  so two runs of the same test do not ride the same pads tick for tick.

The full `./kit check` passed before `./kit done`.

## Next

Nothing for this task.
