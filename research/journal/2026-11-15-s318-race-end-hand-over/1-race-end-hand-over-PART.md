# 1 — race end hand over (PART)

Task S318. 2026-11-15.

## What I did

- Read where the shell's race ends today. P313 already calls `Menu::race_ended` at the lap line's
  Over (`crates/openmx/src/main.rs`, the lap-line block after the stadium flashes), through
  `race_start::hand_over`. That call already passes the cards in the lap line's list order, every
  finish's `race_over` answer (`race_watch.overs`) and `race_end`'s answer.
- What it lacks is what the task is for, and none of it exists in the Sim yet:
  - each card's statistics tracker words (laps led, times in lead, passes, times passed, wrecks,
    preloads, clutches) and the card's longest jump (`+0xf0`) and hang time (`+0x100`).
    `hand_over` passes `CardStats::default()`, and `race_end`'s `EndSeat` gets 0 preloads and
    0 clutches, so the tips can't be right. The tracker port is **S319**, still open.
  - the running Race's per-rider `RaceCard` from `RaceRules::card`, which is **S301**, still open.
    The task's own line says "after S301".
- No code changed. Wiring zeros through again would only move the stand-in somewhere else.

## What the original does

Not read this session. Nothing could be wired without S319's tracker.

## How it was proved

Nothing was proved. No product change.

## Not verified / not 1:1

- The whole task. It waits on S319 (tracker words and the card's jump and hang) and S301 (the
  running Race's cards). Until then the results board's tracker pages read 0, and the
  preload and clutch tips read 0 counts.
- AI riders are named `Rider N` in `race_start::hand_over`, not by the AI's own name. Not checked
  whether the Sim keeps the AI's name anywhere.
- `race_over` is handed a default `HallOfFame`, so no Hall of Fame mark can come out. The saved
  tables aren't loaded into the race yet.

## Next

Once S301 and S319 land: fill `CardStats` per card in `race_start::hand_over` from the rider's
tracker and card, give `EndSeat` the tracker's preloads and clutches, then add a test in
`crates/openmx/tests/menu.rs` that rides a race to the Over and checks the board's tracker pages.
