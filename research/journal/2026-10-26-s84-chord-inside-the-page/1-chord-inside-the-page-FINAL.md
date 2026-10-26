# 1 — chord inside the page (FINAL)

Task S84. 2026-10-26.

## What I did

- Read the second respawn's code path. The chord's request (player_hand_reset, page 0) respawns the
  bike at once. It calls the screen's vt +0xa0, which only touches HUD flags (S72), so the bike stays a
  listener on the fader. The black draw then runs bike_page_listener 0x00281c10 → player_respawn
  0x0027bb20 → bike_unwreck 0x00252ee0 → vt +0x138 0x0027b650 → ghost_start. That chain places the
  node on the put-back record +0x3b0 as it stands at that moment, and starts the ghost at 3.0. It is
  the same chain as every other put-back, so "in place" was never what the original does.
- Captured it. I wrote `research/s84_chord.py` and `watch/s84-chord.toml`: S72's fields plus the chord
  latch +0x14d0, the kind +0x14d4, the record +0x3b0/+0x3c0/+0x3d0, the applied point +0x5e0 and pad
  0's buffer.
  - The state was copy 5's slot 9, the one S72 recorded from, copied into copy 2's slot 9. Copy 2's
    old slot 9 is saved at /tmp/claude-1000/-home-rycs-repos-openmx-slots-s2/2cb22e43-652f-4709-a659-cc6fede3d61b/scratchpad/copy2-slot09.p2s.
  - It starts in its own reason-6 wreck, which asks for the page about 77 rows in.
  - L1 is held from the first row. R1 goes down when a fader reads fading out (`--delay 0`) or one row
    later (`--delay 1`).
  - Four captures are in context/capture/s84/ (not committed):
    - d0a: chord on R+1.
    - d0b: chord on R+2, the black draw's own tick.
    - d1a, d1b: the press came a tick too late and landed on R+3, after the page's own put-back.
  - I worked out the chord's tick from the clock, which freezes at the put-back, not from the frame
    counter, because rows get read at different points inside a tick.
- Measured, on both in-window chords:
  - The black draw puts the riding bike down a second time. +0x5e0 equals +0x3b0 bit for bit, using
    the record from the draw's row: the one this tick's keep left after the chord's riding tick or
    ticks.
  - The ghost restarts at 3.0.
  - The chassis then rides on from the new spot: d0a's centre of gravity jumps about 2.5 ft from
    where the bike had ridden to, and d0b's jumps too.
- Ported it: `rider.rs` step 7 now calls `ride.place_back()` for a riding bike too. The port's riding
  keep runs inside `frame_bumped`, before step 7, so `ride.back` holds the record as kept that tick,
  which is the original's order.
- opentennis has nothing like this. I took nothing from it.

## How it was proved

- `tests/wreck_page.rs`:
  - `the_originals_draw_sets_a_chorded_bike_down_again`, over the four captures. For each in-window
    chord it checks:
    - The chord's put-back came while fading out and landed on the record.
    - The next change of the applied point is on a held row, with the bike already riding.
    - That point equals the record of the draw's row (or the row before's) bit for bit, and the port's
      `place_back` gives the same point.
    - The ghost started over.
  - It counts 2 inside the window and 2 after it. Raising its count assert to 3 made it fail, so the
    test does read the data.
  - `the_rider_is_put_back_by_the_page` (the wiring): after a chord in the fade out, the record is
    moved off the bike and the bike given a speed. The page's draw must then set the bike on
    `ride.back` from rest, with the ghost at 3.0. Without the fix it fails ("not left at (0, 0.953,
    0)").
- The full gate runs in `./kit done`.

## Not verified / not 1:1

- **The chassis after the second put-back is not compared tick by tick.** Only the applied point (the
  node's placement) and the ghost are checked bit for bit. The velocities being zeroed and the next
  ticks' motion are taken from the shared code path: the listener's respawn is the same
  player_respawn as the chord's and the page's own put-back, which S66/S74 proved. A full ride replay
  of free09's state does not exist.
- **Chords on R+3 and later (in the fade in) are not checked against the port.** In d1a and d1b the
  chord put the bike back again while the screen was held. The port's chord is not gated on the page
  either, but those two captures are only counted, not compared.
- **The orientation (+0x3c0/+0x3d0 → node rows) of the second put-back is not compared.** Only the
  point is. The port's `place_back` builds the rows the same way as S66's proved put-back.
- **Two in-window chords, one state, one wreck reason (6).** The press is wall-clock, so which tick it
  landed on is luck; R+1 and R+2 were each hit once. A reason-2 page was not tried. A cannonball's page
  cannot be chorded at all (the chord is gated on reason != 13).
- **The second respawn's sound and HUD reset** (player_respawn's other calls) are presentation and
  not ported, as before.

## Next

Done.
