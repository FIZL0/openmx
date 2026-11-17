# 1 — planes meet on a tape (FINAL)

Task N24. 2026-11-17.

## What I did

- Wrote a two-pad tape writer (`research/n24/make_tape2.py`) and a two-plane live log (`log2.py`).
- Made `meet.p2m2` on C16's state: P2 switches to plane 1 from its own pause menu and creeps forward;
  P1 taxis plane 0 into its tail at 32 ft/s and pushes it. Made `order.p2m2`, where P2 switches first.
- Captured the meeting with both planes' blocks (`watch/n24-meet.toml`, made by `make_spec.py`) and saved
  17 RAM images around it.
- Tests in `crates/game-sim/tests/c16_fly.rs` (N24 section):
  - `n24_p1s_plane_is_plane_0_whoever_switched_first`: on both tapes, seat 0's plane is the actor list's
    first and the level node's first object.
  - `n24_planes_meet_as_the_original_flies_them`: four runs of 100 ticks, both orders, pinned in `MEETS`.
  - Two ignored helpers print every run (`n24_meeting_holds`) and the port beside the rows
    (`n24_rows_beside_the_port`).
- `seeded_plane` now wraps `seeded_plane_at`, which seeds from any plane record (plane 1 too).
- Details, recipes and the table: `research/n24/index.md`.

## What the original does

- Each player's plane is an actor; P1's is always the list's first entry, whichever player switched to
  a plane first. Its collision object sits before P2's in the level node's array, so plane 0 searches
  and resolves first in each substep.
- The node is not at S88's fixed address in this heap. The test finds it as the one node whose object
  array holds both planes' records.

## How it was proved

- A save-state capture of a generated two-pad tape (1232 fields), plus 17 RAM images of the original.
- From four images (138200, 138213, 138219, 138500), the port flies both planes together with a Met
  each. With P1's plane first, both planes hold all 100 ticks bit for bit (two rows read inside a frame
  at 138212-138213 aside). With P2's plane first they part after 0 to 19 ticks.
- So the original flies seat 0's plane first, and each post's refill against the other plane's hull is
  right through the touch (row 138217) and the hard hit (row 138221).
- The seat test reads two images: the meeting's (P1 switched first) and the order tape's (P2 first).

## Not verified / not 1:1

- **The port's shell flies the planes the wrong way round.** It flies the last flying seat first
  (main.rs and likely net/rides.rs). Not fixed here: N24 only touches c16_fly.rs. Follow-up filed.
- **Two planes pushing one bike in one frame: not checked.** Both players' bikes leave the world while
  they fly, so no state has two flown planes and a bike. It needs a third rider (AI) or a parked bike
  that stays, which this session doesn't offer.
- **Not airborne.** The meeting is a ground taxi. Planes meeting in the air are not tested.
- **Torn rows.** Runs from 138000, 138150 and 138225 part on rows that are none of the port's states,
  in both orders alike (rows read a tick late, then on time, or mid-step in the hit). They are left out
  of `MEETS`, not explained bit for bit.
- **Mid-frame rows pass by a looser rule.** `flight::substeps` keeps no bodies for planes flown
  together, so rows 138212-138213 pass when every axis lies between two port states, not bit for bit.
- **The tape offset** (field n polled at 137404 + n, C16's) was not searched after the in-tape pause
  menus. The runs hold on it, which is the evidence it is right.
- **No crash and no wrap.** Neither plane crashes, and the world wrap is not reached.
- **The order tape is checked on one image** (frame 137750), not captured.
- Nothing was taken from opentennis.

## Next

- N28: fly the planes in seat order in the shell and online.
