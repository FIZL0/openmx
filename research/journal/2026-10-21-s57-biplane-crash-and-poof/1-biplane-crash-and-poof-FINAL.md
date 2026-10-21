# 1 — biplane crash and poof (FINAL)

Task S57. 2026-10-21. The user: the biplane cannot crash. In the original it does, then it poofs (a
puff of smoke, the plane gone, the player back with a fade-in), and L1 and R1 do the same poof.

## What I did

- **Looked at the original first, live, on copy 5.** I flew `capture/flight.pad` from the biplane
  recording's state, logging `capture/poof.toml` every frame and recording PCSX2's Video Capture
  beside it. On the bike I rode `capture/bike.pad` from first_recording's state with
  `capture/bike.toml`. The recipe is `capture/run.sh`. The CSVs and MP4s are in `context/s57/` and
  are not committed.
- **What the original's biplane does is not a poof or a fade.**
  - A ground crash shows white engine smoke in the dive and a dark dust puff on the wreck tick.
  - The wreck then tumbles for 5 s (151 ticks).
  - Then there is a **hard cut** to the spawn on the unwreck tick. There is no screen fade, no puff at
    the respawn, and the plane is drawn opaque straight away.
  - L1+R1 (both, on the wheels or wrecked) is that same unwreck, on the press frame.
  - L1 or R1 alone is the plane's rudder, and nothing else.
  - None of the three faders' fields moved in either capture.
- **The user's "poof and fade-in" is the bike's hand reset.** The bike's L1+R1 (reset_kind 1) puts the
  bike back in place. The bike and the rider are drawn see-through, and they come back to opaque over
  about 0.7–1 s, while the shadow stays opaque. There is no screen fade there either: the bike's hand
  reset asks for no fade, and the only fade the reset request knows (100 ms out, 100 ms in) is for the
  other kind of request. I did not find the timer and alpha of that model fade (see below).
- **Ported the plane's chord: `Flight::reset_chord`** (`crates/game-sim/src/flight.rs`). With
  `respawn` set, and the plane on its wheels or wrecked, it runs the unwreck: back to the spawn at
  rest, with the wreck's gravity, damping and inertia given back. In the air it does nothing.
  `openmx`'s `Plane::step` (`crates/openmx/src/vehicle.rs`) calls it before the frame, the way the
  original's input slot runs first. The `respawn` it passes is `Controls::from_pad(..).respawn` off
  pad 0. It returns whether the plane went back, and `Garage::step` hands that out as the frame's
  `respawned`.
- **The schemes.** P40's new scheme moves only the gas and the brake (R2 to Cross, L2 to Square).
  L1 and R1 are the shoulder buttons under both schemes, so the chord is L1+R1 under both, and the
  test checks both.
- **Left out the original's once-a-frame latch on purpose.** The original's L1 and R1 press events are
  two calls, so it needs a latch to reset once per frame. Here the chord is one `respawn` a frame,
  so there is nothing to latch.

## What the original does

- The plane's chord: `./kit fn 0x355df0`. It reads commands 0x4c and 0x4d together. It is gated on
  grounded (+0x590) or wrecked (+0x3b0), and it calls the vtable's +0x1c4, which is the unwreck,
  `./kit fn 0x3536e0`. Its latch +0xe0c is cleared in the MX frame, `./kit fn 0x354b20`.
- The fader manager is `*(*(*(0x41f770)+0x3d0)+0xd4)`. Its fields and its users are in
  research/oracle.txt (the S57 block):
  - The fade on a ride reset: `./kit fn 0x1ffe80`, called from `./kit fn 0x27b9f0` only with page ≠ 0.
  - The bike's hand reset, `./kit fn 0x27bef0`, passes page 0.
  - The other fade users are a freeride item system (`./kit fn 0x3620d0`) and the replay's end
    (`./kit fn 0x3b40d0`).
- The oracle's older `wreck_page_raise` and `wreck_page_show` names are wrong. The S57 block corrects
  them, but I did not rename the lines (other tasks own them).

## How it was proved

- **The crash and the respawn, bit for bit, are S36's.** `the_biplane_crashes_and_respawns`
  (`crates/game-sim/tests/biplane.rs`) already replays the captured crash through both exits, the
  chord's and the 151-tick wait, including the respawn at the spawn. S57's own capture (run1) shows
  the same: the wreck at frame 12067, the cut at 12218, 151 ticks, back at (2304, 138.614, 2281).
- **`the_reset_chord_puts_the_flown_plane_back`** (`crates/game-sim/tests/flight.rs`) uses the disc's
  plane over flat ground. It checks each case:
  - no chord: nothing happens;
  - the chord on the wheels: the plane is at the spawn with the throttle 0 and the velocity 0;
  - the chord in the air: nothing happens;
  - a map-edge wreck (reason 13): the chord gives back gravity, damping and inertia. The whole
    `Flight` is `==` a fresh `Flight::reset` at the same spawn.

  Mutation check: an assert changed to reason 14 failed at that line, so the test runs and does not
  skip.
- **`l1_and_r1_put_the_biplane_back_on_the_press_frame`** (`crates/openmx/tests/ride.rs`) runs under
  both schemes, through the real pad scheme, input poll and `Garage::step`:
  - L1 alone and R1 alone do nothing;
  - L1 then L1+R1 on the ground puts the plane back on that frame;
  - a held chord does not fire a second time;
  - the chord 50 ft up does nothing.

  Mutation check: with the chord disabled, the test failed ("New: R1 pressed with L1 held did not put
  the plane back").
- The full `./kit check` passed.

## Not verified / not 1:1

- **The plane still cannot crash into the ground or into an object in the port.** `Flight` feeds the
  crash test no contact (`touching: false`). Calling the world's contact search in the substeps is
  S48's (open), so a captured ground crash cannot be replayed through `Flight` yet. Only the chord,
  the wait-out and the map edge work. The state-level replay is S36's.
- **No crash into an object was recorded.** The flight never hit one.
- **The bike's see-through fade-in after a hand reset is not ported, and its timer and alpha were not
  found.** I tried:
  - the decompilation's strings;
  - a RAM diff of three save states around a chord (`capture/ghost_states.sh`).

  Both gave only noise, because the states were too close together. The ~0.7–1 s length is read off
  the video by eye.
- **The particles are not drawn**: the wreck's dust puff and the engine smoke. The port has no
  biplane particles.
- **P41's menu switch and the M key are left as the hard cut they already were.** The brief asked
  them to use "the same poof and fade". The original's plane has neither, so there is nothing 1:1 to
  give them. If the user wants the bike's ghost fade there, that waits on the follow-up below.
- **The latch is left out** (see above). That is equivalent only as long as `respawn` is computed
  once a frame.
- **For the plane, `main.rs` prints "back on the start line", as it does for the bike.** The original
  shows no message.

## Next

Follow-ups are filed in the plan:

- After S48: replay a live ground crash (`capture/flight.pad` + `poof.toml`) through `Flight`, bit
  for bit.
- Find and port the bike's hand-reset ghost fade-in. Then, if the user wants, use it for P41 and M.
- The biplane's wreck dust and engine smoke.
