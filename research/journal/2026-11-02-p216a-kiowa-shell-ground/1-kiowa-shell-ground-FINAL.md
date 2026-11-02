# P216a: the shell's Kiowa on C14a's ground

## What changed

- `vehicle::KiowaRide` carries the collision object's ray probe (`rays`, set by `with_rays` off
  `helicopter\col\kiowam.col`, loaded beside `kiowa.phy` in main.rs). Without the file the probe is
  empty and the helicopter meets nothing.
- `Heli` carries a `kiowa::Collision`, built in `KiowaRide::built` off `[Mass Properties]`, the body,
  and the built inertia tensor `KIOWA_INERTIA` (C14's image, body +0x110: 0x4628adfa 0x461f901b
  0x44a2ae40), its nearby list refreshed where it stands. `Heli::touching()` is the record's flag
  (chase.rs reads it).
- `KiowaRide::step` searches the collision object, then ticks with `KiowaAround`, which is now
  `kiowa::Grounded` plus the port's wreck: `crashed` hides the blades on a wreck, as before, and
  asks for the rebuild on a put-back. The rebuild keeps the rng.
- `KiowaRide::wrecks` counts wrecks since it was taken; the shell's `kiowa:` line ends
  `wrecked N times`.

## Checked

`./kit check -- -p openmx --test kiowa_switch`: 4 passed, none skipped (run with --nocapture):

- `a_landing_at_speed_wrecks`: 60 ticks of Cross (110 ft up), 200 ticks of stick forward
  (~128 ft/s), then Square with the stick forward. It wrecks once and is put back on its skids
  ~990 ft on.
- `a_soft_landing_stands`: 60 ticks up, 400 of Square straight down: 0 wrecks, on its skids, 0.6 ft
  under where it was taken (the 0.25 ft drop and the skids settling).
- P216's two (climb on Cross, stands left alone) still pass. Left alone it now sits 0.6 ft under
  the taken point (the drop is the port's KIOWA_DROP; the record settles it).

`./kit check -- -p openmx --lib --test bays --test kiowa_switch --test kiowa_particles --test
overlay`: 115 passed.

## Not checked

- Which test fired the wreck in the fast landing (speed or rotor in ground): only that one did.
- The wreck's body effects, blade throw and put-back are the port's until S220a1 (P216a1).
- The touchdown sound's rand: `touchdown_sound` stays false (no clips, V114).
- No picture looked at.
