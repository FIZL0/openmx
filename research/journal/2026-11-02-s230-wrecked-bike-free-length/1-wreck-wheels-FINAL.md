# 1 — the wreck entry zeroes the wheels' compression (FINAL)

Task S230. 2026-11-02.

## What was holding the squat

- The pitch is not the problem. Once a crash has taken over, the bike is drawn from the wreck body's
  own pose (`game_render::chassis::wrecked`), and that body is simulated.
- The squat is. `openmx`'s animation pass calls `chassis.frame(.., Drive::of(ride))` on every moving
  tick, wrecks included. `Drive::of` reads `ride.front/rear.w.compression`, and while the rider is down
  nothing writes those, because the ground step is skipped while a wreck reason is set. So the
  drawn fork and swingarm kept the compression of the wreck's own tick. On a wheelie that falls off
  the back, that means a rear shock near the bottom of its travel.
- The original's base wreck entry (`bike_wreck_entry`, decompiled with `./kit fn`) stores
  `0` to `+0xf0` of both `+0x7b0` (front) and `+0x7b4` (rear) wheels after the body's velocity
  and spin copies. `+0xf0` is the compression (`wheel.rs`, `Drive`'s doc). `oracle.txt`'s
  description "the wheel loads zeroed" is this write. `rider.rs`'s entry comment had it as not ported.
- The entry does not write `+0xf4` (last compression). An ELF scan of every `0xf4(` store in the
  vehicle/wheel range found only the wheel constructor (`0x0025a730`, called from the vehicle build)
  zeroing it, and nothing on the wreck path.

## What changed

- `crates/game-sim/src/ride.rs`: `Ride::wreck_wheels`, which zeroes both wheels' `compression` and
  leaves `last_compression` alone.
- `crates/game-sim/src/rider.rs`: the wreck entry calls it right after `Crash::begin`, whether or
  not the rider carries a wreck body, as the original's entry always does. The comment listing what
  is not ported no longer names the wheel loads.
- Nothing in the renderer changed. It already draws `Drive::of`, which now reads 0 on both wheels
  for the whole time the rider is down. Compression 0 is the free length.

## Checked

- `tests/ride.rs` `the_originals_wreck_entry_zeroes_both_wheels_compression`: runs the original's
  own entry (vtable `+0xfc`, reason 11) in `ps2-oracle` on the human's bike in
  `first_recording`'s RAM, with both wheels' compression and last compression set beforehand. After
  the call `+0xf0` is 0 on both and `+0xf4` is unchanged. It ran here; it did not skip.
- `tests/ride.rs` `a_wrecked_bike_springs_back_to_its_free_length`: S26's 30 ft drop wreck (reason 5).
  The suspension is loaded on the tick before the wreck. From the wreck tick to the put-back, both
  compressions are exactly 0, and the last compression holds the wreck tick's nonzero value. As a
  mutation, with the call taken out it fails on the wreck tick at about (0.88, 0.99), nearly bottomed out.
- P153's captured state (`context/capture/p153/wreck-3790.p2s`, s1 only) is 65 ticks into a
  reason-11 wreck (entered at 3725, the throw for falling off the back). Its player bike (reason 11 at
  `+0x748`) reads compression 0.0 on both wheels. A riding AI bike in the same image reads 0.55 and
  0.43. The script that read this was a throwaway in the scratchpad.
- `md25_mx_vs_atv_rides_as_before` re-pinned: the gesture ride's fold moved
  (`0xf7529726e979e7cf` → `0xe4d29850faf9e8b9`). That ride's 360 ends in a wreck and compression is
  hashed. The diagonal's fold did not move.
- `./kit check -- -p game-sim` over ride, air_wrecks, crash_lockout, hash, off_the_map, ragdoll_bike,
  ragdoll_entry, ragdoll_gate, ragdoll_riding, ragdoll_wrecks, replay, rider_anim, snapshot, state,
  two_p_rider, wrap_rest, wreck_bike, wreck_page, wreck_sound, wreck_thresholds: 298 passed.

## Not verified / not 1:1

- The original springs back in one tick, at the entry, not over "a few seconds". That is what its code
  does and what the port now does. No live PCSX2 capture of the wheel compression over a wreck was
  recorded, because P153's csv has no compression columns. The proof is the entry run in the oracle
  plus one RAM image taken mid-wreck.
- The bike's drawn chassis clip during a wreck is still the shell's own stance (V13's open half), not
  the original's. Only the drives' compression is fixed here.
- The rest of the base wreck entry that the port still lacks (the vtable `+0x104` call, the
  collision-record clear's sibling writes `+0x4a0/+0x4a4/+0x7dc/+0xb7c`, the actor-list walk) is
  untouched. None of it reads or writes the suspension.
