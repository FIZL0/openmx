# 1 — hud speed zero when down (FINAL)

Task P182. 2026-10-28.

## What I did

- `ride_hud::speed_shown(speed, down)`: the speedometer's speed, 0 while the rider is down.
- The shell's `RideHud::with` (`main.rs`) hands the HUD `speed_shown(target speed, bike up && rider.crash is Some)`.
  Before, it handed the camera target's speed, which during a wreck is `chase::wrecked`'s — the tumbling wreck
  body's speed (P180 saw 33 falling to 17). The camera keeps that target; only the HUD reads 0.
- Both readouts take it: the digital text says `0` and the analog needle sits at rest.
- Test `crates/openmx/tests/ride_hud.rs::a_wrecked_rides_hud_speed_is_0`: a crash whose wreck body moves at
  30 ft/s, through `chase::wrecked`, drawn by `ride_hud::sprites` at 1080p in both styles — the readout is "0"
  and the needle is at rest; riding, the readout is the ridden speed unchanged.
- Nothing taken from opentennis (it has no speedometer).

## What the original does

The readout (`./kit fn 0x002ac580`, the text update) has no wreck gate of its own: it formats `%.0f` of
`**(iface+0x1c) * 0.68182`, floored at 0. The rider's per-tick copy-out (`./kit fn 0x00258d50`) writes
`rider+0x860 = chassis+0x34` (the speed) every tick, and `rider+0x868` the wreck reason. On film (S109 run2.mp4,
29.6–33.6 s) the readout is 0 from the wreck's tick to the put-back, so whatever the pointer reads is 0 while
the rider is down — the chassis is not stepped during a wreck, and its speed reads 0 there.

## How it was proved

Looked right beside the original's film (S109 run2.mp4, by P180): 0 from the wreck to the put-back. The port's
rule is the unit test above. No RAM capture of the speed word during a wreck exists (run2.csv has no speed
column).

## Not verified / not 1:1

- **Which word zeroes, and when exactly.** The pointer at `iface+0x1c` was never pinned (V26's note: at
  rec-start it held `rider+0x88c`, while `+0x20/+0x34` held `rider+0x860`), and no writer that zeroes the
  chassis speed on the wreck entry was traced. The port shows 0 from the tick `rider.crash` is set to the tick
  it clears; the film agrees to the video's resolution, but a one-tick offset at either end was not measured.
- **The biplane's wreck.** A crashed plane's HUD keeps showing the flight's airspeed (floored at 0) as before:
  the original's plane-wreck readout was not filmed or captured, so it was left alone.
- **Online/other seats** go through the same `RideHud::with`, so they take the rule too; not run in a split or
  netplay ride by hand.

## Next

Nothing for P182.
