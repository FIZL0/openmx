# 1 — rider statistics tracker (FINAL)

Task S319. 2026-11-14.

## What I did

- Found every store to the tracker's words by scanning boot.elf's disassembly for `0x12e8`..`0x1318`
  off the bike pointer. Five functions write them (below), plus the bike's constructor.
- Ported them as `race::Tracker` (`lap`, `place_changed`, `wrecked`, `controls`, `placed`) and
  `race::BikeCounts`; each `race::Entry` carries one (`stats`).
- `Laps::tick` tells the tracker each `LineNotice::Lap` (with the place the last close left) and each
  `PlaceLost` / `PlaceGained` from the close (with the place the ranking left).
- `Sim` calls `Laps::bike` right after each rider's frame (single-player `step` and `step_all`), with the
  ride's preload timer, the clutch lever the frame was ridden on, and the ride's wreck entries.
- The tracker is in the snapshot wire and the state hash.
- openmx `race_start::hand_over` fills `CardStats` from the tracker plus the card's `Jumps::best_length`
  (+0xf0) and `Jumps::best_air_time` (+0x100), and the human's end seat gets the real preloads/clutches.
- research/oracle.txt: `bike_lap_counted`, `bike_stats_controls(_end)`, `bike_grid_reset`,
  `bike_grid_place`, with the tracker's layout.

## What the original does

The tracker is inline in the motocross bike (type 2, `./kit fn 0x00279ed0` builds it) at bike+0x12c4.

- Lap counted — `./kit fn 0x0027ddd0`, the bike's card-owner slot +0xc: on a null card answers 0. A
  per-lap record list is appended only when +0x12e4 is set, which only the constructor writes (0). Then
  laps led += 1 when the card's place (+0x2c) is 1.
- Place change — `./kit fn 0x0027edb0` (card, other card, kind): crowd sounds first; kind 0 is a pass,
  and a time in the lead when the bike's *own* card (+0xf9c) is first; kind 1 is a time passed; any other
  kind counts nothing. Answers 1.
- Wreck entry — `./kit fn 0x0027b810`: after its callees, +0x14b4 = (reason != 0) and wrecks += 1 on
  every entry, reason 0 too. The port's `state::Ride::entries` counts the same entries.
- Preload and clutch — the player update (`./kit fn 0x0027c890`, pc 0x0027cc2c..0x0027ccdc): a latch
  that is clear and a value that is not `<= 0` (c.ole.s, so NaN counts) adds one and sets the latch; a set
  latch and a value `<= 0` clears it. Preload reads the preload timer (+0x4b0), clutch the lever (+0x644,
  not the rate-limited +0x65c).
- Grid reset — `./kit fn 0x0027f9e0`, the slot the race grid calls to place a bike: unless the event's
  activity type is 5 (freestyle) it zeroes the counts, latches, air clock, top speed and odometer, and
  sets +0x1308 to -1.0.
- The card's +0xf0 / +0x100 are written only by score_card_jumps (`./kit fn 0x00267d90`), ported by S302.

## How it was proved

Function oracle, crates/game-sim/tests/race.rs:

- 45: the lap listener on 20,000 trackers, places and null cards.
- 46: the place listener on 20,000 trackers, kinds (0, 1, 2, any word), own and told cards, crowd hooked.
- 47: the wreck entry on 10,000 reasons, callees hooked, counts and the shown flag; the ride's entries
  counted on through `Tracker::bike`.
- 48: the update's stretch on 40,000 latches (0, 1, any nonzero word), timers and levers including
  signed zeros, denormals and NaNs.
- 49: the grid reset on 10,000 trackers and event kinds.
- 50: two riders walked round sx01 (the faster one leaving late): the trackers against the lap and place
  notices each tick told — 3 laps led, a pass, a time in the lead, a time passed.
- openmx race_start test: the hand-over's 13 words carry the tracker and jump words.

## Not verified / not 1:1

- Feed timing: the original counts preloads and clutches inside each bike's update and the lap/place
  counts inside the score manager's tick. The port counts the controls right after each rider's frame and
  the notices in `race_frame`, which runs before the riders step. Same counts, but no capture compared
  them tick by tick against a live race.
- Wreck count: the port counts the ride's wreck entries since its last look rather than in the wreck
  entry itself. The first look only takes the baseline (`entries_seen`), so a wreck in the race's very
  first frame after the laps were made is not counted. This baseline is the port's own.
- The grid reset is ported as `Tracker::placed` and oracled, but nothing calls it: a fresh tracker is made
  with each race's `Laps::new`, which equals the reset (non-freestyle). A freestyle event keeping its
  counts across grid placements is not modelled.
- The per-lap record list (+0x12d0..+0x12e4) is not ported: its gate is only ever written 0 by the
  constructor in the scan, but a store through another pointer to the tracker can't be ruled out by a
  scan for the bike offsets.
- The tracker's other words — air clock (+0x130c), top speed (+0x1310), the -1.0 at +0x1308, wheelie
  clocks and the odometer (+0x133c) — are not ported; the results menu does not read them.
- AI bikes are assumed to run the same player update (same class, same vtable); not traced per AI kind.
- Callers of the grid slot other than the race grid were not traced.
- No live capture of a race's results screen was compared.

## Next

Nothing waiting.
