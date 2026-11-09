# 1 — other listeners dropped (FINAL)

Task F31b5m. 2026-11-09.

## What I did

- **The sim lists' accessors in Rust**: `tail::list_get` and `tail::list_remove`
  (`crates/game-sim/src/handling/tail.rs`), on `node::Ram`. They are the getters of the sim's `+0x24`
  and `+0x3c` lists and the `+0x3c` list's remove. The pure run of `posts_frame`
  (`crates/game-sim/tests/mxvatv_bike_ride.rs`, the second run of `replay_pure`) now uses them in
  place of the interpreter. So every test built on `replay_pure` (nothing handed on, every port at
  once, the part leaves) runs them, and those tests compare the whole RAM against the handing run's.
- **`"other listeners dropped"`**, a new `IN_RUST` mode (`dropped_others`). In the pure run it leaves out:
  - the sim's records other than the bike's (their clear `+0x7c` and pass `+0x84`; state 4 has none);
  - the world's PreProcesses other than the bike's (the five other vehicles on the preforce list);
  - the tail's first other leaf;
  - the manager's own work. Its closing walk still runs, through `tail::walk`, and still steps the
    rider. The rider is out of scope here: F31b5i.

  Each dropped record or PreProcess is counted in `answered` as `dropped …`.
- **The read check.** In that mode, `replay_pure` diffs the two RAMs (and scratchpads) before each frame
  (`differing_bytes`). It turns on `Ee::reads` for the pure run's frame, and asserts that the frame read
  no byte in a differing range (`read_where_parted`). `Ee::reads` logs host reads through `u32_at`
  too, so the reads of the Rust ports count, not just the interpreter's.
- **Tests:**
  - `ride_replays_with_the_other_listeners_dropped` and `flip_replays_with_the_other_listeners_dropped`
    (on `replay_without_others` and `others_dropped`): every port on, plus the drop.
  - `the_sim_lists_accessors_are_the_originals`: the accessor oracle.
- `research/oracle.txt` `# F31b5m`: the accessors' addresses and what is left out.
- Nothing taken from opentennis: it has no interpreter-backed sim frame.

## What the original does

- Both getters (`./kit fn 0x00208390 -o atv`, `./kit fn 0x002080f0 -o atv`) are the same five
  instructions: `array = list+8`, answer `array[i]`. There are no checks.
- The remove (`./kit dis 0x00208040 -p atv`):
  - it answers 0 when the count is `<= 0`, the array is 0, or `i >= count`. The compare is signed
    (`slt`), so a negative `i` passes and writes before the array;
  - otherwise it sets `array[i] = array[count-1]`, then reads the count *again* (the store may have
    hit it), decrements it, and answers 1.
- What is dropped and where it lives is F31b5d's list: the five preforce elements on the one thunk
  (`./kit fn 0x004de0c0 -o atv`), the leaf `0x00eb6420` (step `0x001c9540`) and the manager
  `0x01ed82d0` (step `0x002cf9f0`).

## How it was proved

- **The accessor oracle**: 10 000 generated lists. The count runs from -3 to 40. Some lists have no
  array. In one case in eight (n > 2) the list's own count and array words sit inside its array, so the
  remove's move writes the count (568 cases). The index runs from 4 before the array to 3 past the
  count. Both of SLUS-21104's getters and its remove ran against the port. The answers and the 0x200
  bytes around the list are equal, and the original writes nothing else. 7285 removes, 2715 refusals.
- **Replays**: C15 ride-2 (459 rows) and flip-1 (465 rows), off `context/mxvatv-ps2/state04.ee.bin`,
  every port on, the other listeners dropped:
  - every row word for word C15's;
  - the bike object and its six physics blocks equal to the handing run's after every frame;
  - flip-1 wrecks on row 125 and is put back on row 193;
  - the frame reads no byte of RAM or scratchpad where the two runs differ going into it.
- **Where the RAMs part**:
  - ride-2: 6 word ranges, all from row 0: `0x10af6f0`, `0x110df70`, `0x11139d0`, `0x1163500`,
    `0x11722e0`, `0x1181f30`;
  - flip-1: the same 6, plus the dropped cue's word (row 72, as before) and `0xeb6454` (the first leaf's
    own word) from row 126.
- **What the dropped listeners did**: 5 PreProcesses a frame, 2295 on ride-2 and 2325 on flip-1. No
  record was dropped: the list has only the bike's. The census lost "the tail: the first other leaf" and
  "the tail: the manager". "The bike's world step" fell from 2.5M to 1.54M instructions on ride-2 (2.0M
  to 1.0M on flip-1).
- `./kit check -- -p game-sim --test mxvatv_bike_ride -- the_sim_lists_accessors other_listeners_dropped every_port_at_once`:
  5 passed. Numbers from the built binary with `--nocapture` (146 s for the two replays).
- Gate: `./kit check --all` in `./kit done`.

## Not verified / not 1:1

- **"Every word the bike's frame reads" is shown as two halves.**
  - *Shown here:* the frame reads no byte where the RAMs already differ when the frame starts.
  - *Not shown here:* within one frame, the handing run's dropped listeners write and then something
    later in that same frame reads it. That half rests on F31b5d's watched run, which ran the full
    original frame, not this mix of ports. F31b5d found nothing but the rider's writes read, and the
    rider is kept here.
  - The sets are per frame and unordered. A byte the pure frame writes first and then reads counts as
    read. That errs towards failing, and the check passed.
- **Only C15's two rides on state 4.** As in F31b5d: another state could have other records on the
  `+0x24` list or entries on the `+0x3c` list. Records other than the bike's would be dropped silently
  (counted in `answered`; no test asserts zero beyond these rides). The `+0x3c` list's elements still run
  in the interpreter (vt `+8`); the list stays empty on both rides.
- **The remove is never reached in the replays.** The `+0x3c` list is empty, so `list_remove` is proved
  by the oracle alone.
- **The census's "records' clear and pass" bucket still holds 186k (ride-2) / 78k (flip-1)
  instructions.** It is not the other records (none) or the getters (about 4.6k a ride). It is the bike's
  own record's handed-on calls, F54's part stand and hit listeners. The F31b5 census put it under
  F31b5m; it is F54's.
- **The other vehicles' PreProcess is dropped, not ported.** That is right for a solo ride on state 4,
  where they are parked riders. A race with AI riders would need them.
- **The every-port tests (`*_with_every_port_at_once`) do not drop the others.** I kept them as the
  whole-RAM proof. The drop has its own two tests. F31b5's pure frame should start from
  `replay_without_others`' names.

## Next

Nothing for F31b5m. F31b5 still waits on F31b5i, k, l, n and F54.
