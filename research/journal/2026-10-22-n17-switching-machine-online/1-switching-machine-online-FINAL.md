# 1 — switching machine online (FINAL)

Task N17. 2026-10-22.

## What I did

- **The plane is in the session state already: each seat's `Sim::plane`.** game-sim's own plane lane
  (`Option<Flight>`) is cloned, hashed and encoded by `game_sim::snapshot` with the rest of the `Sim`. So the
  online state type (`rides::State`, a `Sim` a seat) did not change, and rollback, the checksums, the resync
  and the join's restart carry a flying seat with no new code. A seat is on the plane exactly when its lane is
  `Some`. I chose this over putting the shell's `Garage` into the state, because the garage holds the livery
  and drawing fields that are not ride state.
- **The switch is a byte of the seat's pad.** `net::SWITCH` is report byte 20 (the game reads bytes 0–19 only),
  and `net::WIRE` grows from 20 to 21. Its layout is `vehicle::Wanted`: bits 0–1 hold the machine wanted
  (0 none, 1 bike, 2 biplane), bit 2 every_bank and bit 3 round_stick. The seat's biplane scheme rides along
  because `Plane::step` reads it, and every machine must step the plane alike.
  - **A level, not an edge.** The byte says which machine the player wants up, on every tick. So a predicted
    pad (ggrs repeats the last one) switches nothing wrongly, and a lost or rolled-back tick cannot drop a
    switch. A refused switch (no ground, no plane) is retried each tick, so it is quiet but not free.
- **`Rides::step`** (net/rides.rs), per seat: it switches to the wanted machine first
  (`vehicle::switch_lane`, which is P102's `Garage::switch` on a scratch garage round the lane). Then, if the
  seat flies, `vehicle::fly_lane` steps the plane (P102's `Plane::step`), and the bike, the markers and the
  `after` hook are skipped, as offline only `up` steps.
- **The plane taken is one template every machine builds alike.** `rides::plane_on(stock biplane.phy, mods,
  like)` is a fresh `Flight` on the ride's modifiers, with the ride's ray probe and area. It is never a flown
  offline flight, because that may hold state the reset does not clear. `Online::planes` sets it, and a host
  modifier change (P100's `Modded::remodify`) rebuilds it and retunes the flying seats' planes (`replane`).
  Decoded and joined seats get the template's ray probe back (`Rides::probe`), since rays are a disc constant
  that the codec does not carry.
- **The shell.**
  - The pause's machine row and the M key set `online_want`, which goes into seat 0's pad every frame.
  - After the session steps, `Garage::follow` puts each seat's garage onto its `Sim`'s lane. A seat with no
    parked plane gets one in the plain livery. A changed machine snaps the camera and notes it, through
    `took`, which was split out of `switch_machine`.
  - "Open to Online" no longer refuses a player in the plane. The flight goes into the session `Sim`'s lane.
  - `web.rs` (the lib) mounts `vehicle`, and the test crates that mount `net` mount it too.

From opentennis: nothing applicable (one machine a player there).

## What the original does

The original has no online play, and its switch does not spawn on the player (P102's journal: the bike is
hidden where it was left, and the plane stays at its bay). Both the switch and its online carriage are the
port's own, so there is nothing in PCSX2 to compare against.

## How it was proved

- **tests/netplay.rs `two_peers_take_the_plane_and_the_bike_back_and_stay_in_sync`.**
  - Setup: two peers on a link with 5% loss and 3 rounds of lag, every seat mashing (`mash`). Host seat 1
    wants the plane over ticks 120–450 and g's seat 2 over 300–600 (the flights overlap). The plane is the
    disc's real `biplane.phy`.
  - Result: no desyncs, and >700 settled ticks with equal per-tick hashes and pads.
  - The logged pads were then ridden offline (no network) with the same template:
    - The state hash equals the host's before every tick.
    - The machine up after each tick is exactly the one that tick's pad asked for.
    - Each plane is put down at the bike's x/z.
    - Every 50 ticks the state is encoded and decoded onto fresh seats with an equal hash (flying seats
      included; asserted ≥ 8 went through).
    - A `Garage` following each seat switches at (121, biplane) / (451, bike) and (301, biplane) /
      (601, bike), the one-tick input delay. It draws the plane exactly where the flight's pose is, and the
      bike only when it is up.
  - **It really ran with the disc:** before I fixed its off-by-one expectation, the first run failed on
    `[(121, 450)]` vs `[(120, 449)]`, so the plane was loaded and both seats flew. (A missing disc would make it
    skip and pass.)
- **`a_wanted_machine_goes_in_one_byte`:** the byte round-trips all 12 combinations, 0 means no switch, and
  `mash` leaves byte 20 at 0. **`a_pad_goes_on_the_wire_as_its_own_bytes`** now checks 21 wire bytes.
- **Two headless shells** (`openmx --headless --frames 600 --online … --seats host,g`, debug build) rode the
  disc's track over localhost with the 21-byte wire: 591 settled, 0 desyncs, and the same `state before tick
  300 0xabe9fdbe77ecc288` on both. Their pads are idle, so no switch was made there.
- `./kit check`: TESTS PASSED (full gate). The first full run had `four_peers_and_eight_riders_on_a_bad_network`
  "g waited 30 rounds", the known load flake. It passed alone twice, and the next full run passed.

## Not verified / not 1:1

- **The whole feature is the port's own.** The original has no online and no spawn-on-player switch, so it is
  checked only against the port's own offline ride.
- **No switch was made in a real shell.** The M key, the pause row, `online_want`, `Garage::follow` in the shell
  loop and the camera snap online were not exercised. A headless online shell takes no pad or tape (`--online`
  refuses `--play`), and there is no display automation for two windows here. The session path they feed is
  what the netplay test proves, and `follow` is proved there on a test garage.
- **Starting online on the plane is still refused.** `--online` and a lobby ride start every seat on the bike,
  so a seat must switch after the start. Only "Open to Online" from the plane carries the plane in. Filed as a
  follow-up.
- **The flying seat's plane is the template, not that seat's own parked plane.** Liveries are per-machine
  (shell only), and every machine flies the same physics file, as every seat's bike set is the ride's. A
  per-seat plane file online would need it in the join's bytes.
- **The offline plane flown before "Open to Online"** goes into the lane as it is, not rebuilt. Its ray probe
  is the shell's. A joiner decodes it onto its own template's rays, so this is equal only while every machine
  has the same disc.
- **A refused switch is retried every tick** while the pad still asks (no ground under the bike, say), which is
  cheap but not free, and the console prints nothing for it online.
- **`sim.ticks` does not advance while a seat flies**, as offline (`Garage::step` steps only `up`). Online that
  affects nothing that reads ticks for that seat, but I have not audited every reader.
- **A marker teleport onto a flying player** still goes to their stale bike spot (P104, unchanged).
- **Tapes online** stay refused (`--online` refuses `--play`/`--record`), so an online switch is never recorded.
- **Seats 2–4 on one machine** have no pad online (P35), so only a machine's first seat can switch.

## Next

Nothing: the task lands. Follow-up filed: N18 (start an online ride on the plane).
