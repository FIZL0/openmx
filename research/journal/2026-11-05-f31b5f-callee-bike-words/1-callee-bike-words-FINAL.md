# 1 — callee bike words (FINAL)

Task F31b5f. 2026-11-05.

## What I did

F31b5c's census found four interpreter callees that write bike-object words but leave the physics
blocks alone. For each one I either ported it or showed its words go unread:

- **Trick hold** (vt `+0x4ec`, `./kit fn -p atv 4e4810`). Ported as `update::trick_hold`. Its words are
  read: the wreck test reads the fraction `+0x14e4`, and the hold reads its own clocks.
  - `Updates` has a new method, `noise`, the engine's smooth noise. The test bridge answers it with
    `SmoothNoise::at_accumulated` on the original's tables.
  - The trick step still hands the hold out as `Out::TrickHold`, as before. `UpdateInRam` now serves
    it with the port, the way F42's four are served. This keeps the F42 trick oracle's call logs as
    they were.
- **Settle** (vt `+0x4a8`, `./kit dis -p atv 4e4cd0`). Ported as `wreck::settle`. The census only saw it
  change `+0x1754`. The watch shows it also stores 0 into the wheelie/stoppie clocks (`+0x1744`,
  `+0x1748`) and their four words past (`+0x704..+0x710`). The frame reads `+0x704` and `+0x708`
  (update.rs reads them as well). So it cannot simply be dropped, and I ported it.
  - It calls the stance (`Out::Stance(-1)`) and a new `Out::Told(listener)` for each of the player's
    `+0x1d4` listeners.
  - Its last two calls are the bike's vt `+0x53c` and `+0x540`. Each is a single store
    (`./kit dis -p atv 4e7a30 --len 0x20`). The port writes `+0x1754` and `+0x1758` itself, and the test
    asserts both slots.
  - `WreckInRam` serves `Out::Settle` with the port.
- **Kind set** (vt `+0x3a0`) and **every event post** (`0x00272300`). Not ported. On both replays their
  bike words are shown unread, and the frame is bit-exact without them.
- The census lists change accordingly: `update TrickHold` and `wreck Settle` leave `FLIP_CALLEES`,
  which goes from 44 to 42. No `wreck Told` shows up, because the player's listener list is empty on
  state 4.
- Filed **F48** (low): the posts' camera controller writes the bike's flags at `+0x444` (see below).
- Nothing taken from opentennis. It has no counterpart to these MX vs. ATV callees.

## What the original does

- Trick hold:
  - A ramp from the hold clock `+0x14dc` over 0..0.5 s, written as a lerp on the accumulator
    (`adda 0,0` then `madd`) to 0.07, then clamped with `max`/`min`.
  - The shake is that ramp times the smooth noise at `2·(+0x14e0)`.
  - `+0x14e0`, `+0x14dc` and `+0x14e8` each go up by dt, in that order.
  - `+0x14e4 = (+0x14d8 + shake) / clip+0x48`.
- Settle:
  - Guard: the run `+0x684` and the player `+0x680` must both be set.
  - Calls vt `+0x4b0(-1)`.
  - Stores 0 into six words.
  - Tells each listener: vt `+0xa0`, with arguments (l, 5, &0, 0, 0).
  - Calls `+0x53c(0)` and `+0x540(0)`.
- Kind set: the hung parts are faded at 0.5, then `0x004a7350`. Its only bike word is `+0x9a8`.
- The post's registration for bike `+0x674` has 11 listeners. The only one that writes the bike is the
  camera's controller (`./kit fn -p atv 490e60`). It writes `+0x444`, `+0x460` and `+0x468`.
- Details are in research/oracle.txt `# F31b5f`.

## How it was proved

All tests are in `crates/game-sim/tests/mxvatv_bike_ride.rs`.

- `the_trick_hold_is_the_originals`: 10 000 cases off state04.ee.bin, comparing the original vt
  `+0x4ec` with the port. The randomized inputs:
  - the hold clock, both sides of its ramp (each of the three bands ≥ 1000);
  - the other clocks, the clip's run and dt, with zeros, denormals, huge values and any bits.

  The bike's words, the clip and the noise's knot all match word for word.
- `the_settle_is_the_originals`: 10 000 cases. The randomized inputs:
  - run and player present or absent;
  - a fake listener list 0–4 long;
  - the eight words, with random bits.

  The calls (stance, each listener's, hooked on the original's side) match in order, and so do the
  words. All three paths are ≥ 1000.
- `the_kind_sets_and_posts_bike_words_are_unread`: C15 ride-2 (459 rows) and flip-1 (465 rows), run
  whole in the interpreter with every row bit-exact. Each of the four writers runs as itself. Every
  bike-object byte it stores is marked, and every later read of a marked byte, before the frame stores
  it again, is logged. The stored words are pinned exactly. The kind set's are never read. The posts'
  are read only by a later post, apart from `+0x444` (next section).
- `the_frame_without_the_kind_set_and_posts_is_bit_exact`: both replays with every post and the kind
  set returning at once. Every row is bit-exact, and the wreck state matches. After every frame, the
  whole bike object and its blocks match the full frame's, apart from `+0x460`/`+0x468` from row 125.
  `+0x9a8` is stored again within its own frame.
- The F31b5c census tests are green with the ports in place, so the port's frame stays bit-exact on
  both replays.
- Ran: `./kit check -- -p game-sim --test mxvatv_bike_ride` with the filters above. The full gate runs
  in `./kit done`.

## Not verified / not 1:1

- **The bike's flags at `+0x444`.** The posts' camera controller stores them on flip-1 rows 125 and 193,
  and the frame reads them (in `update::sync`'s camera mask). On these two replays the store puts back
  the value that was already there, so dropping the posts is bit-exact. I did not show from the
  controller's code (`0x00490770`) that the store can never change the flags. Filed as F48 (low).
- **Only C15's two replays on state 4.** The posts and the kind set are shown to be unread on these
  rides only. Paths of the kind set and of the camera controller that these replays do not reach are
  not shown inert.
- **Effects outside the bike object are not watched.** The kind set's record-part calls and part fade,
  and the posts' other ten listeners (HUD, audio, camera), are out of scope. The census watched the
  physics blocks: none of them changed.
- **Settle's listeners** (`Out::Told`) are still interpreter calls. On state 4 the list is empty, so
  nothing reaches them. The oracle only checks that they are called the same way on both sides, with
  hooks standing in for the listeners.
- **The trick hold's `noise`** is answered by `SmoothNoise::at_accumulated` (an earlier port) on the
  original's tables. The oracle exercises it on the image's tables only. Its lazy table build is the
  original's builder, run from the test bridge.
- The read watch has the same limits as F31b5d's. The reads are a set per stretch of the frame. A
  writer reached inside another writer counts as the outer one. Reads by the writer itself are logged
  under its own name.

## Next

Nothing for F31b5f. F31b5 can serve the trick hold and the settle from Rust and drop the kind set and
the posts. F48 covers the `+0x444` store for rides other than C15's.
