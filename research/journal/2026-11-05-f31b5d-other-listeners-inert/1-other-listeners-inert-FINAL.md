# 1 — other listeners inert (FINAL)

Task F31b5d. 2026-11-05.

## What I did

- Listed state 4's sim lists from the RAM image. The world (sim `+0x6c`) has one body (the bike's), six
  preforce elements (the bike's `+0x30` and five riders', all using the one preforce thunk), one force
  element (the bike's `+0x3c`), no constraints and one post element (the bike's `+0x34`). The sim's
  `+0x24` list has one element, the bike's collision record. The `+0x3c` list is empty.
- `listeners_flow` (tests/mxvatv_bike_ride.rs) runs C15's ride-2 and flip-1 whole in the interpreter,
  as `replay` does, with every row checked bit-exact against the capture. On every frame it also checks
  the world's five lists (before and after the frame) and what the `+0x24` and `+0x3c` getters hand the
  frame (hooked to log). It has three modes for the other listeners:
  - **Watched.** Each other listener runs as itself, from its hook. The bytes it writes are marked. The
    rest of the frame's reads (`Ee::reads`) are checked against the marked bytes at every boundary, and
    a byte the frame writes again loses its mark. The bike's own listeners are hooked as well, so each
    read can be put down to a stretch of the frame: record clear / pass, PreProcess, Force, PostProcess,
    vehicle update, or the rest.
  - **Run.** The other listeners run unhooked.
  - **Dropped.** Their hooks return at once. The rider's `+0x920` is poked from row r after frame r, the
    way the put-back spot is.
- Two tests:
  - `only_the_riders_writes_are_read_by_the_bikes_frame` (Watched).
  - `the_bikes_frame_without_the_other_listeners_is_bit_exact` (Run against Dropped, compared every
    frame over the bike object's 0x1800 bytes and its six physics blocks).
- No product code changed. This is proof only, which F31b5's pure frame needs before it can leave these
  listeners out.

## What the original does

- The sim frame (`./kit fn 0x00204040`) calls each `+0x24` element's vt `+0x7c`, then each one's vt
  `+0x84`. It then steps the world, walks the tail (`0x00128af0`) and runs the `+0x3c` list's vt `+8`.
  On state 4 the `+0x24` list is only the bike's record (`0x00204c30` / `0x002051f0`), and the `+0x3c`
  list stays empty on every frame of both rides.
- The world step calls the preforce thunk `0x004de0c0` on the six elements. The five that are not the
  bike's are other riders (F31b5's existing check). The force list's code is `0x0049bd50` and the post
  list's is `0x004de0d0`, each on the bike's element alone.
- The tail's other leaves are `OTHER_LEAVES` (F39). One is `0x00eb6420` (step `0x001c9540`). The other
  is the manager `0x01ed82d0` (step `0x002cf9f0`), whose own walk steps the rider `0x010af350` (step
  `0x004f2ad0`).
- **What the frame reads of theirs.** Nothing written by the five PreProcess, the first leaf or the
  manager's own work is ever read by the rest of the frame, on any row of either ride. The rider's
  writes are read, though. Because the reads are a set, this list over-counts (see below):

  | read by | ride-2 | flip-1 |
  |---|---|---|
  | Force | 875 words, rows 1..=458 | 951 words |
  | PostProcess | 20 words | 90 words |
  | record pass | 1 word | 89 words |
  | vehicle update | 29 words | 179 words |
  | the rest of the frame | 527 words | 2605 words |

  The words are spread over 0x5415c0..0x1f515ec. They look like the rider's ragdoll parts in the
  collision world and shared scratch buffers. I did not chase which reads are real (see Not verified).
  None of it changes the bike: with the rider dropped, both rides stay bit-exact. The one exception is
  `+0x920`, the rider's "a ragdoll part is touching" flag, which the wreck clock reads (F41). With every
  other listener dropped and that flag *not* fed, ride-2 still goes bit-exact on all 459 rows. flip-1
  parts at row 193, the put-back (121 words of body, node, …), because the clock never sees the flag.
- **What they write into the bike.** Outside the rider's windows they write nothing of the bike object or
  its physics blocks. Without the others, the bike's words that part from the full frame's are exactly
  the rider's:
  - ride-2: `+0x930..+0x938` and `+0x940`.
  - flip-1: `+0x870..+0x888` but `+0x87c`, `+0x890..+0x898`, `+0x920` (fed, so one frame late),
    `+0x930..+0x938`, `+0x940`, and `+0x950..+0x97c` but `+0x95c`.

  That is the set F39 wrote down in `OTHER_LEAVES`' comment.

## How it was proved

- Bit-exact replay of both C15 captures, ride-2 (459 rows) and flip-1 (465 rows), off
  `context/mxvatv-ps2/state04.ee.bin`, run three ways (Watched, Run, Dropped). Every row's body, node,
  record, wheels and engine match the capture word for word in all three, and the wreck state matches
  too (flip-1 wrecks at row 125 and is put back at row 193).
- A full-RAM write and read watch on the other listeners (`Ee::written`, `Ee::reads`).
- A frame-by-frame comparison of the bike object and its blocks between Run and Dropped, with the words
  that part asserted exactly.
- Timing: the Watched test takes about 80 s and the Dropped test about 25–60 s, depending on load.

## Not verified / not 1:1

- **The rider's reads list is an upper bound, not exact.** `Ee::reads` and `Ee::written` are sets per
  stretch of the frame, with no order. So a byte the frame writes and then reads within one stretch (a
  shared scratch buffer, say) counts as read. I did not separate the real reads from those. It does not
  affect the result, because the Dropped replay shows none of them changes the bike.
- **Only these two rides on state 4.** "Inert" is shown for C15 ride-2 and flip-1, on state 4's world.
  The bike's Force and record pass do read the rider's writes. A ride where the rider's ragdoll parts sit
  in the way of the wheel rays or the record's searches (F46's database half) could depend on them. On
  another state, the lists could also hold more than they do here.
- **The rider's `+0x920` flag is still an input.** The Dropped frame takes it from the capture, a frame
  late. Porting it needs the rider's ragdoll (S278, F41).
- **The pure frame itself is not built here.** These tests run the original's frame with the others
  dropped. F31b5 drops them from its pure-Rust `steps_frame` once F31b5c, F46 and F47 land.
- The interpreter's stack region is left unmarked in the Watched run. A stack frame is written before it
  is read, but a real read of a dead frame there would go unseen.

## Next

Nothing for F31b5d. F31b5 can leave out the other listeners, apart from the rider flag input.
