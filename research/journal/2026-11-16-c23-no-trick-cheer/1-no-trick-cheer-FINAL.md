# 1 — no trick cheer (FINAL)

Task C23. 2026-11-16.

## What I did

- Took out P42's crowd cheer on a landed trick (`audio.rs`, the seat loop of `Sound::frame`), and the
  seat's `was_grounded` flag that only it read. `SeatFrame::landed_trick` stays (main.rs and
  `online_wreck_sound.rs` fill it; not this task's files), documented as read by no sound.
- New test `tests/audio.rs` `a_landed_trick_queues_no_cheer`: a seat lands from the air on a track with
  a crowd and a cheer to pick, with and without a trick; the mixer holds the same voices. It failed on the
  old code (2 voices against 1) and passes now.
- Chose "remove" over "hang it on `Change::Place`": the original's rule (below) needs who passed whom,
  the gainer card's ridden time and whether each side is human, and `race_start::Watch`'s `Change::Place`
  only says a rider's new place. Doing it half would be a guess. Filed as C36 (low).
- `research/bike-sounds.md`: the trick row now reads **same**; follow-ups point at C23 and C36.
- Nothing from `../opentennis` applies: no crowd cheer logic there.

## What the original does

From P285 (`research/oracle.txt`, `bike_place_change_crowd`, `race_ranking`): the crowd's Cheer and
PlayerCheer come only from the race ranking's place change, its Boo from a human losing a place to an AI
who is then first, its Oh from the wreck entry. Nothing calls the crowd on a trick landing.

## How it was proved

- Unit test above (mixer voice count).
- `./kit xref 0x001fe380` (the crowd play): its only callers are the wreck entry (`./kit fn 0x0027b810`)
  and the place change (`./kit fn 0x0027edb0`, twice). No trick code calls it.

## Not verified / not 1:1

- Not heard on screen or by ear: no track sets `CrowdPresent`, so the crowd is silent in both games.
- The place-change cheer and boo are still missing (C36): needs the passer/loser pair from the ranking.
- The xref lists direct calls only; a call through a function pointer would not show. The crowd play is
  a plain function, not a vtable slot, so none is expected.

## Next

- C36 when races and crowds matter.
