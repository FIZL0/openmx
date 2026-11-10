# 1 — frame post fade search kind clear (FINAL)

Task F31b5l. 2026-11-09.

## What I did

- **The tail's frame post is Rust's** (`"frame post"` in `IN_RUST`). In
  `crates/game-sim/tests/mxvatv_bike_ride.rs`, `TailInRam` serves `tail::Out::Message(id)` with
  `frame_post`: `reset::post` (already ported) on state 4's queue for the bike's poster (`+0x674`),
  message `tail::FRAME_MESSAGE` and the bike's id. The registration's tells go to `FramePost`. Each
  listener is told the original's code, keyed by its receive (`FRAME_LISTENERS`, seven kinds, eleven
  listeners), booked by name, and watched: no tell may change a word of the bike or its physics blocks.
  An unknown listener panics. With `"frame post tells dropped"` nobody is told.
- **The wreck fade's world search is Rust's** (`"fade search"`). The new
  `atv_search::collide_world` runs MX's `broad_phase::probe_collide_world` through a new
  `probe_collide_world_passing`, which takes a `retired` predicate. That gives ATV's two differences:
  1. the record must be in a space to be active;
  2. the static pass passes over a retired object (first word 1) before marking it.

  The part probe gets a `broad_phase::Probe` impl and its box (`atv_part::bounds`, the part's vt
  `+0xc4`). In the test, `fade_search_in_ram` reads the world (`GatherInRam::read` plus the plant
  groups' parts) and runs the search. It writes back the probe's last/budget, the groups' `+0x88`,
  the static marks, the tree's query list (left open, as the original leaves it) and the part record.
  `GatherInRam::write_world` (split out of `write`) now copes with a query that stays open: it keeps
  the taken pool nodes and resets the pool only once the query has closed.
- **The kind clear is Rust's** (`"kind clear"`). New `update::kind_clear` (vt `+0x39c`), with
  `kind_three`, `show` (vt `+0x398`), `category` (the category table's name→bit, adding a missing
  name) and `uncategorise`. Two new `update::Out`s go out from the show: `ObjectOn(obj)` (the shown
  object's switch-on, its vt `+0x20`) and `Insert{obj, space, at}` (the spatial insert). The test still
  hands both to the interpreter (28 and 707 instructions on flip-1).
- **Every-port tests** (`ride_/flip_replays_with_every_port_at_once`) now also assert `three_served`:
  no `the tail: out Message`, `the tail: the update's Search` or `the tail: the update's KindClear` in
  the census. The tail's `out` book skips the Message when the post is Rust's (its tells book
  themselves). Flip-1 passes with the RAM the handing run's except for the cue's word, as before.
- **The listeners' reach**: `ride_/flip_replays_without_the_frame_posts_tells` run every port with no
  listener told. Each row is checked against C15 and the bike and its blocks against the handing run's.
- **Oracles**: `the_parts_box_is_the_originals` and `the_kind_clear_is_the_originals` (below).

## What the original does

- **Frame post.** `./kit fn 0x00272300 -o atv` is `reset::post` over the queue at `0x00442eb0`. Eleven
  listeners hear it each frame (receives in research/oracle.txt "# F31b5l"):
  - the HUD (`./kit fn 0x002f7a70 -o atv`): speed into its gauges;
  - the race progress (`./kit fn 0x0029bee0 -o atv`): walks its rider along the track nodes;
  - a speed flag, which is inactive on state 4;
  - a menu list, inactive;
  - five pickers, the camera controller and an empty receive: nothing for id 4.

  None is the bike's code.
- **Fade search.** `./kit fn 0x001148e0 -o atv` with `(bike +0xcc, 0)`. It is MX's
  `probe_collide_world`, with the space-link activity test and the retired pass-over. The static query
  (`0x0019d2b0`) is never ended. On flip-1 it runs 91 times, ~30k instructions each, and never hits
  (flags `0x12` stay `0x12`).
- **Part box.** `./kit dis 0x001bd1d0 -p atv`. If the part has nodes, the box is node 0's (centre
  `+0xc`, half `+0x18`); otherwise it is ray 0's midpoint and half span. The box goes through the frame
  as a VU0 `vaddax/vmaddax/vmadday/vmaddz` chain for the centre and `vabs` rows summed `vadda/vmaddw`
  for the half; w is 1.
- **Kind clear.** `./kit fn 0x004e1910 -o atv`, then `./kit fn 0x004a71c0 -o atv`. Unless the state's
  kind is 3:
  1. it releases the hung parts;
  2. it shows the bike (`./kit fn 0x004e1870 -o atv`): switch the object on, and insert it into its
     space if it is in none;
  3. it marks it dynamic;
  4. it switches both parts on and removes four category bits from their lists. The bits are the
     interned `DYNAMICOBJECT`, the bike's own, the loaded one's, `PHYSICSBODY` and `PHYSICSOBJECT`.
  5. it releases the model.

  The table lookup (`./kit fn 0x00113d00 -o atv`) appends a missing name. On flip-1 it runs once
  (6,385 instructions).

## Census (flip-1, every port on, instructions the interpreter still ran)

| what | flip-1 |
|---|---|
| frame post tells: HUD | 455k |
| frame post tells: race progress | 474k |
| frame post tells: picker (×5) | 60k |
| frame post tells: camera / menu list / speed flag / empty | 21k / 14k / 10k / 2k |
| the update's ObjectOn / Insert (kind clear's show) | 28 / 707 |

The fade search (2.8M) and the kind clear (6.4k) no longer appear. The post itself is Rust's; its
tells sum to the old `out Message` (1.04M).

## How it was proved

- **Bit-exact replays** of C15 ride-2 (459 rows) and flip-1 (465 rows) off
  `context/mxvatv-ps2/state04.ee.bin`, with every port on and the three Rust's. Each row is word for
  word C15's and the whole RAM is the handing run's after every frame. Both pass
  (flip-1's RAM differs only at the cue's word, as before).
- **`the_parts_box_is_the_originals`**: 20,000 random part probes (half with a tree, half rays; floats
  include 0, −0, 1, denormals, the largest and any bits) through `0x001bd1d0` and `atv_part::bounds`,
  bit for bit.
- **`the_kind_clear_is_the_originals`**: 10,000 random cases through vt `+0x39c` and `update::kind_clear`.
  The cases vary the state there or not, kind 3, 0–4 hung parts, the shown object there or not and in
  a space or not, the `+0x44` bit, the part category lists (0–6 long, drawn from the cleared bits and
  others), the bike's own and loaded category, and a renamed table entry. The object's switch-on and
  the spatial insert are hooked and logged on both sides. Tally: 3,179 kind 3, 6,821 cleared, 2,033
  inserted, 1,239 with a name added. Same calls in order, same words of the bike, the scratch area, the
  table and the loaded category. Pass.
- `ride_/flip_replays_without_the_frame_posts_tells`: pass. With no listener told, ride-2 is C15's on all 459
  rows and flip-1 on all 465 (wreck 125, put-back 193), the bike and its blocks the handing run's; the
  RAM parts only in the listeners' own words.
- Gate: `./kit check --all` in `./kit done`. That includes the whole `mxvatv_bike_ride` binary.
- **Second session.** The first session ended before its gate ran, and its work was saved on
  wip/F31b5l. This session picked it onto main after F31b5k (the net queue) and F31b5m (the other
  listeners) had landed. Both conflicts were additions next to each other, so both sides were kept. The
  every-port tests now assert F31b5k's no-NetQueue and this task's `three_served` together. Then the
  every-port replays were re-run on the merged tree: both pass, and the gate ran in `./kit done`.
- **Third session (2026-11-10).** The second session also ended before committing (wip/F31b5l, based
  on main after F31b5k and F31b5m). This session cherry-picked it onto main after F31b5n and F51. F51
  had added the same thing this task needed in `GatherInRam`: a query write-back without a nearby
  list, which keeps the nodes when the walk leaves its list open (`write_query`, with `left_open`).
  So this task's `write_world` was dropped and `fade_search_in_ram` now calls F51's `write_query`.
  The oracle.txt and test additions sat next to F31b5n's, so both sides were kept. The S294a audit fix
  was no longer needed (main had already changed that line). F31b5o and the F31b5 note were filed with
  `./kit plan add` / `plan note` instead of being edited into the plan by hand. The six tests were
  re-run on the merged tree, and all six pass. Each replay alone took 150–460 s, including the wait for a
  check slot. The two oracles passed in the first, combined run. The gate ran in `./kit done`.

## Not verified / not 1:1

- **The frame post's listeners are not ported.** The post is Rust's but its tells still run the
  original's code (HUD, race progress and the rest, ~1M instructions a frame on flip-1). They are shown
  not to touch the bike or its blocks on C15's two replays only: each tell is watched, and dropping
  them all keeps both replays C15's. They write their own words (presentation and race standing), so
  dropping them is not RAM alike. Filed as F31b5o.
- **The kind clear's ObjectOn and Insert are still interpreted** (the object's vt `+0x20` and the
  spatial insert). They are `update::Out`s now; serving them is F31b5o.
- **The category append path** (a cleared name missing from the table) is proved by the oracle only.
  State 4's table already holds all three names, so the replays never append.
- **The kind clear runs once per flip** on C15. The replays prove one path; the oracle covers the rest.
- **The fade search's moving pass and group pass** are proved only by the flip-1 replay, with no
  function oracle of `collide_world` against `0x001148e0`. On C15 the search never hits, so the
  hit write-back (`+0x60 = (0, −10000, 0, 1)`, budget 0) is unexercised. It is MX's ported code, which
  has its own oracles, but ATV's two differences are checked only where the replay reaches them.
- **The part box takes node 0 as the tree's root.** The original reads node 0. The test's
  `tree_in_ram` builds the tree from root 0, so a part whose root is not node 0 would not show here.
- **Listeners keyed by receive, not by object.** The eleven listeners are state 4's registration. A
  session with other listeners panics on the unknown receive rather than misbehaving silently.

## Next

F31b5o: port the frame post's listeners and the show's ObjectOn/Insert, or move the HUD and race
progress out of the pure frame.
