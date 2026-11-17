# 1 — untamed monster replay (FINAL)

Task MD161a. 2026-11-16.

## What I did

- Built an Untamed car frame harness, `crates/game-sim/tests/mxvatv_car_frame/untamed.rs`, beside
  drives.rs (which only gained three `pub(crate)`s). It is keyed by `UDrive` (name, image, car, vtable),
  so MD163a's baja and MD170's golf cart can add their own drives to it.
- It runs SLUS-21701's sim frame in the interpreter from MD160's spawn and flipped images, and feeds
  the tape through `atv_car::input_untamed`.
- On every frame, inside the physics world step, it reads the truck through a view at SLUS-21104's
  offsets (`u()`, plus the remaps below). It runs the port's `frame_untamed`, or `atv_car::Ride` with
  the Untamed switches on, and compares every word against the original's after its step.
- The view remaps Untamed's layouts that differ from SLUS-21104's:
  - the contact entries
  - the collidables
  - the sort probe
  - the vehicle state's pad and wheel words
  - the terrain extent
  - the nodes
- Contacts with another record (the loose props the truck pushes) are answered with
  `atv_contact::resolve`, on the records and bodies as they stand when the original resolves.
- One bug found on the way: the view handed Untamed's vehicle state pad pointer to the port as the
  "wheel" word, so the port steered with no rate limit. Untamed's wheel word is vehicle state +0x58.
- Twelve tests, all passing:
  - `untamed_{throttle,steer,jump,wreck}_capture`
  - `untamed_*_frame_beside`
  - `untamed_*_ride_beside`
- Filed follow-ups:
  - MD175: Untamed's put-back
  - MD176: a tire standing on an object
  - MD177: ground and noise from the port

## What the original does

- The car step is SLUS-21104's at moved offsets. On these four drives, no port code needed to change.
- The rows in research/oracle.txt `# MD161a` give the layouts:
  - contact entry: kind u16, surface i16, other +0x80, mover +0x84
  - collidable: fields +0x2c and +0x30 later
  - sort probe: flags +0x144, list +0x1b0
  - vehicle state: wheel +0x58, pad +0x54
- The record resolve runs four times a frame from the vehicle force when there are contacts.
- The unwreck's respawn calls Untamed's own put-back (`./kit dis 0x4ca900 -p game` on the Untamed
  scratch config):
  - the node is placed at the kept spot
  - a ground query under it, raised 0.3 along the normal
  - the rows come from the spot
  - the body is placed from the node, then an event is posted
  - then the tire and engine resets, as in SLUS-21104

## How it was proved

- **Bit-exact replay** of MD160's four captures (context/capture/md160, tapes in
  context/recordings/md160).
- **Through `frame_untamed` and through `Ride`, every word** of the car, body, pieces, vehicle and
  record, per drive:

  | drive | frames | words | not compared |
  |---|---|---|---|
  | throttle | 305 | 652,989 | 5 |
  | steer | 280 | 598,782 | 0 |
  | jump | 370 | 794,514 | 0 |
  | wreck | 414 | 885,531 | 1 |

  - Throttle: 92 frames have contacts with a prop.
  - Jump: two landings.
  - Wreck: 61 frames wrecked; the skipped frame is the put-back.
- **The interpreter against the capture** (the `_capture` tests): the body, engine and tires equal each
  row, except early and torn rows. Fewer than a third of rows may be off, and one of the last ten must
  match.
- Only test files changed, so every existing replay hash is unchanged.

## Not verified / not 1:1

- **Throttle frames 161, 258, 259, 260 and 268 are not compared.** A tire stands on a loose prop. The
  port hands that tire's step to its caller, and nothing answers it for Untamed (MD176). The frames
  after them are compared again, because each frame is re-seeded from the original.
- **Wreck frame 83 is not compared.** The unwreck stands the truck at its spot through Untamed's own
  put-back. The port only has MX Unleashed's (MD175). That frame's ground queries are answered with
  the nearest one the original asked.
- **Ground queries and engine noise come from the original's own calls.** They are not from a terrain
  read or the ported noise, as in drives.rs. The harness only checks the port asks at the same points
  (MD177).
- **The sort search is answered from the original's RAM**, as in drives.rs.
- **The props' own words after the step are not compared.** Their records and bodies feed the port's
  resolve as a snapshot taken at the original's first resolve call of the frame. The other three
  calls are only counted.
- **The put-back spot words (car +0xd30..+0xd6c) are laid in from the capture each frame.** Free
  Ride's script writes them outside the sim frame, and that script is not ported.
- **Early and torn capture rows are not compared** against the interpreter. The recorder read them
  before or during the frame.
- **The offset map's edges inside its unmatched gaps are guesses.** The readers read nothing there.
- **The flipped image's wreck comes from MD160's state poke**, not a natural crash.
- **The jump drive starts from the spawn image**, as MD160 recorded it. The air image is not used.
- **The car's keys are read from the image**, with the Untamed switches set. They are not built
  through `atv_car::Load` from monstertruck.phy. MD161's untamed_truck test covers that file.
- **Only the player's truck is compared.** The second car's tire probes are filtered out.

## Next

Land it (`kit done`). Then MD163a puts its baja drives on `UDrive` in untamed.rs.
