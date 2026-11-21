# 1 — driven car store writer (FINAL)

Task R68. 2026-11-21.

## What I did

- Found the writer of a driven car's put-back store (car +0xe0 spot, +0xf0 nose, +0x100 up): the car's
  **safe-spot recorder**, `./kit fn 0x003a1a00`. It runs twice a frame on a car the human has switched
  into, and moves the store to the ground under the car.
- Answered R68's question: **a Restart after driving puts the car back at its last recorded safe spot, not
  its bay.** Proved by running the world reset over s342's image.
- Wrote it into `research/oracle.txt` (block "R68", after R67's block).
- Filed S501 (port the recorder) and noted S500 (put back from the moved store).
- Nothing taken from `../opentennis`: it has no cars.

## What the original does

- The recorder is the car's virtual +0x31c (on the +0x48 vtable 0x0045cf80, slot 0x0045d29c), a thunk
  `j 0x003a1a00` at 0x003a2180 that takes 0xd24 off its argument. So it is also the vehicle interface's
  method (the interface sits at car +0xd24, its vtable at interface +0x20, slot +0x20).
- Two callers a frame (watched in the interpreter, return addresses 0x003a0508 and 0x00269180):
  - The car's post step `./kit dis 0x003a04e0 0x003a0508`: when car +0xd28 is 0, call vt+0x31c(car, root
    node +0xb0, root node +0xa0): the car's place and its nose row.
  - The mode-3 score manager `./kit dis 0x00269160 0x00269180` (score_mode3 0x00268d00): on its current
    vehicle's interface (mgr +4), vt+0x20(iface, mgr +0x60, mgr +0x120).
- Interface words: car +0xd24 is the vehicle's **score card** (card +4 points back at the interface,
  +8 the card's flags); car +0xd28 is the activity the car is entered in (MachineRace +0x50 in R52's
  free07 image, where the baja is a waiting machine racer; 0 in free ride).
- The recorder (`./kit fn 0x003a1a00`, `./kit dis 0x003a1a00 0x003a1d80`):
  - No card (car +0xd24 == 0): returns 0, writes nothing.
  - Card, no activity, and card flag 0x800000 clear: the probe point is the root node's place
    (0x001b2870) raised by the float at 0x00421960, dropped onto the ground by 0x001dc960 / 0x003a10b0
    through the car's ray at +0xdd0. If the ground is not found below the lift, return 1 with nothing
    written. (Card flag 0x800000 set, or an activity: the passed spot is the probe point.)
  - Car state +0x754 != 13: 0x003a1580(car, point, nose arg, &normal, 0). On a hit with normal.y > 0.8
    (0x3f4ccccd): spot = point + 0.5 (0x00421968) × normal (.xyz), nose = the nose arg with y = 0,
    rsqrt-normalised; up = normal; then nose is squared against up by two opmula/opmsub pairs
    (side = nose × up, nose = up × side; the same pair physics_object_place uses). Returns 1.
  - Car state +0x754 == 13 and car +0x530 != 0: spot = root node +0xb0, nose = root node +0xa0 with y = 0
    normalised, 0x003a1580(..., car +0x100, 1) writes the up straight into the store; spot and nose are
    stored only on a hit.
- The world reset (R67) then puts the car back from whatever the store holds.

## How it was proved

- **The writer, by a store log** (`Ee.log`) over the original's whole frame in the interpreter, run off
  `context/states/s342-baja-free07.ee.bin` (the frame runner of `game-sim/tests/ragdoll_wrecks.rs`
  `whole_frame`, copied into a throwaway test, not committed; dt 1/30, the image's pads left as saved).
  Eleven frames ran: every store into car +0xe0..+0x10f came from 0x003a1c28 / 0x003a1c34 / 0x003a1c38 /
  0x003a1c5c / 0x003a1c68 / 0x003a1ca0 (all in 0x003a1a00), twice a frame. The spot followed the coasting
  baja: z 2164.85 → 2160.37 over ten frames, about 0.4 ft below the body's place (body +0x50).
- **The callers**, by a hook on 0x003a1a00 that logs and runs it: ra 0x003a0508 (args the root node's
  place and nose) and ra 0x00269180 (mgr +0x60, mgr +0x120). Both returned 1.
- **Parked cars stay put**: the same run off `r52-free07-baja.ee.bin` (machine racer, car +0xd28 set):
  no store writes in two frames. C34's free-ride image before the switch (`context/c34/cheat.p2s`,
  unpacked with `./kit state ram`): the buggy (0x00605270) has no card (+0xd24 = 0), store = bay. After
  the switch (`switched.p2s`): card 0x00922ba0, +0xd28 = 0, store still the bay (not yet moved).
- **Where a Restart puts it**: `./kit oracle 0x0020b780 --ints 0x006168c0 --ram
  context/states/s342-baja-free07.ee.bin --read 0x01342680:16 --read 0x00605350:48 --budget 200000000`
  (world = app 0x00479990 +0x3cc, read from the image). The body's place (body 0x01342630 +0x50) becomes
  457b2aeb 430b1e31 45074da8 = (4018.68, 139.12, 2164.85), the store spot, bit for bit. The bay (car
  +0x270) is (4020, 138.72, 2357): the car lands ~192 ft from it.
- R67's note had the s342 store spot as (4017.2, 139.1, 2164.8); the image holds 4018.6824 (0x457b2aeb).

## Not verified / not 1:1

- **Switching out was not seen.** No image after the human leaves a car: whether the card stays on it
  (so an awake, rolling car keeps recording) or goes was not checked. Either way nothing found puts the
  bay back in the store, but that rests on the eleven-frame log and R67's vt+0xe0 scan, not a global one.
- **Other writers in other situations** (a wreck, a reset after a wreck, the car flipped, a machine race)
  were not traced: the log covered eleven coasting frames only.
- **Car state +0x754 == 13** (the second branch, with car +0x530) is not named or run: no image has it.
- **Card flag 0x800000** (use the passed spot instead of probing) was not followed: s342's card flags are
  0x180c06 (clear).
- **The ground probes** 0x003a1580, 0x003a10b0 and 0x001dc960 were run, not read clause by clause; their
  failure cases (no hit, normal.y <= 0.8: store left as it was) are from the decompilation.
- **The frame run is not a faithful replay**: the pads were left as the image saved them and the run hit
  an unmapped read (pc 0x001970c8) on frame 11. The writer and its callers do not depend on that; the
  exact spot bits of frames after the first were not checked against the live game.
- **Not seen live**: no Restart was done in PCSX2 after driving (no pad here; the reset is proved in the
  interpreter, as R67's was).

## Next

- S501: port the recorder for switched cars. S500: put back from the moved store (noted on it).
