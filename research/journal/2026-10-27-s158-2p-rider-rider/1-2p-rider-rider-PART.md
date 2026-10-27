# 1 — 2p rider rider (PART)

Task S158. 2026-10-27. Blocked: the 2p recording holds no rider-vs-rider contact.

## What I did

- Read C10's index (research/journal/2026-10-26-c10-2p-capture, "Rider-vs-rider contacts"): its table is
  empty. "No record entry names the other rider's record on any row; the riders are never closer than
  136.7 ft."
- Checked that myself on `context/capture/2p/2p.csv` (`two_p::CAPTURE`), with a scratch script that
  streams every row:
  - 12,917 rows (= `two_p::ROWS`);
  - every collision-record entry's `other` (`p1_c0..c10.other`, `p2_c0..c10.other`) is 0 on every row,
    so every entry is scenery;
  - both wheels' owner words (`p{1,2}_wf_owner`, `p{1,2}_wr_owner`) are 0 on every row;
  - `rec_struck` is set on 5 rows for P1 and 4 for P2, which are C10's four scenery body-ray contacts
    (S157's subject);
  - the closest the two chassis centres come is 136.70 ft, at frame_counter 129845 (row 1963,
    0-based). A rider-vs-rider candidate needs the frames within a frame radius of each other's search
    reach (`rider_contact`'s broad phase), a few feet, not 136.
- `crates/game-sim/tests/two_p.rs` already pins this:
  `no_rider_vs_rider_nor_truck_contact_in_this_ride` asserts every entry's `other` is 0 and the riders
  are never within 100 ft. A second copy of that test in rider_rider_capture.rs would prove nothing
  new, so I added none and left the code alone.
- What is proved for rider-vs-rider is still S88's work on S93's split tapes
  (`tests/rider_rider_capture.rs`): two contacts, both chassis bit for bit, the wreck, and the original
  stepping the last player first. Those pins stay as they are.

Nothing in the 2p ride can be replayed against the port for this task. It waits on plan/HUMAN.md's
open item "A second 2p recording with the riders meeting and a truck ride" (head-on, side swipe, landing
on, into a wreck), which C10 filed. S159 (trucks) waits on the same recording.

## What the original does

Not read again for this task: S88's journal has the rider-vs-rider search and resolve.

## How it was proved

That there is nothing to replay: a full scan of the capture of record (12,917 rows, both lanes, all 22
entry `other` words and four wheel owner words), which agrees with C10's index and with two_p.rs's
existing test.

## Not verified / not 1:1

- **No 2p rider-vs-rider contact is replayed.** The ride has none (above). Head-on, side swipe, landing
  on and into-a-wreck in the 2p reference are all unproven until a recording holds them.
- **Race500P rider-vs-rider is unproven.** S88's contacts were on race125p (the split tapes); the 2p
  bikes are Race500P (S157). The frame radius, mass and the "Race50" hit reach the rider-vs-rider resolve
  and wreck read for a 500 have never been checked against the original in a contact.
- **Which chassis resolves first** is still proved only on S93's images (S88), not on the 2p state.

## Next

When `context/recordings/split/2p-contacts.p2m2` (with its `_SaveState.p2s`) is in: C10's way of
capturing it (research/c10/play_capture.sh, both lanes with the entries' `other`), save a RAM image at
the tick before each contact (research/c10/at_ticks.py), and add a 2p case per contact to
rider_rider_capture.rs, built like S88's four tests (image → original tick lands on the row; port
`gather` = original search; port `Bump::resolve` = original resolve on both chassis; the tick with the
port's resolve lands on the row and wrecks the same rider).
