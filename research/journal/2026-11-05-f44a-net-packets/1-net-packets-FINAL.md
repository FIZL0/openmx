# 1 — net packets (FINAL)

Task F44a. 2026-11-05.

## What I did

- `crates/game-sim/src/handling/update.rs`: ports of the three packets `send` calls out, and what they end in:
  - `net_stunt` (bike vt `+0x544`). It builds a 16-byte packet: the flag byte (2, plus 4 while `+0x14e8` > 0), the time stamp, and
    five fields copied through the net record's pointers. The stamp comes from the timer count word and the hardware counter. It
    is read between `Out::IrqOff` and `Out::IrqOn`, as `Out::Counter`.
  - `net_full` (vt `+0x400`). The 41-byte whole state: the flag, the net clock, and the three vectors. Each of their nine floats is
    then kept in its quantiser (`keep`).
  - `net_delta` (vt `+0x404`). The change: nine `quantise`s packed from byte 7, behind a 3-byte mask (sizes, turned, fast) and the clock.
  - `lookup` / `keep` / `quantise`: the quantiser table. `lookup` is a hash-bucket walk through `Out::Hash` and `Out::SameKey`. For
    state 4's class those are ported as `decimal_hash`, which hashes the key's `%d` text (shift and xor of each lower-case
    character), and `same_key`.
  - `sender_send` (the sender's vt at `+0x38`, `+0x28`). The 2-byte header and the flags word, then `Out::NetQueue`.
  - New `Out`s: `SenderSend`, `NetQueue`, `Hash`, `SameKey`, `IrqOff`, `Counter`, `IrqOn`, and the `Packet` type (len plus 64 bytes).
- `tests/mxvatv_bike_ride.rs`:
  - `UpdateInRam::out` runs each port when the slot holds the original it stands for and no hook sits on that original.
  - `hooked()` no longer stubs the stunt packet. It rewrites the packet's two RCNT0 instructions (asserted first) to read a scratch
    word instead. So the original stunt packet, and the port, now run on every replay in this file.
  - The census lists: `NetDelta`, `NetFull` and `NetStunt` leave the interpreter. `IrqOff`, `IrqOn` and `NetQueue` are what is left.
  - F41's rider-read watch logs the stunt packet's reads separately and asserts it sends only the rider's word `+0x940`. The packet
    reads a word the rider writes, which is how it showed up.
  - A new oracle, `the_bikes_net_packets_are_the_originals`.
- research/oracle.txt `# F44a`: every address, slot and state-4 value.
- Plan: F49 (the connection's queue, and the other test files' stunt stubs).
- opentennis: nothing to take. Its netplay is ggrs, not this sender.

## What the original does

To see each function: `./kit dis 0x004e7e70 --len 0x140 -p atv` (stunt), `0x004a8e30` (whole), `0x004a90c0` (change), `0x00267620`
(quantiser), `0x00267ba0` (lookup), `0x00267910` / `0x00267980` (keep), `0x001c3820` (hash), `0x0025e9d0` (send). The full
description is in research/oracle.txt `# F44a`.

Some points that are easy to miss:
- The quantiser's new last value is `adda(0, x).madd(q, step) − d`, a fused sequence, not `x + q·step − d`.
- The stamp is converted to an unsigned int through the 2^31 split.
- The change's mask is 1 | (len−1) << (2+2k), with bit 20 for turned and bit 22 for fast (`+0x318` above 0.1).
- The stunt reads RCNT0 in the delay slot just before EI.

## How it was proved

- **Function oracle.** `the_bikes_net_packets_are_the_originals` ran 10 000 generated cases against SLUS-21104's state 4 in Ee,
  over all eight functions.
  - Compared: every queued packet (header, bytes, conn, flags), the answer, and the bike, the table, the entries, the record, the
    vectors, the timer word, the clock and the out bytes, word for word.
  - Each of the 26 paths is taken at least 100 times. That includes all four byte sizes, the dead band, no room, a missing key,
    the stamp past 2^31, and negative hashes. A quarter of the quantise cases sit exactly on or one below the ±0x80 / ±0x8000 /
    ±0x800000 thresholds.
- **Mutants.** Four mutants of the port all failed the oracle: the 0x8000 and 0x800000 thresholds moved by one, and the mask's
  turned bit. The first threshold mutant passed until the edge cases were added; it now fails.
- **Replays.** ride-2 and flip-1 stay bit-exact, including their port record passes, with the ports in the tail. A temporary
  counter showed that ride-2's record pass runs 46 stunt, 15 whole and 138 change ports.
- `./kit check -- -p game-sim --test mxvatv_bike_ride`: 64 tests passed. The full gate runs in `./kit done`.

## Not verified / not 1:1

- **RCNT0's real count is not captured.** Ee cannot read hardware registers. In the replays the stunt packet's counter is a
  stand-in word of 0. In the oracle it is random, but both sides read the same word. The stamp's arithmetic is proved, but not the
  value the PS2 would have stamped.
- **DI / EI are hardware.** The port emits `IrqOff` / `IrqOn`. The harness serves them with the originals, hooked to return 0.
  Their interrupt effect is not modelled.
- **The hash's `%d` formatting** is ported as plain decimal formatting. It is proved only through the hash oracle on random keys,
  including negative ones, and not against snprintf on its own.
- **The connection's queue and its flush are still the original**, through `Out::NetQueue`. That is F49.
- **Hash and eq classes other than state 4's** still run their originals through `Out::Hash` / `Out::SameKey`. The port is only
  used when the table's slot holds state 4's functions.
- **The other test files still stub the stunt packet.** Only mxvatv_bike_ride.rs runs it, and those files' replays queue fewer
  packets than the game would. Those files are unchanged by this task and still pass as before. F49.

## Next

Nothing for this task: it lands. F49 picks up the queue.
