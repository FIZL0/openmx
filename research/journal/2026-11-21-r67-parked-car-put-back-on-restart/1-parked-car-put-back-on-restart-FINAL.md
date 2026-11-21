# 1 — parked car put back on restart (FINAL)

Task R67. 2026-11-21.

## What I did

- Answered: **yes**, the original puts every parked car back again in play, once per world reset. The
  pause menu's Restart runs one; so do row 8 (Restart Activity, by its place) and the results' Restart.
- Traced the chain in the code, then ran it in the interpreter over R52's free07 baja image.
- Wrote it into `research/oracle.txt` (block "R67", beside `physics_object_place`; a note on
  `actor_manager_each_early`).
- Filed **S500** (the port change: put each parked car back on the shell's restart, re-squaring its store)
  and **R68** (what moves a driven car's store; see below).
- Nothing taken from `../opentennis`: it has no parked machines.

## What the original does

- Pause menu input (`./kit fn 0x0028be30`) switches on the picked row's id. Row ids name their rows through
  `./kit dis 0x0028d410 0x0028d510` (table at 0x004349f0): 0 Resume, 1 Restart, 2 Player Options, 3 Event
  Options, 4 Audio Options, 5 Quit. Rows 1 and 8 unpause, then call the world reset (`./kit fn 0x0020b780`)
  on app+0x3cc, unless the event's mode (+4) is 4.
- The world reset calls the game manager (world+0x30) vt+0x74 = `./kit fn 0x0031a440`. That resets the
  players, then the actors: the actor manager's vt+0x7c (`./kit fn 0x0021aa80`) walks every actor and calls
  its reset (vt+0x28 on the actor's +0x48 vtable).
- A car's reset (`./kit dis 0x00398e70 0x00398ec0`) calls its vt+0x198, the put-back 0x00396e90, which is
  S484/S498's `physics_object_reset` on the wrapper: squares the stored nose and up in place and turns the
  body by them.
- The world load (`./kit fn 0x00205c40`) runs the same world reset once. That is the load's put-back S498
  saw (the image's store is squared exactly once). So after N restarts the store is squared N+1 times.
- The fliers' actor reset slot is the base class's (0x00245680), not a put-back: a world reset leaves a
  parked plane's and helicopter's store as it was.
- Found on the way: a **driven** car's store spot follows the car. In `s342-baja-free07.ee.bin` the baja's
  store spot (car +0xe0) is (4017.2, 139.1, 2164.8), by its body, not its bay (car +0x270, z 2357). The
  store nose differs from the bay's too. The only virtual store calls (wrapper vt+0xe0) are in the
  vehicle factory. So the writer is something else. Filed as R68.

## How it was proved

- A save-state RAM image run in the interpreter (`./kit oracle`), over
  `context/states/r52-free07-baja.ee.bin`:
  - `./kit oracle 0x0021aa80 --ints 0x007d11c0` (the actor manager, from game manager 0x007d0b90 +0x34),
    and the whole world reset `./kit oracle 0x0020b780 --ints 0x006168c0` (app 0x00479990 +0x3cc). Both
    gave the same words:
  - store up (car 0x00605270 +0x100): 3d06e5fb 3f7e608d 3ddc225a → **3d06e5fc 3f7e608f 3ddc225d**. These
    are S498's twice-squared bits. The nose keeps its bits.
  - body +0x170: **3d4d3378 3f3220f5 3cd26c82 3f374c7b**. This is S498's quat from a second put-back.
  - One put-back per world reset: the whole reset gives the same bits as the actor walk alone.
- Fliers: the same actor walk over `r52-free10-biplane.ee.bin` (manager 0x007a6780) and
  `r52-free06-heli.ee.bin` (0x007d1070). The plane's and helicopter's store (actor +0xe0..+0x108) xyz are
  unchanged.
- The actor lists, read from the images (scratchpad script): free07 holds the baja (vt 0x0045cf80, +0x28
  0x00398e70), the bike and the rider. No other actor puts the car back.
- Row id 1 = "Restart": read off the row-name table (0x0028d460 formats STR_PAUSEMENU_RESTART).

## Not verified / not 1:1

- **Not seen live.** No pause → Restart in PCSX2: there is no pad here (`/dev/uinput` is root-only), and a
  tape through the pause menu was more than this check needed. The menu row → world reset link is read
  from the code. The world reset's effect on the car is proved in the interpreter.
- **Row 8's name** (Restart Activity) is a guess from the row-name strings. Ids 6+ are not in the 0..5
  table, and the builder that names them was not read. Row 8 calls the same world reset either way.
- **Other callers of the world reset** (0x00292a70, 0x0029f580) were not identified.
- **Repeats past the second were not run.** The CLI oracle starts from the image each time, so "squared
  three times" was not computed. The port's `spawn_axes` repeated is expected to give it (S498 matched it
  for twice).
- **What a reset does to a car that is moving or being driven** was not run. Only a car asleep in its bay
  was put back. A driven car's store moves (R68), so where it lands is open.
- **The port's restart was not read.** Whether the shell rebuilds parked cars on a restart, or keeps them,
  is S500's to find. The port builds each car once with `Parked::settled`.
- **Fliers on a restart**: only the actor-list reset was run on them. Another restart path (e.g. their
  racer interface, vt+0x198 callers at 0x00355c50..0x00355d30) was not traced.

## Next

- S500: the port change. R68: what moves a driven car's store.
