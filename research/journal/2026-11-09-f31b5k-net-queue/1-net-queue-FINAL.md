# 1 — net queue (FINAL)

Task F31b5k. 2026-11-09.

## What I did

- **Ported the connection's queue** (`update::net_queue` and `queue_push`, `crates/game-sim/src/handling/update.rs`).
  This is where `sender_send`'s `Out::NetQueue` ends up. It works at word level over RAM through `Updates`, like
  the rest of F44a. The flush's socket send is the one call out, the new `Out::NetFlush { socket, len, reliable }`.
  New constants: `CONN_SOCKET`, `CONN_UP`, `CONN_QUEUES`, `FLUSH_BUFFER`.
- **Served it in the replay rig.** `UpdateInRam::out` answers `Out::NetQueue` with the port whenever no oracle
  hooks the original (the same `ported` rule as F44a). `Out::NetFlush` is handed to the original socket send with
  its six arguments, so the rig stays complete even though state 4 never reaches it.
- **Oracle** `the_connections_queue_is_the_originals` (`crates/game-sim/tests/mxvatv_bike_ride.rs`), described
  below.
- **The replays**: `ride_/flip_replays_with_every_port_at_once` now also assert that the interpreter census
  (`ran`) has no NetQueue entry. Ride-2's census loses its 1.25M-instruction "the update's NetQueue" line. The
  F31b5c census lists `RIDE_CALLEES` / `FLIP_CALLEES` lose "update NetQueue".
- I chose to port the queue rather than show it inert. The task allowed either, but the queue does write RAM (the
  connection's buffers), and the every-port replays compare the whole RAM against the handing run's after every
  frame. With the port, they stay RAM-identical, and nothing has to argue that no reader exists.
- Nothing taken from opentennis: its netplay is ggrs/iroh. It has no copy of this game's connection queue.

## What the original does

`./kit fn 0x00282190 -o atv` and `./kit fn 0x00282ea0 -o atv`. Read instruction by instruction through a scratch
`KIT_CONFIG` (context `context/mxvatv-ps2`, `elf/boot.elf` a link to `atv_game.elf`) and `./kit dis`.
Ghidra drops the header argument on the push calls; it stays in `$5` throughout. Details are in
`research/oracle.txt` `# F31b5k`. In words:

- The connection has five queues, each `{count byte address, start, end, room}`. Each message is two header
  bytes (the sender's id, the length) followed by the data, starting at `start + 1`.
- **Which queue.** With flags bit 1, the push goes to the second pair (`+0xfc`, then `+0x10c`). With bit 4, it
  goes to `+0x11c`, but only while connected, and the answer is 1 whether or not it was taken. Anything else goes
  to the first pair (`+0xdc`, then `+0xec`).
- **When the pair's first queue refuses.** If the connection is up (`+0x2c` set, socket `+0x28` not negative),
  that queue is copied into a fixed buffer and sent out the socket (reliable `1, 1000` for bit 1, `0, 0` for the
  plain pair). If the send does not fail, the queue is emptied. Either way, the message then tries the pair's
  second queue, and its answer is the function's answer.
- **A push that doesn't fit drops messages from the front.** It walks the messages, stopping at the first one
  whose own size plus the free room (the room that was already free, not counting what earlier drops freed)
  covers the new message. If no message within the count does, it refuses and changes nothing. Otherwise it
  moves the rest down (the forward memcpy, which acts as memmove here) and lowers the count. If every counted
  message went, the queue is reset instead.
- An empty queue that is too small still looks at one message past its end.
- **State 4 is offline** (`+0x2c` is 0, `+0x28` is -1), so a C15 ride never flushes. The first queue fills and
  then drops from the front, and the overflow queue `+0xec` stays unused for as long as drops make room.

## How it was proved

- **The oracle**: `the_connections_queue_is_the_originals`, 10 000 generated cases on the state04 image.
  - Each case fills the five queues with random messages. The room is 0x514 one time in four, small otherwise.
    One queue in three is packed to the brim with tiny messages, so no single drop makes room.
  - It also draws the connected word and socket, the flags (0, 2, 0x10, 0x12, or random), and the header and
    0–64 data bytes. The hooked socket send answers success or failure.
  - SLUS-21104's function and the port (through `UpdateInRam`) start from the same words. Every word of the
    connection, all five buffers and the flush buffer matches, as do the socket sends' six arguments and the
    answer. The original writes nothing outside those regions.
  - Paths (each ≥195): plain 3712, bit 4 2007, bit 1 4281, refused 590, flushed and sent 210, flushed reliably
    and sent 280, flush failed 439, emptied by the flush 446, pushed into room 6474, some messages dropped 1342,
    every message dropped 195.
- **The replays**: `./kit check -- -p game-sim --test mxvatv_bike_ride -- every_port_at_once nothing_handed_on
  tail_callees_leave net_packets`: 7 passed. Ride-2 and flip-1 stay bit-exact row by row, with the RAM as before.
  The census printout shows no NetQueue.
- Gate: `./kit check --all` in `./kit done`.

## Not verified / not 1:1

- **The socket send itself (`0x002845f0`) is not ported.** It is the connection's I/O. The port hands it on as
  `Out::NetFlush`, and the oracle hooks it. State 4 is offline, so no C15 frame reaches it. A pure frame has
  nothing to answer it with offline; an online one needs our own transport, not the original's socket layer.
- **The oracle draws only well-formed queues**: count equal to the messages present, under 256 messages,
  `end - start` within the room, the count byte at `start`, packets of 64 bytes or fewer. Corrupt queues (a count
  past the messages, or a start that differs from the count byte's address) are not covered. In those cases the
  original's memcpy length can go negative. The bike's sender cannot produce them.
- **The flush buffer is compared only over 0x530 bytes** (the largest queue span). A flush is never longer,
  because a queue never holds more than its room.
- **The queue's byte stores go through `set_byte`'s word read-modify-write.** The resulting RAM is the same, but
  the order and width of the stores is not the original's memcpy's. Nothing else runs in between, so no reader
  can see the difference.
- **No new capture.** The replays are the existing C15 rides. The connected paths (flush, `+0x11c`) are proved
  by the oracle only, never seen live, because state 4 is offline.

## Next

Nothing for this task. F31b5 still waits on F31b5i, F31b5j, F31b5l, F31b5m and F54.
