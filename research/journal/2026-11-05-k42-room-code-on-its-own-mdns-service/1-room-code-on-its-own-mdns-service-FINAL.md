# 1 — room code on its own mdns service (FINAL)

Task K42. 2026-11-05.

## What I did

**Why the guest missed the listing.** The LAN goes through `iroh-mdns-address-lookup` 0.6.0, which runs on
`swarm-discovery` 0.6.4 at `Discoverer::new_interactive`'s cadence (τ = 0.7 s, φ = 2.5). Read in
`swarm-discovery/src/sender.rs`: every endpoint on a service answers each PTR query only after a random
delay of up to `100 ms × swarm_size / cutoff`. It stays quiet if `cutoff = ⌈τφ⌉ = 2` answers arrive first.
An endpoint that answered last round also waits longer, for fairness. Nobody sends a record on its own
when its data changes: a new listing goes out only when the host wins a round. So with N endpoints on
`_openmx._udp.local` (every LAN `Net` of every lobby binary on the machine), a given host is heard in about
2/N of the rounds. The guest's `LAN_CODE_WAIT` (20 s) is about 25 rounds, and the chance of missing the
host altogether grows quickly with N. The lookup wasn't losing a packet. The protocol keeps a big swarm
down to two answers a round on purpose. The listing's TTL and expiry (`updater.rs`'s grace,
3 × N·τ / cutoff) were not the cause: they only drop peers, and here the host had never been heard.

**The fix, in `crates/openmx-net`:**
- `board::code_service(code)` gives `openmx-<code in lower case>`, so a room has its own mDNS service.
- `board::CodeListing` is an `AddressLookup` registered on the endpoint next to the main mDNS lookup (only
  when the LAN is on). It resolves nothing. It keeps the endpoint's last published `EndpointData` (its
  addresses and the listing in user data) and passes it on to an advertising `MdnsAddressLookup` on the
  room code's service while the host has a code. `list(id, Some(code))` starts that lookup, `None` stops
  it, and the same code twice leaves the lookup alone.
- `net::host` calls `list` from its listing pass: the room's `short` code, or none for a private room or
  before the code exists, which matches what goes into the main listing. An `Unlist` drop guard takes it
  down however `host` returns (leave, handover, any other return).
- `net::resolve` (a code typed with the LAN on) also starts `board::watch_code`. That is a listen-only
  (`advertise(false)`) lookup on the code's service, which hands back the `EndpointAddr` of the endpoint
  whose listing has that code. On that service the host is the only one that answers, so suppression
  never holds it back, and the first query round (≈0.7 s plus a little) reaches it. The check of the old
  LAN-wide map (`board::lan_code`) stays in the loop as before, so whichever is heard first wins. The
  watch ends when `resolve` returns, because its receiver is dropped.
- `crates/openmx/tests/lobby.rs`: only `by_room_code`'s doc comment changed, to say why the test is now
  robust. No timeout changed. `LAN_CODE_WAIT` is still 20 s.

**Taken from opentennis:** nothing. Its `hst-base/src/net.rs` has the same single shared service
(`service_name(MDNS)`) and so the same limit.

## What the original does

Not applicable (kit task: the LAN lookup and its test).

## How it was proved

- I built the `lobby` test binary before the change (HEAD) and after it. I ran N copies of the whole
  binary at once with `wide.sh` (a scratchpad script: N background copies, each `wait`ed on by its pid) and
  counted the room-code test's result. The load average was 90-140 from the other slots.
  - before, 16 wide: **2 of 16 failed** ("both seated: not in 30 s", lobby.rs:302); binaries took 9-31 s.
  - after, 16 wide, three rounds: **0 of 48 failed**.
  - before, 32 wide: **18 of 32 failed**; binaries took 13-33 s.
  - after, 32 wide: **0 of 32 failed**; every binary finished in 13.7-15.7 s.
- `./kit check -- -p openmx-net` (26 tests, with the new
  `a_room_code_s_service_is_a_short_mdns_name_of_its_own`) and `./kit check -- -p openmx --test lobby`
  (80 tests): pass.
- I built `openmx-net --lib` for wasm32 with the web build's scratch toolchain. It builds with no
  warnings: `CodeListing::list` does nothing there and `watch_code` returns a closed receiver.

## Not verified / not 1:1

- **Two real machines on a real LAN were not tried.** Every run is on this one machine's loopback and
  multicast. A second service means a second multicast socket set on port 5353 per host while it hosts,
  and one per guest while it looks a code up. On this machine they bind with port reuse. A network or OS
  that allows only one mDNS socket per process would print `mDNS <code>: …` and fall back to the old
  LAN-wide lookup, which still runs. I have not seen that case, only read the code that handles it.
- **The before-change 8-wide figure (0/8) was not rerun.** It comes from K38a's journal. I measured 16
  and 32 wide instead.
- **A private room still cannot be joined by room code on the LAN.** That was already true, because its
  main listing is empty too, and I kept it that way on purpose: it is not part of this task.
- **Two LAN hosts with the same room code would both answer on the one service.** The watch takes the
  first listing whose code matches. The same thing could already happen with `lan_code` (the first match
  in the map), so nothing new. Unlikely: the codes are 20 bits, and only a host that is online re-rolls
  a taken code; a LAN-only host does not.

## Next

Nothing: the task is done.
