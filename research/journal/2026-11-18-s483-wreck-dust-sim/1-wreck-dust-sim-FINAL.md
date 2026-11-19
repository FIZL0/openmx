# 1 — wreck dust sim (FINAL)

Task S483. 2026-11-18.

## What I did

- `tests/ai/wreck_dust.rs` `s483_the_sims_wreck_draws_collision_dust_as_the_original`: S325's nose dive
  (`capture/s138/s325-nose76-2604`) run twice for 40 ticks.
  - The original in Ee as S422 runs it. The human's collision models and bone collision entries (its
    manager, rider `+0x1490`) are counted per tick by the rands their updates make. Each frame's camera is kept.
  - The Sim: the human's bike built and seeded off the image as C33's riders are (race4.rs's `c33_bike`,
    race125p, the bike's body rays). The wreck part, wreck set and ragdoll kit come off the disc. Its riding
    body is the image's, a copy of replay.rs's `b19_riding` on an `Ee`. The world is free09 off the disc (S323).
    Its dust is the human's five collision entries, seeded with S422's `seed`. Each tick the shell's camera
    is the original's.
- Found and fixed a one-tick-early wreck in the Sim's dust. The manager's wreck flag (feed `+0x24` → rider
  `+0x868`) is the state pack's copy of the reason `+0x748`. The pack runs before the frame's verdict, so
  the copy holds the frame before's reason. The manager then reads the copy the frame before made. The
  Sim's feed used `crash.is_some()` straight after the verdict frame, so its collision models drew on 2616.
  The original's first draw is on 2617. New `RiderDust::wrecked` holds the frame before's crash, and the
  snapshot carries that.
- `s483_the_feeds_wreck_word_is_the_frame_befores_reason` proves the lag on nose76 (reason 1 at 2614, copy
  at 2615) and flat (reason 5), and on the dive image, which is already down (8/8 throughout).
- race4.rs: `Bikes`, its `col`, `open`, `phy` and `c33_bike` made `pub(crate)`. Its `RiderDust` literal
  gets `wrecked: false`.
- Nothing taken from ../opentennis: it has no ragdoll or particle-collision code.

## What the original does

- Rider `+0x868` is written by the state pack (`./kit fn 0x00258d50`) from `+0x748`. bike_fx_wire points
  the manager's feed `+0x24` at it (oracle.txt S328c / bike_fx_wire).
- On every whole frame, after the frame `+0x868` equals `+0x748` as it stood after the frame before.
  The manager reads the snapshot before the frame's pack. So a wreck's collision entries switch on two
  frames after the verdict (2616) and emit from the third (2617).

## How it was proved

- A save-state run against the Sim's own ride. The original's whole frames in Ee from
  `s325-nose76-2604.ee.bin`, beside the Sim seeded off the same image.
  - Every tick: the reason (both 1 from 2614) and the snapshot's wreck flag (both from 2615) are equal.
  - Every tick: the two collision models' draws are equal, entry by entry. That is 54 draws on each side
    over 40 ticks.
  - Every entry agrees through 2618, including the bone collisions' first draws (10 + 2 on 2618).
  - After 2618 the bone collisions part. They agree again on 2621, 2630 and 2640. Totals: 156 for the
    original, 114 for the Sim.
- The wreck-word lag: Ee whole frames on three S325 images (20, 10 and 30 frames), asserted every frame.

## Not verified / not 1:1

- **Bone-collision draws after 2618 are counts only, not 1:1.** The Sim's ragdoll drifts 0.1-0.6 ft
  from the original's (R24). Contacts land on different ticks: the Sim draws 114 against 156, and parts
  on 2619-2620, 2622-2629, 2631-2639 and 2641-2644.
- **The wreck-word lag is proved only for a wreck starting** (reason 0 → 1 and 0 → 5). A wreck's end (the
  put-back clearing `+0x748`) is not in these 40 ticks. The Sim treats the end the same way (one frame
  late), unproved.
- **The Sim's ride here is not replay.rs's full rig.** There are no toys or path cars, no kept bike search
  (`seed_bike_search_kept`), and no animated skeleton (`r24_posed`). The idles generator is not seeded.
  The bike is seeded by race4's S174 seed, not by `seed_ride`. The verdict tick and reason still match
  (2614, reason 1). The ragdoll may drift more than R24's rig.
- **The camera is handed in from the original's run**, as a shell would hand in its own. The Sim's own
  camera is not used here.
- **The new `RiderDust::wrecked` is not in the state hash.** The feed it fills was not hashed either.
  A rollback clones the Sim, so the field is carried. Only a hash compare would miss a difference in it.
- **The dive past the airborne limit (reason 8) only shows the steady state.** Its earliest image is
  already down.

## Next

- Nothing for S483. The bone collisions' remaining parting is R24's drift, not the dust.
