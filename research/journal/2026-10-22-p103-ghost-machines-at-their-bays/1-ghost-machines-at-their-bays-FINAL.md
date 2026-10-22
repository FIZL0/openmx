# 1 — ghost machines at their bays (FINAL)

Task P103. 2026-10-22.

## What I did

- `vehicle.rs`: the bays shown (`bays_shown`: the lot's ported machines, so the planes, one a
  player; none with the setting off), a bay's parked plane (`bay_plane`), the original's trigger
  (`offer`: kinds in the original's order, nearest bay within < 30 ft in 3D, not on top of the
  rider, no offer to a wrecked rider or above 58.667 ft/s, never the kind the rider is on), the
  pop-up's question per machine (`offer_text`), and `Answer`, which takes the offer on Circle's
  press edge or a tap. `BAY_ALPHA` is P57's put-back ghost alpha (`Ghost::START`, 0.5).
  `Garage::plane_flown` is the plane only while it is up; the bay draws the parked one.
- `main.rs`: every seat riding the bike gets `seat_offer`; Circle (raw pad, before the scheme)
  takes it through P102's `switch_machine`, so the plane spawns on the player, and the tape and the
  pause menu hear of it just as they do for the menu's switch. Offers are off with a front menu
  open (other than riding), online and during a tape. The GPU path draws every bay's plane at
  `BAY_ALPHA` through the `Actor.alpha` path. Bays are in no collision world: the bike asks the
  track's, the plane flies over the heightfield. The radar marks every bay.
- `overlay.rs`: the pop-up as PCSX2 draws it: a rounded see-through panel, the question wrapped
  in two centred lines (squeezed to 0.9 of the font's width, as the original draws it), the black
  pill, `nav_circle_off` and "Accept". `Drawn.offer` / `offer_button` report it.
- `menu.rs`: a Settings row "Machines at bays On/Off", saved as `machine_bays = off` in
  settings.txt (no line when on). Off: no bays, so no ghost, no blip, no offer; the pause menu's
  switch is untouched.
- `tests/bays.rs`: alpha < 1, and off shows nothing; a bike ridden through a bay rides tick for
  tick as one with no bay; the trigger's range, 3D distance, speed and wreck rules; Circle's press
  edge and the spawn on the player; the pop-up drawn off the disc (lines, button centre, panel
  darkening, pill, red Circle, grey text), not drawn when not offered or with the setting off; the
  GPU draws it as the CPU does. `tests/menu.rs`: the settings round trip writes the row off and
  reads it back; the VSync/frame-limit walk now steps over the new row (Cross off and on) before
  the modifiers, still the last row.

## What the original does

`./kit fn 0x003a8330` each frame scans kinds MotocrossVehicle, Biplane, Helicopter, MXBuggy,
MXMonsterTruck, MXBaja; for each, the nearest driverless actor in 3D, kept if < 30 ft. Locked
kinds are skipped, unless the cheat flag `read32(0x4e2778)+0x5c` is set. `./kit fn 0x003a7210`
shows the prompt unless the rider is wrecked or above 58.667 ft/s. `./kit fn 0x003601b0` is the
answer: `machine_swap` into the parked machine where it stands.

## How it was proved

- Live on PCSX2 (KIT_PCSX2=2, free09, Monster Truck unlocked by the cheat flag): the prompt
  showed beside the truck; Circle swapped (the truck actor took the human's driver, a "Driving
  Controls" card showed); Cross did nothing. Screenshots in context/shots/p103-*.png (not
  committed).
- Looked right beside a screenshot: the port's CPU drawing of the truck's pop-up
  (`tests/bays.rs` writes it to target/tmp/p103-pop-up.rgb), compared ink box by ink box with
  context/shots/p103-unlocked.png. Line one inks 213–427 × 336–353 to the original's 214–429 ×
  336–352; line two 222–418 from 358, then 356 after the line step went 21 → 19; "Accept"
  301–357 × 398–411 to 301–358 × 398–411; the Circle's ink 277–293 × 396–412 to the measured 16
  px at (278,395).
- `./kit check`: passes, with tests/bays.rs's 6 tests on the disc (their --nocapture run printed the
  picture's path and the GPU comparison: 0.011 / 0.026 apart on average, so neither skipped).

## Not verified / not 1:1

- Distance: the port measures from the bike's position to the bay's spawn point. The original
  measures between its actors' own positions (box centre / node), so the 30 ft edge may sit a
  foot or two differently.
- No unlock list: the port offers the plane whenever it is in the garage. The original skips
  locked kinds.
- The other gates in `0x003a7210` (activity and menu conditions besides wreck and speed) are not
  ported. The port's own gates are: menu, online and tape.
- No Decline: the original's popup may take a second button (Triangle?) to dismiss; it showed only
  "Accept", and the port has only Circle. Riding away hides it in both.
- The geometry was measured on the Monster Truck's prompt on free09. The biplane's was not seen
  live; its text comes from `STR_SWITCH_BIPLANE` and its wrap is the same rule.
- Split screen: the original's layout moves the popup (lyt offsets x −150 / y −40 per view). The
  port anchors it bottom-centre of each view, unmeasured.
- The Flash popup's animation (fade/slide in) and the "Driving Controls" card after a swap are
  not reproduced.
- Spawn: the original puts the player into the parked machine where it stands. The port spawns
  it on the player, as the brief and P102 ask. The bay's ghost stays drawn while that player's
  plane is up.
- The software rasteriser draws no ghosts (it draws solid only). Bays show on the GPU path alone.
- Only the biplane is ported, so only planes stand at bays and are offered; the other machines'
  bays are read and not shown.
- A first 150 ms Circle closed the original's prompt with no swap; unexplained (maybe the press
  fell in a frame the popup was not taking input). A 300 ms press swapped.
- The text's 0.9 squeeze is measured, not read from the .fsh's matrix.
- hud.rs is untouched: the pop-up is drawn in overlay.rs with the rest of the HUD art.

## Next

none
