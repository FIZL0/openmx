# 1 — camera link flags (FINAL)

Task F48. 2026-11-16.

## What I did

- Found the store behind the posts' write of the bike's flags (`+0x444`). It is the bike's link
  interface's add-link (`./kit dis -p atv 0x004f7210`, store at `0x004f723c`), reached from the
  registry's link pass (`./kit fn -p atv 5075b0`) after the camera controller switches the camera.
- Ported its effect on the flags as `update::camera_link` (`crates/game-sim/src/handling/update.rs`).
  Each key a registered object wants that one of the bike's seven interface masks answers is or-ed into
  the flags.
- The answer to F48's question: **the store can change the flags**, so the claim "it cannot" is false in
  general. It only adds bits. It adds one only when a wanted key is new to the flags. On C15 both posts
  (rows 125, 193) or in 8 and 0x10, already set. But a post that switches to the first-person camera
  (the remembered camera, slot `+0x4c`, can be it) ors in 0x20. The bike answers 0x20 (`+0xa94`) and
  the sync then takes its first-person branches.
- Filed **S345** (low): show the sync's first-person words are presentation only, or carry the camera
  link in the frame.
- Nothing taken from `../opentennis`. It has no such link registry.

## What the original does

- **Add-link** (bike link interface vt `+0x44`): flags `|=` key. Then a (key, wanter) entry goes on its
  list unless one is there (that is `+0x460`/`+0x468`). It never clears a bit.
- **Unlinks** (vt `+0x48`, `+0x4c`) clear bits. Neither runs on the bike on C15.
- **Link pass**: for every registered object, for each key it wants, every object whose lookup (vt
  `+0x40`) answers the key is told (its vt `+0x44`). The wanter gets the answer (its vt `+0x34`).
- **The bike's lookup** (`./kit dis -p atv 0x004ddf20`) answers a key when it shares a bit with one of
  `+0x734`, `+0x9c4`, `+0xa94`, `+0xb54`, `+0x6b4`, `+0x498`, `+0x6a8`.
- **Every wanter class wants one key**, held in one word. The word is set in the constructor from an
  interface name: "CameraInterface" (0x10 on state 4) or "FirstPersonCameraInterface" (0x20).
- **The controller's posts** switch to the wreck camera (`+0x88`, id 0xb), the remembered camera
  (`+0x4c`, ids 7 and 0xd) or the chase camera (`+0x78`, id 6). The switch removes the old camera and
  runs the pass. The chase and wreck cameras want 0x10. The remembered camera is whichever was current,
  so it can be the first-person one (`./kit fn -p atv 490480`, `48ffd0`).
- Details: research/oracle.txt `# F48`.

## How it was proved

All in `crates/game-sim/tests/mxvatv_bike_ride.rs`,
`the_camera_link_into_the_bike_flags_is_the_originals`:

- **Function oracle, 10 000 cases** off state04.ee.bin. Each case is a registry holding the bike's real
  link interface (lookup and add-link the original's) and 0–3 stand-in objects in random order.
  - Stand-ins want 0–2 keys each: a lone bit, bits of the bike's masks, any bits, or 0. The get
    sometimes says none.
  - Stand-ins sometimes answer keys themselves.
  - The bike's flags and its seven masks are random.
  - The original's pass and `camera_link` give the same `+0x444`. The bike is told once per answered
    key.
  - All four paths come up ≥ 1000 times: nothing wanted, none answered, flags grow, flags kept.
- **State 4 facts**, asserted:
  - controller class and slots;
  - each camera's key, asked of the camera itself through its own vt (chase ×2 and wreck 0x10,
    first-person 0x20);
  - the registry (bike, an object wanting 8, the chase camera);
  - `camera_link` on the post targets' keys leaves 0x18, and the first-person key gives 0x38.
- The store's pc, the keys and the removal were found by a temporary probe on C15 flip-1 (removed).
- Ran `./kit check -- -p game-sim --test mxvatv_bike_ride the_camera_link_into`. The full gate runs in
  `./kit done`.

## Not verified / not 1:1

- **The pure frame still drops the posts.** F48 asked to show the store cannot change the flags. It can:
  a post switching to a remembered first-person camera ors in 0x20. The port can compute the change
  (`camera_link`), but nothing in the frame calls it. The original's camera state (which camera is
  remembered) is not in the simulation. Filed S345.
- **Not captured.** No ride switching into first-person and then wrecking was recorded, so the 0x20
  case is shown from the code and the oracle only, not seen live.
- **The camera switch itself (controller vt `+0x84`) was not read.** That it removes the old camera and
  runs the pass was seen in the probe on C15 (removal, then the pass, inside the post). Its other paths
  are not shown. The same goes for the guards on posts 7, 6 and 0xd, and for the first-person camera's
  removal when it is the old camera.
- **The stand-ins' answers** (`+0x34`, `+0x44` on non-bike objects) are hooks. Only the bike's words
  are compared. Their own lists and flags are not watched.
- **The list words `+0x460`/`+0x468`** are not ported. F31b5f showed they are read only by later posts.
- **Interface bits are assigned at run time by name** (`0x00214b30`). 0x10 and 0x20 are state 4's
  values; another boot order could number them differently. `camera_link` takes the keys as given.

## Next

Nothing for F48. S345 decides whether the 0x20 case matters to the simulation.
