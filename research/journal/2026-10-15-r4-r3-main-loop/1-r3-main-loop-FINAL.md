# 1 — the game's state machine (FINAL)

Task R4. 2026-10-15. Static reading plus a live perturbation proof on PCSX2 copy 2 (BIOS available now,
which R3 did not have).

## What I did

R3 stopped at "the frame is one slot in a dispatch table": `main` loops on `app->iface[0x40](app)` and
`MXGame`'s override of that slot is `0x00201d00`. R4 reads that override. Its whole body is a `switch` on
one word of the app object — **`app+0x404`** — and that word is the game's top-level state machine.

Three things were needed to make the result trustworthy:

1. **Read the switch and every function it calls** (`./kit fn`, `./kit dis` to confirm the fall-through and
   the `return 0` in case 8). Eleven callees, no more: `./kit xref @main_loop` *is* the mode table.
2. **Find out which mode is which** without guessing from the call graph alone. Two levers:
   - the `.cfg` parser (`0x00203260`) and the StartMode name parser (`0x00203a70`), which spell the modes
     out in the game's own words ("FrontEndUI", "WaypointEditor", "SingleRace", "Freestyle", …);
   - a **function → source-file map** built from the heap API: every allocation entry
     (`operator new` `0x001e50b0`, `malloc` `0x001e4e90`, …) takes `(size, __FILE__, __LINE__)`, so a
     function that allocates names its own `.cpp` in its arguments. 1,385 of 8,558 functions attribute
     directly; the linker lays each translation unit out contiguously, so the gaps interpolate into 334
     runs. That turns `FUN_00201d00` into "MXGame.cpp" and `FUN_0022bd90` into "UIBase.cpp". Scripts are
     in the session scratchpad, not committed — they are 40 lines of regex over the dump and are cheaper
     to rebuild than to maintain.
3. **Drive each mode live and watch it happen.** With the game at its main menu I wrote each request value
   into `app+0x404` over PINE and polled the scene pointer, the two gameplay singletons, the in-game flag
   and the frame counter. That is what the "proved" column below means.

Nothing was renamed, moved or ported. The deliverable is 60-odd names in `kit.toml [symbols]` and this
entry.

## What the original does

### The state machine

`MXGame::Tick` (`0x00201d00`, app slot 0x40) is, in full:

```c
bool MXGame::Tick(app) {
  switch (app->mode_request) {           /* app+0x404 */
    case 1: enter_world(); app->mode_request = 0; break;
    case 2: if (ui_still_loading) goto tick;            /* re-check next frame */
            world_wait(); reset_viewports(); loading_begin("Main");
            leave_world(); build_front_end("Main"); loading_end("Main");
            app->mode_request = 0; break;
    case 4: if (!two_frame_delay_elapsed) goto tick;
            ui_flush(); tear_down_front_end("Initial");
            loading_begin("Main"); build_front_end("Main"); loading_end("Main");
            app->mode_request = 0; break;
    case 5: loading_begin("Main"); build_front_end("Main"); loading_end("Main");
            app->mode_request = 0; break;
    case 6: if (!two_frame_delay_elapsed) goto tick;
            tear_down_front_end("Main"); app+0x584 = 0;
            commit_event(settings->event); enter_world();
            app->mode_request = 0; break;
    case 7: if (ui_still_loading) goto tick;
            leave_world(); app->mode_request = 1; break;   /* chains! */
    case 8: shutdown(); app->mode_request = 0; return false;   /* leaves main's loop */
  }
tick:
  Game::Tick(app);                       /* 0x00131100 — R3's frame */
  if (app->quit_flag) { gfx_drain(); gfx->driver[0x28](gfx, 1); }
  return !app->quit_flag;
}
```

So the state machine is **not** a "current state" variable: `app+0x404` is a *request* mailbox. It is 0
almost always; a menu, a race or the shutdown path writes a number into it, and the next frame's tick
services exactly one request, then clears it back to 0. The actual state — "am I in a race?" — is the
**scene pointer `app+0x3cc`**: non-null exactly while a world is loaded. The two gameplay singletons
(`MXGameManager` `0x004e1e98`, `MXActivityManager` `0x004e1df8`) are null outside gameplay and the UI's
in-game flag `0x004e2650` mirrors `app+0x3cc != 0`.

The requests, with the transition each performs:

| req | transition | handlers | proved live |
|-----|------------|----------|-------------|
| 1 | build the world and its activity | `0x00202940` | yes — blocking: frame counter frozen at `0x133e` for ~19 s |
| 2 | gameplay → MainMenu | `0x00211290` `0x00202010` `0x00202190` `0x00202da0` `0x00202300` `0x00202270` | yes — scene → 0, in-game flag → 0, `app+0x580` → 1; also blocking (frozen at `0x160c`) |
| 3 | — | — | does not exist: no case, and nothing in the image writes 3 |
| 4 | the initial memory-card check → MainMenu | `0x0022bd90` `0x002027b0("Initial")` then the three "Main" ones | **no** — needs the `InitialMemCardCheck` page, which is already gone by the time PINE is usable |
| 5 | rebuild the front-end UI in place | `0x00202190` `0x00202300` `0x00202270` | yes — done in ~5 frames, MXUIManager pointer unchanged, no world touched |
| 6 | MainMenu → gameplay | `0x002027b0("Main")` `0x002b0850` `0x00202940` | yes — scene, both singletons and the in-game flag appear; screenshot shows a live race |
| 7 | restart the current event | `0x00202da0`, then sets the request to 1 | yes — scene → 0, then request reads 1, then the world is back |
| 8 | shut the game down | `0x00203160`, returns 0 | partly — see *Not verified* |

Cases 4 and 6 arm a two-frame delay first (`app+0x578` set, `app+0x57c` counting); cases 2 and 7 instead
wait on the UI manager's "still loading" pair (`uiMgr+0x100`, `**(uiMgr+0x104)`). That is the refinement R3
asked for: R3 wrote "the frame keeps ticking while a level loads", and that is true only of those *guards* —
the frame does **not** tick inside `0x00202940` or case 2's teardown, which stop the counter dead for
seconds at a time. Anything in the port that assumes a monotonic frame clock across a mode change is wrong.

### Which mode the game boots into

A second, much simpler variable: **`app+0x408`**, the StartMode, set once from the `.cfg` by
`0x00203a70` and branched on by `MXGame::Start` (`0x002017c0`, app slot 0x28):

| `.cfg` StartMode | `app+0x408` | what Start does |
|---|---|---|
| `FrontEndUI` | 0 | build the front end: `build_front_end("Initial")` then `("Main")` |
| `WaypointEditor` | 1 | create a bare `MXScene` (0x248 bytes) at `app+0x3cc`, no activity |
| `None` / `Practice` / `SingleRace` / `GhostRace` / `FastLapAttack` / `FreeRide` / `Freestyle` | 2 | `0x00204130` then `enter_world()` — straight into a race, no menus |

For StartMode 2 the name also selects the activity: it writes `event+4` (the event struct is
`settings+0xc`) to 0 FreeRide, 1 Practice/None, 2 SingleRace, 3 GhostRace, 4 FastLapAttack, 5 Freestyle.
`Freestyle` additionally sets `event[0] = 2`.

`MXGame::Configure` (`0x00203260`, app slot 0x20) is the full `.cfg` key list, which is a free description
of every knob the original had: `BuildTrackResFiles`, `BuildUIResFiles`, `TrackName`, `NumberOfRacers`,
`NumberOfLaps`, `ViewportMode` (Single/Horizontal/Vertical), `StartMode`, `BikeNumber`,
`EnableViewportSwitchTest`, `VehicleTuningMode`, `EnableMotocrossWrecks`, `AllAI`, `EnableAIBowling`,
`Sync` (60Hz/30Hz/NoSync), `SpeakerMode` (Stereo/Mono/Swapped/Dolby), `PlayMusic`, `PhysicsModel`,
`UseFrontEndUITestPage`, `SkipFrontEndUIVideoPages`, `AttractModeEnabled`.

### What a mode is made of

The modes the task line guessed at — title, menu, loading, in-game, replay, results — are **not** states of
this machine. The machine has exactly two destinations, the front end and a loaded world, and everything
finer than that lives one level down:

- **Title / menu / results / pause** are *pages* in `MXUIManager` (`app+0x3d0`), which never dies. Pages
  are addressed by name through a hash table at `0x004e1e40`: groups `MXFrontEnd` and `MXInGame`, pages
  `InitialMemCardCheck`, `MainMenu`, `TestPage`, `PauseMenu`, `RaceResultsMenu`,
  `SinglePlayer`/`OverlaysSinglePlayer`. The front-end machine is data-driven by the `.res` packs, which is
  why AGENT.md gets to call menus "yours to design" — there is no code-shaped state machine to copy.
- **Loading** is not a state either: it is `MXLoading` (`app+0x460`) raised and dropped *inside* one mode
  handler, with a kind at +0x34 (1 front end, 2 gameplay, 4 the memory-card check).
- **In-game** is the scene plus `MXGameManager::Init` (`0x0031a270`), which builds five children: the VCR
  (`0x003e9f00` — that is where replay lives), the track (`0x0026f4d0`), two managers (`0x00245880`,
  `0x0021a780`) and `MXActivityManager` (`0x0021b460`).
- **The race type** is an *activity*, made by name by `MXActivityManager::Create` (`0x0021b880`):
  `Supercross`/`Nationals` → `0x002f9b70` (0x4e0 B), `Ghost` → `0x0031a510`, `HitMaker`/`HitFreeride` →
  `0x00330460`, `HitProgression` → `0x0033cbb0`, `HitFastLap` → `0x00349a50`, `Run1`/`Run2` →
  `0x00337600`, `MachineRace` → `0x003a21b0`, `StuntCompetition` → `0x003dd340`, anything else → the base
  class `0x002794f0`.

This is the map the rest of the port navigates by, and it is flatter than expected: **one mailbox, two
destinations, a page machine and an activity factory.**

### The app object, as far as R4 needed it

`app = 0x00479990` (R3). Offsets confirmed by reading them live both at the menu and in a race:

```
+0x32c phase (R3)        +0x374 frame counter      +0x3c4 dispatch table -> 0x00452370
+0x3cc MXScene*          +0x3d0 MXUIManager*       +0x3d4 MXSettings*
+0x3d8..+0x3ec  game-data objects, built on demand by 0x00203dc0
+0x3f0 MXProfile list    +0x3fc MXSettings(0x9c)   +0x400 audio/media
+0x404 MODE REQUEST      +0x408 START MODE         +0x40c TrackName[0x40]
+0x44c BikeNumber        +0x450 resource loader    +0x454 quit flag
+0x458/+0x45c Build*ResFiles                       +0x460 MXLoading*
+0x570 PlayMusic         +0x574 VehicleTuningMode
+0x578/+0x57c two-frame delay for requests 4 and 6
+0x580 front-end-built   +0x584 ? (cleared by request 6)   +0x588 EnableAIBowling
```

`kit.toml` is git-ignored, so the names this task delivers are repeated here as the durable copy. The
`[symbols]` block added is: `app`, `app_dispatch`, `main_loop`, `engine_tick`, `render_frame`,
`game_mode_request`, `game_start_mode`, `game_quit_flag`, `mode_enter_world`, `mode_leave_world`,
`mode_build_front_end`, `mode_tear_down_front_end`, `mode_reset_viewports`, `mode_commit_event`,
`mode_shutdown`, `mode_loading_begin`, `mode_loading_end`, `mode_world_wait`, `mode_ui_flush`,
`game_configure`, `game_start`, `game_parse_start_mode`, `game_build_subsystems`, `game_scene`,
`ui_manager`, `game_settings`, `game_profiles`, `loading_screen`, `front_end_built`, `in_game_flag`,
`game_manager`, `activity_manager`, `build_target`, `frame_counter`, `frame_phase`, `activity_create`,
`activity_supercross`, `activity_ghost`, `activity_hit_maker`, `activity_hit_progression`,
`activity_hit_fast_lap`, `activity_run`, `activity_machine_race`, `activity_stunt_competition`,
`game_manager_init`, `vcr` — with the request→handler table in a comment above them.

## How it was proved

**Live perturbation on PCSX2 copy 2**, game booted to its main menu, one PINE connection, each run a single
`./kit pcsx2 with -- …` job: write a value to `@game_mode_request` (`0x00479d94`), then poll
`@game_scene`, `@game_manager`, `@activity_manager`, `@in_game_flag`, `@front_end_built` and
`@frame_counter` until they settle, and take a screenshot.

| wrote | observed |
|---|---|
| 6 | request clears; scene, `MXGameManager` and `MXActivityManager` become non-null; in-game flag → 1; screen shows a race, LAP 1/5 (`context/shots/r4-after-request-6.png`) |
| 7 | scene → 0, request reads 1 on the next poll, then the world is back — the chain in case 7 is real |
| 1 | frame counter frozen at `0x133e` for ~19 s, then advances again with a world loaded — case 1 blocks the loop |
| 2 | scene → 0, in-game flag → 0, `front_end_built` → 1; frame frozen at `0x160c` during it; screen shows the main menu, Career…Training (`context/shots/r4-after-request-2.png`) |
| 5 | front end rebuilt in ~5 frames, `@ui_manager` unchanged — it is a page rebuild, not a restart |
| 8 | screen goes black (`context/shots/r4-after-request-8.png`), then the EE stops — see below |

Corroborated R3's open question on the way past: `0x0046a808` (the 30 fps cap, set by the MXGame
constructor) reads 1 live, and the frame counter advanced 30–32 per wall-clock second.

Screenshots are under `context/` and are **not** committed, per AGENT.md.

R4 adds no code, so `./kit check` is unchanged by this task; it was run to confirm that.

## Not verified / not 1:1

- **Request 4 was never exercised live.** It transitions from the `InitialMemCardCheck` page, which the
  game has already left by the time it is far enough booted for PINE to be useful; forcing it from the
  main menu would run `tear_down_front_end("Initial")` against a page group that is not up. Its handler
  list is from the decompilation only. Unblocked by: a save state taken on the memory-card screen.
- **Request 8's ending is not understood.** After the request cleared, the frame counter kept rising for
  ~273 more frames; then everything froze, including the Timer-0 tick at `0x0047b190`, the screen stayed
  black, and a later read of `app+0x404` came back as 2. "Returns 0, so `main`'s loop ends, so
  `app->iface[0x6c]` runs and the EE halts" is consistent with the freeze but does not explain the 273
  frames or the 2. Nothing downstream in R4 depends on it; a port only needs "request 8 means exit".
- **Request 2's and request 1's blocking durations are wall-clock observations, not measurements.** The
  frame counter was polled over PINE, so the ~19 s is accurate to a poll interval and the "~2 s" figure for
  request 2 was dropped rather than quoted.
- **`0x004e2240` is asserted to be always 0** from "17 read sites, 0 write sites in the image" plus one
  live read. If some loader patches it, every `== 2` / `== 3` branch I called dead is live. Cheap to
  re-check later with a watch on it.
- **`app+0x584` is unexplained.** Request 6 clears it; nothing I read sets it.
- **Three activity classes are unnamed**: `0x00330460` (HitMaker/HitFreeride), `0x0033cbb0`
  (HitProgression), `0x003dd340` (StuntCompetition). They are below the file-attribution map's coverage
  (they do not allocate with `__FILE__`, and they fall in interpolation gaps), so I have their factory
  names and sizes but not their source files.
- **The event struct is only decoded at +0x4** (activity type) and +0x114 (a byte, player count). The rest
  of it is the next task's problem.
- **The page/layer machine is mapped to its entry points, not decoded.** `PauseMenu`, `RaceResultsMenu`,
  the VCR/replay path: I know which object owns them and which name reaches them, not their internal
  states. Deliberate — AGENT.md makes the front end "yours to design", and replay gets its own task.
- **No oracle run and no `./kit record` capture.** Neither applies: `MXGame::Tick` is not a pure function
  of its arguments, and there is no port code yet to compare a recording against.
- **The source-file names (MXGame.cpp, MXUIManager.cpp, …) are inferred**, not read from a symbol table:
  direct `__FILE__` evidence for the functions that allocate, linker-contiguity interpolation for the rest.
  A function in an interpolated gap may be attributed to its neighbour's file.

## Next

R5 and onwards can take the two destinations as given. The first thing anyone porting the loop should know
is in the second paragraph of *What the original does*: `app+0x404` is a mailbox, not a state, and the
frame counter stops while a request is serviced.
