# 1 — fade search oracle (FINAL)

Task F49. 2026-11-16.

## What I did

- **Found the port already there.** The brief was filed from F43b's journal (2026-11-05). Since then:
  - F31b5l (merged 2026-11-10) serves the fade's search in Rust in the update rig. That is
    `fade_search_in_ram` over `atv_search::collide_world`, with the part's narrow phase (`atv_part`) and
    the structures' begin/next. Group parts and listeners go out as calls.
  - F51 ported a sibling, the sweep's global search over the same function.
  - F31b5l's journal left one gap: no function oracle of the fade search against the original. On C15 it
    never pairs and never hits. So this task is that oracle, plus the harness fixes the oracle turned up.
- `crates/game-sim/tests/mxvatv_bike_ride.rs`:
  - New test `the_fade_search_is_the_originals` (described under "How it was proved").
  - `fade_search_in_ram` now:
    - returns false, and writes nothing back, when a listener searched the database inside the walk
      (F60, as F51 does);
    - writes the mesh descent's descriptor and flag when a mesh was paired.
  - The update rig asserts the true answer, so a replay that ever reaches a re-entering listener panics
    instead of going wrong silently.
  - `PartWorldInRam` answers a box (kind 6) through F53's `f53_box_in_ram` (`oriented_box`). Before,
    the part search panicked on one; the fade search meets two on nat05.
  - `stale_stack_copies`: reads which bytes the original's message post copied from frame slots it
    never sets (see below), so the test can mask them.
- `research/oracle.txt # F49`: the addresses.
- Nothing from `../opentennis` applies (this is ATV collision code).

## What the original does

- **Placing the part.** `./kit fn 0x001bd180 -o atv`: the part's vt `+0xc8` re-stands it at its node,
  so the usual place call (`0x00114360`) cannot move it somewhere else. Instead the test:
  - stands it at a frame of its own with vt `+0xcc` (`./kit dis 0x001bcfe0 -p atv`): 16 words go to
    `+0x130`, and the centre and reach to `+0x50`/`+0x70`;
  - then calls the box move (`0x00114480`).
- **Mesh pair globals.** The descent writes `0x003dce38` = the mesh's address + `0x230`. That is an
  address, not the word stored there; my first try wrote the word and failed. It also writes
  `0x003dce30` = 1. The two stack pointers and the leaf scratch are the interpreter's frame (F53's
  handling).
- **A box's on_struck** (`./kit fn 0x001a8000 -o atv` → `0x001a9630` → `0x001a9730`) can post a message
  through a listener's vt `+0x14`. The post (`./kit dis 0x00187850 --len 0x4d0 -p atv`) copies a 52-byte
  record from its frame (`sp+176..sp+216`). It never sets `sp+208` or `sp+212`, so the record's
  `+0x2c`/`+0x30` get stale stack words. Seen: the part's address, and two floats left by deeper calls.

## How it was proved

- **`the_fade_search_is_the_originals`**, 1,500 searches on the state-4 RAM image.
  - Setup:
    - Two RAMs: one runs SLUS-21104's `0x001148e0(part, 0)` and one runs `fade_search_in_ram`.
    - The riding part (bike `+0xcc`) is moved by its frame. Two in three spots are by a nat05 mesh
      triangle (F51's segment generator, ±1 ft); the rest are anywhere out to 400 ft from the bike.
    - It is turned about y (sometimes tipped ±0.5 rad), or keeps the image's rows.
    - It is stood and its box moved by the original's calls on both RAMs. The RAMs are compared after
      that too.
    - Drawn per search: last centre (the image's, or within 8 ft), budget (0, or −5..50), inactive 1 in
      15, `NO_GROUPS` 1 in 4, the hit bit before.
  - Checks:
    - The answer matches the budget written back, bit for bit (`f32::MAX` cases are covered by the RAM
      check).
    - Every RAM byte outside the stack matches after each search.
  - Tally:
    - 671 hits, 717 clear, 110 inactive, 1,122 with plants searched.
    - 2,155 mesh pairs, 234 plant-sphere pairs, 2 box pairs, 228 plant part stands.
    - 911 hits heard by the bike's and the pair log's listeners, 57 pair-log growths, 14 heard_struck
      and 5 on_struck hand-ons.
- **Mutants:**
  - descriptor `+0x234` instead of `+0x230`: fails;
  - dropping the group point distance's `max(0)` in `broad_phase::point_distance`: fails.
- The C15 replays (ride-2 and flip-1 with every port on, "fade search" Rust's) run in the gate. The
  changes cannot alter them: on C15 the search never pairs, reaches no box and has no re-entering
  listener.
- Gate: `./kit check --all` in `./kit done`.

## Not verified / not 1:1

- **The message post's stale stack words are masked, not matched.** In 2 of 1,500 searches a box's
  on_struck posts a message. That record's `+0x2c`/`+0x30` are copied from frame slots the post never
  sets. Their value depends on what earlier calls left at that stack depth, and the harness's hand-on
  runs 0x140 bytes shallower than the original's walk. The test copies the original's bytes there (by
  the store log's pcs) before it compares. Nothing in Rust could reproduce them short of matching the
  interpreter's stack. What reads those words is not known.
- **A listener that searches the database inside the walk is still not followed** (F60). That was 2 of
  1,500 cases here; they are skipped and the original's RAM copied over. The rig now panics if a replay
  ever reaches one.
- **The part stand, the callbacks and the listeners other than the bike's and the pair log's** still run
  the original's code, handed on (as in F31b5l and F51).
- **No hierarchy (kind 1) or finite plane (kind 8)** exists on nat05. The part search still panics on a
  hierarchy (its parts are not read here). F51 and F53 prove those pairs on made-up objects.
- **Only the state-4 part's own ray set and tree** are searched. Its rays and tree are not varied; F53's
  oracle varies them for the pair.
- **Stood parts' group words (`+0x88`) are written back after the walk.** The original writes each
  one as the part stands. A hand-on that read a part's group mid-walk would see the old one. Writing
  them before each hand-on changed nothing on these 1,500 searches, so I left it out.
- **`lazy` is always 0**, the fade's own argument. F51's oracle covers lazy searches of the shared code.
- **No recorded ride** reaches a pair, hit or box in the fade search. The proof here is the generated
  oracle.

## Next

Nothing for F49. F60 (the searching listener) is already filed.
