# 2 — s13 laps (FINAL)

Task S13c. 2026-11-09.

## What I did

- **Held the race handler against C31's live sx01 laps.** The per-function pieces had all landed: the
  lap line (part 1), the other crossing tests (S292), the handler per card with its path queries
  (S293), the line lists (S294a), and the finish and ranking (S294). C31 captured two sx01 single races.
  What was left was the hold against a live lap. It is proof 26 in `crates/game-sim/tests/race.rs`,
  `the_race_handler_holds_c31s_live_sx01_laps`.
  - **sx01's track** comes out of C31's own RAM image (`context/states/c31-sx01-ai4.ee.bin`, the same
    session as both captures):
    - the paths through `track_at` + `TrackPaths::new`;
    - the search reach (125 / 75) and lap length (4153.4) from the track's `+0x38/+0x3c/+0x54`;
    - the line from `+0x5c`, with its path spot at the line's `+0x94`.
    - A captured spot's words are RAM addresses. They are turned into path and node numbers by walking
      the image's path slots and node lists.
  - **Every tick, every rider's card:**
    - seeded from the previous row's raw 0x1d0 bytes (`c31_card`), its `now` set to this row's;
    - the manager's words from the previous row's raw bytes (`c31_rules`), with `look_for_waited_for`;
    - stepped in the manager's list order (`+0x34`, next `+0x1c8`, ranked by address) through
      `RaceRules::card` on one `TrackPaths` carried through the run;
    - compared with this row on 23 words, bit for bit (`c31_words`): flags, both line distances, the
      search distance, progress, odometer, the lap lengths, laps, on-track, lap time and start, best lap
      and its number, last, both spots, race clock, lap clock, off-track clock, time left, the path's
      way ahead, and the way back to the track.
  - **Both captures** go through it: `sx01-gas.csv`, by rider id, and `sx01-ai4-by-rider.csv`.
- **No port change was needed.** Every word matched except the two cases below, which have causes
  outside the ported code.
- `race::race_kind` already has C31's corrected meaning (+0x60 = 1 for a race that ends at its laps,
  2 for practice). S294 fixed it, so part 1's "a single race never finishes at the line" no longer
  stands: the hold sees the line finish each AI card at its third lap.
- Filed **P328**: the HUD's white and checkered flags. S294 found that no code in the handler or the
  lap line draws a flag. They come from the presentation listeners.
- Noted on **S302**: the flag bits the unported helpers change, and the route-scratch effect below.
- Nothing taken from opentennis. It has no laps.

## What the original does

Read off the captures by holding the port against them, not off new decompilation:

- **The handler runs once a tick on each card, after the rider has moved.** It reads the card's `now`
  (already this tick's place) and copies it into `last` at the end. A row read after the handler has
  `now == last` for every racing card.
- **At a counted lap:**
  - lap time = the race clock as it stood before this tick's `+= dt`, minus the old lap start;
  - lap start = that clock;
  - the card is put on the line (its spot = the line's spot with along + 1e-5);
  - both distances to the line are the lap length;
  - the clocks then advance (race clock = lap start + dt, lap clock = dt).
  - At the last lap of a race kind 1, the card goes `OUT`, the manager gets `FINISHED` (0x10), and the
    clocks stop.
- **Off the track:**
  - The off-track clock runs while the spot search finds no spot.
  - With card flag 0x100_0000 and manager `+0xd4` = 5 s, the handler zeroes it at 5 s and tells
    `OffTooLong`. 19 times in the gas run.
  - A listener then puts the rider back on the track: `now` jumps on the next tick, which the hold
    sees as the next step.
- **The spot search's distance for a rider just behind its kept spot** is the route forward from the
  kept spot, the long way round the lap. Its answer depends on the scratch (`dist`/`best`) that earlier
  route searches leave on the paths.
  - The idle rider at the gate in sx01-ai4 gets 9000.71, 9001.33, … 8390.3, 8997.8 from one tick to
    the next while standing still.
  - The port reproduces the arithmetic: the same two queries on the port's paths give the original's
    8202.003 at frame 293 once the scratch is as a later search leaves it.
  - It cannot reproduce the scratch, because the unported computer aim (all four cards carry
    0x800000) searches the same paths first.

## How it was proved

`./kit check -- -p game-sim --test race -- c31s_live_sx01_laps`. Every rider, every tick of both
captures, against `RaceRules::card`, bit for bit on the 23 words.

| capture | steps | torn pairs | laps held | best laps | finishes | off-track ticks | off too long | back on |
|---|---|---|---|---|---|---|---|---|
| sx01-gas.csv | 25,058 | 2,337 | 9 of 10 | 5 | 3 of 3 | 4,192 | 19 | 28 of 37 |
| sx01-ai4-by-rider.csv | 24,893 | 1,835 | 7 of 9 | 4 | 3 of 3 | 0 | 0 | 0 |

- **Laps held** (frame, rider, lap):
  - gas: (2292,1,1) (2312,2,1) (2379,3,1) (4302,1,2) (4575,3,2) (6080,0,1, the human's) (6378,1,3)
    (6395,2,3) (6624,3,3). Missing: rider 2's second lap.
  - ai4: (2301,3,1) (2331,2,1) (4337,3,2) (4395,2,2) (6346,3,3) (6418,2,3) (6515,1,3). Missing: rider
    1's first two.
- **Torn pairs are skipped.** About one row in five holds the previous tick again, and the row after
  it is two ticks on, so that step cannot be rebuilt. A pair is used only if:
  - every card racing on both sides advanced its race clock by exactly this row's dt, and
  - every racing card has `now == last` on both sides.

  The missing laps fall on such pairs.
- **Asserted**, so a regression cannot pass quietly:
  - no word differs;
  - the flag differences stay inside `C31_UNPORTED_FLAGS`;
  - per capture: > 20,000 steps, < 3,000 torn, ≥ 7 laps, ≥ 4 bests, exactly 3 finishes;
  - the gas run: > 4,000 off-track ticks, ≥ 10 off-too-long, ≥ 25 back-on.
- **First runs**, before the tear and phase filters and the list order: the differences were all torn
  rows (a card two ticks ahead) and mid-tick rows (`now != last`). The flag differences were all in
  0x18_3ce0. Once those were filtered, the only `on_path` difference left was the long-way case.
- Not mutation-tested this session. The words compared are the ones the per-function oracles
  (part 1, S293, S294, S294a) already mutation-tested.

## Not verified / not 1:1

- **Card flag bits 0x18_3ce0 are not compared.** They change in both captures, set and cleared by work
  outside the ported handler: the card-0x80000 helper, the manager-0x800/0x1000 helpers and the computer
  aim (S302), or the listeners. None is a bit the lap line or the ported handler writes. Which of them
  sets which bit was not traced. Noted on S302.
- **`on_path` (card `+0x18`) is let differ when the rider stands just behind its kept spot** on the same
  stretch, and both values are past the search reach.
  - Gas: once (frame 293, the human rolling back at the start). ai4: 3,036 ticks (the idle human at
    the gate).
  - The original's value hangs on route scratch that the unported computer aim also writes.
  - Both values are over 125, so the progress test, the only reader of the word, reads them alike.
  - Not 1:1 until S302 ports the aim and the hold runs it in order.
- **Two laps in ai4 and one in gas are not held**, along with 9 of the gas run's 37 off-track
  episodes. They fall on torn row pairs, which the recorder lost under load. Holding them needs a
  cleaner capture (lower the copy's NominalScalar; see the kit note on rows missed under load).
- **No human lap ridden on the track.** The human's one lap (frame 6080) comes from riding off the
  track and being put back. The on-track laps are the AI riders'. They go through the same handler,
  but the human's card flags differ (0x4 vs 0x8).
- **The lap records (card `+0x1bc`, manager flag 0x400 set on sx01) are not compared.** The capture
  holds only the list pointer, not the records, and the free list's length was not captured, so the
  hold gives the rules a free list of 64. The records are proved by part 1's oracle only.
- **The per-line words (`+0xe4`) and the line lists:** sx01's manager lists are empty (`+0x70` and
  `+0x78` are 0, even with manager flag 0x200 set) and the cards' words pointer is 0. Checkpoint order
  is proved by S294a's oracle only; no captured track exercises it.
- **The ranking, the finish countdown and the race-over draw are not in this hold.** The ranking and
  gaps are held against sx01-ai4 by S294's own test. The countdown and the draw need a race the human
  finishes (S13d's capture).
- **The white and checkered flags are not ported.** They are presentation, drawn by listeners the port
  does not have; filed as P328.
- **Listener notices are not compared.** The capture cannot see them. The test only counts
  `OffTrack` and `OffTooLong`.
- **The rider being put back on the track** (the `now` jump after `OffTooLong`) is a listener's or the
  bike's work, not ported here. The hold feeds the captured `now`.
- **The track's search pools and route scratch are not seeded from the original.** They start as
  `TrackPaths::new` leaves them, and only the port's own searches move them.
- **The race's other kinds** (practice, ghost, fast-lap, the gate races) have no live capture. They
  rest on S292/S294's oracles.

## Next

Nothing for S13c once this lands. Follow-ups: S302 (the helpers, which would shrink the flag mask and
the long-way exception), S301 (wire `RaceRules::card` into the running race), P328 (the HUD flags),
S13d (the results, now unblocked).
