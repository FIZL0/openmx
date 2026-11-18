# 1 — gate race card gate (FINAL)

Task S376. 2026-11-17.

## What I did

- `ai::Racing::of` and `ai::racing_of` now give the card's next gate. They share a new helper,
  `race_card` (ai.rs). It looks up the lap card's `gate` index in `Laps::lists.checkpoints`. The
  crossing test gets that gate, and the drivers' `Card::gate` gets its flags, centre, normal, side
  facing, speed and spot.
- `race::ListGate` gained `side` (`+0xb0`) and `speed` (`+0xa4`), the two gate words the gate driver reads
  beyond the crossing test's. The snapshot wire and the two test literals in tests/race.rs were updated.
- No new list was needed. The gate race's chain of gates sits at the score manager's `+0x70`
  (`./kit fn 0x002611c0`: the lap wrap stores `mgr+0x70` back into card `+0x80`). That is the same word
  `LineLists::checkpoints` already ports for the lap race. So a card's gate index is an index into the
  checkpoint list.
- In a lap race (sx01) the cards have no gate, so nothing changes there. Mode 2 stays exact.
- Nothing from ../opentennis: it has no AI race drivers.

## What the original does

- `./kit fn 0x00285090` (the gate driver, vt+0x9c) reads card `+0x80`, a pointer to the next gate.
  Under the watch flag it uses `+0x30` (flags), `+0x40`/`+0x50`/`+0x60`/`+0x70`/`+0x74`/`+0x7c` (the
  crossing test's) and `+0x94` (spot). Without a net it also uses `+0xb0` (side facing, on flag 0x20)
  and `+0xa4` (side speed).
- `./kit fn 0x002611c0` (the gate race's line) walks the same objects as a chain: next at `+0x90`, head
  at `mgr+0x70`.

## How it was proved

- A save-state image of the original, driven against the port: tests/ai/drive.rs,
  `the_joined_gate_driver_rides_sx01s_race_image` and `..._aims_at_the_gate_...`. They cover 3 brains ×
  201 ticks, with and without the AI net.
- The gate-driver test now runs through laps, the same way the race-driver test (S332) already did. The
  stand-in gate becomes the race's whole chain (`mgr+0x70`, its `+0x90` cleared). The laps' checkpoint
  list and the card's index are read by walking that chain, and the port drives by `Racing::of`. The test
  asserts that the race, card, gate, line and race word it feeds equal what the image's pointers hold.
  It also checks every written brain word against the original, each tick.
- For the third brain, the stand-in is a side gate (flag 0x20, facing = the line's across, speed 37.5).
  That way the side and speed words carry real values.
- Mutants (each reverted): indexing `i + 1`, speed fed as 0, and side fed as the normal. Each one fails
  both tests.

## Not verified / not 1:1

- **The Sim never holds a gate race.** Nothing loads a gate race's chain into `Laps::lists.checkpoints`,
  and the Sim's race handler is the lap race. So in a live Sim ride the gate is still `None`. The feeding
  is ready for when a gate race (the machine race, S292a's `LapRules::machine_tick`) is run by the Sim
  and fills that list. That load is filed as S423 (later).
- **No real machine-race gate was read.** The proof's gate is the sx01 line gate copied as a stand-in,
  made a side gate by hand for one brain. No state of a running gate race was used. free09 states hold 8
  machine race gates (oracle.txt, category 0x200020), but no card points at them. The `+0xa4`/`+0xb0`
  offsets come from the driver's reads, not from a real gate's values.
- **Spots.** The chain gates' `+0x94` spots are read here as spots on the AI net, the same way the
  existing test reads the card's gate. In a real gate race that word may be a spot on the race track's
  paths instead. The port stores the raw words either way, so the feed is the same. Which paths it
  really refers to was not checked.

## Next

Nothing.
