# 1 — wreck bike leaves at the put back (FINAL)

Task S108. 2026-10-23.

## What I did

- Found what takes the wreck bike (B, the vehicle's wreck part `+0x7a8` → `+0x128`) out of the broad
  phase, and when: **the put-back**, not "some 400 frames" into the wreck. S105's 400 was only where
  the late s62 save states happen to be. In every s62 capture B+4 (its placement record) and B+0x31
  (3 → 0; bit 0 is the gate the sphere pair checks first) both drop on the row where the wreck
  reason clears, 79 ticks after each entry: back 2477→2556, front 2602→2681, front3 →2681, side
  2607→2686, tip 2472→2551. B+0x30 is 0xff on every row, so S105 read the wrong byte.
- Port: `ragdoll_ride::Down::step_posed_with(.., bike_in)`. `step_posed` keeps its old behaviour
  (`bike_in = true`). With the bike out, the gather is told it is not offered (so a rebuilt list
  never holds it) and the scene pairs no sphere with it, even when a kept list still holds it (the
  original's gate does the same).
- `rider.rs` steps the doll (step 2) before the verdict and the page update, but the original's
  body runs after both. So before the doll step the rider now predicts this tick's put-back: the
  page's last fade-out update (`wreck_page::Page::puts_back_next`, new), or the ride machine's
  `Reset::Now` peeked on a copy of the machine. On that tick it steps the bones with the bike out.
  The L1+R1 chord has no doll step on its tick, so it needs nothing.
- Corrected S105's "400 frames / B+0x30 0xff" in research/oracle.txt, the `ragdoll_scene` module doc
  and `the_bike_is_in_the_tree_where_its_sphere_stands`'s doc.

## What the original does

- Every put-back (page fader, the machine's reset, the chord) runs the player respawn, which runs
  `bike_unwreck` (`./kit fn 0x00252ee0`). That calls the wreck part's stop through its vtable
  (`+0x38` → `+0x120`, a1 0): `./kit fn 0x0015b670`. For the part's `+0x124`/`+0x12c` and its
  `+0x128` (the bike), the stop calls `./kit fn 0x001ab1d0`, which takes the collidable out of each
  tree node its record lists (`./kit fn 0x001ae2f0`, the tree's vt `+0x2c` remove, here
  `./kit fn 0x001968a0`), frees the record and sets `+4 = 0`. Then it calls the collidable's vt
  `+0x18` on `+8` (`./kit fn 0x00132ae0`), which clears `+0x31` bits 0 and 1, the gate.
- The reverse happens at wreck entry: the part's start, vt `+0x11c` (`./kit fn 0x0015b550`), inserts
  each collidable that is not placed (through 0x00119c60) and raises its gate.
- The body actor's update runs after the bike's update and the fader, so on the put-back's tick the
  bones gather once more with B already in no node (key none, nothing offered).

## How it was proved

- **Save-state oracle**, `tests/ragdoll_wrecks.rs::the_unwreck_takes_the_wreck_bike_out_of_the_broad_phase`.
  It runs the original's `bike_unwreck` (the whole function, vehicle reached through the game
  manager) on each s62 wreck's last entry state: back 2479, front 2604, front3 2632, side 2609, tip
  2474. Before: B is the wreck part's `+0x128`, the part's vt `+0x120` is the stop, B is placed and
  gated in, and the tree holds it. After: `+4 == 0`, gate bit clear, the tree holds it in no node.
  A probe panic confirmed all five states ran (not skipped).
- **Replay of the captures**, `every_sphere_test_of_the_wrecks_is_the_bike_meshs`. Its gather hook
  now records each gather where B+4 is 0 and checks that one has no key and a lowered gate. The
  result is exactly one per wreck, on the put-back tick: `["front 2681", "side 2686", "back 2556",
  "tip 2551", "front3 2681"]`.
- **Port**, `tests/ragdoll.rs`:
  - `the_riders_bones_lose_the_wreck_bike_on_the_put_back_tick`: a Rider down in a reason-5 wreck,
    with the wreck clock run out. For screen on (page fader) and screen off (the machine's
    `Reset::Now`), the bike is listed on every tick before the put-back and not on the put-back's
    tick. Forcing `bike_in = true` in rider.rs turns it red (checked once).
  - `the_rides_bones_gather_without_a_wreck_bike_gone_from_the_broad_phase`: rebuilt lists never
    hold an absent bike, kept lists keep what they held, and no sphere meets it.
- `tests/wreck_page.rs::the_page_says_its_put_back_before_the_update`: `puts_back_next` agrees with
  `update` over a whole page.
- `./kit check` passed. One earlier full run failed `smooth_picture::a_shootoff_and_its_put_back_draw_once`
  ("down for 76 frames of 361": a paced ride that drew only 361 of its 600 ticks under the busy
  gate). It passed when run alone, and the next full run was green.

## Not verified / not 1:1

- No capture of the put-back tick's bones compares against the port. The port's rider-level test
  only shows the gather drops the bike on that tick. Whether the bones' positions on that tick match
  the original was not checked, because no capture of that tick's bones exists.
- In the port's own wrecks no sphere ever meets the bike, so no test shows a would-be contact being
  refused when the bike is out. The slope wreck's bones never reach it in 150 ticks. Refusal rests
  on S105's `WreckBikeScene`, given `listed = false`.
- The put-back prediction in rider.rs peeks the machine with the settle flag and no off-map closure
  (`|_| None`). If an edge throw wrecks the rider again on the same tick the machine resets, the
  peek could disagree with the real step. That corner has never been seen and is untested.
- A kept list still holds `listed = true` in the port's `BikeSearch` on the put-back tick. Only the
  pairing is gated (as in the original, where the gate refuses it), and the port drops the ragdoll
  right after, so this is never read. `ragdoll_shown` is only drawn.
- Calls in `bike_unwreck` other than the stop (attitude, actions, physics-world body removal) are
  not part of this task. They ran inside the oracle call but nothing about them was asserted.

## Next

none
