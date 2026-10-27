# 1 — 2p truck rides (READY)

Task S159. 2026-10-27.

## What I did

- Read C10's index (research/journal/2026-10-26-c10-2p-capture/1-2p-capture-FINAL.md, "Truck rides"):
  it lists **none**. Then I checked that against the capture of record, `context/capture/2p/2p.csv`
  (12,917 rows, frames 127882..140798), and did not just take C10's word for it:
  - `python3 -B research/c10/owners.py 2p`: both riders' front and rear wheel path-car words
    (`p?_wf_car`, `p?_wr_car`, the wheel's +0x1f4 from S136) and owner words are 0 on all 12,917 rows.
  - `python3 -B research/c10/near.py 2p`: the closest any rider gets to any of the six path cars is
    P1 to car 3, 183.9 ft at 131338 (dy −59.2). P2 never gets within 525 ft of one.
- So there is nothing for this task to replay: the per-tick comparison it asks for (carried velocity, deck
  contacts, staying on vs sliding off, getting on and off, each from the RAM image before it) has no
  stretch to run on, and C10 saved no RAM image for one. No 2p truck-ride test was written, because a test
  over an empty list would only pass while checking nothing.
- No new recording has turned up: `context/recordings/split/` here and in the main checkout still holds only
  `2p.p2m2` and the two older split tapes. plan/HUMAN.md already asks the user for
  `2p-contacts.p2m2` (riders meeting, a truck ride, prop hits), so I did not file the request again.
- Blocked the task on that recording.

## What the original does

Not read again for this task. S136/S145 describe the carry (the wheel's path-car word, the car's previous
matrix at +0x1e0, the owner branch).

## How it was proved

Nothing was proved: the recording has no truck ride to prove against. The finding that it has none comes
from reading every row of `2p.csv` (owners.py, near.py, above).

## Not verified / not 1:1

- **The whole task**: riding a free09 trailer/truck in the 2p ride was not compared against the original,
  because the recording has no such ride (no wheel on a path car on any row, and no rider within 184 ft of
  one). S136's carry and S146's drag-slip guess are just as unconfirmed as they were before.

## Next

Waits on the human (plan/HUMAN.md, "A second 2p recording with the riders meeting and a truck ride").
When `context/recordings/split/2p-contacts.p2m2` exists and C10's play_capture.sh shows that it plays back
into the same ride: capture it with C10's spec (the spec includes the path-car columns), index the truck
stretches with research/c10/index.py, save a RAM image before each on/off tick, then do this task as written
in `crates/game-sim/tests/platforms.rs`.
