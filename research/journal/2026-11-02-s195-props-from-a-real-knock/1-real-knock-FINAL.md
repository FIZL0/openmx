# 1 — real knock (FINAL)

Task S195. 2026-11-02.

## What I did

- PCSX2 copy 3 (s3's), first_recording's state (free09), quarter speed. No pad (/dev/uinput is
  root-only), so research/s195/aim.py loads the state, teleports the bike (s76_tele.py's way) 40 ft
  short of BeachBall 2 along its heading, lets it settle, puts it back on that line and pokes 25 ft/s
  into the chassis' world velocity (+0x260) and the vehicle's (+0x130); saved to slot 9. (Ball 0 had
  ball 3 in line, so ball 2.)
- research/s195/knock.py from slot 9: watch/s195-knock.toml a row a tick into
  context/capture/s195/knock.csv (2700 rows, frames 2839..5538, 0 torn, 0 dropped), and states saved
  around the strike (kept as context/s195/knock-<frame>.p2s; RAM images by
  `python3 -B research/s195/slots.py ram knock-<frame>` into context/states/s195-knock-<frame>.eeMemory.bin).
  An earlier 240-row run (try1) woke ball 2 on the same frame, 2882: the strike is deterministic.
- research/s195/edge.py: the knock state 15 ticks after the strike (knock-2897's slot), ball 2 moved
  to x 6909.4 by shifting every copy of its position (body, node, collision record) by one delta;
  recorded as edge.csv (300 rows, 2922..3221) with states edge-2922..2924, 2937, 3045.
- crates/game-sim/tests/prop_knock.rs: the woken collections read out of a state's RAM into the
  port's `PropSet` (woken the port's way, then the body, node, countdown, fresh, the collision record's
  moving parts and the node's listener order are the state's), then `frame_among` a row at a time
  with the row's `dt`, compared bit for bit on the body's six lanes, the node origin, enabled, active,
  fresh, countdown and the record's four flags. Three tests:
  - `the_strike_puts_the_objects_listener_on_first` — in knock-2881's image (asked for with the bike
    8 ft off, it landed on the strike's tick) balls 1 and 2 are woken with the object's listener
    before the collection's settle on the world node's list.
  - `the_knocked_balls_fly_land_sleep_and_go_back` — from knock-2884: ball 2 lands at 2912 and agrees
    through 3092; at 3093 the game's record touches another object (the bike) and the port's does
    not. Ball 1 (nudged by the bike passing at 2865) sleeps at 3010 and agrees until the bike wakes it
    at 3322; set from the game's row at its second sleep (3380) it counts 60 s down, is put back at
    5181 and agrees to the capture's end (2699).
  - `a_ball_over_the_edge_goes_back_on_the_tick` — from edge-2937 (row 2938, x 6911.98): put back on
    2939 (disabled, at its place, countdown −1, the collection asleep), agreeing until the bike, parked
    by its place, touches it at 2991.
- No port code changed: everything the replay reaches already agreed once the listener order and the
  capture's phases were read right.

## What the original does

- free09 shares one world node for its props: its listener list (+0x48 count, +0x50 array) holds
  every woken collection's settle (collection +0x34) and object listener (object +0x50), plus one
  other (0x00e1e2b0 in knock-2881). A strike puts the object's listener on first, then the
  collection's settle: `PropSet::wake` (activate, then enable) files them the other way. The order
  shows on the sleep tick: object-first, the body sleeps and the settle sees every body asleep in the
  same tick (active 0 on the sleep row — ball 1 at 3010, 3380); settle-first, the collection stays
  active one more tick (ball 2 at 3274/3275, after the bike's second contact re-woke it).
- An asleep ball keeps its record's flags (touching, on the ground) until it is put back.
- The object's countdown runs from `ResetTime` 60 by the frame's dt while the collection is already
  inactive; on going below zero the ball is put back (identity turn, zero velocity) and the countdown
  is left at what it reached (−0.0269 at 30 Hz), every ball the same.
- Over x 6912 the post check (origin from the last position) disables and puts the ball back on the
  crossing tick, countdown −1, collection asleep the same tick.

## How it was proved

A bit-exact replay against save-state captures: crates/game-sim/tests/prop_knock.rs, 3 tests, over
~2900 compared rows (knock.csv 2885..5538 for two balls, edge.csv 2938..2990).
`./kit check -- -p game-sim --test prop_knock --test prop_collision --test prop_set`: 8 passed (the
capture is in s3's context, so prop_knock ran, not skipped).

Two capture phases the replay allows, each counted in its `Run`:
- `clear`: a row whose four record flags read all clear while the port's are set, the body lanes equal
  — the row read after the next tick's clear (clear → collide → step). 62 such rows on ball 1's
  long run, none parting a body lane.
- `ahead`: a row holding the ball exactly as the next row has it (read a tick late: frame 3636's
  countdown moved two dt, 3637's none). 3 rows.

## Not verified / not 1:1

- The strike itself: the struck listener with the contact's push is S197's; the replay starts after
  it (states two ticks after the wake), and stops where the bike touches a ball again (3093, 3322,
  2991). The port's props tick has no bike.
- `PropSet::wake` files settle then object; a strike files object then settle (above). Not changed
  here (S197 brings the strike and should wake that way); the replay seeds the RAM's order.
- Ball 1's put-back is from a resync to the game's sleep row (body lanes, countdown, active, flags;
  not the forces/shocks, which a sleeping body does not read). Ball 2's own put-back (5075) is not
  replayed: resynced at its 3274 sleep, the port clears its record flags on the next tick where the
  game keeps them (its port path had diverged since 3093, filed differently); not chased.
- The edge was reached by a poke (no free09 prop reaches x 6912 in play). From edge-2922 (ball 2
  pushed out of the edge's ground) the port parts on the second tick by 0.007 ft/s in vel y; the
  poke did not move whatever else the record keeps (its triangle cache), so only the crossing tick is
  claimed (from edge-2937). The z edges and the low (0) side are the same check, not captured.
- Other free09 props' own collision (`f.col`) is not in the scene list the replay loads; no ball came
  near one.
- The captures and states are only in s3's context/; the tests skip elsewhere.

## Next

none — S197 (the strike) should wake object-first and can use knock-2881/2882 and knock.csv rows
2839..2884 for the wake itself.
