# 1 — r7 freeride boot (FINAL)

Task R7. 2026-10-15.

## What I did

Drove the retail disc from a cold boot into a loaded, rideable freeride area over PINE, with no front
end, no menu input and no human save state, and named everything the boot path touches:

- the freeride track resources (`Tracks\free06.res` … and the `_2` / `_ss` variants) and the code that
  builds those names,
- the `HitMaker` / `HitFreeride` activity class R4 left unnamed (ctor at `0x00330460`), its size, its
  name string, its vtables and the factory slot it lands in,
- the `NumberOfRacers` and `AllAI` knobs that keep opponents out, both as `.cfg` keys and as the RAM
  words they write,
- and the league × activity-type table that decides whether a track can load at all.

The names are in `research/oracle.txt` under `# R7` and the live-RAM subset is in `kit.toml`'s
`[symbols]` (git-ignored, so oracle.txt is the committed copy).

**The brief's premise needs one correction, and it is the whole trick of this task.** `StartMode =
FreeRide` really does set app+0x408 = 2 and event+4 = 0 — that is `./kit fn 0x00203a70`, the StartMode
name parser, and it is what R4 recorded. But on a *freeride area* those two words do not load anything:
the activity-set lookup (`./kit fn 0x00207580`) maps the event's league (event+0) and type (event+4) to
an activity-set name, and for league 2 (the free tracks) only type 5 has a name. Types 0/1/2 with league
2 produce an empty name, the lookup returns 0 and the loader gives up on the loading screen — which is
exactly what I saw for two minutes with the frame counter frozen at 1.

The game's own autodetect agrees: `./kit fn 0x00203c70` takes the `TrackName` key, lowercases it, and
for a name containing `free` sets app+0x408 = 2, event+0 = 2 **and event+4 = 5**, plus the freestyle
course fields and `NumberOfRacers = 1`. So the two words that boot a freeride area are
`app+0x408 = 2` and `event+4 = 5`, with `event+0 = 2`; `StartMode = FreeRide` (type 0) is for riding a
*supercross or nationals* track with no race on it.

## What the original does

App object at `0x00479990`; the event the whole boot reads is `*( *(app+0x3d4) + 0xc )`.

**Boot-time configuration** — `./kit fn 0x00203260` (`MXGame::Configure`) reads a `.cfg` through a
key-value reader, every key gated on a section that the retail disc does not ship, so on retail every
key keeps its constructor default. The keys this task cares about:

| key | where it writes | default |
| --- | --- | --- |
| `TrackName` | app+0x40c, then copied into event+8 | `NONE` |
| `StartMode` | app+0x408 (+ event+4) via `./kit fn 0x00203a70` | `FrontEndUI` |
| `NumberOfRacers` | `*(app+0x3d4) + 0x00` — the settings object's first word | 1 |
| `AllAI` | `0x004e06b8` | 0 |
| `NumberOfLaps` | event+0x115 | |
| `BikeNumber` | app+0x44c | |
| `EnableAIBowling` | app+0x588 | |

`StartMode` names and what each one writes (0x408, then event):
`FrontEndUI` → 0 · `WaypointEditor` → 1 · `Freestyle` → 2, league 2, type 5 · `None` → 2, type 1 ·
`Practice` → 2, type 1 · `SingleRace` → 2, type 2 · `FastLapAttack`/`fastlap` → 2, type 4 ·
`GhostRace` → 2, type 3 · `FreeRide` → 2, type 0.

`Configure` then always calls the TrackName autodetect (`./kit fn 0x00203c70`), which lowercases the
name and matches substrings: `UI` → StartMode 0 · contains `free` → StartMode 2, league 2, type 5,
event+0x11c = the freestyle course record's +0x24, event+0x120 = 1, event+0x124 = 0 · `nat` → league 1,
type 1 · `sx` → league 0, type 1. Whatever the name, its last store is `NumberOfRacers = 1`.

**Loading a world.** `./kit fn 0x00207580` clears the live activities, then switches on event+4:
0/1/2 → `Nationals` if event+0 == 1, `Supercross` if event+0 == 0, **nothing if event+0 == 2** ·
3 → `Ghost` · 4 → `FastLap` · 5 → `Freestyle`. With a non-empty name it loads `<prefix><name>`, trying
a runtime prefix first and then `Tracks\`; an empty name returns 0 and the caller treats that as a
failed load. That activity-set resource is what asks the activity factory for `HitMaker`/`HitFreeride`.

`./kit fn 0x00205c40` is the track loader proper. It builds, with `%s%s.res` and the prefix `Tracks\`:
`Tracks\<TrackName>.res` and `Tracks\<TrackName>_2.res`, and for a split-screen freestyle session the
`%s_SS` variant first, i.e. `Tracks\<TrackName>_SS.res` / `_SS_2.res`. The archive on the disc holds
exactly that spelling, lowercased, for five freeride areas: `free06`, `free07`, `free09`, `free10`,
`free11` — each as `.res`, `_2.res`, `_ss.res`, `_ss_2.res`, with a `UI\free<nn>.lsi` loading screen.
The ELF itself only names two of them, `Free10` and `Free06`.

**The activity class.** `./kit fn 0x0021b880` is the activity factory: a chain of name comparisons, and
for `HitMaker` or `HitFreeride` it does `operator new(0x1a0, "MXActivityManager.cpp", 203)`, runs the
constructor at `0x00330460`, stores the object at `*(activity_mgr+0x58) + 0xc`, and calls the small
init at `0x00330620`. The constructor takes the class name `HitFreeride` (0x0043d850), installs three
vtables (+0x44 = 0x00459da0, +0x58 = 0x00459dec, +0x17c = 0x00459e60), zeroes a 7-word block at +0x160,
sets +0xb4 = 3, +0xb0 = 1, +0x144 = 0.5f and or's 0x14000 into the flags at +0xac.

Its interesting virtual is `./kit fn 0x00330630`, the per-actor attach: for a player vehicle it binds
the `RaceStatusVehicleInterface` and `MXVehicleAIInterface` components, and then compares the current
track name (`*(activity_mgr+0x50) + 0x140`) against `Free10` and `Free06` — if it is neither, it sets
bit 0x40 on the AI interface's flags at +0x30; on the two named areas it clears it. So the two freeride
areas the shipped game exposes are special-cased by name inside the hit/stunt activity itself.

## How it was proved

Live, on the original, from a cold boot of the retail disc (PCSX2 copy 4, `./kit pcsx2 with --`).

The script connects over PINE as soon as the socket appears and, every 20 ms until a scene exists,
writes: `app+0x408 = 2`; `event+0 = 2`; `event+4 = 5`; `event+8 = "Free06"` (NUL-terminated);
`*(app+0x3d4) + 0 = 1` (NumberOfRacers); `event+0x114 = 1` (players); `0x004e06b8 = 0` (AllAI); and
`event+0x10c = 2`, `event+0x110 = 0` (250cc, Race). It stops at the first non-null scene pointer. No
pad input is sent before the world exists, and no save state is loaded.

Result, twice, from two separate cold boots:

- `Free06`: scene at +48 s (after the intro movies), `in_game = 1`, frame counter running at 30/s,
  activity slot `*(activity_mgr+0x58)+0xc` populated — the `HitFreeride` object.
  `context/shots/r7-freeride-boot.png`: the bike on the ground in the pit area, freestyle HUD (score,
  compass, speedo).
- `Free10`: scene at +3.8 s, same state, `context/shots/r7-free10-boot.png` — bike on the ground.

Rideable, not just loaded: holding cross for 2.6 s on the `Free06` boot moved the bike —
`context/shots/r7-freeride-ride.png` shows the terrain and compass turned over and the odometer at
`00000.1`, against `00000.0` in the first shot.

The contrast case is proof of the league × type table as much as the decompilation is: the same script
with `event+4 = 0` (the brief's `FreeRide`) on the same area never leaves the loading screen — frame
counter stuck at 1, 32 MiB of RAM unchanged over two minutes except the interrupt counters, vsync still
60/s, no response to the pad. A return-address scan of EE RAM in that state put the stalled call inside
`0x00205c40 + 0x3f0 → 0x00207580 + 0x40`, i.e. exactly the empty-name path above.

Addresses: `research/oracle.txt` `# R7`. Screenshots are under `context/` and not committed.

## Not verified / not 1:1

- **The freestyle course fields are left at their defaults.** The original's own autodetect sets
  event+0x11c from the freestyle course record (`./kit fn 0x003712c0` on game-data+0x1c), event+0x120 =
  1 and event+0x124 = 0; my boot leaves 6 / 0 / 4. The area loads and rides, the freestyle HUD scores,
  but anything keyed on the course record (target lists, best-trick tables) may not match a boot the
  original configured itself. Reason: reproducing it needs the record lookup, which I cannot call over
  PINE, and the disc ships no `.cfg` to make the game call it.
- **`StartMode = FreeRide` (event+4 = 0) is proved not to load a freeride area, not proved to load
  anything.** I did not run the case it is presumably for — league 0 or 1 with type 0, i.e. riding
  `SX01` with no race. Reason: time; the task only needs a freeride area.
- **Only `Free06` and `Free10` were booted.** `free07`, `free09` and `free11` exist on the disc with a
  full set of `.res` files and a loading screen, but nothing in the ELF names them and I did not boot
  them. Reason: time.
- **The `_SS` split-screen variants are read from the loader's format strings, not observed.** No
  split-screen freestyle session was booted.
- **The `.res` names are the archive's spelling, lowercased** (`Tracks\free06.res`); the game builds
  them from the event's `TrackName`, which is mixed case (`Free06`), so the lookup is case-insensitive
  somewhere I did not chase.
- **`AllAI` is named from `MXGame::Configure`'s key only.** The retail disc has no `.cfg`, so the key
  never fires; I write `0x004e06b8 = 0` to match its default and did not test `AllAI = 1`. Nothing in
  the xref index references that word, so its readers are reached gp-relative or through a table.
- **`NumberOfRacers` is the settings object's first word, proved by the writes into it, not by watching
  it spawn fewer riders.** On a freeride area there are no opponents to count either way.
- **Not ported.** This task produced names and a repeatable boot recipe; no Rust changed, so there is
  nothing here for `./kit check` to test beyond the existing suite.
- **The boot timing is not deterministic.** Scene arrival varied from 3.8 s to 48 s between the two
  runs, because the intro movies are skipped or not depending on how far the emulator got before the
  script connected. A capture harness should wait on the scene pointer, not on a frame count.

## Next

If a later task wants a byte-identical freeride boot rather than an equivalent one, make the game do
its own derivation: write `TrackName` into app+0x40c *before* `MXGame::Configure` runs, so the
autodetect at `0x00203c70` sets the league, the type and the freestyle course fields itself. That
window is very early — my script never connected in time — so it probably needs a breakpoint or a
patched boot rather than PINE polling.
