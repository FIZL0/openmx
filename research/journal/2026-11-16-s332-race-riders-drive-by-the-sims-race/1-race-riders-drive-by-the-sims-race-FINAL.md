# 1 — race riders drive by the sims race (FINAL)

Task S332. 2026-11-16.

## What I did

- `ai::Racing::of(laps, player, net)`: what a mode-2/4 brain drives by, built from the Sim's race laps.
  - race: kind (score manager `+0x60`), the race track's ahead reach (`+0x38`), the line's spot.
  - card: player's lap card flags, distances to the line, on-track word, step, kept spot.
  - the AI net, the race track's paths, the line gate's centre and normal, the race word (`+0x5c`).
- `ai::Pilot::think` takes the Sim's `race` and the AI `net` and always feeds `Around::racing`.
  So a Sim race rider never tells "mode two" / "mode four" missing any more.
  With no laps the drivers get no race and no card and answer false, as the original's do with a null race.
- The pilot's own card (`Around::card`) now reads its flag word and progress off the rider's lap card.
  Needed: the controller lets go of all four controls without the card's "taking part" bit (0x2), and
  the lane's card copy kept the pre-drop `COMPUTER_CARD` word for ever. In the original both are one card.
- The other riders' progress (`ai::Other::progress`) is their lap card's (`Sim::think`, tick.rs).
- `ai::Lane::net`: the race's AI path net, shared by every brain. `Sim::load_ai_net` fills it from the
  track's `<track>AI.tdf` (`ai::path::load_paths`, split out of `Net::load`: ground snap, prop raise,
  walk), with the search reaches set to `race::RACE_REACH` (125/75).
- Shell (`crates/openmx`): after the race's AI riders spawn, it reads `tracks\<stem>\<stem>AI.tdf` and
  loads the net. Prints `race: AI path net …, N paths`.
- Nothing taken from opentennis: it has no AI riders.

## What the original does

- The race driver (`./kit fn` on `ai_race_drive`, oracle.txt) reads the brain's race (`+0xd8`), card
  (`+0xd4`) and AI net (`+0xdc`). The race's track is at race `+0x68`, its line gate at track `+0x5c`.
- In sx01's live race image, the AI net's `+0x38/+0x3c` are 125/75, the same as the race track's
  (not the 200/200 the track constructor writes).
- The card's flag word at card `+8` is the one the controller tests (bit 0x2) and the drivers read.

## How it was proved

- `tests/ai/drive.rs` `the_joined_race_driver_rides_sx01s_race_image` (S314b's proof) now builds its
  `Racing` through `Racing::of`, over a `race::Laps` filled from `s282c-sx01-race.ee.bin` by the laps'
  own field offsets. Three brains × 201 drives match the original's `ai_race_drive` run whole in the
  interpreter (brain words, follower, net scratch, generator). Each drive also asserts that the race,
  card, line, race word and gate it feeds equal the ones read straight from the brain's pointers.
  A mutant (`track_reach` = behind + ahead) fails it.
- Same test: the image's AI net has search reaches 125/75 and pick 3, which `Sim::load_ai_net` sets.
- `ai::tests::a_race_pilot_drives_by_the_sims_race`: a mode-2 pilot's `think` tells no "mode two"
  missing. A mutant that feeds `racing: None` fails it.
- Live, headless shell on sx01 with 4 racers: the net loads (38 paths, as the image's net has).
  Before the card fix the AI riders crept 2 ft past the gate and stood. After it they ride the track,
  swap places, and start laps 2 and 3 within 12000 frames. Looked at by printed positions only.

## Not verified / not 1:1

- No capture compares a Sim race (riders on bikes, many ticks) against the original. The proof is the
  driver on the image's brains plus a live look. A race replay against C31's captures would close it (S337).
- The card's next gate is always `None`: `race::Laps` keeps a card's gate only as an index into a gate
  race's list it does not hold. Lap races (sx01) have none, so mode 2 is exact there; a gate race
  (mode 4) would drive as if it had no gate. Follow-up S336.
- Where the original writes the AI net's 125/75 reaches was not traced; the values are from the image.
- The AI net's flagged nodes are not raised onto props in the shell (no prop scene passed), as with the
  race track's laps (P313). sx01's files have no such nodes.
- `ai::Other` list order is player order, not the race's card list order (by place) the original walks.
  Unchanged by this task; only its progress is new.
- `Seat::race_phase`, `ai::Card::bike_word` and `Seat::live` still stay as the spawn left them.
- The pilot's card stays its spawn copy apart from flags and progress; the habits' racing bit
  (0x8_0000) still overlays the lap card's.
- The lane's net is not on the wire and not hashed (as the lane itself, S323).

## Next

- S336 (gate races' next gate), S337 (prove a Sim race against C31).
