# R53: a downed P2's wreck bike rolling into free09's parked truck

Goal: a capture on the original of P2's wreck bike rolling into the parked truck in the plane lane,
the plane far off, for c16_fly's `a_wreck_bike_in_the_plane_lane_wakes_a_parked_truck`.

Result (second attempt, 2026-11-21): caught, with the wreck forced by a poke. See "The forced wreck"
below. The first attempt's notes follow it: no staged run there gave "P2 wrecked first, then its
wreck bike wakes a sleeping truck".

## The forced wreck (caught)

Steps, on a kit copy with the accessibility bus on (`KIT_PCSX2=N`, a display: a Hyprland headless
output or a private weston):

1. `bash research/r53/start.sh 11 2`: the two-player boot into slot 11 (frame 82).
2. `tele_p2.py PID 11 18 0 3 4 12`: P2 18 ft south of the truck, facing it (+z), settled. Slot 12.
3. `wreck_p2.py PID 12 25 13`: P2's bike velocity 25 ft/s at the truck and its forward limit (rider
   +0x758, 22.0) -1e30, saved as slot 14 (frame 347). Then resumed: the original's wreck test
   (`./kit fn 0x2522c0`, `+0x758 < +0x558`) enters the wreck, reason 4. Paused, limit put back, slot 13
   (frame 348).
4. `./kit record research/r53/r53-wreck-truck.toml --state 13 --frames 150 --out context/capture/r53/wreck-truck.csv`
   (`settle_ms = 0`, else the first ~9 ticks are lost). `show.py CSV` prints it.
5. `extract.py COPY SLOT NAME` writes `context/states/r53/NAME.{ram,spr,p2s}`.

Files (shared context):

- `capture/r53/wreck-truck.csv`: rows 348..497 from slot 13. Row 348 is a pre-load read; use 349 on.
- `capture/r53/wreck-entry.csv`: rows 347..426 from the poked slot 14. It matches wreck-truck on every
  column from row 350.
- `states/r53/r53-348`: the wreck entered (its wreck body is row 349's). `r53-347-poked`: the tick
  before the entry, limit still poked. `r53-staged`, `r53-boot`.

What it shows:

- 349-357: the wreck bike tumbles at the truck, 25 → 16.9 ft/s. The truck sleeps.
- 358: the truck wakes. One own contact entry (kind 0); its other object is 0x96c060, the wreck body
  0x96b020's collision object (+0x1040). Wreck body to truck body 10.6 ft. Plane 391 ft off, parked.
  Row 358 is torn: its wreck columns repeat 357's.
- 359: the wreck bike bounces back at -2.3 ft/s. The truck moves only ~0.02 ft/s.
- The truck sleeps at 364 and wakes again at 374 on a second touch. It flickers to ~410 and sleeps
  from 449. P2 respawns at 445.

Words (MX Unleashed's, not handling/wreck.rs's, which are SLUS-21104's): rider +0x748 reason,
+0x758 forward limit, +0x558 the load it is held to, +0x794 the wreck clock, +0x7a4 chassis (+0x260
velocity), *(+0x7a8)+0x110 the wreck body. +0xb7c stays 0 here. Truck collision object = car +0x170:
+0x100 contact count, +0x108 the entry list (0x80 each: +0 kind, +0x74 other object).

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

## What could unblock it (first attempt; the first route was taken above)

- Wreck P2 by a poke instead of a hit (find the original's wreck entry, the `Some(7)` path's caller,
  and call or force it with P2 a few ft from the truck and the plane parked far off), then record.
- Or a human rides it with two pads (P2 head-on into a fence or prop 10-20 ft in front of the truck).
- Or accept run 2's order (truck woken by the pinned rider, then the wreck) and change the test.

## Tools here

boot.py / start.sh (2p boot), cheat.py, make_tape.py (two-pad tapes), tele_p2.py (move P2 near the
truck), aim.py (put the plane), log.py (live per-frame log), peek.py, players.py, event_diff.py.
Second attempt: wreck_p2.py (the forced wreck), r53-wreck-truck.toml (the capture spec), show.py
(print it), extract.py (a slot to context/states/r53), ram_peek.py (P2 and the truck in an image).
The states that matter are in context/states/r53; the copies' slots are scratch.
