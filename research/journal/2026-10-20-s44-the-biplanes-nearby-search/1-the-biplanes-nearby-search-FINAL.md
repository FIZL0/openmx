# 1 — the biplane's nearby search (FINAL)

Task S44. 2026-10-20.

## What I did

- **Split the task first.** Reading the plane's collision object in the save state showed the brief
  covered five pieces of work, each the size of a task. Two of them are narrow phases that were never
  ported (a tree-against-tree ray test, and a sphere-bundle sweep that S4's mesh sweep only half covers).
  So, per AGENT.md, S44 is now the search loop and the pair filter, and the rest is filed:
  - **S45**: the broad phase (the gather and the world's collidable tree), plus the whole-world search
    `probe_collide_world`;
  - **S46**: the ray probe's narrow phase;
  - **S47**: the sweep probe's narrow phase;
  - **S48**: implementing `contact_search::Search` on S44–S47, the plane's listeners, and the call to
    `collide` before `resolve` in the ride. This waits on S41 too: `Flight::frame` does not run the
    substeps yet, so nothing in the ride calls `contact::resolve` either.

  I rewrote S44's own line to say what it now covers, and named the split's addresses in
  `research/oracle.txt` (S44 block) so the plan lines carry names, not hex.
- Ported `collide_against_search` and `collision_pair_filter` as `crates/game-sim/src/world_search.rs`,
  a new module:
  - `search_nearby`: the search;
  - `accepts`: the filter;
  - `Collidable`, `Nearby` / `Candidate`: what they read;
  - `Narrow`: the trait standing in for the probe's narrow phase (clear, prepare, pair), a placed
    group's parts, and the listeners and callbacks.

  Nothing else changed apart from `lib.rs`'s module line.

## What the original does

**The plane uses the nearby search.** Its record's `+0x10c` is set in the save state, so the contact
search always calls `collide_against_search` (`./kit fn 11a9d0`) and never `probe_collide_world`. That
search is the probe against the list its broad phase gathered: 34 candidates in the save state,
26 meshes (shape kind 0) and 8 hierarchies (kind 1), all of class 2. None of them is within either
probe's reach in that frame (distances 353–2185; the ray probe reaches 14.5, the sweep probe 0.02).

Step by step:

1. Set the probe's `+0x116` bit `0x10`, then call its class vtable (`+0x38`) slot `+0x90`, the clear.
   The ray probe's clear (`./kit dis 1eb280`) lowers bit `0x20` and empties the result.
2. If the search's `+0x38` is set, call slot `+0xb0`, the prepare.
3. For each candidate (8 bytes: object, distance), go on only if `distance <= probe+0x60`
   (`c.ole.s`, so a candidate exactly on the reach counts). The array and the count are re-read on
   every pass.
   - **Class 2** (`+2`): set its bit `0x10`. Then, if `collision_pair_filter(probe, it, 1)` passes and
     slot `+0x94` (the narrow phase) returns non-zero, it is a hit.
   - **Class 3**, a placed group, but only if the probe's bit `0x01` is clear: for each part `i`,
     with the count re-read after every part, ask `./kit fn 129490` for it. A null part is skipped.
     Otherwise set its `+0x78` to the group and its bit `0x10`, and call slot `+0x94` on it. **No
     filter for parts.**
   - Any other class is skipped.
4. On a hit:
   - set the probe's bit `0x20`;
   - call each of the probe's listeners' vt `+8` (listener, probe, struck);
   - call each of the struck one's listeners' vt `+0xc` (listener, struck, probe);
   - call the probe's `+0x10c` (probe, struck), then the struck one's `+0x110` (struck, probe).

   A listener list is the count at `+0xf4` and the array at `+0xfc`, and both are re-read on every
   step.
5. Return bit `0x20`. Only the clear ever lowers it.

**The pair filter** (`./kit fn 119d70`, `a` asking about `b`) refuses, in order:

- `a == b`;
- `b`'s bit `0x04` clear;
- when asked as "nearby", `b+0x31` bit 0 clear;
- `a+0xcc & b+0x84 == 0` (this way round only).

Then the object lists. Each is a mode, an array and a count:

- `+0xd0`/`+0xd4`/`+0xd8` and `+0xdc`/`+0xe0`/`+0xe4`;
- if the list is not empty: mode 1 refuses a listed object, mode 0 refuses an unlisted one, any other
  mode refuses neither;
- they are checked in the order `a`'s first about `b`, `b`'s first about `a`, `a`'s second, `b`'s
  second.

Then the category lists (`+0xe8` mode, `+0xec`/`+0xf0` masks):

- mode 1 refuses if any mask meets the other's category;
- any other mode refuses unless one does, so **an empty list in mode 0 refuses everything**;
- `a`'s list is checked about `b+0x84`, then `b`'s about `a+0x84`.

**The listeners, in the save state:**

- **The ray probe has two.**
  - The plane's own: `plane_probe_hit_listener`, at plane+0x98, through the thunk `./kit dis 15c290`.
    If its owner's `+0x58` is set and the struck `+0x84` meets the global mask at `0x00468f68`, it
    calls the owner's vt `+0x124`.
  - A global sound object: `plane_probe_sound_listener`, `./kit fn 3ea8a0`. It makes two sound calls
    and nothing else.
- **The sweep probe has none.**
- **Five candidates have one each.** Three are other riders' bikes (`bike_struck_listener`) and two
  are track objects.
- No callbacks (`+0x10c` / `+0x110`) are set on any of them.

These are behaviour of the plane and the bikes, filed under S48.

**Water.** Nothing in the search or the filter reads a surface or a depth. If water reaches the plane,
it does so as a collidable the narrow phases strike (S46/S47) or as a struck object's listener (S48).
The save state's 34 candidates are all meshes or hierarchies; whether any of their triangles is
water is for S46/S47 to see.

## How it was proved

Function oracle in `crates/game-sim/tests/world_search.rs`, on C7's biplane save state, using the
plane's real collidables: its two probes and the first ten candidates of its own search. Each case
rewrites everything the search and the filter read on all twelve collidables:

- class (2, 3, others), flags, the nearby byte, reach, group, category, mask;
- both object lists (with foreign pointers);
- the category list in all modes;
- 0–3 listeners and the two callbacks.

There are two tests:

- **`the_pair_filter_matches_the_original`**: 40 000 pairs over 10 000 cases, both ways of asking,
  the original's filter against `accepts`.
  - Accepted 6 606 / 5 461, refused 13 471 / 14 462 (without / with "nearby").
  - Two in three collidables are generated permissive so that both answers are common.
- **`the_nearby_search_matches_the_original`**: 10 000 cases of the real search with either probe.
  - **Hooked:** the probe's class vtable is swapped for one whose clear, prepare and narrow phase are
    hooks. The part count and the part, the listeners' two slots and four callbacks are hooks too.
    Each hook logs; the narrow phase, the count and the part answer off a generated script shared
    with the port; the clear lowers bit `0x20` as both real ones do.
  - **Run as the original:** the filter, the listener-list accessor and the search itself.
  - **The gathered list:** 0–8 candidates, the probe itself now and then, repeats allowed, distances
    inside, outside, at 0 and exactly on the reach.
  - **Compared:** the return, the whole log of calls out of the search with their arguments in order,
    and every collidable's flags and group afterwards.
  - **Reached, all asserted > 300:**

    | what | count |
    |---|---|
    | hits | 3 566 |
    | narrow-phase calls | 10 025 |
    | parts | 4 704 |
    | probe-listener calls | 5 770 |
    | struck-listener calls | 5 883 |
    | callbacks | 6 670 |
    | prepares | 3 321 |
    | candidates in reach the filter refused | 15 172 |
    | candidates exactly on the reach | 8 578 |

**Mutations**, each failing and then restored:

| mutation | caught at |
|---|---|
| reach `<=` → `<` | case 0 |
| group bit ignored | case 3 |
| nearby flag ignored | case 0 |
| category mode inverted | case 1 |
| object-list mode 0 ignored | case 15 |
| the 0x10 bit not set | case 0 (flags) |
| the other side's category list dropped | case 1 |
| a part's group not set | case 0 |
| the two callbacks swapped | case 2 |

`./kit check`: 643 tests pass.

## Not verified / not 1:1

- **The narrow phases are not ported.** `Narrow::pair`, `clear` and `prepare` stand in for the ray
  probe's tree-against-tree test and the sweep probe's sphere sweep. The proof hooks them, so what
  they strike, and the result record they fill, are S46 and S47.
- **The broad phase is not ported.** `Nearby` is taken as given: which candidates the gather keeps,
  their distances, their order and the `+0x38` prepare flag are S45. So is `probe_collide_world`,
  which the plane never reaches with `+0x10c` set.
- **A placed group's parts are hooked.**
  - The count is a read through the world manager's model table, not done in the port.
  - The part comes from `./kit fn 129490` (540 bytes, which sets the part up as a collidable), not
    done in the port.

  The search's side (re-reading the count, skipping null parts, the group link, no filter) is proved.
- **No real listener or callback ran.** The proof hooks all of them. The plane's own listener, the
  bikes' and the track objects' are behaviour, filed under S48. The sound listener is presentation.
- **A listener that changes the lists mid-search is not exercised.** The original re-reads the
  gathered list, its count and the listener counts on every step. The port re-reads the listener lists
  but takes `Nearby` by shared reference, so a `Narrow` cannot change the gathered list during a
  search. No hook in the proof changes any list.
- **The flag bits are named from use, not from a writer.** `0x01` "never searches groups", `0x04`
  "can be struck", `0x08` "gather only within its bound" and `0x10` "searched" are read off the
  search, the filter and the gather. Nothing else that sets or reads them was looked at.
- **Generated values are bounded**: categories from 4 bits, reaches 0–20, no NaN distances, and
  negative part counts only −1.
- **Not wired into anything.** No caller in the port yet. That is S48, after S41.

## Next

S45, the broad phase; it can run in parallel with S46 and S47 (all three are after S44 only).
