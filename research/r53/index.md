# R53: a downed P2's wreck bike rolling into free09's parked truck (not caught)

Goal: a capture on the original of P2's wreck bike rolling into the parked truck in the plane lane,
the plane far off, for c16_fly's `a_wreck_bike_in_the_plane_lane_wakes_a_parked_truck`.

Result: blocked. The two-player boot works and the truck, plane and both riders are found and logged,
but no staged run gave "P2 wrecked first, then its wreck bike wakes a sleeping truck".

## The two-player boot (boot.py)

R48's boot.py with these extra pokes (settings = *0x00479d64, event = settings + 0xc):

- settings +0 = players (2).
- viewport mode `*(settings + 8) + 4 = 2` (vertical split). Without it the game reads null at pc
  0x3309c8 (FUN_00330970, the StuntScoreOverlays UI lookup) and PCSX2 stops on an R5900 exception.
- event +0x114 byte = players, event +0x124 = 2.
- R48's own: +0x11c = 1, +0x128 = 4 (parks [Monster2]), track free09.

The boot ticks at 30 Hz (two tape fields a tick), not C16's 60.

## Addresses in this boot

- plane 0x604740 (type 1, player 0: P1 is switched into it by the cheat-flag nav tape).
- truck 0x1339fd0 (type 2, vtable 0x0045edac at +0x88), body at +0x160: +0 awake, +0x60 pos, +0x70 vel.
  Parked at (4687.97, 134.62, 3837.87), nose -z.
- P2 rider 0x911c90 (type 4): chassis +0x7a4, wreck reason +0x748, wrecked +0xb7c, wreck body
  *(rider + 0x7a8) + 0x110 (+0x60 pos, +0x70 vel). P2 vehicle 0x1600c70 (type 8), position +0x1b0.
- P1 rider 0x14770e0, vehicle 0x890800.

## Display

Hyprland had no outputs, so PCSX2 segfaulted on its first draw. A private headless weston
(`weston --backend=headless --renderer=gl --shell=kiosk --socket=r53-wl`; pixman fails with
VK_ERROR_SURFACE_LOST_KHR) and PCSX2 started with `setsid` (else it dies with the shell) works.

## Runs (log.py beside research/c14/play.sh, P2 idle)

1. P2 throttling head-on into the truck at 56 ft/s: no wreck. The truck woke, was shoved ~10 ft and
   slept again ~110 frames on. A riding bike that rides into the car does not wreck.
2. P2 22 ft south of the truck, plane 90 ft south nose +z, 160 fields of throttle (~63 ft/s): the plane
   carried P2 into the truck (truck awake at 868), P2 wrecked with reason 7 at 890 pinned between
   them, its wreck body stopped ~10 ft from the truck. P2 respawned at 970 (the wreck lasts ~80
   frames). The truck was woken by the plane and riding P2 first, and stayed awake under the plane.
3. Plane 75 ft west: it stuck on the barn west of the truck, never reached P2.
4. Plane from the south-west at 56 ft/s into P2 (22 ft from the truck): the plane stopped dead, P2
   was shoved a few ft, no wreck.
5. P2 60 ft south, plane 150 ft south, 120 fields of throttle: the plane carried P2 at 60 ft/s into the
   truck, P2 stopped without a wreck, the plane flew over the truck.

## Why it does not stage

- Reason 7 is the head-on contact wreck (handling/wreck.rs: the contact's surface id equals +0x1530
  and the hit's component is over 0.95). In these runs it came only with P2 pinned against the truck,
  so the truck is always woken by the push first.
- A plane hit in the open shoves or carries a riding bike but did not wreck it.
- The wreck bike lives ~80 frames (2.7 s) before the respawn, so it cannot roll far to a truck.

## What could unblock it

- Wreck P2 by a poke instead of a hit (find the original's wreck entry, the `Some(7)` path's caller,
  and call or force it with P2 a few ft from the truck and the plane parked far off), then record.
- Or a human rides it with two pads (P2 head-on into a fence or prop 10-20 ft in front of the truck).
- Or accept run 2's order (truck woken by the pinned rider, then the wreck) and change the test.

## Tools here

boot.py / start.sh (2p boot), cheat.py, make_tape.py (two-pad tapes), tele_p2.py (move P2 near the
truck), aim.py (put the plane), log.py (live per-frame log), peek.py, players.py, event_diff.py.
Copy 2's save slots 11-16 hold the boot, the cheat flag, P1 in the plane (13) and P2 staged (14, 15).
