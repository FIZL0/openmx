# 1 — kiowa strikes props (FINAL)

Task B34c. 2026-11-18.

## What I did

- Car half: already done by S340 (commit 006ff1ba5, journal 2026-11-18-s340-car-strike-searches-the-ball).
  `Machine::step_props_in` strikes through `PropSet::strike_in`, so a ball the car wakes searches in its
  strike pass. The shell (vehicle.rs `CarRide::step_traffic`) and car_drive.rs use it. No car change here.
  Only car_props.rs still calls `step_props` (no pass), as S340 noted.
- Kiowa half: checked whether it strikes props at all. In the original it does; in the port it never does.
- Read C14's image (context/states/c14-kiowa.ee.bin, heli 0x007ccae0, record *(heli+0x170)): the lists of
  the record's ray probe, sweep and collidable. Written in research/oracle.txt under C14a (B34c).
- No code change: porting it needs a capture of the Kiowa striking a prop, which does not exist yet.
  Filed C43 (the capture) and S471 (the port, after C43).
- opentennis: nothing there on vehicles striking props; nothing taken.

## What the original does

- The Kiowa's collision record is the plane's class (C14a). Its ray probe's category list is mode 1
  ("never these") with one entry, 0x100000. Its object list skips only its own collidable and sweep.
- The knockable props' f.col collidables are category 0x18 (prop_world::COLLIDABLE), collides-with all.
- So the Kiowa's rays meet any knockable prop in its nearby list, as the plane's do (B34, S241). Its
  sweep excludes 0x8 and 0x800, the same as the port's `sweep_categories`.
- The port: `kiowa::Collision::search_knocking` offers its rays the thrown blades only, and the shell
  (vehicle.rs `drive_met`) runs the traffic's whole frame, then the Kiowa. A Kiowa flies through free09's
  signs and toys and never wakes one.

## How it was proved

- A save-state read: C14's image, the three probes' list words, read by hand (python over the .ee.bin).
- Nothing about a strike's outcome is proved: no capture of one exists.

## Not verified / not 1:1

- The Kiowa striking a knockable prop: not ported, not 1:1. The port's Kiowa passes through them. Needs an
  original capture of a strike (C43), then the port (S471).
- That the Kiowa does strike props in the original rests on its masks in C14's image, not on a strike seen
  live. The masks leave no way out that I found, but no frame shows it.
- The car's strike: proved by S340 on C34's capture through fc 780; nothing more checked here.

## Next

- C43: capture the Kiowa flying into a free09 knockable prop from C14's image.
- S471: port the Kiowa's prop strike as the car's `step_props_in` has it, proved on C43.
